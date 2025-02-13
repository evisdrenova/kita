import { useState, useEffect } from "react";
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
  Table,
  Check,
  MemoryStick,
  Cpu,
} from "lucide-react";
import {
  SearchResult,
  SearchCategory,
  FileMetadata,
  SearchSection,
  AppInfo,
} from "../../src/types/index";
import { ThemeToggle } from "../../src/ThemeProvider";
import { FaRegFilePdf } from "react-icons/fa";
import { Button } from "../../components/ui/button";
import WindowAction from "../../components/WindowActions";
import { toast } from "sonner";
import { Input } from "../../components/ui/input";

export const searchCategories = [
  "Applications",
  "Calculator",
  "Contacts",
  "Conversion",
  "Definition",
  "Developer",
  "Documents",
  "Events & Reminders",
  "Folders",
  "Fonts",
  "Images",
  "Mail & Messages",
  "Movies",
  "Music",
  "Other",
  "PDF Documents",
  "Presentations",
  "Siri Suggestions",
  "Spreadsheets",
  "System Settings",
  "Tips",
  "Websites",
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
  const [updatedApps, setUpdatedApps] = useState<AppInfo[]>([]);

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
    const handler = (event: any, apps: AppInfo[]) => {
      setUpdatedApps(apps);
    };

    window.electron.onResourceUsageUpdated(handler);
    return () => {
      window.electron.removeResourceUsageUpdated(handler);
    };
  }, []);

  const handleSearch = async (query: string) => {
    setSearchQuery(query);

    if (!query.trim()) {
      setSearchSections([]);
      return;
    }

    try {
      const sections = await window.electron.searchFiles(query);
      setSearchSections(sections);
      setSelectedSection(0);
      setSelectedItem(0);
    } catch (error) {
      toast.error("Error searching:", error);
    }
  };

  const handleSelectFolder = async () => {
    try {
      const result = await window.electron.selectDirectory();
      if (result.canceled || !result.filePaths.length) return;

      setIsIndexing(true);
      setIndexingProgress(null);

      const directory = result.filePaths[0];
      await window.electron.indexDirectories([directory]);

      setIsIndexing(false);
      setIndexingProgress(null);
    } catch (error) {
      console.error("Error indexing directory:", error);
      setIsIndexing(false);
      setIndexingProgress(null);
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
    item: FileMetadata | AppInfo,
    type: "apps" | "files"
  ) => {
    try {
      if (type === "apps") {
        await window.electron.launchOrSwitch(item as AppInfo);
      } else {
        await window.electron.openFile((item as FileMetadata).path);
      }
    } catch (error) {
      toast.error("Error opening item");
    }
  };

  return (
    <div className="h-screen flex flex-col overflow-hidden">
      <Header handleSearch={handleSearch} searchQuery={searchQuery} />
      {searchSections.length === 0 ? (
        <div className="flex h-full items-center justify-center">
          <EmptyState />
        </div>
      ) : (
        <main className="flex-1 px-2 pt-4 overflow-auto scrollbar">
          {searchSections.map((section, sectionIndex) => (
            <div
              key={section.type}
              className={`${sectionIndex > 0 ? "mt-6" : ""}`}
            >
              <h2 className="text-xs font-semibold text-muted-foreground mb-2">
                {section.title}
              </h2>
              <SearchResults
                items={section.items}
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
          ))}
        </main>
      )}
      <div className="sticky bottom-0">
        <Footer setIsSettingsOpen={setIsSettingsOpen} />
      </div>
      <FolderSettings
        selectedCategories={selectedCategories}
        toggleCategory={toggleCategory}
        searchCategories={searchCategories}
        isIndexing={isIndexing}
        indexingProgress={indexingProgress}
        handleSelectFolder={handleSelectFolder}
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
          onChange={(e) => handleSearch(e.target.value)}
          className="text-xs placeholder:pl-2 border-0 focus-visible:outline-none focus-visible:ring-0 "
        />
      </div>
    </div>
  );
}

interface SearchResultsProps {
  items: (FileMetadata | AppInfo)[];
  selectedItem: number;
  onSelect: (item: FileMetadata | AppInfo, index: number) => void;
  updatedApps: AppInfo[];
}

function truncatePath(path: string, maxLength: number = 50) {
  const parts = path.split("/");
  const fileName = parts.pop() || "";
  const directory = parts.join("/");

  if (path.length <= maxLength) return path;

  // Calculate how many characters we can show from start and end of the directory
  const dotsLength = 3;
  const maxDirLength = maxLength - fileName.length - dotsLength;
  const startLength = Math.floor(maxDirLength / 2);
  const endLength = Math.floor(maxDirLength / 2);

  const startPath = directory.slice(0, startLength);
  const endPath = directory.slice(-endLength);

  return `${startPath}...${endPath}/${fileName}`;
}

