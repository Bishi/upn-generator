import { createFileRoute, Link } from "@tanstack/react-router";
import { Fragment, useEffect, useRef, useState } from "react";
import {
  AlertTriangle,
  Mail,
  Download,
  Eye,
  CheckCircle2,
  XCircle,
  Files,
  ChevronDown,
  Loader2,
} from "lucide-react";
import { ipc } from "@/lib/ipc";
import { useBillingPeriodSelection } from "@/lib/billing-period-selection";
import { useWorkflowSnapshotContext } from "@/lib/workflow-snapshot";
import type {
  Apartment,
  EmailResult,
  SplitRow,
  UpnDeliveryEvent,
  UpnPacketHash,
} from "@/lib/types";
import { formatEur } from "@/lib/types";
import { BillingPageShell } from "@/components/BillingPageShell";
import { Button, buttonVariants } from "@/components/ui/button";
import { downloadPeriodUpnPdfs, sendPeriodEmails } from "@/lib/upn-actions";

export const Route = createFileRoute("/upn")({
  component: UpnPage,
});

function parseRecipientList(raw: string) {
  return raw
    .split(",")
    .map((recipient) => recipient.trim())
    .filter(Boolean);
}

function hasSendableRecipient(raw: string) {
  return parseRecipientList(raw).length > 0;
}

function statusLabel(status: EmailResult["status"]) {
  switch (status) {
    case "sent":
      return "sent";
    case "blocked":
      return "blocked";
    case "partial":
      return "partial";
    case "changed":
      return "changed";
    default:
      return "failed";
  }
}

function statusClass(status: EmailResult["status"]) {
  if (status === "sent") return "bg-success-soft text-success";
  if (status === "blocked") return "bg-warning-soft text-warning";
  if (status === "partial") return "bg-warning-soft text-warning";
  if (status === "changed") return "bg-warning-soft text-warning";
  return "bg-danger-soft text-danger";
}

function normalizedRecipients(raw: string) {
  return parseRecipientList(raw).map((recipient) => recipient.toLowerCase()).sort();
}

function sameRecipients(left: string[], right: string[]) {
  if (left.length !== right.length) return false;
  return left.every((value, index) => value === right[index]);
}

function aggregateHistoryEvents(
  events: UpnDeliveryEvent[],
  apartmentsById: Map<number, Apartment>,
  packetHashesByApartmentId: Map<number, UpnPacketHash>,
): Map<number, EmailResult> {
  const latestAttemptByApartment = new Map<number, string>();
  const latestSortByApartment = new Map<number, string>();

  for (const event of events) {
    const sortKey = `${event.created_at}:${String(event.id).padStart(12, "0")}`;
    const previous = latestSortByApartment.get(event.apartment_id);
    if (!previous || sortKey >= previous) {
      latestSortByApartment.set(event.apartment_id, sortKey);
      latestAttemptByApartment.set(event.apartment_id, event.attempt_id);
    }
  }

  const result = new Map<number, EmailResult>();
  for (const [apartmentId, attemptId] of latestAttemptByApartment) {
    const rows = events.filter(
      (event) => event.apartment_id === apartmentId && event.attempt_id === attemptId,
    );
    const statuses = new Set(rows.map((event) => event.status));
    const status: EmailResult["status"] =
      statuses.size === 1
        ? (rows[0]?.status as EmailResult["status"])
        : "partial";
    const apartment = apartmentsById.get(apartmentId) ?? null;
    const currentRecipients = normalizedRecipients(apartment?.contact_email ?? "");
    const historicalRecipients = rows
      .map((event) => event.recipient.trim().toLowerCase())
      .filter(Boolean)
      .sort();
    const currentPacketHash = packetHashesByApartmentId.get(apartmentId);
    const historicalHashes = [...new Set(rows.map((event) => event.attachment_sha256))].filter(
      Boolean,
    );
    const matchesRecipients = sameRecipients(currentRecipients, historicalRecipients);
    const matchesPacket =
      Boolean(currentPacketHash?.attachment_sha256) &&
      historicalHashes.length === 1 &&
      historicalHashes[0] === currentPacketHash?.attachment_sha256;
    const isCurrent = matchesRecipients && matchesPacket;
    const displayStatus: EmailResult["status"] = isCurrent ? status : "changed";
    const errors = rows
      .map((event) => event.error)
      .filter(Boolean)
      .filter((error, index, all) => all.indexOf(error) === index);
    if (!isCurrent) {
      errors.unshift(
        currentPacketHash?.error
          ? `Current packet could not be checked: ${currentPacketHash.error}`
          : "Current recipients or UPN packet changed since this delivery attempt.",
      );
    }

    result.set(apartmentId, {
      apartment_id: apartmentId,
      apartment_label: apartment?.label ?? `Apartment ${apartmentId}`,
      email: apartment?.contact_email ?? rows.map((event) => event.original_recipient).join(", "),
      status: displayStatus,
      recipient: rows.map((event) => event.recipient).join(", "),
      original_recipient: rows.map((event) => event.original_recipient).join(", "),
      success: displayStatus === "sent",
      error: errors.length > 0 ? errors.join("; ") : null,
    });
  }

  return result;
}

