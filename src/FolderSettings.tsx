import { Folder, Loader2 } from "lucide-react";
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from "./components/ui/sheet";
import { Button } from "./components/ui/button";
import { Checkbox } from "./components/ui/checkbox";
import { useEffect } from "react";
import { Progress } from "./components/ui/progress";
import { IndexingProgress, SearchCategory } from "./types/types";

interface FolderSettingsProps {
  toggleCategory: (category: SearchCategory) => void;
  selectedCategories: Set<SearchCategory>;
  searchCategories: readonly SearchCategory[];
  isIndexing: boolean;
  setIsIndexing: (val: boolean) => void;
  indexingProgress: IndexingProgress | null;
  handleSelectPaths: () => void;
  isSettingsOpen: boolean;
  setIsSettingsOpen: (val: boolean) => void;
  setIndexingProgress: (val: IndexingProgress | null) => void;
}

export default function FolderSettings(props: FolderSettingsProps) {
  const {
    toggleCategory,
    selectedCategories,
    searchCategories,
    isIndexing,
    indexingProgress,
    handleSelectPaths,
    isSettingsOpen,
    setIsSettingsOpen,
    setIsIndexing,
    setIndexingProgress,
  } = props;

  useEffect(() => {
    if (indexingProgress?.percentage === 100) {
      // Add a small delay to show the 100% state briefly
      const timer = setTimeout(() => {
        setIsIndexing(false);
        setIndexingProgress(null);
      }, 500);

      return () => clearTimeout(timer);
    }
  }, [indexingProgress?.percentage]);

  return (
    <Sheet open={isSettingsOpen} onOpenChange={setIsSettingsOpen}>
      <SheetContent
        side="right"
        className="w-[400px] sm:w-[540px] border-l border-border"
      >
        <SheetHeader>
          <SheetTitle className="justify-start flex">
            Files & Folders
          </SheetTitle>
          <SheetDescription className="justify-start flex">
            Select files and folders to include in search results.
          </SheetDescription>
        </SheetHeader>
        <div className="mt-6 space-y-4">
          <Button
            variant="outline"
            size="sm"
            onClick={handleSelectPaths}
            disabled={isIndexing}
            className="shrink-0"
          >
            {isIndexing ? (
              <Loader2 className="h-4 w-4 animate-spin mr-2" />
            ) : (
              <Folder className="h-4 w-4 mr-2" />
            )}
            {isIndexing ? "Processing..." : "Select Folders/Files"}
          </Button>
          {isIndexing && indexingProgress && (
            <div className="mt-2 space-y-1">
              <Progress value={indexingProgress.percentage} className="h-1" />
              <p className="text-xs text-muted-foreground">
                Processed {indexingProgress.processed} of{" "}
                {indexingProgress.total} files ({indexingProgress.percentage}%)
              </p>
            </div>
          )}
          {searchCategories.map((category) => (
            <div key={category} className="flex items-center space-x-2">
              <Checkbox
                id={category}
                checked={selectedCategories.has(category)}
                onCheckedChange={() => toggleCategory(category)}
              />
              <label
                htmlFor={category}
                className="text-sm font-medium leading-none peer-disabled:cursor-not-allowed peer-disabled:opacity-70"
              >
                {category}
              </label>
            </div>
          ))}
        </div>
      </SheetContent>
    </Sheet>
  );
}
