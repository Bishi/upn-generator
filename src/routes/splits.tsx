import { createFileRoute, Link } from "@tanstack/react-router";
import { useEffect, useRef, useState } from "react";
import { Loader2, RefreshCw, Check, X } from "lucide-react";
import { notifyWorkflowStatusChanged } from "@/lib/workflow-status";
import { ipc } from "@/lib/ipc";
import { useBillingPeriodSelection } from "@/lib/billing-period-selection";
import { useWorkflowSnapshotContext } from "@/lib/workflow-snapshot";
import type { SplitRow } from "@/lib/types";
import { formatEur } from "@/lib/types";
import { BillingPageShell } from "@/components/BillingPageShell";
import { Button, buttonVariants } from "@/components/ui/button";
import { Input } from "@/components/ui/input";

export const Route = createFileRoute("/splits")({
  component: SplitsPage,
});

function ReviewIndicator({ note }: { note: string }) {
  return (
    <span
      className="mt-0.5 inline-flex size-2.5 shrink-0 cursor-help rounded-full bg-warning ring-1 ring-warning/60"
      title={note}
      aria-label={note}
    />
  );
}

function splitBasisLabel(splitBasis: SplitRow["split_basis"]) {
  switch (splitBasis) {
    case "occupants":
      return "Split by people";
    case "equal_apartments":
      return "Split equally";
    default:
      return "Split by m2";
  }
}

function splitBasisDetail(split: SplitRow) {
  switch (split.split_basis) {
    case "occupants":
      return `${split.occupant_count} people`;
    case "equal_apartments":
      return "Equal share";
    default:
      return `${split.m2_percentage.toFixed(2)}%`;
  }
}

function buildMatrix(splits: SplitRow[]) {
  const apartments = [
    ...new Map(
      splits.map((s) => [
        s.apartment_id,
        { label: s.apartment_label, unitCode: s.apartment_unit_code },
      ]),
    ).entries(),
  ].sort((a, b) => a[1].label.localeCompare(b[1].label));
  const bills = [
    ...new Map(
      splits.map((s) => [
        s.bill_id,
        {
          filename: s.bill_source_filename,
          provider: s.provider_name,
          total: s.bill_amount_cents,
          splitBasis: s.split_basis,
          parseNote: s.bill_parse_note,
        },
      ]),
    ).entries(),
  ];

  const matrix: Map<number, Map<number, SplitRow>> = new Map();
  for (const s of splits) {
    if (!matrix.has(s.bill_id)) matrix.set(s.bill_id, new Map());
    matrix.get(s.bill_id)!.set(s.apartment_id, s);
  }

  return { apartments, bills, matrix };
}

function EditableCell({
  split,
  onSave,
}: {
  split: SplitRow;
  onSave: (splitId: number, cents: number) => void;
}) {
  const [editing, setEditing] = useState(false);
  const [value, setValue] = useState(String(split.split_amount_cents / 100));

  if (editing) {
    return (
      <div className="flex items-center gap-1">
        <Input
          className="h-6 w-20 text-xs font-mono"
          value={value}
          onChange={(e) => setValue(e.target.value)}
          autoFocus
        />
        <button
          className="text-success hover:text-success/80"
          onClick={() => {
            const cents = Math.round(parseFloat(value) * 100) || 0;
            if (split.split_id) onSave(split.split_id, cents);
            setEditing(false);
          }}
        >
          <Check className="size-3" />
        </button>
        <button
          className="text-muted-foreground hover:text-foreground"
          onClick={() => {
            setValue(String(split.split_amount_cents / 100));
            setEditing(false);
          }}
        >
          <X className="size-3" />
        </button>
      </div>
    );
  }

  return (
    <span
      className="font-mono text-sm cursor-pointer hover:underline"
      onClick={() => setEditing(true)}
    >
      {formatEur(split.split_amount_cents)} €
    </span>
  );
}

