import { contextBridge, ipcRenderer, IpcRendererEvent } from "electron";
import { SettingsValue } from "./settings/Settings";
import { AppMetadata } from "./types";

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
  indexAndEmbedPaths: (directories: string[]) => {
    return ipcRenderer.invoke("index-and-embed-paths", directories);
  },
  onIndexingProgress: (callback: (progress: any) => void) => {
    return ipcRenderer.on("indexing-progress", callback);
  },
  removeIndexingProgress: (callback: (progress: any) => void) => {
    return ipcRenderer.removeListener("indexing-progress", callback);
  },
  selectPaths: (options: Electron.OpenDialogOptions) => {
    return ipcRenderer.invoke("dialog:selectPaths", options);
  },
  searchFiles: (query: string) => {
    return ipcRenderer.invoke("search-files", query);
  },
  searchFilesAndEmbeddings: (query: string) => {
    return ipcRenderer.invoke("search-files-and-embeddings", query);
  },
  launchOrSwitch: (appInfo: AppMetadata) => {
    return ipcRenderer.invoke("launch-or-switch", appInfo);
  },

  openFile: (filePath: string) => {
    return ipcRenderer.invoke("open-file", filePath);
  },
  minimizeWindow: () => ipcRenderer.send("window-minimize"),
  maximizeWindow: () => ipcRenderer.send("window-maximize"),
  closeWindow: () => ipcRenderer.send("window-close"),
  onResourceUsageUpdated: (
    callback: (event: IpcRendererEvent, updatedApps: AppMetadata[]) => void
  ) => ipcRenderer.on("resource-usage-updated", callback),
  removeResourceUsageUpdated: (
    callback: (event: IpcRendererEvent, updatedApps: AppMetadata[]) => void
  ) => ipcRenderer.removeListener("resource-usage-updated", callback),
  startResourceMonitoring: () => {
    return ipcRenderer.invoke("start-resource-monitoring");
  },
  stopResourceMonitoring: () => {
    return ipcRenderer.invoke("stop-resource-monitoring");
  },
  getRecents: () => {
    return ipcRenderer.invoke("get-recents");
  },
});
