import { getUiTheme, setUiTheme as persistUiThemeToDb } from "@/services/tauri/commands";

/** 与 `index.html` 内联预加载脚本使用同一键名。 */
export const THEME_STORAGE_KEY = "ysscrawler-color-scheme";

const STORAGE_KEY = THEME_STORAGE_KEY;

export type ThemeMode = "light" | "dark";

function writeLocalStorage(mode: ThemeMode): void {
  if (typeof window === "undefined") return;
  try {
    localStorage.setItem(STORAGE_KEY, mode);
  } catch {
    // 私密模式 / 配额等：仍尽量保持 DOM 与数据库一致
  }
}

function applyDom(mode: ThemeMode): void {
  const dark = mode === "dark";
  document.documentElement.classList.toggle("dark", dark);
  document.documentElement.style.colorScheme = dark ? "dark" : "light";
}

/** 同步本地缓存与 DOM（不写数据库，供 hydrate 或仅本地场景使用）。 */
export function applyThemeLocally(mode: ThemeMode): void {
  writeLocalStorage(mode);
  applyDom(mode);
}

export function getTheme(): ThemeMode {
  if (typeof window === "undefined") return "light";
  const raw = localStorage.getItem(STORAGE_KEY);
  if (raw === "dark" || raw === "light") return raw;
  return "light";
}

export function setTheme(mode: ThemeMode): void {
  applyThemeLocally(mode);
  void persistUiThemeToDb(mode).catch(() => {
    // 浏览器直开 Vite、或非 Tauri 环境
  });
}

/** 仅用 localStorage + DOM；桌面端入口请用 `hydrateThemeFromBackend`。 */
export function initTheme(): void {
  applyThemeLocally(getTheme());
}

/**
 * 桌面端以 SQLite `app_settings.ui.theme` 为准；缺省时回退 localStorage 并回写数据库。
 * 须在 `ReactDOM.createRoot(...).render` 之前 await，减少首帧主题错误。
 */
export async function hydrateThemeFromBackend(): Promise<void> {
  try {
    const mode = await getUiTheme();
    if (mode === "dark" || mode === "light") {
      applyThemeLocally(mode);
      return;
    }
  } catch {
    initTheme();
    return;
  }

  initTheme();
  try {
    await persistUiThemeToDb(getTheme());
  } catch {
    // 非 Tauri
  }
}

export function toggleTheme(): ThemeMode {
  const next: ThemeMode = getTheme() === "dark" ? "light" : "dark";
  setTheme(next);
  return next;
}
