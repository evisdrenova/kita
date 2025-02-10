import OpenAI from "openai";
import Anthropic from "@anthropic-ai/sdk";
import { SettingsValue } from "../../src/settings/Settings";
import { CoreMessage } from "ai";
import { searchCategories } from "../../src/pages/Home";

// always returns a promise since the IPC communication is async even if the underlying implementation is synchronous
export interface IElectronAPI {
  // settings methods
  getSettings: <T extends SettingsValue>(key: string) => Promise<T | undefined>;
  setSettings: (key: string, value: SettingsValue) => Promise<boolean>;
  getAllSettings: () => Promise<Record<string, SettingsValue>>;
  setMultipleSettings: (
    settings: Record<string, SettingsValue>
  ) => Promise<boolean>;
  indexDirectories: (directories: string[]) => Promise<{
    success: boolean;
    totalFiles: number;
  }>;
  selectDirectory: () => Promise<DirectorySelectionResult>;
  onIndexingProgress: (
    callback: (event: any, progress: IndexingProgress) => void
  ) => void;
  removeIndexingProgress: (
    callback: (event: any, progress: IndexingProgress) => void
  ) => void;
  searchFiles: (query: string) => Promise<FileMetadata[]>;
  openFile: (filePath: string) => Promise<boolean>;
  minimizeWindow: () => void;
  maximizeWindow: () => void;
  closeWindow: () => void;
}
export interface FileMetadata {
  id: number;
  path: string;
  name: string;
  extension: string;
  size: number;
  modified: string;
}

export interface IndexingProgress {
  total: number;
  processed: number;
  percentage: number;
}

export interface DirectorySelectionResult {
  canceled: boolean;
  filePaths: string[];
}

export interface SearchResult {
  id: number;
  title: string; // this will be the file name
  category: SearchCategory; // this will be based on file extension
  path: string;
  size: number;
  modified: string;
  icon?: React.ReactNode;
}

export type SearchCategory = (typeof searchCategories)[number];

declare global {
  interface Window {
    electron: IElectronAPI;
  }
}
