import { createFileRoute, Link } from "@tanstack/react-router";
import { open } from "@tauri-apps/plugin-dialog";
import { Fragment, useEffect, useRef, useState } from "react";
import { AlertTriangle, Check, CheckCircle2, FilePlus, Mail, Pencil, Plus, Trash2, X } from "lucide-react";
import { notifyWorkflowStatusChanged } from "@/lib/workflow-status";
import { ipc } from "@/lib/ipc";
import { useBillingPeriodSelection } from "@/lib/billing-period-selection";
import { useWorkflowSnapshotContext } from "@/lib/workflow-snapshot";
import type { Bill, InboxImportResult } from "@/lib/types";
import { formatEur } from "@/lib/types";
import { BillingPageShell } from "@/components/BillingPageShell";
import { Button, buttonVariants } from "@/components/ui/button";
import { Input } from "@/components/ui/input";

export const Route = createFileRoute("/bills")({
  component: BillsPage,
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
      <Fragment>
        <tr className="bg-accent/30">
          <td />
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
              className="h-7 text-xs font-mono"
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
          <td className="px-3 py-2 text-xs text-muted-foreground">
            {bill.source_filename}
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
            <div className="flex gap-1">
              <button
                onClick={save}
                className="text-success hover:text-success/80"
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
        <tr className="border-b border-border bg-accent/30">
          <td />
          <td colSpan={6} className="px-3 pb-3">
            <label className="mb-1 block text-[11px] font-bold uppercase tracking-wide text-muted-foreground">
              UPN purpose text
            </label>
            <Input
              className="h-8 text-sm"
              value={draft.purpose_text}
              onChange={(e) =>
                setDraft({ ...draft, purpose_text: e.target.value })
              }
            />
          </td>
        </tr>
      </Fragment>
    );
  }

  return (
    <Fragment>
      <tr className={`border-b border-border transition-colors hover:bg-accent/20 ${bill.parse_note ? "bg-warning-soft/70" : ""}`}>
        <td className="px-3 py-2 text-center">
          {bill.parse_note ? (
            <ReviewIndicator note={bill.parse_note} />
          ) : (
            <CheckCircle2 className="mx-auto size-4 text-success" />
          )}
        </td>
        <td className="px-3 py-2 text-sm max-w-60">
          <div>
            <div className="truncate font-semibold">
              {bill.provider_name ?? (bill.creditor_name || bill.source_filename)}
            </div>
            <div className="truncate text-xs text-muted-foreground">
              {bill.provider_name ? bill.creditor_name : bill.source_filename}
            </div>
          </div>
        </td>
        <td className="px-3 py-2 text-xs font-mono">{bill.reference}</td>
        <td className="px-3 py-2 text-sm">{bill.due_date}</td>
        <td className="px-3 py-2">
          {bill.parse_note ? (
            <span className="inline-flex items-center gap-1.5 rounded-full bg-warning-soft px-2 py-1 text-xs font-semibold text-warning">
              <AlertTriangle className="size-3" />
              OCR - verify
            </span>
          ) : (
            <span className="inline-flex items-center gap-1.5 rounded-full bg-surface-3 px-2 py-1 text-xs font-semibold text-muted-foreground">
              Auto-matched
            </span>
          )}
        </td>
        <td className="px-3 py-2 text-right text-sm font-mono font-semibold">
          {formatEur(bill.amount_cents)} EUR
        </td>
        <td className="px-3 py-2">
          <div className="flex justify-end gap-1">
            <button
              onClick={() => setEditing(true)}
              className="text-muted-foreground hover:text-foreground"
            >
              <Pencil className="size-3.5" />
            </button>
            <button
              onClick={() => bill.id && onDelete(bill.id)}
              className="text-muted-foreground hover:text-danger"
            >
              <Trash2 className="size-3.5" />
            </button>
          </div>
        </td>
      </tr>
      {bill.parse_note && (
        <tr className="border-b border-border bg-warning-soft/70">
          <td />
          <td colSpan={6} className="px-3 py-3">
            <div className="flex flex-wrap items-center gap-3">
              <p className="max-w-2xl text-xs leading-relaxed text-muted-foreground">
                <span className="font-semibold text-warning">Verify this import.</span>{" "}
                {bill.parse_note}
              </p>
              <Button
                variant="warning"
                size="sm"
                className="ml-auto"
                onClick={() => setEditing(true)}
              >
                <Check className="size-3.5" />
                Review
              </Button>
            </div>
          </td>
        </tr>
      )}
    </Fragment>
  );
}

