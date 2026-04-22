import { useCallback, useEffect, useState } from "react";
import { AlertCircleIcon, RefreshCwIcon } from "lucide-react";
import { toast } from "sonner";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Separator } from "@/components/ui/separator";
import { cn } from "@/lib/utils";
import type { Account, AccountLogEntry } from "@/features/domain/types";
import { listAccountLogs } from "@/services/tauri/commands";

/**
 * 账号风控档位 → Badge 文案。来自后端 `AccountStatus`，与 `AccountPage` 顶层
 * `RISK_STATUS_MAP` 同源；这里按 modal 的展示需要单独维护一份，避免跨文件耦合。
 */
const RISK_BADGE: Record<
  string,
  {
    label: string;
    variant: "default" | "secondary" | "destructive";
    hint: string;
  }
> = {
  normal: {
    label: "正常",
    variant: "default",
    hint: "近 5 分钟内归责到该账号的失败次数低于阈值。",
  },
  restricted: {
    label: "受限",
    variant: "secondary",
    hint: "近 5 分钟内出现 ≥5 次失败。可继续使用，连续 10 次成功后会自动回落。",
  },
  error: {
    label: "异常",
    variant: "destructive",
    hint: "近 5 分钟内 ≥3 次跳登录页 / Cookie 失效。任务调度会暂停该账号，需要重新扫码登录后才会恢复。",
  },
};

/**
 * 错误归因 → 友好文案。和 `IpLogModal.KIND_LABEL` 保持一致：账号失败事件的
 * `errorKind` 取值范围与代理事件完全相同（都来自 `risk::ErrorKind::as_tag`）。
 */
const KIND_LABEL: Record<string, string> = {
  network: "网络异常",
  http_status: "HTTP 状态",
  login_required: "登录失效",
  business_reject: "业务拒绝",
  other: "其他",
};

function formatOccurredAt(iso: string): string {
  // 后端 RFC3339（带时区），转本地时间字符串。失败回退到原文，避免 UI 因脏数据炸掉。
  try {
    const d = new Date(iso);
    if (Number.isNaN(d.getTime())) return iso;
    return d.toLocaleString();
  } catch {
    return iso;
  }
}

/**
 * 账号日志 modal。展示：
 * - 顶部：账号风控档位 Badge（直接读 `Account.riskStatus`，账号档位仍是持久字段，
 *   不像代理那样按事件实时派生）+ 账号用户名 / 平台；
 * - 中部：累计错误数 / 最近一次错误时间；
 * - 底部：最近 N 条失败事件流（来源：`account_failure_events`）。
 *
 * 与 `IpLogModal` 结构完全对称：业务规则 "IP 与账号仅在任务配置做笛卡尔积" 决定了
 * 这两个 modal 各自只看自己的事件流，不做反向关联。
 */
