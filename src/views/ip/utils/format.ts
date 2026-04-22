import {
  PLATFORM_LABELS,
  type LatencyOutcome,
  type ProxyIp,
  type ProxyPlatformRow,
} from "@/features/domain/types";

/**
 * IP 管理页用到的纯展示工具函数集合。
 * 提取出来便于子组件复用 + 单测，主页面只剩状态机。
 */

/** 风险系数颜色：0~20 绿、21~50 黄、>50 红。 */
export function riskColor(score: number): string {
  if (score <= 20) return "text-green-600";
  if (score <= 50) return "text-yellow-600";
  return "text-destructive";
}

/** 「最后一次响应延迟」(ms) 颜色：成功用对应档位，失败 / 未知都灰。 */
export function lastLatencyColor(ms: number | undefined): string {
  if (ms == null || ms <= 0) return "text-muted-foreground";
  if (ms <= 100) return "text-green-600";
  if (ms <= 300) return "text-yellow-600";
  return "text-destructive";
}

/** 「最后一次响应延迟」文案。失败 / 未知都返回「—」，由 lastStatus 表达失败。 */
export function formatLastLatency(ms: number | undefined): string {
  if (ms == null || ms <= 0) return "—";
  return `${ms}ms`;
}

/** LatencyOutcome → 颜色 class。 */
export function latencyOutcomeColor(o: LatencyOutcome | undefined): string {
  if (!o) return "text-muted-foreground";
  switch (o.kind) {
    case "untested":
      return "text-muted-foreground";
    case "failed":
      return "text-destructive";
    case "success":
      if (o.ms <= 100) return "text-green-600";
      if (o.ms <= 300) return "text-yellow-600";
      return "text-destructive";
  }
}

/** LatencyOutcome → 主体文案（仅 ms / 失败 / 「—」）。 */
export function formatLatencyOutcome(o: LatencyOutcome | undefined): string {
  if (!o) return "—";
  switch (o.kind) {
    case "untested":
      return "—";
    case "failed":
      return "失败";
    case "success":
      return `${o.ms}ms`;
  }
}

/** LatencyOutcome → tooltip 描述（含探测时间）。 */
export function latencyOutcomeTooltip(
  o: LatencyOutcome | undefined,
  scope: "国内" | "国外",
): string {
  if (!o || o.kind === "untested") {
    return `${scope}探针尚未探测；点上方「刷新并测延迟」`;
  }
  return `${scope}探针上次完成于 ${o.probedAt}`;
}

/** 把代理 IP 的 country / region / city 拼成「国家 · 省 · 城市」，自动去重相邻段。 */
export function formatGeoLocation(ip: ProxyIp): string {
  const parts = [ip.geoCountry, ip.geoRegion, ip.geoCity]
    .map((s) => (s ?? "").trim())
    .filter(Boolean);
  const dedup: string[] = [];
  for (const p of parts) {
    if (dedup[dedup.length - 1] !== p) dedup.push(p);
  }
  return dedup.join(" · ");
}

/**
 * 代理地址的安全展示：从 `scheme://user:pwd@host:port/...` 中只剥出 `host:port`，
 * 不暴露账号密码（用于列表、toast、下拉项等所有前端展示）。
 */
export function formatProxyEndpoint(address: string): string {
  const trimmed = address.trim();
  if (!trimmed) return address;
  const schemeIdx = trimmed.indexOf("://");
  const noScheme = schemeIdx >= 0 ? trimmed.slice(schemeIdx + 3) : trimmed;
  const atIdx = noScheme.lastIndexOf("@");
  let hostPort = atIdx >= 0 ? noScheme.slice(atIdx + 1) : noScheme;
  hostPort = hostPort.split("/")[0]?.split("?")[0] ?? hostPort;
  return hostPort.length > 0 ? hostPort : address;
}

/** 平台 tag → 中文展示名，未识别 tag 原样返回。 */
export function platformLabel(p: string): string {
  return (PLATFORM_LABELS as Record<string, string>)[p] ?? p;
}

/**
 * 错误归类 tag → 中文标签。与后端 `risk::ErrorKind::as_tag` 一一对齐。
 * 用于「最后一次响应状态」展示，避免 raw `network` / `http_status` 这种英文 tag
 * 直接漏到 UI 里。
 */
export const ERROR_KIND_LABELS: Record<string, string> = {
  network: "网络",
  login_required: "登录失效",
  business_reject: "业务拒绝",
  http_status: "HTTP",
  other: "其它",
};

/** 把 ProxyPlatformRow 的「最后一次响应」字段渲染成 (label, className) 对。 */
export function describeLastStatus(row: ProxyPlatformRow): {
  label: string;
  className: string;
} {
  if (!row.lastStatus) return { label: "—", className: "text-muted-foreground" };
  if (row.lastStatus === "success") {
    return { label: "成功", className: "text-green-600" };
  }
  // failure：附带 (httpStatus) 或 (errorKind 中文) 提示
  const hint =
    row.lastHttpStatus != null
      ? String(row.lastHttpStatus)
      : ERROR_KIND_LABELS[row.lastErrorKind ?? "other"] ?? "未知";
  return { label: `失败 (${hint})`, className: "text-destructive" };
}

/** 三档 IP 状态 → 徽章配置，与后端 IpStatus 对齐。 */
export const PROXY_STATUS_MAP: Record<
  string,
  { label: string; variant: "default" | "secondary" | "destructive"; hint: string }
> = {
  available: {
    label: "可用",
    variant: "default",
    hint: "近 5 分钟内未触发受限阈值，可正常调度。",
  },
  restricted: {
    label: "受限",
    variant: "secondary",
    hint: "近 5 分钟在该平台 scope 触发受限阈值；其他平台不受影响，5 分钟静默后自动回落。",
  },
  invalid: {
    label: "失效",
    variant: "destructive",
    hint: "近 5 分钟内累计 ≥10 次网络类错误（超时、无法连通、DNS 失败等），判定出口不可用；与单纯业务 HTTP 码无关。",
  },
};
