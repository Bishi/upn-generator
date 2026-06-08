import {
  createContext,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";

export const THEME_STORAGE_KEY = "upn-generator.theme";
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
] as const;

export type ThemeId = (typeof THEMES)[number]["id"];

type ThemeContextValue = {
  theme: ThemeId;
  setTheme: (theme: ThemeId) => void;
};

const ThemeContext = createContext<ThemeContextValue | null>(null);

export function isThemeId(value: string | null): value is ThemeId {
  return value === "refined" || value === "crisp" || value === "official";
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

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [theme, setThemeState] = useState<ThemeId>(() => readStoredTheme());

  useEffect(() => {
    applyTheme(theme);
    try {
      window.localStorage.setItem(THEME_STORAGE_KEY, theme);
    } catch {
      // Theme persistence is optional; the CSS refined fallback remains valid.
    }
  }, [theme]);

  const value = useMemo(
    () => ({
      theme,
      setTheme: setThemeState,
    }),
    [theme],
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
