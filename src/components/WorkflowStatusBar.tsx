import { Link, useLocation } from "@tanstack/react-router";
import { type ReactNode, useEffect, useState } from "react";
import { FileText, SplitSquareHorizontal, CreditCard } from "lucide-react";
import { resolveStoredBillingPeriod } from "@/lib/billing-period-selection";
import { ipc } from "@/lib/ipc";
import { MONTHS, type BillingPeriod } from "@/lib/types";
import { Badge } from "@/components/ui/badge";

type WorkflowSnapshot = {
  period: BillingPeriod | null;
  billCount: number;
  splitCount: number;
  apartmentCount: number;
};

const SELECTION_EVENT = "billing-period-selection-changed";

export function WorkflowStatusBar() {
  const location = useLocation();
  const [snapshot, setSnapshot] = useState<WorkflowSnapshot>({
    period: null,
    billCount: 0,
    splitCount: 0,
    apartmentCount: 0,
  });

  useEffect(() => {
    if (location.pathname === "/settings") return;

    const refresh = async () => {
      const periods = await ipc.getBillingPeriods();
      const period = resolveStoredBillingPeriod(periods);
      if (!period?.id) {
        setSnapshot({
          period: null,
          billCount: 0,
          splitCount: 0,
          apartmentCount: 0,
        });
        return;
      }

      const [bills, splits] = await Promise.all([
        ipc.getBills(period.id),
        ipc.getSplits(period.id),
      ]);

      setSnapshot({
        period,
        billCount: bills.length,
        splitCount: splits.length,
        apartmentCount: new Set(splits.map((split) => split.apartment_id)).size,
      });
    };

    void refresh();
    window.addEventListener(SELECTION_EVENT, refresh);
    return () => window.removeEventListener(SELECTION_EVENT, refresh);
  }, [location.pathname]);

  if (location.pathname === "/settings") return null;

  if (!snapshot.period) {
    return (
      <div className="mb-6 rounded-xl border border-border bg-card px-4 py-3">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div>
            <div className="text-sm font-semibold">No billing period selected</div>
            <div className="text-sm text-muted-foreground">
              Create a billing period in Bills to start the monthly workflow.
            </div>
          </div>
          <Link
            to="/bills"
            className="inline-flex h-8 items-center justify-center rounded-md bg-primary px-3 text-xs font-medium text-primary-foreground shadow hover:bg-primary/90"
          >
            Go to Bills
          </Link>
        </div>
      </div>
    );
  }

  const selectedLabel = `${MONTHS[snapshot.period.month - 1]} ${snapshot.period.year}`;
  const billsReady = snapshot.billCount > 0;
  const splitsReady = snapshot.splitCount > 0;
  const upnsReady = billsReady && splitsReady;

  return (
    <div className="mb-6 rounded-xl border border-border bg-card px-4 py-4 shadow-sm">
      <div className="flex flex-col gap-4">
        <div className="space-y-1">
          <div className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
            Current Workflow
          </div>
          <div className="text-lg font-semibold">{selectedLabel}</div>
          <div className="text-sm text-muted-foreground">
            Keep this same period across Bills, Splits, and UPN while you work.
          </div>
        </div>

        <div className="flex flex-wrap gap-2">
          <StatusChip
            icon={<FileText className="size-3.5" />}
            label={billsReady ? `${snapshot.billCount} bill${snapshot.billCount === 1 ? "" : "s"} imported` : "Import bills"}
            tone={billsReady ? "ready" : "waiting"}
            to="/bills"
          />
          <StatusChip
            icon={<SplitSquareHorizontal className="size-3.5" />}
            label={splitsReady ? `${snapshot.apartmentCount} apartment${snapshot.apartmentCount === 1 ? "" : "s"} split` : "Recalculate splits"}
            tone={splitsReady ? "ready" : "waiting"}
            to="/splits"
          />
          <StatusChip
            icon={<CreditCard className="size-3.5" />}
            label={upnsReady ? "UPNs ready to preview/send" : "UPNs waiting for splits"}
            tone={upnsReady ? "ready" : "waiting"}
            to="/upn"
          />
        </div>
      </div>
    </div>
  );
}

function StatusChip({
  icon,
  label,
  tone,
  to,
}: {
  icon: ReactNode;
  label: string;
  tone: "ready" | "waiting";
  to: "/bills" | "/splits" | "/upn";
}) {
  return (
    <Link to={to} className="inline-flex">
      <Badge
        variant="outline"
        className={
          tone === "ready"
            ? "gap-2 border-emerald-300 bg-emerald-50 text-emerald-700 hover:bg-emerald-100"
            : "gap-2 border-amber-300 bg-amber-50 text-amber-700 hover:bg-amber-100"
        }
      >
        {icon}
        {label}
      </Badge>
    </Link>
  );
}
