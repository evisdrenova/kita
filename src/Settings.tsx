import { Folder, Loader2 } from "lucide-react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from "./components/ui/dialog";
import { Button } from "./components/ui/button";
import { Checkbox } from "./components/ui/checkbox";
import { forwardRef, useEffect, useState } from "react";
import { Progress } from "./components/ui/progress";
import { IndexingProgress, SearchCategory } from "./types/types";
import { ScrollArea } from "./components/ui/scroll-area";
import { Separator } from "./components/ui/separator";

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
type SettingCategory =
  | "General"
  | "Appearance"
  | "Indexing"
  | "Shortcuts"
  | "Advanced";

const FolderSettings = forwardRef<HTMLDivElement, FolderSettingsProps>(
  (props, _) => {
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

    const [selectedSettingCategory, setSelectedSettingCategory] =
      useState<SettingCategory>("Indexing");

    useEffect(() => {
      if (indexingProgress?.percentage === 100) {
        const timer = setTimeout(() => {
          setIsIndexing(false);
          setIndexingProgress(null);
        }, 500);

        return () => clearTimeout(timer);
      }
    }, [indexingProgress?.percentage]);

    const settingCategoryComponents: Record<SettingCategory, JSX.Element> = {
      General: <General />,
      Appearance: <Appearance />,
      Indexing: (
        <IndexingSettings
          selectedCategories={selectedCategories}
          toggleCategory={toggleCategory}
          searchCategories={searchCategories}
          isIndexing={isIndexing}
          indexingProgress={indexingProgress}
          handleSelectPaths={handleSelectPaths}
        />
      ),
      Shortcuts: <Shortcuts />,
      Advanced: <Advanced />,
    };

    return (
      <Dialog open={isSettingsOpen} onOpenChange={setIsSettingsOpen}>
        <DialogContent className="sm:max-w-lg md:max-w-2xl lg:max-w-3xl xl:max-w-4xl h-max-3/4 overflow-hidden p-0">
          <div className="flex flex-col">
            <div className="flex items-center border-b p-4">
              <DialogTitle className="font-normal">Settings</DialogTitle>
              <DialogDescription />
            </div>
            <div className="flex flex-1 overflow-hidden">
              <div className="w-[200px] border-r">
                <ScrollArea className="h-full">
                  <div className="p-2">
                    <nav className="flex flex-col gap-0.5">
                      {Object.keys(settingCategoryComponents).map(
                        (category) => (
                          <Button
                            key={category}
                            variant={
                              category === selectedSettingCategory
                                ? "secondary"
                                : "ghost"
                            }
                            className="justify-start h-9 px-2 font-normal"
                            onClick={() =>
                              setSelectedSettingCategory(
                                category as SettingCategory
                              )
                            }
                          >
                            {category}
                          </Button>
                        )
                      )}
                    </nav>
                  </div>
                </ScrollArea>
              </div>
              <div className="flex-1">
                <ScrollArea className="h-full">
                  {settingCategoryComponents[selectedSettingCategory]}
                </ScrollArea>
              </div>
            </div>
          </div>
        </DialogContent>
      </Dialog>
    );
  }
);

interface IndexSettingsProps {
  toggleCategory: (category: SearchCategory) => void;
  selectedCategories: Set<SearchCategory>;
  searchCategories: readonly SearchCategory[];
  isIndexing: boolean;
  indexingProgress: IndexingProgress | null;
  handleSelectPaths: () => void;
}

function IndexingSettings(props: IndexSettingsProps) {
  const {
    toggleCategory,
    selectedCategories,
    searchCategories,
    isIndexing,
    indexingProgress,
    handleSelectPaths,
  } = props;
  return (
    <div className="space-y-6 p-6">
      <h3 className="text-lg font-medium mb-1">Indexing</h3>
      <p className="text-sm text-muted-foreground mb-4">
        Choose which folders and file types to include in search results.
      </p>
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
      </div>
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
      <Separator className="my-4" />
      <h4 className="text-sm font-medium mb-3">File Types</h4>
      <div className="space-y-3">
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
    </div>
  );
}

function General() {
  return (
    <div className="space-y-6 p-6">
      <h3 className="text-lg font-medium mb-1">General Settings</h3>
      <p className="text-sm text-muted-foreground mb-4">
        Configure general application settings.
      </p>
      {/* Add general settings content here */}
      <div className="text-sm">General settings content will go here.</div>
    </div>
  );
}

function Appearance() {
  return (
    <div className="space-y-6 p-6">
      <h3 className="text-lg font-medium mb-1">Appearance</h3>
      <p className="text-sm text-muted-foreground mb-4">
        Customize the look and feel of the application.
      </p>
      <div className="text-sm">Appearance settings content will go here.</div>
    </div>
  );
}

function Shortcuts() {
  return (
    <div className="space-y-6 p-6">
      <h3 className="text-lg font-medium mb-1">Keyboard Shortcuts</h3>
      <p className="text-sm text-muted-foreground mb-4">
        Customize keyboard shortcuts for common actions.
      </p>
      <div className="text-sm">Keyboard shortcuts content will go here.</div>
    </div>
  );
}

function Advanced() {
  return (
    <div className="space-y-6 p-6">
      <h3 className="text-lg font-medium mb-1">Advanced Settings</h3>
      <p className="text-sm text-muted-foreground mb-4">
        Configure advanced application settings.
      </p>
      <div className="text-sm">Advanced settings content will go here.</div>
    </div>
  );
}

export default FolderSettings;
