import { useEffect, useMemo, useState } from "react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { AlertCircleIcon, EyeIcon, EyeOffIcon } from "lucide-react";
import { toast } from "sonner";
import { cn } from "@/lib/utils";
import type { ProxyIp } from "@/features/domain/types";
import { formatProxyEndpoint } from "./utils/format";
import { addProxy, updateProxy } from "@/services/tauri/commands";

type AddableProxyType = "HTTP" | "SOCKS5";

const TYPE_OPTIONS: { value: AddableProxyType; label: string; hint: string }[] = [
  {
    value: "HTTP",
    label: "HTTP / HTTPS",
    hint: "通过 HTTP CONNECT 隧道转发，适配大多数代理服务商。",
  },
  {
    value: "SOCKS5",
    label: "SOCKS5",
    hint: "更通用的代理协议，需要服务商显式支持。",
  },
];

/** 主机：IPv4 / IPv6（裸写或带方括号都可）/ 域名。不允许空格 / 中文。 */
function isLikelyValidHost(raw: string): boolean {
  const v = raw.trim();
  if (!v) return false;
  if (/\s/.test(v)) return false;
  if (/[\u4e00-\u9fa5]/.test(v)) return false;
  // IPv6 字面量（带或不带方括号）
  if (v.startsWith("[") && v.endsWith("]")) {
    return v.length > 2 && v.slice(1, -1).includes(":");
  }
  if (v.includes("::") || /^[0-9a-fA-F:]+$/.test(v) && v.includes(":")) {
    return true;
  }
  // IPv4 / 域名：a-z 0-9 . - _ 且不以点开头/结尾
  return /^[a-zA-Z0-9]([a-zA-Z0-9._-]*[a-zA-Z0-9])?$/.test(v);
}

function isLikelyValidPort(raw: string): boolean {
  if (!/^\d{1,5}$/.test(raw)) return false;
  const n = Number.parseInt(raw, 10);
  return n >= 1 && n <= 65535;
}

/** 把 host 包装成 URL 安全形式：IPv6 字面量需要方括号。 */
function bracketHostIfNeeded(host: string): string {
  const h = host.trim();
  if (h.startsWith("[")) return h;
  // 含两个及以上冒号视为 IPv6
  const colonCount = (h.match(/:/g) ?? []).length;
  return colonCount >= 2 ? `[${h}]` : h;
}

/**
 * 把表单字段拼回后端能识别的地址字符串：
 * - 无账号密码：返回 `host:port`，让后端按选择的代理类型自动补 scheme（与旧逻辑兼容）。
 * - 有账号密码：返回完整 URL `<scheme>://user:pass@host:port`，账号密码均做百分号编码，
 *   避免 `@` `:` `/` 等特殊字符破坏 URL 解析。
 */
function buildAddress(
  host: string,
  port: string,
  username: string,
  password: string,
  proxyType: AddableProxyType,
): string {
  const bracketed = bracketHostIfNeeded(host.trim());
  const hostPort = `${bracketed}:${port.trim()}`;
  const u = username.trim();
  const p = password; // 密码可能含前后空格，原样保留
  if (!u && !p) return hostPort;
  const scheme = proxyType === "SOCKS5" ? "socks5" : "http";
  const auth = `${encodeURIComponent(u)}${p ? `:${encodeURIComponent(p)}` : ""}`;
  return `${scheme}://${auth}@${hostPort}`;
}

/**
 * 反解析后端存的 `address`，把 host / port / username / password 拆回成表单字段，
 * 用于「编辑」模式预填。与 [`buildAddress`] 互逆，处理：
 * - `host:port` / `[ipv6]:port`
 * - `scheme://host:port` / `scheme://user:pass@host:port`（auth 段做百分号解码）
 * - 路径 / 查询参数会被丢弃（与 `formatProxyEndpoint` 保持一致）
 *
 * 解析失败 / 字段缺失时回退到空串，由调用方再走 form 校验提示用户补全。
 */
