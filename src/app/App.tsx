import "./App.css";
import { Navigate, Outlet, Route, Routes, useLocation } from "react-router-dom";
import { TooltipProvider } from "@/components/ui/tooltip";
import { SidebarInset, SidebarProvider, SidebarTrigger } from "@/components/ui/sidebar";
import { Toaster } from "@/components/ui/sonner";
import { AppSidebar } from "@/components/app-sidebar";
import { WindowControls } from "@/app/ui/WindowControls";
import { PAGE_TITLE, getPageIdFromPathname } from "@/app/route-meta";
import { HomePage } from "@/views/home/HomePage";
import { CrawlPage } from "@/views/crawl/CrawlPage";
import { AccountPage } from "@/views/account/AccountPage";
import { IpPage } from "@/views/ip/IpPage";
import { DatabasePage } from "@/views/database/DatabasePage";

function AppShell() {
  const { pathname } = useLocation();
  const pageId = getPageIdFromPathname(pathname);

  return (
    <TooltipProvider>
      {/* 顶层：`header` 与 SidebarTrigger、页面标题、窗口控制同一行；左侧为拖拽区。 */}
      <div className="flex h-dvh flex-col overflow-hidden">
        <SidebarProvider className="min-h-0 flex-1 overflow-hidden">
          <AppSidebar />
          <SidebarInset className="min-h-0 overflow-hidden">
            <header className="sticky top-0 z-10 flex h-12 shrink-0 items-stretch border-b bg-background pl-4 pr-0">
              <div
                data-tauri-drag-region
                className="flex min-w-0 flex-1 items-center gap-2 pr-2"
              >
                <SidebarTrigger className="-ml-1 shrink-0" />
                <span className="truncate text-sm font-medium">{PAGE_TITLE[pageId]}</span>
              </div>
              <WindowControls />
            </header>
            <div className="flex min-h-0 flex-1 flex-col overflow-y-auto">
              <Outlet />
            </div>
          </SidebarInset>
        </SidebarProvider>
      </div>
      {/* 全局 toast 容器：业务侧 `import { toast } from "sonner"` 即可触发 */}
      <Toaster />
    </TooltipProvider>
  );
}

export default function App() {
  return (
    <Routes>
      <Route element={<AppShell />}>
        <Route index element={<HomePage />} />
        <Route path="crawl" element={<CrawlPage />} />
        <Route path="account" element={<AccountPage />} />
        <Route path="ip" element={<IpPage />} />
        <Route path="database" element={<DatabasePage />} />
        <Route path="*" element={<Navigate to="/" replace />} />
      </Route>
    </Routes>
  );
}
