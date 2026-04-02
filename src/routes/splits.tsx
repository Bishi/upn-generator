import { createFileRoute, Link } from "@tanstack/react-router";
import { useEffect, useRef, useState } from "react";
import { RefreshCw, Check, X } from "lucide-react";
import { ipc } from "@/lib/ipc";
import { useBillingPeriodSelection } from "@/lib/billing-period-selection";
import type { BillingPeriod, SplitRow } from "@/lib/types";
import { formatEur, MONTHS } from "@/lib/types";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";

export const Route = createFileRoute("/splits")({
  component: SplitsPage,
});

function YearSelector({ years, selectedYear, onSelectYear }: { years: number[]; selectedYear: number; onSelectYear: (y: number) => void }) {
  return (
    <div className="flex gap-1 flex-wrap">
      {years.map((y) => (
        <button
          key={y}
          onClick={() => onSelectYear(y)}
          className={`px-3 py-1.5 rounded-md text-sm font-medium border transition-colors ${
            selectedYear === y ? "bg-primary text-primary-foreground border-primary" : "border-border hover:bg-accent"
          }`}
        >
          {y}
        </button>
      ))}
    </div>
  );
}

function MonthTabs({ periods, selected, onSelect }: { periods: BillingPeriod[]; selected: BillingPeriod | null; onSelect: (p: BillingPeriod) => void }) {
  return (
    <div className="flex gap-1 flex-wrap">
      {periods.map((p) => (
        <button
          key={p.id}
          onClick={() => onSelect(p)}
          className={`px-3 py-1.5 rounded-md text-sm border transition-colors ${
            selected?.id === p.id ? "bg-primary text-primary-foreground border-primary" : "border-border hover:bg-accent"
          }`}
        >
          {MONTHS[p.month - 1]}
        </button>
      ))}
    </div>
  );
}

function buildMatrix(splits: SplitRow[]) {
  const apartments = [...new Map(splits.map((s) => [s.apartment_id, { label: s.apartment_label, unitCode: s.apartment_unit_code }])).entries()]
    .sort((a, b) => a[1].label.localeCompare(b[1].label));
  const bills = [...new Map(splits.map((s) => [s.bill_id, { filename: s.bill_source_filename, provider: s.provider_name, total: s.bill_amount_cents, splitBasis: s.split_basis }])).entries()];

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
          className="text-green-600 hover:text-green-700"
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
          onClick={() => { setValue(String(split.split_amount_cents / 100)); setEditing(false); }}
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
      {formatEur(split.split_amount_cents)}
    </span>
  );
}

