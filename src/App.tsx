import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import "./globals.css";
import Footer from "./Footer";
import {
  AppMetadata,
  AppResourceUsage,
  FileMetadata,
  IndexingProgress,
  searchCategories,
  SearchCategory,
  SearchItem,
  SearchSection,
  SearchSectionType,
  SemanticMetadata,
} from "./types/types";
import Header from "./Header";
import { toast } from "sonner";
import {
  FormatFileSize,
  getCategoryFromExtension,
  truncatePath,
} from "./lib/utils";
import {
  Check,
  Copy,
  Cpu,
  Database,
  File,
  FileArchive,
  FileCode,
  FileSpreadsheet,
  FileText,
  Film,
  Image,
  MemoryStick,
  Music,
  Package,
  Search,
} from "lucide-react";

import { FaRegFilePdf } from "react-icons/fa";

export default function App() {
  const [searchQuery, setSearchQuery] = useState<string>("");
  const [selectedCategories, setSelectedCategories] = useState<
    Set<SearchCategory>
  >(new Set(searchCategories));
  const [isIndexing, setIsIndexing] = useState(false);
  const [indexingProgress, setIndexingProgress] =
    useState<IndexingProgress | null>(null);
  const [isSettingsOpen, setIsSettingsOpen] = useState<boolean>(false);
  const [searchSections, setSearchSections] = useState<SearchSection[]>([]);
  const [selectedSection, setSelectedSection] = useState<number>(0);
  const [selectedItem, setSelectedItem] = useState<number>(0);
  const [recents, setRecents] = useState<FileMetadata[]>([]);
  const [isSearchActive, setIsSearchActive] = useState(false);
  const [resourceData, setResourceData] = useState<
    Record<number, { cpu_usage: number; memory_mb: number }>
  >({}); // this is {pid:{cpu, memory}}

  // Set up listener for resource usage updates
  useEffect(() => {
    let unlistenResource: UnlistenFn;
    let unlistenAppUpdate: UnlistenFn;
    let unlistenAppLaunch: UnlistenFn;

    const setupListeners = async () => {
      // Listen for resource usage updates
      unlistenResource = await listen("resource-usage-updated", (event) => {
        const updates = event.payload as Record<
          number,
          { cpu_usage: number; memory_mb: number }
        >;
        setResourceData((prev) => ({ ...prev, ...updates }));

        // Also update the app sections with the new resource data
        setSearchSections((prev) => {
          return prev.map((section) => {
            if (section.type_ === SearchSectionType.Apps) {
              const updatedItems = section.items.map((item) => {
                const app = item as AppMetadata;
                if (app.pid && updates[app.pid]) {
                  // Create a proper SearchItem (AppMetadata) with updated resource usage
                  return {
                    ...app,
                    resource_usage: {
                      pid: app.pid, // Make sure pid is included
                      cpu_usage: updates[app.pid].cpu_usage,
                      memory_mb: updates[app.pid].memory_mb,
                      memory_bytes: updates[app.pid].memory_mb * 1024 * 1024, // Convert MB to bytes
                    },
                  } as AppMetadata as SearchItem; // Explicit cast to maintain type safety
                }
                return item;
              });

              return { ...section, items: updatedItems };
            }
            return section;
          });
        });
      });

      // Listen for apps with resources updates
      unlistenAppUpdate = await listen(
        "apps-with-resources-updated",
        (event) => {
          const updatedApps = event.payload as AppMetadata[];

          setSearchSections((prev) => {
            return prev.map((section) => {
              if (section.type_ === SearchSectionType.Apps) {
                return {
                  ...section,
                  items: updatedApps.map((app) => app as SearchItem),
                };
              }
              return section;
            });
          });
        }
      );

      // Listen for app activation/launch events
      unlistenAppLaunch = await listen("app-launched", (event) => {
        const launchedApp = event.payload as AppMetadata;

        // Update the app in the search sections
        setSearchSections((prev) => {
          return prev.map((section) => {
            if (section.type_ === SearchSectionType.Apps) {
              const updatedItems = section.items.map((item) => {
                const app = item as AppMetadata;
                if (app.path === launchedApp.path) {
                  return launchedApp;
                }
                return app;
              });

              return { ...section, items: updatedItems };
            }
            return section;
          });
        });
      });

      // Start resource monitoring for all running apps
      await startResourceMonitoring();
    };

    setupListeners();

    // Clean up listeners on unmount
    return () => {
      if (unlistenResource) unlistenResource();
      if (unlistenAppUpdate) unlistenAppUpdate();
      if (unlistenAppLaunch) unlistenAppLaunch();

      // Stop resource monitoring
      invoke("stop_resource_monitoring").catch((err) => {
        console.error("Failed to stop resource monitoring:", err);
      });
    };
  }, []);

  // useEffect(() => {
  //   const handleProgress = (_: any, progress: IndexingProgress) => {
  //     setIndexingProgress(progress);
  //   };

  //   window.electron.onIndexingProgress(handleProgress);
  //   return () => {
  //     window.electron.removeIndexingProgress(handleProgress);
  //   };
  // }, []);

  // useEffect(() => {
  //   const handler = (event: any, apps: AppMetadata[]) => {
  //     setUpdatedApps(apps);
  //   };

  //   window.electron.onResourceUsageUpdated(handler);
  //   return () => {
  //     window.electron.removeResourceUsageUpdated(handler);
  //   };
  // }, []);

  // // retrieves the recents
  // useEffect(() => {
  //   const fetchRecents = async () => {
  //     try {
  //       const recentFiles = await window.electron.getRecents();
  //       setRecents(recentFiles);
  //     } catch (error) {
  //       console.error("Error fetching recents:", error);
  //     }
  //   };
  //   fetchRecents();

  //   const interval = setInterval(fetchRecents, 60000);
  //   return () => clearInterval(interval);
  // }, []);

  // useEffect(() => {
  //   return () => {
  //     // Stop resource monitoring when component unmounts
  //     if (isSearchActive) {
  //       window.electron.stopResourceMonitoring();
  //     }
  //   };
  // }, [isSearchActive]);

  // const handleSelectPaths = async () => {
  //   try {
  //     const result = await window.electron.selectPaths({
  //       properties: ["openFile", "openDirectory", "multiSelections"],
  //     });

  //     if (result.canceled || !result.filePaths.length) return;

  //     setIsIndexing(true);
  //     setIndexingProgress(null);

  //     // Index all selected paths
  //     await window.electron.indexAndEmbedPaths(result.filePaths);

  //     setIsIndexing(false);
  //     setIndexingProgress(null);
  //   } catch (error) {
  //     console.error("Error indexing paths:", error);
  //     setIsIndexing(false);
  //     setIndexingProgress(null);
  //     toast.error("Errr indexing selected paths");
  //   }
  // };

  // const toggleCategory = (category: SearchCategory) => {
  //   setSelectedCategories((prev) => {
  //     const newSet = new Set(prev);
  //     if (newSet.has(category)) {
  //       newSet.delete(category);
  //     } else {
  //       newSet.add(category);
  //     }
  //     return newSet;
  //   });
  // };

  // handles key up and down acions
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (searchSections.length === 0) return;

      if (e.key === "ArrowDown") {
        e.preventDefault();
        const currentSection = searchSections[selectedSection];
        if (selectedItem < currentSection.items.length - 1) {
          setSelectedItem(selectedItem + 1);
        } else if (selectedSection < searchSections.length - 1) {
          setSelectedSection(selectedSection + 1);
          setSelectedItem(0);
        }
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        if (selectedItem > 0) {
          setSelectedItem(selectedItem - 1);
        } else if (selectedSection > 0) {
          setSelectedSection(selectedSection - 1);
          setSelectedItem(searchSections[selectedSection - 1].items.length - 1);
        }
      } else if (e.key === "Enter") {
        e.preventDefault();
        const section = searchSections[selectedSection];
        const item = section?.items[selectedItem];
        if (item) {
          handleResultSelect(item);
        }
      }
    };

    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [searchSections, selectedSection, selectedItem]);

  // handles switching to the file or app
  async function handleResultSelect(app: SearchItem) {
    async () =>
      await invoke<AppMetadata[]>("launch_or_switch_to_application", {
        app: app,
      });
  }

  const startResourceMonitoring = async () => {
    try {
      // Get all running apps with their PIDs
      const apps = await invoke<AppMetadata[]>("get_apps_with_resources");

      // Extract PIDs of running apps
      const runningPids = apps
        .filter((app) => app.pid !== undefined && app.pid !== null)
        .map((app) => app.pid as number);

      if (runningPids.length > 0) {
        // Start continuous monitoring of these PIDs
        await invoke("start_resource_monitoring", { pids: runningPids });

        // Also start the live resource updates stream
        await invoke("get_apps_with_live_resources");

        setIsSearchActive(true);
      }
    } catch (error) {
      console.error("Failed to start resource monitoring:", error);
    }
  };

  // Gets all of the apps
  useEffect(() => {
    const fetchAllApps = async () => {
      try {
        // Try to get apps with resource usage directly
        const apps = await invoke<SearchSection[]>("get_search_data");
        setSearchSections(apps);

        // If we have apps with PIDs, start monitoring them
        const hasRunningApps = apps.some(
          (section) =>
            section.type_ === SearchSectionType.Apps &&
            section.items.some(
              (item) => (item as AppMetadata).pid !== undefined
            )
        );

        if (hasRunningApps) {
          startResourceMonitoring();
        }
      } catch (error) {
        console.error("Failed to fetch apps:", error);
      }
    };

    fetchAllApps();

    // Refresh apps every 10 seconds to catch newly launched apps
    const interval = setInterval(fetchAllApps, 10000);
    return () => clearInterval(interval);
  }, []);

  // filters results based on what the user searches for
  const filteredResults = useMemo(() => {
    if (!searchQuery.trim()) {
      return searchSections;
    }

    const query = searchQuery.toLowerCase();
    const filteredSections = searchSections
      .map((section) => ({
        ...section,
        items: section.items.filter((item) =>
          item.name.toLowerCase().includes(query)
        ),
      }))
      .filter((section) => section.items.length > 0);

    return [...filteredSections].sort((a, b) => {
      if (a.type_ === SearchSectionType.Apps) return -1;
      if (b.type_ === SearchSectionType.Apps) return 1;
      if (a.type_ === SearchSectionType.Files) return -1;
      if (b.type_ === SearchSectionType.Files) return 1;
      return 0;
    });
  }, [searchQuery, searchSections]);

  return (
    <div className="h-screen flex flex-col overflow-hidden">
      <Header setSearchQuery={setSearchQuery} searchQuery={searchQuery} />
      <main className="flex-1 px-2 pt-4 overflow-auto scrollbar">
        {searchQuery.trim() === "" ? (
          recents.length > 0 ? (
            <Recents
              recents={recents}
              handleResultSelect={handleResultSelect}
            />
          ) : (
            // Show empty state if no recents
            <div className="flex h-full items-center justify-center">
              <EmptyState />
            </div>
          )
        ) : filteredResults.length === 0 ? (
          // Show empty state if searching but no results
          <div className="flex h-full items-center justify-center">
            <EmptyState />
          </div>
        ) : (
          // Show search results
          filteredResults.map((section, sectionIndex) => (
            <div
              key={sectionIndex}
              className={`${sectionIndex > 0 ? "mt-6" : ""}`}
            >
              <h2 className="text-xs font-semibold text-muted-foreground mb-2">
                {section.title}
              </h2>
              <SearchResults
                section={section}
                selectedItem={
                  sectionIndex === selectedSection ? selectedItem : -1
                }
                onSelect={(item, index) => {
                  setSelectedSection(sectionIndex);
                  setSelectedItem(index);
                  handleResultSelect(item);
                }}
                searchQuery={searchQuery}
                resourceData={resourceData}
              />
            </div>
          ))
        )}
      </main>
      <div className="sticky bottom-0">
        <Footer setIsSettingsOpen={setIsSettingsOpen} />
      </div>
      {/* <FolderSettings
          selectedCategories={selectedCategories}
          toggleCategory={toggleCategory}
          searchCategories={searchCategories}
          isIndexing={isIndexing}
          indexingProgress={indexingProgress}
          handleSelectPaths={handleSelectPaths}
          setIsIndexing={setIsIndexing}
          isSettingsOpen={isSettingsOpen}
          setIsSettingsOpen={setIsSettingsOpen}
          setIndexingProgress={setIndexingProgress}
        /> */}
    </div>
  );
}

