import { createFileRoute } from "@tanstack/react-router";
import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";
import { FilePlus, Trash2, Pencil, Check, X, Plus } from "lucide-react";
import { ipc } from "@/lib/ipc";
import type { Bill, BillingPeriod } from "@/lib/types";
import { formatEur, MONTHS } from "@/lib/types";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";

export const Route = createFileRoute("/bills")({
  component: BillsPage,
});

// ─── Period selector ───────────────────────────────────────────────────────

function PeriodSelector({
  periods,
  selected,
  onSelect,
  onCreate,
  onDelete,
}: {
  periods: BillingPeriod[];
  selected: BillingPeriod | null;
  onSelect: (p: BillingPeriod) => void;
  onCreate: (month: number, year: number) => void;
  onDelete: (id: number) => void;
}) {
  const now = new Date();
  const [newMonth, setNewMonth] = useState(now.getMonth() + 1);
  const [newYear, setNewYear] = useState(now.getFullYear());
  const [showNew, setShowNew] = useState(false);

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
          {MONTHS[p.month - 1]} {p.year}
        </button>
      ))}

      {showNew ? (
        <div className="flex items-center gap-2 border rounded-md px-3 py-1.5 bg-card">
          <select
            className="bg-transparent text-sm outline-none"
            value={newMonth}
            onChange={(e) => setNewMonth(Number(e.target.value))}
          >
            {MONTHS.map((m, i) => (
              <option key={i} value={i + 1}>{m}</option>
            ))}
          </select>
          <Input
            type="number"
            className="w-20 h-6 text-sm"
            value={newYear}
            onChange={(e) => setNewYear(Number(e.target.value))}
          />
          <button
            className="text-green-600 hover:text-green-700"
            onClick={() => {
              onCreate(newMonth, newYear);
              setShowNew(false);
            }}
          >
            <Check className="size-4" />
          </button>
          <button
            className="text-muted-foreground hover:text-foreground"
            onClick={() => setShowNew(false)}
          >
            <X className="size-4" />
          </button>
        </div>
      ) : (
        <Button size="sm" variant="outline" onClick={() => setShowNew(true)}>
          <Plus className="size-3.5 mr-1" /> New Period
        </Button>
      )}

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
            onChange={(e) => setDraft({ ...draft, creditor_name: e.target.value })}
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
            onChange={(e) => setDraft({ ...draft, purpose_text: e.target.value })}
          />
        </td>
        <td className="px-3 py-2">
          <div className="flex gap-1">
            <button onClick={save} className="text-green-600 hover:text-green-700">
              <Check className="size-4" />
            </button>
            <button onClick={cancel} className="text-muted-foreground hover:text-foreground">
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
  const [periods, setPeriods] = useState<BillingPeriod[]>([]);
  const [selected, setSelected] = useState<BillingPeriod | null>(null);
  const [bills, setBills] = useState<Bill[]>([]);
  const [importing, setImporting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const loadPeriods = async () => {
    const ps = await ipc.getBillingPeriods();
    setPeriods(ps);
    if (!selected && ps.length > 0) setSelected(ps[0]);
  };

  const loadBills = async (periodId: number) => {
    const bs = await ipc.getBills(periodId);
    setBills(bs);
  };

  useEffect(() => {
    loadPeriods();
  }, []);

  useEffect(() => {
    if (selected?.id) loadBills(selected.id);
  }, [selected]);

  const createPeriod = async (month: number, year: number) => {
    const p = await ipc.createBillingPeriod(month, year);
    const updated = await ipc.getBillingPeriods();
    setPeriods(updated);
    setSelected(p);
  };

  const deletePeriod = async (id: number) => {
    if (!confirm("Delete this billing period and all its bills?")) return;
    await ipc.deleteBillingPeriod(id);
    const updated = await ipc.getBillingPeriods();
    setPeriods(updated);
    setSelected(updated[0] ?? null);
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
          await ipc.importBill(path, selected.id!);
        } catch (e) {
          setError(`Failed to import ${path}: ${e}`);
        }
      }
      await loadBills(selected.id!);
    } finally {
      setImporting(false);
    }
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
          <Button onClick={importFiles} disabled={importing}>
            <FilePlus className="size-4 mr-2" />
            {importing ? "Importing…" : "Import PDFs"}
          </Button>
        )}
      </div>

      <PeriodSelector
        periods={periods}
        selected={selected}
        onSelect={(p) => { setSelected(p); setBills([]); }}
        onCreate={createPeriod}
        onDelete={deletePeriod}
      />

      {error && (
        <div className="rounded-md bg-destructive/10 border border-destructive/30 px-4 py-3 text-sm text-destructive">
          {error}
        </div>
      )}

      {!selected && (
        <p className="text-muted-foreground text-sm">
          Create or select a billing period to get started.
        </p>
      )}

      {selected && bills.length === 0 && (
        <p className="text-muted-foreground text-sm">
          No bills yet. Import PDF invoices from your utility providers.
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
                <td className="px-3 py-2 text-xs text-muted-foreground" colSpan={2}>
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
    </div>
  );
}
