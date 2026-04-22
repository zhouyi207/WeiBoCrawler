import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import {
  WeiboBindAlertsSection,
  WeiboBindPoolsSection,
} from "@/views/crawl/CreateTaskModalWeiboBind";
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
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { cn } from "@/lib/utils";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { AlertCircleIcon, InfoIcon } from "lucide-react";
import { toast } from "sonner";
import {
  LOCAL_DIRECT_PROXY_ID,
  TASK_TYPE_LABELS,
  type Platform,
  type TaskType,
  type CrawlStrategy,
  type RateLimitScope,
  type Account,
  type ProxyHealthBrief,
  type ProxyIp,
  type WeiboTaskPayload,
  type CrawlTask,
} from "@/features/domain/types";
import {
  createTask,
  listAccounts,
  listProxies,
  listProxiesHealth,
  updateTask,
} from "@/services/tauri/commands";

/**
 * 后端可能存 `YYYY-MM-DD`、`YYYY-MM-DD-H`，或历史数据里的 ISO 日期前缀；
 * `<input type="date">` 只取 `YYYY-MM-DD`。
 */
function toDateInputValue(s: string | null | undefined): string {
  if (!s) return "";
  const t = s.trim();
  if (t.length >= 10 && /^\d{4}-\d{2}-\d{2}/.test(t)) {
    return t.slice(0, 10);
  }
  return "";
}

const STRATEGY_LABELS: Record<CrawlStrategy, string> = {
  round_robin: "轮询",
  random: "随机",
};

const STRATEGIES = Object.keys(STRATEGY_LABELS) as CrawlStrategy[];

const RATE_LIMIT_SCOPE_LABELS: Record<RateLimitScope, string> = {
  per_worker: "每 worker",
  per_account: "按账号共享",
};

const RATE_LIMIT_SCOPES = Object.keys(
  RATE_LIMIT_SCOPE_LABELS,
) as RateLimitScope[];

/** 暂未实现 / 暂下线的任务类型，仅在「新建/编辑任务」选择器中隐藏。
 * 不修改 `TASK_TYPE_LABELS`，使既有此类型的历史任务在列表 / 数据页仍能正常显示标签。 */
const HIDDEN_TASK_TYPES: TaskType[] = [
  "trending",
  "comment_level1",
  "comment_level2",
];
const ALL_TASK_TYPES = (Object.keys(TASK_TYPE_LABELS) as TaskType[]).filter(
  (t) => !HIDDEN_TASK_TYPES.includes(t),
);
const NON_WEIBO_TASK_TYPES: TaskType[] = ["keyword", "user_profile"];

const LIST_KINDS = ["综合", "实时", "高级"] as const;
const ADVANCED_KINDS = ["综合", "热度", "原创"] as const;

function buildWeiboPayload(
  taskType: TaskType,
  opts: {
    searchFor: string;
    listKind: string;
    advancedKind: string;
    timeStart: string;
    timeEnd: string;
    bodyIdsText: string;
    commentUidText: string;
    commentMidText: string;
  }
): WeiboTaskPayload | null {
  switch (taskType) {
    case "trending":
      return null;
    case "keyword":
      return {
        api: "list",
        search_for: opts.searchFor.trim(),
        list_kind: opts.listKind,
        advanced_kind:
          opts.listKind === "高级" ? opts.advancedKind : null,
        time_start:
          opts.listKind === "高级" && opts.timeStart.trim()
            ? opts.timeStart.trim()
            : null,
        time_end:
          opts.listKind === "高级" && opts.timeEnd.trim()
            ? opts.timeEnd.trim()
            : null,
      };
    case "user_profile": {
      const ids = opts.bodyIdsText.trim().split(/\s+/).filter(Boolean);
      return { api: "body", status_ids: ids };
    }
    case "comment_level1":
    case "comment_level2": {
      const uids = opts.commentUidText.trim().split(/\s+/).filter(Boolean);
      const mids = opts.commentMidText.trim().split(/\s+/).filter(Boolean);
      if (uids.length !== mids.length) {
        throw new Error("uid 与 mid 条数须一致（空格分隔，与 WeiBoCrawler 列表搜索页相同）");
      }
      const pairs = uids.map((uid, i) => ({ uid, mid: mids[i]! }));
      return taskType === "comment_level1"
        ? { api: "comment_l1", pairs }
        : { api: "comment_l2", pairs };
    }
    default:
      return null;
  }
}