function parseAddress(address: string): {
  host: string;
  port: string;
  username: string;
  password: string;
} {
  let s = address.trim();
  const schemeIdx = s.indexOf("://");
  if (schemeIdx >= 0) s = s.slice(schemeIdx + 3);

  let username = "";
  let password = "";
  const atIdx = s.lastIndexOf("@");
  if (atIdx >= 0) {
    const auth = s.slice(0, atIdx);
    s = s.slice(atIdx + 1);
    const colonIdx = auth.indexOf(":");
    try {
      if (colonIdx >= 0) {
        username = decodeURIComponent(auth.slice(0, colonIdx));
        password = decodeURIComponent(auth.slice(colonIdx + 1));
      } else {
        username = decodeURIComponent(auth);
      }
    } catch {
      // 不是合法的 percent-encoding，按原文兜底（避免抛错）
      if (colonIdx >= 0) {
        username = auth.slice(0, colonIdx);
        password = auth.slice(colonIdx + 1);
      } else {
        username = auth;
      }
    }
  }

  s = s.split("/")[0]?.split("?")[0] ?? s;

  let host = "";
  let port = "";
  if (s.startsWith("[")) {
    const end = s.indexOf("]");
    if (end >= 0) {
      host = s.slice(1, end);
      const after = s.slice(end + 1);
      if (after.startsWith(":")) port = after.slice(1);
    } else {
      host = s;
    }
  } else {
    const lastColon = s.lastIndexOf(":");
    if (lastColon >= 0) {
      host = s.slice(0, lastColon);
      port = s.slice(lastColon + 1);
    } else {
      host = s;
    }
  }
  return { host, port, username, password };
}

/** 用于「地址预览」：只展示 `host:port`，不展示类型前缀与账号密码。 */
function buildAddressPreview(host: string, port: string): string {
  if (!host.trim() || !port.trim()) return "";
  const bracketed = bracketHostIfNeeded(host.trim());
  return `${bracketed}:${port.trim()}`;
}

