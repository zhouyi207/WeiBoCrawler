import { useCallback, useEffect, useRef, useState } from "react";
import { AlertCircleIcon, RotateCcwIcon } from "lucide-react";
import { toast } from "sonner";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { PLATFORMS, PLATFORM_LABELS } from "@/features/domain/types";
import {
  getWorkerBackoffSettings,
  updateWorkerBackoffSettings,
} from "@/services/tauri/commands";

const DEFAULT_BACKOFF_FALLBACK = 30;

interface Props {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

/**
 * 采集管理页：各平台 Worker 连续失败熔断后的退避秒数（与进度条「连续失败…退避…秒」一致）。
 */
export function CrawlBackoffSettingsDialog({ open, onOpenChange }: Props) {
  const [backoffByPlatform, setBackoffByPlatform] = useState<Record<string, number>>({});
  const [defaultBackoffSec, setDefaultBackoffSec] = useState(DEFAULT_BACKOFF_FALLBACK);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const submitInFlightRef = useRef(false);
  const [error, setError] = useState("");

  const reload = useCallback(async () => {
    setLoading(true);
    setError("");
    try {
      const backoff = await getWorkerBackoffSettings();
      setDefaultBackoffSec(backoff.defaultBackoffSeconds);
      const next: Record<string, number> = {};
      for (const p of PLATFORMS) {
        next[p] = backoff.secondsByPlatform[p] ?? backoff.defaultBackoffSeconds;
      }
      setBackoffByPlatform(next);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    if (open) void reload();
  }, [open, reload]);

  const backoffValid = PLATFORMS.every((p) => {
    const v = backoffByPlatform[p] ?? defaultBackoffSec;
    return Number.isFinite(v) && v >= 1 && v <= 3600;
  });
  const canSubmit = backoffValid && !saving && !loading;

  const handleSubmit = useCallback(async () => {
    if (submitInFlightRef.current || loading) return;
    const valid = PLATFORMS.every((p) => {
      const v = backoffByPlatform[p] ?? defaultBackoffSec;
      return Number.isFinite(v) && v >= 1 && v <= 3600;
    });
    if (!valid) return;
    submitInFlightRef.current = true;
    setSaving(true);
    try {
      const payload: Record<string, number> = {};
      for (const p of PLATFORMS) {
        const raw = backoffByPlatform[p] ?? defaultBackoffSec;
        payload[p] = Math.min(3600, Math.max(1, Math.floor(Number(raw))));
      }
      await updateWorkerBackoffSettings(payload);
      toast.success("采集熔断退避已保存", {
        description: "新值在后续采集周期生效。",
      });
      onOpenChange(false);
    } catch (e) {
      toast.error("保存失败", {
        description: e instanceof Error ? e.message : String(e),
      });
    } finally {
      submitInFlightRef.current = false;
      setSaving(false);
    }
  }, [backoffByPlatform, defaultBackoffSec, loading, onOpenChange]);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="flex max-h-[min(90vh,720px)] flex-col gap-0 overflow-hidden p-0 sm:max-w-2xl">
        <DialogHeader className="shrink-0 border-b border-border/60 px-4 py-3">
          <DialogTitle>采集熔断退避（秒）</DialogTitle>
          <DialogDescription>
            同一 worker 连续失败达到阈值后暂停的时长；按任务平台分别配置，与采集进度条提示一致。
          </DialogDescription>
        </DialogHeader>

        {error ? (
          <div className="shrink-0 px-4 pt-3">
            <Alert variant="destructive">
              <AlertCircleIcon />
              <AlertTitle>读取设置失败</AlertTitle>
              <AlertDescription>{error}</AlertDescription>
            </Alert>
          </div>
        ) : null}

        <div className="min-h-0 flex-1 space-y-4 overflow-y-auto px-4 py-3 pb-4">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <p className="text-xs text-muted-foreground">
              未单独配置的平台使用默认值 {defaultBackoffSec}s。
            </p>
            <Button
              variant="ghost"
              size="sm"
              className="h-8 gap-1 text-xs"
              type="button"
              onClick={() => {
                const o: Record<string, number> = {};
                for (const p of PLATFORMS) {
                  o[p] = defaultBackoffSec;
                }
                setBackoffByPlatform(o);
              }}
            >
              <RotateCcwIcon className="size-3" />
              全部恢复默认
            </Button>
          </div>

          <div className="grid gap-3 sm:grid-cols-2">
            {PLATFORMS.map((p) => (
              <div key={p} className="space-y-1.5">
                <div className="flex items-center justify-between gap-1">
                  <Label htmlFor={`crawl-backoff-${p}`} className="text-xs font-normal">
                    {PLATFORM_LABELS[p]}
                  </Label>
                  <Button
                    variant="ghost"
                    size="sm"
                    className="h-6 px-1.5 text-[10px]"
                    type="button"
                    onClick={() =>
                      setBackoffByPlatform((prev) => ({
                        ...prev,
                        [p]: defaultBackoffSec,
                      }))
                    }
                  >
                    默认
                  </Button>
                </div>
                <Input
                  id={`crawl-backoff-${p}`}
                  type="number"
                  min={1}
                  max={3600}
                  step={1}
                  value={backoffByPlatform[p] ?? defaultBackoffSec}
                  onChange={(e) => {
                    const n = parseInt(e.target.value, 10);
                    setBackoffByPlatform((prev) => ({
                      ...prev,
                      [p]: Number.isNaN(n) ? defaultBackoffSec : n,
                    }));
                  }}
                  autoComplete="off"
                />
              </div>
            ))}
          </div>
        </div>

        <DialogFooter className="shrink-0 !m-0 flex-row justify-end gap-2 border-t bg-muted/30 px-4 py-3">
          <Button
            variant="outline"
            onClick={() => onOpenChange(false)}
            disabled={saving}
            type="button"
          >
            取消
          </Button>
          <Button
            onClick={() => void handleSubmit()}
            disabled={!canSubmit}
            type="button"
          >
            {saving ? "保存中…" : "保存"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
