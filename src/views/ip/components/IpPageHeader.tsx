import { PlusIcon, RefreshCwIcon, Settings2Icon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

/**
 * IP 管理页顶部标题 + 工具栏。
 *
 * 拆出来主要是为了让 `IpPage.tsx` 不再夹杂多个 Button + 一坨 title 文本，
 * 同时把「刷新按钮的语义随当前 tab 变」这件事内聚在一处：
 * - 全局 tab：`刷新并测延迟` —— 一个按钮把 geo 反查和 cn / intl 双探针
 *   并行做完（每条 ~10s），写回 `proxies` 行；
 * - 平台 tab：只重拉一次 runtime 快照，瞬时返回。
 */
export interface IpPageHeaderProps {
  activeTabIsGlobal: boolean;
  refreshing: boolean;
  refreshDisabled: boolean;
  onOpenProbeSettings: () => void;
  onRefresh: () => void;
  onAdd: () => void;
}

export function IpPageHeader({
  activeTabIsGlobal,
  refreshing,
  refreshDisabled,
  onOpenProbeSettings,
  onRefresh,
  onAdd,
}: IpPageHeaderProps) {
  const refreshButtonLabel = activeTabIsGlobal
    ? refreshing
      ? "刷新中…"
      : "刷新并测延迟"
    : refreshing
      ? "刷新中…"
      : "刷新当前平台";
  const refreshButtonTitle = activeTabIsGlobal
    ? "对所有代理（含本机直连）并行刷新：geo 反查 + 国内 / 国外双延迟探针，每条 ~10s。"
    : "重新拉取该平台 tab 下所有代理的最近一次响应、绑定 / 运行账号数、风险系数。";

  return (
    <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
      <div className="min-w-0">
        <h1 className="text-2xl font-bold tracking-tight">IP 代理管理</h1>
        <p className="text-sm text-muted-foreground">
          按平台分类管理代理资源；全局 tab 看出口连通性，平台 tab 看运行画像
        </p>
      </div>
      <div className="flex shrink-0 flex-wrap items-center justify-end gap-2">
        <Button
          variant="outline"
          onClick={onOpenProbeSettings}
          title="编辑国内 / 国外延迟探针 URL（「刷新并测延迟」）"
        >
          <Settings2Icon className="size-4" />
          延迟探针设置
        </Button>
        <Button
          variant="outline"
          onClick={onRefresh}
          disabled={refreshDisabled}
          title={refreshButtonTitle}
        >
          <RefreshCwIcon className={cn("size-4", refreshing && "animate-spin")} />
          {refreshButtonLabel}
        </Button>
        <Button onClick={onAdd}>
          <PlusIcon className="size-4" />
          添加代理
        </Button>
      </div>
    </div>
  );
}