interface SearchResultsProps {
  section: SearchSection;
  selectedItem: number;
  onSelect: (item: SearchItem, index: number) => void;
  searchQuery: string;
  resourceData?: Record<number, { cpu_usage: number; memory_mb: number }>;
}

function SearchResults(props: SearchResultsProps) {
  const {
    section,
    selectedItem,
    onSelect,
    searchQuery,
    resourceData = {},
  } = props;
  const [copiedId, setCopiedId] = useState<number | null>(null);

  const handleCopy = async (path: string, id: number) => {
    try {
      await navigator.clipboard.writeText(path);
      setCopiedId(id);
      setTimeout(() => setCopiedId(null), 2000);
    } catch (err) {
      toast.error("Failed to copy path");
    }
  };

  // Apply real-time resource data to the app
  const getUpdatedApp = (app: AppMetadata): AppMetadata => {
    // Use resource data from real-time updates if available
    if (app.pid && resourceData[app.pid]) {
      return {
        ...app,
        resource_usage: {
          pid: app.pid,
          cpu_usage: resourceData[app.pid].cpu_usage,
          memory_mb: resourceData[app.pid].memory_mb,
          memory_bytes: resourceData[app.pid].memory_mb * 1024 * 1024,
        },
      };
    }

    // Otherwise use the resource data that came with the app (if any)
    return app;
  };

  const sortedItems = useMemo(() => {
    if (section.type_ === SearchSectionType.Apps) {
      return [...section.items].sort((a, b) => {
        const appA = a as AppMetadata;
        const appB = b as AppMetadata;

        // Sort running apps first
        if (appA.pid !== appB.pid) {
          return appA.pid ? -1 : 1;
        }

        // If both are running or both are not running, sort alphabetically
        return appA.name.localeCompare(appB.name);
      });
    }
    return section.items;
  }, [section, resourceData]); // Include resourceData in dependencies

  // console.log("section", section);
  // console.log("sortedItems", sortedItems);

  return (
    <div className="flex flex-col">
      {sortedItems
        .filter((app) => app.name.toLowerCase().includes(searchQuery))
        .map((item, index) => {
          return (
            <div
              key={item.id || index}
              className={`flex items-center justify-between cursor-pointer hover:bg-muted p-2 rounded-md group ${
                selectedItem === index ? "bg-muted" : ""
              }`}
              onClick={() => onSelect(item, index)}
            >
              {(() => {
                switch (section.type_) {
                  case SearchSectionType.Apps:
                    return <AppRow app={getUpdatedApp(item as AppMetadata)} />;
                  // case SearchSectionType.Files:
                  //   return (
                  //     <FileRow
                  //       file={item as FileMetadata}
                  //       handleCopy={handleCopy}
                  //       copiedId={copiedId}
                  //     />
                  //   );
                  // case SearchSectionType.Semantic:
                  //   return (
                  //     <SemanticRow
                  //       file={item as SemanticMetadata}
                  //       handleCopy={handleCopy}
                  //       copiedId={copiedId}
                  //     />
                  //   );
                }
              })()}
            </div>
          );
        })}
    </div>
  );
}

