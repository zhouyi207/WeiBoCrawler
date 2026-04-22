import { useEffect, useRef, useState } from "react";
import {
  CircleCheckIcon,
  Loader2Icon,
  PlusIcon,
  ScanLineIcon,
  SmartphoneIcon,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectLabel,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { AlertCircleIcon } from "lucide-react";
import { toast } from "sonner";
import {
  PLATFORMS,
  PLATFORM_LABELS,
  type Platform,
  type ProxyIp,
} from "@/features/domain/types";
import {
  generateLoginQr,
  listProxies,
  pollWeiboQrLogin,
} from "@/services/tauri/commands";
import { formatProxyEndpoint } from "@/views/ip/utils/format";

const POLL_MS = 2000;

/** 根据微博返回的 `msg` 粗判是否已进入「已扫码、待手机确认」阶段 */
function isLikelyScannedPhase(message: string | undefined): boolean {
  if (!message?.trim()) return false;
  return /确认|成功|已扫|扫描|手机|authorize|scan/i.test(message);
}

function defaultProxyId(proxies: ProxyIp[]): string {
  return proxies[0]?.id ?? "";
}

export function AddAccountDialog({
  onAccountsChanged,
}: {
  onAccountsChanged?: () => void | Promise<void>;
} = {}) {
  const [open, setOpen] = useState(false);
  const [platform, setPlatform] = useState<Platform>("weibo");
  const [proxies, setProxies] = useState<ProxyIp[]>([]);
  /** 选中的 `proxies.id` */
  const [ipBinding, setIpBinding] = useState<string>("");
  const [qrReady, setQrReady] = useState(false);
  const [qrData, setQrData] = useState("");
  const [qrSessionAccountId, setQrSessionAccountId] = useState<string | null>(
    null
  );
  /** 微博扫码：等待扫码 → 已扫码待确认 → 成功 / 失败 */
  const [qrPhase, setQrPhase] = useState<
    "idle" | "await_scan" | "await_confirm" | "success" | "failed"
  >("idle");
  const [pollHint, setPollHint] = useState("");
  const [pollError, setPollError] = useState("");
  const [genError, setGenError] = useState("");
  /** 「生成」按钮按下到后端返回 qr_data 之间的 inflight 状态 */
  const [generating, setGenerating] = useState(false);
  const pollTimerRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const handleGenerateRef = useRef<() => Promise<void>>(() => Promise.resolve());

  function clearPollTimer() {
    if (pollTimerRef.current !== null) {
      clearInterval(pollTimerRef.current);
      pollTimerRef.current = null;
    }
  }

  function resetQrState() {
    clearPollTimer();
    setQrReady(false);
    setQrData("");
    setQrSessionAccountId(null);
    setQrPhase("idle");
    setPollHint("");
    setPollError("");
    setGenError("");
    setGenerating(false);
  }

  function handleOpenChange(next: boolean) {
    setOpen(next);
    if (next) {
      clearPollTimer();
      setQrReady(false);
      setQrData("");
      setQrSessionAccountId(null);
      setQrPhase("idle");
      setPollHint("");
      setPollError("");
      setGenError("");
      setPlatform("weibo");
      setProxies([]);
      setIpBinding("");
      void (async () => {
        try {
          const list = await listProxies();
          setProxies(list);
          setIpBinding(defaultProxyId(list));
        } catch {
          setProxies([]);
          setIpBinding("");
        }
      })();
    } else {
      clearPollTimer();
      setQrReady(false);
      setQrData("");
      setQrSessionAccountId(null);
      setQrPhase("idle");
      setPollHint("");
      setPollError("");
      setGenError("");
      setPlatform("weibo");
      setIpBinding("");
      setProxies([]);
      void onAccountsChanged?.();
    }
  }

  useEffect(() => {
    if (
      !open ||
      !qrReady ||
      !qrSessionAccountId ||
      platform !== "weibo" ||
      !qrData
    ) {
      clearPollTimer();
      return;
    }

    const accountId = qrSessionAccountId;

    async function tick() {
      try {
        const res = await pollWeiboQrLogin(accountId);
        const hint = res.message?.trim();

        if (res.status === "waiting") {
          if (hint) setPollHint(hint);
          if (isLikelyScannedPhase(res.message)) {
            setQrPhase("await_confirm");
          } else {
            setQrPhase("await_scan");
          }
          return;
        }

        if (res.status === "success") {
          clearPollTimer();
          setQrPhase("success");
          if (res.mergedIntoAccountId) {
            setPollHint(
              `该微博已绑定到已有账号（${res.mergedIntoAccountId.slice(0, 8)}…），未新建重复行。`,
            );
          } else {
            setPollHint("账号已写入数据库。");
          }
          void onAccountsChanged?.();
          return;
        }

        if (res.status === "failed") {
          clearPollTimer();
          setQrPhase("failed");
          setPollError(res.message?.trim() || "扫码登录失败");
        }
      } catch (e) {
        clearPollTimer();
        setQrPhase("failed");
        setPollError(e instanceof Error ? e.message : String(e));
      }
    }

    void tick();
    pollTimerRef.current = setInterval(() => void tick(), POLL_MS);
    return () => {
      clearPollTimer();
    };
  }, [open, qrReady, qrSessionAccountId, platform, qrData, onAccountsChanged]);

  async function handleGenerate() {
    setGenError("");
    clearPollTimer();
    setQrSessionAccountId(null);
    setQrReady(false);
    setQrData("");
    setQrPhase("idle");
    setPollHint("");
    setPollError("");
    if (!ipBinding.trim()) {
      toast.error("请选择代理");
      return;
    }
    setGenerating(true);
    try {
      const res = await generateLoginQr({
        platform,
        ipId: ipBinding.trim(),
      });
      setQrData(res.qrData);
      setQrSessionAccountId(res.accountId);
      setQrReady(true);
      if (platform === "weibo" && res.qrData) {
        setQrPhase("await_scan");
        setPollHint("等待使用微博客户端扫码…");
      } else {
        setQrPhase("idle");
      }
    } catch (e) {
      setGenError(e instanceof Error ? e.message : String(e));
    } finally {
      setGenerating(false);
    }
  }

  handleGenerateRef.current = handleGenerate;

  const showWeiboPollUi =
    platform === "weibo" && qrReady && !!qrData && !!qrSessionAccountId;

  const canGenerate = proxies.length > 0 && ipBinding.trim().length > 0;

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogTrigger asChild>
        <Button>
          <PlusIcon className="size-4" />
          添加账号
        </Button>
      </DialogTrigger>
      <DialogContent className="gap-4 sm:max-w-md">
        <DialogHeader>
          <DialogTitle>添加账号</DialogTitle>
          <DialogDescription>
            选择平台与代理出口；生成后由后端返回二维码用于登录授权（请求经所选代理发出）。
          </DialogDescription>
        </DialogHeader>

        <div className="grid min-w-0 gap-4">
          <div className="grid gap-2">
            <Label htmlFor="add-account-platform">平台</Label>
            <Select
              value={platform}
              onValueChange={(v) => {
                if (v === platform) return;
                resetQrState();
                setPlatform(v as Platform);
              }}
            >
              <SelectTrigger id="add-account-platform" className="w-full">
                <SelectValue placeholder="选择平台" />
              </SelectTrigger>
              <SelectContent>
                {PLATFORMS.map((p) => (
                  <SelectItem key={p} value={p}>
                    {PLATFORM_LABELS[p]}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          <div className="grid gap-2">
            <Label htmlFor="add-account-ip">代理出口</Label>
            <Select
              value={ipBinding}
              onValueChange={(v) => {
                if (v === ipBinding) return;
                resetQrState();
                setIpBinding(v);
              }}
              disabled={proxies.length === 0}
            >
              <SelectTrigger id="add-account-ip" className="w-full">
                <SelectValue
                  placeholder={
                    proxies.length === 0 ? "请先在 IP 管理中添加代理" : "选择代理"
                  }
                />
              </SelectTrigger>
              <SelectContent>
                <SelectGroup>
                  <SelectLabel>代理库</SelectLabel>
                  {proxies.length === 0 ? (
                    <SelectLabel className="text-muted-foreground">
                      暂无代理，请在「IP 管理」中添加（可将本机配置为 HTTP/SOCKS5 代理后在此选用）
                    </SelectLabel>
                  ) : (
                    proxies.map((ip) => (
                      <SelectItem key={ip.id} value={ip.id}>
                        {formatProxyEndpoint(ip.address)}
                        {ip.remark ? ` · ${ip.remark}` : ""}
                      </SelectItem>
                    ))
                  )}
                </SelectGroup>
              </SelectContent>
            </Select>
          </div>

          <Button
            type="button"
            className="w-full"
            onClick={() => void handleGenerate()}
            disabled={generating || !canGenerate}
          >
            {generating ? (
              <>
                <Loader2Icon className="size-4 animate-spin" />
                生成中…
              </>
            ) : (
              "生成"
            )}
          </Button>

          {generating ? (
            <div className="flex items-center justify-center gap-2 rounded-lg border border-dashed border-muted-foreground/40 bg-muted/30 px-4 py-6 text-sm text-muted-foreground">
              <Loader2Icon className="size-4 animate-spin" />
              已发送二维码请求，请稍候…
            </div>
          ) : genError ? (
            <Alert variant="destructive" className="min-w-0">
              <AlertCircleIcon />
              <AlertTitle>生成二维码失败</AlertTitle>
              <AlertDescription>
                <p className="w-full break-all whitespace-pre-wrap text-xs">
                  {genError}
                </p>
              </AlertDescription>
            </Alert>
          ) : (
            qrReady && (
              <div className="grid min-w-0 gap-2">
                <div
                  role="img"
                  aria-label="登录二维码"
                  className="flex w-full flex-col items-center justify-center gap-2 rounded-lg border border-dashed border-muted-foreground/40 bg-muted/30 p-4 text-center"
                >
                  {qrData ? (
                    <img
                      src={qrData}
                      alt="微博登录二维码"
                      className="size-40 object-contain"
                    />
                  ) : (
                    <>
                      <div className="size-40 rounded-md bg-muted/80" />
                      <p className="text-xs text-muted-foreground">
                        当前平台暂无二维码（或非微博）
                      </p>
                    </>
                  )}
                </div>

                {showWeiboPollUi ? (
                  <div className="grid min-w-0 gap-2">
                    {qrPhase === "await_scan" ? (
                      <Alert className="min-w-0">
                        <ScanLineIcon />
                        <AlertTitle>等待扫码</AlertTitle>
                        <AlertDescription>
                          <p className="w-full break-all text-xs">
                            {pollHint || "请使用微博客户端扫描上方二维码。"}
                          </p>
                        </AlertDescription>
                      </Alert>
                    ) : null}

                    {qrPhase === "await_confirm" ? (
                      <Alert className="min-w-0 border-blue-300 bg-blue-50 text-blue-900 dark:border-blue-900/60 dark:bg-blue-950/30 dark:text-blue-100">
                        <SmartphoneIcon />
                        <AlertTitle>已扫码，待手机确认</AlertTitle>
                        <AlertDescription className="text-blue-800/80 dark:text-blue-200/80">
                          <p className="w-full break-all text-xs">
                            {pollHint ||
                              "请在手机微博中确认登录，确认后此处将自动切到「登录成功」。"}
                          </p>
                        </AlertDescription>
                      </Alert>
                    ) : null}

                    {qrPhase === "success" ? (
                      <Alert className="min-w-0 border-green-300 bg-green-50 text-green-900 dark:border-green-900/60 dark:bg-green-950/30 dark:text-green-100">
                        <CircleCheckIcon />
                        <AlertTitle>登录成功</AlertTitle>
                        <AlertDescription className="text-green-800/80 dark:text-green-200/80">
                          <p className="w-full break-all text-xs">
                            {pollHint || "账号已写入数据库，列表已刷新。"}
                          </p>
                        </AlertDescription>
                      </Alert>
                    ) : null}

                    {pollError ? (
                      <Alert variant="destructive" className="min-w-0">
                        <AlertCircleIcon />
                        <AlertTitle>登录失败</AlertTitle>
                        <AlertDescription>
                          <p className="w-full break-all whitespace-pre-wrap text-xs">
                            {pollError}
                          </p>
                        </AlertDescription>
                      </Alert>
                    ) : null}
                  </div>
                ) : null}
              </div>
            )
          )}
        </div>
      </DialogContent>
    </Dialog>
  );
}
