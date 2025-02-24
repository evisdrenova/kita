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
  icon?: string;
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

export interface IndexingProgress {
  total: number;
  processed: number;
  percentage: number;
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

export type SearchCategory = (typeof searchCategories)[number];

export const searchCategories = [
  "Applications",
  "Documents",
  "Folders",
  "Images",
  "Mail",
  "Messages",
  "Other",
  "PDF Documents",
  "Spreadsheets",
] as const;

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
