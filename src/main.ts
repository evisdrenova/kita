import {
  app,
  BrowserWindow,
  ipcMain,
  Menu,
  MenuItem,
  globalShortcut,
  dialog,
  shell,
  nativeImage,
} from "electron";
import path from "path";
import started from "electron-squirrel-startup";
import Database from "better-sqlite3";
import log from "electron-log/main";
import FileProcessor from "./FileProcessor";
import { exec } from "child_process";
import { AppInfo, DBResult, FileMetadata, SearchSection } from "./types";
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
        modified TEXT,
        created_at DATETIME DEFAULT CURRENT_TIMESTAMP
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
      icon: path.join(__dirname, "../assets/kita_logo.icns"),
      webPreferences: {
        nodeIntegration: false,
        contextIsolation: true,
        preload: path.join(__dirname, "preload.js"),
      },
    });

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

    // Open the DevTools.
    mainWindow.webContents.openDevTools();
    mainWindow.webContents.on("context-menu", (event, params) => {
      const menu = new Menu();
      menu.append(
        new MenuItem({
          label: "Inspect Element",
          click: () => {
            mainWindow.webContents.inspectElement(params.x, params.y);
          },
        })
      );
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

ipcMain.handle("index-directories", async (_, directories: string[]) => {
  try {
    const processor = new FileProcessor(db, mainWindow);
    return await processor.processDirectories(directories);
  } catch (error) {
    console.error("Error indexing directories:", error);
    throw error;
  }
});

ipcMain.handle("dialog:selectDirectory", async () => {
  const result = await dialog.showOpenDialog({
    properties: ["openDirectory"],
  });
  return result;
});

ipcMain.handle(
  "search-files",
  async (_, query: string): Promise<SearchSection[]> => {
    try {
      const apps = await appHandler.getAllApps(query);

      // Get matching files from database with proper typing
      const stmt = db.prepare(`
      SELECT 
        id,
        name,
        path,
        extension,
        size,
        modified
      FROM files 
      WHERE name LIKE ? 
      OR path LIKE ? 
      LIMIT 50
    `);

      const searchPattern = `%${query}%`;
      const dbResults = stmt.all(searchPattern, searchPattern) as DBResult[];

      // Explicitly type the database results
      const files = dbResults.map(
        (row: any): FileMetadata => ({
          id: row.id,
          name: row.name,
          path: row.path,
          extension: row.extension,
          size: row.size,
          modified: row.modified,
        })
      );

      // Return organized sections with proper typing
      const sections: SearchSection[] = [];

      if (apps.length > 0) {
        sections.push({
          type: "apps",
          title: "Applications",
          items: apps,
        });
      }

      if (files.length > 0) {
        sections.push({
          type: "files",
          title: "Files",
          items: files,
        });
      }

      return sections;
    } catch (error) {
      console.error("Error searching:", error);
      throw error;
    }
  }
);

ipcMain.handle(
  "launch-or-switch",
  async (_, appInfo: AppInfo): Promise<boolean> => {
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

// ipcMain.handle("db-get-setting", async (_event, key: string) => {
//   return settingManager.get(key);
// });

// ipcMain.handle(
//   "db-set-setting",
//   async (_event, key: string, value: SettingsValue) => {
//     return settingManager.set(key, value);
//   }
// );

// ipcMain.handle("db-get-all-settings", async () => {
//   return settingManager.getAll();
// });

// ipcMain.handle(
//   "db-set-multiple-settings",
//   async (_event, settings: Record<string, SettingsValue>) => {
//     return settingManager.setMultiple(settings);
//   }
// );

// ipcMain.handle("set-user", (_, user: User) => {
//   const stmt = db.prepare(`
//     INSERT into user (name) values (?)`);

//   return stmt.run(user.name);
// });

// ipcMain.handle("get-user", () => {
//   try {
//     const stmt = db.prepare(`SELECT id, name from user`);
//     const user = stmt.get();
//     return user;
//   } catch (error) {
//     throw error;
//   }
// });

// ipcMain.handle("get-providers", () => {
//   try {
//     const stmt = db.prepare(
//       "SELECT id, name, type, baseUrl, apiPath, apiKey, model, config FROM providers"
//     );
//     return stmt.all();
//   } catch (error) {
//     throw error;
//   }
// });

// ipcMain.handle("add-provider", (_, provider: Provider) => {
//   try {
//     const stmt = db.prepare(
//       "INSERT INTO providers (name, type, baseUrl, apiPath, apiKey, model, config) VALUES (?, ?, ?, ?, ?, ?, ?)"
//     );
//     return stmt.run(
//       provider.name,
//       provider.type,
//       provider.baseUrl,
//       provider.apiPath,
//       provider.apiKey,
//       provider.model,
//       provider.config
//     );
//   } catch (error) {
//     console.log("unable to create new provider", error);
//     throw new error("unable to create new provider");
//   }
// });

// ipcMain.handle("delete-provider", (_, id: number) => {
//   try {
//     const stmt = db.prepare("DELETE FROM providers WHERE id = ?");
//     const result = stmt.run(id);
//     if (result.changes === 0) {
//       throw new Error(`No provider found with id ${id}`);
//     }
//     return result;
//   } catch (error) {
//     console.error("Error deleting provider:", error);
//     throw error;
//   }
// });

// ipcMain.handle("update-provider", async (_, provider: Provider) => {
//   try {
//     const stmt = db.prepare(
//       "UPDATE providers SET name = ?, type = ?, baseUrl = ?, apiPath = ?, apiKey = ?, model = ?, config = ? WHERE id = ?"
//     );
//     return stmt.run(
//       provider.name,
//       provider.type,
//       provider.baseUrl,
//       provider.apiPath,
//       provider.apiKey,
//       provider.model,
//       provider.config,
//       provider.id
//     );
//   } catch (error) {
//     console.log("unable to update provider", error);
//     throw new Error("unable to update provider");
//   }
// });

// ipcMain.handle("select-provider", (_, provider: Provider) => {
//   providers.setProvider(provider);
//   return true;
// });

// ipcMain.handle("get-servers", () => {
//   return mcp.getServers();
// });

// ipcMain.handle("add-server", async (_, config: ServerConfig) => {
//   try {
//     // Install the server and get updated config
//     const server = await mcp.serverManager.installServer(config, db);

//     // Save to database
//     const stmt = db.prepare(`
//       INSERT INTO servers (
//         name,
//         description,
//         installType,
//         package,
//         startCommand,
//         args,
//         version,
//         enabled
//       ) VALUES (?,?,?,?,?,?,?,?)
//     `);

//     const result = stmt.run(
//       server.name,
//       server.description || null,
//       server.installType,
//       server.package,
//       server.startCommand || null,
//       JSON.stringify(server.args),
//       server.version || null,
//       server.enabled ? 1 : 0
//     );

//     if (result.changes === 0) {
//       throw new Error("Failed to save server to database");
//     }

//     return result.lastInsertRowid;
//   } catch (error) {
//     log.error("Error adding server:", error);
//     throw error;
//   }
// });

// ipcMain.handle("delete-server", async (_, id: number) => {
//   try {
//     const getStmt = db.prepare("SELECT name FROM servers WHERE id = ?");
//     const server = getStmt.get(id) as { name: string } | undefined;

//     if (!server) {
//       throw new Error(`No server found with id ${id}`);
//     }

//     // Clean up the server
//     await mcp.serverManager.cleanupServer(id, server.name);

//     // Delete from database
//     const deleteStmt = db.prepare("DELETE FROM servers WHERE id = ?");
//     const result = deleteStmt.run(id);

//     if (result.changes === 0) {
//       throw new Error(`Failed to delete server from database`);
//     }

//     return result;
//   } catch (error) {
//     log.error("Error deleting server:", error);
//     throw error;
//   }
// });

// ipcMain.handle("update-server", (_, config: ServerConfig) => {
//   const stmt = db.prepare(`
//     UPDATE servers SET
//       name = ?,
//       description = ?,
//       installType = ?,
//       package = ?,
//       startCommand = ?,
//       args = ?,
//       version = ?,
//       enabled = ?
//     WHERE id = ?
//   `);

//   const args = Array.isArray(config.args) ? JSON.stringify(config.args) : "[]";
//   const result = stmt.run(
//     config.name,
//     config.description || null,
//     config.installType,
//     config.package,
//     config.startCommand || null,
//     args,
//     config.version || null,
//     config.enabled ? 1 : 0,
//     config.id
//   );
//   return result;
// });

// ipcMain.handle("start-server", async (_, serverId: number) => {
//   const stmt = db.prepare("SELECT * FROM servers WHERE id = ?");
//   const dbRecord = stmt.get(serverId) as ServerConfig;
//   if (!dbRecord) throw new Error("Server not found");

//   const server: ServerConfig = {
//     id: dbRecord.id,
//     name: dbRecord.name,
//     description: dbRecord.description || undefined,
//     installType: dbRecord.installType,
//     package: dbRecord.package,
//     startCommand: dbRecord.startCommand || undefined,
//     args: JSON.parse(String(dbRecord.args)),
//     version: dbRecord.version || undefined,
//     enabled: dbRecord.enabled === true,
//   };

//   return mcp.createClient(server);
// });

// ipcMain.handle("stop-server", async (_, id: number) => {
//   const stmt = db.prepare("SELECT * FROM servers WHERE id = ?");
//   const dbRecord = stmt.get(id) as ServerConfig;
//   if (!dbRecord) throw new Error("Server not found");

//   const server: ServerConfig = {
//     id: dbRecord.id,
//     name: dbRecord.name,
//     description: dbRecord.description || undefined,
//     installType: dbRecord.installType,
//     package: dbRecord.package,
//     startCommand: dbRecord.startCommand || undefined,
//     args: JSON.parse(String(dbRecord.args)),
//     version: dbRecord.version || undefined,
//     enabled: dbRecord.enabled === true,
//   };
//   return mcp.closeClient(server);
// });

// ipcMain.handle("get-conversations", () => {
//   try {
//     const stmt = db.prepare(`
//       SELECT
//         c.id,
//         c.providerId,
//         c.title,
//         c.createdAt,
//         c.parent_conversation_id,
//         json_group_array(
//           json_object(
//             'id', m.id,
//             'conversationId', m.conversationId,
//             'role', m.role,
//             'content', m.content,
//             'createdAt', m.createdAt
//           )
//         ) as messages
//       FROM conversations c
//       LEFT JOIN messages m ON c.id = m.conversationId
//       GROUP BY c.id
//     `);

//     const rawResults = stmt.all() as {
//       id: number;
//       providerId: number;
//       title: string;
//       createdAt: string;
//       parent_conversation_id: number | null;
//       messages: string;
//     }[];

//     const conversations = rawResults.map((convo) => {
//       const parsedMessages = JSON.parse(convo.messages as string);

//       return {
//         ...convo,
//         messages: parsedMessages.filter((m: any) => m.id !== null),
//       };
//     });

//     return conversations;
//   } catch (error) {
//     console.log("unable to get conversations", error);
//     throw error;
//   }
// });

// ipcMain.handle("create-conversation", (_, convo: Partial<Conversation>) => {
//   try {
//     const stmt = db.prepare(`
//     INSERT into conversations (providerId, title, parent_conversation_id ) VALUES(?,?,?)
//     `);

//     const result = stmt.run(
//       convo.providerId,
//       convo.title,
//       convo.parent_conversation_id || null
//     );

//     return result.lastInsertRowid;
//   } catch (error) {
//     console.log("unable to create new conversation");
//     throw error;
//   }
// });

// ipcMain.handle("delete-conversation", (_, convoId: number) => {
//   try {
//     const getStmt = db.prepare("SELECT title FROM conversations WHERE id = ?");
//     const convo = getStmt.get(convoId) as { title: string } | undefined;

//     if (!convo) {
//       throw new Error(`No conversation found with id ${convoId}`);
//     }

//     db.transaction(() => {
//       const deleteMessages = db.prepare(
//         "DELETE FROM messages WHERE conversationId = ?"
//       );
//       deleteMessages.run(convoId);

//       const deleteConvo = db.prepare("DELETE FROM conversations WHERE id = ?");
//       const result = deleteConvo.run(convoId);

//       if (result.changes === 0) {
//         throw new Error(`Failed to delete conversation from database`);
//       }
//     })();

//     return { success: true };
//   } catch (error) {
//     console.log("unable to delete conversation", error);
//     throw error;
//   }
// });

// ipcMain.handle("save-message", (_, message: Message) => {
//   try {
//     const content = message.content.content || message.content;

//     const stmt = db.prepare(`
//       INSERT INTO messages (
//         conversationId,
//         role,
//         content
//       ) VALUES (?, ?, ?)
//     `);

//     const result = stmt.run(
//       message.conversationId,
//       message.role,
//       JSON.stringify(content) // Store the actual content array
//     );

//     return result.lastInsertRowid;
//   } catch (error) {
//     console.log("unable to save message", error);
//     throw error;
//   }
// });

// ipcMain.handle("save-messages", (_, messages: Message[]) => {
//   try {
//     const stmt = db.prepare(`
//       INSERT INTO messages (
//         conversationId,
//         role,
//         content
//       ) VALUES (?, ?, ?)
//     `);

//     const results = db.transaction((messages: Message[]) => {
//       return messages.map((message) =>
//         stmt.run(message.conversationId, message.role, message.content)
//       );
//     })(messages);

//     return results.map((result) => result.lastInsertRowid);
//   } catch (error) {
//     console.log("unable to save messages", error);
//     throw error;
//   }
// });

// ipcMain.handle("delete-message", (_, messageId: number) => {
//   try {
//     const stmt = db.prepare(`
//   DELETE from messages where id = ?`);

//     return stmt.run(messageId);
//   } catch (error) {
//     console.log("unable to delete message", error);
//   }
// });

// ipcMain.handle(
//   "update-conversation-title",
//   (_, convoId: number, newTitle: string) => {
//     try {
//       const stmt = db.prepare(`
//       UPDATE conversations
//       SET title = ?
//       WHERE id = ?
//     `);

//       const result = stmt.run(newTitle, convoId);

//       if (result.changes === 0) {
//         throw new Error(`No conversation found with id ${convoId}`);
//       }

//       return { success: true };
//     } catch (error) {
//       console.log("unable to update conversation title", error);
//       throw error;
//     }
//   }
// );

// ipcMain.handle("get-conversation-messages", (_, conversationId: number) => {
//   try {
//     const stmt = db.prepare(`
//       SELECT
//         id,
//         conversationId,
//         role,
//         content,
//         createdAt
//       FROM messages
//       WHERE conversationId = ?
//       ORDER BY createdAt ASC
//     `);

//     return stmt.all(conversationId);
//   } catch (error) {
//     console.log("unable to get conversation messages", error);
//     throw error;
//   }
// });

// ipcMain.handle("chat", async (_, data: Message[]) => {
//   if (!providers.getCurrentProvider()) {
//     throw new Error("No provider selected");
//   }
//   return providers.processQuery(data);
// });

// ipcMain.handle("summarize-context", async (_, data: Message[]) => {
//   if (!providers.getCurrentProvider()) {
//     throw new Error("No provider selected");
//   }
//   return providers.summarizeContext(data);
// });

// ipcMain.handle("extractPDFText", async (_, file) => {
//   // Implement PDF text extraction
// });

// ipcMain.handle("parseCSV", async (_, file) => {
//   // Implement CSV parsing
// });

// ipcMain.handle("parseSpreadsheet", async (_, file) => {
//   // Implement spreadsheet parsing
// });

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
