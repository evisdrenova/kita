import { createContext, useContext, useEffect, useState } from "react";
import { Sun, Moon } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { AppSettings } from "./types/types";

type Theme = "light" | "dark";

interface ThemeContextType {
  theme: Theme;
  toggleTheme: () => void;
  settings: AppSettings;
  updateSettings: (newSettings: AppSettings) => Promise<void>;
}

const ThemeContext = createContext<ThemeContextType | undefined>(undefined);

export function ThemeProvider({ children }: { children: React.ReactNode }) {
  const [settings, setSettings] = useState<AppSettings>({});
  const [theme, setTheme] = useState<Theme>("dark"); // Default until settings load
  const [isLoading, setIsLoading] = useState(true);

  // Load settings when provider mounts
  useEffect(() => {
    async function loadSettings() {
      try {
        const appSettings = await invoke<AppSettings>("get_settings");
        setSettings(appSettings);

        // Set theme from settings or use system preference as fallback
        const savedTheme = appSettings.theme as Theme;
        if (savedTheme) {
          setTheme(savedTheme);
        } else {
          // If no theme in settings, check system preference
          const systemTheme = window.matchMedia("(prefers-color-scheme: dark)")
            .matches
            ? "dark"
            : "light";
          setTheme(systemTheme);

          // Update settings with system theme
          const updatedSettings = { ...appSettings, theme: systemTheme };
          await invoke("update_settings", { settings: updatedSettings });
          setSettings(updatedSettings);
        }
      } catch (error) {
        console.error("Failed to load settings:", error);
        // Fallback to system preference
        if (window.matchMedia("(prefers-color-scheme: dark)").matches) {
          setTheme("dark");
        } else {
          setTheme("light");
        }
      } finally {
        setIsLoading(false);
      }
    }

    loadSettings();
  }, []);

  // Update document class when theme changes
  useEffect(() => {
    if (isLoading) return;

    if (theme === "dark") {
      document.documentElement.classList.add("dark");
    } else {
      document.documentElement.classList.remove("dark");
    }
  }, [theme, isLoading]);

  // Listen for system theme changes
  useEffect(() => {
    const mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
    const handleChange = async (e: MediaQueryListEvent) => {
      const newTheme = e.matches ? "dark" : "light";
      setTheme(newTheme);

      // Update settings when system theme changes
      try {
        const updatedSettings = { ...settings, theme: newTheme };
        await invoke("update_settings", { settings: updatedSettings });
        setSettings(updatedSettings);
      } catch (error) {
        console.error("Failed to update theme in settings:", error);
      }
    };

    mediaQuery.addEventListener("change", handleChange);
    return () => mediaQuery.removeEventListener("change", handleChange);
  }, [settings]);

  // Update settings helper function
  const updateSettings = async (newSettings: AppSettings) => {
    try {
      await invoke("update_settings", { settings: newSettings });
      setSettings(newSettings);

      // Update theme if it changed
      if (newSettings.theme && newSettings.theme !== theme) {
        setTheme(newSettings.theme as Theme);
      }
    } catch (error) {
      console.error("Failed to update settings:", error);
    }
  };

  const toggleTheme = async () => {
    const newTheme = theme === "light" ? "dark" : "light";
    setTheme(newTheme);

    // Update settings with new theme
    try {
      const updatedSettings = { ...settings, theme: newTheme };
      await invoke("update_settings", { settings: updatedSettings });
      setSettings(updatedSettings);
    } catch (error) {
      console.error("Failed to update theme in settings:", error);
    }
  };

  return (
    <ThemeContext.Provider
      value={{
        theme,
        toggleTheme,
        settings,
        updateSettings,
      }}
    >
      {children}
    </ThemeContext.Provider>
  );
}

export function useTheme() {
  const context = useContext(ThemeContext);
  if (context === undefined) {
    throw new Error("useTheme must be used within a ThemeProvider");
  }
  return context;
}

export function ThemeToggle() {
  const { theme, toggleTheme } = useTheme();

  return (
    <button
      onClick={toggleTheme}
      aria-label="Toggle theme"
      className="flex flex-row items-center gap-1 text-primary-foreground/70 hover:text-primary-foreground"
    >
      {theme === "light" ? <Moon size={16} /> : <Sun size={16} />}
    </button>
  );
}
