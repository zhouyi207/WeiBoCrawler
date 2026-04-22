import {
  BugIcon,
  DatabaseIcon,
  GlobeIcon,
  HomeIcon,
  NetworkIcon,
  UserIcon,
} from "lucide-react";
import { NavLink, useLocation } from "react-router-dom";
import type { PageId } from "@/features/domain/types";
import { PAGE_PATH } from "@/app/route-meta";
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarRail,
  SidebarSeparator,
  useSidebar,
} from "@/components/ui/sidebar";
import { Button } from "@/components/ui/button";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { DownloadIcon } from "lucide-react";
import { checkAndInstallUpdate } from "@/app/updater";
import { cn } from "@/lib/utils";

const MAIN_NAV_ITEMS: { id: PageId; label: string; icon: React.ElementType }[] = [
  { id: "home", label: "主页", icon: HomeIcon },
  { id: "crawl", label: "采集", icon: BugIcon },
  { id: "account", label: "账号", icon: UserIcon },
  { id: "ip", label: "IP 代理", icon: GlobeIcon },
  { id: "database", label: "数据库", icon: DatabaseIcon },
];

const REQUEST_LOGS_NAV: { id: PageId; label: string; icon: React.ElementType } = {
  id: "requestLogs",
  label: "请求日志",
  icon: NetworkIcon,
};

function navIsActive(pathname: string, id: PageId): boolean {
  const to = PAGE_PATH[id];
  if (id === "home") {
    return pathname === "/" || pathname === "";
  }
  return pathname === to || pathname.startsWith(`${to}/`);
}

/** 与 `SidebarMenuButton` 内建的 `group-data-[collapsible=icon]:size-8!` 对齐，避免自定义高度把标签文字挤出可视区域。 */
const navButtonClass = (isActive: boolean) =>
  cn(
    "h-9 gap-3 rounded-lg transition-colors duration-150",
    "group-data-[collapsible=icon]:size-8! group-data-[collapsible=icon]:gap-0 group-data-[collapsible=icon]:p-2!",
    "group-data-[collapsible=icon]:[&>span:last-child]:hidden",
    isActive &&
      cn(
        "bg-primary/12 font-medium text-primary shadow-sm ring-1 ring-primary/20 hover:bg-primary/16 hover:text-primary dark:bg-primary/18 dark:ring-primary/25",
        "group-data-[collapsible=icon]:ring-0 group-data-[collapsible=icon]:shadow-none",
      ),
  );

