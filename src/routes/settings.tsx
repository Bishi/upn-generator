import { createFileRoute } from "@tanstack/react-router";
import { useState } from "react";
import { Building2, Home, Zap, Mail } from "lucide-react";
import { cn } from "@/lib/utils";
import { BuildingSection } from "@/components/settings/BuildingSection";
import { ApartmentsSection } from "@/components/settings/ApartmentsSection";
import { ProvidersSection } from "@/components/settings/ProvidersSection";
import { SmtpSection } from "@/components/settings/SmtpSection";

export const Route = createFileRoute("/settings")({
  component: SettingsPage,
});

type Tab = "building" | "apartments" | "providers" | "smtp";

const tabs: { id: Tab; label: string; icon: React.ReactNode }[] = [
  { id: "building", label: "Building", icon: <Building2 className="size-4" /> },
  { id: "apartments", label: "Apartments", icon: <Home className="size-4" /> },
  { id: "providers", label: "Providers", icon: <Zap className="size-4" /> },
  { id: "smtp", label: "Email (SMTP)", icon: <Mail className="size-4" /> },
];

function SettingsPage() {
  const [activeTab, setActiveTab] = useState<Tab>("building");

  return (
    <div>
      <h2 className="text-2xl font-bold mb-6">Settings</h2>

      <div className="flex gap-1 border-b border-border mb-6">
        {tabs.map((tab) => (
          <button
            key={tab.id}
            onClick={() => setActiveTab(tab.id)}
            className={cn(
              "flex items-center gap-2 px-4 py-2 text-sm font-medium border-b-2 -mb-px transition-colors",
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
    </div>
  );
}
