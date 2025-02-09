import { useState, useEffect } from "react";
import TitleBar from "../../components/Titlebar";
import { Input } from "../../components/ui/input";
import { Progress } from "../../components/ui/progress";
import { Button } from "../../components/ui/button";
import { cn } from "../../src/lib/utils";
import { Folder, Loader2 } from "lucide-react";
import {
  SearchResult,
  SearchCategory,
  FileMetadata,
} from "../../src/types/index";

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

interface IndexingProgress {
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

  const getCategoryFromExtension = (extension: string): SearchCategory => {
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
      // Add more cases as needed
      default:
        return "Other";
    }
  };

  return (
    <div className="flex flex-col h-full">
      <TitleBar
        selectedCategories={selectedCategories}
        toggleCategory={toggleCategory}
        searchCategories={searchCategories}
      />

      <div className="flex flex-col max-w-2xl mx-auto mt-8 w-full bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/60 rounded-xl border shadow-xl">
        <div className="p-3 border-b">
          <div className="flex items-center gap-2">
            <Input
              value={searchQuery}
              onChange={(e) => handleSearch(e.target.value)}
              className="border-none bg-transparent focus-visible:ring-0 focus-visible:ring-offset-0 placeholder:text-muted-foreground/60"
              placeholder="Search your computer"
            />
            <Button
              variant="outline"
              size="sm"
              onClick={handleSelectFolder}
              disabled={isIndexing}
              className="shrink-0"
            >
              {isIndexing ? (
                <Loader2 className="h-4 w-4 animate-spin mr-2" />
              ) : (
                <Folder className="h-4 w-4 mr-2" />
              )}
              {isIndexing ? "Indexing..." : "Index Folder"}
            </Button>
          </div>

          {/* Progress bar */}
          {indexingProgress && (
            <div className="mt-2 space-y-1">
              <Progress value={indexingProgress.percentage} className="h-1" />
              <p className="text-xs text-muted-foreground">
                Processed {indexingProgress.processed} of{" "}
                {indexingProgress.total} files ({indexingProgress.percentage}%)
              </p>
            </div>
          )}
        </div>

        <SearchResults
          results={searchResults}
          selectedId={selectedResultIndex}
          onSelect={setSelectedResultIndex}
        />
      </div>
    </div>
  );
}

interface SearchResultsProps {
  results: SearchResult[];
  selectedId: number;
  onSelect: (id: number) => void;
}

function SearchResults(props: SearchResultsProps) {
  const { results, selectedId, onSelect } = props;

  return (
    <div className="p-2 max-h-[60vh] overflow-auto">
      {results.map((result) => (
        <div
          key={result.id}
          onClick={() => onSelect(result.id)}
          className={cn(
            "flex items-center gap-3 px-3 py-2 rounded-md cursor-pointer",
            selectedId === result.id
              ? "bg-primary text-primary-foreground"
              : "hover:bg-muted"
          )}
        >
          {result.icon}
          <div className="flex flex-col">
            <span className="text-sm font-medium">{result.title}</span>
            <span className="text-xs text-muted-foreground">
              {result.category}
            </span>
          </div>
        </div>
      ))}
    </div>
  );
}
