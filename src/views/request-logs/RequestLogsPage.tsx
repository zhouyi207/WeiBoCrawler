import { useCallback, useEffect, useState } from "react";
import {
  ChevronLeftIcon,
  ChevronRightIcon,
  ChevronsLeftIcon,
  ChevronsRightIcon,
  Loader2Icon,
  RefreshCwIcon,
  Trash2Icon,
} from "lucide-react";
import { toast } from "sonner";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { FloatingScrollArea } from "@/app/ui/FloatingScrollArea";
import { PLATFORM_LABELS, type Platform } from "@/features/domain/types";
import { clearRequestLogs, listRequestLogs } from "@/services/tauri/commands";
import type { RequestLogEntry } from "@/features/domain/types";

const PAGE_SIZE = 100;

const REQUEST_KIND_LABELS: Record<string, string> = {
  list_html: "列表 HTML",
  body: "正文 API",
  comment_l1: "一级评论",
  comment_l2: "二级评论",
};

function kindLabel(kind: string): string {
  return REQUEST_KIND_LABELS[kind] ?? kind;
}

function platformLabel(tag: string): string {
  const p = tag as Platform;
  return PLATFORM_LABELS[p] ?? tag;
}

function nameOrDash(name: string | null | undefined): string {
  if (name && name.trim() !== "") return name;
  return "—";
}

