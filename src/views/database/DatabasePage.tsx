import { useCallback, useEffect, useMemo, useState } from "react";
import { toast } from "sonner";
import {
  ChevronLeftIcon,
  ChevronRightIcon,
  ChevronsLeftIcon,
  ChevronsRightIcon,
  BracesIcon,
  DownloadIcon,
  FilterIcon,
  SearchIcon,
  Trash2Icon,
  Loader2Icon,
  RefreshCwIcon,
} from "lucide-react";
import {
  ExportProgressDialog,
  type ExportFormat,
  type ExportPhase,
  type ExportStatus,
} from "./ExportProgressDialog";
import {
  AlertDialog,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
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
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { FloatingScrollArea } from "@/app/ui/FloatingScrollArea";
import { PLATFORM_LABELS, type CrawledRecord, type Platform } from "@/features/domain/types";
import {
  weiboAuthorProfileUrl,
  weiboStatusDetailUrl,
} from "@/features/domain/weiboLinks";
import { save } from "@tauri-apps/plugin-dialog";
import { desktopDir, join } from "@tauri-apps/api/path";
import {
  queryRecordsPaged,
  exportRecordsJson,
  exportRecordsExcel,
  writeExportFile,
  deduplicateRecords,
  deleteRecordsFiltered,
  listRecordTaskNames,
  type RecordListFilter,
} from "@/services/tauri/commands";

const PAGE_SIZE = 50;

const ENTITY_LABELS: Record<string, string> = {
  feed: "列表",
  body: "详细页",
  comment_l1: "一级评论",
  comment_l2: "二级评论",
};

const ENTITY_TYPE_OPTIONS: { value: string; label: string }[] = [
  { value: "feed", label: ENTITY_LABELS.feed },
  { value: "body", label: ENTITY_LABELS.body },
  { value: "comment_l1", label: ENTITY_LABELS.comment_l1 },
  { value: "comment_l2", label: ENTITY_LABELS.comment_l2 },
];

/** 与「全部任务」选项 `all` 区分，避免任务名恰好为 `all` 时冲突。 */
const TASK_NAME_SELECT_PREFIX = "tn:";

function taskNameToSelectValue(name: string): string {
  return `${TASK_NAME_SELECT_PREFIX}${encodeURIComponent(name)}`;
}

/** `null` 表示「全部任务」；`string` 为 `records.task_name`（可与字面量 `"all"` 区分）。 */
function selectValueToTaskName(v: string): string | null {
  if (v === "all") return null;
  if (v.startsWith(TASK_NAME_SELECT_PREFIX)) {
    return decodeURIComponent(v.slice(TASK_NAME_SELECT_PREFIX.length));
  }
  return null;
}

function exportTimestampStamp(): string {
  const d = new Date();
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}_${p(d.getHours())}-${p(d.getMinutes())}-${p(d.getSeconds())}`;
}

/** 默认导出文件名（另存为对话框初始指向用户桌面）。 */
function defaultExcelFilename(): string {
  return `YssCrawler_${exportTimestampStamp()}.xlsx`;
}

function defaultJsonFilename(): string {
  return `YssCrawler_${exportTimestampStamp()}.json`;
}

/** 关键词列：仅展示 `records.keyword`（搜索词）。 */
function RecordKeywordCell({ record }: { record: CrawledRecord }) {
  const t = record.keyword?.trim();
  return (
    <span className="block max-w-[140px] truncate text-sm font-medium">
      {t || "—"}
    </span>
  );
}

/** 内容预览：可点击时与微博正文链接一致（`weiboStatusDetailUrl`）。 */
function RecordContentPreviewCell({ record }: { record: CrawledRecord }) {
  const href = weiboStatusDetailUrl(record);
  const text = record.contentPreview || "—";
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        {href ? (
          <a
            href={href}
            target="_blank"
            rel="noopener noreferrer"
            className="block max-w-[320px] cursor-pointer truncate text-xs text-primary underline-offset-2 hover:underline"
          >
            {text}
          </a>
        ) : (
          <div className="cursor-default truncate text-xs text-muted-foreground">
            {text}
          </div>
        )}
      </TooltipTrigger>
      <TooltipContent
        side="top"
        className="block !w-[min(32rem,calc(100vw-2rem))] !max-w-[min(32rem,calc(100vw-2rem))] min-h-0 min-w-0 max-h-[min(70vh,28rem)] overflow-x-hidden overflow-y-auto whitespace-pre-wrap break-words text-left font-normal leading-relaxed [overflow-wrap:anywhere] [word-break:break-word] [scrollbar-width:none] [-ms-overflow-style:none] [&::-webkit-scrollbar]:hidden"
      >
        {record.contentPreview || "（无）"}
      </TooltipContent>
    </Tooltip>
  );
}

/** 作者列：`weibo.com/u/{uid}`，uid 来自 `jsonData`。 */
function RecordAuthorLinkCell({ record }: { record: CrawledRecord }) {
  const href = weiboAuthorProfileUrl(record);
  if (href) {
    return (
      <a
        href={href}
        target="_blank"
        rel="noopener noreferrer"
        className="block truncate text-primary underline-offset-2 hover:underline"
      >
        {record.author}
      </a>
    );
  }
  return <span className="block truncate">{record.author}</span>;
}

export function DatabasePage() {
  const [platformFilter, setPlatformFilter] = useState<Platform | "all">("all");
  /** `null` 表示全部任务；否则为 `records.task_name` 原文。 */
  const [taskNameFilter, setTaskNameFilter] = useState<string | null>(null);
  const [entityTypeFilter, setEntityTypeFilter] = useState<string>("all");
  const [recordTaskNames, setRecordTaskNames] = useState<string[]>([]);
  const [searchQuery, setSearchQuery] = useState("");
  const [records, setRecords] = useState<CrawledRecord[]>([]);
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(1);
  const [loading, setLoading] = useState(false);
  const [exporting, setExporting] = useState(false);
  const [deduping, setDeduping] = useState(false);
  const [deleteDialogOpen, setDeleteDialogOpen] = useState(false);
  const [deletingFiltered, setDeletingFiltered] = useState(false);

  // 导出进度对话框：`exporting` 用于禁用按钮，这组状态用于驱动 modal 内容。
  const [exportDialogOpen, setExportDialogOpen] = useState(false);
  const [exportFormat, setExportFormat] = useState<ExportFormat>("excel");
  const [exportPhase, setExportPhase] = useState<ExportPhase>("query");
  const [exportStatus, setExportStatus] = useState<ExportStatus>("running");
  const [exportError, setExportError] = useState<string | null>(null);

  const totalPages = Math.max(1, Math.ceil(total / PAGE_SIZE));

  const loadRecordTaskNames = useCallback(async () => {
    try {
      const platform = platformFilter === "all" ? null : platformFilter;
      const names = await listRecordTaskNames(platform);
      setRecordTaskNames(names);
      setTaskNameFilter((prev) =>
        prev === null || names.includes(prev) ? prev : null,
      );
    } catch (err) {
      console.error("Failed to load record task names:", err);
    }
  }, [platformFilter]);

  useEffect(() => {
    void loadRecordTaskNames();
  }, [loadRecordTaskNames]);

  /** 与列表查询一致，用于导出 Excel / 清除重复。 */
  const listFilter = useMemo((): RecordListFilter => {
    return {
      platform: platformFilter === "all" ? null : platformFilter,
      keyword: searchQuery.trim() || null,
      taskName: taskNameFilter,
      entityType: entityTypeFilter === "all" ? null : entityTypeFilter,
    };
  }, [
    platformFilter,
    searchQuery,
    taskNameFilter,
    entityTypeFilter,
  ]);

  const fetchRecords = useCallback(async () => {
    setLoading(true);
    try {
      const platform = platformFilter === "all" ? null : platformFilter;
      const keyword = searchQuery.trim() || null;
      const entityType =
        entityTypeFilter === "all" ? null : entityTypeFilter;
      const data = await queryRecordsPaged(
        platform,
        keyword,
        page,
        PAGE_SIZE,
        taskNameFilter,
        entityType,
      );
      setRecords(data.items);
      setTotal(data.total);
    } catch (err) {
      console.error("Failed to fetch records:", err);
    } finally {
      setLoading(false);
    }
  }, [
    platformFilter,
    searchQuery,
    page,
    entityTypeFilter,
    taskNameFilter,
  ]);

  useEffect(() => {
    fetchRecords();
  }, [fetchRecords]);

  useEffect(() => {
    setPage(1);
  }, [platformFilter, searchQuery, taskNameFilter, entityTypeFilter]);

  /** 进入导出流程前的统一初始化：打开 modal、重置阶段与错误。 */
  const beginExport = (format: ExportFormat) => {
    setExportFormat(format);
    setExportPhase("query");
    setExportStatus("running");
    setExportError(null);
    setExportDialogOpen(true);
    setExporting(true);
  };

  const handleExportExcel = async () => {
    beginExport("excel");
    try {
      // 阶段 1：后端查询并生成 xlsx 字节流。
      const xlsx = await exportRecordsExcel(listFilter);

      // 阶段 2：弹出系统「另存为」让用户选路径。
      setExportPhase("pickPath");
      const defaultPath = await join(await desktopDir(), defaultExcelFilename());
      const path = await save({
        title: "导出 Excel",
        defaultPath,
        filters: [{ name: "Excel", extensions: ["xlsx"] }],
      });
      if (path == null) {
        setExportStatus("cancelled");
        return;
      }

      // 阶段 3：把字节流写入磁盘。
      setExportPhase("writeFile");
      await writeExportFile(path, xlsx);

      setExportPhase("done");
      setExportStatus("done");
      toast.success("导出 Excel 成功", { description: path });
    } catch (err) {
      console.error("Export Excel failed:", err);
      const msg = err instanceof Error ? err.message : String(err);
      setExportError(msg);
      setExportStatus("error");
      toast.error("导出 Excel 失败", { description: msg });
    } finally {
      setExporting(false);
    }
  };

  const handleExportJson = async () => {
    beginExport("json");
    try {
      // 阶段 1：后端查询并序列化为 JSON 字符串。
      const text = await exportRecordsJson(listFilter);
      const bytes = new TextEncoder().encode(text);

      // 阶段 2：弹出系统「另存为」让用户选路径。
      setExportPhase("pickPath");
      const defaultPath = await join(await desktopDir(), defaultJsonFilename());
      const path = await save({
        title: "导出 JSON",
        defaultPath,
        filters: [{ name: "JSON", extensions: ["json"] }],
      });
      if (path == null) {
        setExportStatus("cancelled");
        return;
      }

      // 阶段 3：把字节流写入磁盘。
      setExportPhase("writeFile");
      await writeExportFile(path, bytes);

      setExportPhase("done");
      setExportStatus("done");
      toast.success("导出 JSON 成功", { description: path });
    } catch (err) {
      console.error("Export JSON failed:", err);
      const msg = err instanceof Error ? err.message : String(err);
      setExportError(msg);
      setExportStatus("error");
      toast.error("导出 JSON 失败", { description: msg });
    } finally {
      setExporting(false);
    }
  };

  const handleDeduplicate = async () => {
    setDeduping(true);
    try {
      const removed = await deduplicateRecords(listFilter);
      console.log(`Deduplicated: removed ${removed} records`);
      fetchRecords();
    } catch (err) {
      console.error("Deduplicate failed:", err);
    } finally {
      setDeduping(false);
    }
  };

  const handleConfirmDeleteFiltered = async () => {
    setDeletingFiltered(true);
    try {
      const n = await deleteRecordsFiltered(listFilter);
      toast.success("已删除", { description: `共 ${n} 条记录` });
      setDeleteDialogOpen(false);
      void loadRecordTaskNames();
      void fetchRecords();
    } catch (err) {
      console.error("Delete filtered failed:", err);
      const msg = err instanceof Error ? err.message : String(err);
      toast.error("删除失败", { description: msg });
    } finally {
      setDeletingFiltered(false);
    }
  };

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-4 p-4">
      <div className="flex shrink-0 flex-wrap items-start justify-between gap-3">
        <div>
          <h1 className="text-2xl font-bold tracking-tight">数据管理</h1>
          <p className="text-sm text-muted-foreground">
            采集数据的统一存储、查询与导出
          </p>
        </div>
        <div className="flex flex-wrap gap-2">
          <Button
            variant="outline"
            size="sm"
            className="gap-1.5"
            onClick={() => {
              void loadRecordTaskNames();
              void fetchRecords();
            }}
            disabled={loading}
          >
            {loading ? (
              <Loader2Icon className="size-4 animate-spin" />
            ) : (
              <RefreshCwIcon className="size-4" />
            )}
            刷新
          </Button>
        </div>
      </div>

      {/* Filters */}
      <Card className="shrink-0">
        <CardHeader>
          <CardTitle className="flex items-center gap-2 text-base">
            <FilterIcon className="size-4" />
            筛选条件
          </CardTitle>
        </CardHeader>
        <CardContent>
          <div className="flex flex-wrap items-center gap-3">
                <Select
                  value={platformFilter}
                  onValueChange={(v) => {
                    setPlatformFilter(v as Platform | "all");
                    setTaskNameFilter(null);
                  }}
                >
                  <SelectTrigger className="w-40">
                    <SelectValue placeholder="选择平台" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="all">全部平台</SelectItem>
                    {Object.entries(PLATFORM_LABELS).map(([key, label]) => (
                      <SelectItem key={key} value={key}>
                        {label}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>

                <Select
                  value={
                    taskNameFilter === null
                      ? "all"
                      : taskNameToSelectValue(taskNameFilter)
                  }
                  onValueChange={(v) =>
                    setTaskNameFilter(selectValueToTaskName(v))
                  }
                >
                  <SelectTrigger className="w-52 min-w-[13rem]">
                    <SelectValue placeholder="选择任务" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="all">全部任务</SelectItem>
                    {recordTaskNames.map((name) => (
                      <SelectItem
                        key={name}
                        value={taskNameToSelectValue(name)}
                      >
                        {name}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>

                <Select
                  value={entityTypeFilter}
                  onValueChange={setEntityTypeFilter}
                >
                  <SelectTrigger className="w-40">
                    <SelectValue placeholder="选择类型" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="all">全部类型</SelectItem>
                    {ENTITY_TYPE_OPTIONS.map((o) => (
                      <SelectItem key={o.value} value={o.value}>
                        {o.label}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>

                <div className="relative min-w-[12rem] flex-1 basis-[200px]">
                  <SearchIcon className="absolute top-1/2 left-2.5 size-4 -translate-y-1/2 text-muted-foreground" />
                  <Input
                    placeholder="搜索"
                    className="pl-9"
                    value={searchQuery}
                    onChange={(e) => setSearchQuery(e.target.value)}
                  />
                </div>
              </div>
            </CardContent>
          </Card>

      {/* Data Table — 布局与「请求日志」网络请求记录一致：Card 撑满剩余高度 + FloatingScrollArea 包表 */}
      <Card className="flex min-h-0 flex-1 flex-col overflow-hidden">
        <CardHeader className="flex shrink-0 flex-row flex-wrap items-center justify-between gap-2 space-y-0 pb-3">
          <CardTitle className="flex h-7 min-w-0 items-center text-base leading-7">
            数据列表
          </CardTitle>
          <div className="flex flex-wrap items-center justify-end gap-2">
            <span className="text-muted-foreground text-xs">
              共 {total} 条
              {total > 0
                ? ` · 本页 ${(page - 1) * PAGE_SIZE + 1}–${Math.min(page * PAGE_SIZE, total)}`
                : ""}
            </span>
            <Button
              variant="outline"
              size="sm"
              className="gap-1.5 text-destructive hover:bg-destructive/10 hover:text-destructive"
              onClick={() => setDeleteDialogOpen(true)}
              disabled={
                loading ||
                deduping ||
                exporting ||
                deletingFiltered ||
                total === 0
              }
            >
              <Trash2Icon
                className={`size-4 ${deletingFiltered ? "animate-pulse" : ""}`}
              />
              删除当前数据
            </Button>
            <Button
              variant="outline"
              size="sm"
              className="gap-1.5 text-destructive hover:bg-destructive/10 hover:text-destructive"
              onClick={handleDeduplicate}
              disabled={deduping || loading || deletingFiltered}
            >
              <Trash2Icon
                className={`size-4 ${deduping ? "animate-pulse" : ""}`}
              />
              清除重复
            </Button>
            <Button
              variant="outline"
              size="sm"
              className="gap-1.5"
              onClick={handleExportJson}
              disabled={exporting || loading || deletingFiltered}
            >
              <BracesIcon className="size-4" />
              导出 JSON
            </Button>
            <TooltipProvider delayDuration={300}>
              <Tooltip>
                <TooltipTrigger asChild>
                  <span className="inline-flex">
                    <Button
                      variant="outline"
                      size="sm"
                      className="gap-1.5"
                      onClick={handleExportExcel}
                      disabled={exporting || loading || deletingFiltered}
                    >
                      <DownloadIcon className="size-4" />
                      导出 Excel
                    </Button>
                  </span>
                </TooltipTrigger>
                <TooltipContent
                  side="bottom"
                  align="end"
                  className="max-w-[20rem] text-left text-xs leading-relaxed"
                >
                  Excel 单格约 3.2 万字符上限，内容预览与 jsonData 等长字段可能被截断。需要完整文本或大体量数据时，请使用「导出 JSON」。
                </TooltipContent>
              </Tooltip>
            </TooltipProvider>
          </div>
        </CardHeader>
        <CardContent className="flex min-h-0 flex-1 flex-col overflow-hidden pt-0">
          <FloatingScrollArea className="min-h-0 flex-1">
            <div className="overflow-x-auto pr-2">
              <TooltipProvider delayDuration={200}>
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead className="whitespace-nowrap">平台</TableHead>
                      <TableHead className="whitespace-nowrap">任务</TableHead>
                      <TableHead className="whitespace-nowrap">类型</TableHead>
                      <TableHead className="whitespace-nowrap">关键词</TableHead>
                      <TableHead className="whitespace-nowrap">作者</TableHead>
                      <TableHead className="min-w-[200px] max-w-[320px]">
                        内容预览
                      </TableHead>
                      <TableHead className="whitespace-nowrap">采集时间</TableHead>
                      <TableHead className="w-16 whitespace-nowrap text-center">
                        其他
                      </TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {loading && records.length === 0 ? (
                      <TableRow>
                        <TableCell
                          colSpan={8}
                          className="text-muted-foreground h-24 text-center"
                        >
                          <Loader2Icon className="mx-auto size-6 animate-spin opacity-70" />
                        </TableCell>
                      </TableRow>
                    ) : records.length === 0 ? (
                      <TableRow>
                        <TableCell
                          colSpan={8}
                          className="text-muted-foreground h-24 text-center"
                        >
                          暂无数据。调整筛选条件或开始采集后将显示在此。
                        </TableCell>
                      </TableRow>
                    ) : (
                      records.map((record) => (
                        <TableRow key={record.id}>
                          <TableCell className="whitespace-nowrap">
                            <Badge variant="outline" className="text-xs">
                              {PLATFORM_LABELS[record.platform]}
                            </Badge>
                          </TableCell>
                          <TableCell className="max-w-[180px] truncate text-xs">
                            {record.taskName}
                          </TableCell>
                          <TableCell className="text-xs">
                            {record.entityType ? (
                              <span className="text-muted-foreground">
                                {ENTITY_LABELS[record.entityType] ??
                                  record.entityType}
                              </span>
                            ) : (
                              "—"
                            )}
                          </TableCell>
                          <TableCell className="max-w-[140px]">
                            <RecordKeywordCell record={record} />
                          </TableCell>
                          <TableCell className="max-w-[120px] text-xs">
                            <RecordAuthorLinkCell record={record} />
                          </TableCell>
                          <TableCell className="max-w-[320px] py-2">
                            <RecordContentPreviewCell record={record} />
                          </TableCell>
                          <TableCell className="whitespace-nowrap font-mono text-xs text-muted-foreground">
                            {record.crawledAt}
                          </TableCell>
                          <TableCell className="text-center">
                            <span
                              className="cursor-default text-xs text-muted-foreground"
                              title="另含 jsonData 等；可导出 JSON（完整）或 Excel 查看"
                            >
                              …
                            </span>
                          </TableCell>
                        </TableRow>
                      ))
                    )}
                  </TableBody>
                </Table>
              </TooltipProvider>
            </div>
          </FloatingScrollArea>
          <div className="mt-4 flex shrink-0 flex-wrap items-center justify-end gap-2 border-t pt-4">
            <span className="text-muted-foreground mr-auto text-xs">
              第 {page} / {totalPages} 页
            </span>
            <Button
              type="button"
              variant="outline"
              size="icon"
              className="size-8"
              disabled={loading || page <= 1}
              onClick={() => setPage(1)}
            >
              <ChevronsLeftIcon className="size-4" />
            </Button>
            <Button
              type="button"
              variant="outline"
              size="icon"
              className="size-8"
              disabled={loading || page <= 1}
              onClick={() => setPage((p) => Math.max(1, p - 1))}
            >
              <ChevronLeftIcon className="size-4" />
            </Button>
            <Button
              type="button"
              variant="outline"
              size="icon"
              className="size-8"
              disabled={loading || page >= totalPages}
              onClick={() => setPage((p) => Math.min(totalPages, p + 1))}
            >
              <ChevronRightIcon className="size-4" />
            </Button>
            <Button
              type="button"
              variant="outline"
              size="icon"
              className="size-8"
              disabled={loading || page >= totalPages}
              onClick={() => setPage(totalPages)}
            >
              <ChevronsRightIcon className="size-4" />
            </Button>
          </div>
        </CardContent>
      </Card>

      <AlertDialog open={deleteDialogOpen} onOpenChange={setDeleteDialogOpen}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>删除当前筛选下的数据？</AlertDialogTitle>
            <AlertDialogDescription className="space-y-2">
              <span>
                将永久删除与当前列表一致的数据（共{" "}
                <span className="font-medium text-foreground tabular-nums">
                  {total}
                </span>{" "}
                条），筛选维度与上方「筛选条件」相同，此操作不可恢复。
              </span>
              <span className="block">
                若未设置任何筛选，将删除库内全部采集数据。
              </span>
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={deletingFiltered}>取消</AlertDialogCancel>
            <Button
              variant="destructive"
              disabled={deletingFiltered}
              className="gap-2"
              onClick={() => void handleConfirmDeleteFiltered()}
            >
              {deletingFiltered ? (
                <Loader2Icon className="size-4 animate-spin" />
              ) : null}
              删除
            </Button>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      <ExportProgressDialog
        open={exportDialogOpen}
        format={exportFormat}
        phase={exportPhase}
        status={exportStatus}
        errorMessage={exportError}
        onOpenChange={setExportDialogOpen}
      />
    </div>
  );
}
