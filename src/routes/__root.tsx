import { createRootRoute, Link, Outlet } from "@tanstack/react-router";
import { Building2, FileText, SplitSquareHorizontal, CreditCard, Settings } from "lucide-react";

export const Route = createRootRoute({
  component: RootLayout,
});

function RootLayout() {
  return (
    <div className="flex h-screen bg-background text-foreground">
      <nav className="w-56 shrink-0 border-r border-border bg-card p-4 flex flex-col gap-1">
        <h1 className="text-lg font-bold px-3 py-2 mb-4">UPN Generator</h1>
        <NavLink to="/" icon={<Building2 className="size-4" />} label="Dashboard" />
        <NavLink to="/bills" icon={<FileText className="size-4" />} label="Bills" />
        <NavLink to="/splits" icon={<SplitSquareHorizontal className="size-4" />} label="Splits" />
        <NavLink to="/upn" icon={<CreditCard className="size-4" />} label="UPN Preview" />
        <div className="mt-auto">
          <NavLink to="/settings" icon={<Settings className="size-4" />} label="Settings" />
        </div>
      </nav>
      <main className="flex-1 overflow-auto p-6">
        <Outlet />
      </main>
    </div>
  );
}

function NavLink({ to, icon, label }: { to: string; icon: React.ReactNode; label: string }) {
  return (
    <Link
      to={to}
      className="flex items-center gap-3 rounded-lg px-3 py-2 text-sm text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground [&.active]:bg-accent [&.active]:text-accent-foreground"
      activeOptions={{ exact: to === "/" }}
    >
      {icon}
      {label}
    </Link>
  );
}