function BillsPage() {
  const { selected } = useBillingPeriodSelection();
  const snapshot = useWorkflowSnapshotContext();
  const [bills, setBills] = useState<Bill[]>(() => snapshot.bills);
  const [loadingBills, setLoadingBills] = useState(() => snapshot.bills.length === 0);
  const [importing, setImporting] = useState(false);
  const [inboxImporting, setInboxImporting] = useState(false);
  const [inboxResults, setInboxResults] = useState<InboxImportResult[]>([]);
  const [error, setError] = useState<string | null>(null);
  const loadRequestRef = useRef(0);
  const loadedPeriodIdRef = useRef<number | null>(snapshot.bills.length > 0 ? selected?.id ?? null : null);

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

    if (loadedPeriodIdRef.current !== selected.id || bills.length === 0) {
      setLoadingBills(true);
    }
    void ipc
      .getBills(selected.id)
      .then((bs) => {
        if (loadRequestRef.current !== requestId) return;
        setBills(bs);
        loadedPeriodIdRef.current = selected.id;
      })
      .finally(() => {
        if (loadRequestRef.current === requestId) setLoadingBills(false);
      });
    return () => {
      loadRequestRef.current += 1;
    };
  }, [selected]);

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

  const importInbox = async () => {
    if (!selected?.id) return;
    setError(null);
    setInboxResults([]);
    setInboxImporting(true);
    try {
      const results = await ipc.importInboxAttachments(selected.id);
      setInboxResults(results);
      await loadBills(selected.id);
      notifyWorkflowStatusChanged();
    } catch (e) {
      setError(`Failed to import from inbox: ${e}`);
    } finally {
      setInboxImporting(false);
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
  const reviewBills = bills.filter((bill) => bill.parse_note?.trim());
  const cleanCount = Math.max(0, bills.length - reviewBills.length);
  const inboxImported = inboxResults.filter((result) => result.status === "imported");
  const inboxSkipped = inboxResults.filter((result) => result.status.startsWith("skipped_"));
  const inboxFailed = inboxResults.filter((result) => result.status === "failed");

  return (
    <BillingPageShell
      title="Bills"
      subtitle={null}
      actions={
        selected ? (
          <>
            <Button variant="outline" onClick={addBlankBill}>
              <Plus className="size-4 mr-2" />
              Add Bill
            </Button>
            <Button variant="outline" onClick={importInbox} disabled={inboxImporting}>
              <Mail className="size-4 mr-2" />
              {inboxImporting ? "Checking..." : "Import from Inbox"}
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
      {!selected && (
        <div className="rounded-lg border border-dashed border-border px-4 py-5 text-sm text-muted-foreground">
          No billing period selected. Use the month picker above to add or select a year.
        </div>
      )}

      {error && (
        <div className="rounded-md border border-danger/30 bg-danger-soft px-4 py-3 text-sm text-danger">
          {error}
        </div>
      )}

      {selected && inboxResults.length > 0 && (
        <div className="rounded-lg border border-border bg-card px-4 py-3 text-sm shadow-card">
          <div className="flex flex-wrap items-center gap-2">
            <span className="mr-1 text-[11px] font-bold uppercase tracking-wide text-muted-foreground">
              Inbox import
            </span>
            <span className="inline-flex items-center gap-2 rounded-md bg-success-soft px-3 py-1 text-xs font-semibold text-success">
              <CheckCircle2 className="size-3.5" />
              {inboxImported.length} imported
            </span>
            <span className="inline-flex items-center gap-2 rounded-md bg-surface-3 px-3 py-1 text-xs font-semibold text-muted-foreground">
              {inboxSkipped.length} skipped
            </span>
            {inboxFailed.length > 0 && (
              <span className="inline-flex items-center gap-2 rounded-md bg-danger-soft px-3 py-1 text-xs font-semibold text-danger">
                <AlertTriangle className="size-3.5" />
                {inboxFailed.length} failed
              </span>
            )}
          </div>
          {inboxSkipped.length > 0 && (
            <div className="mt-3 space-y-1 border-t border-border pt-3">
              {inboxSkipped.map((result, index) => (
                <div
                  key={`${result.attachment_filename}-skipped-${index}`}
                  className="text-xs text-muted-foreground"
                >
                  <span className="font-semibold text-foreground">
                    {result.attachment_filename}
                  </span>
                  {": "}
                  {result.skipped_reason ?? "Skipped."}
                </div>
              ))}
            </div>
          )}
          {inboxFailed.length > 0 && (
            <div className="mt-3 space-y-1 border-t border-border pt-3">
              {inboxFailed.map((result, index) => (
                <div
                  key={`${result.attachment_filename}-${index}`}
                  className="text-xs text-muted-foreground"
                >
                  <span className="font-semibold text-foreground">
                    {result.attachment_filename}
                  </span>
                  {": "}
                  {result.error ?? "Import failed."}
                </div>
              ))}
            </div>
          )}
        </div>
      )}

      {selected && bills.length > 0 && (
        <div className="flex flex-wrap items-center gap-2 rounded-lg border border-border bg-card px-4 py-3 text-sm shadow-card">
          <span className="mr-1 text-[11px] font-bold uppercase tracking-wide text-muted-foreground">
            Imported from
          </span>
          {cleanCount > 0 && (
            <span className="inline-flex items-center gap-2 rounded-md bg-success-soft px-3 py-1 text-xs font-semibold text-success">
              <CheckCircle2 className="size-3.5" />
              {cleanCount} file{cleanCount === 1 ? "" : "s"} clean
            </span>
          )}
          {reviewBills.map((bill) => (
            <span
              key={bill.id ?? bill.source_filename}
              className="inline-flex items-center gap-2 rounded-md bg-warning-soft px-3 py-1 text-xs text-warning"
              title={bill.parse_note}
            >
              <AlertTriangle className="size-3.5" />
              <span className="font-semibold text-foreground">{bill.source_filename}</span>
              <span className="text-muted-foreground">needs review</span>
            </span>
          ))}
        </div>
      )}

      {selected && (
        <div className="min-h-[268px] overflow-hidden rounded-lg border border-border bg-card shadow-card">
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
                <tr className="bg-surface-2 text-left text-xs font-medium text-muted-foreground">
                  <th className="w-8 px-3 pt-3.5 pb-2.5"></th>
                  <th className="px-3 pt-3.5 pb-2.5">Provider</th>
                  <th className="px-3 pt-3.5 pb-2.5">Reference</th>
                  <th className="px-3 pt-3.5 pb-2.5">Due Date</th>
                  <th className="px-3 pt-3.5 pb-2.5">Detection</th>
                  <th className="px-3 pt-3.5 pb-2.5 text-right">Amount</th>
                  <th className="px-3 pt-3.5 pb-2.5"></th>
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
                <tr className="bg-surface-2 font-medium">
                  <td />
                  <td
                    className="px-3 py-2 text-xs text-muted-foreground"
                  >
                    Total ({bills.length} bills)
                  </td>
                  <td colSpan={3}></td>
                  <td className="px-3 py-2 text-right font-mono">
                    {formatEur(totalCents)} EUR
                  </td>
                  <td></td>
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
            className={buttonVariants()}
          >
            Continue to Splits
          </Link>
        </div>
      )}
    </BillingPageShell>
  );
}