function SearchResults(props: SearchResultsProps) {
  const { items, selectedItem, onSelect, updatedApps } = props;
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

  const isAppInfo = (item: FileMetadata | AppInfo): item is AppInfo => {
    return "isRunning" in item;
  };

  console.log("items", items);

  const getUpdatedApp = (app: AppInfo): AppInfo => {
    const updated = updatedApps.find(
      (u) => u.name.toLowerCase() === app.name.toLowerCase()
    );
    return updated
      ? { ...app, memoryUsage: updated.memoryUsage, cpuUsage: updated.cpuUsage }
      : app;
  };

  return (
    <div className="flex flex-col">
      {items.map((item, index) => {
        const isApp = isAppInfo(item);
        const id = isApp ? index : (item as FileMetadata).id;
        const itemToRender = isApp ? getUpdatedApp(item as AppInfo) : item;

        return (
          <div
            key={id}
            className={`flex items-center justify-between cursor-pointer hover:bg-muted p-2 rounded-md group ${
              selectedItem === index ? "bg-muted" : ""
            }`}
            onClick={() => onSelect(item, index)}
          >
            <AppRow
              isApp={isAppInfo(item)}
              item={itemToRender}
              appPath={itemToRender.path}
              handleCopy={handleCopy}
              copiedId={copiedId}
              id={id}
            />
          </div>
        );
      })}
    </div>
  );
}

interface AppRowProps {
  item: AppInfo | FileMetadata;
  isApp: boolean;
  appPath: string;
  handleCopy: (path: string, id: number) => Promise<void>;
  copiedId: number;
  id: number;
}

function AppRow(props: AppRowProps) {
  const { item, isApp, appPath, handleCopy, copiedId, id } = props;

  // narrow the types since item is a union type
  const getAppInfo = (item: FileMetadata | AppInfo): AppInfo | null => {
    return isApp ? (item as AppInfo) : null;
  };

  const getFileInfo = (item: FileMetadata | AppInfo): FileMetadata | null => {
    return !isApp ? (item as FileMetadata) : null;
  };

  const appInfo = getAppInfo(item);
  const fileInfo = getFileInfo(item);

  return (
    <div className="flex items-center gap-2 min-w-0 flex-1">
      <div className="flex flex-col min-w-0 flex-1">
        <div className="flex flex-row items-center gap-1">
          {isApp ? (
            appInfo?.iconDataUrl ? (
              <img
                src={appInfo.iconDataUrl}
                className="w-4 h-4 object-contain"
                alt={appInfo.name}
              />
            ) : (
              <Package className="h-4 w-4" />
            )
          ) : (
            getFileIcon(appPath)
          )}
          <span className="text-sm">
            {isApp ? appInfo?.name : fileInfo?.name}
          </span>
          {isApp && appInfo?.isRunning && (
            <Circle className="bg-green-500 border-0 rounded-full w-2 h-2" />
          )}
          {isApp && appInfo?.isRunning && (
            <span className="text-xs text-gray-500 ml-2">
              {appInfo.memoryUsage !== undefined ? (
                <div className="flex flex-row items-center gap-1">
                  <MemoryStick className="w-3 h-3" />
                  {appInfo.memoryUsage.toFixed(1)} MB
                </div>
              ) : (
                "—"
              )}
            </span>
          )}
          {isApp && appInfo?.isRunning && appInfo?.cpuUsage !== undefined && (
            <span className="text-xs text-gray-500 ml-2">
              <div className="flex flex-row items-center gap-1">
                <Cpu className="w-3 h-3" />
                {appInfo.cpuUsage.toFixed(1)}% CPU
              </div>
            </span>
          )}
          {!isApp && (
            <button
              onClick={(e) => {
                e.stopPropagation();
                handleCopy(appPath, id);
              }}
              className={`opacity-0 group-hover:opacity-100 ml-2 p-1 hover:bg-background rounded transition-opacity duration-200 ${
                copiedId === id ? "text-green-500" : "text-muted-foreground"
              }`}
            >
              {copiedId === id ? (
                <Check className="h-3 w-3" />
              ) : (
                <Copy className="h-3 w-3" />
              )}
            </button>
          )}
        </div>
        <div className="flex items-center gap-2 min-w-0 h-0 group-hover:h-auto overflow-hidden transition-all duration-200">
          {!isApp && (
            <span className="text-xs text-muted-foreground whitespace-nowrap overflow-hidden text-ellipsis pl-5 flex-1">
              {truncatePath(appPath)}
            </span>
          )}
          {!isApp && (
            <span className="text-xs text-muted-foreground whitespace-nowrap">
              {getCategoryFromExtension((item as FileMetadata).extension)}
            </span>
          )}
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

function getCategoryFromExtension(extension: string): SearchCategory {
  switch (extension.toLowerCase()) {
    case ".app":
      return "Applications";
    case ".pdf":
      return "PDF Documents";
    case ".doc":
    case ".docx":
    case ".txt":
      return "Documents";
    case ".jpg":
    case ".jpeg":
    case ".png":
    case ".gif":
      return "Images";
    default:
      return "Other";
  }
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
