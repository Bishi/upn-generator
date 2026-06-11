import { useCallback, useMemo, useState } from "react";
import { useLocation } from "@tanstack/react-router";
import {
  Calendar,
  Check,
  ChevronDown,
  FileText,
  Layers,
  Loader2,
  Plus,
  Send,
} from "lucide-react";
import { notifyWorkflowStatusChanged } from "@/lib/workflow-status";
import { useBillingPeriodSelection } from "@/lib/billing-period-selection";
import { ipc } from "@/lib/ipc";
import { formatEur, type BillingPeriod } from "@/lib/types";
import {
  EMPTY_PERIOD_STATUS,
  type PeriodStatus,
  useWorkflowSnapshot,
} from "@/lib/workflow-snapshot";
import { cn } from "@/lib/utils";

const SHORT_MONTHS = [
  "Jan",
  "Feb",
  "Mar",
  "Apr",
  "May",
  "Jun",
  "Jul",
  "Aug",
  "Sep",
  "Oct",
  "Nov",
  "Dec",
];

type WorkflowContextBarProps = {
  snapshot: ReturnType<typeof useWorkflowSnapshot>;
};

type StepState = "done" | "now" | "todo" | "blocked";

export function WorkflowContextBar({ snapshot }: WorkflowContextBarProps) {
  const location = useLocation();
  const {
    allPeriods,
    years,
    selectedYear,
    selected,
    loadPeriods,
    setSelectedYear,
    setSelected,
  } = useBillingPeriodSelection();
  const [pickerOpen, setPickerOpen] = useState(false);
  const [creatingYear, setCreatingYear] = useState<number | null>(null);

  const selectedStatus = selected?.id
    ? snapshot.periodStatuses.get(selected.id) ?? snapshot.selectedStatus
    : EMPTY_PERIOD_STATUS;
  const billsReady = selectedStatus.bills;
  const splitsReady = selectedStatus.splits;
  const routeStage = location.pathname.includes("/splits")
    ? "splits"
    : location.pathname.includes("/upn")
      ? "upn"
      : location.pathname === "/"
        ? splitsReady
          ? "upn"
          : billsReady
            ? "splits"
            : "bills"
        : "bills";

  const createAndSelectYear = useCallback(
    async (year: number) => {
      setCreatingYear(year);
      try {
        await ipc.createYearPeriods(year);
        const periods = await loadPeriods();
        const preferredMonth = selected?.month ?? 1;
        const next =
          periods.find(
            (period) => period.year === year && period.month === preferredMonth,
          ) ??
          periods.find((period) => period.year === year && period.month === 1) ??
          null;
        if (next) setSelected(next);
        notifyWorkflowStatusChanged();
        await snapshot.refresh();
      } finally {
        setCreatingYear(null);
      }
    },
    [loadPeriods, selected, setSelected, snapshot],
  );

  const yearTabs = useMemo(() => {
    const currentYear = new Date().getFullYear();
    return [...new Set([...years, selectedYear, currentYear])].sort((a, b) => a - b);
  }, [selectedYear, years]);
  const yearHasPeriods = allPeriods.some((period) => period.year === selectedYear);
  const nextAddYear = yearHasPeriods
    ? Math.max(...yearTabs, new Date().getFullYear()) + 1
    : selectedYear;

  const periodLabel = selected
    ? `${SHORT_MONTHS[selected.month - 1]} ${selected.year}`
    : yearHasPeriods
      ? `Select ${selectedYear}`
      : `Add ${selectedYear}`;

  const steps: Array<{
    label: string;
    detail: string;
    state: StepState;
    icon: typeof FileText;
  }> = [
    {
      label: "Import bills",
      detail: billsReady
        ? `${selectedStatus.billCount} bill${selectedStatus.billCount === 1 ? "" : "s"}${selectedStatus.needsReview > 0 ? ` - ${selectedStatus.needsReview} review` : ""}`
        : selected
          ? "No bills yet"
          : "No period selected",
      state: billsReady ? "done" : routeStage === "bills" && selected ? "now" : "todo",
      icon: FileText,
    },
    {
      label: "Calculate splits",
      detail: splitsReady
        ? `${new Set(snapshot.splits.map((split) => split.apartment_id)).size} apartments`
        : billsReady
          ? "Ready to calculate"
          : "Waiting for bills",
      state: splitsReady
        ? "done"
        : !billsReady
          ? "blocked"
          : routeStage === "splits"
            ? "now"
            : "todo",
      icon: Layers,
    },
    {
      label: "Send UPNs",
      detail: splitsReady
        ? `${new Set(snapshot.splits.map((split) => split.apartment_id)).size} packets ready`
        : "Waiting for splits",
      state: !splitsReady ? "blocked" : routeStage === "upn" ? "now" : "todo",
      icon: Send,
    },
  ];

  return (
    <div className="relative z-20 flex min-h-[62px] items-center gap-4 border-b border-border bg-card px-6">
      <div className="relative shrink-0">
        <button
          type="button"
          onClick={() => setPickerOpen((open) => !open)}
          className={cn(
            "inline-flex h-8 items-center gap-2 rounded-md bg-accent px-3 text-xs font-semibold text-accent-foreground transition-shadow",
            pickerOpen && "shadow-[0_0_0_3px_var(--accent-soft-2)]",
          )}
        >
          <Calendar className="size-3.5" />
          {periodLabel}
          <ChevronDown className={cn("size-3 transition-transform", pickerOpen && "rotate-180")} />
        </button>
        {pickerOpen && (
          <PeriodPicker
            selected={selected}
            selectedYear={selectedYear}
            allPeriods={allPeriods}
            yearTabs={yearTabs}
            periodStatuses={snapshot.periodStatuses}
            creatingYear={creatingYear}
            nextAddYear={nextAddYear}
            onSelectYear={setSelectedYear}
            onSelectPeriod={(period) => {
              setSelected(period);
              setPickerOpen(false);
            }}
            onAddYear={createAndSelectYear}
          />
        )}
      </div>

      <div className="flex min-w-0 flex-1 items-center gap-3">
        {steps.map((step, index) => (
          <WorkflowStep
            key={step.label}
            index={index}
            step={step}
            railDone={index === 0 ? billsReady : index === 1 ? splitsReady : false}
          />
        ))}
      </div>

      <div className="shrink-0 min-w-36 text-right">
        <div className="text-[10px] font-medium text-muted-foreground">
          Total this billing month
        </div>
        <div className="font-mono text-sm font-semibold">
          {selectedStatus.bills ? `${formatEur(selectedStatus.totalCents)} €` : "-"}
        </div>
      </div>
    </div>
  );
}

