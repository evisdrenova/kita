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
  Table,
  FileSpreadsheet,
} from "lucide-react";
import {
  SearchResult,
  SearchCategory,
  FileMetadata,
} from "../../src/types/index";
import { ThemeToggle } from "../../src/ThemeProvider";
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from "../../components/ui/command";
import { FaRegFilePdf } from "react-icons/fa";
import { Button } from "../../components/ui/button";
import WindowAction from "../../components/WindowActions";

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
  const [selectedResultIndex, setSelectedResultIndex] = useState<number>(0);
  const [isIndexing, setIsIndexing] = useState(false);
  const [indexingProgress, setIndexingProgress] =
    useState<IndexingProgress | null>(null);
  const [searchResults, setSearchResults] = useState<SearchResult[]>([]);
  const [isSettingsOpen, setIsSettingsOpen] = useState<boolean>(false);

  useEffect(() => {
    // Add listener for indexing progress
    const handleProgress = (_: any, progress: IndexingProgress) => {
      console.log("progress", progress);
      setIndexingProgress(progress);
    };

    window.electron.onIndexingProgress(handleProgress);

    return () => {
      window.electron.removeIndexingProgress(handleProgress);
    };
  }, []);

  const handleSearch = async (query: string) => {
    setSearchQuery(query);

    if (!query.trim()) {
      setSearchResults([]);
      return;
    }

    try {
      const results = await window.electron.searchFiles(query);

      // Transform the database results into SearchResult format
      const formattedResults = results.map((file: FileMetadata) => ({
        id: file.id,
        title: file.name,
        path: file.path,
        category: getCategoryFromExtension(file.extension),
        size: file.size,
        modified: file.modified,
      }));

      setSearchResults(formattedResults);
    } catch (error) {
      console.error("Error searching files:", error);
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
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setSelectedResultIndex((current) =>
          current >= searchResults.length - 1 ? 0 : current + 1
        );
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        setSelectedResultIndex((current) =>
          current <= 0 ? searchResults.length - 1 : current - 1
        );
      } else if (e.key === "Enter") {
        e.preventDefault();
        const selectedItem = searchResults[selectedResultIndex]; // Changed name here
        if (selectedItem) {
          // Handle result selection
        }
      }
    };

    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [searchResults, selectedResultIndex]);

  const handleResultSelect = async (result: SearchResult) => {
    setSelectedResultIndex(result.id);
    try {
      await window.electron.openFile(result.path);
    } catch (error) {
      console.error("Error opening file:", error);
      // Optionally show an error toast/notification
    }
  };

  return (
    <div>
      <div className="h-8 bg-background flex justify-between items-center select-none dragable px-3">
        <WindowAction />
        <div className="no-drag">
          <div>Kita</div>
        </div>
        <div className="flex flex-row items-center rounded-lg no-drag ">
          <Button
            variant="titleBar"
            onClick={() => setIsSettingsOpen(true)}
            size="sm"
            className="group flex flex-row items-center gap-1 z-10"
          >
            <Folder className="h-4 w-4" />
            <span>Folders</span>
          </Button>
          <ThemeToggle />
        </div>
      </div>
      <Command
        className="rounded-lg border border-border shadow-md"
        shouldFilter={false}
      >
        <CommandInput
          placeholder="Type a command or search..."
          value={searchQuery}
          onValueChange={(e) => handleSearch(e)}
          className="border border-border"
        />
        <div className="flex flex-col h-full min-h-[300px] max-h-[400px]">
          {searchResults.length === 0 ? (
            <>
              <CommandGroup heading="Suggestions">
                <CommandEmpty>No results found.</CommandEmpty>
              </CommandGroup>
            </>
          ) : (
            <div className="flex flex-col flex-1 overflow-hidden">
              <div className="sticky top-0 bg-background border-b border-border z-10">
                <CommandGroup
                  heading={`Found ${searchResults.length} results`}
                />
              </div>
              <CommandList className="flex-1 overflow-auto">
                <CommandGroup>
                  {searchResults.map((result) => (
                    <CommandItem
                      key={result.id}
                      value={result.title}
                      className="flex items-center justify-between cursor-pointer"
                      onSelect={() => {
                        setSelectedResultIndex(result.id);
                        handleResultSelect(result);
                      }}
                    >
                      <div className="flex items-center gap-2 flex-1">
                        <div className="flex flex-col flex-1">
                          <div className="flex flex-row items-start gap-1">
                            {getFileIcon(result.path)}
                            <span>{result.title}</span>
                          </div>
                          <span className="text-xs text-muted-foreground truncate pl-5">
                            {result.path}
                          </span>
                        </div>
                        <span className="text-xs text-muted-foreground">
                          {result.category}
                        </span>
                      </div>
                    </CommandItem>
                  ))}
                </CommandGroup>
              </CommandList>
            </div>
          )}
        </div>
      </Command>
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
