import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import "./globals.css";
import Footer from "./Footer";
import {
  AppMetadata,
  FileMetadata,
  IndexingProgress,
  searchCategories,
  SearchCategory,
} from "./types/types";
import Header from "./Header";
import { errorToast } from "./components/ui/toast";
import { open } from "@tauri-apps/plugin-dialog";
import { documentDir } from "@tauri-apps/api/path";
import Settings from "./Settings";
import AppTable from "./AppTable";
import FilesTable from "./FilesTable";

export default function App() {
  const [searchQuery, setSearchQuery] = useState<string>("");
  const [selectedCategories, setSelectedCategories] = useState<
    Set<SearchCategory>
  >(new Set(searchCategories));
  const [isIndexing, setIsIndexing] = useState(false);
  const [indexingProgress, setIndexingProgress] =
    useState<IndexingProgress | null>(null);
  const [isSettingsOpen, setIsSettingsOpen] = useState<boolean>(false);
  const [appsData, setAppsData] = useState<AppMetadata[]>([]);
  const [filesData, setFilesData] = useState<FileMetadata[]>([]);
  const [selectedItem, setSelectedItem] = useState<string>();
  // const [recents, setRecents] = useState<FileMetadata[]>([]);
  // const [isSearchActive, setIsSearchActive] = useState(false);
  const [resourceData, setResourceData] = useState<
    Record<number, { cpu_usage: number; memory_bytes: number }>
  >({});
  const [showProgress, setShowProgress] = useState(false);
  const [indexElapsedTime, setIndexElapsedTime] = useState<number | null>(null);

  // handles key up and down acions
  // useEffect(() => {
  //   const handleKeyDown = (e: KeyboardEvent) => {
  //     if (searchSections.length === 0) return;

  //     let currentSection = selectedSection;
  //     if (currentSection === undefined) {
  //       const appsIndex = searchSections.findIndex(
  //         (sec) => sec.type_ === SearchSectionType.Apps
  //       );
  //       currentSection = appsIndex >= 0 ? appsIndex : 0;
  //     }

  //     if (e.key === "ArrowDown") {
  //       e.preventDefault();

  //       if (selectedSection === undefined) {
  //         setSelectedSection(currentSection);
  //         setSelectedItem(0);
  //         return;
  //       }

  //       const section = searchSections[currentSection];
  //       if (selectedItem < section.items.length - 1) {
  //         setSelectedItem(selectedItem + 1);
  //       } else if (currentSection < searchSections.length - 1) {
  //         setSelectedSection(currentSection + 1);
  //         setSelectedItem(0);
  //       }
  //     } else if (e.key === "ArrowUp") {
  //       e.preventDefault();
  //       if (selectedSection === undefined) {
  //         return;
  //       }

  //       if (selectedItem > 0) {
  //         setSelectedItem(selectedItem - 1);
  //       } else if (currentSection > 0) {
  //         setSelectedSection(currentSection - 1);
  //         setSelectedItem(searchSections[currentSection - 1].items.length - 1);
  //       }
  //     } else if (e.key === "Enter") {
  //       e.preventDefault();
  //       if (selectedSection === undefined) {
  //         return;
  //       }

  //       const section = searchSections[currentSection];
  //       const item = section?.items[selectedItem];
  //       if (item) {
  //         handleResultSelect(item);
  //       }
  //     }
  //   };

  //   document.addEventListener("keydown", handleKeyDown);
  //   return () => document.removeEventListener("keydown", handleKeyDown);
  // }, [searchSections, selectedSection, selectedItem]);

  // handles switching to the file or app
  async function handleAppSelect(app: AppMetadata) {
    await invoke<AppMetadata[]>("launch_or_switch_to_app", {
      app: app,
    });
  }

  async function handleFileSelect(file: FileMetadata) {
    await invoke<FileMetadata[]>("launch_or_switch_to_app", {
      app: file,
    });
  }

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

  const handleSelectPaths = async () => {
    setShowProgress(false);
    setIndexingProgress(null);
    setIndexElapsedTime(null);

    try {
      const selected = await open({
        multiple: true,
        directory: true,
        title: "Select Files or Folders to Index",
        defaultPath: await documentDir(),
      });

      if (!selected || (Array.isArray(selected) && !selected.length)) return;
      const paths = Array.isArray(selected) ? selected : [selected];
      const indexStartTime = Date.now();

      setIsIndexing(true);
      setShowProgress(true);
      setIndexingProgress(null);

      const unlistenProgress = await listen<IndexingProgress>(
        "file-processing-progress",
        (event) => {
          console.log("Received progress event:", event);
          const progress = event.payload;
          if (progress) {
            setIndexingProgress(progress);
          }
        }
      );

      await invoke("process_paths_command", { paths });

      const indexEndTime = Date.now();
      const indexTimeElapsed = (indexEndTime - indexStartTime) / 1000;

      setIndexElapsedTime(indexTimeElapsed);
      unlistenProgress();
      setIsIndexing(false);
    } catch (error) {
      const err = error as Error;
      setIsIndexing(false);
      setShowProgress(false);
      setIndexingProgress(null);
      errorToast("Error indexing selected paths:", err.message);
    }
  };

  // reset the progress indicator if the settings are closed
  useEffect(() => {
    setShowProgress(false);
    setIndexingProgress(null);
  }, [isSettingsOpen]);

  // listens for resource events and app update events
  useEffect(() => {
    let unlistenUsage: UnlistenFn | undefined;
    let unlistenApps: UnlistenFn | undefined;

    (async () => {
      try {
        const appData = await invoke<AppMetadata[]>("get_apps_data");
        setAppsData(appData);

        const filesData = await invoke<FileMetadata[]>("get_files_data", {
          query: searchQuery,
        });
        setFilesData(filesData);

        console.log("filesData", filesData);

        const pids = appData
          .filter((app) => app.pid != null)
          .map((app) => app.pid);

        await invoke("start_resource_monitoring", { pids });

        unlistenUsage = await listen("resource-usage-updated", (event) => {
          const updates = event.payload as Record<
            number,
            { cpu_usage: number; memory_bytes: number }
          >;

          setResourceData((prev) => {
            const newState = { ...prev };
            Object.entries(updates).forEach(([pidStr, usage]) => {
              const pidNum = Number(pidStr);
              newState[pidNum] = {
                cpu_usage: usage.cpu_usage,
                memory_bytes: usage.memory_bytes,
              };
            });
            return newState;
          });
        });

        unlistenApps = await listen("apps-with-resources-updated", (event) => {
          const updatedApps = event.payload as AppMetadata[];
          setAppsData((prev) => {
            return prev.map((apps) => {
              return {
                ...apps,
                items: updatedApps.map((app) => app),
              };

              return apps;
            });
          });
        });
      } catch (err) {
        console.error("Failed to set up resource monitoring:", err);
      }
    })();

    return () => {
      if (unlistenUsage) unlistenUsage();
      if (unlistenApps) unlistenApps();

      invoke("stop_resource_monitoring").catch((err) => {
        console.error("Failed to stop resource monitoring:", err);
      });
    };
  }, []);

  const filterItems = <T extends { name: string }>(
    items: T[],
    query: string
  ): T[] => {
    if (!query.trim()) {
      return items;
    }

    return items.filter((item) => {
      return item.name.toLowerCase().includes(query.toLowerCase());
    });
  };

  async function refreshApps() {
    try {
      const appData = await invoke<AppMetadata[]>("get_apps_data");
      setAppsData(appData);
    } catch (err) {
      console.error("Failed to refresh apps:", err);
    }
  }

  return (
    <div className="h-screen flex flex-col overflow-hidden">
      <Header setSearchQuery={setSearchQuery} searchQuery={searchQuery} />
      <main className="flex-1 px-2 pt-4 overflow-auto scrollbar">
        {/* {searchQuery.trim() === "" ? (
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
        ) : filteredResults.length === 0 ? (
          // Show empty state if searching but no results
          <div className="flex h-full items-center justify-center">
            <EmptyState />
          </div>
        ) : ( */}
        <AppTable
          data={useMemo(() => {
            return filterItems(appsData, searchQuery);
          }, [appsData, searchQuery])}
          onRowClick={(app) => {
            setSelectedItem(app.name);
            handleAppSelect(app);
          }}
          appResourceData={resourceData}
          refreshApps={refreshApps}
        />
        <FilesTable
          data={useMemo(() => {
            return filterItems(filesData, searchQuery);
          }, [filesData, searchQuery])}
          onRowClick={(file) => {
            setSelectedItem(file.name);
            handleFileSelect(file);
          }}
        />
      </main>
      <div className="sticky bottom-0">
        <Footer setIsSettingsOpen={setIsSettingsOpen} />
      </div>
      <Settings
        selectedCategories={selectedCategories}
        toggleCategory={toggleCategory}
        searchCategories={searchCategories}
        isIndexing={isIndexing}
        indexingProgress={indexingProgress}
        handleSelectPaths={handleSelectPaths}
        isSettingsOpen={isSettingsOpen}
        setIsSettingsOpen={setIsSettingsOpen}
        showProgress={showProgress}
        indexElapsedTime={indexElapsedTime ?? 0}
      />
    </div>
  );
}
