import { createFileRoute, Link } from "@tanstack/react-router";
import { open } from "@tauri-apps/plugin-dialog";
import { Fragment, useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { AlertTriangle, Calendar, Check, CheckCircle2, ChevronDown, Clock, FilePlus, Inbox, Loader2, Mail, Minus, Pencil, Plus, RefreshCw, Settings, Trash2, X } from "lucide-react";
import { ipc } from "@/lib/ipc";
import { useBillingPeriodSelection } from "@/lib/billing-period-selection";
import { useWorkflowSnapshotContext } from "@/lib/workflow-snapshot";
import type { Bill, InboxConfig, InboxImportResult, InboxPreviewCandidate, InboxPreviewSession } from "@/lib/types";
import { formatEur } from "@/lib/types";
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
  billingTableBodyRowClass,
  billingTableCellClass,
  billingTableNumericCellClass,
} from "@/components/BillingTable";
import { Button, buttonVariants } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";

export const Route = createFileRoute("/bills")({
  component: BillsPage,
});

function ReviewIndicator({ note }: { note: string }) {
  return (
    <span
      className="inline-flex size-2.5 shrink-0 cursor-help rounded-full bg-warning ring-1 ring-warning/60"
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
      <tr
        className={cn(
          billingTableBodyRowClass,
          "hover:bg-accent/20",
          bill.parse_note && "bg-warning-soft/70",
        )}
      >
        <td className="w-[68px] p-0 align-middle">
          <div className="grid min-h-16 place-items-center">
            {bill.parse_note ? (
              <ReviewIndicator note={bill.parse_note} />
            ) : (
              <CheckCircle2 className="block size-4 text-success" />
            )}
          </div>
        </td>
        <td className="py-3 pr-3 align-middle text-sm max-w-60">
          <div>
            <div className="truncate font-semibold">
              {bill.provider_name ?? (bill.creditor_name || bill.source_filename)}
            </div>
            <div className="truncate text-xs text-muted-foreground">
              {bill.provider_name ? bill.creditor_name : bill.source_filename}
            </div>
          </div>
        </td>
        <td className={`${billingTableCellClass} text-xs font-mono`}>{bill.reference}</td>
        <td className={`${billingTableCellClass} text-sm`}>{bill.due_date}</td>
        <td className={billingTableCellClass}>
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
        <td className={`${billingTableNumericCellClass} text-sm font-semibold`}>
          {formatEur(bill.amount_cents)} €
        </td>
        <td className={billingTableCellClass}>
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
      return "Duplicate";
    case "skipped_duplicate_bill":
      return "Duplicate";
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

function formatBillingPeriodLabel(month?: number | null, year?: number | null): string {
  if (!month || !year) return "No period";
  const monthName = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"][month - 1];
  return monthName ? `${monthName} ${year}` : `${String(month).padStart(2, "0")}.${year}`;
}

function candidateProviderIds(candidate: InboxPreviewCandidate) {
  return [
    ...new Set(
      candidate.bills
        .map((bill) => bill.provider_id)
        .filter((providerId): providerId is number => providerId != null),
    ),
  ];
}

function defaultInboxPreviewSelection(candidates: InboxPreviewCandidate[]) {
  const providerUseCount = new Map<number, number>();

  for (const candidate of candidates) {
    if (!candidate.selectable) continue;
    for (const providerId of candidateProviderIds(candidate)) {
      providerUseCount.set(providerId, (providerUseCount.get(providerId) ?? 0) + 1);
    }
  }

  return candidates
    .filter((candidate) => {
      if (!candidate.selectable) return false;
      return candidateProviderIds(candidate).every(
        (providerId) => (providerUseCount.get(providerId) ?? 0) === 1,
      );
    })
    .map((candidate) => candidate.id);
}

function parseSenderAllowlist(raw?: string | null): string[] {
  return (raw ?? "")
    .split(",")
    .map((item) => item.trim())
    .filter(Boolean);
}

function InboxChip({ children, className = "", title }: { children: ReactNode; className?: string; title?: string }) {
  return (
    <span title={title} className={`inline-flex h-6 max-w-full items-center gap-1.5 rounded-full border border-border bg-surface-3 px-2.5 text-xs font-semibold text-muted-foreground ${className}`}>
      {children}
    </span>
  );
}

function SenderAllowlistChip({ senders, busy, onEditSettings }: { senders: string[]; busy: boolean; onEditSettings: () => void }) {
  const [open, setOpen] = useState(false);
  const [popoverPosition, setPopoverPosition] = useState<{ top: number; left: number } | null>(null);
  const rootRef = useRef<HTMLDivElement | null>(null);
  const buttonRef = useRef<HTMLButtonElement | null>(null);
  const popoverRef = useRef<HTMLDivElement | null>(null);
  const label = senders.length === 0 ? "Any sender" : senders.length === 1 ? "1 sender" : `${senders.length} senders`;

  useEffect(() => {
    if (!open) {
      setPopoverPosition(null);
      return;
    }
    const updatePosition = () => {
      const rect = buttonRef.current?.getBoundingClientRect();
      if (!rect) return;
      const popoverWidth = 240;
      setPopoverPosition({
        top: rect.bottom + 8,
        left: Math.max(12, Math.min(rect.left, window.innerWidth - popoverWidth - 12)),
      });
    };
    const onPointerDown = (event: PointerEvent) => {
      const target = event.target as Node;
      if (!rootRef.current?.contains(target) && !popoverRef.current?.contains(target)) setOpen(false);
    };
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      event.stopPropagation();
      setOpen(false);
    };
    updatePosition();
    window.addEventListener("pointerdown", onPointerDown, true);
    window.addEventListener("keydown", onKeyDown, true);
    window.addEventListener("resize", updatePosition);
    window.addEventListener("scroll", updatePosition, true);
    return () => {
      window.removeEventListener("pointerdown", onPointerDown, true);
      window.removeEventListener("keydown", onKeyDown, true);
      window.removeEventListener("resize", updatePosition);
      window.removeEventListener("scroll", updatePosition, true);
    };
  }, [open]);

  return (
    <div ref={rootRef} className="relative inline-flex">
      <button
        ref={buttonRef}
        type="button"
        className="inline-flex h-6 max-w-full items-center gap-1.5 rounded-full border border-border bg-surface-3 px-2.5 text-xs font-semibold text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
        aria-haspopup="dialog"
        aria-expanded={open}
        aria-label={senders.length > 0 ? `Show ${senders.length} allowed inbox senders` : "Show inbox sender filter"}
        onClick={() => setOpen((current) => !current)}
      >
        {label}
        <ChevronDown className="size-3" />
      </button>
      {open && popoverPosition
        ? createPortal(
            <div
              ref={popoverRef}
              role="dialog"
              aria-label="Sender allowlist"
              className="fixed z-[70] w-60 rounded-md border border-border bg-card p-3 text-left shadow-pop"
              style={{ top: popoverPosition.top, left: popoverPosition.left }}
            >
              <div className="mb-2 text-[10px] font-bold uppercase tracking-wide text-muted-foreground">Sender allowlist</div>
              {senders.length > 0 ? (
                <div className="flex max-h-48 flex-col gap-1.5 overflow-y-auto pr-1">
                  {senders.map((sender) => (
                    <div key={sender} className="font-mono text-xs text-muted-foreground">
                      {sender}
                    </div>
                  ))}
                </div>
              ) : (
                <div className="text-xs leading-relaxed text-muted-foreground">No sender filter is configured. The preview will inspect supported attachments from any sender.</div>
              )}
              <div className="mt-3 border-t border-border pt-3">
                <Link
                  to="/settings"
                  search={{ tab: "delivery" }}
                  className={`${buttonVariants({ variant: "ghost", size: "sm" })} h-7 w-full justify-center text-xs`}
                  onClick={(event) => {
                    if (busy) {
                      event.preventDefault();
                      return;
                    }
                    setOpen(false);
                    onEditSettings();
                  }}
                >
                  <Settings className="size-3.5" />
                  Edit in Settings
                </Link>
              </div>
            </div>,
            document.body
          )
        : null}
    </div>
  );
}

function InboxStatusChip({ candidate }: { candidate: InboxPreviewCandidate }) {
  const [open, setOpen] = useState(false);
  const [popoverPosition, setPopoverPosition] = useState<{ top: number; right: number } | null>(null);
  const rootRef = useRef<HTMLDivElement | null>(null);
  const buttonRef = useRef<HTMLButtonElement | null>(null);
  const popoverRef = useRef<HTMLDivElement | null>(null);
  const detail = candidate.error ?? candidate.skipped_reason ?? candidate.notices[0]?.message ?? null;
  const hasNotices = candidate.notices.length > 0;
  const isReady = candidate.status === "ready" && !hasNotices;
  const isFailed = candidate.status === "failed";
  const label = candidate.status === "ready" && hasNotices ? "Review" : previewStatusLabel(candidate.status);
  const className = isReady ? "bg-success-soft text-success" : isFailed ? "bg-danger-soft text-danger" : hasNotices ? "bg-warning-soft text-warning" : "bg-surface-3 text-muted-foreground";

  useEffect(() => {
    if (!open) return;
    const updatePosition = () => {
      const rect = buttonRef.current?.getBoundingClientRect();
      if (!rect) return;
      setPopoverPosition({
        top: rect.bottom + 8,
        right: window.innerWidth - rect.right,
      });
    };
    const onPointerDown = (event: PointerEvent) => {
      const target = event.target as Node;
      if (!rootRef.current?.contains(target) && !popoverRef.current?.contains(target)) setOpen(false);
    };
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      event.stopPropagation();
      setOpen(false);
    };
    updatePosition();
    window.addEventListener("pointerdown", onPointerDown, true);
    window.addEventListener("keydown", onKeyDown, true);
    window.addEventListener("resize", updatePosition);
    window.addEventListener("scroll", updatePosition, true);
    return () => {
      window.removeEventListener("pointerdown", onPointerDown, true);
      window.removeEventListener("keydown", onKeyDown, true);
      window.removeEventListener("resize", updatePosition);
      window.removeEventListener("scroll", updatePosition, true);
    };
  }, [open]);

  if (!detail) {
    return (
      <span className={`inline-flex whitespace-nowrap rounded-md px-2 py-1 text-xs font-semibold ${className}`}>
        {label}
      </span>
    );
  }

  return (
    <div ref={rootRef} className="relative inline-flex">
      <button
        ref={buttonRef}
        type="button"
        className={`inline-flex items-center gap-1.5 whitespace-nowrap rounded-md px-2.5 py-1 text-xs font-semibold transition-colors hover:bg-accent hover:text-accent-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring ${className}`}
        aria-haspopup="dialog"
        aria-expanded={open}
        aria-label={`Show ${label.toLowerCase()} reason`}
        onClick={() => setOpen((current) => !current)}
      >
        {label}
        <ChevronDown className="size-3" />
      </button>
      {open && popoverPosition
        ? createPortal(
            <div
              ref={popoverRef}
              role="dialog"
              aria-label="Inbox preview status reason"
              className="fixed z-[70] w-64 rounded-md border border-border bg-card p-3 text-left shadow-pop"
              style={{ top: popoverPosition.top, right: popoverPosition.right }}
            >
              <div className="mb-2 text-[10px] font-bold uppercase tracking-wide text-muted-foreground">Reason</div>
              <div className="break-words text-xs leading-relaxed text-muted-foreground">{detail}</div>
            </div>,
            document.body
          )
        : null}
    </div>
  );
}

