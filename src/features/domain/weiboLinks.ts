import type { CrawledRecord } from "./types";

/** 解析 `jsonData` 中的发布者 uid（数字串）。支持正文 API 的 `user.idstr`。 */
function parseWeiboUid(j: Record<string, unknown>): string | null {
  let raw: unknown = j.f_uid ?? j.uid;
  if (raw == null && j.user && typeof j.user === "object" && j.user !== null) {
    const u = j.user as Record<string, unknown>;
    raw = u.idstr ?? u.id;
  }
  const uid =
    typeof raw === "string"
      ? raw.trim()
      : raw != null
        ? String(raw).trim()
        : "";
  return /^\d+$/.test(uid) ? uid : null;
}

/** 解析详情页路径第二段：mblogid 或 mid。 */
function parsePathSecond(
  j: Record<string, unknown>,
  record: CrawledRecord,
): string | null {
  const et = record.entityType;
  if (et === "comment_l1" || et === "comment_l2") {
    const b = record.blogId?.trim();
    if (b) return b;
    const k = record.keyword?.trim();
    return k && k.length > 0 ? k : null;
  }
  const mb = j.mblogid ?? j.mid;
  if (typeof mb === "string" && mb.trim()) return mb.trim();
  if (typeof mb === "number") return String(mb);
  const b = record.blogId?.trim();
  if (b) return b;
  const k = record.keyword?.trim();
  return k && k.length > 0 ? k : null;
}

/**
 * 可打开的微博正文页：`https://weibo.com/{uid}/{mblogid}`。
 * 评论类优先用 `blogId`；feed/body 从 JSON 或 `blogId` 取 mblogid/mid。
 */
export function weiboStatusDetailUrl(record: CrawledRecord): string | null {
  if (record.platform !== "weibo" || !record.jsonData) return null;
  try {
    const j = JSON.parse(record.jsonData) as Record<string, unknown>;
    const uid = parseWeiboUid(j);
    if (!uid) return null;
    const second = parsePathSecond(j, record);
    if (!second) return null;
    return `https://weibo.com/${uid}/${second}`;
  } catch {
    return null;
  }
}

/** 评论：`user.id` / `idstr`；列表 feed 常在根级 `uid`。 */
function parseAuthorUidFromJson(j: Record<string, unknown>): string | null {
  if (j.user && typeof j.user === "object" && j.user !== null) {
    const u = j.user as Record<string, unknown>;
    const raw = u.idstr ?? u.id;
    const uid =
      typeof raw === "string"
        ? raw.trim()
        : raw != null
          ? String(raw).trim()
          : "";
    if (/^\d+$/.test(uid)) return uid;
  }
  const root = j.uid;
  if (typeof root === "string" && /^\d+$/.test(root.trim())) return root.trim();
  if (typeof root === "number") return String(Math.trunc(root));
  return null;
}

/**
 * 作者个人主页：`https://weibo.com/u/{uid}`。
 * uid 来自 `jsonData.user.id`（或 `idstr`）；无有效 uid 时返回 null。
 */
export function weiboAuthorProfileUrl(record: CrawledRecord): string | null {
  if (record.platform !== "weibo" || !record.jsonData) return null;
  try {
    const j = JSON.parse(record.jsonData) as Record<string, unknown>;
    const uid = parseAuthorUidFromJson(j);
    if (!uid) return null;
    return `https://weibo.com/u/${uid}`;
  } catch {
    return null;
  }
}
