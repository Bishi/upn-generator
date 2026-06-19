import { createFileRoute, Link } from "@tanstack/react-router";
import { useEffect, useMemo, useState, type ReactNode } from "react";
import {
  AlertTriangle,
  ArrowRight,
  Banknote,
  CheckCircle2,
  FilePlus,
  Inbox,
  Layers,
  Users,
} from "lucide-react";
import { useBillingPeriodSelection } from "@/lib/billing-period-selection";
import { serviceIconFor } from "@/lib/service-icons";
import type { BillingPeriod, Provider } from "@/lib/types";
import { formatEur, MONTHS } from "@/lib/types";
import {
  createVirtualBillingPeriod,
  useWorkflowSnapshotContext,
} from "@/lib/workflow-snapshot";
import { Card } from "@/components/ui/card";

export const Route = createFileRoute("/")({
  component: DashboardPage,
});

type PeriodTotal = {
  periodId: number | null;
  month: number;
  year: number;
  isAnchor: boolean;
  totalCents: number;
};

type ProviderRow = {
  key: string;
  label: string;
  providerName: string;
  total: number;
  count: number;
};

function DashboardPage() {
  const { allPeriods, selected, setSelected } = useBillingPeriodSelection();
  const snapshot = useWorkflowSnapshotContext();
  const { apartments, providers, bills, splits } = snapshot;
  const [historySelected, setHistorySelected] = useState<BillingPeriod | null>(selected);
  const selectedStatusKnown =
    selected?.id != null && snapshot.periodStatuses.has(selected.id);
  const useStatusSummary = snapshot.loading && selectedStatusKnown;
  const selectedStatus = snapshot.selectedStatus;

  useEffect(() => {
    if (!snapshot.loading) {
      setHistorySelected(selected);
    }
  }, [selected, snapshot.loading]);

  const history = useMemo<PeriodTotal[]>(
    () => {
      const now = new Date();
      const anchorYear = historySelected?.year ?? now.getFullYear();
      const anchorMonth = historySelected?.month ?? now.getMonth() + 1;
      const anchorValue = anchorYear * 12 + anchorMonth;
      const periodsByMonth = new Map<number, BillingPeriod & { id: number }>();

      allPeriods
        .filter((period): period is BillingPeriod & { id: number } => {
          if (period.id == null) return false;
          const periodValue = period.year * 12 + period.month;
          return periodValue <= anchorValue;
        })
        .forEach((period) => {
          periodsByMonth.set(period.year * 12 + period.month, period);
        });

      return Array.from({ length: 6 }, (_, index) => {
        const monthValue = anchorValue - 5 + index;
        const year = Math.floor((monthValue - 1) / 12);
        const month = ((monthValue - 1) % 12) + 1;
        const period = periodsByMonth.get(monthValue);

        return {
          periodId: period?.id ?? null,
          month,
          year,
          isAnchor: monthValue === anchorValue,
          totalCents: period
            ? snapshot.periodStatuses.get(period.id)?.totalCents ?? 0
            : 0,
        };
      });
    },
    [allPeriods, historySelected, snapshot.periodStatuses],
  );

  const totalCents = useStatusSummary
    ? selectedStatus.totalCents
    : bills.reduce((sum, bill) => sum + bill.amount_cents, 0);
  const billCount = useStatusSummary ? selectedStatus.billCount : bills.length;
  const splitCount = useStatusSummary ? selectedStatus.splitCount : splits.length;
  const apartmentsWithSplits = useStatusSummary
    ? Math.min(apartments.length, selectedStatus.splitCount)
    : new Set(splits.map((split) => split.apartment_id)).size;
  const reviewBills = useStatusSummary
    ? []
    : bills.filter(
        (bill) =>
          (bill.parse_note?.trim() || bill.status === "needs_review") &&
          !bill.reviewed_at?.trim(),
      );
  const firstReviewBill = reviewBills[0] ?? null;
  const needsReview = useStatusSummary ? selectedStatus.needsReview : reviewBills.length;
  const validationErrorCount =
    snapshot.selectedPreSendValidation?.error_count ??
    selectedStatus.validationErrorCount ??
    0;
  const validationWarningCount =
    snapshot.selectedPreSendValidation?.warning_count ??
    selectedStatus.validationWarningCount ??
    0;
  const unmatchedBillCount = useStatusSummary
    ? 0
    : bills.filter((bill) => bill.provider_id == null).length;
  const selectedLabel = selected
    ? `${MONTHS[selected.month - 1]} ${selected.year}`
    : "No billing period";
  const buildingLabel = `${snapshot.buildingName}, ${snapshot.buildingCity}`;
  const billsReady = billCount > 0;
  const splitsReady = splitCount > 0;
  const upnsReady = billsReady && splitsReady;
  const deliveryComplete = selectedStatus.sent;
  const maxHistory = Math.max(1, ...history.map((entry) => entry.totalCents));
  const selectedHistoryIndex = history.findIndex((entry) =>
    historySelected?.id != null ? entry.periodId === historySelected.id : entry.isAnchor,
  );
  const previousHistory =
    selectedHistoryIndex > 0 ? history[selectedHistoryIndex - 1] : null;
  const monthlyDelta =
    previousHistory && previousHistory.totalCents > 0
      ? ((totalCents - previousHistory.totalCents) / previousHistory.totalCents) * 100
      : null;
  const totalDetail =
    !billsReady
      ? "No bills imported yet"
      : monthlyDelta != null
        ? `${monthlyDelta >= 0 ? "+" : ""}${monthlyDelta.toFixed(1)}% vs ${
            MONTHS[previousHistory!.month - 1]
          }`
        : `${billCount} source bill${billCount === 1 ? "" : "s"}`;
  const expectedBillCount = providers.length || billCount;
  const billsImportedValue = billsReady
    ? expectedBillCount > 0
      ? `${billCount} / ${expectedBillCount}`
      : String(billCount)
    : expectedBillCount > 0
      ? `0 / ${expectedBillCount}`
      : "0";
  const billsImportedDetail = !billsReady
    ? "Import from PDF or scan"
    : unmatchedBillCount > 0
      ? `${unmatchedBillCount} bill${unmatchedBillCount === 1 ? "" : "s"} unmatched`
      : "All providers detected";
  const apartmentsBilledDetail =
    splitsReady && apartments.length > 0 && apartmentsWithSplits >= apartments.length
      ? "100% of m2 allocated"
      : splitsReady
        ? `${apartmentsWithSplits} of ${apartments.length} apartments allocated`
        : "Calculate splits first";
  const reviewLabel =
    firstReviewBill?.provider_name ??
    firstReviewBill?.creditor_name ??
    firstReviewBill?.source_filename ??
    "Bill";
  const reviewNote = firstReviewBill?.parse_note?.trim() ?? "";
  const reviewDetail =
    needsReview > 0
      ? firstReviewBill
        ? reviewLabel
        : "Review flagged bill notes"
      : "No flagged bill notes";
  const dashboardChecking = snapshot.loading && !selectedStatusKnown;
  const goToNextMonth = () => {
    if (!selected) return;
    const nextMonthValue = selected.year * 12 + selected.month;
    const nextYear = Math.floor(nextMonthValue / 12);
    const nextMonth = (nextMonthValue % 12) + 1;
    const existing =
      allPeriods.find(
        (period) => period.year === nextYear && period.month === nextMonth,
      ) ?? createVirtualBillingPeriod(nextMonth, nextYear);
    setSelected(existing);
  };

  const providerById = useMemo(
    () =>
      new Map(
        providers
          .filter((provider): provider is Provider & { id: number } => provider.id != null)
          .map((provider) => [provider.id, provider]),
      ),
    [providers],
  );

  const providerRows = useMemo<ProviderRow[]>(() => {
    const byProvider = new Map<string, ProviderRow>();

    for (const bill of bills) {
      const provider = bill.provider_id != null ? providerById.get(bill.provider_id) : null;
      const key = provider?.id != null ? `provider-${provider.id}` : `bill-${bill.id}`;
      const label =
        provider?.service_type?.trim() ||
        provider?.name?.trim() ||
        bill.provider_name?.trim() ||
        bill.creditor_name?.trim() ||
        bill.source_filename;
      const providerName =
        provider?.name?.trim() ||
        bill.provider_name?.trim() ||
        bill.creditor_name?.trim() ||
        bill.source_filename;
      const current =
        byProvider.get(key) ?? {
          key,
          label,
          providerName,
          total: 0,
          count: 0,
        };

      byProvider.set(key, {
        ...current,
        total: current.total + bill.amount_cents,
        count: current.count + 1,
      });
    }

    return [...byProvider.values()].sort((a, b) => b.total - a.total);
  }, [bills, providerById]);

  return (
    <div className="flex flex-col gap-5">
      <section className="flex flex-wrap items-start justify-between gap-4">
        <div>
          <h2 className="text-3xl font-semibold leading-tight">{selectedLabel}</h2>
          <p className="mt-1 text-sm text-muted-foreground">
            {buildingLabel} - {apartments.length} apartment
            {apartments.length === 1 ? "" : "s"}
          </p>
        </div>

        <DashboardActions
          billsReady={billsReady}
          splitsReady={splitsReady}
          deliveryComplete={deliveryComplete}
          onNextMonth={goToNextMonth}
        />
      </section>

      <div className="grid gap-4 lg:grid-cols-4">
        <StatTile
          icon={<Banknote className="size-4" />}
          label={splitsReady ? "Total this month" : "Total imported"}
          value={billsReady ? `${formatEur(totalCents)} €` : "-"}
          detail={totalDetail}
          tone={billsReady ? "accent" : "neutral"}
        />
        <StatTile
          icon={<Inbox className="size-4" />}
          label="Bills imported"
          value={billsImportedValue}
          detail={billsImportedDetail}
          tone={billsReady ? "good" : "neutral"}
        />
        <StatTile
          icon={<Users className="size-4" />}
          label="Apartments billed"
          value={splitsReady ? String(apartmentsWithSplits) : "-"}
          detail={apartmentsBilledDetail}
          tone={splitsReady ? "good" : "neutral"}
        />
        <StatTile
          icon={
            needsReview > 0 ? (
              <AlertTriangle className="size-4" />
            ) : (
              <CheckCircle2 className="size-4" />
            )
          }
          label="Needs review"
          value={
            dashboardChecking
              ? "Checking"
              : needsReview > 0
                ? `${needsReview} bill${needsReview === 1 ? "" : "s"}`
                : "Clear"
          }
          detail={dashboardChecking ? "Checking billing month status" : reviewDetail}
          tone={dashboardChecking ? "neutral" : needsReview > 0 ? "warn" : "good"}
        />
      </div>

      <div className="grid gap-4 xl:grid-cols-[minmax(0,1.5fr)_minmax(280px,1fr)]">
        <Card className="overflow-hidden">
          <div className="flex items-baseline justify-between border-b border-border px-5 py-4">
            <h3 className="text-sm font-semibold">Where the money goes</h3>
            <span className="text-xs text-muted-foreground">
              {providerRows.length > 0
                ? `${providerRows.length} provider${providerRows.length === 1 ? "" : "s"}`
                : ""}
            </span>
          </div>
          {snapshot.loading && billsReady && providerRows.length === 0 ? (
            <EmptyPanel
              icon={<Banknote className="size-7" />}
              title="Loading bills"
              detail="Checking the selected billing month."
            />
          ) : providerRows.length === 0 ? (
            <EmptyPanel
              icon={<FilePlus className="size-7" />}
              title="No bills yet"
              detail="Import a PDF or scan to start this monthly workflow."
            />
          ) : (
            <div className="divide-y divide-border px-5 py-2">
              {providerRows.map((provider) => {
                const share = totalCents > 0 ? provider.total / totalCents : 0;
                const ProviderIcon = serviceIconFor(provider.label, provider.providerName);
                return (
                  <div
                    key={provider.key}
                    className="grid grid-cols-[30px_minmax(120px,1fr)_minmax(100px,220px)_100px_44px] items-center gap-3 py-3 text-sm"
                  >
                    <span className="grid size-8 place-items-center rounded-md bg-surface-3 text-muted-foreground">
                      <ProviderIcon className="size-4" />
                    </span>
                    <div className="min-w-0">
                      <div className="truncate font-semibold">{provider.label}</div>
                      <div className="truncate text-xs text-muted-foreground">
                        {provider.providerName}
                      </div>
                    </div>
                    <div className="h-1.5 overflow-hidden rounded-full bg-surface-3">
                      <div
                        className="h-full rounded-full bg-primary"
                        style={{ width: `${Math.max(4, Math.round(share * 100))}%` }}
                      />
                    </div>
                    <div className="text-right font-mono font-semibold">
                      {formatEur(provider.total)} €
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
            <div className="mb-4 flex items-baseline justify-between gap-3">
              <h3 className="text-sm font-semibold">Monthly total</h3>
              <div className="flex items-baseline gap-2">
                {billsReady && (
                  <span className="font-mono text-sm font-semibold">
                    {formatEur(totalCents)} €
                  </span>
                )}
                <span className="rounded-full bg-surface-3 px-2.5 py-1 text-xs font-semibold text-muted-foreground">
                  {history.length} mo
                </span>
              </div>
            </div>
            <div className="flex h-20 items-end gap-2">
              {history.length === 0 ? (
                <div className="flex h-full w-full items-center justify-center text-sm text-muted-foreground">
                  History appears after bills are imported.
                </div>
              ) : (
                history.map((entry) => {
                  const monthLabel = `${MONTHS[entry.month - 1]} ${entry.year}`;
                  const totalLabel = `${formatEur(entry.totalCents)} €`;
                  const isSelected =
                    historySelected?.id != null
                      ? entry.periodId === historySelected.id
                      : entry.isAnchor;

                  return (
                    <div
                      key={`${entry.year}-${entry.month}`}
                      tabIndex={0}
                      role="img"
                      aria-label={`${monthLabel}: ${totalLabel}`}
                      className="group relative flex h-full flex-1 cursor-default flex-col justify-end gap-1 rounded-sm outline-none"
                    >
                      <span className="pointer-events-none absolute bottom-7 left-1/2 z-10 w-max max-w-32 -translate-x-1/2 rounded-md bg-popover px-2 py-1 text-center text-[11px] font-semibold text-popover-foreground opacity-0 shadow-pop ring-1 ring-border transition duration-150 group-hover:-translate-y-1 group-hover:opacity-100 group-focus-visible:-translate-y-1 group-focus-visible:opacity-100">
                        <span className="block font-mono">{totalLabel}</span>
                        <span className="block text-[10px] font-medium text-muted-foreground">
                          {monthLabel}
                        </span>
                      </span>
                      <div
                        className={`min-h-1 rounded-t transition-[transform,box-shadow] duration-150 group-hover:-translate-y-0.5 group-hover:shadow-sm group-focus-visible:-translate-y-0.5 ${
                          isSelected
                            ? "bg-primary group-hover:bg-accent-strong"
                            : "bg-accent-soft-2 group-hover:bg-primary"
                        }`}
                        style={{
                          height: `${Math.max(
                            6,
                            Math.round((entry.totalCents / maxHistory) * 60),
                          )}px`,
                        }}
                      />
                      <span className="text-center text-[10px] text-muted-foreground">
                        {MONTHS[entry.month - 1].slice(0, 3)}
                      </span>
                    </div>
                  );
                })
              )}
            </div>
          </Card>

          <Card
            className={`p-5 ${
              validationErrorCount > 0
                ? "border-danger bg-danger-soft"
                : needsReview > 0 || validationWarningCount > 0
                  ? "border-warning bg-warning-soft"
                  : ""
            }`}
          >
            <div className="flex gap-3">
              <span
                className={`mt-0.5 ${
                  validationErrorCount > 0
                    ? "text-danger"
                    : needsReview > 0 || validationWarningCount > 0
                      ? "text-warning"
                      : "text-success"
                }`}
              >
                {validationErrorCount > 0 || needsReview > 0 || validationWarningCount > 0 ? (
                  <AlertTriangle className="size-5" />
                ) : (
                  <CheckCircle2 className="size-5" />
                )}
              </span>
              <div>
                <h3
                  className={`text-sm font-semibold ${
                    validationErrorCount > 0
                      ? "text-danger"
                      : needsReview > 0 || validationWarningCount > 0
                        ? "text-warning"
                        : ""
                  }`}
                >
                  {validationErrorCount > 0
                    ? `UPN validation blocked: ${validationErrorCount} issue${validationErrorCount === 1 ? "" : "s"}`
                    : needsReview > 0
                    ? `${needsReview} bill${needsReview === 1 ? "" : "s"} needs review`
                    : validationWarningCount > 0
                      ? `${validationWarningCount} UPN validation warning${validationWarningCount === 1 ? "" : "s"}`
                    : "No alerts this billing month"}
                </h3>
                <p className="mt-1 text-sm text-muted-foreground">
                  {validationErrorCount > 0
                    ? "Open UPN Preview to fix blockers before sending emails, marking delivered, or exporting PDFs."
                    : needsReview > 0
                    ? firstReviewBill
                      ? `${reviewLabel} - ${reviewNote || "Verify parser and OCR notes before sending UPN packets."}`
                      : "Review flagged bill notes before sending UPN packets."
                    : validationWarningCount > 0
                      ? "Review warnings before closing out delivery."
                    : upnsReady
                      ? "Bills and splits are ready for UPN preview."
                      : "The next workflow step will appear here as the period progresses."}
                </p>
                {validationErrorCount > 0 ? (
                  <Link
                    to="/upn"
                    className="mt-3 inline-flex h-8 items-center gap-2 rounded-md bg-danger px-3 text-xs font-semibold text-white"
                  >
                    Open UPN
                    <ArrowRight className="size-3.5" />
                  </Link>
                ) : needsReview > 0 ? (
                  <Link
                    to="/bills"
                    className="mt-3 inline-flex h-8 items-center gap-2 rounded-md bg-warning px-3 text-xs font-semibold text-white"
                  >
                    Open bills
                    <ArrowRight className="size-3.5" />
                  </Link>
                ) : null}
              </div>
            </div>
          </Card>
        </div>
      </div>
    </div>
  );
}

function DashboardActions({
  billsReady,
  splitsReady,
  deliveryComplete,
  onNextMonth,
}: {
  billsReady: boolean;
  splitsReady: boolean;
  deliveryComplete: boolean;
  onNextMonth: () => void;
}) {
  if (!billsReady) {
    return (
      <Link
        to="/bills"
        className="inline-flex h-9 items-center justify-center gap-2 rounded-md bg-primary px-4 text-sm font-medium text-primary-foreground shadow-card transition-colors hover:bg-primary/90"
      >
        <FilePlus className="size-4" />
        Go to Import Bills
      </Link>
    );
  }

  if (!splitsReady) {
    return (
      <div className="flex flex-wrap gap-2">
        <Link
          to="/bills"
          className="inline-flex h-9 items-center justify-center gap-2 rounded-md border border-input bg-card px-4 text-sm font-medium shadow-card transition-colors hover:bg-accent hover:text-accent-foreground"
        >
          <FilePlus className="size-4" />
          Import more
        </Link>
        <Link
          to="/splits"
          className="inline-flex h-9 items-center justify-center gap-2 rounded-md bg-primary px-4 text-sm font-medium text-primary-foreground shadow-card transition-colors hover:bg-primary/90"
        >
          <Layers className="size-4" />
          Calculate splits
        </Link>
      </div>
    );
  }

  if (deliveryComplete) {
    return (
      <div className="flex flex-wrap gap-2">
        <Link
          to="/upn"
          className="inline-flex h-9 items-center justify-center gap-2 rounded-md border border-input bg-card px-4 text-sm font-medium shadow-card transition-colors hover:bg-accent hover:text-accent-foreground"
        >
          <CheckCircle2 className="size-4" />
          View history
        </Link>
        <button
          type="button"
          onClick={onNextMonth}
          className="inline-flex h-9 items-center justify-center gap-2 rounded-md bg-primary px-4 text-sm font-medium text-primary-foreground shadow-card transition-colors hover:bg-primary/90"
        >
          <ArrowRight className="size-4" />
          Next month
        </button>
      </div>
    );
  }

  return (
    <Link
      to="/upn"
      className="inline-flex h-9 items-center justify-center gap-2 rounded-md bg-primary px-4 text-sm font-medium text-primary-foreground shadow-card transition-colors hover:bg-primary/90"
    >
      <ArrowRight className="size-4" />
      Open UPNs
    </Link>
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
