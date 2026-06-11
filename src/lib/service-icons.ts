import {
  Building2,
  Droplet,
  Flame,
  Sparkles,
  Trash2,
  Wind,
  Zap,
  type LucideIcon,
} from "lucide-react";

export function serviceIconFor(label: string, providerName = ""): LucideIcon {
  const value = `${label} ${providerName}`.toLowerCase();

  if (value.includes("elektr") || value.includes("electric")) return Zap;
  if (value.includes("voda") || value.includes("water")) return Droplet;
  if (value.includes("gas") || value.includes("plin") || value.includes("energetika")) {
    return Flame;
  }
  if (value.includes("snaga") || value.includes("waste") || value.includes("trash")) {
    return Trash2;
  }
  if (value.includes("clean") || value.includes("cisc")) return Sparkles;
  if (value.includes("chimney") || value.includes("dimnik")) return Wind;

  return Building2;
}
