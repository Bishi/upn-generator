import { createFileRoute, Link } from "@tanstack/react-router";
import { confirm } from "@tauri-apps/plugin-dialog";
import { Fragment, useCallback, useEffect, useRef, useState } from "react";
import { toast } from "sonner";
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
  RotateCcw,
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

type DeliveryRowStatus = EmailResult["status"] | "saved" | "delivered";
type DeliveryRowResult = Omit<EmailResult, "status"> & {
  status: DeliveryRowStatus;
  delivery_type: "email" | "pdf" | "manual" | null;
};

function statusLabel(status: DeliveryRowStatus) {
  switch (status) {
    case "sent":
      return "sent";
    case "saved":
      return "pdf saved";
    case "delivered":
      return "delivered";
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
  if (status === "sent" || status === "delivered") return "bg-success-soft text-success";
  if (status === "saved") return "bg-warning-soft text-warning";
  if (status === "blocked") return "bg-warning-soft text-warning";
  if (status === "partial") return "bg-warning-soft text-warning";
  if (status === "changed") return "bg-warning-soft text-warning";
  return "bg-danger-soft text-danger";
}

function summarizeEmailResults(results: EmailResult[]) {
  const counts = results.reduce(
    (summary, result) => {
      if (result.status === "sent") summary.sent += 1;
      else if (result.status === "blocked") summary.blocked += 1;
      else if (result.status === "partial") summary.partial += 1;
      else summary.failed += 1;
      return summary;
    },
    { sent: 0, blocked: 0, failed: 0, partial: 0 },
  );
  const issues = counts.blocked + counts.failed + counts.partial;
  const details = [
    counts.sent > 0 ? `${counts.sent} sent` : null,
    counts.partial > 0 ? `${counts.partial} partial` : null,
    counts.blocked > 0 ? `${counts.blocked} blocked` : null,
    counts.failed > 0 ? `${counts.failed} failed` : null,
  ].filter(Boolean);

  return {
    hasIssues: issues > 0,
    title: issues > 0 ? "Email send completed with issues" : "Emails sent",
    description: details.length > 0 ? details.join(", ") : `${results.length} processed`,
  };
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
    const containsManual = rows.some((event) => event.delivery_type === "manual");
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
          : containsManual
            ? "Current UPN packet changed since this manual delivery confirmation."
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
      success: displayStatus === "sent" || displayStatus === "delivered",
      error: errors.length > 0 ? errors.join("; ") : null,
      delivery_type: containsManual
        ? "manual"
        : containsPdf && !containsEmail
          ? "pdf"
          : "email",
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
    const isManualOnly = row.manual_delivered && !row.email_sent;
    return {
      apartment_id: row.apartment_id,
      apartment_label: label,
      email,
      status: isManualOnly ? "delivered" : "sent",
      recipient: "",
      original_recipient: "",
      success: true,
      error:
        row.current_failed_event_count > 0 || row.current_blocked_event_count > 0
          ? `${row.current_failed_event_count + row.current_blocked_event_count} current delivery warning(s)`
          : null,
      delivery_type: isManualOnly ? "manual" : "email",
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
              {deliveryResult.status === "sent" || deliveryResult.status === "delivered" ? (
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
  const [markingDelivered, setMarkingDelivered] = useState(false);
  const [emailResults, setEmailResults] = useState<EmailResult[]>([]);
  const [deliveryEvents, setDeliveryEvents] = useState<UpnDeliveryEvent[]>([]);
  const [packetHashes, setPacketHashes] = useState<UpnPacketHash[]>([]);
  const loadRequestRef = useRef(0);
  const lastSnapshotErrorRef = useRef<string | null>(null);
  const [expandedApartmentId, setExpandedApartmentId] = useState<number | null>(null);

  const setPageError = useCallback((message: string | null) => {
    if (!message) return;
    toast.error("UPN action failed", {
      id: "upn-action-error",
      description: message,
      duration: Infinity,
    });
  }, []);

  const loadDeliveryState = useCallback(
    async (
      billingPeriodId: number,
      requestId?: number,
    ) => {
      const [events, hashes] = await Promise.all([
        ipc.getUpnDeliveryEvents(billingPeriodId),
        ipc.getUpnPacketHashes(billingPeriodId),
      ]);
      if (requestId != null && loadRequestRef.current !== requestId) return;
      setDeliveryEvents(events);
      setPacketHashes(hashes);
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
            setPageError(String(error));
            setDeliveryEvents([]);
            setPacketHashes([]);
          }
        });
    } else {
      setDeliveryEvents([]);
      setPacketHashes([]);
    }
    return () => {
      loadRequestRef.current += 1;
    };
  }, [loadDeliveryState, selected?.id]);

  useEffect(() => {
    if (!snapshot.error) {
      lastSnapshotErrorRef.current = null;
      return;
    }
    if (lastSnapshotErrorRef.current === snapshot.error) return;
    lastSnapshotErrorRef.current = snapshot.error;
    toast.error("Workflow refresh failed", {
      id: "upn-workflow-error",
      description: snapshot.error,
      duration: Infinity,
    });
  }, [snapshot.error]);

  const sendEmails = async () => {
    if (!selected?.id) return;
    setSending(true);
    try {
      const results = await sendPeriodEmails(selected.id);
      setEmailResults(results);
      const summary = summarizeEmailResults(results);
      if (summary.hasIssues) {
        toast.warning(summary.title, { description: summary.description });
      } else {
        toast.success(summary.title, { description: summary.description });
      }
      await loadDeliveryState(selected.id);
      await snapshot.refresh({ periods: false, core: false, selected: true, statuses: true });
    } catch (e) {
      setPageError(String(e));
      await loadDeliveryState(selected.id).catch(() => undefined);
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
      toast.success("PDFs saved", {
        description: `${result.count} packet${result.count === 1 ? "" : "s"} exported.`,
      });
    } catch (e) {
      setPageError(String(e));
    } finally {
      setDownloading(false);
    }
  };

  const markDelivered = async () => {
    if (!selected?.id) return;
    const confirmed = await confirm(
      "Mark this billing period as delivered? This will count all current UPN packets as delivered.",
      {
        title: "Mark Delivered",
        kind: "warning",
        okLabel: "Mark Delivered",
        cancelLabel: "Cancel",
      },
    );
    if (!confirmed) return;

    setMarkingDelivered(true);
    try {
      const rollup = await ipc.markUpnPeriodDelivered(selected.id);
      setEmailResults([]);
      await loadDeliveryState(selected.id);
      await snapshot.refresh({ periods: false, core: false, selected: true, statuses: true });
      toast.success("Marked delivered", {
        description: `${rollup.packet_count} packet${rollup.packet_count === 1 ? "" : "s"} updated.`,
      });
    } catch (e) {
      setPageError(String(e));
      await loadDeliveryState(selected.id).catch(() => undefined);
      await snapshot
        .refresh({ periods: false, core: false, selected: true, statuses: true })
        .catch(() => undefined);
    } finally {
      setMarkingDelivered(false);
    }
  };

  const unmarkDelivered = async () => {
    if (!selected?.id) return;
    const confirmed = await confirm(
      "Remove manual delivery marks for this billing period? Email delivery history will stay unchanged.",
      {
        title: "Unmark Delivered",
        kind: "warning",
        okLabel: "Unmark Delivered",
        cancelLabel: "Cancel",
      },
    );
    if (!confirmed) return;

    setMarkingDelivered(true);
    try {
      await ipc.unmarkUpnPeriodDelivered(selected.id);
      setEmailResults([]);
      await loadDeliveryState(selected.id);
      await snapshot.refresh({ periods: false, core: false, selected: true, statuses: true });
      toast.success("Manual delivery marks removed", {
        description: "Email delivery history was kept.",
      });
    } catch (e) {
      setPageError(String(e));
      await loadDeliveryState(selected.id).catch(() => undefined);
      await snapshot
        .refresh({ periods: false, core: false, selected: true, statuses: true })
        .catch(() => undefined);
    } finally {
      setMarkingDelivered(false);
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
    (snapshot.selectedDeliveryRollup?.apartments ?? []).flatMap((row) => {
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
  const deliveryRollup = snapshot.selectedDeliveryRollup;
  const deliveryPacketCount =
    deliveryRollup?.packet_count ??
    (snapshot.selectedStatus.packetCount > 0
      ? snapshot.selectedStatus.packetCount
      : apartments.length);
  const deliveryDeliveredCount =
    deliveryRollup?.current_delivered_count ??
    (snapshot.selectedStatus.packetCount > 0
      ? snapshot.selectedStatus.deliveredCount
      : 0);
  const deliveryComplete =
    deliveryRollup?.complete ??
    (deliveryPacketCount > 0 && snapshot.selectedStatus.sent);
  const hasManualDelivery = (deliveryRollup?.manual_delivered_count ?? 0) > 0;

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
          {hasManualDelivery ? (
            <Button
              variant="outline"
              onClick={unmarkDelivered}
              disabled={
                !selected?.id || snapshot.loading || splits.length === 0 || markingDelivered
              }
            >
              {markingDelivered ? (
                <Loader2 className="size-4 animate-spin" />
              ) : (
                <RotateCcw className="size-4" />
              )}
              Unmark Delivered
            </Button>
          ) : (
            <Button
              variant="outline"
              onClick={markDelivered}
              disabled={
                !selected?.id || snapshot.loading || splits.length === 0 || markingDelivered
              }
            >
              {markingDelivered ? (
                <Loader2 className="size-4 animate-spin" />
              ) : (
                <CheckCircle2 className="size-4" />
              )}
              Mark Delivered
            </Button>
          )}
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
          {deliveryPacketCount > 0 && (
            <SummaryChip
              className={
                deliveryComplete
                  ? "bg-success-soft text-success"
                  : "bg-surface-3 text-muted-foreground"
              }
            >
              <CheckCircle2 className="size-3.5" />
              {deliveryDeliveredCount}/{deliveryPacketCount} delivered
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
                  onPreviewError={setPageError}
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
