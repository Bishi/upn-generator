import { createFileRoute } from "@tanstack/react-router";
import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";
import {
  FilePlus,
  Trash2,
  Pencil,
  Check,
  X,
  Plus,
  CalendarPlus,
} from "lucide-react";
import { ipc } from "@/lib/ipc";
import type { Bill, BillingPeriod } from "@/lib/types";
import { formatEur, MONTHS } from "@/lib/types";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";

export const Route = createFileRoute("/bills")({
  component: BillsPage,
});

// ─── Confirm dialog ─────────────────────────────────────────────────────────

function ConfirmDialog({
  message,
  onConfirm,
  onCancel,
}: {
  message: string;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
      <div className="bg-card border border-border rounded-lg p-6 max-w-sm w-full space-y-4">
        <p className="text-sm">{message}</p>
        <div className="flex gap-2 justify-end">
          <Button variant="outline" size="sm" onClick={onCancel}>
            Cancel
          </Button>
          <Button
            variant="destructive"
            size="sm"
            onClick={onConfirm}
          >
            Delete
          </Button>
        </div>
      </div>
    </div>
  );
}

// ─── Year selector ──────────────────────────────────────────────────────────

function YearSelector({
  years,
  selectedYear,
  onSelectYear,
  onAddYear,
}: {
  years: number[];
  selectedYear: number;
  onSelectYear: (y: number) => void;
  onAddYear: (y: number) => void;
}) {
  const [showAdd, setShowAdd] = useState(false);
  const [newYear, setNewYear] = useState(new Date().getFullYear());

  return (
    <div className="flex items-center gap-2">
      {years.map((y) => (
        <button
          key={y}
          onClick={() => onSelectYear(y)}
          className={`px-3 py-1.5 rounded-md text-sm font-medium border transition-colors ${
            selectedYear === y
              ? "bg-primary text-primary-foreground border-primary"
              : "border-border hover:bg-accent"
          }`}
        >
          {y}
        </button>
      ))}
      {showAdd ? (
        <div className="flex items-center gap-2 border rounded-md px-3 py-1.5 bg-card">
          <Input
            type="number"
            className="w-20 h-6 text-sm"
            value={newYear}
            onChange={(e) => setNewYear(Number(e.target.value))}
          />
          <button
            className="text-green-600 hover:text-green-700"
            onClick={() => {
              onAddYear(newYear);
              setShowAdd(false);
            }}
          >
            <Check className="size-4" />
          </button>
          <button
            className="text-muted-foreground hover:text-foreground"
            onClick={() => setShowAdd(false)}
          >
            <X className="size-4" />
          </button>
        </div>
      ) : (
        <Button
          size="sm"
          variant="outline"
          onClick={() => setShowAdd(true)}
          className="gap-1"
        >
          <CalendarPlus className="size-3.5" /> Add Year
        </Button>
      )}
    </div>
  );
}

// ─── Month tabs ─────────────────────────────────────────────────────────────

function MonthTabs({
  periods,
  selected,
  onSelect,
  onDelete,
}: {
  periods: BillingPeriod[];
  selected: BillingPeriod | null;
  onSelect: (p: BillingPeriod) => void;
  onDelete: (id: number) => void;
}) {
  return (
    <div className="flex items-center gap-2 flex-wrap">
      {periods.map((p) => (
        <button
          key={p.id}
          onClick={() => onSelect(p)}
          className={`px-3 py-1.5 rounded-md text-sm border transition-colors ${
            selected?.id === p.id
              ? "bg-primary text-primary-foreground border-primary"
              : "border-border hover:bg-accent"
          }`}
        >
          {MONTHS[p.month - 1]}
        </button>
      ))}

      {selected && selected.id && (
        <Button
          size="sm"
          variant="ghost"
          className="text-destructive hover:text-destructive ml-auto"
          onClick={() => onDelete(selected.id!)}
        >
          <Trash2 className="size-3.5 mr-1" /> Delete Period
        </Button>
      )}
    </div>
  );
}

// ─── Bill edit row ─────────────────────────────────────────────────────────

