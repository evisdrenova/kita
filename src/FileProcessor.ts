import fs from "fs/promises";
import path from "path";
import Database from "better-sqlite3";
import { BrowserWindow } from "electron";
import { statSync } from "fs";
import fetch from "node-fetch";
import { getCategoryFromExtension } from "./lib/utils";
import { FileMetadata, SearchSectionType } from "./types";
import mammoth from "mammoth";
import pdfParse from "pdf-parse";
import textract from "textract";

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
            type: SearchSectionType.Files,
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

    if (!content) {
      return;
    }

    try {
      // First check if file exists in database
      const checkStmt = this.db.prepare("SELECT id FROM files WHERE path = ?");
      const existingFile = checkStmt.get(file.path) as
        | { id: number }
        | undefined;

      if (existingFile) {
        // File exists, update file metadata
        const updateFileStmt = this.db.prepare(`
          UPDATE files 
          SET name = ?, 
              category = ?, 
              updated_at = CURRENT_TIMESTAMP 
          WHERE id = ?
        `);

        updateFileStmt.run(name, category, existingFile.id);

        // Generate new embedding and update embeddings table
        const embedding = await this.addFileEmbedding(existingFile.id, content);
        const embeddingJson = JSON.stringify(embedding);

        const updateEmbeddingStmt = this.db.prepare(`
          INSERT OR REPLACE INTO embeddings (
            file_id, 
            embedding, 
            updated_at
          ) VALUES (?, ?, CURRENT_TIMESTAMP)
        `);

        updateEmbeddingStmt.run(existingFile.id, embeddingJson);
      } else {
        // File doesn't exist, insert file first to get ID
        const insertFileStmt = this.db.prepare(`
          INSERT INTO files (
            path, 
            name, 
            category, 
            extension,
            created_at,
            updated_at
          ) VALUES (?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
        `);

        const result = insertFileStmt.run(
          file.path,
          name,
          category,
          file.extension
        );
        const newFileId = result.lastInsertRowid as number;

        // Now generate embedding and insert into embeddings table
        const embedding = await this.addFileEmbedding(newFileId, content);
        const embeddingJson = JSON.stringify(embedding);

        const insertEmbeddingStmt = this.db.prepare(`
          INSERT INTO embeddings (
            file_id, 
            embedding,
            created_at,
            updated_at
          ) VALUES (?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
        `);

        insertEmbeddingStmt.run(newFileId, embeddingJson);
      }
    } catch (error) {
      console.error(`Error processing file ${file.path}:`, error);
      throw error;
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
            type: SearchSectionType.Files,
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

    // Extensions we can treat as plain text.
    const plainTextExtensions = new Set([
      ".txt",
      ".js",
      ".ts",
      ".jsx",
      ".tsx",
      ".py",
      ".java",
      ".cpp",
      ".html",
      ".css",
      ".json",
      ".xml",
      ".yaml",
      ".yml",
    ]);

    if (plainTextExtensions.has(ext)) {
      return this.extractTextFromPlain(filePath);
    } else if (ext === ".docx") {
      return this.extractTextFromDocx(filePath);
    } else if (ext === ".pdf") {
      return this.extractTextFromPDF(filePath);
    } else if (ext === ".doc" || ext === ".rtf") {
      return this.extractTextFromDocOrRtf(filePath);
    }

    // If extension is not supported, return an empty string.
    return "";
  }

  async extractTextFromDocOrRtf(filePath: string): Promise<string> {
    return new Promise<string>((resolve, reject) => {
      textract.fromFileWithPath(filePath, (err, text) => {
        if (err) {
          console.error(`Error extracting DOC/RTF file ${filePath}:`, err);
          reject(err);
        } else {
          resolve(text);
        }
      });
    }).catch(() => "");
  }

  async extractTextFromPDF(filePath: string): Promise<string> {
    try {
      const dataBuffer = await fs.readFile(filePath);
      const data = await pdfParse(dataBuffer);
      return data.text;
    } catch (error) {
      console.error(`Error extracting PDF file ${filePath}:`, error);
      return "";
    }
  }

  async extractTextFromDocx(filePath: string): Promise<string> {
    try {
      const result = await mammoth.extractRawText({ path: filePath });
      return result.value;
    } catch (error) {
      console.error(`Error extracting DOCX file ${filePath}:`, error);
      return "";
    }
  }

  async extractTextFromPlain(filePath: string): Promise<string> {
    try {
      return await fs.readFile(filePath, "utf-8");
    } catch (error) {
      console.error(`Error reading plain text file ${filePath}:`, error);
      return "";
    }
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

  /**
   * Generates an embedding for the given text and then adds or updates that file's embedding
   * in the vector index via the /add_file endpoint.
   *
   * @param fileId - A unique identifier for the file (managed by your app)
   * @param text - The text content extracted from the file
   * @returns The embedding vector as an array of numbers
   */
  private async addFileEmbedding(
    fileId: number,
    text: string
  ): Promise<number[]> {
    const embedding = await this.getEmbedding(text);

    // update the vector index
    const response = await fetch("http://127.0.0.1:8000/add_file", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ file_id: fileId ?? 0, embedding }),
    });
    if (!response.ok) {
      throw new Error("Failed to add file embedding");
    }
    return embedding;
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
