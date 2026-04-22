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

export type DualHealthRefreshStatus = "running" | "done" | "error";

export type DualHealthRefreshMode = "global" | "platform";

interface DualHealthRefreshDialogProps {
  open: boolean;
  mode: DualHealthRefreshMode;
  status: DualHealthRefreshStatus;
  /** 进行中时当前高亮步骤下标（由父组件用定时器推进，营造与导出 modal 类似的分段感）。 */
  activeStepIndex: number;
  errorMessage?: string | null;
  onOpenChange: (open: boolean) => void;
}

const GLOBAL_STEPS = [
  { label: "并行反查地理信息", progress: 38 },
  { label: "并行探测国内 / 国际延迟", progress: 72 },
  { label: "写回数据库并刷新列表", progress: 94 },
] as const;

const PLATFORM_STEPS = [
  { label: "拉取当前平台运行快照", progress: 45 },
  { label: "同步全局与其它平台缓存", progress: 88 },
] as const;

function titleForMode(mode: DualHealthRefreshMode): string {
  return mode === "global" ? "刷新并测延迟" : "刷新平台数据";
}

function descriptionFor(
  mode: DualHealthRefreshMode,
  status: DualHealthRefreshStatus,
): string {
  if (status === "done") {
    return mode === "global" ? "探测与写回已完成。" : "平台数据已刷新。";
  }
  if (status === "error") return "刷新过程中出现错误，请稍后重试。";
  return mode === "global"
    ? "正在为全部代理并行执行地理反查与国内 / 国际探针（单条约数秒至十余秒）…"
    : "正在拉取并同步运行数据…";
}

function progressValue(
  mode: DualHealthRefreshMode,
  status: DualHealthRefreshStatus,
  activeStepIndex: number,
): number {
  if (status === "done") return 100;
  const steps = mode === "global" ? GLOBAL_STEPS : PLATFORM_STEPS;
  const idx = Math.max(0, Math.min(activeStepIndex, steps.length - 1));
  return steps[idx]!.progress;
}

/**
 * IP 管理页「刷新并测延迟 / 平台刷新」进度对话框，布局与数据库导出进度 modal 一致。
 */
export function DualHealthRefreshDialog({
  open,
  mode,
  status,
  activeStepIndex,
  errorMessage,
  onOpenChange,
}: DualHealthRefreshDialogProps) {
  const closable = status !== "running";
  const steps = mode === "global" ? GLOBAL_STEPS : PLATFORM_STEPS;
  const value = progressValue(mode, status, activeStepIndex);
  const errStep = Math.min(activeStepIndex, steps.length - 1);

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (!next && !closable) return;
        onOpenChange(next);
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
            {titleForMode(mode)}
            {status === "running" && (
              <Loader2Icon className="size-4 animate-spin text-muted-foreground" />
            )}
          </DialogTitle>
          <DialogDescription>{descriptionFor(mode, status)}</DialogDescription>
        </DialogHeader>

        <div className="space-y-4 py-2">
          <Progress value={value} className="h-2" />

          <ul className="space-y-2 text-sm">
            {steps.map((meta, i) => {
              const completed =
                status === "done" ||
                (status === "running" && i < activeStepIndex);
              const active =
                status === "running" && i === activeStepIndex;
              const failed = status === "error" && i === errStep;

              return (
                <li key={meta.label} className="flex items-center gap-2">
                  <span
                    className={cn(
                      "flex size-5 shrink-0 items-center justify-center rounded-full border text-xs",
                      completed &&
                        "border-primary bg-primary text-primary-foreground",
                      active &&
                        !completed &&
                        "border-primary text-primary",
                      failed && "border-destructive text-destructive",
                      !completed &&
                        !active &&
                        !failed &&
                        "border-muted text-muted-foreground",
                    )}
                  >
                    {completed ? (
                      <CheckIcon className="size-3" />
                    ) : active ? (
                      <Loader2Icon className="size-3 animate-spin" />
                    ) : failed ? (
                      <XIcon className="size-3" />
                    ) : (
                      <span>{i + 1}</span>
                    )}
                  </span>
                  <span
                    className={cn(
                      "text-sm",
                      completed && "text-foreground",
                      active && "font-medium text-foreground",
                      failed && "text-destructive",
                      !completed &&
                        !active &&
                        !failed &&
                        "text-muted-foreground",
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
