import fs from "fs/promises";
import path from "path";
import fetch from "node-fetch";
import { getCategoryFromExtension } from "./lib/utils";

export async function extractText(filePath: string): Promise<string> {
  const ext = path.extname(filePath).toLowerCase();
  if (ext === ".txt") {
    return fs.readFile(filePath, "utf-8");
  }
  // For PDFs, DOCX, etc., you would use specialized libraries.
  return "";
}

export async function getEmbedding(text: string): Promise<number[]> {
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

export async function processFile(filePath: string, db: any): Promise<void> {
  const ext = path.extname(filePath);
  const category = getCategoryFromExtension(ext);
  const name = path.basename(filePath);
  const content = await extractText(filePath);
  let embedding: number[] = [];
  if (content) {
    embedding = await getEmbedding(content);
  }
  // Insert or update SQLite record.
  const embeddingJson = JSON.stringify(embedding);
  // Try to update first.
  const updateStmt = db.prepare(`
      UPDATE files SET name = ?, embedding = ? WHERE path = ?
    `);
  const result = updateStmt.run(name, category, embeddingJson, filePath);
  if (result.changes === 0) {
    // Insert if update didn't change anything.
    const insertStmt = db.prepare(`
        INSERT INTO files (path, name, embedding) VALUES (?, ?, ?)
      `);
    insertStmt.run(filePath, name, category, embeddingJson);
  }
}
