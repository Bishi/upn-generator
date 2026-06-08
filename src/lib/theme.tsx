import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import { ipc } from "@/lib/ipc";

export const THEME_STORAGE_KEY = "upn-generator.theme";
const THEME_DB_MIGRATION_KEY = "upn-generator.theme.db-migrated";
export const DEFAULT_THEME = "refined";

export const THEMES = [
  {
    id: "refined",
    name: "Refined",
    description: "Warm, clear, and closest to the chosen redesign direction.",
  },
  {
    id: "crisp",
    name: "Crisp",
    description: "Cooler institutional blue for contrast testing.",
  },
  {
    id: "official",
    name: "Official",
    description: "UPN-slip inspired red for document-forward testing.",
  },
  {
    id: "dark-crisp",
    name: "Dark Crisp",
    description: "Dark navy with banking-blue accents from the refined mock.",
  },
  {
    id: "dark-mono",
    name: "Dark Mono",
    description: "High-contrast grayscale for a quieter night-mode pass.",
  },
  {
    id: "dark-shadow",
    name: "Dark Shadow",
    description: "Near-black, low-glow theme inspired by the new mock.",
  },
] as const;

export type ThemeId = (typeof THEMES)[number]["id"];

type ThemeContextValue = {
  theme: ThemeId;
  setTheme: (theme: ThemeId) => void;
};

const ThemeContext = createContext<ThemeContextValue | null>(null);

export function isThemeId(value: string | null): value is ThemeId {
  return THEMES.some((theme) => theme.id === value);
}

export function applyTheme(theme: ThemeId) {
  document.documentElement.dataset.theme = theme;
}

export function readStoredTheme(): ThemeId {
  try {
    const stored = window.localStorage.getItem(THEME_STORAGE_KEY);
    return isThemeId(stored) ? stored : DEFAULT_THEME;
  } catch {
    return DEFAULT_THEME;
  }
}

export function initializeTheme() {
  applyTheme(readStoredTheme());
}

function writeStoredTheme(theme: ThemeId) {
  try {
    window.localStorage.setItem(THEME_STORAGE_KEY, theme);
  } catch {
    // Local persistence is a startup fallback; the SQLite setting is canonical.
  }
}

function readThemeMigrated() {
  try {
    return window.localStorage.getItem(THEME_DB_MIGRATION_KEY) === "1";
  } catch {
    return true;
  }
}

function writeThemeMigrated() {
  try {
    window.localStorage.setItem(THEME_DB_MIGRATION_KEY, "1");
  } catch {
    // Migration marker is best-effort; a failed marker should not block theming.
  }
}

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [theme, setThemeState] = useState<ThemeId>(() => readStoredTheme());

  useEffect(() => {
    let cancelled = false;

    async function loadThemeFromDb() {
      try {
        const settings = await ipc.getAppSettings();
        const dbTheme = isThemeId(settings.theme) ? settings.theme : DEFAULT_THEME;
        const localTheme = readStoredTheme();
        const shouldMigrateLocalTheme = !readThemeMigrated();
        const nextTheme = shouldMigrateLocalTheme ? localTheme : dbTheme;

        if (shouldMigrateLocalTheme) {
          await ipc.saveAppSettings({ theme: nextTheme });
          writeThemeMigrated();
        }

        if (!cancelled) {
          setThemeState(nextTheme);
          applyTheme(nextTheme);
          writeStoredTheme(nextTheme);
        }
      } catch {
        // In plain Vite preview or failed IPC startup, localStorage keeps the UI usable.
      }
    }

    void loadThemeFromDb();

    return () => {
      cancelled = true;
    };
  }, []);

  const setTheme = useCallback((nextTheme: ThemeId) => {
    setThemeState(nextTheme);
    applyTheme(nextTheme);
    writeStoredTheme(nextTheme);
    writeThemeMigrated();
    void ipc.saveAppSettings({ theme: nextTheme }).catch(() => {
      // Keep the immediate UI response even if persistence is temporarily unavailable.
    });
  }, []);

  const value = useMemo(
    () => ({
      theme,
      setTheme,
    }),
    [theme, setTheme],
  );

  return (
    <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>
  );
}

export function useTheme() {
  const context = useContext(ThemeContext);
  if (!context) {
    throw new Error("useTheme must be used inside ThemeProvider");
  }
  return context;
}
