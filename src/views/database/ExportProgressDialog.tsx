import { CheckIcon, Loader2Icon, XIcon } from "lucide-react";

import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Progress } from "@/components/ui/progress";
import { cn } from "@/lib/utils";

/**
 * 导出流程阶段。各阶段对应 `ExportProgressDialog` 顶部进度条上的一个固定百分比。
 *
 * - `query`     : 后端 `export_records_*` 同步查询并序列化数据（最长的阻塞等待）。
 * - `pickPath`  : 弹出系统「另存为」对话框，由用户选择保存路径。
 * - `writeFile` : 调用 `write_export_file` 把缓冲区写入磁盘。
 * - `done`      : 全部完成；外部应在短暂展示后关闭对话框。
 *
 * `cancelled` / `error` 不属于"进行中"阶段，单独通过 `status` 表达。
 */
export type ExportPhase = "query" | "pickPath" | "writeFile" | "done";

export type ExportStatus = "running" | "done" | "cancelled" | "error";

export type ExportFormat = "excel" | "json";

interface ExportProgressDialogProps {
  open: boolean;
  format: ExportFormat;
  phase: ExportPhase;
  status: ExportStatus;
  /** 失败时展示的错误信息；其余状态忽略。 */
  errorMessage?: string | null;
  /** 仅在 `done` / `cancelled` / `error` 时允许关闭，用户主动点叉时调用。 */
  onOpenChange?: (open: boolean) => void;
}

/** 阶段 → (label, 进度百分比, 排序权重)。`done` 不出现在阶段列表里。 */
const PHASE_META: Record<
  Exclude<ExportPhase, "done">,
  { label: string; progress: number; order: number }
> = {
  query: { label: "查询并生成数据", progress: 55, order: 0 },
  pickPath: { label: "选择保存位置", progress: 70, order: 1 },
  writeFile: { label: "写入文件", progress: 95, order: 2 },
};

const PHASE_ORDER: Exclude<ExportPhase, "done">[] = ["query", "pickPath", "writeFile"];

function formatTitle(format: ExportFormat): string {
  return format === "excel" ? "导出 Excel" : "导出 JSON";
}

function statusDescription(status: ExportStatus, phase: ExportPhase): string {
  if (status === "done") return "导出已完成。";
  if (status === "cancelled") return "已取消导出（用户未选择保存位置）。";
  if (status === "error") return "导出过程中出现错误，请稍后重试。";
  if (phase === "done") return "即将完成…";
  return PHASE_META[phase].label + "中…";
}

/**
 * 计算进度条数值。
 *
 * - `running` 阶段：取阶段起点（让条体落在该阶段对应的位置；动态填充由阶段字幕表达）。
 * - `done`：100。
 * - `cancelled`：保留触发时的阶段进度（视觉上"卡住"，配合标题灰色化）。
 * - `error`：同上，由调用方决定是否清空。
 */
function progressValue(status: ExportStatus, phase: ExportPhase): number {
  if (status === "done" || phase === "done") return 100;
  return PHASE_META[phase].progress;
}

/**
 * 导出进度对话框：单文件展示当前所处阶段（查询 → 选择路径 → 写入 → 完成），
 * 顶部为整体进度条，下方为分阶段状态列表。
 *
 * 设计要点：
 * - 后端导出命令是"原子式"返回（一次性回传完整字节/字符串），无法上报真实进度，
 *   所以这里采用阶段化伪进度（snap 到固定百分比），同时用 spinner 表达"进行中"。
 * - 进行中（`running`）禁止用户通过点击遮罩或叉按钮关闭，避免误中断；完成 / 取消 /
 *   失败后才允许关闭，由 `onOpenChange` 通知外部清理状态。
 */
export function ExportProgressDialog({
  open,
  format,
  phase,
  status,
  errorMessage,
  onOpenChange,
}: ExportProgressDialogProps) {
  const closable = status !== "running";
  const value = progressValue(status, phase);

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (!next && !closable) return;
        onOpenChange?.(next);
      }}
    >
      <DialogContent
        className="sm:max-w-md"
        showCloseButton={closable}
        onEscapeKeyDown={(e) => {
          if (!closable) e.preventDefault();
        }}
      >
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            {formatTitle(format)}
            {status === "running" && (
              <Loader2Icon className="size-4 animate-spin text-muted-foreground" />
            )}
          </DialogTitle>
          <DialogDescription>{statusDescription(status, phase)}</DialogDescription>
        </DialogHeader>

        <div className="space-y-4 py-2">
          <Progress value={value} className="h-2" />

          <ul className="space-y-2 text-sm">
            {PHASE_ORDER.map((p) => {
              const meta = PHASE_META[p];
              const currentOrder = phase === "done" ? PHASE_ORDER.length : PHASE_META[phase].order;
              const completed =
                status === "done" || (status === "running" && meta.order < currentOrder);
              const active = status === "running" && phase === p;
              const failed =
                (status === "cancelled" || status === "error") && phase === p;

              return (
                <li key={p} className="flex items-center gap-2">
                  <span
                    className={cn(
                      "flex size-5 shrink-0 items-center justify-center rounded-full border text-xs",
                      completed && "border-primary bg-primary text-primary-foreground",
                      active && "border-primary text-primary",
                      failed && "border-destructive text-destructive",
                      !completed && !active && !failed && "border-muted text-muted-foreground",
                    )}
                  >
                    {completed ? (
                      <CheckIcon className="size-3" />
                    ) : active ? (
                      <Loader2Icon className="size-3 animate-spin" />
                    ) : failed ? (
                      <XIcon className="size-3" />
                    ) : (
                      <span>{meta.order + 1}</span>
                    )}
                  </span>
                  <span
                    className={cn(
                      "text-sm",
                      completed && "text-foreground",
                      active && "font-medium text-foreground",
                      failed && "text-destructive",
                      !completed && !active && !failed && "text-muted-foreground",
                    )}
                  >
                    {meta.label}
                  </span>
                </li>
              );
            })}
          </ul>

          {status === "error" && errorMessage && (
            <p className="rounded-md border border-destructive/40 bg-destructive/5 p-2 text-xs text-destructive">
              {errorMessage}
            </p>
          )}
        </div>
      </DialogContent>
    </Dialog>
  );
}