function PeriodPicker({
  selected,
  selectedYear,
  allPeriods,
  yearTabs,
  periodStatuses,
  creatingYear,
  nextAddYear,
  onSelectYear,
  onSelectPeriod,
  onAddYear,
}: {
  selected: BillingPeriod | null;
  selectedYear: number;
  allPeriods: BillingPeriod[];
  yearTabs: number[];
  periodStatuses: Map<number, PeriodStatus>;
  creatingYear: number | null;
  nextAddYear: number;
  onSelectYear: (year: number) => void;
  onSelectPeriod: (period: BillingPeriod) => void;
  onAddYear: (year: number) => Promise<void>;
}) {
  const periodByMonth = new Map(
    allPeriods
      .filter((period) => period.year === selectedYear)
      .map((period) => [period.month, period]),
  );

  return (
    <div className="absolute left-0 top-[calc(100%+8px)] z-50 w-80 rounded-lg border border-border-2 bg-popover p-4 text-popover-foreground shadow-pop">
      <div className="mb-4 flex items-center gap-1.5">
        {yearTabs.map((year) => (
          <button
            key={year}
            type="button"
            onClick={() => onSelectYear(year)}
            className={cn(
              "h-7 rounded-md px-3 text-xs font-semibold transition-colors",
              selectedYear === year
                ? "bg-accent-soft text-accent-foreground"
                : "bg-surface-3 text-muted-foreground hover:bg-accent hover:text-accent-foreground",
            )}
          >
            {year}
          </button>
        ))}
        <button
          type="button"
          onClick={() => void onAddYear(nextAddYear)}
          disabled={creatingYear != null}
          className="ml-auto inline-flex h-7 items-center gap-1.5 rounded-md px-2 text-xs font-semibold text-accent-foreground hover:bg-accent disabled:opacity-60"
        >
          {creatingYear != null ? (
            <Loader2 className="size-3 animate-spin" />
          ) : (
            <Plus className="size-3" />
          )}
          Add year
        </button>
      </div>

      <div className="grid grid-cols-4 gap-1.5">
        {SHORT_MONTHS.map((monthName, index) => {
          const month = index + 1;
          const period = periodByMonth.get(month) ?? null;
          const status =
            period?.id != null
              ? periodStatuses.get(period.id) ?? EMPTY_PERIOD_STATUS
              : EMPTY_PERIOD_STATUS;
          const isSelected = selected?.id != null && selected.id === period?.id;

          return (
            <button
              key={month}
              type="button"
              disabled={!period}
              onClick={() => period && onSelectPeriod(period)}
              className={cn(
                "flex min-h-14 flex-col items-center justify-center gap-1 rounded-md border border-transparent px-2 py-2 text-xs transition-colors",
                isSelected
                  ? "bg-primary text-primary-foreground"
                  : period
                    ? "bg-surface-3 text-foreground hover:border-border-2 hover:bg-accent"
                    : "cursor-default bg-surface-2 text-muted-foreground opacity-50",
              )}
            >
              <span className="font-semibold">{monthName}</span>
              <span className="flex gap-1">
                <StatusDot active={status.bills} selected={isSelected} />
                <StatusDot active={status.splits} selected={isSelected} />
                <StatusDot active={status.sent} selected={isSelected} />
              </span>
            </button>
          );
        })}
      </div>

      <div className="mt-3 flex items-center gap-3 border-t border-border pt-3 text-[11px] text-muted-foreground">
        <LegendDot label="Bills" />
        <LegendDot label="Splits" />
        <LegendDot label="Sent" />
        <span className="ml-auto flex items-center gap-1.5">
          <span className="size-1.5 rounded-full bg-border-2" />
          Pending
        </span>
      </div>
      {periodByMonth.size === 0 && (
        <div className="mt-3 rounded-md bg-warning-soft px-3 py-2 text-xs text-warning">
          No months exist for {selectedYear}. Add the year to create all 12 months.
        </div>
      )}
    </div>
  );
}

