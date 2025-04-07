import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import "./globals.css";
import Footer from "./Footer";
import {
  AppMetadata,
  AppSettings,
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
import Settings from "./settings/Settings";
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
  const [settings, setSettings] = useState<AppSettings>();
  const [loadingSettings, setLoadingSettings] = useState<boolean>(false);

  // used to load the app once it's ready so it doesn't flash a white screen
  useEffect(() => {
    const showWindow = async () => {
      try {
        await invoke("show_main_window");
      } catch (e) {
        console.error("Failed to show window:", e);
      }
    };

    // Add a small delay to ensure React has fully rendered
    setTimeout(showWindow, 100);
  }, []);

  // loads settings
  useEffect(() => {
    const getSettings = async () => {
      try {
        setLoadingSettings(true);
        const appSettings = await invoke<AppSettings>("get_settings");
        setSettings(appSettings);
      } catch (error) {
        errorToast("Unable to fetch settings");
      } finally {
        setLoadingSettings(false);
      }
    };

    getSettings();
  }, []);

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
      const startTime = Date.now();

      console.log("paths", paths);

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

      const res = await invoke("process_paths_command", { paths });

      console.log("res", res);

      const indexEndTime = Date.now();
      const indexTimeElapsed = (indexEndTime - startTime) / 1000;

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

  const combinedFileResults = useMemo(() => {
    // Start with files from regular search
    const mergedResults = [...filesData];
    const existingIds = new Set(filesData.map((file) => file.id));

    // Add semantic results that don't have matching IDs
    semanticData.forEach((semanticItem) => {
      if (!existingIds.has(semanticItem.id)) {
        // Convert semantic item to file metadata format
        mergedResults.push({
          ...semanticItem,
          size: semanticItem.size, // Default size if not available
          updated_at: undefined,
          created_at: undefined,
        });
      }
    });

    // Create a map of distances for sorting
    const distanceMap: Record<string, number> = {};
    semanticData.forEach((item) => {
      if (item.id) {
        distanceMap[item.id] = item.distance;
      }
    });

    // Sort files by semantic match strength (if present)
    return mergedResults.sort((a, b) => {
      const aDistance = distanceMap[a.id || ""] || 1;
      const bDistance = distanceMap[b.id || ""] || 1;

      // Strong match: > 80% similarity (distance < 0.2)
      const aIsStrong = aDistance < 0.2;
      const bIsStrong = bDistance < 0.2;

      // Good match: 50-80% similarity (distance between 0.2 and 0.5)
      const aIsGood = aDistance >= 0.2 && aDistance < 0.5;
      const bIsGood = bDistance >= 0.2 && bDistance < 0.5;

      // Weak match: < 50% similarity (distance >= 0.5)
      const aIsWeak = aDistance >= 0.5 && aDistance < 0.85;
      const bIsWeak = bDistance >= 0.5 && bDistance < 0.85;

      // Sort by match category first
      if (aIsStrong && !bIsStrong) return -1;
      if (!aIsStrong && bIsStrong) return 1;
      if (aIsGood && !bIsGood && !bIsStrong) return -1;
      if (!aIsGood && bIsGood && !aIsStrong) return 1;
      if (aIsWeak && !bIsWeak && !bIsGood && !bIsStrong) return -1;
      if (!aIsWeak && bIsWeak && !bIsGood && !bIsStrong) return 1;

      // Within the same category, sort by actual distance
      if (
        (aIsStrong && bIsStrong) ||
        (aIsGood && bIsGood) ||
        (aIsWeak && bIsWeak)
      ) {
        return aDistance - bDistance;
      }

      // For non-semantic results or results with same distance, preserve original order
      return 0;
    });
  }, [filesData, semanticData]);

  const semanticMatchesById = useMemo(() => {
    const matchesMap: Record<string, SemanticMetadata> = {};
    semanticData.forEach((item) => {
      if (item.id) {
        matchesMap[item.id] = item;
      }
    });
    return matchesMap;
  }, [semanticData]);

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

        // Only run semantic search if there's an actual query
        if (searchQuery.trim()) {
          const semanticData = await invoke<SemanticMetadata[]>(
            "get_semantic_files_data",
            {
              query: searchQuery,
            }
          );

          if (isMounted) {
            setSemanticData(semanticData);
          }
        } else {
          // Clear semantic data when query is empty
          if (isMounted) {
            setSemanticData([]);
          }
        }

        if (isMounted) {
          setFilesData(fileData);
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
    () => filterFiles(combinedFileResults, searchQuery),
    [filterFiles, combinedFileResults, searchQuery]
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

  const sortAppsByRunningStatusAndName = (
    apps: AppMetadata[]
  ): AppMetadata[] => {
    return [...apps].sort((a, b) => {
      // First sort by running status (running apps first)
      if (a.pid && !b.pid) return -1;
      if (!a.pid && b.pid) return 1;

      // Then sort alphabetically within each group
      return a.name.toLowerCase().localeCompare(b.name.toLowerCase());
    });
  };

  // memoized full section array
  const sections: Section[] = useMemo(() => {
    const sortedApps = sortAppsByRunningStatusAndName(filteredApps);

    return [
      {
        ...sectionDefinitions[0],
        counts: filteredApps.length,
        component: (
          <AppTable
            data={sortedApps}
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
        // Add a method to get a limited component with pre-sorted data
        getLimitedComponent: (limit: number) => (
          <AppTable
            data={sortedApps.slice(0, limit)}
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
            semanticMatches={semanticMatchesById}
          />
        ),
        // Add a method to get a limited component
        getLimitedComponent: () => (
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
            semanticMatches={semanticMatchesById}
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

  console.log("the query", searchQuery);

  // TODO: the front end should just know what files to return when depending on what the user is trying to search for, instead of trying to return all of them

  // maybe that means combined the semantic data with the files data in the useEffect when the query changes and then returning the union of them and handling duplicates. It need to just like work...

  // so when the query changes, we should start the LIKE searching
  // we shoudl do the full text search
  // we shodl do the similaity search
  // then de-dupe on the front end and show the relevant results to the user

  return (
    <div className="flex flex-col overflow-hidden rounded-xl border border-border h-screen ">
      <Header
        setSearchQuery={setSearchQuery}
        searchQuery={searchQuery}
        settings={settings ?? {}}
        setIsSettingsOpen={setIsSettingsOpen}
      />
      <main className="flex-1 overflow-auto scrollbar">
        <SectionNav
          sections={sections}
          activeSection={activeSection}
          setActiveSection={setActiveSection}
          totalCount={totalCount}
        />
      </main>
      <div className="sticky bottom-0">
        <Footer
          setIsSettingsOpen={setIsSettingsOpen}
          searchQuery={searchQuery}
        />
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
