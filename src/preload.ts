import { contextBridge, ipcRenderer } from "electron";
import { SettingsValue } from "./settings/Settings";

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

  // window methods
  // send() is for one-way communication, invoke() returns a promise
  minimizeWindow: () => ipcRenderer.send("window-minimize"),
  maximizeWindow: () => ipcRenderer.send("window-maximize"),
  closeWindow: () => ipcRenderer.send("window-close"),
});