interface AppRowProps {
  app: AppMetadata;
}

function AppRow(props: AppRowProps) {
  const { app } = props;

  const memoryUsage = app.resource_usage?.memory_mb;
  const cpuUsage = app.resource_usage?.cpu_usage;

  return (
    <div className="flex items-center gap-2 min-w-0 flex-1">
      <div className="flex flex-col min-w-0 flex-1">
        <div className="flex flex-row items-center gap-1">
          {app?.icon ? (
            <img
              src={app.icon}
              className="w-4 h-4 object-contain"
              alt={app.name}
            />
          ) : (
            <Package className="h-4 w-4" />
          )}
          <span className="text-sm text-primary-foreground">{app?.name}</span>
          {app?.pid && (
            <div className="relative flex items-center justify-center">
              <div className="absolute w-2 h-2 bg-green-500/30 rounded-full animate-ping" />
              <div className="relative w-[6px] h-[6px] bg-green-500 rounded-full shadow-lg shadow-green-500/50" />
            </div>
          )}
          {app?.pid && memoryUsage !== undefined && (
            <span className="text-xs text-gray-500 ml-2">
              <div className="flex flex-row items-center gap-1">
                <MemoryStick className="w-3 h-3" />
                {typeof memoryUsage === "number"
                  ? memoryUsage.toFixed(1)
                  : memoryUsage}{" "}
                MB
              </div>
            </span>
          )}
          {app?.pid && cpuUsage !== undefined && (
            <span className="text-xs text-gray-500 ml-2">
              <div className="flex flex-row items-center gap-1">
                <Cpu className="w-3 h-3" />
                {typeof cpuUsage === "number" ? cpuUsage.toFixed(1) : cpuUsage}%
                CPU
              </div>
            </span>
          )}
        </div>
      </div>
    </div>
  );
}

