import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./globals.css";
import { Button } from "@/src/components/ui/button";
import Footer from "./Footer";
import {
  AppMetadata,
  FileMetadata,
  IndexingProgress,
  searchCategories,
  SearchCategory,
  SearchSection,
} from "./types/types";
import Header from "./Header";
import { toast } from "sonner";

export default function App() {
  const [runningApps, setRunningApps] = useState<AppMetadata[]>([]);

  const [searchQuery, setSearchQuery] = useState<string>("");
  const [selectedCategories, setSelectedCategories] = useState<
    Set<SearchCategory>
  >(new Set(searchCategories));
  const [isIndexing, setIsIndexing] = useState(false);
  const [indexingProgress, setIndexingProgress] =
    useState<IndexingProgress | null>(null);
  const [isSettingsOpen, setIsSettingsOpen] = useState<boolean>(false);
  const [searchSections, setSearchSections] = useState<SearchSection[]>([]);
  const [selectedSection, setSelectedSection] = useState<number>(0);
  const [selectedItem, setSelectedItem] = useState<number>(0);
  const [updatedApps, setUpdatedApps] = useState<AppMetadata[]>([]);
  const [recents, setRecents] = useState<FileMetadata[]>([]);
  const [isSearchActive, setIsSearchActive] = useState(false);

  // useEffect(() => {
  //   const handleProgress = (_: any, progress: IndexingProgress) => {
  //     setIndexingProgress(progress);
  //   };

  //   window.electron.onIndexingProgress(handleProgress);
  //   return () => {
  //     window.electron.removeIndexingProgress(handleProgress);
  //   };
  // }, []);

  // useEffect(() => {
  //   const handler = (event: any, apps: AppMetadata[]) => {
  //     setUpdatedApps(apps);
  //   };

  //   window.electron.onResourceUsageUpdated(handler);
  //   return () => {
  //     window.electron.removeResourceUsageUpdated(handler);
  //   };
  // }, []);

  // // retrieves the recents
  // useEffect(() => {
  //   const fetchRecents = async () => {
  //     try {
  //       const recentFiles = await window.electron.getRecents();
  //       setRecents(recentFiles);
  //     } catch (error) {
  //       console.error("Error fetching recents:", error);
  //     }
  //   };
  //   fetchRecents();

  //   const interval = setInterval(fetchRecents, 60000);
  //   return () => clearInterval(interval);
  // }, []);

  const handleSearch = async (query: string) => {
    setSearchQuery(query);

    // Start monitoring resources when the user is typing a search
    if (query.trim()) {
      if (!isSearchActive) {
        setIsSearchActive(true);
        // window.electron.startResourceMonitoring();
      }
    } else {
      setSearchSections([]);
      if (isSearchActive) {
        setIsSearchActive(false);
        // window.electron.stopResourceMonitoring();
      }
    }

    // Rest of your existing search logic
    if (!query.trim()) {
      setSearchSections([]);
      return;
    }

    try {
      // const sections = await window.electron.searchFilesAndEmbeddings(query);
      const sections: SearchSection[] = [];
      setSearchSections(sections);
      setSelectedSection(0);
      setSelectedItem(0);
    } catch (error) {
      toast.error("Error searching:");
    }
  };

  // useEffect(() => {
  //   return () => {
  //     // Stop resource monitoring when component unmounts
  //     if (isSearchActive) {
  //       window.electron.stopResourceMonitoring();
  //     }
  //   };
  // }, [isSearchActive]);

  // const handleSelectPaths = async () => {
  //   try {
  //     const result = await window.electron.selectPaths({
  //       properties: ["openFile", "openDirectory", "multiSelections"],
  //     });

  //     if (result.canceled || !result.filePaths.length) return;

  //     setIsIndexing(true);
  //     setIndexingProgress(null);

  //     // Index all selected paths
  //     await window.electron.indexAndEmbedPaths(result.filePaths);

  //     setIsIndexing(false);
  //     setIndexingProgress(null);
  //   } catch (error) {
  //     console.error("Error indexing paths:", error);
  //     setIsIndexing(false);
  //     setIndexingProgress(null);
  //     toast.error("Error indexing selected paths");
  //   }
  // };

  // const toggleCategory = (category: SearchCategory) => {
  //   setSelectedCategories((prev) => {
  //     const newSet = new Set(prev);
  //     if (newSet.has(category)) {
  //       newSet.delete(category);
  //     } else {
  //       newSet.add(category);
  //     }
  //     return newSet;
  //   });
  // };

  // useEffect(() => {
  //   const handleKeyDown = (e: KeyboardEvent) => {
  //     if (searchSections.length === 0) return;

  //     if (e.key === "ArrowDown") {
  //       e.preventDefault();
  //       const currentSection = searchSections[selectedSection];
  //       if (selectedItem < currentSection.items.length - 1) {
  //         setSelectedItem(selectedItem + 1);
  //       } else if (selectedSection < searchSections.length - 1) {
  //         setSelectedSection(selectedSection + 1);
  //         setSelectedItem(0);
  //       }
  //     } else if (e.key === "ArrowUp") {
  //       e.preventDefault();
  //       if (selectedItem > 0) {
  //         setSelectedItem(selectedItem - 1);
  //       } else if (selectedSection > 0) {
  //         setSelectedSection(selectedSection - 1);
  //         setSelectedItem(searchSections[selectedSection - 1].items.length - 1);
  //       }
  //     } else if (e.key === "Enter") {
  //       e.preventDefault();
  //       const section = searchSections[selectedSection];
  //       const item = section?.items[selectedItem];
  //       if (item) {
  //         handleResultSelect(item, section.type);
  //       }
  //     }
  //   };

  //   document.addEventListener("keydown", handleKeyDown);
  //   return () => document.removeEventListener("keydown", handleKeyDown);
  // }, [searchSections, selectedSection, selectedItem]);

  // const handleResultSelect = async (
  //   item: SearchItem,
  //   type: SearchSectionType
  // ) => {
  //   try {
  //     if (type === SearchSectionType.Apps) {
  //       await window.electron.launchOrSwitch(item as AppMetadata);
  //     } else {
  //       await window.electron.openFile((item as FileMetadata).path);
  //     }
  //   } catch (error) {
  //     toast.error("Error opening item");
  //   }
  // };

  // console.log("search sections", searchSections);
  // console.log("recents", recents);

  // // sort sections with apps first
  // const sortedSections = useMemo(() => {
  //   return [...searchSections].sort((a, b) => {
  //     if (a.type === SearchSectionType.Apps) return -1;
  //     if (b.type === SearchSectionType.Apps) return 1;
  //     if (a.type === SearchSectionType.Files) return -1;
  //     if (b.type === SearchSectionType.Files) return 1;
  //     return 0;
  //   });
  // }, [searchSections]);

  useEffect(() => {
    const fetchRunningApps = async () => {
      try {
        const start = performance.now();
        const apps = await invoke<AppMetadata[]>("get_all_applications");
        const end = performance.now();
        console.log(`Call took ${end - start}ms`);
        setRunningApps(apps);
      } catch (error) {
        console.error("Failed to fetch running apps:", error);
      }
    };

    fetchRunningApps();
  }, []);

  console.log("running", runningApps);

  return (
    <main className="container">
      <div className="h-screen flex flex-col overflow-hidden">
        <Header handleSearch={handleSearch} searchQuery={searchQuery} />
        <div className="overflow-auto">
          <h1>Running Applications</h1>
          <ul>
            {runningApps.map((app) => (
              <li key={app.name}>
                <Button
                  key={app.path}
                  onClick={async () =>
                    await invoke<AppMetadata[]>(
                      "launch_or_switch_to_application",
                      { app: app }
                    )
                  }
                >
                  <div className="flex flex-row items-center">
                    <img
                      src={app.icon}
                      alt={`${app.name} icon`}
                      className="w-8 h-8"
                    />
                    {app.name}
                  </div>
                </Button>
              </li>
              // <li key={app.path}>{app.name}</li>
            ))}
          </ul>
        </div>
        {/* <main className="flex-1 px-2 pt-4 overflow-auto scrollbar">
          {searchQuery.trim() === "" ? (
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
          ) : searchSections.length === 0 ? (
            // Show empty state if searching but no results
            <div className="flex h-full items-center justify-center">
              <EmptyState />
            </div>
          ) : (
            // Show search results
            sortedSections.map((section, sectionIndex) => (
              <div
                key={sectionIndex}
                className={`${sectionIndex > 0 ? "mt-6" : ""}`}
              >
                <h2 className="text-xs font-semibold text-muted-foreground mb-2">
                  {section.title}
                </h2>
                <SearchResults
                  section={section}
                  selectedItem={
                    sectionIndex === selectedSection ? selectedItem : -1
                  }
                  onSelect={(item, index) => {
                    setSelectedSection(sectionIndex);
                    setSelectedItem(index);
                    handleResultSelect(item, section.type);
                  }}
                  updatedApps={updatedApps}
                />
              </div>
            ))
          )}
        </main> */}
        <div className="sticky bottom-0">
          <Footer setIsSettingsOpen={setIsSettingsOpen} />
        </div>
        {/* <FolderSettings
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
        /> */}
      </div>
    </main>
  );
}
