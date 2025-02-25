import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
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
} from "lucide-react";

import { FaRegFilePdf } from "react-icons/fa";

export default function App() {
  const [searchQuery, setSearchQuery] = useState<string>("");
  const [allApps, setAllApps] = useState<AppMetadata[]>([]);

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
  const [updatedApps, setUpdatedApps] = useState<AppMetadata[]>([]);
  const [recents, setRecents] = useState<FileMetadata[]>([]);
  const [isSearchActive, setIsSearchActive] = useState(false);

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
  //     toast.error("Error indexing selected paths");
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

  // useEffect(() => {
  //   const handleKeyDown = (e: KeyboardEvent) => {
  //     if (searchSections.length === 0) return;

  //     if (e.key === "ArrowDown") {
  //       e.preventDefault();
  //       const currentSection = searchSections[selectedSection];
  //       if (selectedItem < currentSection.items.length - 1) {
  //         setSelectedItem(selectedItem + 1);
  //       } else if (selectedSection < searchSections.length - 1) {
  //         setSelectedSection(selectedSection + 1);
  //         setSelectedItem(0);
  //       }
  //     } else if (e.key === "ArrowUp") {
  //       e.preventDefault();
  //       if (selectedItem > 0) {
  //         setSelectedItem(selectedItem - 1);
  //       } else if (selectedSection > 0) {
  //         setSelectedSection(selectedSection - 1);
  //         setSelectedItem(searchSections[selectedSection - 1].items.length - 1);
  //       }
  //     } else if (e.key === "Enter") {
  //       e.preventDefault();
  //       const section = searchSections[selectedSection];
  //       const item = section?.items[selectedItem];
  //       if (item) {
  //         handleResultSelect(item, section.type);
  //       }
  //     }
  //   };

  //   document.addEventListener("keydown", handleKeyDown);
  //   return () => document.removeEventListener("keydown", handleKeyDown);
  // }, [searchSections, selectedSection, selectedItem]);

  async function handleResultSelect(app: SearchItem) {
    async () =>
      await invoke<AppMetadata[]>("launch_or_switch_to_application", {
        app: app,
      });
  }

  // sort sections with apps first
  const sortedSections = useMemo(() => {
    return [...searchSections].sort((a, b) => {
      if (a.type === SearchSectionType.Apps) return -1;
      if (b.type === SearchSectionType.Apps) return 1;
      if (a.type === SearchSectionType.Files) return -1;
      if (b.type === SearchSectionType.Files) return 1;
      return 0;
    });
  }, [searchSections]);

  // Gets all of the apps
  useEffect(() => {
    const fetchAllApps = async () => {
      try {
        const apps = await invoke<SearchSection[]>("get_search_data");
        setSearchSections(apps);
      } catch (error) {
        console.error("Failed to fetch apps:", error);
      }
    };

    fetchAllApps();
  }, []);

  // Filter apps on client side when search query changes
  // const filteredApps = useMemo(() => {
  //   if (!searchQuery.trim()) {
  //     return allApps;
  //   }

  //   const query = searchQuery.toLowerCase();
  //   return allApps.filter((app) => app.name.toLowerCase().includes(query));
  // }, [searchQuery, allApps]);

  console.log("sorted", searchSections);

  return (
    <main className="container">
      <div className="h-screen flex flex-col overflow-hidden">
        <Header setSearchQuery={setSearchQuery} searchQuery={searchQuery} />
        {/* <div className="overflow-auto">
          <h1>Running Applications</h1>
          <ul>
            {filteredApps.map((app) => (
              <li key={app.name}>
                <Button
                  key={app.path}
                  onClick={async () =>
                    await invoke<AppMetadata[]>(
                      "launch_or_switch_to_application",
                      { app: app }
                    )
                  }
                >
                  <div className="flex flex-row items-center">
                    <img
                      src={app.icon}
                      alt={`${app.name} icon`}
                      className="w-8 h-8"
                    />
                    {app.name}
                  </div>
                </Button>
              </li>
              // <li key={app.path}>{app.name}</li>
            ))}
          </ul>
        </div> */}
        <div>
          {sortedSections.map((section, sectionIndex) => (
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
                updatedApps={updatedApps}
              />
            </div>
          ))}
        </div>

        {/* <main className="flex-1 px-2 pt-4 overflow-auto scrollbar">
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
          ) : searchSections.length === 0 ? (
            // Show empty state if searching but no results
            <div className="flex h-full items-center justify-center">
              <EmptyState />
            </div>
          ) : (
            // Show search results
            sortedSections.map((section, sectionIndex) => (
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
                    handleResultSelect(item, section.type);
                  }}
                  updatedApps={updatedApps}
                />
              </div>
            ))
          )}
        </main> */}
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
    </main>
  );
}

