import { createFileRoute, Link } from "@tanstack/react-router";
import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useRef, useState } from "react";
import { FilePlus, Pencil, Check, X, Plus, Trash2 } from "lucide-react";
import { notifyWorkflowStatusChanged } from "@/lib/workflow-status";
import { ipc } from "@/lib/ipc";
import { useBillingPeriodSelection } from "@/lib/billing-period-selection";
import type { Bill } from "@/lib/types";
import { formatEur } from "@/lib/types";
import { BillingPageShell } from "@/components/BillingPageShell";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";

export const Route = createFileRoute("/bills")({
  component: BillsPage,
});

function ReviewIndicator({ note }: { note: string }) {
  return (
    <span
      className="mt-0.5 inline-flex size-2.5 shrink-0 rounded-full bg-amber-400 ring-1 ring-amber-500/60 cursor-help"
      title={note}
      aria-label={note}
    />
  );
}

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
    <tr className="border-b border-border transition-colors hover:bg-accent/20">
      <td className="px-3 py-2 text-xs text-muted-foreground max-w-56">
        <div className="flex items-start gap-2">
          {bill.parse_note && <ReviewIndicator note={bill.parse_note} />}
          <div>
            <span>{bill.source_filename}</span>
            {bill.provider_name && (
              <span className="ml-1 text-primary"> - {bill.provider_name}</span>
            )}
          </div>
        </div>
      </td>
      <td className="px-3 py-2 text-sm">{bill.creditor_name}</td>
      <td className="px-3 py-2 text-sm font-mono font-medium">
        {formatEur(bill.amount_cents)} EUR
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

function BillsPage() {
  const [bills, setBills] = useState<Bill[]>([]);
  const [loadingBills, setLoadingBills] = useState(false);
  const [importing, setImporting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const loadRequestRef = useRef(0);
  const {
    years,
    yearPeriods,
    selectedYear,
    selected,
    loadPeriods,
    setSelectedYear,
    setSelected,
  } = useBillingPeriodSelection();

  const loadBills = async (periodId: number) => {
    const bs = await ipc.getBills(periodId);
    setBills(bs);
  };

  useEffect(() => {
    const requestId = ++loadRequestRef.current;
    if (!selected?.id) {
      setBills([]);
      setLoadingBills(false);
      return;
    }

    setBills([]);
    setLoadingBills(true);
    void ipc.getBills(selected.id).then((bs) => {
      if (loadRequestRef.current !== requestId) return;
      setBills(bs);
      setLoadingBills(false);
    });
    return () => {
      loadRequestRef.current += 1;
    };
  }, [selected]);

  const addYear = async (year: number) => {
    await ipc.createYearPeriods(year);
    const periods = await loadPeriods();
    const preferredMonth = selected?.month ?? 1;
    const sameMonth =
      periods.find(
        (period) => period.year === year && period.month === preferredMonth,
      ) ??
      periods.find((period) => period.year === year && period.month === 1) ??
      null;
    if (sameMonth) {
      setSelected(sameMonth);
    }
    notifyWorkflowStatusChanged();
  };

  const importFiles = async () => {
    if (!selected?.id) return;
    setError(null);
    setImporting(true);
    try {
      const paths = await open({
        multiple: true,
        filters: [
          {
            name: "Bills (PDF or image)",
            extensions: ["pdf", "jpg", "jpeg", "png", "bmp", "tif", "tiff"],
          },
        ],
      });
      if (!paths) return;
      const pathArr = Array.isArray(paths) ? paths : [paths];
      for (const path of pathArr) {
        try {
          await ipc.importBills(path, selected.id);
        } catch (e) {
          setError(`Failed to import ${path}: ${e}`);
        }
      }
      await loadBills(selected.id);
      notifyWorkflowStatusChanged();
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
      parse_note: "",
      status: "draft",
      source_filename: "(manual)",
      provider_name: null,
    };
    await ipc.saveBill(blank);
    await loadBills(selected.id);
    notifyWorkflowStatusChanged();
  };

  const saveBill = async (bill: Bill) => {
    await ipc.saveBill(bill);
    if (selected?.id) await loadBills(selected.id);
    notifyWorkflowStatusChanged();
  };

  const deleteBill = async (id: number) => {
    await ipc.deleteBill(id);
    if (selected?.id) await loadBills(selected.id);
    notifyWorkflowStatusChanged();
  };

  const totalCents = bills.reduce((s, b) => s + b.amount_cents, 0);

  return (
    <BillingPageShell
      title="Bills"
      subtitle={null}
      years={years}
      selectedYear={selectedYear}
      onSelectYear={setSelectedYear}
      yearPeriods={yearPeriods}
      selected={selected}
      onSelectPeriod={(period) => {
        setSelected(period);
        setBills([]);
      }}
      onAddYear={addYear}
      actions={
        selected ? (
          <>
            <Button variant="outline" onClick={addBlankBill}>
              <Plus className="size-4 mr-2" />
              Add Bill
            </Button>
            <Button onClick={importFiles} disabled={importing}>
              <FilePlus className="size-4 mr-2" />
              {importing ? "Importing..." : "Import Bills"}
            </Button>
          </>
        ) : (
          <Button disabled variant="outline">
            Select Period
          </Button>
        )
      }
    >
      {years.length === 0 && (
        <div className="rounded-lg border border-dashed border-border px-4 py-5 text-sm text-muted-foreground">
          No billing periods yet. Add a year to get started.
        </div>
      )}

      {error && (
        <div className="rounded-md bg-destructive/10 border border-destructive/30 px-4 py-3 text-sm text-destructive">
          {error}
        </div>
      )}

      {selected && (
        <div className="rounded-lg border border-border overflow-hidden min-h-[268px]">
          {loadingBills ? (
            <div className="flex min-h-[268px] items-center justify-center px-6 py-8 text-center">
              <div className="max-w-md space-y-2">
                <div className="text-sm font-medium">Loading bills...</div>
                <div className="text-sm text-muted-foreground">
                  Preparing this billing period.
                </div>
              </div>
            </div>
          ) : bills.length === 0 ? (
            <div className="flex min-h-[268px] items-center justify-center px-6 py-8 text-center">
              <div className="max-w-md space-y-2">
                <div className="text-sm font-medium">No bills yet for this period</div>
                <div className="text-sm text-muted-foreground">
                  Use the buttons above to import PDF or image invoices, or add a bill manually.
                </div>
              </div>
            </div>
          ) : (
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
                    {formatEur(totalCents)} EUR
                  </td>
                  <td colSpan={4}></td>
                </tr>
              </tfoot>
            </table>
          )}
        </div>
      )}

      {selected && !loadingBills && bills.length > 0 && (
        <div className="flex justify-end">
          <Link
            to="/splits"
            className="inline-flex h-9 items-center justify-center rounded-md border border-input bg-background px-4 text-sm font-medium shadow-sm hover:bg-accent hover:text-accent-foreground"
          >
            Continue to Splits
          </Link>
        </div>
      )}
    </BillingPageShell>
  );
}
