import React, { useState, useMemo, memo, useCallback } from "react";
import { Package, MemoryStick, Cpu, X, RefreshCw, Loader2 } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { successToast, errorToast } from "./components/ui/toast";
import { cn, FormatFileSize } from "./lib/utils";
import { AppMetadata, Column } from "./types/types";

interface Props {
  data: AppMetadata[];
  refreshApps: () => Promise<void>;
  appResourceData?: Record<number, { cpu_usage: number; memory_bytes: number }>;
  onRowClick?: (app: AppMetadata) => void;
  selectedItemName?: string;
}

const columns: Column<AppMetadata>[] = [
  {
    key: "name",
    header: "Name",
    width: 70,
    render: (app) => (
      <div className="flex items-center min-w-0">
        {app?.icon ? (
          <img
            src={app.icon}
            className="w-4 h-4 object-contain mr-2"
            alt={app.name}
          />
        ) : (
          <Package className="h-4 w-4 mr-2" />
        )}
        <span className="text-sm truncate text-white">{app?.name}</span>
        {app?.pid && (
          <div className="relative flex items-center justify-center ml-2">
            <div className="absolute w-2 h-2 bg-green-500/30 rounded-full animate-ping" />
            <div className="relative w-[6px] h-[6px] bg-green-500 rounded-full shadow-lg shadow-green-500/50" />
          </div>
        )}
      </div>
    ),
  },
  {
    key: "memory",
    header: "Memory",
    width: 20,
    render: (app) => (
      <MemoryCell
        pid={app.pid}
        memoryBytes={app.resource_usage?.memory_bytes}
      />
    ),
  },
  {
    key: "cpu",
    header: "CPU",
    width: 20,
    render: (app) => (
      <CpuCell pid={app.pid} cpuUsage={app.resource_usage?.cpu_usage} />
    ),
  },
  {
    key: "actions",
    header: "Actions",
    width: 20,
  },
];

// memoize memorycell to reduce re-renders
const MemoryCell = React.memo(
  function MemoryCell({
    pid,
    memoryBytes,
  }: {
    pid?: number;
    memoryBytes?: number;
  }) {
    if (!pid || memoryBytes === undefined) return null;

    return (
      <div className="flex items-center justify-start gap-1 text-xs text-gray-200">
        <MemoryStick className="w-3 h-3" />
        {typeof memoryBytes === "number"
          ? FormatFileSize(memoryBytes)
          : memoryBytes}
      </div>
    );
  },
  (prev, next) => {
    return prev.pid === next.pid && prev.memoryBytes === next.memoryBytes;
  }
);

const CpuCell = React.memo(
  function CpuCell({ pid, cpuUsage }: { pid?: number; cpuUsage?: number }) {
    if (!pid || cpuUsage === undefined) return null;

    return (
      <div className="flex items-center justify-start gap-1 text-xs text-gray-200">
        <Cpu className="w-3 h-3" />
        {typeof cpuUsage === "number" ? cpuUsage.toFixed(1) : cpuUsage}%
      </div>
    );
  },
  (prev, next) => {
    return prev.pid === next.pid && prev.cpuUsage === next.cpuUsage;
  }
);

