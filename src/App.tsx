import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import "./globals.css";
import Footer from "./Footer";
import {
  AppMetadata,
  FileMetadata,
  IndexingProgress,
  searchCategories,
  SearchCategory,
  SearchItem,
  SearchSection,
  SearchSectionType,
} from "./types/types";
import Header from "./Header";
import {
  FormatFileSize,
  getCategoryFromExtension,
  truncatePath,
} from "./lib/utils";
import {
  Cpu,
  Database,
  File,
  FileArchive,
  FileCode,
  FileSpreadsheet,
  FileText,
  Film,
  Image,
  Loader2,
  MemoryStick,
  Music,
  Package,
  RefreshCw,
  Search,
  X,
} from "lucide-react";
import { FaRegFilePdf } from "react-icons/fa";
import { errorToast, successToast } from "./components/ui/toast";
import { open } from "@tauri-apps/plugin-dialog";
import { appDataDir, documentDir } from "@tauri-apps/api/path";
import { join } from "@tauri-apps/api/path";
import Settings from "./Settings";
import { isSet } from "node:util/types";

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
  const [selectedSection, setSelectedSection] = useState<number>();
  const [selectedItem, setSelectedItem] = useState<number>(0);
  const [recents, setRecents] = useState<FileMetadata[]>([]);
  const [isSearchActive, setIsSearchActive] = useState(false);
  const [resourceData, setResourceData] = useState<
    Record<number, { cpu_usage: number; memory_bytes: number }>
  >({});
  const [showProgress, setShowProgress] = useState(false);

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
  //     errorToast("Errr indexing selected paths");
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

      let currentSection = selectedSection;
      if (currentSection === undefined) {
        const appsIndex = searchSections.findIndex(
          (sec) => sec.type_ === SearchSectionType.Apps
        );
        currentSection = appsIndex >= 0 ? appsIndex : 0;
      }

      if (e.key === "ArrowDown") {
        e.preventDefault();

        if (selectedSection === undefined) {
          setSelectedSection(currentSection);
          setSelectedItem(0);
          return;
        }

        const section = searchSections[currentSection];
        if (selectedItem < section.items.length - 1) {
          setSelectedItem(selectedItem + 1);
        } else if (currentSection < searchSections.length - 1) {
          setSelectedSection(currentSection + 1);
          setSelectedItem(0);
        }
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        if (selectedSection === undefined) {
          return;
        }

        if (selectedItem > 0) {
          setSelectedItem(selectedItem - 1);
        } else if (currentSection > 0) {
          setSelectedSection(currentSection - 1);
          setSelectedItem(searchSections[currentSection - 1].items.length - 1);
        }
      } else if (e.key === "Enter") {
        e.preventDefault();
        if (selectedSection === undefined) {
          return;
        }

        const section = searchSections[currentSection];
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
    await invoke<AppMetadata[]>("launch_or_switch_to_application", {
      app: app,
    });
  }

  const toggleCategory = (category: SearchCategory) => {
    setSelectedCategories((prev) => {
      const newSet = new Set(prev);
      if (newSet.has(category)) {
        newSet.delete(category);
      } else {
        newSet.add(category);
      }
      return newSet;
    });
  };

  const handleSelectPaths = async () => {
    setShowProgress(false);
    setIndexingProgress(null);
    try {
      const selected = await open({
        multiple: true,
        directory: true,
        title: "Select Files or Folders to Index",
        defaultPath: await documentDir(),
      });

      if (!selected || (Array.isArray(selected) && !selected.length)) return;

      const paths = Array.isArray(selected) ? selected : [selected];

      setIsIndexing(true);
      setShowProgress(true);
      setIndexingProgress(null);

      const unlistenProgress = await listen<IndexingProgress>(
        "file-processing-progress",
        (event) => {
          console.log("Received progress event:", event);
          const progress = event.payload;
          if (progress) {
            setIndexingProgress(progress);
          }
        }
      );

      await invoke("process_paths_command", { paths });

      unlistenProgress();

      setIsIndexing(false);
    } catch (error) {
      const err = error as Error;
      setIsIndexing(false);
      setShowProgress(false);
      setIndexingProgress(null);
      errorToast("Error indexing selected paths:", err.message);
    }
  };

  // reset the progres if the settings are closed
  useEffect(() => {
    setShowProgress(false);
    setIndexingProgress(null);
  }, [isSettingsOpen]);

  // listens for resource events and app update events
  useEffect(() => {
    let unlistenUsage: UnlistenFn | undefined;
    let unlistenApps: UnlistenFn | undefined;

    (async () => {
      try {
        const sections = await invoke<SearchSection[]>("get_apps_data");
        setSearchSections(sections);

        const appSection = sections.filter(
          (sec) => sec.type_ === SearchSectionType.Apps
        );

        const allAppItems = appSection.flatMap(
          (sec) => sec.items
        ) as AppMetadata[];

        const pids = allAppItems
          .filter((app) => app.pid != null)
          .map((app) => app.pid);

        await invoke("start_resource_monitoring", { pids });

        unlistenUsage = await listen("resource-usage-updated", (event) => {
          const updates = event.payload as Record<
            number,
            { cpu_usage: number; memory_bytes: number }
          >;

          setResourceData((prev) => {
            const newState = { ...prev };
            Object.entries(updates).forEach(([pidStr, usage]) => {
              const pidNum = Number(pidStr);
              newState[pidNum] = {
                cpu_usage: usage.cpu_usage,
                memory_bytes: usage.memory_bytes,
              };
            });
            return newState;
          });
        });

        unlistenApps = await listen("apps-with-resources-updated", (event) => {
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
        });
      } catch (err) {
        console.error("Failed to set up resource monitoring:", err);
      }
    })();

    return () => {
      if (unlistenUsage) unlistenUsage();
      if (unlistenApps) unlistenApps();

      invoke("stop_resource_monitoring").catch((err) => {
        console.error("Failed to stop resource monitoring:", err);
      });
    };
  }, []);

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

    // Sort the *sections* so that Apps always comes first, then Files, then others
    return [...filteredSections].sort((a, b) => {
      if (
        a.type_ === SearchSectionType.Apps &&
        b.type_ !== SearchSectionType.Apps
      ) {
        return -1; // 'a' before 'b'
      }
      if (
        b.type_ === SearchSectionType.Apps &&
        a.type_ !== SearchSectionType.Apps
      ) {
        return 1; // 'b' before 'a'
      }

      if (
        a.type_ === SearchSectionType.Files &&
        b.type_ !== SearchSectionType.Files
      ) {
        return -1;
      }
      if (
        b.type_ === SearchSectionType.Files &&
        a.type_ !== SearchSectionType.Files
      ) {
        return 1;
      }
      return 0;
    });
  }, [searchQuery, searchSections]);

  async function refreshApps() {
    try {
      const sections = await invoke<SearchSection[]>("get_apps_data");
      setSearchSections(sections);
    } catch (err) {
      console.error("Failed to refresh apps:", err);
    }
  }

  return (
    <div className="h-screen flex flex-col overflow-hidden">
      <Header setSearchQuery={setSearchQuery} searchQuery={searchQuery} />
      <main className="flex-1 px-2 pt-4 overflow-auto scrollbar">
        {/* {searchQuery.trim() === "" ? (
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
        ) : ( */}
        {
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
                refreshApps={refreshApps}
              />
            </div>
          ))
          // )}
        }
      </main>
      <div className="sticky bottom-0">
        <Footer setIsSettingsOpen={setIsSettingsOpen} />
      </div>
      <Settings
        selectedCategories={selectedCategories}
        toggleCategory={toggleCategory}
        searchCategories={searchCategories}
        isIndexing={isIndexing}
        indexingProgress={indexingProgress}
        handleSelectPaths={handleSelectPaths}
        isSettingsOpen={isSettingsOpen}
        setIsSettingsOpen={setIsSettingsOpen}
        showProgress={showProgress}
      />
    </div>
  );
}