export function AppSidebar() {
  const { pathname } = useLocation();
  const { state, isMobile } = useSidebar();
  const iconCollapsed = state === "collapsed" && !isMobile;
  const RequestLogsIcon = REQUEST_LOGS_NAV.icon;
  const requestLogsActive = navIsActive(pathname, REQUEST_LOGS_NAV.id);

  return (
    <Sidebar collapsible="icon" variant="inset">
      <SidebarHeader
        className={cn(
          "border-sidebar-border/60 border-b px-2 pb-3 pt-3",
          iconCollapsed && "flex justify-center px-2 py-2 pb-2 pt-2",
        )}
      >
        {iconCollapsed ? (
          <div
            className="flex size-8 shrink-0 items-center justify-center rounded-lg bg-primary text-primary-foreground shadow-md"
            aria-hidden
          >
            <BugIcon className="size-4" strokeWidth={2.25} />
          </div>
        ) : (
          <div
            className={cn(
              "flex items-center gap-2.5 overflow-hidden rounded-xl px-2 py-2",
              "bg-gradient-to-br from-sidebar-accent/90 to-sidebar-accent/50",
              "ring-1 ring-sidebar-border/70 shadow-sm",
            )}
          >
            <div
              className={cn(
                "flex size-9 shrink-0 items-center justify-center rounded-lg",
                "bg-primary text-primary-foreground shadow-md",
              )}
            >
              <BugIcon className="size-[1.125rem]" strokeWidth={2.25} />
            </div>
            <div className="min-w-0 flex-1 overflow-hidden">
              <p className="truncate text-sm font-semibold leading-tight tracking-tight">
                YssCrawler
              </p>
              <p className="text-sidebar-foreground/65 mt-0.5 truncate text-[11px] leading-tight">
                采集工作台
              </p>
            </div>
          </div>
        )}
      </SidebarHeader>

      <SidebarContent className="px-0">
        <SidebarGroup className="min-h-0 flex-1 px-2 pt-3 pb-2">
          <SidebarGroupLabel className="text-sidebar-foreground/55 mb-1 px-2 text-[11px] font-semibold uppercase tracking-wider">
            导航
          </SidebarGroupLabel>
          <SidebarGroupContent>
            <SidebarMenu className="gap-1">
              {MAIN_NAV_ITEMS.map((item) => {
                const to = PAGE_PATH[item.id];
                const isActive = navIsActive(pathname, item.id);
                const Icon = item.icon;
                return (
                  <SidebarMenuItem key={item.id}>
                    <SidebarMenuButton
                      asChild
                      isActive={isActive}
                      tooltip={item.label}
                      className={navButtonClass(isActive)}
                    >
                      <NavLink to={to} end={item.id === "home"}>
                        <Icon className="opacity-90" strokeWidth={2} />
                        <span>{item.label}</span>
                      </NavLink>
                    </SidebarMenuButton>
                  </SidebarMenuItem>
                );
              })}
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>

        <SidebarGroup className="mt-auto px-2 pb-2">
          <SidebarSeparator className="mb-3 bg-sidebar-border/80" />
          <SidebarGroupLabel className="text-sidebar-foreground/55 mb-1 px-2 text-[11px] font-semibold uppercase tracking-wider">
            诊断
          </SidebarGroupLabel>
          <SidebarGroupContent>
            <SidebarMenu className="gap-1">
              <SidebarMenuItem>
                <SidebarMenuButton
                  asChild
                  isActive={requestLogsActive}
                  tooltip={REQUEST_LOGS_NAV.label}
                  className={navButtonClass(requestLogsActive)}
                >
                  <NavLink to={PAGE_PATH[REQUEST_LOGS_NAV.id]}>
                    <RequestLogsIcon className="opacity-90" strokeWidth={2} />
                    <span>{REQUEST_LOGS_NAV.label}</span>
                  </NavLink>
                </SidebarMenuButton>
              </SidebarMenuItem>
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>
      </SidebarContent>

      <SidebarFooter className="border-sidebar-border/60 border-t p-2">
        {iconCollapsed ? (
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                type="button"
                variant="secondary"
                size="sm"
                className={cn(
                  "h-9 w-full justify-center rounded-lg border border-sidebar-border/70 px-0",
                  "bg-sidebar-accent/40 text-sidebar-foreground shadow-none",
                  "hover:bg-sidebar-accent/70 hover:text-sidebar-accent-foreground",
                )}
                onClick={() => void checkAndInstallUpdate()}
              >
                <DownloadIcon className="size-4 opacity-90" />
                <span className="sr-only">检查更新</span>
              </Button>
            </TooltipTrigger>
            <TooltipContent side="right" align="center">
              检查更新
            </TooltipContent>
          </Tooltip>
        ) : (
          <Button
            type="button"
            variant="secondary"
            size="sm"
            className={cn(
              "h-9 w-full gap-2 rounded-lg border border-sidebar-border/70",
              "bg-sidebar-accent/40 text-sidebar-foreground shadow-none",
              "hover:bg-sidebar-accent/70 hover:text-sidebar-accent-foreground",
            )}
            onClick={() => void checkAndInstallUpdate()}
          >
            <DownloadIcon className="size-4 shrink-0 opacity-90" />
            检查更新
          </Button>
        )}
      </SidebarFooter>

      <SidebarRail />
    </Sidebar>
  );
}
