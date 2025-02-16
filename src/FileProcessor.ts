import fs from "fs/promises";
import path from "path";
import Database from "better-sqlite3";
import { BrowserWindow } from "electron";
import { statSync } from "fs";
import fetch from "node-fetch";
import { getCategoryFromExtension } from "./lib/utils";
import { FileMetadata } from "./types";

export default class FileProcessor {
  private db: Database.Database;
  private mainWindow: BrowserWindow | null;
  private totalFiles: number = 0;
  private processedFiles: number = 0;

  constructor(db: Database.Database, mainWindow: BrowserWindow | null = null) {
    this.db = db;
    this.mainWindow = mainWindow;
  }

  /* processes the path provided by the user. The path can be a directory or a single file. If a directory, it will get all of the files in that directory and then index it and create an embedding from it and save it to the database. */
  public async processPaths(
    paths: string[]
  ): Promise<{ success: boolean; totalFiles: number }> {
    try {
      this.totalFiles = 0;
      this.processedFiles = 0;

      // collect all files from the provided paths.
      let allFiles: FileMetadata[] = [];

      for (const targetPath of paths) {
        if (this.isDirectory(targetPath)) {
          const files = await this.getAllFiles(targetPath);
          allFiles = allFiles.concat(files);
        } else {
          // If it's a file, get its stats.
          const stats = await fs.stat(targetPath);
          allFiles.push({
            path: targetPath,
            name: path.basename(targetPath),
            extension: path.extname(targetPath),
            size: stats.size,
            modified: stats.mtime.toISOString(),
          });
        }
      }

      this.totalFiles = allFiles.length;
      this.updateProgress();

      // Process each file by generating embedding and indexing metadata.
      for (const file of allFiles) {
        try {
          await this.processFile(file);
          this.processedFiles++;
          this.updateProgress();
        } catch (e) {
          console.error("Failed to process file:", file.path, e);
        }
      }
      return { success: true, totalFiles: this.processedFiles };
    } catch (error) {
      console.error("Error processing paths:", error);
      throw error;
    }
  }

  /**
   * Processes a single file: extracts text, generates an embedding, and updates SQLite.
   */
  private async processFile(file: FileMetadata): Promise<void> {
    const ext = path.extname(file.path);
    const category = getCategoryFromExtension(ext);
    const name = path.basename(file.path);
    const content = await this.extractText(file.path);
    let embedding: number[] = [];
    if (content) {
      embedding = await this.getEmbedding(content);
    }
    const embeddingJson = JSON.stringify(embedding);

    // Try to update first.
    const updateStmt = this.db.prepare(`
      UPDATE files 
      SET name = ?, category = ?, embedding = ?, updated_at = CURRENT_TIMESTAMP 
      WHERE path = ?
    `);

    const result = updateStmt.run(name, category, embeddingJson, file.path);
    if (result.changes === 0) {
      const insertStmt = this.db.prepare(`
        INSERT INTO files (path, name, category, embedding) VALUES (?, ?, ?, ?)
      `);
      insertStmt.run(file.path, name, category, embeddingJson);
    }
  }

  /**
   * Indexes a single file's metadata in SQLite.
   * This function does NOT generate an embedding; it only updates the file metadata.
   */
  public async indexFile(filePath: string): Promise<void> {
    const ext = path.extname(filePath);
    const category = getCategoryFromExtension(ext);
    const name = path.basename(filePath);
    const stats = await fs.stat(filePath);
    const updateStmt = this.db.prepare(`
        UPDATE files 
        SET name = ?, category = ?, updated_at = CURRENT_TIMESTAMP 
        WHERE path = ?
      `);
    const result = updateStmt.run(name, category, filePath);
    if (result.changes === 0) {
      const insertStmt = this.db.prepare(`
          INSERT INTO files (path, name, category) VALUES (?, ?, ?)
        `);
      insertStmt.run(filePath, name, category);
    }
  }

  /* 
  Creates an embedding for a single file and updates the database with that embedding
  */
  public async createEmbeddingForFile(filePath: string): Promise<number[]> {
    const content = await this.extractText(filePath);
    if (content) {
      return await this.getEmbedding(content);
    }
    return [];
  }

  // gets all of the files for a given directory path by recursively
  // searching through the directory
  private async getAllFiles(dirPath: string): Promise<FileMetadata[]> {
    const files: FileMetadata[] = [];

    try {
      const entries = await fs.readdir(dirPath, {
        withFileTypes: true,
      });

      for (const entry of entries) {
        const fullPath = path.join(dirPath, entry.name);

        if (entry.isDirectory()) {
          // Recursively get files from subdirectories
          const subFiles = await this.getAllFiles(fullPath);
          files.push(...subFiles);
        } else {
          const stats = await fs.stat(fullPath);
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

  /**
   * Extracts text from a file. Currently supports only .txt files.
   */
  private async extractText(filePath: string): Promise<string> {
    const ext = path.extname(filePath).toLowerCase();
    if (ext === ".txt") {
      return await fs.readFile(filePath, "utf-8");
    }
    // TODO: Add support for PDF, DOCX, etc.
    return "";
  }

  /**
   * Calls the Python microservice to generate an embedding from text.
   */
  private async getEmbedding(text: string): Promise<number[]> {
    const response = await fetch("http://127.0.0.1:8000/embed", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ text }),
    });
    if (!response.ok) {
      throw new Error("Failed to get embedding");
    }
    const data = await response.json();
    return data.embedding;
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

  // returns a boolean if the provided path is a directory
  public isDirectory(path: string): boolean {
    if (!path || typeof path !== "string") {
      return false;
    }

    try {
      return statSync(path).isDirectory();
    } catch (error) {
      if (
        error instanceof Error &&
        "code" in error &&
        error.code === "ENOENT"
      ) {
        return false;
      }
      throw error;
    }
  }
}