function SplitsPage() {
  const [splits, setSplits] = useState<SplitRow[]>([]);
  const [loadingSplits, setLoadingSplits] = useState(false);
  const [calculating, setCalculating] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const loadRequestRef = useRef(0);
  const {
    years,
    yearPeriods,
    selectedYear,
    selected,
    setSelectedYear,
    setSelected,
  } = useBillingPeriodSelection();

  const loadSplits = async (periodId: number) => {
    const rows = await ipc.getSplits(periodId);
    setSplits(rows);
  };

  useEffect(() => {
    const requestId = ++loadRequestRef.current;
    if (!selected?.id) {
      setSplits([]);
      setLoadingSplits(false);
      return;
    }

    setSplits([]);
    setLoadingSplits(true);
    void ipc.getSplits(selected.id).then((rows) => {
      if (loadRequestRef.current !== requestId) return;
      setSplits(rows);
      setLoadingSplits(false);
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
    } catch (e) {
      setError(String(e));
    } finally {
      setCalculating(false);
    }
  };

  const saveOverride = async (splitId: number, cents: number) => {
    await ipc.saveSplit({ id: splitId, bill_id: 0, apartment_id: 0, amount_cents: cents });
    if (selected?.id) await loadSplits(selected.id);
  };

  const { apartments, bills, matrix } = buildMatrix(splits);

  const apartmentTotals = new Map<number, number>();
  for (const s of splits) {
    apartmentTotals.set(s.apartment_id, (apartmentTotals.get(s.apartment_id) ?? 0) + s.split_amount_cents);
  }

  return (
    <div className="flex flex-col gap-6">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-2xl font-bold">Splits</h2>
          <p className="text-sm text-muted-foreground">
            VO-KA communal services split by people; all other seeded providers split by m2 percentage.
          </p>
        </div>
        <Button onClick={recalculate} disabled={!selected || calculating}>
          <RefreshCw className={`size-4 mr-2 ${calculating ? "animate-spin" : ""}`} />
          {calculating ? "Calculating..." : "Recalculate"}
        </Button>
      </div>

      <YearSelector years={years} selectedYear={selectedYear} onSelectYear={setSelectedYear} />
      {yearPeriods.length > 0 && (
        <MonthTabs periods={yearPeriods} selected={selected} onSelect={(p) => { setSelected(p); setSplits([]); }} />
      )}

      {error && (
        <div className="rounded-md bg-destructive/10 border border-destructive/30 px-4 py-3 text-sm text-destructive">
          {error}
        </div>
      )}

      {!selected && (
        <p className="text-muted-foreground text-sm">
          Select a billing period to view or calculate splits.
        </p>
      )}

      {selected && splits.length === 0 && (
        <div className="overflow-auto rounded-lg border border-border min-h-[268px]">
          <div className="flex min-h-[268px] items-center justify-center px-6 py-8 text-center">
            <div className="max-w-md space-y-3">
              {loadingSplits ? (
                <>
                  <div className="text-sm font-medium">Loading splits...</div>
                  <div className="text-sm text-muted-foreground">
                    Preparing this billing period.
                  </div>
                </>
              ) : (
                <>
                  <div className="text-sm font-medium">No splits yet for this period</div>
                  <div className="text-sm text-muted-foreground">
                    Import bills first, then use the Recalculate button above.
                  </div>
                  <div>
                    <Link
                      to="/bills"
                      className="inline-flex h-9 items-center justify-center rounded-md border border-input bg-background px-4 text-sm font-medium shadow-sm hover:bg-accent hover:text-accent-foreground"
                    >
                      Go to Bills
                    </Link>
                  </div>
                </>
              )}
            </div>
          </div>
        </div>
      )}

      {!loadingSplits && splits.length > 0 && (
        <div className="overflow-auto rounded-lg border border-border">
          <table className="w-full text-sm">
            <thead>
              <tr className="bg-muted/50 text-xs font-medium text-muted-foreground">
                <th className="px-3 py-2 text-left sticky left-0 bg-muted/50">Bill</th>
                <th className="px-3 py-2 text-right">Total</th>
                {apartments.map(([id, apt]) => (
                  <th key={id} className="px-3 py-2 text-right whitespace-nowrap">
                    <div>{apt.label}</div>
                    <div className="text-[11px] font-normal text-muted-foreground">{apt.unitCode || "No code"}</div>
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {bills.map(([billId, info]) => (
                <tr key={billId} className="border-t border-border hover:bg-accent/10">
                  <td className="px-3 py-2 sticky left-0 bg-background">
                    <div className="font-medium truncate max-w-44">
                      {info.provider ?? info.filename}
                    </div>
                    <div className="text-xs text-muted-foreground truncate max-w-44">
                      {info.splitBasis === "occupants" ? "Split by people" : "Split by m2 %"}
                    </div>
                    {info.provider && (
                      <div className="text-xs text-muted-foreground truncate max-w-44">
                        {info.filename}
                      </div>
                    )}
                  </td>
                  <td className="px-3 py-2 text-right font-mono font-medium">
                    {formatEur(info.total)} EUR
                  </td>
                  {apartments.map(([aptId]) => {
                    const cell = matrix.get(billId)?.get(aptId);
                    return (
                      <td key={aptId} className="px-3 py-2 text-right">
                        {cell ? (
                          <div>
                            <EditableCell split={cell} onSave={saveOverride} />
                            <div className="text-[11px] text-muted-foreground">
                              {cell.split_basis === "occupants"
                                ? `${cell.occupant_count} people`
                                : `${cell.m2_percentage.toFixed(2)}%`}
                            </div>
                          </div>
                        ) : (
                          <span className="text-muted-foreground">—</span>
                        )}
                      </td>
                    );
                  })}
                </tr>
              ))}
            </tbody>
            <tfoot>
              <tr className="border-t border-border bg-muted/30 font-semibold">
                <td className="px-3 py-2">Total per Apartment</td>
                <td className="px-3 py-2 text-right font-mono">
                  {formatEur(splits.reduce((sum, row) => sum + row.split_amount_cents, 0))}
                </td>
                {apartments.map(([aptId]) => (
                  <td key={aptId} className="px-3 py-2 text-right font-mono">
                    {formatEur(apartmentTotals.get(aptId) ?? 0)} EUR
                  </td>
                ))}
              </tr>
            </tfoot>
          </table>
        </div>
      )}

      {!loadingSplits && splits.length > 0 && (
        <div className="flex justify-end">
          <Link
            to="/upn"
            className="inline-flex h-9 items-center justify-center rounded-md border border-input bg-background px-4 text-sm font-medium shadow-sm hover:bg-accent hover:text-accent-foreground"
          >
            Continue to UPN Preview
          </Link>
        </div>
      )}
    </div>
  );
}
