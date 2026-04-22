import { Loader2Icon } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import type {
  ProxyGlobalRow,
  ProxyHealthBrief,
  ProxyIp,
} from "@/features/domain/types";
import {
  formatGeoLocation,
  formatLatencyOutcome,
  latencyOutcomeColor,
  latencyOutcomeTooltip,
} from "../utils/format";
import { ProxyAddressCell } from "./ProxyAddressCell";
import { RowActionsMenu } from "./RowActionsMenu";

/**
 * 全局 tab 的表格：列 = IP / 类型 / 状态 / 实际位置 / 国内延迟 / 国外延迟 / 备注 / 操作。
 *
 * v7 起去掉「单条刷新地理信息」按钮：geo 反查与双延迟探针都收口到顶部
 * 「刷新并测延迟」一个按钮里（后端 `check_all_proxies_dual_health` 内部并行）。
 *
 * 「状态」列在全局视图只显示 **可用 / 不可用** 两态——出口本身的连通性。
 * IP 地址列仅展示文本（无徽章）。平台维度的受限状态在各平台 tab 的「状态」列体现。
 */
export interface GlobalIpTableProps {
  rows: ProxyGlobalRow[];
  loading: boolean;
  healthMap: Record<string, ProxyHealthBrief>;
  onViewLog: (proxy: ProxyIp) => void;
  onEdit: (proxy: ProxyIp) => void;
  onDelete: (proxy: ProxyIp) => void;
}

export function GlobalIpTable({
  rows,
  loading,
  healthMap,
  onViewLog,
  onEdit,
  onDelete,
}: GlobalIpTableProps) {
  return (
    <Table>
      <TableHeader>
        <TableRow>
          <TableHead>IP 地址</TableHead>
          <TableHead>代理类型</TableHead>
          <TableHead className="w-[88px] text-center">状态</TableHead>
          <TableHead className="hidden min-w-[160px] md:table-cell">实际位置</TableHead>
          <TableHead className="text-right">响应延迟(国内)</TableHead>
          <TableHead className="text-right">响应延迟(国外)</TableHead>
          <TableHead className="hidden min-w-[120px] xl:table-cell">IP 备注</TableHead>
          <TableHead className="w-[44px] text-center">
            <span className="sr-only">操作</span>
          </TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {loading && rows.length === 0 && (
          <TableRow>
            <TableCell colSpan={8} className="py-12">
              <div className="flex items-center justify-center">
                <Loader2Icon className="size-6 animate-spin text-muted-foreground" />
              </div>
            </TableCell>
          </TableRow>
        )}
        {!loading && rows.length === 0 && (
          <TableRow>
            <TableCell colSpan={8} className="py-6 text-center text-sm text-muted-foreground">
              暂无代理记录。
            </TableCell>
          </TableRow>
        )}
        {rows.map((row) => {
          const isDirect = row.proxyType === "Direct";
          const location = formatGeoLocation(row);
          const health = healthMap[row.id];
          // 全局视图二态：globalStatus 只可能是 available / invalid（后端语义保证）；
          // 健康档位还没拉到时按 available 兜底，避免首屏闪一下「不可用」。
          const isInvalid = health?.globalStatus === "invalid";
          return (
            <TableRow key={row.id} className={row.isSystem ? "bg-muted/30" : undefined}>
              <TableCell>
                <ProxyAddressCell address={row.address} />
              </TableCell>
              <TableCell>
                <Badge variant={isDirect ? "secondary" : "outline"}>
                  {isDirect ? "直连" : row.proxyType}
                </Badge>
              </TableCell>
              <TableCell className="text-center">
                <Badge
                  variant={isInvalid ? "destructive" : "outline"}
                  className={
                    isInvalid
                      ? undefined
                      : "border-emerald-500/40 text-emerald-700 dark:text-emerald-400"
                  }
                  title={
                    isInvalid
                      ? "出口本身不通：5 min 滑窗内累计失败已达全局阈值"
                      : "出口连通性正常（平台维度的受限状态请看各平台 tab）"
                  }
                >
                  {isInvalid ? "不可用" : "可用"}
                </Badge>
              </TableCell>
              <TableCell className="hidden max-w-[260px] text-xs md:table-cell">
                {location ? (
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <span className="block truncate text-left text-foreground">
                        {location}
                      </span>
                    </TooltipTrigger>
                    <TooltipContent side="top" className="max-w-xs space-y-1 text-xs">
                      <div>
                        <span className="text-muted-foreground">位置：</span>
                        {location}
                      </div>
                      {row.geoIsp && (
                        <div>
                          <span className="text-muted-foreground">ISP：</span>
                          {row.geoIsp}
                        </div>
                      )}
                      {row.geoIp && (
                        <div>
                          <span className="text-muted-foreground">命中 IP：</span>
                          <span className="font-mono">{row.geoIp}</span>
                        </div>
                      )}
                      {row.lastProbedAt && (
                        <div className="text-muted-foreground">
                          上次刷新 {row.lastProbedAt}
                        </div>
                      )}
                    </TooltipContent>
                  </Tooltip>
                ) : (
                  <span
                    className="text-muted-foreground"
                    title={
                      row.lastProbedAt
                        ? `上次刷新 ${row.lastProbedAt}：地理反查未命中`
                        : "尚未反查；点击顶部「刷新并测延迟」"
                    }
                  >
                    —
                  </span>
                )}
              </TableCell>
              <TableCell
                className="text-right"
                title={latencyOutcomeTooltip(row.cnLatency, "国内")}
              >
                <span className={latencyOutcomeColor(row.cnLatency)}>
                  {formatLatencyOutcome(row.cnLatency)}
                </span>
              </TableCell>
              <TableCell
                className="text-right"
                title={latencyOutcomeTooltip(row.intlLatency, "国外")}
              >
                <span className={latencyOutcomeColor(row.intlLatency)}>
                  {formatLatencyOutcome(row.intlLatency)}
                </span>
              </TableCell>
              <TableCell className="hidden max-w-[220px] text-xs text-muted-foreground xl:table-cell">
                {row.remark?.trim() ? (
                  <span className="text-foreground">{row.remark}</span>
                ) : (
                  <span className="text-muted-foreground">—</span>
                )}
              </TableCell>
              <TableCell className="text-center">
                <RowActionsMenu
                  proxy={row}
                  onViewLog={() => onViewLog(row)}
                  onEdit={() => onEdit(row)}
                  onDelete={() => onDelete(row)}
                />
              </TableCell>
            </TableRow>
          );
        })}
      </TableBody>
    </Table>
  );
}
