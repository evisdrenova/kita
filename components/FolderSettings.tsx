import { Folder, FolderGit2, Loader2, Settings } from "lucide-react";
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
  SheetTrigger,
} from "./ui/sheet";
import { Button } from "./ui/button";
import { Checkbox } from "./ui/checkbox";
import { SearchCategory } from "../src/types/index";
import { IndexingProgress } from "../src/pages/Home";
import { Progress } from "./ui/progress";

interface FolderSettingsProps {
  toggleCategory: (category: SearchCategory) => void;
  selectedCategories: Set<SearchCategory>;
  searchCategories: readonly SearchCategory[];
  isIndexing: boolean;
  indexingProgress: IndexingProgress | null;
  handleSelectFolder: () => void;
  isSettingsOpen: boolean;
  setIsSettingsOpen: (val: boolean) => void;
}

export default function FolderSettings(props: FolderSettingsProps) {
  const {
    toggleCategory,
    selectedCategories,
    searchCategories,
    isIndexing,
    indexingProgress,
    handleSelectFolder,
    isSettingsOpen,
    setIsSettingsOpen,
  } = props;
  return (
    <Sheet open={isSettingsOpen} onOpenChange={setIsSettingsOpen}>
      <SheetContent side="right" className="w-[400px] sm:w-[540px]">
        <SheetHeader>
          <SheetTitle>Folders</SheetTitle>
          <SheetDescription>
            Only selected folders will appear in search results.
          </SheetDescription>
        </SheetHeader>
        <div className="mt-6 space-y-4">
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
          {indexingProgress && (
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