interface SearchResultsProps {
  section: SearchSection;
  selectedItem: number;
  onSelect: (item: SearchItem, index: number) => void;
  searchQuery: string;
  resourceData?: Record<number, { cpu_usage: number; memory_bytes: number }>;
  refreshApps: () => Promise<void>;
}

function SearchResults(props: SearchResultsProps) {
  const {
    section,
    selectedItem,
    onSelect,
    searchQuery,
    resourceData = {},
    refreshApps,
  } = props;
  const [copiedId, setCopiedId] = useState<number | null>(null);

  const handleCopy = async (path: string, id: number) => {
    try {
      await navigator.clipboard.writeText(path);
      setCopiedId(id);
      setTimeout(() => setCopiedId(null), 2000);
    } catch (err) {
      errorToast("Failed to copy path");
    }
  };

  // Apply real-time resource data to the app
  const getUpdatedApp = (app: AppMetadata): AppMetadata => {
    if (app.pid && resourceData[app.pid]) {
      return {
        ...app,
        resource_usage: {
          pid: app.pid,
          cpu_usage: resourceData[app.pid].cpu_usage,
          memory_bytes: resourceData[app.pid].memory_bytes,
        },
      };
    }
    return app;
  };

  const sortedItems = useMemo(() => {
    if (section.type_ === SearchSectionType.Apps) {
      return [...section.items].sort((a, b) => {
        const appA = a as AppMetadata;
        const appB = b as AppMetadata;

        // Sort running apps first
        if (appA.pid && !appB.pid) return -1;
        if (!appA.pid && appB.pid) return 1;
        return appA.name.localeCompare(appB.name);
      });
    }
    return section.items;
  }, [section, resourceData]);

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
                    return (
                      <AppRow
                        app={getUpdatedApp(item as AppMetadata)}
                        refreshApps={refreshApps}
                      />
                    );
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
  refreshApps: () => Promise<void>;
}
function AppRow(props: AppRowProps) {
  const { app, refreshApps } = props;

  const [isKilling, setIsKilling] = useState<boolean>(false);
  const [isRestarting, setIsRestarting] = useState<boolean>(false);

  const memoryUsage = app.resource_usage?.memory_bytes;
  const cpuUsage = app.resource_usage?.cpu_usage;

  const handleForceQuit = async (e: React.MouseEvent) => {
    e.stopPropagation();
    if (!app.pid) return;
    console.log("killing");
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
  };

  const handleRestart = async (e: React.MouseEvent) => {
    e.stopPropagation();
    console.log("restarting");
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
  };

  return (
    <div className="flex items-center w-full gap-2">
      <div className="flex items-center flex-grow min-w-0 mr-2">
        {app?.icon ? (
          <img
            src={app.icon}
            className="w-4 h-4 object-contain mr-2"
            alt={app.name}
          />
        ) : (
          <Package className="h-4 w-4 mr-2" />
        )}
        <span className="text-sm text-primary-foreground truncate">
          {app?.name}
        </span>
        {app?.pid && (
          <div className="relative flex items-center justify-center ml-2">
            <div className="absolute w-2 h-2 bg-green-500/30 rounded-full animate-ping" />
            <div className="relative w-[6px] h-[6px] bg-green-500 rounded-full shadow-lg shadow-green-500/50" />
          </div>
        )}
      </div>
      <div className="w-28 flex-shrink-0 text-right">
        {app?.pid && memoryUsage !== undefined ? (
          <span className="text-xs text-gray-500">
            <div className="flex flex-row items-center justify-end gap-1">
              <MemoryStick className="w-3 h-3" />
              {typeof memoryUsage === "number"
                ? FormatFileSize(memoryUsage)
                : memoryUsage}
            </div>
          </span>
        ) : (
          <div></div>
        )}
      </div>
      <div className="w-24 flex-shrink-0 text-right ">
        {app?.pid && cpuUsage !== undefined ? (
          <span className="text-xs text-gray-500">
            <div className="flex flex-row items-center justify-end gap-1">
              <Cpu className="w-3 h-3" />
              {typeof cpuUsage === "number" ? cpuUsage.toFixed(1) : cpuUsage}%
            </div>
          </span>
        ) : (
          <div></div>
        )}
      </div>
      <div className="flex-shrink-0 flex items-center space-x-1 opacity-0 group-hover:opacity-100 transition-opacity">
        {app.pid && (
          <>
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
          </>
        )}
      </div>
    </div>
  );
}

