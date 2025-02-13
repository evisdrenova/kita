import { contextBridge, ipcRenderer, IpcRendererEvent } from "electron";
import { SettingsValue } from "./settings/Settings";
import { AppInfo } from "./types";

contextBridge.exposeInMainWorld("electron", {
  // settings methods
  getSettings: <T extends SettingsValue>(key: string) => {
    return ipcRenderer.invoke("db-get-setting", key) as Promise<T | undefined>;
  },
  setSettings: (key: string, value: SettingsValue) => {
    return ipcRenderer.invoke("db-set-setting", key, value);
  },
  getAllSettings: () => {
    return ipcRenderer.invoke("db-get-all-settings");
  },
  setMultipleSettings: (settings: Record<string, SettingsValue>) => {
    return ipcRenderer.invoke("db-set-multiple-settings", settings);
  },
  indexDirectories: (directories: string[]) => {
    return ipcRenderer.invoke("index-directories", directories);
  },
  onIndexingProgress: (callback: (progress: any) => void) => {
    return ipcRenderer.on("indexing-progress", callback);
  },
  removeIndexingProgress: (callback: (progress: any) => void) => {
    return ipcRenderer.removeListener("indexing-progress", callback);
  },
  selectDirectory: () => {
    return ipcRenderer.invoke("dialog:selectDirectory");
  },
  searchFiles: (query: string) => {
    return ipcRenderer.invoke("search-files", query);
  },
  launchOrSwitch: (appInfo: AppInfo) => {
    return ipcRenderer.invoke("launch-or-switch", appInfo);
  },

  openFile: (filePath: string) => {
    return ipcRenderer.invoke("open-file", filePath);
  },
  minimizeWindow: () => ipcRenderer.send("window-minimize"),
  maximizeWindow: () => ipcRenderer.send("window-maximize"),
  closeWindow: () => ipcRenderer.send("window-close"),
  onResourceUsageUpdated: (
    callback: (event: IpcRendererEvent, updatedApps: AppInfo[]) => void
  ) => ipcRenderer.on("resource-usage-updated", callback),
  removeResourceUsageUpdated: (
    callback: (event: IpcRendererEvent, updatedApps: AppInfo[]) => void
  ) => ipcRenderer.removeListener("resource-usage-updated", callback),
});
