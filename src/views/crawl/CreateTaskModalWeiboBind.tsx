import { memo } from "react";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { cn } from "@/lib/utils";
import {
  AlertCircleIcon,
  InfoIcon,
  Loader2Icon,
  RefreshCwIcon,
  TriangleAlertIcon,
  XIcon,
} from "lucide-react";
import {
  LOCAL_DIRECT_PROXY_ID,
  PLATFORM_LABELS,
  type Account,
  type Platform,
  type ProxyHealthBrief,
  type ProxyIp,
} from "@/features/domain/types";
import { formatProxyEndpoint } from "@/views/ip/utils/format";

export type WeiboBindPoolsProps = {
  open: boolean;
  accounts: Account[];
  accountsLoading: boolean;
  accountsError: string;
  selectedAccountIds: string[];
  proxies: ProxyIp[];
  proxiesLoading: boolean;
  proxiesError: string;
  selectedProxyIds: string[];
  proxyHealth: Record<string, ProxyHealthBrief>;
  effectivePlatform: Platform;
  orphanAccountIds: string[];
  orphanProxyIds: string[];
  errorSelectedAccountCount: number;
  invalidSelectedProxyCount: number;
  onToggleAccount: (id: string) => void;
  onToggleProxy: (id: string) => void;
  onRefreshAccounts: () => void;
  onRefreshProxies: () => void;
  onClearInvalidAccounts: () => void;
  onClearInvalidProxies: () => void;
};

