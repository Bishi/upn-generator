import { createFileRoute } from "@tanstack/react-router";
import { useState } from "react";
import { Building2, Home, Mail, Settings, Zap } from "lucide-react";
import { cn } from "@/lib/utils";
import { AppearanceSection } from "@/components/settings/AppearanceSection";
import { ApartmentsSection } from "@/components/settings/ApartmentsSection";
import { BuildingSection } from "@/components/settings/BuildingSection";
import { DataSection } from "@/components/settings/DataSection";
import { InboxSection } from "@/components/settings/InboxSection";
import { ProvidersSection } from "@/components/settings/ProvidersSection";
import { SmtpSection } from "@/components/settings/SmtpSection";

export const Route = createFileRoute("/settings")({
  component: SettingsPage,
});

type Tab =
  | "building"
  | "apartments"
  | "providers"
  | "delivery"
  | "app";

const tabs: { id: Tab; label: string; icon: React.ReactNode }[] = [
  { id: "building", label: "Building", icon: <Building2 className="size-4" /> },
  { id: "apartments", label: "Apartments", icon: <Home className="size-4" /> },
  { id: "providers", label: "Providers", icon: <Zap className="size-4" /> },
  { id: "delivery", label: "Delivery", icon: <Mail className="size-4" /> },
  { id: "app", label: "App", icon: <Settings className="size-4" /> },
];

function SettingsPage() {
  const [activeTab, setActiveTab] = useState<Tab>("building");

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
            onClick={() => setActiveTab(tab.id)}
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
          <BuildingSection />
        </div>
        <div hidden={activeTab !== "apartments"}>
          <ApartmentsSection />
        </div>
        <div hidden={activeTab !== "providers"}>
          <ProvidersSection />
        </div>
        <div hidden={activeTab !== "delivery"} className="space-y-4">
          <InboxSection />
          <SmtpSection />
        </div>
        <div hidden={activeTab !== "app"} className="space-y-4">
          <AppearanceSection />
          <DataSection />
        </div>
      </div>
    </div>
  );
}
