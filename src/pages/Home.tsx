import { useState, useEffect, useRef } from "react";
import FolderSettings from "../../components/FolderSettings";
import {
  Folder,
  FileText,
  Image,
  FileCode,
  Film,
  Music,
  File,
  Database,
  Package,
  FileArchive,
  FileSpreadsheet,
  ArrowUpDown,
  CornerDownLeft,
  Copy,
  Circle,
  Files,
  Check,
  MemoryStick,
  Cpu,
} from "lucide-react";
import {
  SearchCategory,
  FileMetadata,
  SearchSection,
  AppMetadata,
  SemanticMetadata,
  SearchSectionType,
  SearchItem,
} from "../../src/types/index";
import { ThemeToggle } from "../../src/ThemeProvider";
import { FaRegFilePdf } from "react-icons/fa";
import { Button } from "../../components/ui/button";
import WindowAction from "../../components/WindowActions";
import { toast } from "sonner";
import { Input } from "../../components/ui/input";
import { getCategoryFromExtension, truncatePath } from "../../src/lib/utils";

export const searchCategories = [
  "Applications",
  "Documents",
  "Folders",
  "Images",
  "Mail",
  "Messages",
  "Other",
  "PDF Documents",
  "Spreadsheets",
] as const;

export interface IndexingProgress {
  total: number;
  processed: number;
  percentage: number;
}