function BillRow({
  bill,
  onSave,
  onDelete,
}: {
  bill: Bill;
  onSave: (b: Bill) => void;
  onDelete: (id: number) => void;
}) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState<Bill>(bill);

  const save = () => {
    onSave(draft);
    setEditing(false);
  };

  const cancel = () => {
    setDraft(bill);
    setEditing(false);
  };

  if (editing) {
    return (
      <tr className="bg-accent/30">
        <td className="px-3 py-2 text-xs text-muted-foreground">
          {bill.source_filename}
        </td>
        <td className="px-3 py-2">
          <Input
            className="h-7 text-sm"
            value={draft.creditor_name}
            onChange={(e) =>
              setDraft({ ...draft, creditor_name: e.target.value })
            }
          />
        </td>
        <td className="px-3 py-2">
          <Input
            className="h-7 text-sm"
            value={
              draft.amount_cents === 0 ? "" : String(draft.amount_cents / 100)
            }
            placeholder="123.45"
            onChange={(e) => {
              const val = parseFloat(e.target.value) || 0;
              setDraft({ ...draft, amount_cents: Math.round(val * 100) });
            }}
          />
        </td>
        <td className="px-3 py-2">
          <Input
            className="h-7 text-sm"
            value={draft.reference}
            onChange={(e) => setDraft({ ...draft, reference: e.target.value })}
          />
        </td>
        <td className="px-3 py-2">
          <Input
            className="h-7 text-sm"
            value={draft.due_date}
            onChange={(e) => setDraft({ ...draft, due_date: e.target.value })}
          />
        </td>
        <td className="px-3 py-2">
          <Input
            className="h-7 text-sm"
            value={draft.purpose_text}
            onChange={(e) =>
              setDraft({ ...draft, purpose_text: e.target.value })
            }
          />
        </td>
        <td className="px-3 py-2">
          <div className="flex gap-1">
            <button
              onClick={save}
              className="text-green-600 hover:text-green-700"
            >
              <Check className="size-4" />
            </button>
            <button
              onClick={cancel}
              className="text-muted-foreground hover:text-foreground"
            >
              <X className="size-4" />
            </button>
          </div>
        </td>
      </tr>
    );
  }

  return (
    <tr className="border-b border-border hover:bg-accent/20 transition-colors">
      <td className="px-3 py-2 text-xs text-muted-foreground max-w-32 truncate">
        {bill.source_filename}
        {bill.provider_name && (
          <span className="ml-1 text-primary">· {bill.provider_name}</span>
        )}
      </td>
      <td className="px-3 py-2 text-sm">{bill.creditor_name}</td>
      <td className="px-3 py-2 text-sm font-mono font-medium">
        {formatEur(bill.amount_cents)} €
      </td>
      <td className="px-3 py-2 text-xs font-mono">{bill.reference}</td>
      <td className="px-3 py-2 text-sm">{bill.due_date}</td>
      <td className="px-3 py-2 text-xs text-muted-foreground max-w-40 truncate">
        {bill.purpose_text}
      </td>
      <td className="px-3 py-2">
        <div className="flex gap-1">
          <button
            onClick={() => setEditing(true)}
            className="text-muted-foreground hover:text-foreground"
          >
            <Pencil className="size-3.5" />
          </button>
          <button
            onClick={() => bill.id && onDelete(bill.id)}
            className="text-muted-foreground hover:text-destructive"
          >
            <Trash2 className="size-3.5" />
          </button>
        </div>
      </td>
    </tr>
  );
}

// ─── Main page ─────────────────────────────────────────────────────────────