export function RequestLogsPage() {
  const [rows, setRows] = useState<RequestLogEntry[]>([]);
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(0);
  const [loading, setLoading] = useState(true);
  const [clearOpen, setClearOpen] = useState(false);
  const [clearing, setClearing] = useState(false);

  const load = useCallback(async (p: number) => {
    setLoading(true);
    try {
      const off = p * PAGE_SIZE;
      const res = await listRequestLogs(PAGE_SIZE, off);
      setRows(res.items);
      setTotal(res.total);
      setPage(p);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load(0);
  }, [load]);

  const pageCount = Math.max(1, Math.ceil(total / PAGE_SIZE));
  const safePage = Math.min(page, pageCount - 1);

  async function handleClearConfirmed() {
    setClearing(true);
    try {
      await clearRequestLogs();
      toast.success("已清空请求日志");
      setClearOpen(false);
      await load(0);
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e));
    } finally {
      setClearing(false);
    }
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-4 p-4">
      <AlertDialog open={clearOpen} onOpenChange={setClearOpen}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>清空请求日志？</AlertDialogTitle>
            <AlertDialogDescription>
              将永久删除当前数据库中全部 HTTP 请求记录，此操作不可撤销。
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={clearing}>取消</AlertDialogCancel>
            <AlertDialogAction
              className="bg-destructive text-destructive-foreground hover:bg-destructive/90"
              disabled={clearing}
              onClick={(ev) => {
                ev.preventDefault();
                void handleClearConfirmed();
              }}
            >
              {clearing ? "清除中…" : "全部清除"}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      <Card className="flex min-h-0 flex-1 flex-col overflow-hidden">
        <CardHeader className="flex shrink-0 flex-row flex-wrap items-center justify-between gap-2 space-y-0 pb-4">
          <CardTitle className="text-base">网络请求记录</CardTitle>
          <div className="flex flex-wrap items-center justify-end gap-2">
            <span className="text-muted-foreground text-xs">
              共 {total} 条 · 超出上限时自动丢弃最旧批次
            </span>
            <Button
              type="button"
              variant="outline"
              size="sm"
              className="gap-1.5"
              disabled={loading}
              onClick={() => void load(safePage)}
            >
              {loading ? (
                <Loader2Icon className="size-4 animate-spin" />
              ) : (
                <RefreshCwIcon className="size-4" />
              )}
              刷新
            </Button>
            <Button
              type="button"
              variant="outline"
              size="sm"
              className="gap-1.5 text-destructive hover:bg-destructive/10 hover:text-destructive"
              disabled={loading || clearing || total === 0}
              onClick={() => setClearOpen(true)}
            >
              <Trash2Icon className="size-4" />
              清除日志
            </Button>
          </div>
        </CardHeader>
        <CardContent className="flex min-h-0 flex-1 flex-col overflow-hidden pt-0">
          <FloatingScrollArea className="min-h-0 flex-1">
            <div className="overflow-x-auto pr-2">
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead className="whitespace-nowrap">时间</TableHead>
                    <TableHead className="whitespace-nowrap">平台</TableHead>
                    <TableHead className="whitespace-nowrap">类型</TableHead>
                    <TableHead className="whitespace-nowrap">HTTP</TableHead>
                    <TableHead className="whitespace-nowrap">耗时</TableHead>
                    <TableHead className="min-w-[200px]">URL</TableHead>
                    <TableHead className="whitespace-nowrap">任务</TableHead>
                    <TableHead className="whitespace-nowrap">账号</TableHead>
                    <TableHead className="whitespace-nowrap">IP 地址</TableHead>
                    <TableHead className="whitespace-nowrap">错误</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {rows.length === 0 && !loading ? (
                    <TableRow>
                      <TableCell colSpan={10} className="text-muted-foreground h-24 text-center">
                        暂无请求日志。运行采集任务后将在此记录列表 / 正文 / 评论等 HTTP 请求。
                      </TableCell>
                    </TableRow>
                  ) : (
                    rows.map((r) => (
                      <TableRow key={r.id}>
                        <TableCell className="whitespace-nowrap font-mono text-xs">
                          {r.time}
                        </TableCell>
                        <TableCell className="whitespace-nowrap text-xs">
                          {platformLabel(r.platform)}
                        </TableCell>
                        <TableCell className="text-xs">
                          <span title={r.phase ?? undefined}>
                            {kindLabel(r.requestKind)}
                            {r.phase ? (
                              <span className="text-muted-foreground"> · {r.phase}</span>
                            ) : null}
                          </span>
                        </TableCell>
                        <TableCell className="whitespace-nowrap font-mono text-xs">
                          {r.statusCode ?? "—"}
                        </TableCell>
                        <TableCell className="whitespace-nowrap text-xs">
                          {r.durationMs} ms
                        </TableCell>
                        <TableCell
                          className="max-w-[360px] truncate font-mono text-xs"
                          title={r.url}
                        >
                          {r.url}
                        </TableCell>
                        <TableCell
                          className="max-w-[140px] truncate text-xs"
                          title={
                            r.taskId
                              ? r.taskName
                                ? `任务名：${r.taskName}（id：${r.taskId}）`
                                : `任务 id：${r.taskId}`
                              : undefined
                          }
                        >
                          {nameOrDash(r.taskName)}
                        </TableCell>
                        <TableCell
                          className="max-w-[120px] truncate text-xs"
                          title={
                            r.accountId
                              ? r.accountName
                                ? `账号：${r.accountName}（id：${r.accountId}）`
                                : `账号 id：${r.accountId}`
                              : undefined
                          }
                        >
                          {nameOrDash(r.accountName)}
                        </TableCell>
                        <TableCell
                          className="max-w-[180px] truncate font-mono text-xs"
                          title={
                            r.proxyId
                              ? r.proxyAddress
                                ? `${r.proxyAddress}（id：${r.proxyId}）`
                                : `代理 id：${r.proxyId}（无地址）`
                              : undefined
                          }
                        >
                          {r.proxyAddress?.trim() ? r.proxyAddress : "—"}
                        </TableCell>
                        <TableCell
                          className="max-w-[200px] truncate text-xs text-destructive"
                          title={r.errorMessage ?? undefined}
                        >
                          {r.errorMessage ?? "—"}
                        </TableCell>
                      </TableRow>
                    ))
                  )}
                </TableBody>
              </Table>
            </div>
          </FloatingScrollArea>
          <div className="mt-4 flex shrink-0 flex-wrap items-center justify-end gap-2 border-t pt-4">
            <span className="text-muted-foreground mr-auto text-xs">
              第 {safePage + 1} / {pageCount} 页
            </span>
            <Button
              type="button"
              variant="outline"
              size="icon"
              className="size-8"
              disabled={loading || safePage <= 0}
              onClick={() => void load(0)}
            >
              <ChevronsLeftIcon className="size-4" />
            </Button>
            <Button
              type="button"
              variant="outline"
              size="icon"
              className="size-8"
              disabled={loading || safePage <= 0}
              onClick={() => void load(safePage - 1)}
            >
              <ChevronLeftIcon className="size-4" />
            </Button>
            <Button
              type="button"
              variant="outline"
              size="icon"
              className="size-8"
              disabled={loading || safePage >= pageCount - 1}
              onClick={() => void load(safePage + 1)}
            >
              <ChevronRightIcon className="size-4" />
            </Button>
            <Button
              type="button"
              variant="outline"
              size="icon"
              className="size-8"
              disabled={loading || safePage >= pageCount - 1}
              onClick={() => void load(pageCount - 1)}
            >
              <ChevronsRightIcon className="size-4" />
            </Button>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