function StatusDot({ active, selected }: { active: boolean; selected: boolean }) {
  return (
    <span
      className={cn(
        "size-1.5 rounded-full",
        selected
          ? active
            ? "bg-primary-foreground/80"
            : "bg-primary-foreground/35"
          : active
            ? "bg-success"
            : "bg-border-2",
      )}
    />
  );
}

function LegendDot({ label }: { label: string }) {
  return (
    <span className="flex items-center gap-1.5">
      <span className="size-1.5 rounded-full bg-success" />
      {label}
    </span>
  );
}

function WorkflowStep({
  index,
  step,
  railDone,
}: {
  index: number;
  step: {
    label: string;
    detail: string;
    state: StepState;
    icon: typeof FileText;
  };
  railDone: boolean;
}) {
  const Icon = step.icon;
  const done = step.state === "done";
  const now = step.state === "now";

  return (
    <>
      <div className="flex min-w-0 shrink-0 items-center gap-2.5">
        <div
          className={cn(
            "grid size-7 shrink-0 place-items-center rounded-full text-xs font-bold",
            done
              ? "bg-success text-primary-foreground"
              : now
                ? "bg-primary text-primary-foreground ring-4 ring-accent-soft"
                : "bg-surface-3 text-muted-foreground ring-1 ring-border-2",
          )}
        >
          {done ? <Check className="size-3.5" /> : now ? <Icon className="size-3.5" /> : index + 1}
        </div>
        <div className="min-w-0">
          <div
            className={cn(
              "truncate text-xs font-semibold",
              step.state === "blocked" && "text-muted-foreground",
            )}
          >
            {step.label}
          </div>
          <div className="max-w-36 truncate text-[11px] text-muted-foreground">
            {step.detail}
          </div>
        </div>
      </div>
      {index < 2 && (
        <div
          className={cn(
            "hidden h-px min-w-8 flex-1 rounded-full lg:block",
            railDone ? "bg-success" : "bg-border-2",
          )}
        />
      )}
    </>
  );
}
