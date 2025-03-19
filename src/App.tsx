import { useCallback, useEffect, useMemo, useState } from "react";
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
  Section,
  SemanticMetadata,
} from "./types/types";
import Header from "./Header";
import { errorToast } from "./components/ui/toast";
import { open } from "@tauri-apps/plugin-dialog";
import { documentDir } from "@tauri-apps/api/path";
import Settings from "./Settings";
import AppTable from "./AppTable";
import FilesTable from "./FilesTable";
import SectionNav from "./SectionNav";
import { Command, File } from "lucide-react";
import { register } from "@tauri-apps/plugin-global-shortcut";
import { handleShortcut } from "./globalShortcut";

await register("CommandOrControl+Shift+C", handleShortcut).then(() =>
  console.log("shortcut successfully registered")
);

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
  const [semanticData, setSemanticData] = useState<SemanticMetadata[]>([]);
  const [selectedItem, setSelectedItem] = useState<string>();
  // const [recents, setRecents] = useState<FileMetadata[]>([]);
  // const [isSearchActive, setIsSearchActive] = useState(false);
  const [resourceData, setResourceData] = useState<
    Record<number, { cpu_usage: number; memory_bytes: number }>
  >({});
  const [showProgress, setShowProgress] = useState(false);
  const [indexElapsedTime, setIndexElapsedTime] = useState<number | null>(null);
  const [currentSection, setCurrentSection] = useState<"apps" | "files">(
    "apps"
  );
  const [currentItemIndex, setCurrentItemIndex] = useState<number>(-1);
  const [activeSection, setActiveSection] = useState<number | null>(null);

  // base section definition
  const sectionDefinitions = [
    {
      id: 0,
      name: "Applications",
      icon: <Command className="w-4 h-4" />,
    },
    {
      id: 1,
      name: "Files",
      icon: <File className="w-4 h-4" />,
    },
  ];

  // Reset the selection when search query changes
  useEffect(() => {
    // Reset to apps section and first item when search query changes
    setCurrentSection("apps");
    setCurrentItemIndex(-1);
    setSelectedItem(undefined);
  }, [searchQuery]);

  // handles opening an app when the user selects it
  const handleAppSelect = useCallback(async (app: AppMetadata) => {
    await invoke<AppMetadata[]>("launch_or_switch_to_app", { app });
  }, []);

  // handles opening a file when the user selects it
  const handleFileSelect = useCallback(async (file: FileMetadata) => {
    await invoke<FileMetadata[]>("open_file", { filePath: file.path });
  }, []);

  // toggles categories in the index dialog
  const toggleCategory = useCallback((category: SearchCategory) => {
    setSelectedCategories((prev) => {
      const newSet = new Set(prev);
      if (newSet.has(category)) {
        newSet.delete(category);
      } else {
        newSet.add(category);
      }
      return newSet;
    });
  }, []);

  // starts the indexing when the user selects the paths they want to index
  const handleSelectPaths = useCallback(async () => {
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

      const filesData = await invoke<FileMetadata[]>("get_files_data", {
        query: searchQuery,
      });
      setFilesData(filesData);

      setIsIndexing(false);
    } catch (error) {
      const err = error as Error;
      setIsIndexing(false);
      setShowProgress(false);
      setIndexingProgress(null);
      errorToast("Error indexing selected paths:", err.message);
    }
  }, [searchQuery]);

  // reset the progress indicator if the settings are closed
  useEffect(() => {
    setShowProgress(false);
    setIndexingProgress(null);
  }, [isSettingsOpen]);

  // initial data load - runs once on mount
  useEffect(() => {
    const initialize = async () => {
      try {
        const appData = await invoke<AppMetadata[]>("get_apps_data");
        setAppsData(appData);

        const filesData = await invoke<FileMetadata[]>("get_files_data", {
          query: "",
        });
        setFilesData(filesData);

        const pids = appData
          .filter((app) => app.pid != null)
          .map((app) => app.pid);

        await invoke("start_resource_monitoring", { pids });
      } catch (err) {
        console.error("Failed to initialize data:", err);
      }
    };

    initialize();
  }, []);

  // resource usage monitoring - updates every second
  useEffect(() => {
    let unlistenUsage: UnlistenFn | undefined;

    const setupResourceMonitoring = async () => {
      try {
        unlistenUsage = await listen("resource-usage-updated", (event) => {
          const updates = event.payload as Record<
            number,
            { cpu_usage: number; memory_bytes: number }
          >;

          setResourceData((prev) => {
            const newState = { ...prev };
            let hasChanges = false;

            Object.entries(updates).forEach(([pidStr, usage]) => {
              const pidNum = Number(pidStr);
              if (
                !prev[pidNum] ||
                prev[pidNum].cpu_usage !== usage.cpu_usage ||
                prev[pidNum].memory_bytes !== usage.memory_bytes
              ) {
                newState[pidNum] = usage;
                hasChanges = true;
              }
            });

            return hasChanges ? newState : prev;
          });
        });
      } catch (err) {
        console.error("Failed to set up resource monitoring:", err);
      }
    };

    setupResourceMonitoring();

    return () => {
      if (unlistenUsage) unlistenUsage();
      invoke("stop_resource_monitoring").catch((err) => {
        console.error("Failed to stop resource monitoring:", err);
      });
    };
  }, []);

  // Apps updates monitoring
  useEffect(() => {
    let unlistenApps: UnlistenFn | undefined;

    const setupAppsMonitoring = async () => {
      try {
        unlistenApps = await listen("apps-with-resources-updated", (event) => {
          const updatedApps = event.payload as AppMetadata[];
          setAppsData((prev) => {
            const newApps = prev.map((app) => ({
              ...app,
              items: updatedApps.map((updatedApp) => updatedApp),
            }));

            if (JSON.stringify(prev) === JSON.stringify(newApps)) {
              return prev;
            }

            return newApps;
          });
        });
      } catch (err) {
        console.error("Failed to set up apps monitoring:", err);
      }
    };

    setupAppsMonitoring();

    return () => {
      if (unlistenApps) unlistenApps();
    };
  }, []);

  // fetches fitlered data from backend when searchQuery changes
  useEffect(() => {
    let isMounted = true;
    const fetchFilesData = async () => {
      try {
        const fileData = await invoke<FileMetadata[]>("get_files_data", {
          query: searchQuery,
        });

        const semanticData = await invoke<SemanticMetadata[]>(
          "get_semantic_files_data",
          {
            query: searchQuery,
          }
        );

        if (isMounted) {
          setFilesData(fileData);
          setSemanticData(semanticData);
        }
      } catch (error) {
        console.error("Failed to fetch files data:", error);
      }
    };

    fetchFilesData();
    return () => {
      isMounted = false;
    };
  }, [searchQuery]);

  const filterApps = useCallback(
    (items: AppMetadata[], query: string): AppMetadata[] => {
      if (!query.trim()) {
        return items; // Show all apps when query is empty
      }
      // Always filter apps by name regardless of query length
      return items.filter((item) =>
        item.name.toLowerCase().includes(query.toLowerCase())
      );
    },
    []
  );

  const filterFiles = useCallback(
    (items: FileMetadata[], query: string): FileMetadata[] => {
      if (!query.trim()) {
        return items; // Show all files when query is empty
      }

      if (query.trim().split(" ").length > 2) {
        return items; // Return backend results as-is
      }

      return items.filter((item) =>
        item.name.toLowerCase().includes(query.toLowerCase())
      );
    },
    []
  );

  // Apply the appropriate filters
  const filteredApps = useMemo(
    () => filterApps(appsData, searchQuery),
    [filterApps, appsData, searchQuery]
  );

  const filteredFiles = useMemo(
    () => filterFiles(filesData, searchQuery),
    [filterFiles, filesData, searchQuery]
  );
  // refreshes app data
  const refreshApps = useCallback(async () => {
    try {
      const appData = await invoke<AppMetadata[]>("get_apps_data");
      setAppsData(appData);
    } catch (err) {
      console.error("Failed to refresh apps:", err);
    }
  }, []);

  // handles keyboard navigation
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (appsData.length === 0 && filesData.length === 0) return;

      if (e.key === "ArrowDown") {
        e.preventDefault();

        if (currentSection === "apps") {
          if (currentItemIndex >= filteredApps.length - 1) {
            if (filteredFiles.length > 0) {
              setCurrentSection("files");
              setCurrentItemIndex(0);
              setSelectedItem(filteredFiles[0].name);
            }
          } else {
            const newIndex = currentItemIndex + 1;
            setCurrentItemIndex(newIndex);
            setSelectedItem(filteredApps[newIndex].name);
          }
        } else if (currentSection === "files") {
          if (currentItemIndex < filteredFiles.length - 1) {
            const newIndex = currentItemIndex + 1;
            setCurrentItemIndex(newIndex);
            setSelectedItem(filteredFiles[newIndex].name);
          }
        }
      } else if (e.key === "ArrowUp") {
        e.preventDefault();

        if (currentSection === "files") {
          if (currentItemIndex <= 0) {
            if (filteredApps.length > 0) {
              setCurrentSection("apps");
              setCurrentItemIndex(filteredApps.length - 1);
              setSelectedItem(filteredApps[filteredApps.length - 1].name);
            }
          } else {
            const newIndex = currentItemIndex - 1;
            setCurrentItemIndex(newIndex);
            setSelectedItem(filteredFiles[newIndex].name);
          }
        } else if (currentSection === "apps") {
          if (currentItemIndex > 0) {
            const newIndex = currentItemIndex - 1;
            setCurrentItemIndex(newIndex);
            setSelectedItem(filteredApps[newIndex].name);
          }
        }
      } else if (e.key === "Enter") {
        e.preventDefault();

        if (currentItemIndex >= 0) {
          if (
            currentSection === "apps" &&
            filteredApps.length > currentItemIndex
          ) {
            handleAppSelect(filteredApps[currentItemIndex]);
          } else if (
            currentSection === "files" &&
            filteredFiles.length > currentItemIndex
          ) {
            handleFileSelect(filteredFiles[currentItemIndex]);
          }
        }
      }
    };

    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [
    filteredApps,
    filteredFiles,
    currentSection,
    currentItemIndex,
    handleAppSelect,
    handleFileSelect,
  ]);

  // memoized full section array
  const sections: Section[] = useMemo(() => {
    return [
      {
        ...sectionDefinitions[0],
        counts: filteredApps.length,
        component: (
          <AppTable
            data={filteredApps}
            onRowClick={(app) => {
              setSelectedItem(app.name);
              setCurrentSection("apps");
              setCurrentItemIndex(
                filteredApps.findIndex((a) => a.name === app.name)
              );
              handleAppSelect(app);
            }}
            appResourceData={resourceData}
            refreshApps={refreshApps}
            selectedItemName={
              currentSection === "apps" ? selectedItem : undefined
            }
          />
        ),
        // Add a method to get a limited component
        getLimitedComponent: (limit: number) => (
          <AppTable
            data={filteredApps.slice(0, limit)}
            onRowClick={(app) => {
              setSelectedItem(app.name);
              setCurrentSection("apps");
              setCurrentItemIndex(
                filteredApps.findIndex((a) => a.name === app.name)
              );
              handleAppSelect(app);
            }}
            appResourceData={resourceData}
            refreshApps={refreshApps}
            selectedItemName={
              currentSection === "apps" ? selectedItem : undefined
            }
          />
        ),
      },
      {
        ...sectionDefinitions[1],
        counts: filteredFiles.length,
        component: (
          <FilesTable
            data={filteredFiles}
            onRowClick={(file) => {
              setSelectedItem(file.name);
              setCurrentSection("files");
              setCurrentItemIndex(
                filteredFiles.findIndex((f) => f.name === file.name)
              );
              handleFileSelect(file);
            }}
            selectedItemName={
              currentSection === "files" ? selectedItem : undefined
            }
          />
        ),
        // Add a method to get a limited component
        getLimitedComponent: (limit: number) => (
          <FilesTable
            data={filteredFiles.slice(0, limit)}
            onRowClick={(file) => {
              setSelectedItem(file.name);
              setCurrentSection("files");
              setCurrentItemIndex(
                filteredFiles.findIndex((f) => f.name === file.name)
              );
              handleFileSelect(file);
            }}
            selectedItemName={
              currentSection === "files" ? selectedItem : undefined
            }
          />
        ),
      },
    ];
  }, [
    filteredApps,
    filteredFiles,
    currentSection,
    selectedItem,
    resourceData,
    handleAppSelect,
    handleFileSelect,
    refreshApps,
  ]);
  // total counts for sections
  const totalCount = useMemo(() => {
    return filteredApps.length + filteredFiles.length;
  }, [filteredApps.length, filteredFiles.length]);

  console.log("files", filesData);
  // console.log("get apps data", appsData);

  console.log("the query", searchQuery);

  // TODO: the front end should just know what files to return when depending on what the user is trying to search for, instead of trying to return all of them

  // maybe that means combined the semantic data with the files data in the useEffect when the query changes and then returning the union of them and handling duplicates. It need to just like work...

  return (
    <div className="h-screen flex flex-col overflow-hidden">
      <Header setSearchQuery={setSearchQuery} searchQuery={searchQuery} />
      <main className="flex-1 overflow-auto scrollbar">
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
        <SectionNav
          sections={sections}
          activeSection={activeSection}
          setActiveSection={setActiveSection}
          totalCount={totalCount}
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
