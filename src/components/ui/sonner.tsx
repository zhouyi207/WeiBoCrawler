import { Toaster as Sonner, type ToasterProps } from "sonner"

/**
 * 全局 Toast 容器（基于 sonner）。挂在 App 壳层一次即可，
 * 业务侧使用 `import { toast } from "sonner"` 触发。
 *
 * 注意：颜色直接读取 CSS 变量（`--popover` / `--popover-foreground` / `--border`），
 * 与 shadcn 主题保持一致；明暗模式会自动切换。
 */
function Toaster(props: ToasterProps) {
  return (
    <Sonner
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
  )
}

export { Toaster }
