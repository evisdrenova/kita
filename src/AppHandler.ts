import { exec } from "child_process";
import * as path from "path";
import * as os from "os";
import { AppInfo } from "./types";
import { BrowserWindow, nativeImage } from "electron";
import chokidar from "chokidar";
import { debounce } from "./lib/utils";
import * as fs from "fs/promises";

/**
 * Handles application discovery, launching, and monitoring on macOS.
 *
 * @example
 * ```typescript
 * const appHandler = new AppHandler();
 * const apps = await appHandler.getAllApps('chrome');
 * ```
 */
export default class AppHandler {
  private cachedApps: AppInfo[] = [];
  private readonly appDirectories: string[];
  private readonly debounceDelay = 2000;
  private mainWindow: BrowserWindow; // used to udpate the resource stats in real time

  constructor(mainWindow: BrowserWindow) {
    this.mainWindow = mainWindow;
    this.appDirectories = [
      "/Applications",
      path.join(os.homedir(), "Applications"),
    ];

    this.loadInstalledApps();
    this.setupFileWatchers();
    setInterval(() => {
      this.updateResourceUsage();
    }, 5000);
  }

  /**
   * Retrieves and filters all applications based on the search query.
   *
   * @param searchQuery - The search term to filter applications
   * @returns A promise that resolves to an array of filtered and sorted AppInfo objects
   *
   * @example
   * ```typescript
   * const apps = await appHandler.getAllApps("chrome");
   * // Returns: [{ name: "Google Chrome", path: "/Applications/Google Chrome.app", isRunning: true }, ...]
   * ```
   *
   * @throws {Error} If there's an error accessing the file system or getting app information
   */
  async getAllApps(searchQuery: string): Promise<AppInfo[]> {
    try {
      return this.filterAndSortApps(searchQuery);
    } catch (error) {
      console.error("Error getting apps:", error);
      return [];
    }
  }

  /**
   * Launches a new application or switches to it if it's already running.
   *
   * @param appInfo - The application information object
   * @returns A promise that resolves to true if the operation was successful, false otherwise
   *
   * @example
   * ```typescript
   * const app = { name: "Safari", path: "/Applications/Safari.app", isRunning: true };
   * await appHandler.launchOrSwitchToApp(app);
   * ```
   *
   * @throws {Error} If there's an error launching or switching to the application
   */
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

  /**
   * Loads all installed applications and updates the cache.
   *
   * @returns A promise that resolves when the cache has been updated
   *
   * @throws {Error} If there's an error reading directories or getting app information
   *
   * @private
   */
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

  /**
   * Sets up file system watchers for application directories.
   *
   * @private
   */
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

  /**
   * Gets all installed applications in a specific directory.
   *
   * @param directory - The directory to scan for applications
   * @returns A promise that resolves to an array of application paths
   *
   * @throws {Error} If there's an error accessing or reading the directory
   *
   * @private
   */
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

  /**
   * Gets the names of all currently running applications.
   *
   * @returns A promise that resolves to an array of running application names
   *
   * @throws {Error} If there's an error executing the AppleScript command
   *
   * @private
   */
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

  /**
   * Gets the icon for an application as a data URL.
   *
   * @param appPath - The path to the application
   * @returns A promise that resolves to the icon data URL or undefined if not found
   *
   * @throws {Error} If there's an error creating the thumbnail
   *
   * @private
   */
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

  /**
   * Merges application paths and running states into AppInfo objects.
   *
   * @param appPaths - Array of application paths
   * @param runningAppNames - Array of names of running applications
   * @returns A promise that resolves to an array of AppInfo objects
   *
   * @throws {Error} If there's an error getting app icons
   *
   * @private
   */
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

  /**
   * Filters and sorts applications based on a search query.
   *
   * @param searchQuery - The search term to filter applications
   * @returns Filtered and sorted array of AppInfo objects
   *
   * @example
   * ```typescript
   * const filteredApps = appHandler.filterAndSortApps("chrome");
   * // Returns apps sorted with running apps first, then alphabetically
   * ```
   *
   * @private
   */
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

  /**
   * Switches to a running application.
   *
   * @param appName - The name of the application to switch to
   * @returns A promise that resolves when the switch is complete
   *
   * @throws {Error} If there's an error executing the AppleScript command
   *
   * @private
   */
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

  /**
   * Launches an application.
   *
   * @param appPath - The path to the application to launch
   * @returns A promise that resolves when the launch is complete
   *
   * @throws {Error} If there's an error launching the application
   *
   * @private
   */
  private async launchApp(appPath: string): Promise<void> {
    return new Promise((resolve, reject) => {
      exec(`open "${appPath}"`, (error) => {
        if (error) reject(error);
        else resolve();
      });
    });
  }
  /**
   * Updates resource (memory, cpu) usage information for running applications.
   * Uses ps command with extended formatting to get accurate process information.
   *
   * @private
   */
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

      console.debug("Resource map:", resourceMap);

      const appNameMapping: Record<string, string> = {
        "google chrome": "chrome",
        "microsoft edge": "edge",
        "visual studio code": "code",
      };

      // Update cached apps.
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

      console.debug(
        "Updated apps with resource usage:",
        this.cachedApps.filter(
          (app) =>
            app.isRunning &&
            ((app.memoryUsage || 0) > 0 || (app.cpuUsage || 0) > 0)
        )
      );

      // Notify the renderer that resource usage was updated.
      if (this.mainWindow && this.mainWindow.webContents) {
        this.mainWindow.webContents.send(
          "resource-usage-updated",
          this.cachedApps
        );
      }
    });
  }
}
