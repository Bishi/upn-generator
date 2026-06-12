import { useCallback, useEffect, useRef, useState } from "react";
import { useLocation } from "@tanstack/react-router";
import {
  Calendar,
  Check,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  FileText,
  Layers,
  Send,
} from "lucide-react";
import { useBillingPeriodSelection } from "@/lib/billing-period-selection";
import { formatEur, type BillingPeriod } from "@/lib/types";
import {
  createVirtualBillingPeriod,
  EMPTY_PERIOD_STATUS,
  type PeriodStatus,
  type WorkflowSnapshot,
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
  snapshot: WorkflowSnapshot;
};

type StepState = "done" | "now" | "todo" | "blocked";

export function WorkflowContextBar({ snapshot }: WorkflowContextBarProps) {
  const location = useLocation();
  const {
    allPeriods,
    selectedYear,
    selected,
    setSelectedYear,
    setSelected,
  } = useBillingPeriodSelection();
  const [pickerOpen, setPickerOpen] = useState(false);
  const pickerRef = useRef<HTMLDivElement | null>(null);
  const closePicker = useCallback(() => {
    setPickerOpen(false);
    const currentYear = new Date().getFullYear();
    if (selectedYear !== currentYear) setSelectedYear(currentYear);
  }, [selectedYear, setSelectedYear]);

  useEffect(() => {
    if (!pickerOpen) return;

    const onPointerDown = (event: PointerEvent) => {
      const target = event.target as Node;
      if (!pickerRef.current?.contains(target)) closePicker();
    };
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      event.stopPropagation();
      closePicker();
    };

    window.addEventListener("pointerdown", onPointerDown, true);
    window.addEventListener("keydown", onKeyDown, true);
    return () => {
      window.removeEventListener("pointerdown", onPointerDown, true);
      window.removeEventListener("keydown", onKeyDown, true);
    };
  }, [closePicker, pickerOpen]);

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

  const periodLabel = selected
    ? `${SHORT_MONTHS[selected.month - 1]} ${selected.year}`
    : "Pick month";

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
      detail: selectedStatus.packetCount > 0
        ? `${selectedStatus.deliveredCount}/${selectedStatus.packetCount} delivered`
        : splitsReady
          ? `${new Set(snapshot.splits.map((split) => split.apartment_id)).size} packets ready`
          : "Waiting for splits",
      state: !splitsReady
        ? "blocked"
        : selectedStatus.sent
          ? "done"
          : routeStage === "upn"
            ? "now"
            : "todo",
      icon: Send,
    },
  ];

  return (
    <div className="relative z-20 flex min-h-[62px] items-center gap-4 border-b border-border bg-card px-6">
      <div ref={pickerRef} className="relative shrink-0">
        <button
          type="button"
          onClick={() => {
            if (pickerOpen) {
              closePicker();
            } else {
              setPickerOpen(true);
            }
          }}
          className={cn(
            "inline-grid h-8 w-32 grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-2 rounded-md bg-accent px-3 text-xs font-semibold text-accent-foreground transition-shadow",
            pickerOpen && "shadow-[0_0_0_3px_var(--accent-soft-2)]",
          )}
        >
          <Calendar className="size-3.5" />
          <span className="truncate text-center">{periodLabel}</span>
          <ChevronDown className={cn("size-3 transition-transform", pickerOpen && "rotate-180")} />
        </button>
        {pickerOpen && (
          <PeriodPicker
            selected={selected}
            selectedYear={selectedYear}
            allPeriods={allPeriods}
            periodStatuses={snapshot.periodStatuses}
            onSelectYear={setSelectedYear}
            onSelectMonth={(month) => {
              const period =
                allPeriods.find(
                  (candidate) =>
                    candidate.year === selectedYear && candidate.month === month,
                ) ?? createVirtualBillingPeriod(month, selectedYear);
              setSelected(period);
              closePicker();
            }}
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
  periodStatuses,
  onSelectYear,
  onSelectMonth,
}: {
  selected: BillingPeriod | null;
  selectedYear: number;
  allPeriods: BillingPeriod[];
  periodStatuses: Map<number, PeriodStatus>;
  onSelectYear: (year: number) => void;
  onSelectMonth: (month: number) => void;
}) {
  const periodByMonth = new Map(
    allPeriods
      .filter((period) => period.year === selectedYear)
      .map((period) => [period.month, period]),
  );

  return (
    <div className="absolute left-0 top-[calc(100%+8px)] z-50 w-80 rounded-lg border border-border-2 bg-popover p-4 text-popover-foreground shadow-pop">
      <div className="mb-4 grid h-8 grid-cols-[2rem_minmax(0,1fr)_2rem] items-center gap-2">
        <button
          type="button"
          aria-label="Previous year"
          title="Previous year"
          disabled={selectedYear <= 1900}
          onClick={() => onSelectYear(selectedYear - 1)}
          className="grid size-8 place-items-center rounded-md bg-surface-3 text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground disabled:cursor-default disabled:opacity-40"
        >
          <ChevronLeft className="size-4" />
        </button>
        <div className="select-none text-center text-sm font-semibold tabular-nums text-foreground">
          {selectedYear}
        </div>
        <button
          type="button"
          aria-label="Next year"
          title="Next year"
          disabled={selectedYear >= 9999}
          onClick={() => onSelectYear(selectedYear + 1)}
          className="grid size-8 place-items-center rounded-md bg-surface-3 text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground disabled:cursor-default disabled:opacity-40"
        >
          <ChevronRight className="size-4" />
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
          const isSelected = selected?.year === selectedYear && selected.month === month;

          return (
            <button
              key={month}
              type="button"
              onClick={() => onSelectMonth(month)}
              className={cn(
                "flex min-h-14 flex-col items-center justify-center gap-1 rounded-md border border-transparent px-2 py-2 text-xs transition-colors",
                isSelected
                  ? "bg-primary text-primary-foreground"
                  : "bg-surface-3 text-foreground hover:border-border-2 hover:bg-accent",
              )}
            >
              <span className="font-semibold">{monthName}</span>
              <span className="flex gap-1">
                <StatusDot active={status.bills} selected={isSelected} />
                <StatusDot active={status.splits} selected={isSelected} />
              </span>
            </button>
          );
        })}
      </div>

      <div className="mt-3 flex items-center gap-3 border-t border-border pt-3 text-[11px] text-muted-foreground">
        <LegendDot label="Bills" />
        <LegendDot label="Splits" />
        <span className="ml-auto flex items-center gap-1.5">
          <span className="size-1.5 rounded-full bg-border-2" />
          Pending
        </span>
      </div>
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