function ApartmentRows({
  billingPeriodId,
  apartmentId,
  apartmentLabel,
  apartmentUnitCode,
  contactEmail,
  splits,
  emailResult,
  expanded,
  onToggle,
  onPreviewError,
}: {
  billingPeriodId: number;
  apartmentId: number;
  apartmentLabel: string;
  apartmentUnitCode: string;
  contactEmail: string;
  splits: SplitRow[];
  emailResult?: EmailResult;
  expanded: boolean;
  onToggle: () => void;
  onPreviewError: (message: string | null) => void;
}) {
  const [loadingPreview, setLoadingPreview] = useState<number | null>(null);
  const [previewingAll, setPreviewingAll] = useState(false);

  const previewUpn = async (billId: number) => {
    setLoadingPreview(billId);
    try {
      onPreviewError(null);
      const path = await ipc.openPreviewUpn(billId, apartmentId);
      if (!path || !path.trim()) {
        throw new Error("Preview did not return a PDF path.");
      }
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      onPreviewError(`Could not open the UPN preview. ${message}`);
    } finally {
      setLoadingPreview(null);
    }
  };

  const previewAll = async () => {
    setPreviewingAll(true);
    try {
      onPreviewError(null);
      const path = await ipc.openPreviewApartmentUpns(billingPeriodId, apartmentId);
      if (!path || !path.trim()) {
        throw new Error("Preview All did not return a PDF path.");
      }
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      onPreviewError(`Could not open the combined UPN preview. ${message}`);
    } finally {
      setPreviewingAll(false);
    }
  };

  const total = splits.reduce((s, r) => s + r.split_amount_cents, 0);
  const hasRecipient = hasSendableRecipient(contactEmail);

  return (
    <Fragment>
      <tr
        className="cursor-pointer border-b border-border hover:bg-accent/10"
        onClick={onToggle}
      >
        <td className="px-3 py-3">
          <button
            type="button"
            onClick={(event) => {
              event.stopPropagation();
              onToggle();
            }}
            className="flex min-w-0 items-center gap-2 text-left"
          >
            <ChevronDown
              className={`size-3.5 shrink-0 text-muted-foreground transition-transform ${
                expanded ? "" : "-rotate-90"
              }`}
            />
            <div className="min-w-0">
              <div className="truncate text-sm font-semibold">{apartmentLabel}</div>
              <div className="font-mono text-[11px] text-muted-foreground">
                {apartmentUnitCode || "No unit code"} - {splits.length} UPN
                {splits.length === 1 ? "" : "s"}
              </div>
            </div>
          </button>
        </td>
        <td className="px-3 py-3">
          {hasRecipient ? (
            <span className="font-mono text-xs text-muted-foreground">
              {contactEmail}
            </span>
          ) : (
            <span className="inline-flex items-center gap-1.5 rounded-md bg-warning-soft px-2 py-1 text-xs font-semibold text-warning">
              <Mail className="size-3" />
              no email
            </span>
          )}
        </td>
        <td className="px-3 py-3 text-right">
          {emailResult ? (
            <span
              className={`inline-flex items-center gap-1.5 rounded-md px-2 py-1 text-xs font-semibold ${statusClass(
                emailResult.status,
              )}`}
            >
              {emailResult.status === "sent" ? (
                <CheckCircle2 className="size-3" />
              ) : (
                <XCircle className="size-3" />
              )}
              {statusLabel(emailResult.status)}
            </span>
          ) : hasRecipient ? (
            <span className="inline-flex rounded-md bg-success-soft px-2 py-1 text-xs font-semibold text-success">
              ready
            </span>
          ) : (
            <span className="inline-flex rounded-md bg-warning-soft px-2 py-1 text-xs font-semibold text-warning">
              hold
            </span>
          )}
        </td>
        <td className="px-3 py-3 text-right font-mono font-semibold">
          {formatEur(total)} €
        </td>
        <td className="px-3 py-3" onClick={(event) => event.stopPropagation()}>
          <Button
            variant="ghost"
            size="sm"
            onClick={previewAll}
            disabled={previewingAll || splits.length === 0}
            className="w-full"
          >
            {previewingAll ? (
              <Loader2 className="size-3.5 animate-spin" />
            ) : (
              <Eye className="size-3.5" />
            )}
            Preview all
          </Button>
        </td>
      </tr>
      {expanded &&
        splits.map((split) => (
          <tr key={split.bill_id} className="border-b border-border bg-surface-2">
            <td colSpan={2} className="px-9 py-2">
              <div className="text-sm font-medium">
                {split.provider_name ?? split.bill_source_filename}
              </div>
              <div className="text-xs text-muted-foreground">
                {split.bill_source_filename}
              </div>
            </td>
            <td />
            <td className="px-3 py-2 text-right font-mono text-sm text-muted-foreground">
              {formatEur(split.split_amount_cents)} €
            </td>
            <td className="px-3 py-2">
              <Button
                variant="ghost"
                size="sm"
                onClick={() => previewUpn(split.bill_id)}
                disabled={loadingPreview === split.bill_id}
                className="w-full"
              >
                {loadingPreview === split.bill_id ? (
                  <Loader2 className="size-3.5 animate-spin" />
                ) : (
                  <Eye className="size-3.5" />
                )}
                Preview
              </Button>
            </td>
          </tr>
        ))}
    </Fragment>
  );

}