function InboxImportDrawer({
  open,
  billingPeriodId,
  periodLabel,
  onClose,
  onImported,
}: {
  open: boolean;
  billingPeriodId: number | null;
  periodLabel: string;
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
      setPreview(null);
      setSelectedIds(new Set());
      const clampedDays = Math.min(90, Math.max(1, Math.round(daysToScan || 1)));
      setDaysToScan(clampedDays);
      const nextPreview = await ipc.previewInboxAttachments(billingPeriodId, clampedDays);
      setPreview(nextPreview);
      setSelectedIds(new Set(defaultInboxPreviewSelection(nextPreview.candidates)));
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

  const setClampedScanDays = (next: number) => {
    setDaysToScan(Math.min(90, Math.max(1, Math.round(next || 1))));
  };

  const toggleAllReady = (checked: boolean) => {
    if (!preview) return;
    setSelectedIds(checked ? new Set(defaultInboxPreviewSelection(preview.candidates)) : new Set());
  };

  if (!open) return null;

  const readyCount = preview?.candidates.reduce((sum, candidate) => sum + (candidate.selectable ? candidate.importable_count : 0), 0) ?? 0;
  const skippedCount = preview?.candidates.filter((candidate) => candidate.status.startsWith("skipped_") || candidate.status === "empty").length ?? 0;
  const failedCount = preview?.candidates.filter((candidate) => candidate.status === "failed").length ?? 0;
  const importedCount = results.filter((result) => result.status === "imported").length;
  const resultSkippedCount = results.filter((result) => result.status.startsWith("skipped_")).length;
  const resultFailedCount = results.filter((result) => result.status === "failed").length;
  const scanSummary = preview?.scan_summary;
  const senderEntries = parseSenderAllowlist(config?.sender_allowlist);
  const defaultSelectedIds = preview ? defaultInboxPreviewSelection(preview.candidates) : [];
  const allReadySelected =
    defaultSelectedIds.length > 0 &&
    defaultSelectedIds.every((candidateId) => selectedIds.has(candidateId));
  const accountLabel = config ? `${config.username || "Inbox account"} / ${config.folder || "INBOX"}` : "Loading inbox settings";
  const selectedBillCount = preview?.candidates.reduce((sum, candidate) => sum + (selectedIds.has(candidate.id) ? candidate.importable_count : 0), 0) ?? 0;

  return (
    <div className="fixed inset-0 z-50">
      <button
        type="button"
        className="absolute inset-0 bg-overlay animate-drawer-scrim-in"
        aria-label="Close inbox import"
        disabled={busy}
        onClick={() => void closeDrawer()}
      />
      <aside className="absolute right-0 top-0 flex h-full w-full max-w-4xl flex-col border-l border-border bg-card shadow-pop animate-drawer-slide-in sm:w-[760px]">
        <header className="border-b border-border px-5 py-4">
          <div className="flex items-start justify-between gap-4">
            <h2 className="font-head text-lg font-semibold">Import from Inbox</h2>
            <Button variant="ghost" size="icon" onClick={() => void closeDrawer()} disabled={busy} aria-label="Close">
              <X className="size-4" />
            </Button>
          </div>
          <div className="mt-2 flex flex-wrap items-center gap-2">
            <InboxChip className="bg-card">
              <Mail className="size-3" />
              <span className="max-w-64 truncate">{accountLabel}</span>
            </InboxChip>
            <InboxChip className="bg-accent-soft text-accent-foreground">
              <Calendar className="size-3" />
              {periodLabel}
            </InboxChip>
            <div className="inline-flex h-6 items-center overflow-hidden rounded-full border border-border bg-surface-3 text-muted-foreground">
              <button
                type="button"
                className="grid size-6 place-items-center transition-colors hover:bg-accent hover:text-accent-foreground disabled:opacity-50"
                onClick={() => setClampedScanDays(daysToScan - 1)}
                disabled={loadingConfig || loadingPreview || importing || daysToScan <= 1}
                aria-label="Decrease scan window"
              >
                <Minus className="size-3" />
              </button>
              <input
                aria-label="Scan window days"
                className="input-no-spinner h-6 w-10 border-x border-border bg-transparent text-center font-mono text-xs font-bold text-foreground outline-none disabled:opacity-50"
                type="number"
                min={1}
                max={90}
                value={daysToScan}
                onChange={(event) => setClampedScanDays(Number(event.target.value))}
                disabled={loadingConfig || loadingPreview || importing}
              />
              <button
                type="button"
                className="grid size-6 place-items-center transition-colors hover:bg-accent hover:text-accent-foreground disabled:opacity-50"
                onClick={() => setClampedScanDays(daysToScan + 1)}
                disabled={loadingConfig || loadingPreview || importing || daysToScan >= 90}
                aria-label="Increase scan window"
              >
                <Plus className="size-3" />
              </button>
            </div>
            <SenderAllowlistChip senders={senderEntries} busy={busy} onEditSettings={() => void closeDrawer()} />
          </div>
        </header>

        <div className="flex-1 space-y-4 overflow-y-auto px-5 py-4">
          {error && (
            <div className="rounded-md border border-danger/30 bg-danger-soft px-4 py-3 text-sm text-danger">
              {error}
            </div>
          )}

          {!preview && results.length === 0 && (
            <div className="flex min-h-[360px] flex-col items-center justify-center gap-4 px-6 py-10 text-center">
              <span className="grid size-12 place-items-center rounded-lg bg-surface-3 text-muted-foreground">
                <Inbox className="size-6" />
              </span>
              <div className="max-w-md">
                <h3 className="font-head text-xl font-semibold">Ready to scan</h3>
                <p className="mt-2 text-sm leading-relaxed text-muted-foreground">
                  Will scan the last <span className="font-semibold text-foreground">{daysToScan} days</span> of {config?.folder || "INBOX"} for PDF and image attachments
                  {senderEntries.length > 0 ? <> from <span className="font-semibold text-foreground">{senderEntries.length} senders</span></> : null}. Only bills for the <span className="font-semibold text-foreground">{periodLabel}</span> billing month will be offered.
                </p>
              </div>
              <Button onClick={fetchPreview} disabled={!billingPeriodId || loadingConfig || loadingPreview || importing} className="h-10 px-6">
                {loadingPreview ? <Loader2 className="size-4 animate-spin" /> : <Mail className="size-4" />}
                Fetch preview
              </Button>
              <span className="text-xs text-muted-foreground">This may take a few seconds.</span>
            </div>
          )}

          {preview && results.length === 0 && (
            <div className="space-y-3">
              <div className="flex flex-wrap items-center gap-2 rounded-md border border-border bg-surface-2 px-3 py-2">
                <span className="text-[11px] font-bold uppercase tracking-wide text-muted-foreground">Scanned</span>
                <InboxChip className="h-5 bg-card">
                  <Clock className="size-3" />
                  Last {preview.days_to_scan} days
                </InboxChip>
                <SenderAllowlistChip senders={senderEntries} busy={busy} onEditSettings={() => void closeDrawer()} />
                <Button variant="ghost" size="sm" className="ml-auto h-7" onClick={fetchPreview} disabled={loadingPreview || importing}>
                  {loadingPreview ? <Loader2 className="size-3.5 animate-spin" /> : <RefreshCw className="size-3.5" />}
                  Re-scan
                </Button>
              </div>
              <div className="flex flex-wrap items-center gap-2 text-sm">
                <span className="text-xs font-bold uppercase tracking-wide text-muted-foreground">Preview</span>
                <span className="inline-flex items-center gap-1.5 rounded-md bg-success-soft px-3 py-1 text-xs font-semibold text-success">
                  <Check className="size-3" />
                  {readyCount} ready
                </span>
                <span className="rounded-md bg-surface-3 px-3 py-1 text-xs font-semibold text-muted-foreground">{skippedCount} skipped</span>
                {failedCount > 0 && (
                  <span className="rounded-md bg-danger-soft px-3 py-1 text-xs font-semibold text-danger">{failedCount} failed</span>
                )}
              </div>
              <BillingTableFrame>
                <BillingTable>
                  <thead>
                    <BillingTableHeaderRow>
                      <BillingTableHeaderCell className="w-14 p-0 align-middle">
                        <div className="flex h-11 items-center justify-center">
                          {defaultSelectedIds.length > 0 && (
                            <input
                              aria-label="Select all non-conflicting inbox preview bills"
                              type="checkbox"
                              className="block size-4 accent-primary"
                              checked={allReadySelected}
                              onChange={(event) => toggleAllReady(event.target.checked)}
                              disabled={importing}
                            />
                          )}
                        </div>
                      </BillingTableHeaderCell>
                      <BillingTableHeaderCell>Attachment</BillingTableHeaderCell>
                      <BillingTableHeaderCell>Bill</BillingTableHeaderCell>
                      <BillingTableHeaderCell className="w-28 text-right">Amount</BillingTableHeaderCell>
                      <BillingTableHeaderCell className="w-40 px-6 text-right">Status</BillingTableHeaderCell>
                    </BillingTableHeaderRow>
                  </thead>
                  <tbody>
                    {preview.candidates.length === 0 ? (
                      <tr>
                        <td colSpan={5} className="px-4 py-6 text-sm text-muted-foreground">
                          <div className="text-center font-medium">No supported bill attachments found in this scan window.</div>
                          {scanSummary && (
                            <div className="mx-auto mt-4 grid max-w-2xl gap-2 rounded-md bg-surface-2 p-3 text-left text-xs">
                              <div className="grid gap-2 sm:grid-cols-2">
                                <div>Messages matched: <span className="font-semibold text-foreground">{scanSummary.messages_matched}</span></div>
                                <div>Messages fetched: <span className="font-semibold text-foreground">{scanSummary.messages_fetched}</span></div>
                                <div>Sender-filtered: <span className="font-semibold text-foreground">{scanSummary.messages_skipped_sender}</span></div>
                                <div>Oversize messages: <span className="font-semibold text-foreground">{scanSummary.messages_skipped_oversize}</span></div>
                                <div>Supported attachments: <span className="font-semibold text-foreground">{scanSummary.supported_attachments_found}</span></div>
                                <div>No supported attachments: <span className="font-semibold text-foreground">{scanSummary.messages_without_supported_attachments}</span></div>
                                <div>Unsupported attachments: <span className="font-semibold text-foreground">{scanSummary.unsupported_attachments_found}</span></div>
                              </div>
                              {scanSummary.senders_seen.length > 0 && (
                                <div className="truncate">Senders seen: <span className="text-foreground">{scanSummary.senders_seen.join(", ")}</span></div>
                              )}
                              {scanSummary.unsupported_attachment_names.length > 0 && (
                                <div className="truncate">Unsupported files: <span className="text-foreground">{scanSummary.unsupported_attachment_names.join(", ")}</span></div>
                              )}
                            </div>
                          )}
                        </td>
                      </tr>
                    ) : (
                      preview.candidates.map((candidate) => {
                        const groupClass = candidate.selectable ? "bg-card" : "bg-surface-2/40";
                        const toggleCandidate = (checked: boolean) => {
                          const next = new Set(selectedIds);
                          if (checked) next.add(candidate.id);
                          else next.delete(candidate.id);
                          setSelectedIds(next);
                        };
                        const selectionCell = (keySuffix: string) => (
                          <td className="w-14 p-0 align-middle">
                            <div className="flex justify-center">
                              <input
                                aria-label={`Select ${candidate.attachment_filename}${keySuffix ? ` bill ${keySuffix}` : ""}`}
                                type="checkbox"
                                className="block size-4 accent-primary"
                                checked={selectedIds.has(candidate.id)}
                                disabled={!candidate.selectable || importing}
                                onChange={(event) => toggleCandidate(event.target.checked)}
                              />
                            </div>
                          </td>
                        );
                        const attachmentCell = (
                          <td className="max-w-64 px-3 py-4 align-top">
                            <div className="truncate font-semibold">{candidate.attachment_filename}</div>
                            <div className="truncate text-xs text-muted-foreground">{candidate.sender || "Unknown sender"}</div>
                            <div className="truncate text-xs text-muted-foreground">{candidate.subject || "No subject"}</div>
                          </td>
                        );
                        const statusCell = (
                          <td className="w-40 px-6 py-4 text-right align-top">
                            <InboxStatusChip candidate={candidate} />
                          </td>
                        );

                        if (candidate.bills.length === 0) {
                          return (
                            <tr key={candidate.id} className={`border-b border-border align-top ${groupClass}`}>
                              {selectionCell("")}
                              {attachmentCell}
                              <td className="px-3 py-4 align-top">
                                <span className="text-xs text-muted-foreground">No importable bills</span>
                              </td>
                              <td className="px-3 py-4 text-right align-top text-xs text-muted-foreground">-</td>
                              {statusCell}
                            </tr>
                          );
                        }

                        return (
                          <Fragment key={candidate.id}>
                            {candidate.bills.map((bill, index) => (
                              <tr key={`${candidate.id}-bill-${index}`} className={`border-b border-border align-top ${groupClass}`}>
                                {selectionCell(String(index + 1))}
                                {attachmentCell}
                                <td className="px-3 py-4 align-top">
                                  <div className="font-semibold">{bill.provider_name ?? (bill.creditor_name || "Unmatched bill")}</div>
                                  <div className="mt-1 truncate text-xs text-muted-foreground">{bill.reference || "No reference"} / {bill.due_date || "No due date"}</div>
                                  {bill.parse_note && (
                                    <div className="mt-1 text-xs text-warning">{bill.parse_note}</div>
                                  )}
                                </td>
                                <td className="px-3 py-4 text-right align-top">
                                  <span className="font-mono text-xs">{formatEur(bill.amount_cents)} €</span>
                                </td>
                                {statusCell}
                              </tr>
                            ))}
                          </Fragment>
                        );
                      })
                    )}
                  </tbody>
                </BillingTable>
              </BillingTableFrame>
            </div>
          )}

          {results.length > 0 && (
            <div className="space-y-4">
              <div className="flex items-center gap-4 rounded-lg border border-success/40 bg-success-soft px-4 py-4">
                <CheckCircle2 className="size-8 shrink-0 text-success" />
                <div>
                  <h3 className="font-head text-lg font-semibold text-success">Import complete</h3>
                  <p className="mt-1 text-sm text-muted-foreground">
                    <span className="font-semibold text-foreground">{importedCount}</span> imported /{" "}
                    <span className="font-semibold text-foreground">{resultSkippedCount}</span> skipped
                    {resultFailedCount > 0 ? (
                      <>
                        {" "} / <span className="font-semibold text-foreground">{resultFailedCount}</span> failed
                      </>
                    ) : null}
                  </p>
                </div>
              </div>

              {importedCount > 0 && (
                <div>
                  <div className="mb-2 text-[11px] font-bold uppercase tracking-wide text-muted-foreground">Imported</div>
                  <div className="space-y-2">
                    {results.filter((result) => result.status === "imported").map((result, index) => (
                      <div key={`imported-${index}`} className="flex items-center gap-3 rounded-md border border-border bg-surface-2 px-3 py-2">
                        <Check className="size-4 shrink-0 text-success" />
                        <div className="min-w-0">
                          <div className="truncate text-sm font-semibold">{result.attachment_filename}</div>
                          <div className="text-xs text-muted-foreground">
                            {result.bill_count} bill{result.bill_count === 1 ? "" : "s"}
                          </div>
                        </div>
                      </div>
                    ))}
                  </div>
                </div>
              )}

              {(resultSkippedCount > 0 || resultFailedCount > 0) && (
                <div>
                  <div className="mb-2 text-[11px] font-bold uppercase tracking-wide text-muted-foreground">Skipped</div>
                  <div className="space-y-2">
                    {results.filter((result) => result.status !== "imported").map((result, index) => (
                      <div key={`skipped-${index}`} className="flex items-start gap-3 rounded-md border border-border bg-surface-2 px-3 py-2 opacity-80">
                        <X className={`mt-0.5 size-4 shrink-0 ${result.status === "failed" ? "text-danger" : "text-muted-foreground"}`} />
                        <div className="min-w-0">
                          <div className="truncate text-sm font-semibold">{result.attachment_filename}</div>
                          <div className="mt-0.5 text-xs text-muted-foreground">
                            {result.error ?? result.skipped_reason ?? result.status}
                          </div>
                        </div>
                      </div>
                    ))}
                  </div>
                </div>
              )}
            </div>
          )}
        </div>

        <footer className="flex items-center justify-between gap-3 border-t border-border px-5 py-4">
          {results.length > 0 ? (
            <>
              <div className="text-sm text-muted-foreground">Import complete. You can close this panel.</div>
              <div className="flex gap-2">
                <Button variant="outline" onClick={() => void closeDrawer()} disabled={busy}>
                  Close
                </Button>
                <Button variant="ghost" onClick={fetchPreview} disabled={!billingPeriodId || loadingPreview || importing}>
                  {loadingPreview ? <Loader2 className="size-4 animate-spin" /> : <RefreshCw className="size-4" />}
                  Import again
                </Button>
              </div>
            </>
          ) : preview ? (
            <>
              <div className="text-sm text-muted-foreground">
                <span className="font-semibold text-foreground">{selectedBillCount}</span> selected
              </div>
              <div className="flex gap-2">
                <Button variant="outline" onClick={() => void closeDrawer()} disabled={busy}>
                  Close
                </Button>
                <Button onClick={importSelected} disabled={!preview || selectedIds.size === 0 || importing || loadingPreview}>
                  {importing ? <Loader2 className="size-4 animate-spin" /> : <Check className="size-4" />}
                  Import selected ({selectedBillCount})
                </Button>
              </div>
            </>
          ) : (
            <>
              <div />
              <Button variant="outline" onClick={() => void closeDrawer()} disabled={busy}>
                Close
              </Button>
            </>
          )}
        </footer>
      </aside>
    </div>
  );
}

function BillsPage() {
  const { selected } = useBillingPeriodSelection();
  const snapshot = useWorkflowSnapshotContext();
  const bills = snapshot.bills;
  const [importing, setImporting] = useState(false);
  const [inboxDrawerOpen, setInboxDrawerOpen] = useState(false);
  const [inboxResults, setInboxResults] = useState<InboxImportResult[]>([]);
  const [error, setError] = useState<string | null>(null);

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
      await snapshot.refresh({ core: false, periods: false, selected: true, statuses: true });
    } finally {
      setImporting(false);
    }
  };

  const handleInboxImported = async (results: InboxImportResult[]) => {
    setInboxResults(results);
    if (selected?.id) {
      await snapshot.refresh({ core: false, periods: false, selected: true, statuses: true });
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
    await snapshot.refresh({ core: false, periods: false, selected: true, statuses: true });
  };

  const saveBill = async (bill: Bill) => {
    await ipc.saveBill(bill);
    if (selected?.id) {
      await snapshot.refresh({ core: false, periods: false, selected: true, statuses: true });
    }
  };

  const deleteBill = async (id: number) => {
    await ipc.deleteBill(id);
    if (selected?.id) {
      await snapshot.refresh({ core: false, periods: false, selected: true, statuses: true });
    }
  };

  const totalCents = bills.reduce((s, b) => s + b.amount_cents, 0);
  const reviewBills = bills.filter((bill) => bill.parse_note?.trim());
  const cleanCount = Math.max(0, bills.length - reviewBills.length);
  const inboxImported = inboxResults.filter((result) => result.status === "imported");
  const inboxSkipped = inboxResults.filter((result) => result.status.startsWith("skipped_"));
  const inboxFailed = inboxResults.filter((result) => result.status === "failed");
  const workflowError = error ?? snapshot.error;
  const selectedPeriodId = selected?.id ?? null;
  const selectedStatusKnown =
    selectedPeriodId !== null && snapshot.periodStatuses.has(selectedPeriodId);
  const showBillsLoading =
    selectedPeriodId !== null &&
    snapshot.loading &&
    (bills.length > 0 || snapshot.selectedStatus.bills);
  const showBillsSettling =
    selectedPeriodId !== null &&
    snapshot.loading &&
    !showBillsLoading &&
    selectedStatusKnown;
  const showBillsTable = selectedPeriodId !== null && !snapshot.loading && bills.length > 0;

  return (
    <BillingPageShell
      title="Bills"
      subtitle={null}
      actions={
        selected ? (
          <>
            <Button variant="outline" onClick={addBlankBill}>
              <Plus className="size-4" />
              Add Bill
            </Button>
            <Button variant="outline" onClick={() => setInboxDrawerOpen(true)}>
              <Mail className="size-4" />
              Import from Inbox
            </Button>
            <Button onClick={importFiles} disabled={importing}>
              {importing ? (
                <Loader2 className="size-4 animate-spin" />
              ) : (
                <FilePlus className="size-4" />
              )}
              Import Bills
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
        periodLabel={formatBillingPeriodLabel(selected?.month, selected?.year)}
        onClose={() => setInboxDrawerOpen(false)}
        onImported={handleInboxImported}
      />

      {!selected && (
        <div className="rounded-lg border border-dashed border-border px-4 py-5 text-sm text-muted-foreground">
          No billing period selected. Use the month picker above to add or select a year.
        </div>
      )}

      {workflowError && (
        <div className="rounded-md border border-danger/30 bg-danger-soft px-4 py-3 text-sm text-danger">
          {workflowError}
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

      {selected && showBillsTable && (
        <SummaryStrip>
          <span className="mr-1 text-[11px] font-bold uppercase tracking-wide text-muted-foreground">
            Imported from
          </span>
          {cleanCount > 0 && (
            <SummaryChip className="bg-success-soft text-success">
              <CheckCircle2 className="size-3.5" />
              {cleanCount} file{cleanCount === 1 ? "" : "s"} clean
            </SummaryChip>
          )}
          {reviewBills.map((bill) => (
            <SummaryChip
              key={bill.id ?? bill.source_filename}
              className="bg-warning-soft text-warning font-normal"
              title={bill.parse_note}
            >
              <AlertTriangle className="size-3.5" />
              <span className="font-semibold text-foreground">{bill.source_filename}</span>
              <span className="text-muted-foreground">needs review</span>
            </SummaryChip>
          ))}
        </SummaryStrip>
      )}

      {selected && (
        <BillingTableFrame minHeight>
          {showBillsLoading ? (
            <BillingEmptyState
              loading
              loadingLabel="Loading bills..."
              title="No bills yet for this billing month"
              detail="Use the buttons above to import PDF or image invoices, or add a bill manually."
            />
          ) : showBillsSettling || bills.length === 0 ? (
            <BillingEmptyState
              title="No bills yet for this billing month"
              detail="Use the buttons above to import PDF or image invoices, or add a bill manually."
            />
          ) : (
            <BillingTable>
              <thead>
                <BillingTableHeaderRow>
                  <BillingTableHeaderCell className="w-[68px] p-0" />
                  <BillingTableHeaderCell className="pl-0">Provider</BillingTableHeaderCell>
                  <BillingTableHeaderCell>Reference</BillingTableHeaderCell>
                  <BillingTableHeaderCell>Due Date</BillingTableHeaderCell>
                  <BillingTableHeaderCell>Detection</BillingTableHeaderCell>
                  <BillingTableHeaderCell className="text-right">Amount</BillingTableHeaderCell>
                  <BillingTableHeaderCell />
                </BillingTableHeaderRow>
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
                <BillingTableFooterRow>
                  <td />
                  <td
                    className="py-2 pr-3 text-xs text-muted-foreground"
                  >
                    Total ({bills.length} bills)
                  </td>
                  <td colSpan={3}></td>
                  <td className={billingTableNumericCellClass}>
                    {formatEur(totalCents)} €
                  </td>
                  <td></td>
                </BillingTableFooterRow>
              </tfoot>
            </BillingTable>
          )}
        </BillingTableFrame>
      )}

      {selected && showBillsTable && (
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
