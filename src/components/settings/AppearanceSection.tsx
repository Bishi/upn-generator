import { Check } from "lucide-react";
import { THEMES, useTheme, type ThemeId } from "@/lib/theme";
import { cn } from "@/lib/utils";
import { SETTINGS_PANEL_WIDTH } from "@/components/settings/layout";
import {
  Card,
  CardContent,
} from "@/components/ui/card";

const THEME_SWATCHES: Record<ThemeId, [string, string, string]> = {
  refined: ["#3d7558", "#dff0e8", "#f6ead4"],
  crisp: ["#2f5bea", "#e9eefe", "#fbefdc"],
  official: ["#da3a52", "#fbe6ea", "#f7ead3"],
  "dark-crisp": ["#4a7aef", "#171e2c", "#1a2a50"],
  "dark-mono": ["#ffffff", "#1a1a1a", "#333333"],
  "dark-shadow": ["#f5f5fa", "#111118", "#222230"],
};

export function AppearanceSection() {
  const { theme, setTheme } = useTheme();

  return (
    <Card className={`${SETTINGS_PANEL_WIDTH} overflow-hidden`}>
      <div className="border-b border-border px-5 py-4">
        <h3 className="font-head text-lg font-semibold">Appearance</h3>
        <p className="mt-1 text-sm text-muted-foreground">
          Switch between tokenized visual themes. This preference is saved in
          the app database and included in backups.
        </p>
      </div>
      <CardContent className="p-5">
        <div className="grid gap-3 sm:grid-cols-2">
          {THEMES.map((item) => (
            <ThemeOption
              key={item.id}
              id={item.id}
              name={item.name}
              description={item.description}
              selected={theme === item.id}
              onSelect={setTheme}
            />
          ))}
        </div>
      </CardContent>
    </Card>
  );
}

function ThemeOption({
  id,
  name,
  description,
  selected,
  onSelect,
}: {
  id: ThemeId;
  name: string;
  description: string;
  selected: boolean;
  onSelect: (theme: ThemeId) => void;
}) {
  return (
    <button
      type="button"
      className={cn(
        "rounded-lg border bg-card p-4 text-left shadow-card transition-colors hover:bg-accent hover:text-accent-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background",
        selected
          ? "border-primary bg-accent text-accent-foreground"
          : "border-border text-foreground",
      )}
      onClick={() => onSelect(id)}
      aria-pressed={selected}
    >
      <span className="flex items-center justify-between gap-3">
        <span className="text-sm font-semibold">{name}</span>
        {selected ? <Check className="size-4 text-primary" /> : null}
      </span>
      <span className="mt-2 block text-sm text-muted-foreground">
        {description}
      </span>
      <span className="mt-4 flex gap-1.5" aria-hidden="true">
        {THEME_SWATCHES[id].map((color) => (
          <span
            key={color}
            className="h-2 flex-1 rounded-full"
            style={{ backgroundColor: color }}
          />
        ))}
      </span>
    </button>
  );
}