export function AccountLogModal({
  open,
  onOpenChange,
  account,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** 关闭时为 `null`，避免空打开。 */
  account: Account | null;
}) {
  const [logs, setLogs] = useState<AccountLogEntry[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");

  const refresh = useCallback(async () => {
    if (!account) return;
    setLoading(true);
    setError("");
    try {
      const res = await listAccountLogs(account.id, 100);
      setLogs(res);
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      setError(msg);
      toast.error("加载日志失败", { description: msg });
    } finally {
      setLoading(false);
    }
  }, [account]);

  useEffect(() => {
    if (open && account) {
      void refresh();
    } else if (!open) {
      // 关闭时清空，避免下次打开闪过上一条账号的旧数据。
      setLogs([]);
      setError("");
    }
  }, [open, account, refresh]);

  const riskBadge = account ? RISK_BADGE[account.riskStatus] : null;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      {/*
        布局策略与 IpLogModal 完全一致：
        - DialogContent: `max-h-[85vh]` + `overflow-hidden`，整体永不超过视口；
        - 三段式 flex 列：Header（不滚） / Body（统一滚动区） / Footer（不滚）；
        - Body 即 `flex-1 min-h-0 overflow-y-auto`，所有可变区块共用同一条滚动条。
      */}
      <DialogContent className="flex max-h-[85vh] flex-col gap-0 overflow-hidden p-0 sm:max-w-2xl">
        <DialogHeader className="shrink-0 border-b border-border/60 px-4 py-3">
          <DialogTitle className="flex items-center gap-2">
            <span>账号日志</span>
            {riskBadge && (
              <Badge variant={riskBadge.variant} title={riskBadge.hint}>
                {riskBadge.label}
              </Badge>
            )}
          </DialogTitle>
          <DialogDescription className="flex flex-wrap items-center gap-x-2 gap-y-1">
            <span className="font-mono break-all text-foreground/80">
              {account?.username ?? "—"}
            </span>
            {account?.platform && (
              <span className="text-muted-foreground">· {account.platform}</span>
            )}
          </DialogDescription>
        </DialogHeader>

        <div className="flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto px-4 py-3 pb-4">
          {error && (
            <Alert variant="destructive">
              <AlertCircleIcon />
              <AlertTitle>加载失败</AlertTitle>
              <AlertDescription className="break-words">
                {error}
              </AlertDescription>
            </Alert>
          )}

          <div className="grid grid-cols-2 gap-3 text-sm">
            <div className="min-w-0 rounded-md border p-2">
              <div className="text-xs text-muted-foreground">近窗口错误数</div>
              <div className="text-lg font-semibold">{logs.length}</div>
            </div>
            <div className="min-w-0 rounded-md border p-2">
              <div className="text-xs text-muted-foreground">最近一次</div>
              <div
                className="truncate text-sm"
                title={logs[0]?.occurredAt ?? ""}
              >
                {logs[0] ? formatOccurredAt(logs[0].occurredAt) : "—"}
              </div>
            </div>
          </div>

          <Separator />

          <div className="space-y-2">
            <div className="sticky top-0 z-10 flex items-center justify-between bg-popover/95 py-1 backdrop-blur-sm supports-backdrop-filter:bg-popover/85">
              <span className="text-sm font-medium">事件时间线</span>
              <Button
                variant="ghost"
                size="sm"
                onClick={() => void refresh()}
                disabled={loading}
              >
                <RefreshCwIcon
                  className={cn("size-3.5", loading && "animate-spin")}
                />
                刷新
              </Button>
            </div>

            <div className="rounded-md border">
              {loading && logs.length === 0 ? (
                <div className="p-6 text-center text-sm text-muted-foreground">
                  加载中…
                </div>
              ) : logs.length === 0 ? (
                <div className="p-6 text-center text-sm text-muted-foreground">
                  {/* 没有日志：要么真的没出过错，要么记录已被 24h 清理任务回收。 */}
                  近 24 小时内没有该账号的失败记录。
                </div>
              ) : (
                <ul className="divide-y">
                  {logs.map((log) => (
                    <li key={log.id} className="space-y-1 p-2.5 text-xs">
                      <div className="flex flex-wrap items-center gap-x-2 gap-y-1">
                        <Badge
                          variant={
                            log.errorKind === "login_required"
                              ? "destructive"
                              : "secondary"
                          }
                          className="shrink-0 text-[10px]"
                        >
                          {KIND_LABEL[log.errorKind] ?? log.errorKind}
                          {log.httpStatus != null && ` · ${log.httpStatus}`}
                        </Badge>
                        <span
                          className="shrink-0 text-muted-foreground"
                          title={log.occurredAt}
                        >
                          {formatOccurredAt(log.occurredAt)}
                        </span>
                      </div>
                      {log.message && (
                        <div className="break-all text-muted-foreground">
                          {log.message}
                        </div>
                      )}
                    </li>
                  ))}
                </ul>
              )}
            </div>
          </div>
        </div>

        <DialogFooter className="shrink-0 !m-0 flex-row justify-end gap-2 border-t bg-muted/30 px-4 py-3">
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            关闭
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
