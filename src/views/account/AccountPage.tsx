import { useCallback, useEffect, useState } from "react";
import {
  AlertCircleIcon,
  ListChecksIcon,
  Loader2Icon,
  MoreHorizontalIcon,
  RefreshCwIcon,
  Trash2Icon,
} from "lucide-react";
import { toast } from "sonner";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { FloatingScrollArea } from "@/app/ui/FloatingScrollArea";
import {
  PLATFORMS,
  PLATFORM_LABELS,
  type Account,
  type Platform,
} from "@/features/domain/types";
import { deleteAccount, listAccounts } from "@/services/tauri/commands";
import { formatProxyEndpoint } from "@/views/ip/utils/format";
import { AddAccountDialog } from "@/views/account/AddAccountDialog";
import { AccountLogModal } from "@/views/account/AccountLogModal";

const RISK_STATUS_MAP: Record<
  string,
  { label: string; variant: "default" | "secondary" | "destructive"; hint: string }
> = {
  normal: { label: "正常", variant: "default", hint: "近 5 分钟无异常失败。" },
  restricted: {
    label: "受限",
    variant: "secondary",
    hint:
      "近 5 分钟出现 ≥5 次失败。可继续使用，但建议关注；若连续 10 次成功则自动回落到正常。",
  },
  error: {
    label: "异常",
    variant: "destructive",
    hint:
      "近 5 分钟出现 ≥3 次跳登录页 / Cookie 失效。任务调度会暂停该账号，需要重新扫码登录后才会恢复。",
  },
};

function AccountTableHeader() {
  return (
    <TableHeader>
      <TableRow>
        <TableHead>用户名</TableHead>
        <TableHead>微博 UID</TableHead>
        <TableHead>绑定 IP</TableHead>
        <TableHead>状态</TableHead>
        <TableHead>添加时间</TableHead>
        <TableHead>最后活跃时间</TableHead>
        <TableHead className="w-[72px] text-center">操作</TableHead>
      </TableRow>
    </TableHeader>
  );
}

function PlatformAccountTable({
  platform,
  accounts,
  loading,
  onViewLog,
  onDelete,
}: {
  platform: Platform;
  accounts: Account[];
  loading: boolean;
  onViewLog: (account: Account) => void;
  onDelete: (account: Account) => void;
}) {
  if (loading && accounts.length === 0) {
    return (
      <Table>
        <AccountTableHeader />
        <TableBody>
          <TableRow>
            <TableCell colSpan={7} className="py-12">
              <div className="flex items-center justify-center">
                <Loader2Icon className="size-6 animate-spin text-muted-foreground" />
              </div>
            </TableCell>
          </TableRow>
        </TableBody>
      </Table>
    );
  }

  if (accounts.length === 0) {
    return (
      <div className="flex h-32 items-center justify-center text-sm text-muted-foreground">
        暂无 {PLATFORM_LABELS[platform]} 账号
      </div>
    );
  }

  return (
    <Table>
      <AccountTableHeader />
      <TableBody>
        {accounts.map((account) => {
          const risk = RISK_STATUS_MAP[account.riskStatus];
          return (
            <TableRow key={account.id}>
              <TableCell className="font-medium">
                {account.username}
              </TableCell>
              <TableCell className="font-mono text-xs tabular-nums">
                {account.platform === "weibo" && account.weiboProfile
                  ? account.weiboProfile.uid
                  : "—"}
              </TableCell>
              <TableCell className="font-mono text-xs">
                {account.boundIp
                  ? formatProxyEndpoint(account.boundIp)
                  : "—"}
              </TableCell>
              <TableCell>
                <Badge variant={risk.variant} title={risk.hint}>
                  {risk.label}
                </Badge>
              </TableCell>
              <TableCell className="text-xs text-muted-foreground">
                {account.createdAt}
              </TableCell>
              <TableCell className="text-xs text-muted-foreground">
                {account.lastActiveAt}
              </TableCell>
              <TableCell className="text-center">
                <DropdownMenu>
                  <DropdownMenuTrigger asChild>
                    <Button
                      variant="ghost"
                      size="icon"
                      className="size-7"
                      aria-label="更多操作"
                    >
                      <MoreHorizontalIcon className="size-4" />
                    </Button>
                  </DropdownMenuTrigger>
                  <DropdownMenuContent align="end" className="w-36">
                    <DropdownMenuItem
                      onSelect={(ev) => {
                        ev.preventDefault();
                        onViewLog(account);
                      }}
                    >
                      <ListChecksIcon className="size-4" />
                      查看日志
                    </DropdownMenuItem>
                    <DropdownMenuSeparator />
                    <DropdownMenuItem
                      variant="destructive"
                      onSelect={(ev) => {
                        ev.preventDefault();
                        onDelete(account);
                      }}
                    >
                      <Trash2Icon className="size-4" />
                      删除
                    </DropdownMenuItem>
                  </DropdownMenuContent>
                </DropdownMenu>
              </TableCell>
            </TableRow>
          );
        })}
      </TableBody>
    </Table>
  );
}

