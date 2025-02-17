import {
  app,
  BrowserWindow,
  ipcMain,
  Menu,
  globalShortcut,
  dialog,
  shell,
} from "electron";
import path from "path";
import started from "electron-squirrel-startup";
import Database from "better-sqlite3";
import log from "electron-log/main";
import FileProcessor from "./FileProcessor";
import { exec } from "child_process";
import {
  AppMetadata,
  DBResult,
  EmbeddingSearchResults,
  FileMetadata,
  SearchSection,
  SearchSectionType,
} from "./types";
import AppHandler from "./AppHandler";

if (started) {
  app.quit();
}

let db: Database.Database;
log.initialize();
let mainWindow: BrowserWindow | null = null;
let appHandler: AppHandler;

const initializeDatabase = () => {
  try {
    const dbPath = path.join(app.getPath("userData"), "kita-database.sqlite");

    db = new Database(dbPath);

    db.exec(`
      CREATE TABLE IF NOT EXISTS files (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        path TEXT UNIQUE,
        name TEXT,
        extension TEXT,
        size INTEGER,
        category TEXT,
        created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
        updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
      )
    `);

    db.exec(`
      CREATE TABLE IF NOT EXISTS embeddings (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        file_id INTEGER NOT NULL,
        embedding TEXT NOT NULL,
        created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
        updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
        FOREIGN KEY (file_id) REFERENCES files(id) ON DELETE CASCADE
      )
    `);

    db.exec(`
      CREATE TABLE IF NOT EXISTS recents (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        path TEXT UNIQUE,
        lastClicked DATETIME DEFAULT CURRENT_TIMESTAMP
);
      `);

    return db;
  } catch (error) {
    log.error("Failed to initialize database:", error);
    throw error;
  }
};

const createWindow = async () => {
  try {
    db = initializeDatabase();

    mainWindow = new BrowserWindow({
      width: 600,
      height: 500,
      frame: false,
      icon: path.join(__dirname, "../../assets/kita_icon_margin.icns"),
      webPreferences: {
        nodeIntegration: false,
        contextIsolation: true,
        preload: path.join(__dirname, "preload.js"),
        devTools: true,
      },
    });

    if (process.platform === "darwin") {
      const dockIcon = path.join(__dirname, "../../assets/kita_margin.png");
      app.dock.setIcon(dockIcon);
    }

    const hotkey = "Command+Shift+Space";

    const registered = globalShortcut.register(hotkey, () => {
      if (!mainWindow) return;

      // Toggle the visibility of the window
      if (mainWindow.isVisible()) {
        mainWindow.hide();
      } else {
        mainWindow.show();
        mainWindow.focus();
      }
    });

    if (!registered) {
      console.error(`Failed to register global hotkey: ${hotkey}`);
    } else {
      console.log(`Global hotkey (${hotkey}) registered successfully.`);
    }

    // and load the index.html of the app.
    if (MAIN_WINDOW_VITE_DEV_SERVER_URL) {
      mainWindow.loadURL(MAIN_WINDOW_VITE_DEV_SERVER_URL);
    } else {
      mainWindow.loadFile(
        path.join(__dirname, `../renderer/${MAIN_WINDOW_VITE_NAME}/index.html`)
      );
    }

    // Move DevTools opening after window load
    mainWindow.webContents.once("did-finish-load", () => {
      mainWindow.webContents.openDevTools({ mode: "detach" });
    });

    // Context menu for inspect element
    mainWindow.webContents.on("context-menu", (event, params) => {
      const menu = Menu.buildFromTemplate([
        {
          label: "Inspect Element",
          click: () => {
            mainWindow.webContents.inspectElement(params.x, params.y);
          },
        },
        {
          label: "Toggle DevTools",
          click: () => {
            mainWindow.webContents.toggleDevTools();
          },
        },
      ]);
      menu.popup();
    });
    appHandler = new AppHandler(mainWindow);
  } catch (error) {
    log.error("Failed to create window:", error);
    app.quit();
  }
};

ipcMain.on("window-minimize", () => {
  mainWindow.minimize();
});

ipcMain.on("window-maximize", () => {
  if (mainWindow.isMaximized()) {
    mainWindow.unmaximize();
  } else {
    mainWindow.maximize();
  }
});

ipcMain.on("window-close", () => {
  mainWindow.close();
});

ipcMain.handle("index-and-embed-paths", async (_, directories: string[]) => {
  try {
    const processor = new FileProcessor(db, mainWindow);
    return await processor.processPaths(directories);
  } catch (error) {
    console.error("Error indexing directories:", error);
    throw error;
  }
});

