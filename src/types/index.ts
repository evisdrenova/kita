import { SettingsValue } from "../../src/settings/Settings";
import { searchCategories } from "../../src/pages/Home";
import { IpcRendererEvent } from "electron";

// always returns a promise since the IPC communication is async even if the underlying implementation is synchronous
export interface IElectronAPI {
  // settings methods
  getSettings: <T extends SettingsValue>(key: string) => Promise<T | undefined>;
  setSettings: (key: string, value: SettingsValue) => Promise<boolean>;
  getAllSettings: () => Promise<Record<string, SettingsValue>>;
  setMultipleSettings: (
    settings: Record<string, SettingsValue>
  ) => Promise<boolean>;
  indexAndEmbedPaths: (directories: string[]) => Promise<{
    success: boolean;
    totalFiles: number;
  }>;
  selectPaths: (options: SelectPathsOptions) => Promise<SelectPathsResult>;
  onIndexingProgress: (
    callback: (event: any, progress: IndexingProgress) => void
  ) => void;
  removeIndexingProgress: (
    callback: (event: any, progress: IndexingProgress) => void
  ) => void;
  searchFiles: (query: string) => Promise<SearchSection[]>;
  searchFilesAndEmbeddings: (query: string) => Promise<SearchSection[]>;
  launchOrSwitch: (appInfo: AppMetadata) => Promise<boolean>;
  openFile: (filePath: string) => Promise<boolean>;
  minimizeWindow: () => void;
  maximizeWindow: () => void;
  closeWindow: () => void;
  onResourceUsageUpdated: (
    callback: (event: IpcRendererEvent, updatedApps: AppMetadata[]) => void
  ) => void;
  removeResourceUsageUpdated: (
    callback: (event: IpcRendererEvent, updatedApps: AppMetadata[]) => void
  ) => void;
  getRecents: () => Promise<FileMetadata[]>;
}

interface BaseMetadata {
  id?: number;
  name: string;
  path: string;
}

export interface FileMetadata extends BaseMetadata {
  type: SearchSectionType.Files;
  extension: string;
  size: number;
  updated_at?: string;
  created_at?: string;
}

export interface AppMetadata extends BaseMetadata {
  type: SearchSectionType.Apps;
  isRunning: boolean;
  memoryUsage?: number;
  cpuUsage?: number;
  iconDataUrl?: string;
}

export interface SemanticMetadata extends BaseMetadata {
  type: SearchSectionType.Semantic;
  extension: string;
  distance: number;
  content?: string;
}

export type SearchItem = FileMetadata | AppMetadata | SemanticMetadata;

export enum SearchSectionType {
  Apps = "apps",
  Files = "files",
  Semantic = "semantic",
}

export interface SearchSection {
  type: SearchSectionType;
  title: string;
  items: SearchItem[];
}

export interface IndexingProgress {
  total: number;
  processed: number;
  percentage: number;
}

export interface SelectPathsOptions {
  properties: Array<"openFile" | "openDirectory" | "multiSelections">;
  title?: string;
  buttonLabel?: string;
  filters?: Array<{
    name: string;
    extensions: string[];
  }>;
}

interface SelectPathsResult {
  canceled: boolean;
  filePaths: string[];
}

export interface SearchResult {
  id: number;
  title: string; // this will be the file name
  category: SearchCategory; // this will be based on file extension
  path: string;
  size: number;
  updated_at?: string;
  created_at?: string;
  icon?: React.ReactNode;
}

export interface EmbeddingSearchResults {
  results: EmbeddingSearchResult[];
}

export interface EmbeddingSearchResult {
  file_id: number;
  distance: number;
}

export interface DBResult {
  id: number;
  name: string;
  path: string;
  extension: string;
  size: number;
  updated_at?: string;
  created_at?: string;
}

export interface RecentDbResult {
  id: number;
  path: string;
  lastClicked: string;
}

export type SearchCategory = (typeof searchCategories)[number];

declare global {
  interface Window {
    electron: IElectronAPI;
  }
}