/** 账号 + 代理勾选区：memo 隔离，避免任务名称等字段输入时整表重渲染卡顿。 */
export const WeiboBindPoolsSection = memo(function WeiboBindPoolsSection({
  open,
  accounts,
  accountsLoading,
  accountsError,
  selectedAccountIds,
  proxies,
  proxiesLoading,
  proxiesError,
  selectedProxyIds,
  proxyHealth,
  effectivePlatform,
  orphanAccountIds,
  orphanProxyIds,
  errorSelectedAccountCount,
  invalidSelectedProxyCount,
  onToggleAccount,
  onToggleProxy,
  onRefreshAccounts,
  onRefreshProxies,
  onClearInvalidAccounts,
  onClearInvalidProxies,
}: WeiboBindPoolsProps) {
  return (
    <div className="grid grid-cols-2 gap-3">
      <div className="space-y-2">
        <div className="flex items-center justify-between gap-2">
          <Label>绑定账号</Label>
          <div className="flex shrink-0 items-center gap-1">
            {(errorSelectedAccountCount > 0 || orphanAccountIds.length > 0) && (
              <Button
                type="button"
                variant="ghost"
                size="sm"
                className="h-6 px-2 text-xs text-muted-foreground hover:text-foreground"
                onClick={onClearInvalidAccounts}
                title="移除已选中但异常或已删除的账号"
              >
                清理失效项
              </Button>
            )}
            <Button
              type="button"
              variant="ghost"
              size="icon"
              className="size-6"
              onClick={() => void onRefreshAccounts()}
              disabled={accountsLoading}
              title="刷新账号状态"
            >
              <RefreshCwIcon
                className={cn(
                  "size-3.5",
                  accountsLoading && "animate-spin",
                )}
              />
            </Button>
          </div>
        </div>
        {accountsLoading && accounts.length === 0 && (
          <Alert className="py-2">
            <Loader2Icon className="animate-spin" />
            <AlertDescription>加载账号中…</AlertDescription>
          </Alert>
        )}
        {open && accountsError && (
          <Alert variant="destructive">
            <AlertCircleIcon />
            <AlertTitle>加载账号失败</AlertTitle>
            <AlertDescription>{accountsError}</AlertDescription>
          </Alert>
        )}
        {!accountsLoading &&
          !(open && accountsError) &&
          accounts.length === 0 &&
          orphanAccountIds.length === 0 && (
            <Alert className="py-2">
              <InfoIcon />
              <AlertDescription>
                暂无微博账号，请先在账号管理中添加。
              </AlertDescription>
            </Alert>
          )}
        {(accounts.length > 0 || orphanAccountIds.length > 0) && (
          <ScrollArea className="h-36 rounded-lg border">
            <ul className="divide-y p-1">
              {accounts.map((acc) => {
                const checked = selectedAccountIds.includes(acc.id);
                const subtitle =
                  acc.weiboProfile?.centerWeiboName ?? acc.username;
                const isError = acc.riskStatus === "error";
                const isRestricted = acc.riskStatus === "restricted";
                const lockSelect = isError && !checked;
                const riskHint = isError
                  ? checked
                    ? "账号风控状态：异常。已自动停用，建议取消勾选；如需继续使用请先在账号管理中重新登录。"
                    : "账号风控状态：异常。已有大量登录拦截或失败，请先在账号管理中重新登录。"
                  : isRestricted
                    ? "账号风控状态：受限。最近 5 分钟有较多失败，建议谨慎使用。"
                    : null;
                return (
                  <li key={acc.id}>
                    <label
                      className={cn(
                        "flex cursor-pointer items-start gap-2 rounded-md px-2 py-1.5 text-left hover:bg-muted/60",
                        lockSelect &&
                          "cursor-not-allowed opacity-60 hover:bg-transparent",
                        isError &&
                          checked &&
                          "bg-red-50/60 dark:bg-red-950/20",
                      )}
                      title={riskHint ?? undefined}
                    >
                      <input
                        type="checkbox"
                        className="mt-0.5 size-4 shrink-0 accent-primary disabled:cursor-not-allowed"
                        checked={checked}
                        disabled={lockSelect}
                        onChange={() => onToggleAccount(acc.id)}
                      />
                      <span className="min-w-0 flex-1">
                        <span className="flex items-center gap-1.5">
                          <span className="block truncate text-sm font-medium">
                            {subtitle}
                          </span>
                          {isRestricted && (
                            <span className="inline-flex items-center rounded bg-amber-100 px-1.5 text-[10px] text-amber-800 dark:bg-amber-900/40 dark:text-amber-200">
                              受限
                            </span>
                          )}
                          {isError && (
                            <span className="inline-flex items-center rounded bg-red-100 px-1.5 text-[10px] text-red-700 dark:bg-red-900/40 dark:text-red-200">
                              异常
                            </span>
                          )}
                        </span>
                        <span className="text-xs text-muted-foreground">
                          {acc.id.slice(0, 8)}…
                        </span>
                      </span>
                    </label>
                  </li>
                );
              })}
              {orphanAccountIds.map((id) => (
                <li key={id}>
                  <div
                    className="flex items-start gap-2 rounded-md bg-red-50/60 px-2 py-1.5 text-left dark:bg-red-950/20"
                    title="该账号已被删除，无法继续使用，请取消勾选。"
                  >
                    <button
                      type="button"
                      className="mt-0.5 inline-flex size-4 shrink-0 items-center justify-center rounded border border-red-300 bg-white text-red-700 hover:bg-red-100 dark:border-red-800 dark:bg-red-950 dark:text-red-200"
                      onClick={() => onToggleAccount(id)}
                      aria-label="移除已删除账号"
                      title="移除"
                    >
                      <XIcon className="size-3" />
                    </button>
                    <span className="min-w-0 flex-1">
                      <span className="flex items-center gap-1.5">
                        <span className="block truncate text-sm font-medium text-red-700 dark:text-red-200">
                          {id.slice(0, 8)}…
                        </span>
                        <span className="inline-flex items-center rounded bg-red-100 px-1.5 text-[10px] text-red-700 dark:bg-red-900/40 dark:text-red-200">
                          已删除
                        </span>
                      </span>
                      <span className="text-xs text-muted-foreground">
                        账号已不存在，建议移除
                      </span>
                    </span>
                  </div>
                </li>
              ))}
            </ul>
          </ScrollArea>
        )}
      </div>
      <div className="space-y-2">
        <div className="flex items-center justify-between gap-2">
          <Label>绑定代理</Label>
          <div className="flex shrink-0 items-center gap-1">
            {(invalidSelectedProxyCount > 0 || orphanProxyIds.length > 0) && (
              <Button
                type="button"
                variant="ghost"
                size="sm"
                className="h-6 px-2 text-xs text-muted-foreground hover:text-foreground"
                onClick={onClearInvalidProxies}
                title="移除已选中但失效或已删除的代理"
              >
                清理失效项
              </Button>
            )}
            <Button
              type="button"
              variant="ghost"
              size="icon"
              className="size-6"
              onClick={() => void onRefreshProxies()}
              disabled={proxiesLoading}
              title="刷新代理状态"
            >
              <RefreshCwIcon
                className={cn(
                  "size-3.5",
                  proxiesLoading && "animate-spin",
                )}
              />
            </Button>
          </div>
        </div>
        {proxiesLoading && proxies.length === 0 && (
          <Alert className="py-2">
            <Loader2Icon className="animate-spin" />
            <AlertDescription>加载代理中…</AlertDescription>
          </Alert>
        )}
        {open && proxiesError && (
          <Alert variant="destructive">
            <AlertCircleIcon />
            <AlertTitle>加载代理失败</AlertTitle>
            <AlertDescription>{proxiesError}</AlertDescription>
          </Alert>
        )}
        {!proxiesLoading &&
          !(open && proxiesError) &&
          proxies.length === 0 &&
          orphanProxyIds.length === 0 && (
            <Alert className="py-2">
              <InfoIcon />
              <AlertDescription>
                暂无代理，前往「代理管理」添加。
              </AlertDescription>
            </Alert>
          )}
        {(proxies.length > 0 || orphanProxyIds.length > 0) && (
          <ScrollArea className="h-36 rounded-lg border">
            <ul className="divide-y p-1">
              {proxies.map((px) => {
                const checked = selectedProxyIds.includes(px.id);
                const h = proxyHealth[px.id];
                const isInvalid = h?.globalStatus === "invalid";
                const isRestrictedHere =
                  !isInvalid &&
                  !!h?.restrictions.some(
                    (r) => r.platform === effectivePlatform,
                  );
                const otherRestrictedPlatforms = (h?.restrictions ?? [])
                  .filter((r) => r.platform !== effectivePlatform)
                  .map((r) => r.platform);
                const isDirect = px.proxyType === "Direct";
                const lockSelect = (isInvalid || isRestrictedHere) && !checked;
                const riskHint = isInvalid
                  ? checked
                    ? "代理状态：失效。该出口近 5 分钟内多次出现网络类异常（请求超时、完全连不上、DNS 失败等），已超过阈值；建议取消勾选并检查代理或本机网络。"
                    : "代理状态：失效。近 5 分钟内在该线路上累计 ≥10 次「网络类」错误（含超时、无连接、DNS 等；与 HTTP 业务码无关），系统判定出口暂不可用，已自动停用。"
                  : isRestrictedHere
                    ? isDirect
                      ? `本机直连在「${PLATFORM_LABELS[effectivePlatform]}」上受限。最近 5 分钟该平台出现较多失败，建议挂代理或暂停后再试。`
                      : `代理在「${PLATFORM_LABELS[effectivePlatform]}」上受限。其它平台不受影响。`
                    : otherRestrictedPlatforms.length > 0
                      ? `该代理在其它平台（${otherRestrictedPlatforms
                          .map(
                            (p) =>
                              (PLATFORM_LABELS as Record<string, string>)[p] ?? p,
                          )
                          .join(" / ")}）受限，不影响当前「${PLATFORM_LABELS[effectivePlatform]}」任务。`
                      : null;
                return (
                  <li key={px.id}>
                    <label
                      className={cn(
                        "flex cursor-pointer items-start gap-2 rounded-md px-2 py-1.5 text-left hover:bg-muted/60",
                        lockSelect &&
                          "cursor-not-allowed opacity-60 hover:bg-transparent",
                        isDirect && "bg-muted/40",
                        (isInvalid || isRestrictedHere) &&
                          checked &&
                          "bg-red-50/60 dark:bg-red-950/20",
                      )}
                      title={riskHint ?? undefined}
                    >
                      <input
                        type="checkbox"
                        className="mt-0.5 size-4 shrink-0 accent-primary disabled:cursor-not-allowed"
                        checked={checked}
                        disabled={lockSelect}
                        onChange={() => onToggleProxy(px.id)}
                      />
                      <span className="min-w-0 flex-1">
                        <span className="flex items-center gap-1.5">
                          <span className="block truncate text-sm font-medium">
                            {isDirect
                              ? "本机直连"
                              : formatProxyEndpoint(px.address)}
                          </span>
                          {isDirect && (
                            <span className="inline-flex items-center rounded bg-slate-100 px-1.5 text-[10px] text-slate-700 dark:bg-slate-700/40 dark:text-slate-200">
                              系统
                            </span>
                          )}
                          {isRestrictedHere && (
                            <span className="inline-flex items-center rounded bg-amber-100 px-1.5 text-[10px] text-amber-800 dark:bg-amber-900/40 dark:text-amber-200">
                              本平台受限
                            </span>
                          )}
                          {!isInvalid &&
                            !isRestrictedHere &&
                            otherRestrictedPlatforms.length > 0 && (
                              <span
                                className="inline-flex items-center rounded bg-slate-100 px-1.5 text-[10px] text-slate-600 dark:bg-slate-700/40 dark:text-slate-300"
                                title="该代理在其它平台 scope 上受限，但当前任务平台可用"
                              >
                                其它平台 ⚠ {otherRestrictedPlatforms.length}
                              </span>
                            )}
                          {isInvalid && (
                            <span className="inline-flex items-center rounded bg-red-100 px-1.5 text-[10px] text-red-700 dark:bg-red-900/40 dark:text-red-200">
                              失效
                            </span>
                          )}
                        </span>
                        <span className="text-xs text-muted-foreground">
                          {isDirect
                            ? "DIRECT · 不走代理"
                            : `${px.proxyType.toUpperCase()} · ${
                                isInvalid
                                  ? "invalid"
                                  : isRestrictedHere
                                    ? "restricted (本平台)"
                                    : "available"
                              }`}
                        </span>
                      </span>
                    </label>
                  </li>
                );
              })}
              {orphanProxyIds.map((id) => (
                <li key={id}>
                  <div
                    className="flex items-start gap-2 rounded-md bg-red-50/60 px-2 py-1.5 text-left dark:bg-red-950/20"
                    title="该代理已被删除，无法继续使用，请取消勾选。"
                  >
                    <button
                      type="button"
                      className="mt-0.5 inline-flex size-4 shrink-0 items-center justify-center rounded border border-red-300 bg-white text-red-700 hover:bg-red-100 dark:border-red-800 dark:bg-red-950 dark:text-red-200"
                      onClick={() => onToggleProxy(id)}
                      aria-label="移除已删除代理"
                      title="移除"
                    >
                      <XIcon className="size-3" />
                    </button>
                    <span className="min-w-0 flex-1">
                      <span className="flex items-center gap-1.5">
                        <span className="block truncate text-sm font-medium text-red-700 dark:text-red-200">
                          {id.slice(0, 8)}…
                        </span>
                        <span className="inline-flex items-center rounded bg-red-100 px-1.5 text-[10px] text-red-700 dark:bg-red-900/40 dark:text-red-200">
                          已删除
                        </span>
                      </span>
                      <span className="text-xs text-muted-foreground">
                        代理已不存在，建议移除
                      </span>
                    </span>
                  </div>
                </li>
              ))}
            </ul>
          </ScrollArea>
        )}
      </div>
    </div>
  );
});