function SplitsPage() {
  const { selected } = useBillingPeriodSelection();
  const snapshot = useWorkflowSnapshotContext();
  const [splits, setSplits] = useState<SplitRow[]>(() => snapshot.splits);
  const [loadingSplits, setLoadingSplits] = useState(() => snapshot.loading);
  const [calculating, setCalculating] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const loadRequestRef = useRef(0);
  const loadedPeriodIdRef = useRef<number | null>(snapshot.loading ? null : selected?.id ?? null);

  const loadSplits = async (periodId: number) => {
    const rows = await ipc.getSplits(periodId);
    setSplits(rows);
  };

  useEffect(() => {
    const requestId = ++loadRequestRef.current;
    if (!selected?.id) {
      loadedPeriodIdRef.current = null;
      setSplits([]);
      setLoadingSplits(false);
      return;
    }

    if (loadedPeriodIdRef.current !== selected.id) {
      setLoadingSplits(true);
    }
    void ipc
      .getSplits(selected.id)
      .then((rows) => {
        if (loadRequestRef.current !== requestId) return;
        setSplits(rows);
        setError(null);
        loadedPeriodIdRef.current = selected.id;
      })
      .catch((e) => {
        if (loadRequestRef.current !== requestId) return;
        setError(String(e));
        setSplits([]);
        loadedPeriodIdRef.current = selected.id;
      })
      .finally(() => {
        if (loadRequestRef.current === requestId) setLoadingSplits(false);
      });
    return () => {
      loadRequestRef.current += 1;
    };
  }, [selected]);

  const recalculate = async () => {
    if (!selected?.id) return;
    setError(null);
    setCalculating(true);
    try {
      const rows = await ipc.calculateSplits(selected.id);
      setSplits(rows);
      notifyWorkflowStatusChanged();
    } catch (e) {
      setError(String(e));
    } finally {
      setCalculating(false);
    }
  };

  const saveOverride = async (splitId: number, cents: number) => {
    await ipc.saveSplit({ id: splitId, bill_id: 0, apartment_id: 0, amount_cents: cents });
    if (selected?.id) await loadSplits(selected.id);
    notifyWorkflowStatusChanged();
  };

  const { apartments, bills, matrix } = buildMatrix(splits);
  const splitBasisCounts = splits.reduce(
    (counts, split) => {
      const current = counts.get(split.split_basis) ?? 0;
      counts.set(split.split_basis, current + 1);
      return counts;
    },
    new Map<SplitRow["split_basis"], number>(),
  );
  const billBasisCounts = bills.reduce(
    (counts, [, info]) => {
      const current = counts.get(info.splitBasis) ?? 0;
      counts.set(info.splitBasis, current + 1);
      return counts;
    },
    new Map<SplitRow["split_basis"], number>(),
  );

  const apartmentTotals = new Map<number, number>();
  for (const s of splits) {
    apartmentTotals.set(
      s.apartment_id,
      (apartmentTotals.get(s.apartment_id) ?? 0) + s.split_amount_cents,
    );
  }
  const selectedPeriodId = selected?.id ?? null;
  const splitsLoadedForSelected = loadedPeriodIdRef.current === selectedPeriodId;
  const splitsLoadPending = selectedPeriodId !== null && !splitsLoadedForSelected;
  const showSplitsLoading =
    selectedPeriodId !== null &&
    (loadingSplits || splitsLoadPending) &&
    (splits.length > 0 || snapshot.selectedStatus.splits);
  const showSplitsSettling =
    selectedPeriodId !== null && (loadingSplits || splitsLoadPending) && !showSplitsLoading;
  const showSplitsTable = splitsLoadedForSelected && splits.length > 0;

  return (
    <BillingPageShell
      title="Splits"
      subtitle={null}
      actions={
        <Button onClick={recalculate} disabled={!selected || calculating}>
          {calculating ? (
            <Loader2 className="size-4 animate-spin" />
          ) : (
            <RefreshCw className="size-4" />
          )}
          Recalculate
        </Button>
      }
    >
      {error && (
        <div className="rounded-md border border-danger/30 bg-danger-soft px-4 py-3 text-sm text-danger">
          {error}
        </div>
      )}

      {showSplitsTable && (
        <div className="flex flex-wrap items-center gap-2 rounded-lg border border-border bg-card px-4 py-3 text-sm shadow-card">
          <span className="mr-1 text-[11px] font-bold uppercase tracking-wide text-muted-foreground">
            Split method
          </span>
          <MethodChip
            label="m2"
            detail={`${billBasisCounts.get("m2_percentage") ?? 0} bill${(billBasisCounts.get("m2_percentage") ?? 0) === 1 ? "" : "s"}`}
            tone="neutral"
          />
          <MethodChip
            label="people"
            detail={`${billBasisCounts.get("occupants") ?? 0} bill${(billBasisCounts.get("occupants") ?? 0) === 1 ? "" : "s"}`}
            tone="accent"
          />
          <MethodChip
            label="equal"
            detail={`${billBasisCounts.get("equal_apartments") ?? 0} bill${(billBasisCounts.get("equal_apartments") ?? 0) === 1 ? "" : "s"}`}
            tone="warn"
          />
          <span className="ml-auto text-xs text-muted-foreground">
            {splitBasisCounts.size} active method{splitBasisCounts.size === 1 ? "" : "s"}
          </span>
        </div>
      )}

      {!selected && (
        <p className="text-muted-foreground text-sm">
          Select a billing period to view or calculate splits.
        </p>
      )}

      {selected && (showSplitsLoading || showSplitsSettling || splits.length === 0) && (
        <div className="min-h-[268px] overflow-hidden rounded-lg border border-border bg-card shadow-card">
          {showSplitsLoading ? (
            <div className="flex min-h-[268px] items-center justify-center px-6 py-8 text-center">
              <div className="flex items-center gap-2 text-sm text-muted-foreground">
                <Loader2 className="size-4 animate-spin" />
                Loading splits...
              </div>
            </div>
          ) : (
            <div className="relative min-h-[268px] px-6 py-8 text-center">
              <div className="absolute inset-x-6 top-1/2 -translate-y-1/2">
                <div className="mx-auto max-w-md space-y-2">
                  <div className="text-sm font-medium">No splits yet for this billing month</div>
                  <div className="min-h-10 text-sm leading-5 text-muted-foreground">
                    Import bills first, then use the Recalculate button above.
                  </div>
                </div>
              </div>
              <div className="absolute inset-x-6 bottom-8 flex justify-center">
                <Link
                  to="/bills"
                  className="inline-flex h-9 items-center justify-center rounded-md border border-input bg-card px-4 text-sm font-medium shadow-card hover:bg-accent hover:text-accent-foreground"
                >
                  Go to Bills
                </Link>
              </div>
            </div>
          )}
        </div>
      )}

      {showSplitsTable && (
        <div className="overflow-x-auto rounded-lg border border-border bg-card shadow-card">
          <table className="w-full min-w-max text-sm">
            <thead>
              <tr className="bg-surface-2 text-xs font-medium text-muted-foreground">
                <th className="min-w-48 px-3 pt-3.5 pb-2.5 text-left">
                  Bill
                </th>
                <th className="border-r border-border-2 px-3 pt-3.5 pb-2.5 text-right">
                  Total
                </th>
                {apartments.map(([id, apt]) => (
                  <th key={id} className="px-3 pt-3.5 pb-2.5 text-right whitespace-nowrap">
                    <div>{apt.label}</div>
                    <div className="text-[11px] font-normal text-muted-foreground">
                      {apt.unitCode || "No code"}
                    </div>
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {bills.map(([billId, info]) => (
                <tr
                  key={billId}
                  className="border-t border-border odd:bg-card even:bg-surface-2/50 hover:bg-accent/10"
                >
                  <td className="px-3 py-2">
                    <div className="flex items-start gap-2 max-w-56">
                      {info.parseNote && <ReviewIndicator note={info.parseNote} />}
                      <div className="min-w-0">
                        <div className="font-medium truncate max-w-44">
                          {info.provider ?? info.filename}
                        </div>
                        <div className="text-xs text-muted-foreground truncate max-w-44">
                          {splitBasisLabel(info.splitBasis)}
                        </div>
                      </div>
                    </div>
                  </td>
                  <td className="border-r border-border-2 px-3 py-2 text-right font-mono font-medium">
                    {formatEur(info.total)} €
                  </td>
                  {apartments.map(([aptId]) => {
                    const cell = matrix.get(billId)?.get(aptId);
                    return (
                      <td key={aptId} className="px-3 py-2 text-right">
                        {cell ? (
                          <div>
                            <EditableCell split={cell} onSave={saveOverride} />
                            <div className="text-[11px] text-muted-foreground">
                              {splitBasisDetail(cell)}
                            </div>
                          </div>
                        ) : (
                          <span className="text-muted-foreground">-</span>
                        )}
                      </td>
                    );
                  })}
                </tr>
              ))}
            </tbody>
            <tfoot>
              <tr className="border-t border-border bg-surface-2 font-semibold">
                <td className="px-3 py-2">Total per Apartment</td>
                <td className="border-r border-border-2 px-3 py-2 text-right font-mono">
                  {formatEur(
                    splits.reduce((sum, row) => sum + row.split_amount_cents, 0),
                  )} €
                </td>
                {apartments.map(([aptId]) => (
                  <td key={aptId} className="px-3 py-2 text-right font-mono">
                    {formatEur(apartmentTotals.get(aptId) ?? 0)} €
                  </td>
                ))}
              </tr>
            </tfoot>
          </table>
        </div>
      )}

      {showSplitsTable && (
        <div className="flex justify-end">
          <Link
            to="/upn"
            className={buttonVariants()}
          >
            Continue to UPN Preview
          </Link>
        </div>
      )}
    </BillingPageShell>
  );
}

function MethodChip({
  label,
  detail,
  tone,
}: {
  label: string;
  detail: string;
  tone: "neutral" | "accent" | "warn";
}) {
  const toneClass = {
    neutral: "bg-surface-3 text-muted-foreground",
    accent: "bg-accent-soft text-accent-foreground",
    warn: "bg-warning-soft text-warning",
  }[tone];

  return (
    <span className={`inline-flex items-center gap-2 rounded-md px-3 py-1 text-xs ${toneClass}`}>
      <span className="rounded-full bg-card px-2 py-0.5 font-semibold">{label}</span>
      <span>{detail}</span>
    </span>
  );
}