export default function AppTable(props: Props) {
  const {
    data,
    refreshApps,
    appResourceData = {},
    onRowClick,
    selectedItemName,
  } = props;

  const [sortKey, setSortKey] = useState<string | null>(null);
  const [sortDirection, setSortDirection] = useState<"asc" | "desc">("asc");

  // Handle column sorting
  const handleSort = useCallback(
    (key: string) => {
      setSortKey((prevKey) => {
        if (prevKey === key) {
          // If already sorting by this key, cycle through directions
          if (sortDirection === "asc") {
            setSortDirection("desc");
            return key;
          } else {
            // If already at desc, go back to no sort
            return null;
          }
        } else {
          // New column, start with ascending sort
          setSortDirection("asc");
          return key;
        }
      });
    },
    [sortDirection]
  );

  // Apply resource data and sort in a single pass
  const processedApps = useMemo(() => {
    // First apply resource data
    const appsWithData = data.map((app) => {
      if (app.pid && appResourceData[app.pid]) {
        return {
          ...app,
          resource_usage: {
            pid: app.pid,
            cpu_usage: appResourceData[app.pid].cpu_usage,
            memory_bytes: appResourceData[app.pid].memory_bytes,
          },
        };
      }
      return app;
    });

    // If there's no explicit sort column selected, maintain the pre-sorted order
    if (sortKey === null) {
      return appsWithData;
    }

    // Otherwise, separate active and inactive apps for sorting
    const activeApps: AppMetadata[] = [];
    const inactiveApps: AppMetadata[] = [];

    appsWithData.forEach((app) => {
      if (app.pid) {
        activeApps.push(app);
      } else {
        inactiveApps.push(app);
      }
    });

    // Sort active apps based on sort key
    activeApps.sort((a, b) => {
      if (sortKey === "name") {
        const aName = a.name.toLowerCase();
        const bName = b.name.toLowerCase();
        return sortDirection === "asc"
          ? aName.localeCompare(bName)
          : bName.localeCompare(aName);
      }

      if (sortKey === "memory") {
        const memA = a.resource_usage?.memory_bytes || 0;
        const memB = b.resource_usage?.memory_bytes || 0;
        return sortDirection === "asc" ? memA - memB : memB - memA;
      }

      if (sortKey === "cpu") {
        const cpuA = a.resource_usage?.cpu_usage || 0;
        const cpuB = b.resource_usage?.cpu_usage || 0;
        return sortDirection === "asc" ? cpuA - cpuB : cpuB - cpuA;
      }

      return 0;
    });

    // Sort inactive apps by name
    inactiveApps.sort((a, b) => {
      const aName = a.name.toLowerCase();
      const bName = b.name.toLowerCase();

      if (sortKey === "name") {
        return sortDirection === "asc"
          ? aName.localeCompare(bName)
          : bName.localeCompare(aName);
      }

      // Otherwise, always sort A-Z
      return aName.localeCompare(bName);
    });

    // Return active apps first, then inactive
    return [...activeApps, ...inactiveApps];
  }, [data, appResourceData, sortKey, sortDirection]);

  return (
    <>
      {processedApps.length > 0 && (
        <div className=" flex flex-col">
          <div className="bg-inherit ">
            <table
              className="w-full border-collapse"
              style={{ tableLayout: "fixed" }}
            >
              <thead>
                <tr>
                  {columns.map((column) => (
                    <th
                      key={column.key}
                      className="text-left p-2 text-xs font-medium text-gray-400"
                      onClick={() =>
                        column.key !== "actions" && handleSort(column.key)
                      }
                      style={{
                        cursor:
                          column.key !== "actions" ? "pointer" : "default",
                        width: `${column.width}%`,
                      }}
                    >
                      {column.header}
                      {sortKey === column.key && column.key !== "actions" && (
                        <span className="ml-1">
                          {sortDirection === "asc" ? "↑" : "↓"}
                        </span>
                      )}
                    </th>
                  ))}
                </tr>
              </thead>
            </table>
          </div>
          <div>
            <table
              className="w-full border-collapse"
              style={{ tableLayout: "fixed" }}
            >
              <tbody>
                {processedApps.map((app) => (
                  <TableRow
                    key={app.pid || `${app.path}-${app.name}`}
                    app={app}
                    columns={columns}
                    onRowClick={onRowClick}
                    refreshApps={refreshApps}
                    isSelected={app.name === selectedItemName}
                  />
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}
    </>
  );
}

const TableRow = memo(
  ({
    app,
    columns,
    onRowClick,
    refreshApps,
    isSelected,
  }: {
    app: AppMetadata;
    columns: Column<AppMetadata>[];
    onRowClick?: (app: AppMetadata) => void;
    refreshApps: () => Promise<void>;
    isSelected: boolean;
  }) => {
    const [isKilling, setIsKilling] = useState<boolean>(false);
    const [isRestarting, setIsRestarting] = useState<boolean>(false);

    const handleClick = useCallback(() => {
      if (onRowClick) onRowClick(app);
    }, [app, onRowClick]);

    const handleForceQuit = useCallback(
      async (e: React.MouseEvent) => {
        e.stopPropagation();
        if (!app.pid) return;

        try {
          setIsKilling(true);
          // Show a loading toast while terminating
          const loadingToastId = successToast(`Terminating ${app.name}...`, {
            duration: Infinity, // Don't auto-dismiss
            position: "bottom-right",
          });

          try {
            await invoke("force_quit_application", { pid: app.pid });
            successToast(`Successfully terminated ${app.name}`, {
              id: loadingToastId, // Replace the loading toast
              duration: 3000,
            });
            await refreshApps();
          } catch (error) {
            errorToast(`Failed to terminate ${app.name}: ${error}`, {
              id: loadingToastId, // Replace the loading toast
              duration: 3000,
            });
          }
        } catch (error) {
          console.error("Error in force quit process:", error);
          errorToast(`Failed to quit ${app.name}`);
        } finally {
          setIsKilling(false);
        }
      },
      [app.pid, app.name, refreshApps]
    );

    const handleRestart = useCallback(
      async (e: React.MouseEvent) => {
        e.stopPropagation();
        try {
          setIsRestarting(true);
          await invoke("restart_application", { app });
          successToast(`Restarting ${app.name}`);
          await refreshApps();
        } catch (error) {
          console.error("Failed to restart app:", error);
          errorToast(`Failed to restart ${app.name}`);
        } finally {
          setIsRestarting(false);
        }
      },
      [app, refreshApps]
    );

    const renderActions = () => {
      if (!app.pid) return null;

      return (
        <div className="flex flex-row items-center justify-start gap-4 ">
          <button
            onClick={handleForceQuit}
            disabled={isKilling}
            className="p-1 rounded-sm hover:bg-red-500/10 text-red-500 hover:text-red-600 transition-colors cursor-pointer"
            title="Force quit"
          >
            {isKilling ? (
              <Loader2 className="h-3 w-3 animate-spin" />
            ) : (
              <X className="h-3 w-3" />
            )}
          </button>
          <button
            onClick={handleRestart}
            disabled={isRestarting}
            className="p-1 rounded-sm hover:bg-blue-500/10 text-blue-500 hover:text-blue-600 transition-colors cursor-pointer"
            title="Restart application"
          >
            {isRestarting ? (
              <Loader2 className="h-3 w-3 animate-spin" />
            ) : (
              <RefreshCw className="h-3 w-3" />
            )}
          </button>
        </div>
      );
    };

    const renderCells = (column: Column<AppMetadata>) => {
      if (column.key === "actions") {
        return renderActions();
      } else if (column.render) {
        return column.render(app);
      } else {
        (app as any)[column.key];
      }
    };

    // Create a styling approach that works with table structure
    return (
      <tr
        onClick={handleClick}
        className={cn(
          "hover:bg-zinc-200 dark:hover:bg-zinc-800 transition-colors cursor-pointer mb-2",
          isSelected ? "bg-muted" : ""
        )}
        style={{ borderRadius: "8px", overflow: "hidden" }}
      >
        {columns.map((column, index) => (
          <td
            key={column.key}
            className={cn(
              "p-2",
              index === 0 ? "rounded-l" : "",
              index === columns.length - 1 ? "rounded-r" : ""
            )}
            style={{ width: `${column.width}%` }}
          >
            {renderCells(column)}
          </td>
        ))}
      </tr>
    );
  }
);
