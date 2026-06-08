import { createFileRoute } from "@tanstack/react-router";
import { useState } from "react";
import { Building2, Database, Home, Mail, Palette, Zap } from "lucide-react";
import { cn } from "@/lib/utils";
import { AppearanceSection } from "@/components/settings/AppearanceSection";
import { ApartmentsSection } from "@/components/settings/ApartmentsSection";
import { BuildingSection } from "@/components/settings/BuildingSection";
import { DataSection } from "@/components/settings/DataSection";
import { ProvidersSection } from "@/components/settings/ProvidersSection";
import { SmtpSection } from "@/components/settings/SmtpSection";

export const Route = createFileRoute("/settings")({
  component: SettingsPage,
});

type Tab =
  | "building"
  | "apartments"
  | "providers"
  | "smtp"
  | "appearance"
  | "data";

const tabs: { id: Tab; label: string; icon: React.ReactNode }[] = [
  { id: "building", label: "Building", icon: <Building2 className="size-4" /> },
  { id: "apartments", label: "Apartments", icon: <Home className="size-4" /> },
  { id: "providers", label: "Providers", icon: <Zap className="size-4" /> },
  { id: "smtp", label: "Email (SMTP)", icon: <Mail className="size-4" /> },
  { id: "appearance", label: "Appearance", icon: <Palette className="size-4" /> },
  { id: "data", label: "Data", icon: <Database className="size-4" /> },
];

function SettingsPage() {
  const [activeTab, setActiveTab] = useState<Tab>("building");

  return (
    <div className="flex flex-col gap-5">
      <section>
        <h2 className="text-3xl font-semibold leading-tight">Settings</h2>
        <p className="mt-1 text-sm text-muted-foreground">
          Configure the building, apartments, providers, delivery, appearance, and data.
        </p>
      </section>

      <div className="flex gap-1 overflow-x-auto border-b border-border">
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
        {activeTab === "building" && <BuildingSection />}
        {activeTab === "apartments" && <ApartmentsSection />}
        {activeTab === "providers" && <ProvidersSection />}
        {activeTab === "smtp" && <SmtpSection />}
        {activeTab === "appearance" && <AppearanceSection />}
        {activeTab === "data" && <DataSection />}
      </div>
    </div>
  );
}
