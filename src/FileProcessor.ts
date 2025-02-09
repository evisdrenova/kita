import fs from "fs";
import path from "path";
import Database from "better-sqlite3";
import { BrowserWindow } from "electron";

interface FileMetadata {
  path: string;
  name: string;
  extension: string;
  size: number;
  modified: string;
}

export default class FileProcessor {
  private db: Database.Database;
  private mainWindow: BrowserWindow | null;
  private totalFiles: number = 0;
  private processedFiles: number = 0;

  constructor(db: Database.Database, mainWindow: BrowserWindow | null = null) {
    this.db = db;
    this.mainWindow = mainWindow;
  }

  public async processDirectories(
    directories: string[]
  ): Promise<{ success: boolean; totalFiles: number }> {
    try {
      const stmt = this.db.prepare(`
        INSERT OR REPLACE INTO files (path, name, extension, size, modified)
        VALUES (?, ?, ?, ?, ?)
      `);

      // Create transaction for bulk inserts
      const insertMany = this.db.transaction((files: FileMetadata[]) => {
        for (const file of files) {
          stmt.run(
            file.path,
            file.name,
            file.extension,
            file.size,
            file.modified
          );
          this.processedFiles++;
          this.updateProgress();
        }
      });

      // First pass to count total files
      for (const directory of directories) {
        const files = await this.getAllFiles(directory);
        this.totalFiles += files.length;
      }

      // Reset processed count before actual processing
      this.processedFiles = 0;

      // Second pass to process files
      for (const directory of directories) {
        const files = await this.getAllFiles(directory);
        insertMany(files);
      }

      return { success: true, totalFiles: this.processedFiles };
    } catch (error) {
      console.error("Error processing directories:", error);
      throw error;
    }
  }

  private async getAllFiles(dirPath: string): Promise<FileMetadata[]> {
    const files: FileMetadata[] = [];

    try {
      const entries = await fs.promises.readdir(dirPath, {
        withFileTypes: true,
      });

      for (const entry of entries) {
        const fullPath = path.join(dirPath, entry.name);

        if (entry.isDirectory()) {
          // Recursively get files from subdirectories
          const subFiles = await this.getAllFiles(fullPath);
          files.push(...subFiles);
        } else {
          const stats = await fs.promises.stat(fullPath);
          files.push({
            path: fullPath,
            name: entry.name,
            extension: path.extname(entry.name),
            size: stats.size,
            modified: stats.mtime.toISOString(),
          });
        }
      }
    } catch (error) {
      console.error(`Error reading directory ${dirPath}:`, error);
    }

    return files;
  }

  private updateProgress(): void {
    if (this.mainWindow) {
      this.mainWindow.webContents.send("indexing-progress", {
        total: this.totalFiles,
        processed: this.processedFiles,
        percentage: Math.round((this.processedFiles / this.totalFiles) * 100),
      });
    }
  }
}
