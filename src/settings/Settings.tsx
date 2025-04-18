import { Folder, Loader2 } from "lucide-react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from "../components/ui/dialog";
import { Button } from "../components/ui/button";
import { Checkbox } from "../components/ui/checkbox";
import { forwardRef, useState } from "react";
import { IndexingProgress, SearchCategory } from "../types/types";
import { ScrollArea } from "../components/ui/scroll-area";
import { Separator } from "../components/ui/separator";
import { Badge } from "../components/ui/badge";
import Models from "./Models";

interface FolderSettingsProps {
  toggleCategory: (category: SearchCategory) => void;
  selectedCategories: Set<SearchCategory>;
  searchCategories: readonly SearchCategory[];
  isIndexing: boolean;
  indexingProgress: IndexingProgress | null;
  handleSelectPaths: () => void;
  isSettingsOpen: boolean;
  setIsSettingsOpen: (val: boolean) => void;
  showProgress: boolean;
  indexElapsedTime: number;
}

type SettingCategory = "Indexing" | "Models";

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
      showProgress,
      indexElapsedTime,
    } = props;

    const [selectedSettingCategory, setSelectedSettingCategory] =
      useState<SettingCategory>("Indexing");

    const settingCategoryComponents: Record<SettingCategory, JSX.Element> = {
      Indexing: (
        <IndexingSettings
          selectedCategories={selectedCategories}
          toggleCategory={toggleCategory}
          searchCategories={searchCategories}
          isIndexing={isIndexing}
          indexingProgress={indexingProgress}
          handleSelectPaths={handleSelectPaths}
          showProgress={showProgress}
          indexElapsedTime={indexElapsedTime}
        />
      ),
      Models: <Models />,
    };

    return (
      <Dialog open={isSettingsOpen} onOpenChange={setIsSettingsOpen}>
        <DialogContent
          ref={ref}
          className="sm:max-w-lg md:max-w-2xl lg:max-w-3xl xl:max-w-4xl h-max-3/4 overflow-hidden p-0 border border-border"
        >
          <div className="flex flex-col">
            <div className="flex items-center border-b border-border p-4">
              <DialogTitle className="font-normal text-gray-200">
                Settings
              </DialogTitle>
              <DialogDescription />
            </div>
            <div className="flex flex-1 overflow-hidden">
              <div className="w-[200px] border-r border-border">
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
                <ScrollArea className="p-4 h-[480px]">
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
  showProgress: boolean;
  indexElapsedTime: number;
}

function IndexingSettings(props: IndexSettingsProps) {
  const {
    toggleCategory,
    selectedCategories,
    searchCategories,
    isIndexing,
    indexingProgress,
    handleSelectPaths,
    showProgress,
    indexElapsedTime,
  } = props;

  const formatTime = (seconds: number) => {
    if (seconds < 60) {
      return `${seconds.toFixed(2)} seconds`;
    } else {
      const minutes = Math.floor(seconds / 60);
      const remainingSeconds = seconds % 60;
      return `${minutes} ${
        minutes === 1 ? "minute" : "minutes"
      } ${remainingSeconds.toFixed(0)} seconds`;
    }
  };

  return (
    <div className="space-y-2 p-4 text-white">
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
      {showProgress && (
        <div className="mt-2 space-y-2">
          <div className="flex flex-row items-center gap-2">
            <div className="h-1 w-full bg-gray-200 rounded-full overflow-hidden">
              <div
                className="h-full bg-blue-700 transition-all duration-300"
                style={{
                  width: indexingProgress?.total
                    ? `${Math.round(
                        ((indexingProgress.processed +
                          (indexingProgress.total -
                            indexingProgress.processed)) /
                          indexingProgress.total) *
                          100
                      )}%`
                    : "0%",
                }}
              />
            </div>
            <div>
              {indexingProgress?.total &&
                Math.round(
                  ((indexingProgress.processed +
                    (indexingProgress.total - indexingProgress.processed)) /
                    indexingProgress.total) *
                    100
                )}
              %
            </div>
          </div>
          <div className="grid grid-cols-[30%_70%] gap-x-2 text-xs text-muted-foreground">
            <div>Total files:</div>
            <div>{indexingProgress?.total || 0}</div>

            <div>Files processed:</div>
            <div>{indexingProgress?.processed || 0}</div>

            <div>Files skipped:</div>
            <div>
              {indexingProgress
                ? indexingProgress.total - indexingProgress.processed
                : 0}
            </div>

            {!isIndexing && indexElapsedTime !== null && (
              <>
                <div>Processing time:</div>
                <div>{formatTime(indexElapsedTime)}</div>
              </>
            )}
          </div>

          <Badge className="text-xs text-left bg-green-700 text-white">
            {isIndexing ? <>Processing files...</> : <>Processing complete!</>}
          </Badge>
        </div>
      )}

      {/* <Separator className="my-4" />
      <h4 className="text-sm font-medium mb-3">File Types</h4> */}
      {/* <div className="space-y-3">
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
      </div> */}
    </div>
  );
}

FolderSettings.displayName = "FolderSettings";
export default FolderSettings;