export function AccountPage() {
  const [activePlatform, setActivePlatform] = useState<Platform>("weibo");
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  /** 日志 modal 当前查看的账号；为 `null` 时关闭 modal。 */
  const [viewingLog, setViewingLog] = useState<Account | null>(null);
  /** 删除二次确认目标行；为 `null` 时不显示 AlertDialog。 */
  const [deleting, setDeleting] = useState<Account | null>(null);
  /** 删除提交中标记，防止 dialog 内重复点击 / 关闭过早。 */
  const [deletingBusy, setDeletingBusy] = useState(false);

  const loadAccounts = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const rows = await listAccounts();
      setAccounts(rows);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setAccounts([]);
    } finally {
      setLoading(false);
    }
  }, []);

  /**
   * 提交删除：调用后端 `delete_account`，成功后从本地列表里把该行剔除并 toast 一下。
   * 失败时保留 dialog（让用户看到 toast 并可选择再次尝试）。
   */
  const handleConfirmDelete = useCallback(async () => {
    if (!deleting) return;
    setDeletingBusy(true);
    try {
      await deleteAccount(deleting.id);
      setAccounts((prev) => prev.filter((a) => a.id !== deleting.id));
      toast.success("账号已删除", { description: deleting.username });
      setDeleting(null);
    } catch (e) {
      toast.error("删除账号失败", {
        description: e instanceof Error ? e.message : String(e),
      });
    } finally {
      setDeletingBusy(false);
    }
  }, [deleting]);

  useEffect(() => {
    void loadAccounts();
  }, [loadAccounts]);

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <FloatingScrollArea>
        <div className="space-y-4 p-6">
          <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
            <div className="min-w-0">
              <h1 className="text-2xl font-bold tracking-tight">账号管理</h1>
              <p className="text-sm text-muted-foreground">
                按平台分类管理采集账号资源（数据来自本地数据库）
              </p>
            </div>
            <div className="flex shrink-0 flex-wrap items-center justify-end gap-2">
              <Button
                variant="outline"
                disabled={loading}
                onClick={() => void loadAccounts()}
              >
                <RefreshCwIcon className="size-4" />
                刷新
              </Button>
              <AddAccountDialog onAccountsChanged={loadAccounts} />
            </div>
          </div>

          {error ? (
            <Alert variant="destructive">
              <AlertCircleIcon />
              <AlertTitle>账号列表加载失败</AlertTitle>
              <AlertDescription>{error}</AlertDescription>
            </Alert>
          ) : null}

          <Tabs
            value={activePlatform}
            onValueChange={(v) => setActivePlatform(v as Platform)}
          >
            <TabsList>
              {PLATFORMS.map((p) => {
                const count = accounts.filter((a) => a.platform === p).length;
                return (
                  <TabsTrigger key={p} value={p}>
                    {PLATFORM_LABELS[p]}
                    <Badge variant="secondary" className="ml-1.5 text-[10px]">
                      {count}
                    </Badge>
                  </TabsTrigger>
                );
              })}
            </TabsList>

            {PLATFORMS.map((p) => (
              <TabsContent key={p} value={p}>
                <Card>
                  <CardHeader>
                    <CardTitle className="text-base">
                      {PLATFORM_LABELS[p]} 账号列表
                    </CardTitle>
                  </CardHeader>
                  <CardContent>
                    <PlatformAccountTable
                      platform={p}
                      accounts={accounts.filter((a) => a.platform === p)}
                      loading={loading}
                      onViewLog={setViewingLog}
                      onDelete={setDeleting}
                    />
                  </CardContent>
                </Card>
              </TabsContent>
            ))}
          </Tabs>
        </div>
      </FloatingScrollArea>

      <AccountLogModal
        open={viewingLog != null}
        onOpenChange={(o) => {
          if (!o) setViewingLog(null);
        }}
        account={viewingLog}
      />

      <AlertDialog
        open={deleting != null}
        onOpenChange={(o) => {
          if (!o && !deletingBusy) setDeleting(null);
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>删除该账号？</AlertDialogTitle>
            <AlertDialogDescription>
              将永久删除账号{" "}
              <span className="font-mono">{deleting?.username}</span>
              。运行中持有该账号的扫码会话会被一同清理；正在使用该账号的任务会被中断。
              本操作不可撤销。
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={deletingBusy}>取消</AlertDialogCancel>
            <AlertDialogAction
              disabled={deletingBusy}
              variant="destructive"
              onClick={(ev) => {
                ev.preventDefault();
                void handleConfirmDelete();
              }}
            >
              {deletingBusy ? "删除中…" : "确认删除"}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}
