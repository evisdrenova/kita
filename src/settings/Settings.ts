import { Database } from "better-sqlite3";
import { ipcMain } from "electron";

interface Setting {
  key: string;
  value: string;
}

export type SettingsValue = string | number | boolean | object | null;

type SettingParams = [key: string, value: string];

export interface ISettingsManager {
  get<T extends SettingsValue>(key: string): T | undefined;
  set(key: string, value: SettingsValue): boolean;
  setMultiple(settings: Record<string, SettingsValue>): boolean;
  getAll(): Record<string, SettingsValue>;
}

// we use an in-memory cache for fast reads for our settings otherwise we go to the db
export default class SettingsManager {
  private cache = new Map<string, SettingsValue>();
  private readonly db: Database;

  constructor(db: Database) {
    this.db = db;
    this.cache = new Map<string, SettingsValue>();
    this.initializeCache();
  }

  // in-memory cache
  async initializeCache() {
    const stmt = this.db.prepare(`SELECT key, value FROM settings`);

    const settings = stmt.all() as Setting[];

    settings.forEach((setting) => {
      try {
        this.cache.set(setting.key, JSON.parse(setting.value));
      } catch (error) {
        this.cache.set(setting.key, setting.value);
      }
    });
  }

  public get<T extends SettingsValue>(key: string): T | undefined {
    return this.cache.get(key) as T | undefined;
  }

  public getAll(): Record<string, SettingsValue> {
    return Object.fromEntries(this.cache);
  }

  public set(key: string, value: SettingsValue): boolean {
    // Update cache
    this.cache.set(key, value);

    // Persist to database
    const stmt = this.db.prepare<SettingParams>(
      "INSERT OR REPLACE INTO settings (key, value) VALUES (?, ?)"
    );

    const valueToStore =
      typeof value === "object" ? JSON.stringify(value) : String(value);

    stmt.run(key, valueToStore);
    return true;
  }

  public setMultiple(settings: Record<string, SettingsValue>): boolean {
    const stmt = this.db.prepare<SettingParams>(
      "INSERT OR REPLACE INTO settings (key, value) VALUES (?, ?)"
    );

    const transaction = this.db.transaction(
      (settingsToUpdate: Record<string, SettingsValue>) => {
        for (const [key, value] of Object.entries(settingsToUpdate)) {
          // update the cache
          this.cache.set(key, value);
          const valueToStore =
            typeof value === "object" ? JSON.stringify(value) : String(value);
          stmt.run(key, valueToStore);
        }
      }
    );

    transaction(settings);
    return true;
  }
}