function UpnPage() {
  const { selected } = useBillingPeriodSelection();
  const snapshot = useWorkflowSnapshotContext();
  const [splits, setSplits] = useState<SplitRow[]>(() => snapshot.splits);
  const [apartmentsConfig, setApartmentsConfig] = useState<Apartment[]>(
    () => snapshot.apartments,
  );
  const [loadingApartments, setLoadingApartments] = useState(() => snapshot.loading);
  const [loadingSplits, setLoadingSplits] = useState(() => snapshot.loading);
  const [sending, setSending] = useState(false);
  const [downloading, setDownloading] = useState(false);
  const [emailResults, setEmailResults] = useState<EmailResult[]>([]);
  const [deliveryEvents, setDeliveryEvents] = useState<UpnDeliveryEvent[]>([]);
  const [packetHashes, setPacketHashes] = useState<UpnPacketHash[]>([]);
  const [pageMessage, setPageMessage] = useState<string | null>(null);
  const loadRequestRef = useRef(0);
  const loadedPeriodIdRef = useRef<number | null>(snapshot.loading ? null : selected?.id ?? null);
  const loadedApartmentsRef = useRef(!snapshot.loading);
  const [expandedApartmentId, setExpandedApartmentId] = useState<number | null>(null);

  useEffect(() => {
    let cancelled = false;
    if (!loadedApartmentsRef.current) setLoadingApartments(true);
    void ipc
      .getApartments()
      .then((apartments) => {
        if (!cancelled) {
          setApartmentsConfig(apartments);
          loadedApartmentsRef.current = true;
        }
      })
      .finally(() => {
        if (!cancelled) setLoadingApartments(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    const requestId = ++loadRequestRef.current;
    if (selected?.id) {
      setEmailResults([]);
      if (loadedPeriodIdRef.current !== selected.id) {
        setLoadingSplits(true);
      }
      void Promise.all([
        ipc.getSplits(selected.id),
        ipc.getUpnDeliveryEvents(selected.id),
        ipc.getUpnPacketHashes(selected.id),
      ])
        .then(([rows, events, hashes]) => {
          if (loadRequestRef.current !== requestId) return;
          setSplits(rows);
          setDeliveryEvents(events);
          setPacketHashes(hashes);
          loadedPeriodIdRef.current = selected.id;
          setPageMessage(null);
        })
        .catch((error) => {
          if (loadRequestRef.current === requestId) {
            setPageMessage(String(error));
            setSplits([]);
            setDeliveryEvents([]);
            setPacketHashes([]);
          }
        })
        .finally(() => {
          if (loadRequestRef.current === requestId) setLoadingSplits(false);
        });
    } else {
      loadedPeriodIdRef.current = null;
      setSplits([]);
      setDeliveryEvents([]);
      setPacketHashes([]);
      setLoadingSplits(false);
    }
    return () => {
      loadRequestRef.current += 1;
    };
  }, [selected]);

  const sendEmails = async () => {
    if (!selected?.id) return;
      setPageMessage(null);
      setSending(true);
      try {
      const results = await sendPeriodEmails(selected.id);
      setEmailResults(results);
      const [events, hashes] = await Promise.all([
        ipc.getUpnDeliveryEvents(selected.id),
        ipc.getUpnPacketHashes(selected.id),
      ]);
      setDeliveryEvents(events);
      setPacketHashes(hashes);
    } catch (e) {
      setPageMessage(String(e));
    } finally {
      setSending(false);
    }
  };

  const downloadAll = async () => {
    if (!selected?.id) return;
      setDownloading(true);
      try {
      const result = await downloadPeriodUpnPdfs(selected.id);
      if (!result) return;
      setPageMessage(`Saved ${result.count} PDF(s) to ${result.folder}`);
    } catch (e) {
      setPageMessage(String(e));
    } finally {
      setDownloading(false);
    }
  };

  const apartmentConfigById = new Map<number, Apartment>(
    apartmentsConfig.flatMap((apartment) =>
      apartment.id === null ? [] : [[apartment.id, apartment]],
    ),
  );
  const historyResultsByApartmentId = aggregateHistoryEvents(
    deliveryEvents,
    apartmentConfigById,
    new Map(packetHashes.map((hash) => [hash.apartment_id, hash])),
  );
  const runResultsByApartmentId = new Map(
    emailResults.map((result) => [result.apartment_id, result]),
  );
  const byApartment = new Map<number, { label: string; unitCode: string; contactEmail: string; splits: SplitRow[] }>();
  for (const s of splits) {
    if (!byApartment.has(s.apartment_id)) {
      byApartment.set(s.apartment_id, {
        label: s.apartment_label,
        unitCode: s.apartment_unit_code,
        contactEmail: apartmentConfigById.get(s.apartment_id)?.contact_email ?? "",
        splits: [],
      });
    }
    byApartment.get(s.apartment_id)!.splits.push(s);
  }
  const apartments = [...byApartment.entries()].sort((a, b) =>
    a[1].label.localeCompare(b[1].label)
  );
  const totalSlipCount = splits.length;
  const readyRecipientCount = apartments.filter(([, apartment]) =>
    hasSendableRecipient(apartment.contactEmail),
  ).length;
  const missingRecipientCount = Math.max(0, apartments.length - readyRecipientCount);
  const selectedPeriodId = selected?.id ?? null;
  const splitsLoadedForSelected = loadedPeriodIdRef.current === selectedPeriodId;
  const splitsLoadPending = selectedPeriodId !== null && !splitsLoadedForSelected;
  const apartmentsLoadPending = !loadedApartmentsRef.current;
  const loadingUpnData =
    loadingSplits || loadingApartments || splitsLoadPending || apartmentsLoadPending;
  const showUpnLoading =
    selectedPeriodId !== null &&
    loadingUpnData &&
    (splits.length > 0 || apartments.length > 0 || snapshot.selectedStatus.splits);
  const showUpnSettling =
    selectedPeriodId !== null && loadingUpnData && !showUpnLoading;
  const showUpnTable =
    selectedPeriodId !== null && !loadingUpnData && splitsLoadedForSelected && apartments.length > 0;

  return (
    <BillingPageShell
      title="UPN Preview"
      subtitle={null}
      actions={
        <>
          <Button
            variant="outline"
            onClick={downloadAll}
            disabled={!selected || showUpnLoading || splits.length === 0 || downloading}
          >
            <Download className="size-4 mr-2" />
            {downloading ? "Saving..." : "Download All PDFs"}
          </Button>
          <Button
            onClick={sendEmails}
            disabled={!selected || showUpnLoading || splits.length === 0 || sending}
          >
            <Mail className="size-4 mr-2" />
            {sending ? "Sending..." : "Send All Emails"}
          </Button>
        </>
      }
    >
      {pageMessage && (
        <div className="rounded-md border border-danger/30 bg-danger-soft px-4 py-3 text-sm text-danger">
          {pageMessage}
        </div>
      )}

      {showUpnTable && (
        <div className="flex flex-wrap items-center gap-2 rounded-lg border border-border bg-card px-4 py-3 text-sm shadow-card">
          <span className="inline-flex items-center gap-2 rounded-md bg-success-soft px-3 py-1 text-xs font-semibold text-success">
            <CheckCircle2 className="size-3.5" />
            {readyRecipientCount} ready
            <span className="font-normal text-muted-foreground">recipient on file</span>
          </span>
          {missingRecipientCount > 0 && (
            <span className="inline-flex items-center gap-2 rounded-md bg-warning-soft px-3 py-1 text-xs font-semibold text-warning">
              <AlertTriangle className="size-3.5" />
              {missingRecipientCount} missing email
            </span>
          )}
          <span className="inline-flex items-center gap-2 rounded-md bg-surface-3 px-3 py-1 text-xs font-semibold text-muted-foreground">
            <Files className="size-3.5" />
            {totalSlipCount} slips
            <span className="font-normal">across {apartments.length} packets</span>
          </span>
        </div>
      )}

      {!selected && (
        <p className="text-muted-foreground text-sm">
          Select a billing period to view UPN forms.
        </p>
      )}

      {selected &&
        (showUpnLoading ||
          showUpnSettling ||
          (splitsLoadedForSelected && splits.length === 0)) && (
        <div className="rounded-lg border border-dashed border-border px-4 py-5 text-sm text-muted-foreground min-h-[132px] flex items-center justify-center">
          {showUpnLoading ? (
            <div className="flex items-center gap-2">
              <Loader2 className="size-4 animate-spin" />
              Loading UPN data...
            </div>
          ) : showUpnSettling ? (
            <div aria-busy="true" className="min-h-[1px]" />
          ) : (
            <div className="flex flex-wrap items-center justify-between gap-3 w-full">
              <span>No splits found. Go to Splits and click Recalculate first.</span>
              <Link
                to="/splits"
                className={buttonVariants()}
              >
                Go to Splits
              </Link>
            </div>
          )}
        </div>
      )}

      {showUpnTable && (
        <div className="overflow-hidden rounded-lg border border-border bg-card shadow-card">
          <table className="w-full text-sm">
            <thead>
              <tr className="bg-surface-2 text-left text-xs font-medium text-muted-foreground">
                <th className="px-3 pt-3.5 pb-2.5">Apartment</th>
                <th className="px-3 pt-3.5 pb-2.5">Recipient</th>
                <th className="px-3 pt-3.5 pb-2.5 text-right">Status</th>
                <th className="px-3 pt-3.5 pb-2.5 text-right">Total</th>
                <th className="w-32 px-3 pt-3.5 pb-2.5"></th>
              </tr>
            </thead>
            <tbody>
              {apartments.map(([aptId, { label, unitCode, contactEmail, splits: aptSplits }]) => (
                <ApartmentRows
                  key={aptId}
                  billingPeriodId={selected!.id!}
                  apartmentId={aptId}
                  apartmentLabel={label}
                  apartmentUnitCode={unitCode}
                  contactEmail={contactEmail}
                  splits={aptSplits}
                  emailResult={
                    runResultsByApartmentId.get(aptId) ??
                    historyResultsByApartmentId.get(aptId)
                  }
                  expanded={expandedApartmentId === aptId}
                  onToggle={() =>
                    setExpandedApartmentId((current) =>
                      current === aptId ? null : aptId,
                    )
                  }
                  onPreviewError={setPageMessage}
                />
              ))}
            </tbody>
            <tfoot>
              <tr className="bg-surface-2 font-semibold">
                <td colSpan={3} className="px-3 py-2 text-xs text-muted-foreground">
                  {apartments.length} packet{apartments.length === 1 ? "" : "s"}
                </td>
                <td className="px-3 py-2 text-right font-mono">
                  {formatEur(
                    splits.reduce((sum, split) => sum + split.split_amount_cents, 0),
                  )} €
                </td>
                <td />
              </tr>
            </tfoot>
          </table>
        </div>
      )}

      {emailResults.length > 0 && (
        <div className="rounded-lg border border-border bg-card p-4 shadow-card">
          <h3 className="font-semibold mb-3 text-sm">Email Results</h3>
          <div className="flex flex-col gap-1.5">
            {emailResults.map((r, i) => (
              <div key={i} className="flex items-center gap-2 text-sm">
                {r.status === "sent" ? (
                  <CheckCircle2 className="size-4 shrink-0 text-success" />
                ) : (
                  <XCircle
                    className={`size-4 shrink-0 ${
                      r.status === "failed" ? "text-danger" : "text-warning"
                    }`}
                  />
                )}
                <span className="font-medium">{r.apartment_label}</span>
                <span className="text-muted-foreground">{statusLabel(r.status)}</span>
                <span className="text-muted-foreground">{r.email}</span>
                {r.error && (
                  <span className="text-xs text-danger">{r.error}</span>
                )}
              </div>
            ))}
          </div>
        </div>
      )}
    </BillingPageShell>
  );
}
