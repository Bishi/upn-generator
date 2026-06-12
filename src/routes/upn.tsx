import { createFileRoute, Link } from "@tanstack/react-router";
import { Fragment, useCallback, useEffect, useRef, useState } from "react";
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
  UpnDeliveryApartmentRollup,
  UpnDeliveryEvent,
  UpnDeliveryRollup,
  UpnPacketHash,
} from "@/lib/types";
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
  billingTableTallCellClass,
} from "@/components/BillingTable";
import { Button, buttonVariants } from "@/components/ui/button";
import { downloadPeriodUpnPdfs, sendPeriodEmails } from "@/lib/upn-actions";
import { cn } from "@/lib/utils";

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

type DeliveryRowStatus = EmailResult["status"] | "saved";
type DeliveryRowResult = Omit<EmailResult, "status"> & {
  status: DeliveryRowStatus;
  delivery_type: "email" | "pdf" | null;
};

function statusLabel(status: DeliveryRowStatus) {
  switch (status) {
    case "sent":
      return "sent";
    case "saved":
      return "pdf saved";
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

function statusClass(status: DeliveryRowStatus) {
  if (status === "sent" || status === "saved") return "bg-success-soft text-success";
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
): Map<number, DeliveryRowResult> {
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

  const result = new Map<number, DeliveryRowResult>();
  for (const [apartmentId, attemptId] of latestAttemptByApartment) {
    const rows = events.filter(
      (event) => event.apartment_id === apartmentId && event.attempt_id === attemptId,
    );
    const statuses = new Set(rows.map((event) => event.status));
    const status: DeliveryRowStatus =
      statuses.size === 1
        ? (rows[0]?.status as DeliveryRowStatus)
        : "partial";
    const apartment = apartmentsById.get(apartmentId) ?? null;
    const containsEmail = rows.some((event) => event.delivery_type === "email");
    const containsPdf = rows.some((event) => event.delivery_type === "pdf");
    const currentRecipients = normalizedRecipients(apartment?.contact_email ?? "");
    const historicalRecipients = rows
      .filter((event) => event.delivery_type === "email")
      .map((event) => event.recipient.trim().toLowerCase())
      .filter(Boolean)
      .sort();
    const currentPacketHash = packetHashesByApartmentId.get(apartmentId);
    const historicalHashes = [...new Set(rows.map((event) => event.attachment_sha256))].filter(
      Boolean,
    );
    const matchesPacket =
      Boolean(currentPacketHash?.attachment_sha256) &&
      historicalHashes.length === 1 &&
      historicalHashes[0] === currentPacketHash?.attachment_sha256;
    const matchesRecipients =
      !containsEmail || sameRecipients(currentRecipients, historicalRecipients);
    const isCurrent = matchesRecipients && matchesPacket;
    const displayStatus: DeliveryRowStatus = isCurrent ? status : "changed";
    const errors = rows
      .map((event) => event.error)
      .filter(Boolean)
      .filter((error, index, all) => all.indexOf(error) === index);
    if (!isCurrent) {
      errors.unshift(
        currentPacketHash?.error
          ? `Current packet could not be checked: ${currentPacketHash.error}`
          : containsPdf && !containsEmail
            ? "Current UPN packet changed since this PDF save."
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
      success: displayStatus === "sent" || displayStatus === "saved",
      error: errors.length > 0 ? errors.join("; ") : null,
      delivery_type: containsPdf && !containsEmail ? "pdf" : "email",
    });
  }

  return result;
}

function rollupToRowResult(
  row: UpnDeliveryApartmentRollup,
  apartment: Apartment | null,
): DeliveryRowResult | undefined {
  const label = apartment?.label ?? row.apartment_label;
  const email = apartment?.contact_email ?? "";

  if (row.delivered) {
    const isPdfOnly = row.pdf_saved && !row.email_sent;
    return {
      apartment_id: row.apartment_id,
      apartment_label: label,
      email,
      status: isPdfOnly ? "saved" : "sent",
      recipient: "",
      original_recipient: "",
      success: true,
      error:
        row.current_failed_event_count > 0 || row.current_blocked_event_count > 0
          ? `${row.current_failed_event_count + row.current_blocked_event_count} current delivery warning(s)`
          : null,
      delivery_type: isPdfOnly ? "pdf" : "email",
    };
  }

  if (row.packet_error.trim()) {
    return {
      apartment_id: row.apartment_id,
      apartment_label: label,
      email,
      status: "failed",
      recipient: "",
      original_recipient: "",
      success: false,
      error: row.packet_error,
      delivery_type: null,
    };
  }

  if (row.current_failed_event_count > 0 || row.current_blocked_event_count > 0) {
    const status =
      row.current_failed_event_count > 0 && row.current_blocked_event_count > 0
        ? "partial"
        : row.current_blocked_event_count > 0
          ? "blocked"
          : "failed";
    return {
      apartment_id: row.apartment_id,
      apartment_label: label,
      email,
      status,
      recipient: "",
      original_recipient: "",
      success: false,
      error: `${row.current_failed_event_count + row.current_blocked_event_count} current delivery issue(s)`,
      delivery_type: row.last_current_delivery_type,
    };
  }

  return undefined;
}

function ApartmentRows({
  billingPeriodId,
  apartmentId,
  apartmentLabel,
  apartmentUnitCode,
  contactEmail,
  splits,
  deliveryResult,
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
  deliveryResult?: DeliveryRowResult;
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
        className={cn(billingTableBodyRowClass, "cursor-pointer")}
        onClick={onToggle}
      >
        <td className={billingTableTallCellClass}>
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
        <td className={billingTableTallCellClass}>
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
        <td className={`${billingTableTallCellClass} text-right`}>
          {deliveryResult ? (
            <span
              className={`inline-flex items-center gap-1.5 rounded-md px-2 py-1 text-xs font-semibold ${statusClass(
                deliveryResult.status,
              )}`}
            >
              {deliveryResult.status === "sent" || deliveryResult.status === "saved" ? (
                <CheckCircle2 className="size-3" />
              ) : (
                <XCircle className="size-3" />
              )}
              {statusLabel(deliveryResult.status)}
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
        <td className={`${billingTableTallCellClass} text-right font-mono font-semibold`}>
          {formatEur(total)} €
        </td>
        <td className={billingTableTallCellClass} onClick={(event) => event.stopPropagation()}>
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
            <td className={`${billingTableNumericCellClass} text-sm text-muted-foreground`}>
              {formatEur(split.split_amount_cents)} €
            </td>
            <td className={billingTableCellClass}>
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
  const splits = snapshot.splits;
  const apartmentsConfig = snapshot.apartments;
  const [sending, setSending] = useState(false);
  const [downloading, setDownloading] = useState(false);
  const [emailResults, setEmailResults] = useState<EmailResult[]>([]);
  const [deliveryEvents, setDeliveryEvents] = useState<UpnDeliveryEvent[]>([]);
  const [deliveryRollup, setDeliveryRollup] = useState<UpnDeliveryRollup | null>(null);
  const [packetHashes, setPacketHashes] = useState<UpnPacketHash[]>([]);
  const [pageMessage, setPageMessage] = useState<string | null>(null);
  const loadRequestRef = useRef(0);
  const [expandedApartmentId, setExpandedApartmentId] = useState<number | null>(null);

  const loadDeliveryState = useCallback(
    async (
      billingPeriodId: number,
      requestId?: number,
      clearPageMessage = true,
    ) => {
      const [events, hashes, rollup] = await Promise.all([
        ipc.getUpnDeliveryEvents(billingPeriodId),
        ipc.getUpnPacketHashes(billingPeriodId),
        ipc.getUpnDeliveryRollup(billingPeriodId),
      ]);
      if (requestId != null && loadRequestRef.current !== requestId) return;
      setDeliveryEvents(events);
      setPacketHashes(hashes);
      setDeliveryRollup(rollup);
      if (clearPageMessage) setPageMessage(null);
    },
    [],
  );

  useEffect(() => {
    const requestId = ++loadRequestRef.current;
    if (selected?.id) {
      setEmailResults([]);
      void loadDeliveryState(selected.id, requestId)
        .catch((error) => {
          if (loadRequestRef.current === requestId) {
            setPageMessage(String(error));
            setDeliveryEvents([]);
            setPacketHashes([]);
            setDeliveryRollup(null);
          }
        });
    } else {
      setDeliveryEvents([]);
      setPacketHashes([]);
      setDeliveryRollup(null);
    }
    return () => {
      loadRequestRef.current += 1;
    };
  }, [loadDeliveryState, selected?.id]);

  const sendEmails = async () => {
    if (!selected?.id) return;
    setPageMessage(null);
    setSending(true);
    try {
      const results = await sendPeriodEmails(selected.id);
      setEmailResults(results);
      await loadDeliveryState(selected.id);
      await snapshot.refresh({ periods: false, core: false, selected: true, statuses: true });
    } catch (e) {
      setPageMessage(String(e));
      await loadDeliveryState(selected.id, undefined, false).catch(() => undefined);
      await snapshot
        .refresh({ periods: false, core: false, selected: true, statuses: true })
        .catch(() => undefined);
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
      await loadDeliveryState(selected.id);
      await snapshot.refresh({ periods: false, core: false, selected: true, statuses: true });
      setPageMessage(`Saved ${result.count} PDF(s) to ${result.folder}`);
    } catch (e) {
      setPageMessage(String(e));
      await loadDeliveryState(selected.id, undefined, false).catch(() => undefined);
      await snapshot
        .refresh({ periods: false, core: false, selected: true, statuses: true })
        .catch(() => undefined);
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
  const rollupResultsByApartmentId = new Map(
    (deliveryRollup?.apartments ?? []).flatMap((row) => {
      const result = rollupToRowResult(
        row,
        apartmentConfigById.get(row.apartment_id) ?? null,
      );
      return result ? [[row.apartment_id, result] as const] : [];
    }),
  );
  const runResultsByApartmentId = new Map(
    emailResults.map(
      (result) =>
        [
          result.apartment_id,
          {
            ...result,
            delivery_type: "email" as const,
          },
        ] as const,
    ),
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
  const workflowError = pageMessage ?? snapshot.error;
  const selectedStatusKnown =
    selectedPeriodId !== null && snapshot.periodStatuses.has(selectedPeriodId);
  const showUpnLoading =
    selectedPeriodId !== null &&
    snapshot.loading &&
    (splits.length > 0 || apartments.length > 0 || snapshot.selectedStatus.splits);
  const showUpnSettling =
    selectedPeriodId !== null &&
    snapshot.loading &&
    !showUpnLoading &&
    selectedStatusKnown;
  const showUpnTable =
    selectedPeriodId !== null && !snapshot.loading && apartments.length > 0;

  return (
    <BillingPageShell
      title="UPN Preview"
      subtitle={null}
      actions={
        <>
          <Button
            variant="outline"
            onClick={downloadAll}
            disabled={!selected?.id || snapshot.loading || splits.length === 0 || downloading}
          >
            {downloading ? (
              <Loader2 className="size-4 animate-spin" />
            ) : (
              <Download className="size-4" />
            )}
            Download All PDFs
          </Button>
          <Button
            onClick={sendEmails}
            disabled={!selected?.id || snapshot.loading || splits.length === 0 || sending}
          >
            {sending ? (
              <Loader2 className="size-4 animate-spin" />
            ) : (
              <Mail className="size-4" />
            )}
            Send All Emails
          </Button>
        </>
      }
    >
      {workflowError && (
        <div className="rounded-md border border-danger/30 bg-danger-soft px-4 py-3 text-sm text-danger">
          {workflowError}
        </div>
      )}

      {showUpnTable && (
        <SummaryStrip>
          <SummaryChip className="bg-success-soft text-success">
            <CheckCircle2 className="size-3.5" />
            {readyRecipientCount} ready
            <span className="font-normal text-muted-foreground">recipient on file</span>
          </SummaryChip>
          {missingRecipientCount > 0 && (
            <SummaryChip className="bg-warning-soft text-warning">
              <AlertTriangle className="size-3.5" />
              {missingRecipientCount} missing email
            </SummaryChip>
          )}
          <SummaryChip className="bg-surface-3 text-muted-foreground">
            <Files className="size-3.5" />
            {totalSlipCount} slips
            <span className="font-normal">across {apartments.length} packets</span>
          </SummaryChip>
          {deliveryRollup && deliveryRollup.packet_count > 0 && (
            <SummaryChip
              className={
                deliveryRollup.complete
                  ? "bg-success-soft text-success"
                  : "bg-surface-3 text-muted-foreground"
              }
            >
              <CheckCircle2 className="size-3.5" />
              {deliveryRollup.current_delivered_count}/{deliveryRollup.packet_count} delivered
            </SummaryChip>
          )}
        </SummaryStrip>
      )}

      {!selected && (
        <p className="text-muted-foreground text-sm">
          Select a billing period to view UPN forms.
        </p>
      )}

      {selected &&
        (showUpnLoading ||
          showUpnSettling ||
          (!snapshot.loading && splits.length === 0)) && (
        <BillingTableFrame minHeight>
          <BillingEmptyState
            loading={showUpnLoading}
            loadingLabel="Loading UPN data..."
            title="No UPNs yet for this billing month"
            detail="Calculate splits first, then return here to preview and send UPN forms."
            action={
              <Link
                to="/splits"
                className={buttonVariants({ variant: "outline" })}
              >
                Go to Splits
              </Link>
            }
          />
        </BillingTableFrame>
      )}

      {showUpnTable && (
        <BillingTableFrame>
          <BillingTable>
            <thead>
              <BillingTableHeaderRow>
                <BillingTableHeaderCell>Apartment</BillingTableHeaderCell>
                <BillingTableHeaderCell>Recipient</BillingTableHeaderCell>
                <BillingTableHeaderCell className="text-right">Status</BillingTableHeaderCell>
                <BillingTableHeaderCell className="text-right">Total</BillingTableHeaderCell>
                <BillingTableHeaderCell className="w-32" />
              </BillingTableHeaderRow>
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
                  deliveryResult={
                    runResultsByApartmentId.get(aptId) ??
                    rollupResultsByApartmentId.get(aptId) ??
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
              <BillingTableFooterRow>
                <td colSpan={3} className={`${billingTableCellClass} text-xs text-muted-foreground`}>
                  {apartments.length} packet{apartments.length === 1 ? "" : "s"}
                </td>
                <td className={billingTableNumericCellClass}>
                  {formatEur(
                    splits.reduce((sum, split) => sum + split.split_amount_cents, 0),
                  )} €
                </td>
                <td />
              </BillingTableFooterRow>
            </tfoot>
          </BillingTable>
        </BillingTableFrame>
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
