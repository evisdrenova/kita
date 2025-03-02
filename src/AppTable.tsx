import React, { useState, useMemo, memo, useCallback } from "react";
import { Package, MemoryStick, Cpu, X, RefreshCw, Loader2 } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { successToast, errorToast } from "./components/ui/toast";
import { FormatFileSize } from "./lib/utils";
import { AppMetadata, Column } from "./types/types";

interface Props {
  data: AppMetadata[];
  refreshApps: () => Promise<void>;
  appResourceData?: Record<number, { cpu_usage: number; memory_bytes: number }>;
  onRowClick?: (app: AppMetadata) => void;
}

const columns: Column<AppMetadata>[] = [
  {
    key: "name",
    header: "Application",
    width: 40,
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
        <span className="text-sm truncate">{app?.name}</span>
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
    render: (app) => {
      const memoryUsage = app.resource_usage?.memory_bytes;
      return app?.pid && memoryUsage !== undefined ? (
        <div className="flex items-center justify-end gap-1 text-xs text-gray-500">
          <MemoryStick className="w-3 h-3" />
          {typeof memoryUsage === "number"
            ? FormatFileSize(memoryUsage)
            : memoryUsage}
        </div>
      ) : null;
    },
  },
  {
    key: "cpu",
    header: "CPU",
    width: 20,
    render: (app) => {
      const cpuUsage = app.resource_usage?.cpu_usage;
      return app?.pid && cpuUsage !== undefined ? (
        <div className="flex items-center justify-end gap-1 text-xs text-gray-500">
          <Cpu className="w-3 h-3" />
          {typeof cpuUsage === "number" ? cpuUsage.toFixed(1) : cpuUsage}%
        </div>
      ) : null;
    },
  },
  {
    key: "actions",
    header: "Actions",
    width: 20,
  },
];

export default function AppTable(props: Props) {
  const { data, refreshApps, appResourceData = {}, onRowClick } = props;

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

    // Then sort the processed apps
    return appsWithData.sort((a, b) => {
      // ALWAYS sort running apps first, regardless of sort column
      if (a.pid && !b.pid) return -1; // a is running, b is not
      if (!a.pid && b.pid) return 1; // b is running, a is not

      // If no sort key or in default state, sort alphabetically by name within running groups
      if (!sortKey) {
        const aValue = a.name.toLowerCase();
        const bValue = b.name.toLowerCase();
        return aValue.localeCompare(bValue);
      }

      // If both apps have the same running status, then sort by the selected column
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

      if (sortKey === "name") {
        const aValue = a.name.toLowerCase();
        const bValue = b.name.toLowerCase();
        return sortDirection === "asc"
          ? aValue.localeCompare(bValue)
          : bValue.localeCompare(aValue);
      }

      return 0;
    });
  }, [data, appResourceData, sortKey, sortDirection]);

  return (
    <div className="table-container" style={{ overflowX: "auto" }}>
      <table
        className="w-full border-collapse"
        style={{ tableLayout: "fixed" }}
      >
        <thead>
          <tr>
            {columns.map((column) => (
              <th
                key={column.key}
                className="text-left p-2 text-sm font-medium text-gray-500"
                onClick={() =>
                  column.key !== "actions" && handleSort(column.key)
                }
                style={{
                  cursor: column.key !== "actions" ? "pointer" : "default",
                  width: column.width,
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
        <tbody>
          {processedApps.map((app) => (
            <TableRow
              key={app.pid || app.name}
              app={app}
              columns={columns}
              onRowClick={onRowClick}
              refreshApps={refreshApps}
            />
          ))}
        </tbody>
      </table>
    </div>
  );
}

const TableRow = memo(
  ({
    app,
    columns,
    onRowClick,
    refreshApps,
  }: {
    app: AppMetadata;
    columns: Column<AppMetadata>[];
    onRowClick?: (app: AppMetadata) => void;
    refreshApps: () => Promise<void>;
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
          await invoke("force_quit_application", { pid: app.pid });
          successToast(`Successfully quit: ${app.name}`);
          await refreshApps();
        } catch (error) {
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
        <div className="flex items-center justify-end space-x-1 ">
          <button
            onClick={handleForceQuit}
            disabled={isKilling}
            className="p-1 rounded-sm hover:bg-red-500/10 text-red-500 hover:text-red-600 transition-colors"
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
            className="p-1 rounded-sm hover:bg-blue-500/10 text-blue-500 hover:text-blue-600 transition-colors"
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

    return (
      <tr
        onClick={handleClick}
        className="hover:bg-muted transition-colors group cursor-pointer"
      >
        {columns.map((column) => (
          <td key={column.key} className="p-2">
            {column.key === "actions"
              ? renderActions()
              : column.render
              ? column.render(app)
              : (app as any)[column.key]}
          </td>
        ))}
      </tr>
    );
  }
);
