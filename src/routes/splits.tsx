import { createFileRoute, Link } from "@tanstack/react-router";
import { Fragment, useState } from "react";
import { Loader2, RefreshCw, Check, Pencil, X } from "lucide-react";
import { ipc } from "@/lib/ipc";
import { useBillingPeriodSelection } from "@/lib/billing-period-selection";
import { useWorkflowSnapshotContext } from "@/lib/workflow-snapshot";
import type { SplitRow } from "@/lib/types";
import { formatEur, parseEurInputCents } from "@/lib/types";
import { BillingPageShell } from "@/components/BillingPageShell";
import {
  BillingEmptyState,
  BillingTable,
  BillingTableFooterRow,
  BillingTableFrame,
  BillingTableHeaderCell,
  BillingTableHeaderRow,
  SummaryChip,
  SummaryStrip,
  billingTableCellClass,
  billingTableZebraRowClass,
} from "@/components/BillingTable";
import { Button, buttonVariants } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";

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
          reviewedAt: s.bill_reviewed_at,
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

function SplitAmountCell({ split }: { split: SplitRow }) {
  return (
    <div>
      <div className="text-sm font-medium tabular-nums">
        {formatEur(split.split_amount_cents)} €
      </div>
      <div className="text-[11px] text-muted-foreground">
        {splitBasisDetail(split)}
      </div>
    </div>
  );
}

function SplitBillRow({
  info,
  apartments,
  rowSplits,
  onSaveOverrides,
}: {
  info: ReturnType<typeof buildMatrix>["bills"][number][1];
  apartments: ReturnType<typeof buildMatrix>["apartments"];
  rowSplits: Map<number, SplitRow> | undefined;
  onSaveOverrides: (updates: Array<{ splitId: number; cents: number }>) => Promise<void>;
}) {
  const [editing, setEditing] = useState(false);
  const [saving, setSaving] = useState(false);
  const [draft, setDraft] = useState<Record<number, string>>({});
  const [originalCents, setOriginalCents] = useState<Record<number, number>>({});

  const cells = apartments.map(([aptId, apt]) => ({
    aptId,
    apt,
    split: rowSplits?.get(aptId),
  }));
  const editableCells = cells.filter(
    (cell): cell is typeof cell & { split: SplitRow & { split_id: number } } =>
      Boolean(cell.split?.split_id),
  );

  const resetDraft = () => {
    setDraft(
      Object.fromEntries(
        editableCells.map((cell) => [
          cell.split.split_id,
          formatEur(cell.split.split_amount_cents),
        ]),
      ),
    );
    setOriginalCents(
      Object.fromEntries(
        editableCells.map((cell) => [
          cell.split.split_id,
          cell.split.split_amount_cents,
        ]),
      ),
    );
  };

  const startEditing = () => {
    if (editing) return;
    resetDraft();
    setEditing(true);
  };

  const updates = editableCells
    .map((cell) => {
      const value =
        draft[cell.split.split_id] ?? formatEur(cell.split.split_amount_cents);
      return {
        splitId: cell.split.split_id,
        cents: parseEurInputCents(value),
        originalCents: originalCents[cell.split.split_id] ?? cell.split.split_amount_cents,
      };
    })
    .filter((update) => update.cents !== update.originalCents);
  const dirty = updates.length > 0;

  const save = async () => {
    if (!dirty) return;
    setSaving(true);
    try {
      await onSaveOverrides(updates.map(({ splitId, cents }) => ({ splitId, cents })));
      setEditing(false);
    } finally {
      setSaving(false);
    }
  };

  return (
    <Fragment>
      <tr className={billingTableZebraRowClass}>
        <td className={billingTableCellClass}>
          <div className="flex max-w-56 items-start gap-2">
            <div className="flex min-w-0 items-start gap-2">
              {info.parseNote && !info.reviewedAt?.trim() && (
                <ReviewIndicator note={info.parseNote} />
              )}
              <div className="min-w-0">
                <div className="max-w-44 truncate font-medium">
                  {info.provider ?? info.filename}
                </div>
                <div className="max-w-44 truncate text-xs text-muted-foreground">
                  {splitBasisLabel(info.splitBasis)}
                </div>
              </div>
            </div>
          </div>
        </td>
        {cells.map(({ aptId, split }) => (
          <td key={aptId} className={`${billingTableCellClass} text-right`}>
            {split ? <SplitAmountCell split={split} /> : <span className="text-muted-foreground">-</span>}
          </td>
        ))}
        <td className={`${billingTableCellClass} text-right font-medium tabular-nums`}>
          {formatEur(info.total)} €
        </td>
        <td className={billingTableCellClass}>
          <div className="flex justify-end">
            <button
              onClick={startEditing}
              className={cn(
                "text-muted-foreground hover:text-foreground",
                editing && "text-foreground",
              )}
              aria-label="Edit split amounts"
            >
              <Pencil className="size-3.5" />
            </button>
          </div>
        </td>
      </tr>
      {editing && (
        <tr className="border-b border-border bg-surface-2/80">
          <td colSpan={apartments.length + 3} className="px-3 py-4">
            <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
              {editableCells.map((cell) => (
                <label key={cell.split.split_id} className="space-y-1.5">
                  <span className="block text-[11px] font-bold uppercase tracking-wide text-muted-foreground">
                    {cell.apt.label}
                  </span>
                  <Input
                    className="h-8 text-sm tabular-nums"
                    value={
                      draft[cell.split.split_id] ??
                      formatEur(cell.split.split_amount_cents)
                    }
                    onChange={(e) =>
                      setDraft((current) => ({
                        ...current,
                        [cell.split.split_id]: e.target.value,
                      }))
                    }
                  />
                  <span className="block text-[11px] text-muted-foreground">
                    {splitBasisDetail(cell.split)}
                  </span>
                </label>
              ))}
            </div>
            <div className="mt-4 flex justify-end gap-2">
              <Button
                variant="outline"
                size="sm"
                onClick={() => {
                  resetDraft();
                  setEditing(false);
                }}
                disabled={saving}
              >
                <X className="size-3.5" />
                Discard
              </Button>
              <Button size="sm" onClick={save} disabled={!dirty || saving}>
                {saving ? (
                  <Loader2 className="size-3.5 animate-spin" />
                ) : (
                  <Check className="size-3.5" />
                )}
                Save changes
              </Button>
            </div>
          </td>
        </tr>
      )}
    </Fragment>
  );
}

