import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { toast } from "sonner";

/** 请求更新元数据、下载并安装，成功后重启（与 `tauri.conf.json` 中 updater 配置一致）。 */
export async function checkAndInstallUpdate(): Promise<void> {
  try {
    const update = await check();
    if (!update) {
      toast.message("当前已是最新版本");
      return;
    }
    toast.message(`发现新版本 ${update.version}，正在下载…`);
    await update.downloadAndInstall();
    toast.success("更新已安装，即将重启应用");
    await relaunch();
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    toast.error(`检查或安装更新失败：${msg}`);
  }
}
