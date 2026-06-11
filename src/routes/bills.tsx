import { createFileRoute, Link } from "@tanstack/react-router";
import { open } from "@tauri-apps/plugin-dialog";
import { Fragment, useCallback, useEffect, useRef, useState } from "react";
import { AlertTriangle, Check, CheckCircle2, FilePlus, Loader2, Mail, Pencil, Plus, Trash2, X } from "lucide-react";
import { notifyWorkflowStatusChanged } from "@/lib/workflow-status";
import { ipc } from "@/lib/ipc";
import { useBillingPeriodSelection } from "@/lib/billing-period-selection";
import { useWorkflowSnapshotContext } from "@/lib/workflow-snapshot";
import type { Bill, InboxConfig, InboxImportResult, InboxPreviewCandidate, InboxPreviewSession } from "@/lib/types";
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

function previewStatusLabel(status: InboxPreviewCandidate["status"]): string {
  switch (status) {
    case "ready":
      return "Ready";
    case "skipped_duplicate":
      return "Duplicate attachment";
    case "skipped_duplicate_bill":
      return "Duplicate bill";
    case "skipped_wrong_period":
      return "Wrong period";
    case "skipped_unknown_period":
      return "Unknown period";
    case "skipped_unknown_provider":
      return "Unknown provider";
    case "skipped_already_present":
      return "Already present";
    case "skipped_not_expected":
      return "Not expected";
    case "empty":
      return "No bill";
    case "failed":
      return "Failed";
    default:
      return status;
  }
}

