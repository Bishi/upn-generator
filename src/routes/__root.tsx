import { createRootRoute, Link, Outlet, useLocation } from "@tanstack/react-router";
import { getVersion } from "@tauri-apps/api/app";
import { type ReactNode, useEffect, useState } from "react";
import {
  CreditCard,
  FileText,
  Gauge,
  Banknote,
  Settings,
  SplitSquareHorizontal,
} from "lucide-react";
import { BillingPeriodSelectionProvider } from "@/lib/billing-period-selection";
import { WorkflowContextBar } from "@/components/WorkflowContextBar";
import { useBillingPeriodSelection } from "@/lib/billing-period-selection";
import { useWorkflowSnapshot } from "@/lib/workflow-snapshot";

export const Route = createRootRoute({
  component: RootLayout,
});

function RootLayout() {
  const [version, setVersion] = useState<string | null>(null);

  useEffect(() => {
    void getVersion().then(setVersion).catch(() => setVersion(null));
  }, []);

  return (
    <div className="flex h-screen bg-background text-foreground">
      <nav className="flex w-56 shrink-0 flex-col gap-1 border-r border-border bg-surface-2 p-4">
        <div className="mb-6 flex items-center gap-3 px-2 py-1">
          <div className="grid size-8 shrink-0 place-items-center rounded-md bg-primary text-primary-foreground shadow-card">
            <Banknote className="size-4" />
          </div>
          <div className="min-w-0">
            <h1 className="truncate text-sm font-bold leading-tight">
              UPN Generator
            </h1>
            <p className="truncate text-xs text-muted-foreground">
              Kamniska ulica 36
            </p>
          </div>
        </div>
        <div className="px-3 pb-2 text-[11px] font-bold uppercase tracking-wide text-muted-foreground">
          Monthly Workflow
        </div>
        <NavLink to="/" icon={<Gauge className="size-4" />} label="Dashboard" />
        <NavLink to="/bills" icon={<FileText className="size-4" />} label="Bills" />
        <NavLink to="/splits" icon={<SplitSquareHorizontal className="size-4" />} label="Splits" />
        <NavLink to="/upn" icon={<CreditCard className="size-4" />} label="UPN Preview" />
        <div className="mt-auto">
          <div className="px-3 pb-2 text-[11px] font-bold uppercase tracking-wide text-muted-foreground">
            Configure
          </div>
          <NavLink to="/settings" icon={<Settings className="size-4" />} label="Settings" />
          {version && (
            <div className="mt-4 border-t border-border px-3 pt-3 text-xs leading-relaxed text-muted-foreground">
              Local backed up
              <br />
              Version {version}
            </div>
          )}
        </div>
      </nav>
      <BillingPeriodSelectionProvider>
        <ShellContent />
      </BillingPeriodSelectionProvider>
    </div>
  );
}

function ShellContent() {
  const location = useLocation();
  const { allPeriods, selected } = useBillingPeriodSelection();
  const snapshot = useWorkflowSnapshot(selected?.id, allPeriods);
  const showContextBar =
    location.pathname.startsWith("/bills") ||
    location.pathname.startsWith("/splits") ||
    location.pathname.startsWith("/upn");

  return (
    <main className="flex min-w-0 flex-1 flex-col overflow-hidden">
      {showContextBar && <WorkflowContextBar snapshot={snapshot} />}
      <div className="flex-1 overflow-y-scroll overflow-x-hidden p-6 [scrollbar-gutter:stable]">
        <Outlet />
      </div>
    </main>
  );
}

function NavLink({ to, icon, label }: { to: string; icon: ReactNode; label: string }) {
  return (
    <Link
      to={to}
      className="flex items-center gap-3 rounded-md px-3 py-2 text-sm font-medium text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground [&.active]:bg-accent [&.active]:text-accent-foreground"
      activeOptions={{ exact: to === "/" }}
    >
      {icon}
      {label}
    </Link>
  );
}