export default function Home() {
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
  const [updatedApps, setUpdatedApps] = useState<AppMetadata[]>([]);
  const [recents, setRecents] = useState<FileMetadata[]>([]);

  useEffect(() => {
    const handleProgress = (_: any, progress: IndexingProgress) => {
      setIndexingProgress(progress);
    };

    window.electron.onIndexingProgress(handleProgress);
    return () => {
      window.electron.removeIndexingProgress(handleProgress);
    };
  }, []);

  useEffect(() => {
    const handler = (event: any, apps: AppMetadata[]) => {
      setUpdatedApps(apps);
    };

    window.electron.onResourceUsageUpdated(handler);
    return () => {
      window.electron.removeResourceUsageUpdated(handler);
    };
  }, []);

  // retrieves the recents
  useEffect(() => {
    const fetchRecents = async () => {
      try {
        const recentFiles = await window.electron.getRecents();
        setRecents(recentFiles);
      } catch (error) {
        console.error("Error fetching recents:", error);
      }
    };
    fetchRecents();

    const interval = setInterval(fetchRecents, 60000);
    return () => clearInterval(interval);
  }, []);

  const handleSearch = async (query: string) => {
    setSearchQuery(query);

    if (!query.trim()) {
      setSearchSections([]);
      return;
    }

    try {
      const sections = await window.electron.searchFilesAndEmbeddings(query);
      setSearchSections(sections);
      setSelectedSection(0);
      setSelectedItem(0);
    } catch (error) {
      toast.error("Error searching:", error);
    }
  };

  const handleSelectPaths = async () => {
    try {
      const result = await window.electron.selectPaths({
        properties: ["openFile", "openDirectory", "multiSelections"],
      });

      if (result.canceled || !result.filePaths.length) return;

      setIsIndexing(true);
      setIndexingProgress(null);

      // Index all selected paths
      await window.electron.indexAndEmbedPaths(result.filePaths);

      setIsIndexing(false);
      setIndexingProgress(null);
    } catch (error) {
      console.error("Error indexing paths:", error);
      setIsIndexing(false);
      setIndexingProgress(null);
      toast.error("Error indexing selected paths");
    }
  };

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
          handleResultSelect(item, section.type);
        }
      }
    };

    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [searchSections, selectedSection, selectedItem]);

  const handleResultSelect = async (
    item: SearchItem,
    type: SearchSectionType
  ) => {
    try {
      if (type === SearchSectionType.Apps) {
        await window.electron.launchOrSwitch(item as AppMetadata);
      } else {
        await window.electron.openFile((item as FileMetadata).path);
      }
    } catch (error) {
      toast.error("Error opening item");
    }
  };

  console.log("search sections", searchSections);

  return (
    <div className="h-screen flex flex-col overflow-hidden">
      <Header handleSearch={handleSearch} searchQuery={searchQuery} />
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
        ) : searchSections.length === 0 ? (
          // Show empty state if searching but no results
          <div className="flex h-full items-center justify-center">
            <EmptyState />
          </div>
        ) : (
          // Show search results
          searchSections.map((section, sectionIndex) => (
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
      </main>
      <div className="sticky bottom-0">
        <Footer setIsSettingsOpen={setIsSettingsOpen} />
      </div>
      <FolderSettings
        selectedCategories={selectedCategories}
        toggleCategory={toggleCategory}
        searchCategories={searchCategories}
        isIndexing={isIndexing}
        indexingProgress={indexingProgress}
        handleSelectPaths={handleSelectPaths}
        isSettingsOpen={isSettingsOpen}
        setIsSettingsOpen={setIsSettingsOpen}
      />
    </div>
  );
}

interface HeaderProps {
  searchQuery: string;
  handleSearch: (query: string) => Promise<void>;
}

function Header(props: HeaderProps) {
  const { searchQuery, handleSearch } = props;
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  return (
    <div className="sticky top-0 bg-background flex flex-col gap-2 border-b border-b-border">
      <div className="flex flex-row w-full items-center select-none dragable px-3 mt-2">
        <div className="flex-none">
          <WindowAction />
        </div>
        <div className="flex-1 flex justify-center pr-14">Kita</div>
      </div>
      <div className="py-2">
        <Input
          placeholder="Type a command or search..."
          value={searchQuery}
          ref={inputRef}
          onChange={(e) => handleSearch(e.target.value)}
          className="text-xs placeholder:pl-2 border-0 focus-visible:outline-none focus-visible:ring-0 shadow-none"
        />
      </div>
    </div>
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

  console.log("section", section);

  return (
    <div className="flex flex-col">
      {section.items.map((item, index) => {
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
                  return (
                    <AppRow
                      app={getUpdatedApp(item as AppMetadata)}
                      handleCopy={handleCopy}
                      copiedId={copiedId}
                    />
                  );
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
    <div className="flex items-center gap-2 min-w-0 flex-1">
      <div className="flex flex-col min-w-0 flex-1">
        <div className="flex flex-row items-center gap-1">
          {getFileIcon(file.path)}
          <span className="text-sm text-primary-foreground">{file.name}</span>
          {
            <button
              onClick={(e) => {
                e.stopPropagation();
                handleCopy(file.path, file.id);
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

interface AppRowProps {
  app: Extract<SearchItem, { type: SearchSectionType.Apps }>;
  handleCopy: (path: string, id: number) => Promise<void>;
  copiedId: number | null;
}

function AppRow(props: AppRowProps) {
  const { app, handleCopy, copiedId } = props;

  return (
    <div className="flex items-center gap-2 min-w-0 flex-1">
      <div className="flex flex-col min-w-0 flex-1">
        <div className="flex flex-row items-center gap-1">
          {app?.iconDataUrl ? (
            <img
              src={app.iconDataUrl}
              className="w-4 h-4 object-contain"
              alt={app.name}
            />
          ) : (
            <Package className="h-4 w-4" />
          )}
          <span className="text-sm text-primary-foreground">{app?.name}</span>
          {app?.isRunning && (
            <Circle className="bg-green-500 border-0 rounded-full w-2 h-2" />
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
          {
            <button
              onClick={(e) => {
                e.stopPropagation();
                handleCopy(app.path, app.id);
              }}
              className={`opacity-0 group-hover:opacity-100 ml-2 p-1 hover:bg-background rounded transition-opacity duration-200 ${
                copiedId === app.id ? "text-green-500" : "text-muted-foreground"
              }`}
            >
              {copiedId === app.id ? (
                <Check className="h-3 w-3" />
              ) : (
                <Copy className="h-3 w-3" />
              )}
            </button>
          }
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
                handleCopy(file.path, file.id);
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
interface FooterProps {
  setIsSettingsOpen: (val: boolean) => void;
}

function Footer(props: FooterProps) {
  const { setIsSettingsOpen } = props;
  return (
    <div className="h-8 bg-background flex justify-between items-center px-3 border-t border-t-border">
      <div className="flex flex-row items-center gap-4 text-primary-foreground/60">
        <div className="flex flex-row items-center gap-1 text-xs">
          <ArrowUpDown className="w-3 h-3 " /> <div>Select</div>
        </div>
        <div className="flex flex-row items-center gap-1 text-xs">
          <CornerDownLeft className="w-3 h-3 " /> <div>Open</div>
        </div>
      </div>
      <div className="flex flex-row items-center gap-2">
        <Button
          variant="titleBar"
          onClick={() => setIsSettingsOpen(true)}
          size="sm"
          className="group flex flex-row items-center gap-1 z-10"
        >
          <Folder className="h-4 w-4" />
        </Button>
        <ThemeToggle />
      </div>
    </div>
  );
}

function EmptyState() {
  return (
    <div className="flex flex-col items-center justify-center p-8 space-y-2 border border-border border-dashed text-primary-foreground/40 rounded-xl">
      <Files />
      <h2 className="mb-2 text-xs">No files found</h2>
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