interface FileRowProps {
  file: Extract<SearchItem, { type: SearchSectionType.Files }>;
  handleCopy: (path: string, id: number) => Promise<void>;
  copiedId: number | null;
}

function FileRow(props: FileRowProps) {
  const { file, handleCopy, copiedId } = props;

  return (
    <div className="flex flex-col w-full flex-1 gap-3">
      <div className="flex flex-row justify-between w-full items-center gap-1">
        <div className="flex flex-row w-full items-center gap-1">
          {getFileIcon(file.path)}
          <span className="text-sm text-primary-foreground">{file.name}</span>
          <button
            onClick={(e) => {
              e.stopPropagation();
              // handleCopy(file.path, file.id);
            }}
            className={`opacity-0 group-hover:opacity-100 ml-2 p-1 hover:bg-background rounded transition-opacity duration-200 ${
              copiedId === file.id ? "text-green-500" : "text-muted-foreground"
            }`}
          >
            {copiedId === file.id ? (
              <Check className="h-3 w-3" />
            ) : (
              <Copy className="h-3 w-3" />
            )}
          </button>
        </div>
        <span className="text-xs text-muted-foreground whitespace-nowrap">
          {getCategoryFromExtension(file.extension)}
        </span>
      </div>
      <div className="flex justify-between items-center gap-2 w-full h-0">
        <span className="text-xs text-muted-foreground whitespace-nowrap overflow-hidden text-ellipsis pl-4 flex-1">
          {truncatePath(file.path)}
        </span>
        <span className="text-xs text-muted-foreground whitespace-nowrap">
          {FormatFileSize(file.size)}
        </span>
      </div>
    </div>
  );
}

