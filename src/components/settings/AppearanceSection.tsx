import { Check } from "lucide-react";
import { THEMES, useTheme, type ThemeId } from "@/lib/theme";
import { cn } from "@/lib/utils";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";

export function AppearanceSection() {
  const { theme, setTheme } = useTheme();

  return (
    <Card className="max-w-3xl">
      <CardHeader>
        <CardTitle>Appearance</CardTitle>
        <CardDescription>
          Switch between tokenized visual themes. This preference is saved only
          on this device.
        </CardDescription>
      </CardHeader>
      <CardContent>
        <div className="grid gap-3 sm:grid-cols-3">
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
        <span className="h-2 flex-1 rounded-full bg-primary" />
        <span className="h-2 flex-1 rounded-full bg-accent-soft" />
        <span className="h-2 flex-1 rounded-full bg-warning-soft" />
      </span>
    </button>
  );
}
