import { createFileRoute, Link } from "@tanstack/react-router";
import { useEffect, useState } from "react";
import { FileText, SplitSquareHorizontal, CreditCard, Settings } from "lucide-react";
import { ipc } from "@/lib/ipc";
import type { BillingPeriod } from "@/lib/types";
import { MONTHS } from "@/lib/types";

export const Route = createFileRoute("/")({
  component: Dashboard,
});

function Dashboard() {
  const [latestPeriod, setLatestPeriod] = useState<BillingPeriod | null>(null);
  const [billCount, setBillCount] = useState(0);
  const [splitCount, setSplitCount] = useState(0);

  useEffect(() => {
    ipc.getBillingPeriods().then(async (periods) => {
      if (periods.length === 0) return;
      const p = periods[0];
      setLatestPeriod(p);
      if (p.id) {
        const bills = await ipc.getBills(p.id);
        setBillCount(bills.length);
        const splits = await ipc.getSplits(p.id);
        setSplitCount(splits.length);
      }
    });
  }, []);

  const steps = [
    {
      icon: <Settings className="size-5" />,
      title: "Configure Settings",
      desc: "Set up your building, apartments, providers, and SMTP.",
      to: "/settings",
      done: true,
    },
    {
      icon: <FileText className="size-5" />,
      title: "Import Bills",
      desc: "Create a billing period and import PDF invoices.",
      to: "/bills",
      done: billCount > 0,
    },
    {
      icon: <SplitSquareHorizontal className="size-5" />,
      title: "Calculate Splits",
      desc: "Split bills proportionally by apartment occupancy.",
      to: "/splits",
      done: splitCount > 0,
    },
    {
      icon: <CreditCard className="size-5" />,
      title: "Send UPN Forms",
      desc: "Preview UPN payment slips and email them to tenants.",
      to: "/upn",
      done: false,
    },
  ];

  return (
    <div className="flex flex-col gap-8 max-w-2xl">
      <div>
        <h2 className="text-2xl font-bold mb-1">Dashboard</h2>
        <p className="text-muted-foreground text-sm">
          UPN Generator — automated utility bill splitting for apartment buildings.
        </p>
      </div>

      {latestPeriod && (
        <div className="rounded-lg border border-border bg-card p-4">
          <div className="text-xs text-muted-foreground uppercase tracking-wide mb-1">
            Latest Period
          </div>
          <div className="text-lg font-semibold">
            {MONTHS[latestPeriod.month - 1]} {latestPeriod.year}
          </div>
          <div className="flex gap-4 mt-2 text-sm text-muted-foreground">
            <span>{billCount} bills</span>
            <span>{splitCount} splits</span>
            <span className="capitalize">{latestPeriod.status}</span>
          </div>
        </div>
      )}

      <div className="flex flex-col gap-3">
        {steps.map((step, i) => (
          <Link
            key={i}
            to={step.to}
            className="flex items-start gap-4 rounded-lg border border-border bg-card p-4 hover:bg-accent/30 transition-colors"
          >
            <div
              className={`mt-0.5 rounded-full p-1.5 ${
                step.done
                  ? "bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400"
                  : "bg-muted text-muted-foreground"
              }`}
            >
              {step.icon}
            </div>
            <div>
              <div className="flex items-center gap-2">
                <span className="font-medium">{step.title}</span>
                {step.done && (
                  <span className="text-xs text-green-600 font-medium">✓</span>
                )}
              </div>
              <p className="text-sm text-muted-foreground mt-0.5">{step.desc}</p>
            </div>
          </Link>
        ))}
      </div>
    </div>
  );
}