interface SearchResultsProps {
  section: SearchSection;
  selectedItem: number;
  onSelect: (item: SearchItem, index: number) => void;
  updatedApps: AppMetadata[];
}

function SearchResults(props: SearchResultsProps) {
  const { section, selectedItem, onSelect, updatedApps } = props;
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

  const getUpdatedApp = (app: AppMetadata): AppMetadata => {
    const updated = updatedApps.find(
      (u) => u.name.toLowerCase() === app.name.toLowerCase()
    );
    return updated
      ? { ...app, memoryUsage: updated.memoryUsage, cpuUsage: updated.cpuUsage }
      : app;
  };

  const sortedItems = useMemo(() => {
    if (section.type === SearchSectionType.Apps) {
      return [...section.items].sort((a, b) => {
        const appA = a as AppMetadata;
        const appB = b as AppMetadata;

        // Sort running apps first
        if (appA.isRunning !== appB.isRunning) {
          return appA.isRunning ? -1 : 1;
        }

        // If both are running or both are not running, sort alphabetically
        return appA.name.localeCompare(appB.name);
      });
    }
    return section.items;
  }, [section]);

  return (
    <div className="flex flex-col">
      {sortedItems.map((item, index) => {
        return (
          <div
            key={item.id || index}
            className={`flex items-center justify-between cursor-pointer hover:bg-muted p-2 rounded-md group ${
              selectedItem === index ? "bg-muted" : ""
            }`}
            onClick={() => onSelect(item, index)}
          >
            {(() => {
              switch (section.type) {
                case SearchSectionType.Apps:
                  return <AppRow app={getUpdatedApp(item as AppMetadata)} />;
                case SearchSectionType.Files:
                  return (
                    <FileRow
                      file={item as FileMetadata}
                      handleCopy={handleCopy}
                      copiedId={copiedId}
                    />
                  );
                case SearchSectionType.Semantic:
                  return (
                    <SemanticRow
                      file={item as SemanticMetadata}
                      handleCopy={handleCopy}
                      copiedId={copiedId}
                    />
                  );
              }
            })()}
          </div>
        );
      })}
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

interface AppRowProps {
  app: Extract<SearchItem, { type: SearchSectionType.Apps }>;
}

function AppRow(props: AppRowProps) {
  const { app } = props;

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
          {app?.isRunning && (
            <div className="relative">
              <div className="absolute inset-0 w-2 h-2 bg-green-500/30 rounded-full animate-ping" />
              <div className="relative w-2 h-2 bg-green-500 rounded-full shadow-lg shadow-green-500/50" />
            </div>
          )}
          {app?.isRunning && (
            <span className="text-xs text-gray-500 ml-2">
              {app.memoryUsage !== undefined ? (
                <div className="flex flex-row items-center gap-1">
                  <MemoryStick className="w-3 h-3" />
                  {app.memoryUsage.toFixed(1)} MB
                </div>
              ) : (
                "—"
              )}
            </span>
          )}
          {app?.isRunning && app?.cpuUsage !== undefined && (
            <span className="text-xs text-gray-500 ml-2">
              <div className="flex flex-row items-center gap-1">
                <Cpu className="w-3 h-3" />
                {app.cpuUsage.toFixed(1)}% CPU
              </div>
            </span>
          )}
        </div>
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
