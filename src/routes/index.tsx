import { createFileRoute, Link } from "@tanstack/react-router";
import { useMemo, type ReactNode } from "react";
import {
  AlertTriangle,
  ArrowRight,
  Banknote,
  Check,
  CheckCircle2,
  FilePlus,
  Inbox,
  Layers,
  Send,
  Users,
} from "lucide-react";
import { useBillingPeriodSelection } from "@/lib/billing-period-selection";
import type { BillingPeriod } from "@/lib/types";
import { formatEur, MONTHS } from "@/lib/types";
import { Card } from "@/components/ui/card";
import { useWorkflowSnapshot } from "@/lib/workflow-snapshot";

export const Route = createFileRoute("/")({
  component: DashboardPage,
});

type PeriodTotal = {
  period: BillingPeriod;
  totalCents: number;
};

function DashboardPage() {
  const { allPeriods, selected } = useBillingPeriodSelection();
  const snapshot = useWorkflowSnapshot(selected?.id, allPeriods);
  const { apartments, providers, bills, splits } = snapshot;
  const history = useMemo<PeriodTotal[]>(
    () =>
      [...allPeriods]
        .filter((period): period is BillingPeriod & { id: number } => period.id != null)
        .sort((a, b) => {
          if (a.year !== b.year) return b.year - a.year;
          return b.month - a.month;
        })
        .slice(0, 6)
        .reverse()
        .map((period) => ({
          period,
          totalCents:
            snapshot.periodStatuses.get(period.id)?.totalCents ?? 0,
        })),
    [allPeriods, snapshot.periodStatuses],
  );

  const totalCents = bills.reduce((sum, bill) => sum + bill.amount_cents, 0);
  const apartmentsWithSplits = new Set(splits.map((split) => split.apartment_id)).size;
  const needsReview = bills.filter((bill) => bill.parse_note?.trim()).length;
  const importedProviderCount = new Set(
    bills
      .map((bill) => bill.provider_id)
      .filter((providerId): providerId is number => providerId != null),
  ).size;
  const unmatchedBillCount = bills.filter((bill) => bill.provider_id == null).length;
  const selectedLabel = selected
    ? `${MONTHS[selected.month - 1]} ${selected.year}`
    : "No billing period";
  const buildingLabel = `${snapshot.buildingName}, ${snapshot.buildingCity}`;
  const billsReady = bills.length > 0;
  const splitsReady = splits.length > 0;
  const upnsReady = billsReady && splitsReady;
  const maxHistory = Math.max(1, ...history.map((entry) => entry.totalCents));

  const primaryAction = !billsReady
    ? { to: "/bills" as const, label: "Import bills", icon: <FilePlus className="size-4" /> }
    : !splitsReady
      ? { to: "/splits" as const, label: "Calculate splits", icon: <Layers className="size-4" /> }
      : { to: "/upn" as const, label: "Review & send", icon: <Send className="size-4" /> };

  const providerRows = useMemo(() => {
    const byProvider = new Map<string, { total: number; count: number }>();
    for (const bill of bills) {
      const key =
        bill.provider_name ?? (bill.creditor_name || bill.source_filename);
      const current = byProvider.get(key) ?? { total: 0, count: 0 };
      byProvider.set(key, {
        total: current.total + bill.amount_cents,
        count: current.count + 1,
      });
    }
    return [...byProvider.entries()]
      .map(([label, value]) => ({ label, ...value }))
      .sort((a, b) => b.total - a.total);
  }, [bills]);

  return (
    <div className="flex flex-col gap-5">
      <section className="flex flex-wrap items-start justify-between gap-4">
        <div>
          <div className="text-xs font-bold uppercase tracking-wide text-muted-foreground">
            Monthly workflow
          </div>
          <h2 className="mt-1 text-3xl font-semibold leading-tight">{selectedLabel}</h2>
          <p className="mt-1 text-sm text-muted-foreground">
            {buildingLabel} · {apartments.length} apartment{apartments.length === 1 ? "" : "s"}
          </p>
        </div>
        <div className="flex flex-wrap gap-2">
          {upnsReady && (
            <Link
              to="/bills"
              className="inline-flex h-9 items-center justify-center gap-2 rounded-md border border-input bg-card px-4 text-sm font-medium shadow-card transition-colors hover:bg-accent hover:text-accent-foreground"
            >
              <FilePlus className="size-4" />
              Import more
            </Link>
          )}
          <Link
            to={primaryAction.to}
            className="inline-flex h-9 items-center justify-center gap-2 rounded-md bg-primary px-4 text-sm font-medium text-primary-foreground shadow-card transition-colors hover:bg-primary/90"
          >
            {primaryAction.icon}
            {primaryAction.label}
          </Link>
        </div>
      </section>

      <Card className="p-4">
        <div className="flex flex-wrap items-center gap-4">
          <WorkflowStep
            state={billsReady ? "done" : "now"}
            number={1}
            label="Import bills"
            detail={
              billsReady
                ? `${bills.length} bill${bills.length === 1 ? "" : "s"} imported`
                : "No bills yet"
            }
          />
          <StepRail done={billsReady} />
          <WorkflowStep
            state={splitsReady ? "done" : billsReady ? "now" : "todo"}
            number={2}
            label="Calculate splits"
            detail={
              splitsReady
                ? `${apartmentsWithSplits} apartment${apartmentsWithSplits === 1 ? "" : "s"} split`
                : "Waiting for bills"
            }
          />
          <StepRail done={splitsReady} />
          <WorkflowStep
            state={upnsReady ? "now" : "todo"}
            number={3}
            label="Review & send UPNs"
            detail={upnsReady ? "Apartment packets ready" : "Waiting for splits"}
          />
          <div className="ml-auto min-w-32 text-right">
            <div className="text-xs text-muted-foreground">To collect this month</div>
            <div className="font-mono text-sm font-semibold">
              {billsReady ? `${formatEur(totalCents)} EUR` : "-"}
            </div>
          </div>
        </div>
      </Card>

      <div className="grid gap-4 lg:grid-cols-4">
        <StatTile
          icon={<Banknote className="size-4" />}
          label={splitsReady ? "Total this month" : "Total imported"}
          value={billsReady ? `${formatEur(totalCents)} EUR` : "-"}
          detail={billsReady ? `${bills.length} source bill${bills.length === 1 ? "" : "s"}` : "Import PDF or image bills"}
          tone="accent"
        />
        <StatTile
          icon={<Inbox className="size-4" />}
          label="Providers matched"
          value={
            billsReady
              ? providers.length > 0
                ? `${importedProviderCount} / ${providers.length}`
                : String(importedProviderCount)
              : "0"
          }
          detail={
            billsReady
              ? `${bills.length} bill${bills.length === 1 ? "" : "s"} imported${
                  unmatchedBillCount > 0
                    ? `, ${unmatchedBillCount} unmatched`
                    : ""
                }`
              : "Nothing imported yet"
          }
          tone={billsReady ? "good" : "neutral"}
        />
        <StatTile
          icon={<Users className="size-4" />}
          label="Apartments billed"
          value={splitsReady ? String(apartmentsWithSplits) : "-"}
          detail={splitsReady ? "Ready for packet preview" : "Calculate splits first"}
          tone={splitsReady ? "good" : "neutral"}
        />
        <StatTile
          icon={needsReview > 0 ? <AlertTriangle className="size-4" /> : <CheckCircle2 className="size-4" />}
          label="Needs review"
          value={snapshot.loading ? "Checking" : needsReview > 0 ? `${needsReview} bill${needsReview === 1 ? "" : "s"}` : "Clear"}
          detail={needsReview > 0 ? "OCR or parser note present" : "No flagged bill notes"}
          tone={needsReview > 0 ? "warn" : "good"}
        />
      </div>

      <div className="grid gap-4 xl:grid-cols-[minmax(0,1.5fr)_minmax(280px,1fr)]">
        <Card className="overflow-hidden">
          <div className="flex items-baseline justify-between border-b border-border px-5 py-4">
            <h3 className="text-sm font-semibold">Where the money goes</h3>
            <span className="text-xs text-muted-foreground">
              {providerRows.length > 0 ? `${providerRows.length} provider${providerRows.length === 1 ? "" : "s"}` : ""}
            </span>
          </div>
          {providerRows.length === 0 ? (
            <EmptyPanel
              icon={<FilePlus className="size-7" />}
              title="No bills yet"
              detail="Import a PDF or scan to start this monthly workflow."
            />
          ) : (
            <div className="divide-y divide-border px-5 py-2">
              {providerRows.map((provider) => {
                const share = totalCents > 0 ? provider.total / totalCents : 0;
                return (
                  <div key={provider.label} className="grid grid-cols-[minmax(140px,1fr)_minmax(120px,220px)_100px_44px] items-center gap-3 py-3 text-sm">
                    <div className="min-w-0">
                      <div className="truncate font-semibold">{provider.label}</div>
                      <div className="text-xs text-muted-foreground">
                        {provider.count} bill{provider.count === 1 ? "" : "s"}
                      </div>
                    </div>
                    <div className="h-1.5 overflow-hidden rounded-full bg-surface-3">
                      <div
                        className="h-full rounded-full bg-primary"
                        style={{ width: `${Math.max(4, Math.round(share * 100))}%` }}
                      />
                    </div>
                    <div className="text-right font-mono font-semibold">
                      {formatEur(provider.total)} EUR
                    </div>
                    <div className="text-right font-mono text-xs text-muted-foreground">
                      {Math.round(share * 100)}%
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </Card>

        <div className="flex flex-col gap-4">
          <Card className="p-5">
            <div className="mb-4 flex items-baseline justify-between">
              <h3 className="text-sm font-semibold">Monthly total</h3>
              <span className="rounded-full bg-surface-3 px-2.5 py-1 text-xs font-semibold text-muted-foreground">
                {history.length} mo
              </span>
            </div>
            <div className="flex h-20 items-end gap-2">
              {history.length === 0 ? (
                <div className="flex h-full w-full items-center justify-center text-sm text-muted-foreground">
                  History appears after bills are imported.
                </div>
              ) : (
                history.map((entry) => (
                  <div key={entry.period.id} className="flex h-full flex-1 flex-col justify-end gap-1">
                    <div
                      className={`min-h-1 rounded-t ${
                        entry.period.id === selected?.id ? "bg-primary" : "bg-accent-soft-2"
                      }`}
                      style={{
                        height: `${Math.max(6, Math.round((entry.totalCents / maxHistory) * 60))}px`,
                      }}
                    />
                    <span className="text-center text-[10px] text-muted-foreground">
                      {MONTHS[entry.period.month - 1].slice(0, 3)}
                    </span>
                  </div>
                ))
              )}
            </div>
          </Card>

          <Card className={`p-5 ${needsReview > 0 ? "border-warning bg-warning-soft" : ""}`}>
            <div className="flex gap-3">
              <span
                className={`mt-0.5 ${
                  needsReview > 0 ? "text-warning" : "text-success"
                }`}
              >
                {needsReview > 0 ? (
                  <AlertTriangle className="size-5" />
                ) : (
                  <CheckCircle2 className="size-5" />
                )}
              </span>
              <div>
                <h3 className={`text-sm font-semibold ${needsReview > 0 ? "text-warning" : ""}`}>
                  {needsReview > 0 ? `${needsReview} bill needs review` : "No alerts this period"}
                </h3>
                <p className="mt-1 text-sm text-muted-foreground">
                  {needsReview > 0
                    ? "Open Bills to verify parser and OCR notes before sending UPN packets."
                    : upnsReady
                      ? "Bills and splits are ready for UPN preview."
                      : "The next workflow step will appear here as the period progresses."}
                </p>
                {needsReview > 0 && (
                  <Link
                    to="/bills"
                    className="mt-3 inline-flex h-8 items-center gap-2 rounded-md bg-warning px-3 text-xs font-semibold text-white"
                  >
                    Open bills
                    <ArrowRight className="size-3.5" />
                  </Link>
                )}
              </div>
            </div>
          </Card>
        </div>
      </div>
    </div>
  );
}

function WorkflowStep({
  state,
  number,
  label,
  detail,
}: {
  state: "done" | "now" | "todo";
  number: number;
  label: string;
  detail: string;
}) {
  return (
    <div className="flex items-center gap-3">
      <div
        className={`grid size-8 shrink-0 place-items-center rounded-full text-sm font-bold ${
          state === "done"
            ? "bg-success text-primary-foreground"
            : state === "now"
              ? "bg-primary text-primary-foreground ring-4 ring-accent-soft"
              : "bg-surface-3 text-muted-foreground ring-1 ring-border-2"
        }`}
      >
        {state === "done" ? <Check className="size-4" /> : number}
      </div>
      <div>
        <div className={`text-sm font-semibold ${state === "todo" ? "text-muted-foreground" : ""}`}>
          {label}
        </div>
        <div className="text-xs text-muted-foreground">{detail}</div>
      </div>
    </div>
  );
}

function StepRail({ done }: { done: boolean }) {
  return (
    <div
      className={`hidden h-0.5 min-w-8 flex-1 rounded-full lg:block ${
        done ? "bg-success" : "bg-border-2"
      }`}
    />
  );
}

function StatTile({
  icon,
  label,
  value,
  detail,
  tone,
}: {
  icon: ReactNode;
  label: string;
  value: string;
  detail: string;
  tone: "accent" | "good" | "warn" | "neutral";
}) {
  const toneClass = {
    accent: "bg-accent-soft text-accent-foreground",
    good: "bg-success-soft text-success",
    warn: "bg-warning-soft text-warning",
    neutral: "bg-surface-3 text-muted-foreground",
  }[tone];

  return (
    <Card className="flex min-h-28 flex-col gap-2 p-4">
      <div className="flex items-center justify-between">
        <span className="text-xs font-semibold text-muted-foreground">{label}</span>
        <span className={`grid size-7 place-items-center rounded-md ${toneClass}`}>
          {icon}
        </span>
      </div>
      <div className="font-mono text-2xl font-semibold leading-none">{value}</div>
      <div className="text-xs text-muted-foreground">{detail}</div>
    </Card>
  );
}

function EmptyPanel({
  icon,
  title,
  detail,
}: {
  icon: ReactNode;
  title: string;
  detail: string;
}) {
  return (
    <div className="flex min-h-56 items-center justify-center px-6 py-10 text-center">
      <div>
        <div className="mx-auto mb-3 grid size-12 place-items-center rounded-lg bg-surface-3 text-muted-foreground">
          {icon}
        </div>
        <div className="text-sm font-semibold">{title}</div>
        <div className="mt-1 text-sm text-muted-foreground">{detail}</div>
      </div>
    </div>
  );
}