function SplitsPage() {
  const { selected } = useBillingPeriodSelection();
  const snapshot = useWorkflowSnapshotContext();
  const splits = snapshot.splits;
  const [calculating, setCalculating] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const recalculate = async () => {
    if (!selected?.id) return;
    setError(null);
    setCalculating(true);
    try {
      await ipc.calculateSplits(selected.id);
      await snapshot.refresh({ core: false, periods: false, selected: true, statuses: true });
    } catch (e) {
      setError(String(e));
    } finally {
      setCalculating(false);
    }
  };

  const saveOverrides = async (updates: Array<{ splitId: number; cents: number }>) => {
    for (const update of updates) {
      await ipc.saveSplit({
        id: update.splitId,
        bill_id: 0,
        apartment_id: 0,
        amount_cents: update.cents,
      });
    }
    if (selected?.id) {
      await snapshot.refresh({ core: false, periods: false, selected: true, statuses: true });
    }
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
  const workflowError = error ?? snapshot.error;
  const selectedStatusKnown =
    selectedPeriodId !== null && snapshot.periodStatuses.has(selectedPeriodId);
  const showSplitsLoading =
    selectedPeriodId !== null &&
    snapshot.loading &&
    (splits.length > 0 || snapshot.selectedStatus.splits);
  const showSplitsSettling =
    selectedPeriodId !== null &&
    snapshot.loading &&
    !showSplitsLoading &&
    selectedStatusKnown;
  const showSplitsTable = selectedPeriodId !== null && !snapshot.loading && splits.length > 0;

  return (
    <BillingPageShell
      title="Splits"
      subtitle={null}
      actions={
        <Button onClick={recalculate} disabled={!selected?.id || calculating}>
          {calculating ? (
            <Loader2 className="size-4 animate-spin" />
          ) : (
            <RefreshCw className="size-4" />
          )}
          Recalculate
        </Button>
      }
    >
      {workflowError && (
        <div className="rounded-md border border-danger/30 bg-danger-soft px-4 py-3 text-sm text-danger">
          {workflowError}
        </div>
      )}

      {showSplitsTable && (
        <SummaryStrip>
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
        </SummaryStrip>
      )}

      {!selected && (
        <p className="text-muted-foreground text-sm">
          Select a billing period to view or calculate splits.
        </p>
      )}

      {selected && (showSplitsLoading || showSplitsSettling || splits.length === 0) && (
        <BillingTableFrame minHeight>
          <BillingEmptyState
            loading={showSplitsLoading}
            loadingLabel="Loading splits..."
            title="No splits yet for this billing month"
            detail="Import bills first, then use the Recalculate button above."
            action={
              <Link
                to="/bills"
                className="inline-flex h-9 items-center justify-center rounded-md border border-input bg-card px-4 text-sm font-medium shadow-card hover:bg-accent hover:text-accent-foreground"
              >
                Go to Bills
              </Link>
            }
          />
        </BillingTableFrame>
      )}

      {showSplitsTable && (
        <BillingTableFrame>
          <BillingTable>
            <thead>
              <BillingTableHeaderRow>
                <BillingTableHeaderCell className="min-w-48">
                  Bill
                </BillingTableHeaderCell>
                {apartments.map(([id, apt]) => (
                  <BillingTableHeaderCell key={id} className="text-right whitespace-nowrap">
                    <div>{apt.label}</div>
                  </BillingTableHeaderCell>
                ))}
                <BillingTableHeaderCell className="text-right">
                  Total
                </BillingTableHeaderCell>
                <BillingTableHeaderCell className="w-10" />
              </BillingTableHeaderRow>
            </thead>
            <tbody>
              {bills.map(([billId, info]) => (
                <SplitBillRow
                  key={billId}
                  info={info}
                  apartments={apartments}
                  rowSplits={matrix.get(billId)}
                  onSaveOverrides={saveOverrides}
                />
              ))}
            </tbody>
            <tfoot>
              <BillingTableFooterRow>
                <td className={billingTableCellClass}>Total per Apartment</td>
                {apartments.map(([aptId]) => (
                  <td key={aptId} className={`${billingTableCellClass} text-right font-semibold tabular-nums`}>
                    {formatEur(apartmentTotals.get(aptId) ?? 0)} €
                  </td>
                ))}
                <td className={`${billingTableCellClass} text-right font-semibold tabular-nums`}>
                  {formatEur(
                    splits.reduce((sum, row) => sum + row.split_amount_cents, 0),
                  )} €
                </td>
                <td className={billingTableCellClass} />
              </BillingTableFooterRow>
            </tfoot>
          </BillingTable>
        </BillingTableFrame>
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
    <SummaryChip className={`${toneClass} font-normal`}>
      <span className="rounded-full bg-card px-2 font-semibold leading-4">{label}</span>
      <span>{detail}</span>
    </SummaryChip>
  );
}
