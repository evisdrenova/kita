export interface Section {
  id: number;
  name: string;
  icon: JSX.Element;
  component: JSX.Element;
  counts?: number;
  getLimitedComponent?: (limit: number) => JSX.Element;
}

interface BaseMetadata {
  id?: number;
  name: string;
  path: string;
}

export interface FileMetadata extends BaseMetadata {
  extension: string;
  size: number;
  updated_at?: string;
  created_at?: string;
}

export interface AppMetadata extends BaseMetadata {
  pid: number;
  resource_usage?: AppResourceUsage;
  icon?: string;
}

export interface SemanticMetadata extends BaseMetadata {
  extension: string;
  distance: number;
  content?: string;
  size: number;
}

export interface AppResourceUsage {
  pid: number;
  cpu_usage: number;
  memory_bytes: number;
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

export interface IndexingProgress {
  total: number;
  processed: number;
  percentage: number;
}

export interface Column<T> {
  key: string;
  header: string;
  width?: number;
  render?: (item: T) => React.ReactNode;
}

export interface Model {
  id: string;
  name: string;
  size: number; // Size in MB
  quantization: string;
  is_downloaded: boolean;
}

export interface AppSettings {
  theme?: string;
  custom_model_path?: string;
  selected_model_id?: string;
  window_width?: number;
  window_height?: number;
  global_hotkey?: string;
  index_concurrency?: number;
  selected_categories?: string[];
}

export interface ChatMessage {
  role: "user" | "assistant";
  content: string;
  sources?: string[];
}
