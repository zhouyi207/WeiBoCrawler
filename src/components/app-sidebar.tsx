import {
  BugIcon,
  DatabaseIcon,
  GlobeIcon,
  HomeIcon,
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
} from "@/components/ui/sidebar";
import { Button } from "@/components/ui/button";
import { DownloadIcon } from "lucide-react";
import { checkAndInstallUpdate } from "@/app/updater";

const NAV_ITEMS: { id: PageId; label: string; icon: React.ElementType }[] = [
  { id: "home", label: "主页", icon: HomeIcon },
  { id: "crawl", label: "采集", icon: BugIcon },
  { id: "account", label: "账号", icon: UserIcon },
  { id: "ip", label: "IP 代理", icon: GlobeIcon },
  { id: "database", label: "数据库", icon: DatabaseIcon },
];

function navIsActive(pathname: string, id: PageId): boolean {
  const to = PAGE_PATH[id];
  if (id === "home") {
    return pathname === "/" || pathname === "";
  }
  return pathname === to || pathname.startsWith(`${to}/`);
}

export function AppSidebar() {
  const { pathname } = useLocation();

  return (
    <Sidebar collapsible="icon">
      <SidebarHeader className="px-3 py-4">
        <div className="flex items-center gap-2 overflow-hidden">
          <BugIcon className="size-5 shrink-0 text-primary" />
          <span className="truncate text-base font-semibold tracking-tight">
            YssCrawler
          </span>
        </div>
      </SidebarHeader>

      <SidebarContent>
        <SidebarGroup>
          <SidebarGroupLabel>导航</SidebarGroupLabel>
          <SidebarGroupContent>
            <SidebarMenu>
              {NAV_ITEMS.map((item) => {
                const to = PAGE_PATH[item.id];
                const isActive = navIsActive(pathname, item.id);
                return (
                  <SidebarMenuItem key={item.id}>
                    <SidebarMenuButton
                      asChild
                      isActive={isActive}
                      tooltip={item.label}
                    >
                      <NavLink to={to} end={item.id === "home"}>
                        <item.icon />
                        <span>{item.label}</span>
                      </NavLink>
                    </SidebarMenuButton>
                  </SidebarMenuItem>
                );
              })}
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>
      </SidebarContent>

      <SidebarFooter className="border-t p-2">
        <Button
          type="button"
          variant="outline"
          size="sm"
          className="w-full justify-center gap-2"
          onClick={() => void checkAndInstallUpdate()}
        >
          <DownloadIcon className="size-4" />
          检查更新
        </Button>
      </SidebarFooter>

      <SidebarRail />
    </Sidebar>
  );
}