interface SemanticRowProps {
  file: Extract<SearchItem, { type: SearchSectionType.Semantic }>;
  handleCopy: (path: string, id: number) => Promise<void>;
  copiedId: number | null;
}

function SemanticRow(props: SemanticRowProps) {
  const { file, handleCopy, copiedId } = props;

  return (
    <div className="flex items-center gap-2 min-w-0 flex-1">
      <div className="flex flex-col min-w-0 flex-1">
        <div className="flex flex-row items-center gap-1">
          {getFileIcon(file.path)}
          <span className="text-sm text-primary-foreground">{file.name}</span>
          <span className="pl-2">{Math.floor(file.distance * 100)}%</span>
          {
            <button
              onClick={(e) => {
                e.stopPropagation();
                // handleCopy(file.path, file.id);
              }}
              className={`opacity-0 group-hover:opacity-100 ml-2 p-1 hover:bg-background rounded transition-opacity duration-200 ${
                copiedId === file.id
                  ? "text-green-500"
                  : "text-muted-foreground"
              }`}
            >
              {copiedId === file.id ? (
                <Check className="h-3 w-3" />
              ) : (
                <Copy className="h-3 w-3" />
              )}
            </button>
          }
        </div>
        <div className="flex items-center gap-2 min-w-0 h-0 group-hover:h-auto overflow-hidden transition-all duration-200">
          {
            <span className="text-xs text-muted-foreground whitespace-nowrap overflow-hidden text-ellipsis pl-5 flex-1">
              {truncatePath(file.path)}
            </span>
          }
          {
            <span className="text-xs text-muted-foreground whitespace-nowrap">
              {getCategoryFromExtension(file.extension)}
            </span>
          }
        </div>
      </div>
    </div>
  );
}

