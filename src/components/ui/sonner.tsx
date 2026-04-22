import { useSyncExternalStore } from "react";
import { Toaster as Sonner, type ToasterProps } from "sonner";

const SONNER_CLOSE_FIX_STYLE_ID = "ysscrawler-sonner-close-button-theme";

/**
 * Sonner 在自身模块顶层用 `__insertCSS` 注入样式，关闭钮规则里 `color: var(--gray12)` 没有
 * `!important`；其 `[data-sonner-theme=dark]` 派生规则也未带 `!important`。但实测在某些
 * 主题切换时序下 `color` 仍可能落到浅色盘上的近黑，× 不可见。
 *
 * 这里在 **模块加载** 时（晚于 `import "sonner"`）向 head 末尾追加一段 `!important` 覆盖：
 * - 同时设置按钮自身 `color` 与子级 `<svg>` 的 `stroke`（图标用 `stroke="currentColor"`，
 *   保险起见再写一遍 `stroke`）；
 * - 颜色全部接到 shadcn token，明暗模式都会自动适配。
 */
function injectSonnerCloseButtonOverride() {
  if (typeof document === "undefined") return;
  if (document.getElementById(SONNER_CLOSE_FIX_STYLE_ID)) return;
  const el = document.createElement("style");
  el.id = SONNER_CLOSE_FIX_STYLE_ID;
  el.textContent = `
[data-sonner-toast][data-styled="true"] [data-close-button] {
  color: var(--popover-foreground) !important;
  background-color: var(--popover) !important;
  border-color: var(--border) !important;
  opacity: 1 !important;
}
[data-sonner-toast][data-styled="true"] [data-close-button] svg {
  stroke: var(--popover-foreground) !important;
  color: var(--popover-foreground) !important;
}
[data-sonner-toast][data-styled="true"]:hover [data-close-button]:hover {
  background-color: var(--muted) !important;
  border-color: var(--border) !important;
  color: var(--popover-foreground) !important;
}
[data-sonner-toast][data-styled="true"]:hover [data-close-button]:hover svg {
  stroke: var(--popover-foreground) !important;
}
`;
  document.head.appendChild(el);
}

injectSonnerCloseButtonOverride();

function subscribeHtmlClass(callback: () => void) {
  const mo = new MutationObserver(callback);
  mo.observe(document.documentElement, {
    attributes: true,
    attributeFilter: ["class"],
  });
  return () => mo.disconnect();
}

function snapshotSonnerTheme(): "light" | "dark" {
  return document.documentElement.classList.contains("dark")
    ? ("dark" as const)
    : ("light" as const);
}

/**
 * 全局 Toast 容器（基于 sonner）。挂在 App 壳层一次即可，
 * 业务侧使用 `import { toast } from "sonner"` 触发。
 */
function Toaster(props: ToasterProps) {
  const theme = useSyncExternalStore(
    subscribeHtmlClass,
    snapshotSonnerTheme,
    (): "light" | "dark" => "light",
  );

  return (
    <Sonner
      theme={theme}
      richColors
      closeButton
      position="bottom-right"
      style={
        {
          "--normal-bg": "var(--popover)",
          "--normal-text": "var(--popover-foreground)",
          "--normal-border": "var(--border)",
        } as React.CSSProperties
      }
      {...props}
    />
  );
}

export { Toaster };