export function AddProxyDialog({
  open,
  onOpenChange,
  initial,
  onSubmitted,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /**
   * `null` / `undefined`：新增模式；
   * 传入 `ProxyIp`：编辑模式——预填表单、调 `update_proxy`、文案改成「编辑 / 保存」。
   * 系统行（is_system）不应该走到这里，调用方应在外层 disable 入口。
   */
  initial?: ProxyIp | null;
  /** 提交成功后回调（新增 / 编辑均触发），调用方负责刷新列表或局部替换。 */
  onSubmitted?: (proxy: ProxyIp) => void | Promise<void>;
}) {
  const isEdit = Boolean(initial);
  const [host, setHost] = useState("");
  const [port, setPort] = useState("");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [showPassword, setShowPassword] = useState(false);
  const [proxyType, setProxyType] = useState<AddableProxyType>("HTTP");
  const [remark, setRemark] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState("");

  useEffect(() => {
    if (!open) return;
    // 每次打开重置：根据 initial 区分新增 / 编辑，避免上次 submit 的 error / 残留值
    // 影响下一次。Direct 系统行理论上不会走到这里，做个兜底退回 HTTP。
    if (
      initial &&
      (initial.proxyType === "HTTP" || initial.proxyType === "SOCKS5")
    ) {
      const parsed = parseAddress(initial.address);
      setHost(parsed.host);
      setPort(parsed.port);
      setUsername(parsed.username);
      setPassword(parsed.password);
      setProxyType(initial.proxyType);
      setRemark(initial.remark ?? "");
    } else {
      setHost("");
      setPort("");
      setUsername("");
      setPassword("");
      setProxyType("HTTP");
      setRemark("");
    }
    setShowPassword(false);
    setError("");
    setSubmitting(false);
  }, [open, initial]);

  const hostInvalid = host.length > 0 && !isLikelyValidHost(host);
  const portInvalid = port.length > 0 && !isLikelyValidPort(port);

  const preview = useMemo(
    () => buildAddressPreview(host, port),
    [host, port],
  );

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (!host.trim()) {
      setError("请填写主机地址");
      return;
    }
    if (!isLikelyValidHost(host)) {
      setError("主机格式不合法，请填写 IP 或域名（不允许空格 / 中文）");
      return;
    }
    if (!port.trim()) {
      setError("请填写端口");
      return;
    }
    if (!isLikelyValidPort(port)) {
      setError("端口需为 1-65535 的整数");
      return;
    }
    if (!username.trim() && password) {
      setError("提供密码时必须填写用户名");
      return;
    }
    setSubmitting(true);
    setError("");
    try {
      const address = buildAddress(host, port, username, password, proxyType);
      const payload = {
        address,
        proxyType,
        remark: remark.trim() ? remark.trim() : null,
      } as const;
      const result =
        initial != null
          ? await updateProxy({ id: initial.id, ...payload })
          : await addProxy(payload);
      toast.success(initial != null ? "代理已更新" : "代理添加成功", {
        description: formatProxyEndpoint(address),
      });
      onOpenChange(false);
      await onSubmitted?.(result);
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <form onSubmit={handleSubmit} className="space-y-4">
          <DialogHeader>
            <DialogTitle>{isEdit ? "编辑代理" : "添加代理"}</DialogTitle>
            <DialogDescription>
              {isEdit
                ? "修改地址会触发一次地理信息反查；运行期数据（状态 / 延迟 / 风控分）保留不变。"
                : "「本机直连」由系统自动维护，这里只用于添加 HTTP / SOCKS5 代理。"}
            </DialogDescription>
          </DialogHeader>

          <div className="space-y-3">
            <div className="space-y-1.5">
              <Label>代理类型</Label>
              <Select
                value={proxyType}
                onValueChange={(v) => setProxyType(v as AddableProxyType)}
              >
                <SelectTrigger className="w-full">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {TYPE_OPTIONS.map((o) => (
                    <SelectItem key={o.value} value={o.value}>
                      {o.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
              <p className="text-xs text-muted-foreground">
                {TYPE_OPTIONS.find((o) => o.value === proxyType)?.hint}
              </p>
            </div>

            <div className="grid grid-cols-[1fr_7rem] gap-3">
              <div className="space-y-1.5">
                <Label htmlFor="proxy-host">主机</Label>
                <Input
                  id="proxy-host"
                  value={host}
                  onChange={(ev) => setHost(ev.target.value)}
                  placeholder="103.24.68.12 或 proxy.example.com"
                  autoComplete="off"
                  spellCheck={false}
                  aria-invalid={hostInvalid || undefined}
                  className={cn(
                    hostInvalid && "border-destructive focus-visible:ring-destructive/40",
                  )}
                />
              </div>
              <div className="space-y-1.5">
                <Label htmlFor="proxy-port">端口</Label>
                <Input
                  id="proxy-port"
                  type="number"
                  min={1}
                  max={65535}
                  inputMode="numeric"
                  value={port}
                  onChange={(ev) => setPort(ev.target.value.replace(/\D/g, ""))}
                  placeholder="8080"
                  autoComplete="off"
                  aria-invalid={portInvalid || undefined}
                  className={cn(
                    portInvalid && "border-destructive focus-visible:ring-destructive/40",
                  )}
                />
              </div>
            </div>

            <div className="grid grid-cols-2 gap-3">
              <div className="space-y-1.5">
                <Label htmlFor="proxy-username">
                  用户名
                  <span className="ml-1 text-xs font-normal text-muted-foreground">
                    （可选）
                  </span>
                </Label>
                <Input
                  id="proxy-username"
                  value={username}
                  onChange={(ev) => setUsername(ev.target.value)}
                  placeholder="user"
                  autoComplete="off"
                  spellCheck={false}
                />
              </div>
              <div className="space-y-1.5">
                <Label htmlFor="proxy-password">
                  密码
                  <span className="ml-1 text-xs font-normal text-muted-foreground">
                    （可选）
                  </span>
                </Label>
                <div className="relative">
                  <Input
                    id="proxy-password"
                    type={showPassword ? "text" : "password"}
                    value={password}
                    onChange={(ev) => setPassword(ev.target.value)}
                    placeholder="pass"
                    autoComplete="new-password"
                    spellCheck={false}
                    className="pr-9"
                  />
                  <button
                    type="button"
                    onClick={() => setShowPassword((v) => !v)}
                    className="absolute inset-y-0 right-0 flex w-9 items-center justify-center text-muted-foreground hover:text-foreground"
                    aria-label={showPassword ? "隐藏密码" : "显示密码"}
                    tabIndex={-1}
                  >
                    {showPassword ? (
                      <EyeOffIcon className="size-4" />
                    ) : (
                      <EyeIcon className="size-4" />
                    )}
                  </button>
                </div>
              </div>
            </div>

            <div className="space-y-1.5">
              <Label htmlFor="proxy-remark">
                备注
                <span className="ml-1 text-xs font-normal text-muted-foreground">
                  （可选）
                </span>
              </Label>
              <Input
                id="proxy-remark"
                value={remark}
                onChange={(ev) => setRemark(ev.target.value)}
                placeholder="例如：主线路 · 华东"
                autoComplete="off"
                maxLength={120}
              />
            </div>

            <div className="rounded-md border bg-muted/40 px-2.5 py-2 text-xs">
              <span className="text-muted-foreground">地址预览：</span>
              <span className="ml-1 font-mono break-all text-foreground">
                {preview || "请填写主机与端口"}
              </span>
            </div>

            {error && (
              <Alert variant="destructive">
                <AlertCircleIcon />
                <AlertTitle>{isEdit ? "保存失败" : "添加代理失败"}</AlertTitle>
                <AlertDescription>{error}</AlertDescription>
              </Alert>
            )}
          </div>

          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              onClick={() => onOpenChange(false)}
              disabled={submitting}
            >
              取消
            </Button>
            <Button type="submit" disabled={submitting}>
              {submitting
                ? isEdit
                  ? "保存中…"
                  : "添加中…"
                : isEdit
                ? "保存"
                : "添加"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
