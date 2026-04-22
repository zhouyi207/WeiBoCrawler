import {
  ListChecksIcon,
  MoreHorizontalIcon,
  PencilIcon,
  Trash2Icon,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import type { ProxyIp } from "@/features/domain/types";

/**
 * IP 表格行的「⋯」操作菜单。系统内置行（host 直连等）禁用编辑 / 删除。
 * 全局 tab 与 per-platform tab 共用此菜单，因此入参只需 `ProxyIp` 基础元数据。
 */
export interface RowActionsMenuProps {
  proxy: ProxyIp;
  onViewLog: () => void;
  onEdit: () => void;
  onDelete: () => void;
}

export function RowActionsMenu({
  proxy,
  onViewLog,
  onEdit,
  onDelete,
}: RowActionsMenuProps) {
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button variant="ghost" size="icon" className="size-7" aria-label="更多操作">
          <MoreHorizontalIcon className="size-4" />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-36">
        <DropdownMenuItem
          onSelect={(ev) => {
            ev.preventDefault();
            onViewLog();
          }}
        >
          <ListChecksIcon className="size-4" />
          查看日志
        </DropdownMenuItem>
        <DropdownMenuSeparator />
        <DropdownMenuItem
          disabled={proxy.isSystem}
          onSelect={(ev) => {
            ev.preventDefault();
            if (!proxy.isSystem) onEdit();
          }}
          title={proxy.isSystem ? "系统内置行不允许编辑" : undefined}
        >
          <PencilIcon className="size-4" />
          编辑
        </DropdownMenuItem>
        <DropdownMenuSeparator />
        <DropdownMenuItem
          disabled={proxy.isSystem}
          variant="destructive"
          onSelect={(ev) => {
            ev.preventDefault();
            if (!proxy.isSystem) onDelete();
          }}
          title={proxy.isSystem ? "系统内置行不允许删除" : undefined}
        >
          <Trash2Icon className="size-4" />
          删除
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