ipcMain.handle(
  "dialog:selectPaths",
  async (_, options: Electron.OpenDialogOptions) => {
    const result = await dialog.showOpenDialog({
      properties: ["openFile", "openDirectory", "multiSelections"],
      title: "Select Files and Folders",
      buttonLabel: "Select",
      filters: options.filters || [],
      ...options,
    });
    return result;
  }
);
ipcMain.handle(
  "search-files-and-embeddings",
  async (_, query: string): Promise<SearchSection[]> => {
    try {
      // get apps
      const apps = await appHandler.getAllApps(query);

      // text-based search
      const textStmt = db.prepare(`
        SELECT 
          id,
          name,
          path,
          extension,
          size,
          created_at,
          updated_at
        FROM files 
        WHERE name LIKE ? 
           OR path LIKE ? 
        LIMIT 50
      `);

      const searchPattern = `%${query}%`;
      const fileResults = textStmt.all(
        searchPattern,
        searchPattern
      ) as FileMetadata[];

      // embedding search
      const response = await fetch("http://127.0.0.1:8000/search", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ query, k: 1 }),
      });
      if (!response.ok) throw new Error("Semantic search failed");
      const embeddingResponse =
        (await response.json()) as EmbeddingSearchResults;

      // For each result from the embedding search, query SQLite for metadata
      // Updated to join files and embeddings tables
      const embedStmt = db.prepare(`
        SELECT 
          f.id,
          f.path,
          f.name,
          f.category,
          f.extension,
          f.size,
          f.created_at,
          f.updated_at,
          e.embedding
        FROM files f
        LEFT JOIN embeddings e ON f.id = e.file_id
        WHERE f.id = ?
      `);

      const semanticResults = embeddingResponse.results
        .map((result) => {
          const fileRow = embedStmt.get(result.file_id) as FileMetadata;
          if (!fileRow) return null;
          return {
            ...fileRow,
            distance: result.distance,
          };
        })
        .filter(
          (result): result is NonNullable<typeof result> => result !== null
        );

      // create search sections and return
      const sections: SearchSection[] = [];
      if (fileResults.length > 0) {
        sections.push({
          type: SearchSectionType.Files,
          title: "File Name Matches",
          items: fileResults,
        });
      }
      if (semanticResults.length > 0) {
        sections.push({
          type: SearchSectionType.Semantic,
          title: "Semantic Matches",
          items: semanticResults,
        });
      }

      if (apps.length > 0) {
        sections.push({
          type: SearchSectionType.Apps,
          title: "Applications",
          items: apps,
        });
      }

      return sections;
    } catch (error) {
      console.error("Error in combined-search:", error);
      throw error;
    }
  }
);

ipcMain.handle(
  "query-embeddings",
  async (_, query: string): Promise<SearchSection[]> => {
    try {
      // Get search results from the Python microservice.
      const response = await fetch("http://127.0.0.1:8000/search", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ query, k: 5 }),
      });
      if (!response.ok) throw new Error("Search failed");
      const searchResponse = await response.json();

      // Now, for each result, query SQLite to get metadata with joined embeddings
      const stmt = db.prepare(`
        SELECT 
          f.id,
          f.path,
          f.name,
          f.category,
          f.extension,
          f.size,
          f.created_at,
          f.updated_at,
          e.embedding
        FROM files f
        LEFT JOIN embeddings e ON f.id = e.file_id
        WHERE f.id = ?
      `);

      const matchedFiles = searchResponse.results
        .map((result: any) => {
          const fileRow = stmt.get(result.file_id) as FileMetadata;
          if (!fileRow) return null;
          return {
            ...fileRow,
            distance: result.distance,
          };
        })
        .filter(
          (result: any): result is NonNullable<typeof result> => result !== null
        );

      return [
        {
          type: SearchSectionType.Files,
          title: "Files",
          items: matchedFiles,
        },
      ];
    } catch (error) {
      console.error("Error searching:", error);
      throw error;
    }
  }
);
ipcMain.handle(
  "launch-or-switch",
  async (_, appInfo: AppMetadata): Promise<boolean> => {
    try {
      return await appHandler.launchOrSwitchToApp(appInfo);
    } catch (error) {
      console.error("Error launching/switching app:", error);
      return false;
    }
  }
);

ipcMain.handle("open-file", async (_, filePath: string) => {
  try {
    await shell.openPath(filePath);
    // Insert or update the recents table to track recents
    const stmt = db.prepare(`
      INSERT INTO recents (path, lastClicked)
      VALUES (?, CURRENT_TIMESTAMP)
      ON CONFLICT(path) DO UPDATE SET lastClicked = CURRENT_TIMESTAMP;
    `);
    stmt.run(filePath);
    return true;
  } catch (error) {
    console.error("Error opening file:", error);
    return false;
  }
});

ipcMain.handle("get-recents", async () => {
  try {
    // First, try to get recents from the database.
    const stmt = db.prepare(`
      SELECT path, lastClicked
      FROM recents
      ORDER BY lastClicked DESC
      LIMIT 50
    `);

    const results = stmt.all();

    if (results && results.length > 0) {
      return results.map((row: any) => ({
        path: row.path,
        name: path.basename(row.path),
        extension: path.extname(row.path),
      }));
    }

    // If no recents are stored, query files used in the last 7 days (for example).
    const sevenDaysAgo = new Date();
    sevenDaysAgo.setDate(sevenDaysAgo.getDate() - 7);
    const isoDate = sevenDaysAgo.toISOString();

    // This query finds files whose last used date is later than ISO date.
    const query = `mdfind 'kMDItemLastUsedDate >= "${isoDate}"' | head -n 50`;
    const recents: string[] = await new Promise((resolve, reject) => {
      exec(query, (error, stdout) => {
        if (error) {
          reject(error);
        } else {
          const paths = stdout.trim().split("\n");
          resolve(paths);
        }
      });
    });

    return recents.map((filePath) => ({
      path: filePath,
      name: path.basename(filePath),
      extension: path.extname(filePath),
    }));
  } catch (error) {
    console.error("Error getting recents:", error);
    throw error;
  }
});

// called when Electron has initialized and is ready to create browser windows.
app.on("ready", createWindow);

// Quit when all windows are closed, except on macOS. There, it's common
// for applications and their menu bar to stay active until the user quits
// explicitly with Cmd + Q.
app.on("window-all-closed", () => {
  // if (process.platform !== "darwin") {
  // }
});

app.on("will-quit", () => {
  if (db) {
    db.close();
    globalShortcut.unregisterAll();
  }
  globalShortcut.unregisterAll();
});

app.on("activate", () => {
  // On OS X it's common to re-create a window in the app when the
  // dock icon is clicked and there are no other windows open.
  if (BrowserWindow.getAllWindows().length === 0) {
    createWindow();
  }
});