// interface FileRowProps {
//   file: Extract<SearchItem, { type: SearchSectionType.Files }>;
//   handleCopy: (path: string, id: number) => Promise<void>;
//   copiedId: number | null;
// }

// function FileRow(props: FileRowProps) {
//   const { file, handleCopy, copiedId } = props;

//   return (
//     <div className="flex flex-col w-full flex-1 gap-3">
//       <div className="flex flex-row justify-between w-full items-center gap-1">
//         <div className="flex flex-row w-full items-center gap-1">
//           {getFileIcon(file.path)}
//           <span className="text-sm text-primary-foreground">{file.name}</span>
//           <button
//             onClick={(e) => {
//               e.stopPropagation();
//               // handleCopy(file.path, file.id);
//             }}
//             className={`opacity-0 group-hover:opacity-100 ml-2 p-1 hover:bg-background rounded transition-opacity duration-200 ${
//               copiedId === file.id ? "text-green-500" : "text-muted-foreground"
//             }`}
//           >
//             {copiedId === file.id ? (
//               <Check className="h-3 w-3" />
//             ) : (
//               <Copy className="h-3 w-3" />
//             )}
//           </button>
//         </div>
//         <span className="text-xs text-muted-foreground whitespace-nowrap">
//           {getCategoryFromExtension(file.extension)}
//         </span>
//       </div>
//       <div className="flex justify-between items-center gap-2 w-full h-0">
//         <span className="text-xs text-muted-foreground whitespace-nowrap overflow-hidden text-ellipsis pl-4 flex-1">
//           {truncatePath(file.path)}
//         </span>
//         <span className="text-xs text-muted-foreground whitespace-nowrap">
//           {FormatFileSize(file.size)}
//         </span>
//       </div>
//     </div>
//   );
// }

