import type { PageId } from "@/features/domain/types";

/** 应用内路径（与 `react-router` 的 `Route` `path` 一致，不含 basename 前缀） */
export const PAGE_PATH: Record<PageId, string> = {
  home: "/",
  crawl: "/crawl",
  account: "/account",
  ip: "/ip",
  database: "/database",
};

export const PAGE_TITLE: Record<PageId, string> = {
  home: "主页",
  crawl: "采集",
  account: "账号",
  ip: "IP 代理",
  database: "数据库",
};

/** 从 `location.pathname` 解析当前页（用于顶栏标题等） */
export function getPageIdFromPathname(pathname: string): PageId {
  const p = pathname.replace(/\/+$/, "") || "/";
  if (p === "/") return "home";
  const seg = p.slice(1).split("/")[0];
  if (
    seg === "crawl" ||
    seg === "account" ||
    seg === "ip" ||
    seg === "database"
  ) {
    return seg;
  }
  return "home";
}
