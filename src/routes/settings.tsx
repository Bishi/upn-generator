import { createFileRoute } from "@tanstack/react-router";
import { useState } from "react";
import { Building2, Database, Home, Mail, Zap } from "lucide-react";
import { cn } from "@/lib/utils";
import { ApartmentsSection } from "@/components/settings/ApartmentsSection";
import { BuildingSection } from "@/components/settings/BuildingSection";
import { DataSection } from "@/components/settings/DataSection";
import { ProvidersSection } from "@/components/settings/ProvidersSection";
import { SmtpSection } from "@/components/settings/SmtpSection";

export const Route = createFileRoute("/settings")({
  component: SettingsPage,
});

type Tab = "building" | "apartments" | "providers" | "smtp" | "data";

const tabs: { id: Tab; label: string; icon: React.ReactNode }[] = [
  { id: "building", label: "Building", icon: <Building2 className="size-4" /> },
  { id: "apartments", label: "Apartments", icon: <Home className="size-4" /> },
  { id: "providers", label: "Providers", icon: <Zap className="size-4" /> },
  { id: "smtp", label: "Email (SMTP)", icon: <Mail className="size-4" /> },
  { id: "data", label: "Data", icon: <Database className="size-4" /> },
];

function SettingsPage() {
  const [activeTab, setActiveTab] = useState<Tab>("building");

  return (
    <div>
      <h2 className="mb-6 text-2xl font-bold">Settings</h2>

      <div className="mb-6 flex gap-1 border-b border-border">
        {tabs.map((tab) => (
          <button
            key={tab.id}
            onClick={() => setActiveTab(tab.id)}
            className={cn(
              "flex items-center gap-2 border-b-2 -mb-px px-4 py-2 text-sm font-medium transition-colors",
              activeTab === tab.id
                ? "border-primary text-foreground"
                : "border-transparent text-muted-foreground hover:text-foreground"
            )}
          >
            {tab.icon}
            {tab.label}
          </button>
        ))}
      </div>

      {activeTab === "building" && <BuildingSection />}
      {activeTab === "apartments" && <ApartmentsSection />}
      {activeTab === "providers" && <ProvidersSection />}
      {activeTab === "smtp" && <SmtpSection />}
      {activeTab === "data" && <DataSection />}
    </div>
  );
}
