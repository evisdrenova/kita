import { exec } from "child_process";
import * as path from "path";
import { AppInfo } from "./types";
import { nativeImage } from "electron";

export default class AppHandler {
  private cachedApps: AppInfo[] = [];
  private lastCacheTime: number = 0;
  private readonly CACHE_DURATION = 5000; // 5 seconds

  constructor() {}

  async getAllApps(searchQuery: string): Promise<AppInfo[]> {
    try {
      await this.updateAppCache();

      return this.filterAndSortApps(searchQuery);
    } catch (error) {
      console.error("Error getting apps:", error);
      return [];
    }
  }

  async launchOrSwitchToApp(appInfo: AppInfo): Promise<boolean> {
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

  private async updateAppCache(): Promise<void> {
    const now = Date.now();
    if (now - this.lastCacheTime < this.CACHE_DURATION) {
      return;
    }

    // gets the list of apps
    try {
      const [installedApps, runningAppNames] = await Promise.all([
        this.getInstalledApps(),
        this.getRunningAppNames(),
      ]);

      this.cachedApps = await this.mergeAppInfo(installedApps, runningAppNames);
      this.lastCacheTime = now;
    } catch (error) {
      console.error("Error updating app cache:", error);
      throw error;
    }
  }

  private async getInstalledApps(): Promise<string[]> {
    return new Promise((resolve, reject) => {
      exec(
        "mdfind \"kMDItemContentType == 'com.apple.application-bundle'\"",
        (error, stdout, stderr) => {
          if (error) {
            reject(error);
            return;
          }
          resolve(stdout.trim().split("\n"));
        }
      );
    });
  }

  private async getRunningAppNames(): Promise<string[]> {
    const script = `
      tell application "System Events"
        set runningApps to (name of every process where background only is false)
        return runningApps
      end tell
    `;

    const result = await new Promise<string>((resolve, reject) => {
      exec(`osascript -e '${script}'`, (error, stdout, stderr) => {
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
    console.log("Retrieving app icon for:", appPath);
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
  ): Promise<AppInfo[]> {
    return Promise.all(
      appPaths.map(async (appPath) => {
        const name = path.basename(appPath, ".app");
        const iconDataUrl = await this.getAppIcon(appPath);
        return {
          name,
          path: appPath,
          isRunning: runningAppNames.includes(name),
          iconDataUrl,
        };
      })
    );
  }

  private filterAndSortApps(searchQuery: string): AppInfo[] {
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
}
