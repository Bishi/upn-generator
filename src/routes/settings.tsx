import { createFileRoute } from "@tanstack/react-router";
import { confirm } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useState, type ReactNode } from "react";
import { Building2, Home, Mail, Settings, Zap } from "lucide-react";
import { cn } from "@/lib/utils";
import { AppearanceSection } from "@/components/settings/AppearanceSection";
import { ApartmentsSection } from "@/components/settings/ApartmentsSection";
import { BuildingSection } from "@/components/settings/BuildingSection";
import { DataSection } from "@/components/settings/DataSection";
import { InboxSection } from "@/components/settings/InboxSection";
import { ProvidersSection } from "@/components/settings/ProvidersSection";
import { SmtpSection } from "@/components/settings/SmtpSection";
import type { SettingsDirtyEntry, SettingsTabId } from "@/components/settings/dirty-state";

export const Route = createFileRoute("/settings")({
  validateSearch: (search: Record<string, unknown>): { tab?: Tab } => ({
    tab: isSettingsTab(search.tab) ? search.tab : undefined,
  }),
  component: SettingsPage,
});

type Tab = SettingsTabId;

const tabs: { id: Tab; label: string; icon: ReactNode }[] = [
  { id: "building", label: "Building", icon: <Building2 className="size-4" /> },
  { id: "apartments", label: "Apartments", icon: <Home className="size-4" /> },
  { id: "providers", label: "Providers", icon: <Zap className="size-4" /> },
  { id: "delivery", label: "Delivery", icon: <Mail className="size-4" /> },
  { id: "app", label: "App", icon: <Settings className="size-4" /> },
];

function isSettingsTab(value: unknown): value is Tab {
  return typeof value === "string" && tabs.some((tab) => tab.id === value);
}

function SettingsPage() {
  const search = Route.useSearch();
  const [activeTab, setActiveTab] = useState<Tab>(search.tab ?? "building");
  const [dirtyEntries, setDirtyEntries] = useState<Record<string, SettingsDirtyEntry>>({});

  useEffect(() => {
    if (search.tab) setActiveTab(search.tab);
  }, [search.tab]);

  const registerDirtyEntry = useCallback((id: string, entry: SettingsDirtyEntry) => {
    setDirtyEntries((current) => ({ ...current, [id]: entry }));
    return () => {
      setDirtyEntries((current) => {
        const next = { ...current };
        delete next[id];
        return next;
      });
    };
  }, []);

  const dirtyEntriesForTab = useCallback(
    (tab: Tab) =>
      Object.values(dirtyEntries).filter(
        (entry) => entry.tab === tab && entry.isDirty,
      ),
    [dirtyEntries],
  );

  const switchToTab = (tab: Tab) => {
    setActiveTab(tab);
    const searchParams = new URLSearchParams(window.location.search);
    searchParams.set("tab", tab);
    window.history.replaceState(null, "", `${window.location.pathname}?${searchParams}`);
  };

  const selectTab = async (tab: Tab) => {
    if (tab === activeTab) return;

    const dirtyCurrentTab = dirtyEntriesForTab(activeTab);
    if (dirtyCurrentTab.length > 0) {
      const confirmed = await confirm(
        dirtyCurrentTab.length === 1
          ? `Discard unsaved changes in ${dirtyCurrentTab[0].label} and switch tabs?`
          : `Discard unsaved changes in ${dirtyCurrentTab.length} settings sections and switch tabs?`,
        {
          title: "Discard Unsaved Changes",
          kind: "warning",
          okLabel: "Discard and switch",
          cancelLabel: "Stay",
        },
      );
      if (!confirmed) return;
      dirtyCurrentTab.forEach((entry) => entry.discard());
    }

    switchToTab(tab);
  };

  return (
    <div className="flex flex-col gap-5">
      <section>
        <h2 className="text-3xl font-semibold leading-tight">Settings</h2>
        <p className="mt-1 text-sm text-muted-foreground">
          Configure the building, apartments, providers, delivery, and app preferences.
        </p>
      </section>

      <div className="scrollbar-none flex gap-1 overflow-x-auto border-b border-border">
        {tabs.map((tab) => (
          <button
            key={tab.id}
            onClick={() => selectTab(tab.id)}
            className={cn(
              "-mb-px flex h-10 items-center gap-2 whitespace-nowrap border-b-2 px-4 text-sm font-semibold transition-colors",
              activeTab === tab.id
                ? "border-primary text-accent-foreground"
                : "border-transparent text-muted-foreground hover:text-foreground"
            )}
          >
            {tab.icon}
            {tab.label}
          </button>
        ))}
      </div>

      <div>
        <div hidden={activeTab !== "building"}>
          <BuildingSection onDirtyEntry={registerDirtyEntry} />
        </div>
        <div hidden={activeTab !== "apartments"}>
          <ApartmentsSection onDirtyEntry={registerDirtyEntry} />
        </div>
        <div hidden={activeTab !== "providers"}>
          <ProvidersSection onDirtyEntry={registerDirtyEntry} />
        </div>
        <div hidden={activeTab !== "delivery"} className="space-y-4">
          <InboxSection onDirtyEntry={registerDirtyEntry} />
          <SmtpSection onDirtyEntry={registerDirtyEntry} />
        </div>
        <div hidden={activeTab !== "app"} className="space-y-4">
          <AppearanceSection />
          <DataSection />
        </div>
      </div>
    </div>
  );
}