function InboxImportDrawer({
  open,
  billingPeriodId,
  onClose,
  onImported,
}: {
  open: boolean;
  billingPeriodId: number | null;
  onClose: () => void;
  onImported: (results: InboxImportResult[]) => Promise<void>;
}) {
  const [config, setConfig] = useState<InboxConfig | null>(null);
  const [daysToScan, setDaysToScan] = useState(45);
  const [preview, setPreview] = useState<InboxPreviewSession | null>(null);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [results, setResults] = useState<InboxImportResult[]>([]);
  const [loadingConfig, setLoadingConfig] = useState(false);
  const [loadingPreview, setLoadingPreview] = useState(false);
  const [importing, setImporting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const busy = loadingPreview || importing;

  useEffect(() => {
    if (!open) return;
    let cancelled = false;
    setError(null);
    setPreview(null);
    setResults([]);
    setSelectedIds(new Set());
    setLoadingConfig(true);
    void ipc
      .getInboxConfig()
      .then((loaded) => {
        if (cancelled) return;
        setConfig(loaded);
        setDaysToScan(loaded.days_to_scan);
      })
      .catch((e) => {
        if (!cancelled) setError(`Failed to load inbox settings: ${e}`);
      })
      .finally(() => {
        if (!cancelled) setLoadingConfig(false);
      });
    return () => {
      cancelled = true;
    };
  }, [open]);

  const closeDrawer = useCallback(async () => {
    if (busy) return;
    const sessionId = preview?.session_id;
    if (sessionId) {
      try {
        await ipc.clearInboxPreviewSession(sessionId);
      } catch {
        // Cleanup is best-effort; sessions also expire in Rust state.
      }
    }
    setPreview(null);
    setSelectedIds(new Set());
    onClose();
  }, [busy, onClose, preview?.session_id]);

  useEffect(() => {
    if (!open) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !busy) void closeDrawer();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [open, busy, closeDrawer]);

  const fetchPreview = async () => {
    if (!billingPeriodId) return;
    setError(null);
    setResults([]);
    setLoadingPreview(true);
    try {
      if (preview?.session_id) {
        await ipc.clearInboxPreviewSession(preview.session_id);
      }
      const clampedDays = Math.min(90, Math.max(1, Math.round(daysToScan || 1)));
      setDaysToScan(clampedDays);
      const nextPreview = await ipc.previewInboxAttachments(billingPeriodId, clampedDays);
      setPreview(nextPreview);
      setSelectedIds(new Set(nextPreview.candidates.filter((candidate) => candidate.selectable).map((candidate) => candidate.id)));
    } catch (e) {
      setError(`Failed to preview inbox: ${e}`);
      setPreview(null);
      setSelectedIds(new Set());
    } finally {
      setLoadingPreview(false);
    }
  };

  const importSelected = async () => {
    if (!preview || selectedIds.size === 0) return;
    setError(null);
    setImporting(true);
    try {
      const imported = await ipc.importInboxPreviewSelection(preview.session_id, Array.from(selectedIds));
      setResults(imported);
      setSelectedIds(new Set());
      setPreview({
        ...preview,
        candidates: preview.candidates.filter((candidate) => !selectedIds.has(candidate.id)),
      });
      await onImported(imported);
    } catch (e) {
      setError(`Failed to import selected inbox items: ${e}`);
    } finally {
      setImporting(false);
    }
  };

  if (!open) return null;

  const readyCount = preview?.candidates.filter((candidate) => candidate.selectable).length ?? 0;
  const skippedCount = preview?.candidates.filter((candidate) => candidate.status.startsWith("skipped_") || candidate.status === "empty").length ?? 0;
  const failedCount = preview?.candidates.filter((candidate) => candidate.status === "failed").length ?? 0;
  const importedCount = results.filter((result) => result.status === "imported").length;
  const resultSkippedCount = results.filter((result) => result.status.startsWith("skipped_")).length;
  const resultFailedCount = results.filter((result) => result.status === "failed").length;

  return (
    <div className="fixed inset-0 z-50">
      <button
        type="button"
        className="absolute inset-0 bg-overlay"
        aria-label="Close inbox import"
        disabled={busy}
        onClick={() => void closeDrawer()}
      />
      <aside className="absolute right-0 top-0 flex h-full w-full max-w-4xl flex-col border-l border-border bg-card shadow-pop sm:w-[760px]">
        <header className="flex items-start justify-between gap-4 border-b border-border px-5 py-4">
          <div>
            <h2 className="text-lg font-semibold">Import from Inbox</h2>
            <p className="mt-1 text-sm text-muted-foreground">
              {config ? `${config.username || "Inbox account"} / ${config.folder || "INBOX"}` : "Loading inbox settings"}
            </p>
          </div>
          <Button variant="ghost" size="icon" onClick={() => void closeDrawer()} disabled={busy} aria-label="Close">
            <X className="size-4" />
          </Button>
        </header>

        <div className="flex-1 space-y-4 overflow-y-auto px-5 py-4">
          <div className="grid gap-3 rounded-lg border border-border bg-surface-2 p-3 sm:grid-cols-[1fr_auto]">
            <div className="grid gap-3 sm:grid-cols-3">
              <div>
                <label htmlFor="inbox-run-days" className="mb-1 block text-xs font-semibold text-muted-foreground">
                  Scan window
                </label>
                <Input
                  id="inbox-run-days"
                  type="number"
                  min={1}
                  max={90}
                  value={daysToScan}
                  onChange={(event) => setDaysToScan(Number(event.target.value))}
                  disabled={loadingConfig || loadingPreview || importing}
                />
              </div>
              <div className="min-w-0">
                <div className="mb-1 text-xs font-semibold text-muted-foreground">Sender allowlist</div>
                <div className="truncate rounded-md border border-border bg-card px-3 py-2 text-sm">
                  {config?.sender_allowlist.trim() || "Any sender"}
                </div>
              </div>
              <div className="min-w-0">
                <div className="mb-1 text-xs font-semibold text-muted-foreground">Password</div>
                <div className="rounded-md border border-border bg-card px-3 py-2 text-sm">
                  {config?.password_configured ? "Configured" : "Missing"}
                </div>
              </div>
            </div>
            <div className="flex items-end">
              <Button onClick={fetchPreview} disabled={!billingPeriodId || loadingConfig || loadingPreview || importing}>
                {loadingPreview ? <Loader2 className="size-4 animate-spin" /> : <Mail className="size-4" />}
                Fetch preview
              </Button>
            </div>
          </div>

          {error && (
            <div className="rounded-md border border-danger/30 bg-danger-soft px-4 py-3 text-sm text-danger">
              {error}
            </div>
          )}

          {preview && (
            <div className="space-y-3">
              <div className="flex flex-wrap items-center gap-2 text-sm">
                <span className="text-xs font-bold uppercase tracking-wide text-muted-foreground">Preview</span>
                <span className="rounded-md bg-success-soft px-3 py-1 text-xs font-semibold text-success">{readyCount} ready</span>
                <span className="rounded-md bg-surface-3 px-3 py-1 text-xs font-semibold text-muted-foreground">{skippedCount} skipped</span>
                {failedCount > 0 && (
                  <span className="rounded-md bg-danger-soft px-3 py-1 text-xs font-semibold text-danger">{failedCount} failed</span>
                )}
              </div>
              <div className="overflow-hidden rounded-lg border border-border">
                <table className="w-full text-sm">
                  <thead className="bg-surface-2 text-left text-xs text-muted-foreground">
                    <tr>
                      <th className="w-10 px-3 py-2"></th>
                      <th className="px-3 py-2">Attachment</th>
                      <th className="px-3 py-2">Parsed bills</th>
                      <th className="px-3 py-2">Status</th>
                    </tr>
                  </thead>
                  <tbody>
                    {preview.candidates.length === 0 ? (
                      <tr>
                        <td colSpan={4} className="px-4 py-6 text-center text-sm text-muted-foreground">
                          No supported bill attachments found in this scan window.
                        </td>
                      </tr>
                    ) : (
                      preview.candidates.map((candidate) => (
                        <tr key={candidate.id} className="border-t border-border align-top">
                          <td className="px-3 py-3">
                            <input
                              type="checkbox"
                              className="size-4 accent-primary"
                              checked={selectedIds.has(candidate.id)}
                              disabled={!candidate.selectable || importing}
                              onChange={(event) => {
                                const next = new Set(selectedIds);
                                if (event.target.checked) next.add(candidate.id);
                                else next.delete(candidate.id);
                                setSelectedIds(next);
                              }}
                            />
                          </td>
                          <td className="max-w-64 px-3 py-3">
                            <div className="truncate font-semibold">{candidate.attachment_filename}</div>
                            <div className="truncate text-xs text-muted-foreground">{candidate.sender || "Unknown sender"}</div>
                            <div className="truncate text-xs text-muted-foreground">{candidate.subject || "No subject"}</div>
                          </td>
                          <td className="px-3 py-3">
                            {candidate.bills.length > 0 ? (
                              <div className="space-y-2">
                                {candidate.bills.map((bill, index) => (
                                  <div key={`${candidate.id}-bill-${index}`} className="rounded-md bg-surface-2 px-3 py-2">
                                    <div className="flex flex-wrap items-center justify-between gap-2">
                                      <span className="font-semibold">{bill.provider_name ?? (bill.creditor_name || "Unmatched bill")}</span>
                                      <span className="font-mono text-xs">{formatEur(bill.amount_cents)} EUR</span>
                                    </div>
                                    <div className="mt-1 truncate text-xs text-muted-foreground">{bill.reference || "No reference"} / {bill.due_date || "No due date"}</div>
                                    {bill.parse_note && (
                                      <div className="mt-1 text-xs text-warning">{bill.parse_note}</div>
                                    )}
                                  </div>
                                ))}
                              </div>
                            ) : (
                              <span className="text-xs text-muted-foreground">No importable bills</span>
                            )}
                            {candidate.notices.length > 0 && (
                              <div className="mt-2 space-y-1">
                                {candidate.notices.map((notice, index) => (
                                  <div key={`${candidate.id}-notice-${index}`} className="text-xs text-warning">
                                    {notice.message}
                                  </div>
                                ))}
                              </div>
                            )}
                          </td>
                          <td className="px-3 py-3">
                            <span className={`inline-flex rounded-md px-2 py-1 text-xs font-semibold ${candidate.status === "ready" ? "bg-success-soft text-success" : candidate.status === "failed" ? "bg-danger-soft text-danger" : "bg-surface-3 text-muted-foreground"}`}>
                              {previewStatusLabel(candidate.status)}
                            </span>
                            {(candidate.skipped_reason || candidate.error) && (
                              <div className="mt-2 max-w-56 text-xs text-muted-foreground">
                                {candidate.error ?? candidate.skipped_reason}
                              </div>
                            )}
                          </td>
                        </tr>
                      ))
                    )}
                  </tbody>
                </table>
              </div>
            </div>
          )}

          {results.length > 0 && (
            <div className="rounded-lg border border-border bg-surface-2 px-4 py-3 text-sm">
              <div className="flex flex-wrap items-center gap-2">
                <span className="text-xs font-bold uppercase tracking-wide text-muted-foreground">Imported</span>
                <span className="rounded-md bg-success-soft px-3 py-1 text-xs font-semibold text-success">{importedCount} imported</span>
                <span className="rounded-md bg-card px-3 py-1 text-xs font-semibold text-muted-foreground">{resultSkippedCount} skipped</span>
                {resultFailedCount > 0 && (
                  <span className="rounded-md bg-danger-soft px-3 py-1 text-xs font-semibold text-danger">{resultFailedCount} failed</span>
                )}
              </div>
            </div>
          )}
        </div>

        <footer className="flex items-center justify-between gap-3 border-t border-border px-5 py-4">
          <div className="text-sm text-muted-foreground">
            {selectedIds.size} selected
          </div>
          <div className="flex gap-2">
            <Button variant="outline" onClick={() => void closeDrawer()} disabled={busy}>
              Close
            </Button>
            <Button onClick={importSelected} disabled={!preview || selectedIds.size === 0 || importing || loadingPreview}>
              {importing ? <Loader2 className="size-4 animate-spin" /> : <Check className="size-4" />}
              Import selected
            </Button>
          </div>
        </footer>
      </aside>
    </div>
  );
}

function BillsPage() {
  const { selected } = useBillingPeriodSelection();
  const snapshot = useWorkflowSnapshotContext();
  const [bills, setBills] = useState<Bill[]>(() => snapshot.bills);
  const [loadingBills, setLoadingBills] = useState(() => snapshot.bills.length === 0);
  const [importing, setImporting] = useState(false);
  const [inboxDrawerOpen, setInboxDrawerOpen] = useState(false);
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

  const handleInboxImported = async (results: InboxImportResult[]) => {
    setInboxResults(results);
    if (selected?.id) {
      await loadBills(selected.id);
      notifyWorkflowStatusChanged();
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
            <Button variant="outline" onClick={() => setInboxDrawerOpen(true)}>
              <Mail className="size-4 mr-2" />
              Import from Inbox
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
      <InboxImportDrawer
        open={inboxDrawerOpen}
        billingPeriodId={selected?.id ?? null}
        onClose={() => setInboxDrawerOpen(false)}
        onImported={handleInboxImported}
      />

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
