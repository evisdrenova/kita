import { exec } from "child_process";
import * as path from "path";
import * as os from "os";
import { AppMetadata, SearchSectionType } from "./types";
import { BrowserWindow, nativeImage } from "electron";
import chokidar from "chokidar";
import { debounce } from "./lib/utils";
import * as fs from "fs/promises";

export default class AppHandler {
  private cachedApps: AppMetadata[] = [];
  private readonly appDirectories: string[];
  private readonly debounceDelay = 2000;
  private mainWindow: BrowserWindow; // used to udpate the resource stats in real time
  private resourceInterval: NodeJS.Timeout | null = null;
  private readonly resourceUpdateDelay = 1000;

  constructor(mainWindow: BrowserWindow) {
    this.mainWindow = mainWindow;
    this.appDirectories = [
      "/Applications",
      path.join(os.homedir(), "Applications"),
    ];

    this.loadInstalledApps();
    this.setupFileWatchers();
  }

  async getAllApps(searchQuery: string): Promise<AppMetadata[]> {
    try {
      return this.filterAndSortApps(searchQuery);
    } catch (error) {
      console.error("Error getting apps:", error);
      return [];
    }
  }

  async launchOrSwitchToApp(appInfo: AppMetadata): Promise<boolean> {
    try {
      if (appInfo.isRunning) {
        await this.switchToApp(appInfo.name);
      } else {
        await this.launchApp(appInfo.path);
      }
      return true;
    } catch (error) {
      console.error("Error launching/switching app:", error);
      return false;
    }
  }

  private async loadInstalledApps(): Promise<void> {
    try {
      const installedAppsPromises = this.appDirectories.map((dir) =>
        this.getInstalledApps(dir)
      );
      const appPathsArrays = await Promise.all(installedAppsPromises);
      const allAppPaths = ([] as string[]).concat(...appPathsArrays);
      const runningAppNames = await this.getRunningAppNames();
      this.cachedApps = await this.mergeAppInfo(allAppPaths, runningAppNames);
    } catch (error) {
      console.error("Error loading installed apps:", error);
    }
  }

  private setupFileWatchers(): void {
    const debouncedRefresh = debounce(() => {
      this.loadInstalledApps();
    }, this.debounceDelay);

    this.appDirectories.forEach((directory) => {
      const watcher = chokidar.watch(directory, {
        persistent: true,
        depth: 0, // only the top level directory
        ignoreInitial: true,
        usePolling: false,
      });

      watcher
        .on("add", debouncedRefresh)
        .on("unlink", debouncedRefresh)
        .on("change", debouncedRefresh)
        .on("error", (error) => {
          console.error("Watcher error:", error);
        });
    });
  }

  private async getInstalledApps(directory: string): Promise<string[]> {
    try {
      await fs.access(directory);
    } catch (err) {
      console.warn(`Directory ${directory} does not exist.`);
      return [];
    }

    try {
      const entries = await fs.readdir(directory, { withFileTypes: true });
      const apps: string[] = [];
      for (const entry of entries) {
        if (entry.isDirectory() && entry.name.endsWith(".app")) {
          apps.push(path.join(directory, entry.name));
        }
      }
      return apps;
    } catch (error) {
      console.error(`Error reading directory ${directory}:`, error);
      return [];
    }
  }

