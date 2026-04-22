import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { openUrl } from "@tauri-apps/plugin-opener";
import { MinusIcon, MoonIcon, SunIcon, XIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { getTheme, toggleTheme } from "@/app/theme";
import { useCloseGuard } from "@/app/CloseGuard";

const WEIBO_CRAWLER_GITHUB = "https://github.com/zhouyi207/WeiBoCrawler";

/**
 * 与页面 `header` 同一行使用的窗口控制区（GitHub / 主题 / 最小化 / 最大化 / 关闭）。
 * 配合 `tauri.conf.json` 的 `decorations: false`；左侧拖拽区由调用方在 SidebarTrigger + 标题外包一层
 * `data-tauri-drag-region`。
 */
export function WindowControls() {
  const { requestClose } = useCloseGuard();
  const [maximized, setMaximized] = useState(false);
  const [isDark, setIsDark] = useState(() => getTheme() === "dark");

  useEffect(() => {
    const win = getCurrentWindow();
    let unlisten: (() => void) | undefined;

    void win.isMaximized().then(setMaximized);
    void win
      .onResized(() => {
        void win.isMaximized().then(setMaximized);
      })
      .then((u) => {
        unlisten = u;
      });

    return () => {
      unlisten?.();
    };
  }, []);

  const win = getCurrentWindow();

  return (
    <div className="flex h-full shrink-0">
      <button
        type="button"
        onClick={() => void openUrl(WEIBO_CRAWLER_GITHUB)}
        className="flex h-full w-12 items-center justify-center text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
        aria-label="WeiBoCrawler GitHub"
        title="WeiBoCrawler GitHub"
      >
        <GitHubGlyph />
      </button>
      <Button
        type="button"
        variant="ghost"
        size="icon"
        onClick={() => setIsDark(toggleTheme() === "dark")}
        className="h-full w-12 shrink-0 rounded-none text-muted-foreground hover:bg-muted hover:text-foreground focus-visible:ring-0"
        aria-label={isDark ? "切换到浅色主题" : "切换到深色主题"}
        title={isDark ? "切换到浅色主题" : "切换到深色主题"}
      >
        {isDark ? <SunIcon className="size-4" /> : <MoonIcon className="size-4" />}
      </Button>
      <button
        type="button"
        onClick={() => void win.minimize()}
        className="flex h-full w-12 items-center justify-center text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
        aria-label="最小化"
        title="最小化"
      >
        <MinusIcon className="size-4" />
      </button>
      <button
        type="button"
        onClick={() => void win.toggleMaximize()}
        className="flex h-full w-12 items-center justify-center text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
        aria-label={maximized ? "还原" : "最大化"}
        title={maximized ? "还原" : "最大化"}
      >
        {maximized ? <RestoreGlyph /> : <MaximizeGlyph />}
      </button>
      <button
        type="button"
        onClick={() => void requestClose()}
        className="group flex h-full w-12 items-center justify-center text-muted-foreground transition-colors hover:bg-[#e81123] hover:text-white"
        aria-label="关闭"
        title="关闭"
      >
        <XIcon className="size-4" />
      </button>
    </div>
  );
}

function GitHubGlyph() {
  return (
    <svg
      width={16}
      height={16}
      viewBox="0 0 98 96"
      className="size-4 shrink-0"
      aria-hidden
      focusable="false"
    >
      <path
        fill="currentColor"
        fillRule="evenodd"
        clipRule="evenodd"
        d="M48.854 0C21.839 0 0 22 0 49.217c0 21.756 13.993 40.172 33.405 46.69 2.427.49 3.316-1.059 3.316-2.362 0-1.141-.08-5.052-.08-9.127-13.59 2.934-16.42-5.867-16.42-5.867-2.184-5.704-5.42-7.17-5.42-7.17-4.448-3.015.324-3.015.324-3.015 4.934.326 7.523 5.052 7.523 5.052 4.367 7.496 11.404 5.378 14.235 4.074.404-3.178 1.699-5.378 3.074-6.6-10.839-1.141-22.243-5.378-22.243-24.283 0-5.378 1.94-9.778 5.014-13.2-.485-1.222-2.184-6.275.486-13.038 0 0 4.125-1.304 13.426 5.052a46.97 46.97 0 0 1 12.214-1.63c4.125 0 8.33.571 12.213 1.63 9.302-6.356 13.427-5.052 13.427-5.052 2.67 6.763.97 11.816.485 13.038 3.155 3.422 5.015 7.822 5.015 13.2 0 18.905-11.404 23.06-22.324 24.283 1.78 1.548 3.316 4.481 3.316 9.126 0 6.6-.08 11.897-.08 13.526 0 1.304.89 2.853 3.316 2.364 19.412-6.52 33.405-24.935 33.405-46.691C97.707 22 75.788 0 48.854 0z"
      />
    </svg>
  );
}

function MaximizeGlyph() {
  return (
    <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden focusable="false">
      <rect x="0.5" y="0.5" width="9" height="9" fill="none" stroke="currentColor" />
    </svg>
  );
}

function RestoreGlyph() {
  return (
    <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden focusable="false">
      <rect x="0.5" y="2.5" width="7" height="7" fill="none" stroke="currentColor" />
      <path d="M2.5 2.5 V0.5 H9.5 V7.5 H7.5" fill="none" stroke="currentColor" />
    </svg>
  );
}
