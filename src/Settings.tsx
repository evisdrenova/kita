import { Folder, Loader2 } from "lucide-react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "./components/ui/dialog";
import { Button } from "./components/ui/button";
import { Checkbox } from "./components/ui/checkbox";
import { forwardRef, useEffect } from "react";
import { Progress } from "./components/ui/progress";
import { IndexingProgress, SearchCategory } from "./types/types";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "./components/ui/tabs";

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

const FolderSettings = forwardRef<HTMLDivElement, FolderSettingsProps>(
  (props, ref) => {
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
        const timer = setTimeout(() => {
          setIsIndexing(false);
          setIndexingProgress(null);
        }, 500);

        return () => clearTimeout(timer);
      }
    }, [indexingProgress?.percentage]);

    return (
      <Dialog open={isSettingsOpen} onOpenChange={setIsSettingsOpen}>
        <DialogContent className="sm:max-w-lg md:max-w-2xl lg:max-w-3xl xl:max-w-4xl h-max-3/4">
          <DialogHeader>
            <DialogTitle />
          </DialogHeader>
          <div className="mt-6 space-y-4">
            <SettingsContent
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
            />
          </div>
        </DialogContent>
      </Dialog>
    );
  }
);

interface SettingsContentProps {
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

function SettingsContent(props: SettingsContentProps) {
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
  return (
    <Tabs
      defaultValue="tab-1"
      orientation="vertical"
      className="flex flex-row w-full gap-2"
    >
      <TabsList className="flex-col gap-1 bg-background py-0">
        <TabsTrigger
          value="tab-1"
          className="w-full justify-start data-[state=active]:bg-muted data-[state=active]:shadow-none"
        >
          Indexing
        </TabsTrigger>
      </TabsList>
      <div className="grow rounded-lg text-start">
        <TabsContent value="tab-1">
          <IndexingSettings
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
          />
        </TabsContent>
      </div>
    </Tabs>
  );
}

interface IndexSettingsProps {
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

function IndexingSettings(props: IndexSettingsProps) {
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
  return (
    <div>
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
            Processed {indexingProgress.processed} of {indexingProgress.total}{" "}
            files ({indexingProgress.percentage}
            %)
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
  );
}

export default FolderSettings;
