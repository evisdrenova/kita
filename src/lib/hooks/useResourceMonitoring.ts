import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import { AppMetadata, AppResourceUsage } from "@/src/types/types";

// Custom hook for resource monitoring
export function useResourceMonitoring() {
  const [resourceData, setResourceData] = useState<
    Record<number, AppResourceUsage>
  >({});
  const [isMonitoring, setIsMonitoring] = useState(false);
  const [runningApps, setRunningApps] = useState<AppMetadata[]>([]);

  // Start monitoring resources for specific PIDs
  const startMonitoring = async (pids?: number[]) => {
    try {
      // If no PIDs provided, fetch all running apps and extract their PIDs
      if (!pids || pids.length === 0) {
        const apps = await invoke<AppMetadata[]>("get_apps_with_resources");
        const runningPids = apps
          .filter((app) => app.pid !== undefined && app.pid !== null)
          .map((app) => app.pid as number);

        setRunningApps(apps.filter((app) => app.pid !== undefined));

        if (runningPids.length === 0) {
          console.log("No running apps found to monitor");
          return;
        }

        pids = runningPids;
      }

      // Start continuous monitoring of these PIDs
      await invoke("start_resource_monitoring", { pids });

      // Also start the live resource updates stream
      await invoke("get_apps_with_live_resources");

      setIsMonitoring(true);
    } catch (error) {
      console.error("Failed to start resource monitoring:", error);
    }
  };

  // Stop resource monitoring
  const stopMonitoring = async () => {
    try {
      await invoke("stop_resource_monitoring");
      setIsMonitoring(false);
    } catch (error) {
      console.error("Failed to stop resource monitoring:", error);
    }
  };

  // Set up listeners for resource updates
  useEffect(() => {
    let unlistenResource: UnlistenFn;
    let unlistenAppUpdate: UnlistenFn;

    const setupListeners = async () => {
      // Listen for resource usage updates
      unlistenResource = await listen("resource-usage-updated", (event) => {
        const updates = event.payload as Record<number, AppResourceUsage>;
        setResourceData((prev) => ({ ...prev, ...updates }));
      });

      // Listen for apps with resources updates
      unlistenAppUpdate = await listen(
        "apps-with-resources-updated",
        (event) => {
          const updatedApps = event.payload as AppMetadata[];
          setRunningApps(updatedApps.filter((app) => app.pid !== undefined));
        }
      );
    };

    setupListeners();

    // Clean up listeners on unmount
    return () => {
      if (unlistenResource) unlistenResource();
      if (unlistenAppUpdate) unlistenAppUpdate();
      stopMonitoring();
    };
  }, []);

  // Apply resource data to a specific app
  const getAppWithResources = (app: AppMetadata): AppMetadata => {
    if (app.pid && resourceData[app.pid]) {
      return {
        ...app,
        resource_usage: resourceData[app.pid],
      };
    }
    return app;
  };

  // Get current resource usage for a specific PID
  const getResourceForPid = async (
    pid: number
  ): Promise<AppResourceUsage | null> => {
    try {
      const data = await invoke<Record<number, AppResourceUsage>>(
        "get_resource_data",
        { pids: [pid] }
      );
      return data[pid] || null;
    } catch (error) {
      console.error(`Failed to get resource data for PID ${pid}:`, error);
      return null;
    }
  };

  return {
    resourceData,
    isMonitoring,
    runningApps,
    startMonitoring,
    stopMonitoring,
    getAppWithResources,
    getResourceForPid,
  };
}
