import OpenAI from "openai";
import Anthropic from "@anthropic-ai/sdk";
import { SettingsValue } from "../../src/settings/Settings";
import { CoreMessage } from "ai";

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

  minimizeWindow: () => void;
  maximizeWindow: () => void;
  closeWindow: () => void;
}
export interface FileMetadata {
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
declare global {
  interface Window {
    electron: IElectronAPI;
  }
}
