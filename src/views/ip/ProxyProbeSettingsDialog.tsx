import { useCallback, useEffect, useState } from "react";
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
import { getProxyProbeSettings, updateProxyProbeSettings } from "@/services/tauri/commands";

/**
 * 「恢复默认」用到的 URL 由后端 `get_proxy_probe_settings` 在 response 里
 * 一并下发（`defaultCnTarget` / `defaultIntlTarget`），前端不再硬编码。
 *
 * 在第一次拉取成功之前，先用一个无害的占位 placeholder。
 */
const PLACEHOLDER_CN = "https://www.baidu.com/favicon.ico";
const PLACEHOLDER_INTL = "https://www.cloudflare.com/cdn-cgi/trace";

function isLikelyValidUrl(v: string): boolean {
  const trimmed = v.trim();
  if (!trimmed) return false;
  return trimmed.startsWith("http://") || trimmed.startsWith("https://");
}

interface Props {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

/**
 * IP 管理页：双探针目标 URL（「刷新并测延迟」所用）。
 */
export function ProxyProbeSettingsDialog({ open, onOpenChange }: Props) {
  const [cnTarget, setCnTarget] = useState("");
  const [intlTarget, setIntlTarget] = useState("");
  const [defaultCn, setDefaultCn] = useState(PLACEHOLDER_CN);
  const [defaultIntl, setDefaultIntl] = useState(PLACEHOLDER_INTL);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");

  const reload = useCallback(async () => {
    setLoading(true);
    setError("");
    try {
      const probe = await getProxyProbeSettings();
      setCnTarget(probe.cnTarget);
      setIntlTarget(probe.intlTarget);
      setDefaultCn(probe.defaultCnTarget);
      setDefaultIntl(probe.defaultIntlTarget);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    if (open) void reload();
  }, [open, reload]);

  const cnValid = isLikelyValidUrl(cnTarget);
  const intlValid = isLikelyValidUrl(intlTarget);
  const canSubmit = cnValid && intlValid && !saving && !loading;

  const handleSubmit = useCallback(async () => {
    if (!canSubmit) return;
    setSaving(true);
    try {
      await updateProxyProbeSettings({
        cnTarget: cnTarget.trim(),
        intlTarget: intlTarget.trim(),
      });
      toast.success("延迟探针设置已保存", {
        description: "将在下次「刷新并测延迟」时生效。",
      });
      onOpenChange(false);
    } catch (e) {
      toast.error("保存失败", {
        description: e instanceof Error ? e.message : String(e),
      });
    } finally {
      setSaving(false);
    }
  }, [canSubmit, cnTarget, intlTarget, onOpenChange]);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="flex max-h-[min(90vh,640px)] flex-col gap-0 overflow-hidden p-0 sm:max-w-lg">
        <DialogHeader className="shrink-0 border-b border-border/60 px-4 py-3">
          <DialogTitle>延迟探针设置</DialogTitle>
          <DialogDescription>
            配置国内 / 国外延迟探针目标 URL，用于 IP 管理页「刷新并测延迟」。
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
          <div className="space-y-2">
            <div className="flex items-center justify-between gap-2">
              <Label htmlFor="probe-cn">国内探针 URL</Label>
              <Button
                variant="ghost"
                size="sm"
                className="h-7 gap-1 text-xs"
                onClick={() => setCnTarget(defaultCn)}
                title={`恢复为默认值：${defaultCn}`}
                type="button"
              >
                <RotateCcwIcon className="size-3" />
                恢复默认
              </Button>
            </div>
            <Input
              id="probe-cn"
              value={cnTarget}
              onChange={(e) => setCnTarget(e.target.value)}
              placeholder={defaultCn}
              autoComplete="off"
              spellCheck={false}
            />
            {!cnValid && cnTarget.trim() && (
              <p className="text-xs text-destructive">必须以 http:// 或 https:// 开头</p>
            )}
          </div>

          <div className="space-y-2">
            <div className="flex items-center justify-between gap-2">
              <Label htmlFor="probe-intl">国外探针 URL</Label>
              <Button
                variant="ghost"
                size="sm"
                className="h-7 gap-1 text-xs"
                onClick={() => setIntlTarget(defaultIntl)}
                title={`恢复为默认值：${defaultIntl}`}
                type="button"
              >
                <RotateCcwIcon className="size-3" />
                恢复默认
              </Button>
            </div>
            <Input
              id="probe-intl"
              value={intlTarget}
              onChange={(e) => setIntlTarget(e.target.value)}
              placeholder={defaultIntl}
              autoComplete="off"
              spellCheck={false}
            />
            {!intlValid && intlTarget.trim() && (
              <p className="text-xs text-destructive">必须以 http:// 或 https:// 开头</p>
            )}
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