export function CreateTaskModal({
  open,
  onOpenChange,
  platform,
  editingTask,
  onCreated,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  platform: Platform;
  /** 非空时为编辑模式（不可改平台与任务类型） */
  editingTask?: CrawlTask | null;
  onCreated?: () => void | Promise<void>;
}) {
  const effectivePlatform = editingTask?.platform ?? platform;

  /** 新建时隐藏部分类型；编辑时必须包含当前任务类型，否则 Select 无法匹配、标签会空白。 */
  const taskTypesForPlatform = useMemo(() => {
    const base =
      effectivePlatform === "weibo" ? ALL_TASK_TYPES : NON_WEIBO_TASK_TYPES;
    const t = editingTask?.type;
    if (t && !base.includes(t)) {
      return [...base, t];
    }
    return base;
  }, [effectivePlatform, editingTask?.type]);

  const [name, setName] = useState("");
  const [taskType, setTaskType] = useState<TaskType>("keyword");
  const [strategy, setStrategy] = useState<CrawlStrategy>("round_robin");
  const [rateLimit, setRateLimit] = useState(60);
  const [rateLimitScope, setRateLimitScope] =
    useState<RateLimitScope>("per_worker");
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [accountsLoading, setAccountsLoading] = useState(false);
  const [accountsError, setAccountsError] = useState("");
  const [selectedAccountIds, setSelectedAccountIds] = useState<string[]>([]);
  const [proxies, setProxies] = useState<ProxyIp[]>([]);
  const [proxiesLoading, setProxiesLoading] = useState(false);
  const [proxiesError, setProxiesError] = useState("");
  /**
   * 派生健康档位（id → ProxyHealthBrief）。v4 / 方案 C 起结构是
   * `{ globalStatus, restrictions[] }`：
   * - `globalStatus === "invalid"` → 出口连不上，所有平台都不能选；
   * - `restrictions.some(r => r.platform === effectivePlatform)` → 仅在
   *   当前任务平台受限，禁选；其它平台仍可正常使用该 IP。
   * 拉取失败时该 Map 为空 → 全部代理视作 available（避免首屏卡住用户）。
   */
  const [proxyHealth, setProxyHealth] = useState<Record<string, ProxyHealthBrief>>({});
  const [selectedProxyIds, setSelectedProxyIds] = useState<string[]>([]);
  const [submitting, setSubmitting] = useState(false);
  const [submitError, setSubmitError] = useState("");

  const [searchFor, setSearchFor] = useState("");
  const [listKind, setListKind] = useState<string>("综合");
  const [advancedKind, setAdvancedKind] = useState<string>("综合");
  const [timeStart, setTimeStart] = useState("");
  const [timeEnd, setTimeEnd] = useState("");
  const [bodyIdsText, setBodyIdsText] = useState("");
  const [commentUidText, setCommentUidText] = useState("");
  const [commentMidText, setCommentMidText] = useState("");

  /**
   * 与 `open` 同步（每轮 render 更新）。关闭后仍在飞行的 listAccounts/listProxies/
   * updateTask 若在 catch 里写错误状态，会在「关窗动画」期间再点亮红条，故异步路径需先判断此 ref。
   */
  const openRef = useRef(false);
  openRef.current = open;

  /**
   * 须在绘制前清空：若用 useEffect，首帧已 `open=false` 但错误串仍在 state 里，会闪红。
   */
  useLayoutEffect(() => {
    if (!open) {
      setSubmitError("");
      setAccountsError("");
      setProxiesError("");
      setSubmitting(false);
    }
  }, [open]);

  useEffect(() => {
    if (!open) return;
    if (editingTask) {
      setName(editingTask.name);
      setTaskType(editingTask.type);
      setStrategy(editingTask.strategy);
      setRateLimit(editingTask.rateLimit);
      setRateLimitScope(editingTask.rateLimitScope ?? "per_worker");
      setSelectedAccountIds(editingTask.boundAccountIds ?? []);
      setSelectedProxyIds(editingTask.boundProxyIds ?? []);
      setSubmitError("");
      setAccountsError("");
      setProxiesError("");
      setSearchFor("");
      setListKind("综合");
      setAdvancedKind("综合");
      setTimeStart("");
      setTimeEnd("");
      setBodyIdsText("");
      setCommentUidText("");
      setCommentMidText("");
      const w = editingTask.weiboConfig;
      if (w?.api === "list") {
        setSearchFor(w.search_for);
        setListKind(w.list_kind);
        setAdvancedKind(w.advanced_kind ?? "综合");
        setTimeStart(toDateInputValue(w.time_start));
        setTimeEnd(toDateInputValue(w.time_end));
      } else if (w?.api === "body") {
        setBodyIdsText(w.status_ids.join(" "));
      } else if (w?.api === "comment_l1" || w?.api === "comment_l2") {
        setCommentUidText(w.pairs.map((p) => p.uid).join(" "));
        setCommentMidText(w.pairs.map((p) => p.mid).join(" "));
      }
      return;
    }
    setName("");
    setTaskType("keyword");
    setStrategy("round_robin");
    setRateLimit(60);
    setRateLimitScope("per_worker");
    setSelectedAccountIds([]);
    // 新建任务默认勾选「本机直连」，让用户明确知道当前出口；用户可手动取消改挂代理。
    setSelectedProxyIds([LOCAL_DIRECT_PROXY_ID]);
    setSubmitError("");
    setAccountsError("");
    setProxiesError("");
    setSearchFor("");
    setListKind("综合");
    setAdvancedKind("综合");
    setTimeStart("");
    setTimeEnd("");
    setBodyIdsText("");
    setCommentUidText("");
    setCommentMidText("");
  }, [open, editingTask, platform]);

  /** 与任务类型一致：若后端存了未在常量列表中的值，仍要在 Select 里渲染一项。 */
  const listKindOptions = useMemo(() => {
    const k = listKind.trim();
    if (k && !(LIST_KINDS as readonly string[]).includes(k)) {
      return [...LIST_KINDS, k];
    }
    return [...LIST_KINDS];
  }, [listKind]);

  const advancedKindOptions = useMemo(() => {
    const k = advancedKind.trim();
    if (k && !(ADVANCED_KINDS as readonly string[]).includes(k)) {
      return [...ADVANCED_KINDS, k];
    }
    return [...ADVANCED_KINDS];
  }, [advancedKind]);

  const refreshAccounts = useCallback(async () => {
    if (effectivePlatform !== "weibo") {
      setAccounts([]);
      return;
    }
    setAccountsLoading(true);
    setAccountsError("");
    try {
      const list = await listAccounts("weibo");
      setAccounts(list);
    } catch (e: unknown) {
      if (openRef.current) {
        setAccountsError(e instanceof Error ? e.message : String(e));
      }
    } finally {
      setAccountsLoading(false);
    }
  }, [effectivePlatform]);

  useEffect(() => {
    if (!open) return;
    void refreshAccounts();
  }, [open, refreshAccounts]);

  const toggleAccount = useCallback((id: string) => {
    setSelectedAccountIds((prev) =>
      prev.includes(id) ? prev.filter((x) => x !== id) : [...prev, id],
    );
  }, []);

  const refreshProxies = useCallback(async () => {
    setProxiesLoading(true);
    setProxiesError("");
    try {
      // 列表 + 派生健康档位并发拉取；列表错了就显 alert，健康聚合错了 fallback 全 available
      // （继续可选可勾，至少不卡用户走任务流程）。
      const [list, healthRes] = await Promise.all([
        listProxies(),
        listProxiesHealth().catch((err) => {
          console.warn("[CreateTaskModal] listProxiesHealth failed:", err);
          return [] as Awaited<ReturnType<typeof listProxiesHealth>>;
        }),
      ]);
      setProxies(list);
      const map: Record<string, ProxyHealthBrief> = {};
      for (const h of healthRes) map[h.id] = h;
      setProxyHealth(map);
    } catch (e: unknown) {
      if (openRef.current) {
        setProxiesError(e instanceof Error ? e.message : String(e));
      }
    } finally {
      setProxiesLoading(false);
    }
  }, []);

  useEffect(() => {
    if (!open) return;
    /**
     * 切勿在关窗时 `setProxies([])`：`selectedProxyIds` 仍在，会把全部已选代理算成 orphan，
     * 关窗动画期间整片「已删除」红条会爆闪。
     */
    void refreshProxies();
  }, [open, refreshProxies]);

  const toggleProxy = useCallback((id: string) => {
    setSelectedProxyIds((prev) =>
      prev.includes(id) ? prev.filter((x) => x !== id) : [...prev, id],
    );
  }, []);

  // 已选中但代理列表里找不到的 id（可能源代理已被删除）。
  // 不在 UI 中渲染会导致用户无法取消勾选，需单独列出可移除项。
  const orphanProxyIds = useMemo(() => {
    const known = new Set(proxies.map((p) => p.id));
    return selectedProxyIds.filter((id) => !known.has(id));
  }, [proxies, selectedProxyIds]);

  const orphanAccountIds = useMemo(() => {
    const known = new Set(accounts.map((a) => a.id));
    return selectedAccountIds.filter((id) => !known.has(id));
  }, [accounts, selectedAccountIds]);

  /**
   * 「在当前任务平台不可用」的代理 = 全局失效 ∪ 当前平台 scope 受限。
   * v4 / 方案 C：受限按平台 scope 算，所以同一个 IP 在 weibo 任务里可能受限、
   * 在 douyin 任务里可用。
   */
  function isProxyUnusableForThisTask(id: string): boolean {
    const h = proxyHealth[id];
    if (!h) return false;
    if (h.globalStatus === "invalid") return true;
    return h.restrictions.some((r) => r.platform === effectivePlatform);
  }

  // 已选中但全局失效或在当前平台受限的代理数量（不含 orphan，便于一键清理提示）。
  const invalidSelectedProxyCount = useMemo(
    () =>
      proxies.filter(
        (p) => selectedProxyIds.includes(p.id) && isProxyUnusableForThisTask(p.id),
      ).length,
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [proxies, selectedProxyIds, proxyHealth, effectivePlatform],
  );

  const errorSelectedAccountCount = useMemo(
    () =>
      accounts.filter(
        (a) => selectedAccountIds.includes(a.id) && a.riskStatus === "error",
      ).length,
    [accounts, selectedAccountIds],
  );

  /** 微博任务：至少 1 个仍在列表中的账号 + 至少 1 个出口（本机直连或仍在列表中的代理）。 */
  const weiboBindValid = useMemo(() => {
    if (effectivePlatform !== "weibo") return true;
    const knownAccounts = new Set(accounts.map((a) => a.id));
    const hasAccount = selectedAccountIds.some((id) => knownAccounts.has(id));
    const knownProxies = new Set(proxies.map((p) => p.id));
    const hasProxy = selectedProxyIds.some(
      (id) => id === LOCAL_DIRECT_PROXY_ID || knownProxies.has(id),
    );
    return hasAccount && hasProxy;
  }, [
    effectivePlatform,
    accounts,
    proxies,
    selectedAccountIds,
    selectedProxyIds,
  ]);

  const clearInvalidProxies = useCallback(() => {
    const known = new Set(proxies.map((p) => p.id));
    const blockedIds = new Set(
      proxies
        .filter((p) => {
          const h = proxyHealth[p.id];
          if (!h) return false;
          if (h.globalStatus === "invalid") return true;
          return h.restrictions.some((r) => r.platform === effectivePlatform);
        })
        .map((p) => p.id),
    );
    setSelectedProxyIds((prev) =>
      prev.filter((id) => !blockedIds.has(id) && known.has(id)),
    );
  }, [proxies, proxyHealth, effectivePlatform]);

  const clearInvalidAccounts = useCallback(() => {
    const known = new Set(accounts.map((a) => a.id));
    const errIds = new Set(
      accounts.filter((a) => a.riskStatus === "error").map((a) => a.id),
    );
    setSelectedAccountIds((prev) =>
      prev.filter((id) => !errIds.has(id) && known.has(id)),
    );
  }, [accounts]);

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    const trimmed = name.trim();
    if (!trimmed) {
      setSubmitError("请填写任务名称");
      return;
    }

    let weibo_config: WeiboTaskPayload | null | undefined;
    if (effectivePlatform === "weibo") {
      try {
        if (taskType === "keyword") {
          if (!searchFor.trim()) {
            setSubmitError("列表搜索请填写搜索内容");
            return;
          }
        }
        if (taskType === "user_profile") {
          const ids = bodyIdsText.trim().split(/\s+/).filter(Boolean);
          if (ids.length === 0) {
            setSubmitError("详细页请填写至少一条微博 id（空格分隔）");
            return;
          }
        }
        if (taskType === "comment_level1" || taskType === "comment_level2") {
          const uids = commentUidText.trim().split(/\s+/).filter(Boolean);
          const mids = commentMidText.trim().split(/\s+/).filter(Boolean);
          if (uids.length === 0 || mids.length === 0) {
            setSubmitError("请填写 uid 与 mid 列表");
            return;
          }
          if (uids.length !== mids.length) {
            setSubmitError("uid 与 mid 条数须一致");
            return;
          }
        }

        weibo_config = buildWeiboPayload(taskType, {
          searchFor,
          listKind,
          advancedKind,
          timeStart,
          timeEnd,
          bodyIdsText,
          commentUidText,
          commentMidText,
        });
      } catch (err: unknown) {
        if (openRef.current) {
          setSubmitError(err instanceof Error ? err.message : String(err));
        }
        return;
      }
    } else {
      weibo_config = undefined;
    }

    if (effectivePlatform === "weibo" && !weiboBindValid) {
      setSubmitError(
        "请至少绑定 1 个账号，并选择至少 1 个出口（本机直连或代理）。",
      );
      return;
    }

    setSubmitting(true);
    setSubmitError("");
    try {
      const sharedAccountIds =
        effectivePlatform === "weibo" && selectedAccountIds.length > 0
          ? selectedAccountIds
          : null;
      const sharedProxyIds =
        selectedProxyIds.length > 0 ? selectedProxyIds : null;
      if (editingTask) {
        await updateTask({
          id: editingTask.id,
          name: trimmed,
          strategy,
          rate_limit: rateLimit,
          account_ids: sharedAccountIds,
          proxy_ids: sharedProxyIds,
          rate_limit_scope: rateLimitScope,
          weibo_config: weibo_config ?? null,
        });
        toast.success(`任务「${trimmed}」已更新`);
      } else {
        await createTask({
          platform,
          task_type: taskType,
          name: trimmed,
          strategy,
          rate_limit: rateLimit,
          account_ids: sharedAccountIds,
          proxy_ids: sharedProxyIds,
          rate_limit_scope: rateLimitScope,
          weibo_config: weibo_config ?? null,
        });
        toast.success(`任务「${trimmed}」已创建`);
      }
      onOpenChange(false);
      await onCreated?.();
    } catch (err: unknown) {
      if (openRef.current) {
        setSubmitError(err instanceof Error ? err.message : String(err));
      }
    } finally {
      setSubmitting(false);
    }
  }

  const textareaClass = cn(
    "flex min-h-[80px] w-full rounded-lg border border-input bg-transparent px-2.5 py-1.5 text-sm shadow-xs transition-colors outline-none",
    "placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50",
    "disabled:cursor-not-allowed disabled:opacity-50"
  );

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[min(92vh,720px)] gap-0 overflow-hidden p-0 sm:max-w-3xl">
        <form
          onSubmit={handleSubmit}
          className="flex max-h-[min(92vh,720px)] flex-col"
        >
          <DialogHeader className="shrink-0 px-4 pt-4 pb-2">
            <DialogTitle>
              {editingTask ? "编辑采集任务" : "新建采集任务"}
            </DialogTitle>
            <DialogDescription className="sr-only">
              {editingTask ? "编辑采集任务" : "新建采集任务"}
            </DialogDescription>
          </DialogHeader>

          <div className="min-h-0 flex-1 space-y-3 overflow-y-auto px-4 py-2">
            <TooltipProvider delayDuration={200}>
            {/* Row 1: 任务名称 + 任务类型 + IP 派发策略 + 速率 + 限流粒度 */}
            <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 xl:grid-cols-5">
              <div className="space-y-1.5">
                <Label htmlFor="task-name">任务名称</Label>
                <Input
                  id="task-name"
                  value={name}
                  onChange={(ev) => setName(ev.target.value)}
                  placeholder="例如：热点监控"
                  autoComplete="off"
                />
              </div>
              <div className="space-y-1.5">
                <Label>任务类型</Label>
                <Select
                  value={taskType}
                  onValueChange={(v) => setTaskType(v as TaskType)}
                  disabled={!!editingTask}
                >
                  <SelectTrigger className="w-full">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {taskTypesForPlatform.map((t) => (
                      <SelectItem key={t} value={t}>
                        {TASK_TYPE_LABELS[t]}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
              <div className="space-y-1.5">
                <Label className="flex items-center gap-1">
                  IP 派发
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <InfoIcon className="size-3 text-muted-foreground" />
                    </TooltipTrigger>
                    <TooltipContent
                      side="top"
                      className="max-w-[16rem] text-xs"
                    >
                      并发模式下账号始终全部启用，此处仅决定将 N
                      个绑定代理派发到 worker 的顺序：
                      <br />· 轮询：按账号 × 代理顺序展开。
                      <br />· 随机：展开后随机洗牌。
                    </TooltipContent>
                  </Tooltip>
                </Label>
                <Select
                  value={strategy}
                  onValueChange={(v) => setStrategy(v as CrawlStrategy)}
                >
                  <SelectTrigger className="w-full">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {STRATEGIES.map((s) => (
                      <SelectItem key={s} value={s}>
                        {STRATEGY_LABELS[s]}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
              <div className="space-y-1.5">
                <Label htmlFor="rate-limit">速率（次/分）</Label>
                <Input
                  id="rate-limit"
                  type="number"
                  min={1}
                  value={rateLimit}
                  onChange={(ev) =>
                    setRateLimit(Number.parseInt(ev.target.value, 10) || 0)
                  }
                />
              </div>
              <div className="space-y-1.5">
                <Label className="flex items-center gap-1">
                  限流粒度
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <InfoIcon className="size-3 text-muted-foreground" />
                    </TooltipTrigger>
                    <TooltipContent
                      side="top"
                      className="max-w-[18rem] text-xs"
                    >
                      · 每 worker：每个 (账号, 代理) worker 独立 60_000 /
                      速率 ms 间隔；总吞吐 ≈ N×M×速率/min。
                      <br />· 按账号共享：同账号下的多个 worker
                      共享一个令牌桶，每个账号实际 QPS = 速率/min；用于规避账号风控。
                    </TooltipContent>
                  </Tooltip>
                </Label>
                <Select
                  value={rateLimitScope}
                  onValueChange={(v) =>
                    setRateLimitScope(v as RateLimitScope)
                  }
                >
                  <SelectTrigger className="w-full">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {RATE_LIMIT_SCOPES.map((s) => (
                      <SelectItem key={s} value={s}>
                        {RATE_LIMIT_SCOPE_LABELS[s]}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
            </div>

            {/* Row 2 (keyword): 搜索内容 + 搜索类型 + 筛选条件 + 起始日期 + 结束日期 */}
            {effectivePlatform === "weibo" && taskType === "keyword" && (
              <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 xl:grid-cols-5">
                <div className="space-y-1.5">
                  <Label htmlFor="search-for">搜索内容</Label>
                  <Input
                    id="search-for"
                    value={searchFor}
                    onChange={(ev) => setSearchFor(ev.target.value)}
                    placeholder="#话题#"
                    autoComplete="off"
                  />
                </div>
                <div className="space-y-1.5">
                  <Label>搜索类型</Label>
                  <Select value={listKind} onValueChange={setListKind}>
                    <SelectTrigger>
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      {listKindOptions.map((k) => (
                        <SelectItem key={k} value={k}>
                          {k}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </div>
                <div className="space-y-1.5">
                  <Label>筛选条件</Label>
                  <Select
                    value={advancedKind}
                    onValueChange={setAdvancedKind}
                    disabled={listKind !== "高级"}
                  >
                    <SelectTrigger>
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      {advancedKindOptions.map((k) => (
                        <SelectItem key={k} value={k}>
                          {k}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </div>
                <div className="space-y-1.5">
                  <Label htmlFor="time-start">起始日期</Label>
                  <Input
                    id="time-start"
                    type="date"
                    disabled={listKind !== "高级"}
                    value={timeStart}
                    onChange={(ev) => setTimeStart(ev.target.value)}
                  />
                </div>
                <div className="space-y-1.5">
                  <Label htmlFor="time-end">结束日期</Label>
                  <Input
                    id="time-end"
                    type="date"
                    disabled={listKind !== "高级"}
                    value={timeEnd}
                    onChange={(ev) => setTimeEnd(ev.target.value)}
                  />
                </div>
              </div>
            )}

            {/* Body ids (user_profile) */}
            {effectivePlatform === "weibo" && taskType === "user_profile" && (
              <div className="space-y-2">
                <Label htmlFor="body-ids">微博 id 列表（空格分隔）</Label>
                <textarea
                  id="body-ids"
                  className={textareaClass}
                  value={bodyIdsText}
                  onChange={(ev) => setBodyIdsText(ev.target.value)}
                  placeholder="例如：OiZre8dir Oj0PXme8I"
                  rows={3}
                />
              </div>
            )}

            {/* Comment uid/mid (comment_level1/2) */}
            {effectivePlatform === "weibo" &&
              (taskType === "comment_level1" ||
                taskType === "comment_level2") && (
                <div className="space-y-2 rounded-lg border bg-muted/20 p-3">
                  <Alert className="py-2">
                    <InfoIcon />
                    <AlertDescription className="text-xs font-medium">
                      {taskType === "comment_level1"
                        ? "一级评论（buildComments is_mix=0）"
                        : "二级评论（buildComments is_mix=1）"}
                    </AlertDescription>
                  </Alert>
                  <div className="grid grid-cols-2 gap-3">
                    <div className="space-y-1.5">
                      <Label htmlFor="c-uids">uid 列表（空格分隔）</Label>
                      <textarea
                        id="c-uids"
                        className={textareaClass}
                        value={commentUidText}
                        onChange={(ev) => setCommentUidText(ev.target.value)}
                        placeholder="2035895904 1749277070"
                        rows={3}
                      />
                    </div>
                    <div className="space-y-1.5">
                      <Label htmlFor="c-mids">mid 列表（空格分隔）</Label>
                      <textarea
                        id="c-mids"
                        className={textareaClass}
                        value={commentMidText}
                        onChange={(ev) => setCommentMidText(ev.target.value)}
                        placeholder="5096904217856018 5045463240409185"
                        rows={3}
                      />
                    </div>
                  </div>
                </div>
              )}

            {/* Row 3: 账号池 + IP 池（memo 子组件，减轻输入框每次按键的重渲染） */}
            {effectivePlatform === "weibo" && (
              <WeiboBindPoolsSection
                open={open}
                accounts={accounts}
                accountsLoading={accountsLoading}
                accountsError={accountsError}
                selectedAccountIds={selectedAccountIds}
                proxies={proxies}
                proxiesLoading={proxiesLoading}
                proxiesError={proxiesError}
                selectedProxyIds={selectedProxyIds}
                proxyHealth={proxyHealth}
                effectivePlatform={effectivePlatform}
                orphanAccountIds={orphanAccountIds}
                orphanProxyIds={orphanProxyIds}
                errorSelectedAccountCount={errorSelectedAccountCount}
                invalidSelectedProxyCount={invalidSelectedProxyCount}
                onToggleAccount={toggleAccount}
                onToggleProxy={toggleProxy}
                onRefreshAccounts={refreshAccounts}
                onRefreshProxies={refreshProxies}
                onClearInvalidAccounts={clearInvalidAccounts}
                onClearInvalidProxies={clearInvalidProxies}
              />
            )}

            {effectivePlatform === "weibo" && (
              <WeiboBindAlertsSection
                open={open}
                weiboBindValid={weiboBindValid}
                selectedAccountIds={selectedAccountIds}
                selectedProxyIds={selectedProxyIds}
                invalidSelectedProxyCount={invalidSelectedProxyCount}
                orphanProxyIds={orphanProxyIds}
                errorSelectedAccountCount={errorSelectedAccountCount}
                orphanAccountIds={orphanAccountIds}
              />
            )}

            {open && submitError && (
              <Alert variant="destructive">
                <AlertCircleIcon />
                <AlertTitle>无法保存任务</AlertTitle>
                <AlertDescription>{submitError}</AlertDescription>
              </Alert>
            )}
            </TooltipProvider>
          </div>

          <DialogFooter className="shrink-0 !m-0 flex-row justify-end gap-2 border-t bg-muted/30 px-4 py-3">
            <Button
              type="button"
              variant="outline"
              onClick={() => onOpenChange(false)}
              disabled={submitting}
            >
              取消
            </Button>
            <Button
              type="submit"
              disabled={
                submitting || (effectivePlatform === "weibo" && !weiboBindValid)
              }
            >
              {submitting
                ? editingTask
                  ? "保存中…"
                  : "创建中…"
                : editingTask
                  ? "保存"
                  : "创建"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
