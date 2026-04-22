const STORAGE_KEY = "ysscrawler-color-scheme";

export type ThemeMode = "light" | "dark";

export function getTheme(): ThemeMode {
  if (typeof window === "undefined") return "light";
  const raw = localStorage.getItem(STORAGE_KEY);
  if (raw === "dark" || raw === "light") return raw;
  return "light";
}

export function setTheme(mode: ThemeMode): void {
  localStorage.setItem(STORAGE_KEY, mode);
  document.documentElement.classList.toggle("dark", mode === "dark");
}

/** 在首屏渲染前调用，避免浅色闪一下再变暗 */
export function initTheme(): void {
  const mode = getTheme();
  document.documentElement.classList.toggle("dark", mode === "dark");
}

export function toggleTheme(): ThemeMode {
  const next: ThemeMode = getTheme() === "dark" ? "light" : "dark";
  setTheme(next);
  return next;
}