  private async getRunningAppNames(): Promise<string[]> {
    const script = `
      tell application "System Events"
        set runningApps to (name of every process where background only is false)
        return runningApps
      end tell
    `;
    const result = await new Promise<string>((resolve, reject) => {
      exec(`osascript -e '${script}'`, (error, stdout) => {
        if (error) reject(error);
        else resolve(stdout);
      });
    });
    return result
      .trim()
      .split(", ")
      .map((app) => app.replace(/"/g, "").replace(".app", ""));
  }

  private async getAppIcon(appPath: string): Promise<string | undefined> {
    try {
      const icon = await nativeImage.createThumbnailFromPath(appPath, {
        height: 32,
        width: 32,
      });
      return icon.toDataURL();
    } catch (error) {
      console.error("Error getting icon for app:", appPath, error);
      return undefined;
    }
  }

  private async mergeAppInfo(
    appPaths: string[],
    runningAppNames: string[]
  ): Promise<AppMetadata[]> {
    return Promise.all(
      appPaths.map(async (appPath) => {
        const name = path.basename(appPath, ".app");
        const iconDataUrl = await this.getAppIcon(appPath);
        return {
          name,
          path: appPath,
          isRunning: runningAppNames.includes(name),
          iconDataUrl,
          type: SearchSectionType.Apps,
        };
      })
    );
  }

  private filterAndSortApps(searchQuery: string): AppMetadata[] {
    return this.cachedApps
      .filter((app) =>
        app.name.toLowerCase().includes(searchQuery.toLowerCase())
      )
      .sort((a, b) => {
        if (a.isRunning !== b.isRunning) {
          return a.isRunning ? -1 : 1;
        }
        return a.name.localeCompare(b.name);
      });
  }

  private async switchToApp(appName: string): Promise<void> {
    const script = `
      tell application "System Events"
        set frontmost of process "${appName}" to true
      end tell
    `;
    return new Promise((resolve, reject) => {
      exec(`osascript -e '${script}'`, (error) => {
        if (error) reject(error);
        else resolve();
      });
    });
  }

  private async launchApp(appPath: string): Promise<void> {
    return new Promise((resolve, reject) => {
      exec(`open "${appPath}"`, (error) => {
        if (error) reject(error);
        else resolve();
      });
    });
  }

  private updateResourceUsage(): void {
    const cmd = "ps -axo pid,rss,pcpu,command";
    exec(cmd, (error, stdout) => {
      if (error) {
        console.error("Error getting resource usage:", error);
        return;
      }
      const lines = stdout.split("\n");
      const resourceMap: Record<string, { memory: number; cpu: number }> = {};

      lines.slice(1).forEach((line) => {
        const parts = line.trim().split(/\s+/);
        if (parts.length >= 4) {
          const rssKb = parseInt(parts[1], 10);
          const cpuUsagePercent = parseFloat(parts[2]);
          const command = parts.slice(3).join(" ");
          if (!isNaN(rssKb) && !isNaN(cpuUsagePercent)) {
            let appName = "";
            const appMatch = command.match(/\/([^/]+)\.app/);
            if (appMatch) {
              appName = appMatch[1].toLowerCase();
            }
            if (!appName) {
              appName = command.split(/\s+/)[0].toLowerCase();
            }
            appName = appName
              .replace(/^[./]+/, "")
              .replace(/\s+helper.*$/, "")
              .replace(/\s+renderer.*$/, "")
              .replace(/\s+worker.*$/, "");
            if (appName) {
              if (!resourceMap[appName]) {
                resourceMap[appName] = { memory: 0, cpu: 0 };
              }
              resourceMap[appName].memory += rssKb / 1024;
              resourceMap[appName].cpu += cpuUsagePercent;
            }
          }
        }
      });

      const appNameMapping: Record<string, string> = {
        "google chrome": "chrome",
        "microsoft edge": "edge",
        "visual studio code": "code",
      };

      // Update cached apps
      this.cachedApps = this.cachedApps.map((app) => {
        if (app.isRunning) {
          let searchName = app.name.toLowerCase();
          searchName = appNameMapping[searchName] || searchName;
          const variations = [
            searchName,
            searchName.replace(/\s+/g, ""),
            searchName.split(" ")[0],
            searchName.replace(/[^a-z0-9]/g, ""),
          ];
          let memoryUsage = 0;
          let cpuUsage = 0;
          for (const variant of variations) {
            if (resourceMap[variant]) {
              memoryUsage = resourceMap[variant].memory;
              cpuUsage = resourceMap[variant].cpu;
              break;
            }
          }
          return { ...app, memoryUsage, cpuUsage };
        }
        return app;
      });

      // Notify the renderer that resource usage was updated.
      if (this.mainWindow && this.mainWindow.webContents) {
        this.mainWindow.webContents.send(
          "resource-usage-updated",
          this.cachedApps
        );
      }
    });
  }

  public startResourceMonitoring() {
    this.stopResourceMonitoring();
    this.resourceInterval = setInterval(() => {
      this.updateResourceUsage();
    }, this.resourceUpdateDelay);
    this.updateResourceUsage();
  }

  public stopResourceMonitoring() {
    if (this.resourceInterval) {
      clearInterval(this.resourceInterval);
      this.resourceInterval = null;
    }
  }
}