function BillsPage() {
  const [allPeriods, setAllPeriods] = useState<BillingPeriod[]>([]);
  const [selectedYear, setSelectedYear] = useState(new Date().getFullYear());
  const [selected, setSelected] = useState<BillingPeriod | null>(null);
  const [bills, setBills] = useState<Bill[]>([]);
  const [importing, setImporting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [confirmDeleteId, setConfirmDeleteId] = useState<number | null>(null);

  // Derived: unique years and periods for selected year
  const years = [...new Set(allPeriods.map((p) => p.year))].sort(
    (a, b) => b - a
  );
  const yearPeriods = allPeriods
    .filter((p) => p.year === selectedYear)
    .sort((a, b) => a.month - b.month);

  const loadPeriods = async () => {
    const ps = await ipc.getBillingPeriods();
    setAllPeriods(ps);
    return ps;
  };

  const loadBills = async (periodId: number) => {
    const bs = await ipc.getBills(periodId);
    setBills(bs);
  };

  useEffect(() => {
    loadPeriods().then((ps) => {
      if (ps.length > 0) {
        // Auto-select the most recent year and its latest period
        const latestYear = ps[0].year;
        setSelectedYear(latestYear);
        setSelected(ps[0]);
      }
    });
  }, []);

  useEffect(() => {
    if (selected?.id) loadBills(selected.id);
    else setBills([]);
  }, [selected]);

  // When year changes, auto-select the latest period in that year
  useEffect(() => {
    const yp = allPeriods
      .filter((p) => p.year === selectedYear)
      .sort((a, b) => b.month - a.month);
    if (yp.length > 0 && (!selected || selected.year !== selectedYear)) {
      setSelected(yp[0]);
    }
  }, [selectedYear, allPeriods]);

  const addYear = async (year: number) => {
    await ipc.createYearPeriods(year);
    const ps = await loadPeriods();
    setSelectedYear(year);
    // Select first period of new year
    const yp = ps
      .filter((p) => p.year === year)
      .sort((a, b) => a.month - b.month);
    if (yp.length > 0) setSelected(yp[0]);
  };

  const deletePeriod = async (id: number) => {
    await ipc.deleteBillingPeriod(id);
    setConfirmDeleteId(null);
    const ps = await loadPeriods();
    // Select another period in the same year, or clear
    const yp = ps
      .filter((p) => p.year === selectedYear)
      .sort((a, b) => a.month - b.month);
    setSelected(yp[0] ?? null);
    setBills([]);
  };

  const importFiles = async () => {
    if (!selected?.id) return;
    setError(null);
    setImporting(true);
    try {
      const paths = await open({
        multiple: true,
        filters: [{ name: "PDF Files", extensions: ["pdf"] }],
      });
      if (!paths) return;
      const pathArr = Array.isArray(paths) ? paths : [paths];
      for (const path of pathArr) {
        try {
          await ipc.importBills(path, selected.id!);
        } catch (e) {
          setError(`Failed to import ${path}: ${e}`);
        }
      }
      await loadBills(selected.id!);
    } finally {
      setImporting(false);
    }
  };

  const addBlankBill = async () => {
    if (!selected?.id) return;
    const blank: Bill = {
      id: null,
      billing_period_id: selected.id,
      provider_id: null,
      raw_text: "",
      amount_cents: 0,
      creditor_name: "",
      creditor_iban: "",
      creditor_address: "",
      creditor_city: "",
      creditor_postal_code: "",
      reference: "",
      due_date: "",
      purpose_code: "OTHR",
      purpose_text: "",
      invoice_number: "",
      status: "draft",
      source_filename: "(manual)",
      provider_name: null,
    };
    await ipc.saveBill(blank);
    await loadBills(selected.id);
  };

  const saveBill = async (bill: Bill) => {
    await ipc.saveBill(bill);
    if (selected?.id) await loadBills(selected.id);
  };

  const deleteBill = async (id: number) => {
    await ipc.deleteBill(id);
    if (selected?.id) await loadBills(selected.id);
  };

  const totalCents = bills.reduce((s, b) => s + b.amount_cents, 0);

  return (
    <div className="flex flex-col gap-6">
      <div className="flex items-center justify-between">
        <h2 className="text-2xl font-bold">Bills</h2>
        {selected && (
          <div className="flex gap-2">
            <Button variant="outline" onClick={addBlankBill}>
              <Plus className="size-4 mr-2" />
              Add Bill
            </Button>
            <Button onClick={importFiles} disabled={importing}>
              <FilePlus className="size-4 mr-2" />
              {importing ? "Importing…" : "Import PDFs"}
            </Button>
          </div>
        )}
      </div>

      {/* Year selector */}
      <YearSelector
        years={years}
        selectedYear={selectedYear}
        onSelectYear={setSelectedYear}
        onAddYear={addYear}
      />

      {/* Month tabs for selected year */}
      {yearPeriods.length > 0 && (
        <MonthTabs
          periods={yearPeriods}
          selected={selected}
          onSelect={(p) => {
            setSelected(p);
            setBills([]);
          }}
          onDelete={(id) => setConfirmDeleteId(id)}
        />
      )}

      {years.length === 0 && (
        <p className="text-muted-foreground text-sm">
          No billing periods yet. Add a year to get started.
        </p>
      )}

      {error && (
        <div className="rounded-md bg-destructive/10 border border-destructive/30 px-4 py-3 text-sm text-destructive">
          {error}
        </div>
      )}

      {selected && bills.length === 0 && (
        <p className="text-muted-foreground text-sm">
          No bills yet. Import PDF invoices or add a bill manually.
        </p>
      )}

      {bills.length > 0 && (
        <div className="rounded-lg border border-border overflow-hidden">
          <table className="w-full text-sm">
            <thead>
              <tr className="bg-muted/50 text-left text-xs font-medium text-muted-foreground">
                <th className="px-3 py-2">File / Provider</th>
                <th className="px-3 py-2">Creditor</th>
                <th className="px-3 py-2">Amount</th>
                <th className="px-3 py-2">Reference</th>
                <th className="px-3 py-2">Due Date</th>
                <th className="px-3 py-2">Purpose</th>
                <th className="px-3 py-2"></th>
              </tr>
            </thead>
            <tbody>
              {bills.map((b) => (
                <BillRow
                  key={b.id}
                  bill={b}
                  onSave={saveBill}
                  onDelete={deleteBill}
                />
              ))}
            </tbody>
            <tfoot>
              <tr className="bg-muted/30 font-medium">
                <td
                  className="px-3 py-2 text-xs text-muted-foreground"
                  colSpan={2}
                >
                  Total ({bills.length} bills)
                </td>
                <td className="px-3 py-2 font-mono">
                  {formatEur(totalCents)} €
                </td>
                <td colSpan={4}></td>
              </tr>
            </tfoot>
          </table>
        </div>
      )}

      {/* Delete confirmation modal */}
      {confirmDeleteId !== null && (
        <ConfirmDialog
          message="Delete this billing period and all its bills? This cannot be undone."
          onConfirm={() => deletePeriod(confirmDeleteId)}
          onCancel={() => setConfirmDeleteId(null)}
        />
      )}
    </div>
  );
}
