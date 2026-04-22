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
  PLATFORM_LABELS,
  type Platform,
  type ProxyIp,
  type ProxyPlatformRow,
} from "@/features/domain/types";
import {
  PROXY_STATUS_MAP,
  describeLastStatus,
  formatLastLatency,
  lastLatencyColor,
  riskColor,
} from "../utils/format";
import { ProxyAddressCell } from "./ProxyAddressCell";
import { RowActionsMenu } from "./RowActionsMenu";

/**
 * per-platform tab 的表格：列 = IP / 最后响应时间 / 账号 / 延迟 / 状态 /
 * 绑定数 / 运行数 / IP 状态 / 风险系数 / 操作。
 *
 * `ProxyPlatformRow` 现在已经 `extends ProxyIp`（后端用 #[serde(flatten)]
 * 同步），因此这里直接把整行当 `ProxyIp` 传给 `RowActionsMenu`，无需再写
 * `toBaseIp` 适配函数。
 */
export interface PlatformIpTableProps {
  platform: Platform;
  rows: ProxyPlatformRow[];
  loading: boolean;
  onViewLog: (proxy: ProxyIp) => void;
  onEdit: (proxy: ProxyIp) => void;
  onDelete: (proxy: ProxyIp) => void;
}

export function PlatformIpTable({
  platform,
  rows,
  loading,
  onViewLog,
  onEdit,
  onDelete,
}: PlatformIpTableProps) {
  return (
    <Table>
      <TableHeader>
        <TableRow>
          <TableHead>IP 地址</TableHead>
          <TableHead className="hidden min-w-[140px] md:table-cell">
            最后一次响应时间
          </TableHead>
          <TableHead className="hidden md:table-cell">最后一次响应的账号</TableHead>
          <TableHead className="text-right">最后一次响应的延迟</TableHead>
          <TableHead className="hidden md:table-cell">最后一次响应状态</TableHead>
          <TableHead className="text-center">绑定账号数量</TableHead>
          <TableHead className="text-center">运行账号数量</TableHead>
          <TableHead className="text-center">状态</TableHead>
          <TableHead className="text-center">风险系数</TableHead>
          <TableHead className="w-[72px] text-center">操作</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {loading && rows.length === 0 && (
          <TableRow>
            <TableCell colSpan={10} className="py-12">
              <div className="flex items-center justify-center">
                <Loader2Icon className="size-6 animate-spin text-muted-foreground" />
              </div>
            </TableCell>
          </TableRow>
        )}
        {!loading && rows.length === 0 && (
          <TableRow>
            <TableCell colSpan={10} className="py-6 text-center text-sm text-muted-foreground">
              暂无 {PLATFORM_LABELS[platform]} 相关记录。
            </TableCell>
          </TableRow>
        )}
        {rows.map((row) => {
          const status = PROXY_STATUS_MAP[row.status] ?? PROXY_STATUS_MAP.available;
          const lastStatus = describeLastStatus(row);
          const lastAccount = row.lastAccountName ?? row.lastAccountId ?? "—";
          const hasRequest = Boolean(row.lastRespondedAt);
          return (
            <TableRow key={row.id} className={row.isSystem ? "bg-muted/30" : undefined}>
              <TableCell>
                <ProxyAddressCell address={row.address} />
              </TableCell>
              <TableCell className="hidden text-xs text-muted-foreground md:table-cell">
                {row.lastRespondedAt ?? "—"}
              </TableCell>
              <TableCell className="hidden max-w-[160px] truncate md:table-cell">
                {hasRequest ? lastAccount : <span className="text-muted-foreground">—</span>}
              </TableCell>
              <TableCell className="text-right">
                <span className={lastLatencyColor(row.lastLatencyMs)}>
                  {formatLastLatency(row.lastLatencyMs)}
                </span>
              </TableCell>
              <TableCell className="hidden md:table-cell">
                <span className={lastStatus.className}>{lastStatus.label}</span>
              </TableCell>
              <TableCell
                className="text-center tabular-nums"
                title="任务规划维度：累计有多少账号被任务勾选要在该 IP 上跑（任意 status 都算，去重）。bound_proxy_ids 为空的任务计入「本机直连」行。"
              >
                {row.boundAccountCount}
              </TableCell>
              <TableCell
                className="text-center tabular-nums"
                title="来自 in-memory worker 注册表，应用重启 / 任务暂停的瞬间会归零"
              >
                {row.runningAccountCount}
              </TableCell>
              <TableCell className="text-center">
                <Badge variant={status.variant} title={status.hint}>
                  {status.label}
                </Badge>
              </TableCell>
              <TableCell
                className="text-center"
                title="近 5 分钟该 (proxy, platform) 归责到 IP 的失败次数 × 10，封顶 100"
              >
                {hasRequest ? (
                  <span className={riskColor(row.riskScore)}>{row.riskScore}</span>
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