// interface SemanticRowProps {
//   file: Extract<SearchItem, { type: SearchSectionType.Semantic }>;
//   handleCopy: (path: string, id: number) => Promise<void>;
//   copiedId: number | null;
// }

// function SemanticRow(props: SemanticRowProps) {
//   const { file, handleCopy, copiedId } = props;

//   return (
//     <div className="flex items-center gap-2 min-w-0 flex-1">
//       <div className="flex flex-col min-w-0 flex-1">
//         <div className="flex flex-row items-center gap-1">
//           {getFileIcon(file.path)}
//           <span className="text-sm text-primary-foreground">{file.name}</span>
//           <span className="pl-2">{Math.floor(file.distance * 100)}%</span>
//           {
//             <button
//               onClick={(e) => {
//                 e.stopPropagation();
//                 // handleCopy(file.path, file.id);
//               }}
//               className={`opacity-0 group-hover:opacity-100 ml-2 p-1 hover:bg-background rounded transition-opacity duration-200 ${
//                 copiedId === file.id
//                   ? "text-green-500"
//                   : "text-muted-foreground"
//               }`}
//             >
//               {copiedId === file.id ? (
//                 <Check className="h-3 w-3" />
//               ) : (
//                 <Copy className="h-3 w-3" />
//               )}
//             </button>
//           }
//         </div>
//         <div className="flex items-center gap-2 min-w-0 h-0 group-hover:h-auto overflow-hidden transition-all duration-200">
//           {
//             <span className="text-xs text-muted-foreground whitespace-nowrap overflow-hidden text-ellipsis pl-5 flex-1">
//               {truncatePath(file.path)}
//             </span>
//           }
//           {
//             <span className="text-xs text-muted-foreground whitespace-nowrap">
//               {getCategoryFromExtension(file.extension)}
//             </span>
//           }
//         </div>
//       </div>
//     </div>
//   );
// }

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

async function getDbPath() {
  const appData = await appDataDir();
  return await join(appData, "kita-database.sqlite");
}