interface RecentsProps {
  recents: FileMetadata[];
  handleResultSelect: (
    item: SearchItem,
    type: SearchSectionType
  ) => Promise<void>;
}

function Recents(props: RecentsProps) {
  const { recents, handleResultSelect } = props;

  return (
    <div>
      <h2 className="text-xs font-semibold text-muted-foreground mb-2">
        Recent Files
      </h2>
      <div className="flex flex-col">
        {recents.map((file, index) => (
          <div
            key={index}
            className="flex items-center cursor-pointer hover:bg-muted p-2 rounded-md group"
            onClick={() => handleResultSelect(file, SearchSectionType.Files)}
          >
            <div className="flex items-center gap-2 min-w-0 flex-1">
              <div className="flex flex-col min-w-0 flex-1">
                <div className="flex flex-row items-center gap-1">
                  {getFileIcon(file.path)}
                  <span className="text-sm">{file.name}</span>
                </div>
                <div className="flex items-center gap-2 min-w-0 h-0 group-hover:h-auto overflow-hidden transition-all duration-200">
                  <span className="text-xs text-muted-foreground whitespace-nowrap overflow-hidden text-ellipsis pl-5 flex-1">
                    {truncatePath(file.path)}
                  </span>
                  <span className="text-xs text-muted-foreground whitespace-nowrap">
                    {getCategoryFromExtension(file.extension)}
                  </span>
                </div>
              </div>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

function EmptyState() {
  return (
    <div className="flex flex-col items-center justify-center p-8 space-y-2 text-primary-foreground/40 rounded-xl">
      <div className="bg-gray-200 dark:bg-muted rounded-full p-4">
        <Search className="text-primary-foreground/80" />
      </div>
      <h2 className="mb-2 text-xs font-semibold text-primary-foreground">
        No files found
      </h2>
      <p>Try searching for something else</p>
    </div>
  );
}

function getFileIcon(filePath: string) {
  const extension =
    filePath.split(".").length > 1
      ? `.${filePath.split(".").pop()?.toLowerCase()}`
      : "";

  let icon;
  switch (extension) {
    case ".app":
    case ".exe":
    case ".dmg":
      icon = <Package className="h-3 w-3" />;
      break;
    case ".pdf":
      icon = <FaRegFilePdf className="h-3 w-3" />;
      break;
    case ".doc":
    case ".docx":
    case ".txt":
    case ".rtf":
      icon = <FileText className="h-3 w-3" />;
      break;
    case ".jpg":
    case ".jpeg":
    case ".png":
    case ".gif":
    case ".svg":
    case ".webp":
      icon = <Image className="h-3 w-3" />;
      break;
    case ".js":
    case ".ts":
    case ".jsx":
    case ".tsx":
    case ".py":
    case ".java":
    case ".cpp":
    case ".html":
    case ".css":
      icon = <FileCode className="h-3 w-3" />;
      break;
    case ".mp4":
    case ".mov":
    case ".avi":
    case ".mkv":
      icon = <Film className="h-3 w-3" />;
      break;
    case ".mp3":
    case ".wav":
    case ".flac":
    case ".m4a":
      icon = <Music className="h-3 w-3" />;
      break;
    case ".json":
    case ".xml":
    case ".yaml":
    case ".yml":
      icon = <Database className="h-3 w-3" />;
      break;
    case ".xlsx":
    case ".xls":
    case ".csv":
      icon = <FileSpreadsheet className="h-3 w-3" />;
      break;
    case ".zip":
    case ".rar":
    case ".7z":
    case ".tar":
    case ".gz":
      icon = <FileArchive className="h-3 w-3" />;
      break;
    default:
      icon = <File className="h-3 w-3" />;
  }

  return icon;
}