export type WeiboBindAlertsProps = {
  open: boolean;
  weiboBindValid: boolean;
  selectedAccountIds: string[];
  selectedProxyIds: string[];
  invalidSelectedProxyCount: number;
  orphanProxyIds: string[];
  errorSelectedAccountCount: number;
  orphanAccountIds: string[];
};

export const WeiboBindAlertsSection = memo(function WeiboBindAlertsSection({
  open,
  weiboBindValid,
  selectedAccountIds,
  selectedProxyIds,
  invalidSelectedProxyCount,
  orphanProxyIds,
  errorSelectedAccountCount,
  orphanAccountIds,
}: WeiboBindAlertsProps) {
  return (
    <>
      {open && !weiboBindValid && (
        <Alert variant="warning" className="py-2">
          <TriangleAlertIcon />
          <AlertTitle className="text-sm">绑定不完整</AlertTitle>
          <AlertDescription className="text-xs">
            创建或保存前需至少勾选 1 个账号与 1
            个出口（本机直连或代理）；仅勾选已删除项无效。
          </AlertDescription>
        </Alert>
      )}
      {open &&
        selectedAccountIds.length >= 2 &&
        selectedProxyIds.length > 0 &&
        selectedProxyIds.every((id) => id === LOCAL_DIRECT_PROXY_ID) && (
          <Alert variant="warning" className="py-2">
            <TriangleAlertIcon />
            <AlertTitle className="text-sm">出口风险提示</AlertTitle>
            <AlertDescription className="text-xs">
              多个账号共享同一出口 IP（本机直连）。一旦本机 IP 触发 Weibo
              限流（典型表现为 HTTP 414），所有账号会同时报错。
              建议至少添加一个外部代理稀释风险。
            </AlertDescription>
          </Alert>
        )}
      {open &&
        (invalidSelectedProxyCount > 0 ||
          orphanProxyIds.length > 0 ||
          errorSelectedAccountCount > 0 ||
          orphanAccountIds.length > 0) && (
          <Alert variant="destructive" className="py-2">
            <AlertCircleIcon />
            <AlertTitle className="text-sm">当前选中含不可用项</AlertTitle>
            <AlertDescription className="text-xs">
              当前选中包含
              {invalidSelectedProxyCount > 0 &&
                ` ${invalidSelectedProxyCount} 个失效或本平台受限代理`}
              {orphanProxyIds.length > 0 &&
                `${invalidSelectedProxyCount > 0 ? "、" : " "}${orphanProxyIds.length} 个已删除代理`}
              {errorSelectedAccountCount > 0 &&
                `${invalidSelectedProxyCount + orphanProxyIds.length > 0 ? "、" : " "}${errorSelectedAccountCount} 个异常账号`}
              {orphanAccountIds.length > 0 &&
                `${invalidSelectedProxyCount + orphanProxyIds.length + errorSelectedAccountCount > 0 ? "、" : " "}${orphanAccountIds.length} 个已删除账号`}
              ，保存后这些项不会派发新的 worker。建议使用「清理失效项」移除。
            </AlertDescription>
          </Alert>
        )}
    </>
  );
});
