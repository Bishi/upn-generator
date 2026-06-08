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
import type { Apartment, EmailResult, SplitRow } from "@/lib/types";
import { formatEur } from "@/lib/types";
import { BillingPageShell } from "@/components/BillingPageShell";
import { Button } from "@/components/ui/button";
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
      <tr className="border-b border-border hover:bg-accent/10">
        <td className="px-3 py-3">
          <button
            type="button"
            onClick={onToggle}
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
              className={`inline-flex items-center gap-1.5 rounded-md px-2 py-1 text-xs font-semibold ${
                emailResult.success
                  ? "bg-success-soft text-success"
                  : "bg-danger-soft text-danger"
              }`}
            >
              {emailResult.success ? (
                <CheckCircle2 className="size-3" />
              ) : (
                <XCircle className="size-3" />
              )}
              {emailResult.success ? "sent" : "failed"}
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
          {formatEur(total)} EUR
        </td>
        <td className="px-3 py-3">
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
              {formatEur(split.split_amount_cents)} EUR
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
  const [splits, setSplits] = useState<SplitRow[]>([]);
  const [apartmentsConfig, setApartmentsConfig] = useState<Apartment[]>([]);
  const [loadingSplits, setLoadingSplits] = useState(false);
  const [sending, setSending] = useState(false);
  const [downloading, setDownloading] = useState(false);
  const [emailResults, setEmailResults] = useState<EmailResult[]>([]);
  const [pageMessage, setPageMessage] = useState<string | null>(null);
  const loadRequestRef = useRef(0);
  const { selected } = useBillingPeriodSelection();
  const [expandedApartmentId, setExpandedApartmentId] = useState<number | null>(null);

  useEffect(() => {
    void ipc.getApartments().then(setApartmentsConfig);
  }, []);

  useEffect(() => {
    const requestId = ++loadRequestRef.current;
    if (selected?.id) {
      setEmailResults([]);
      setSplits([]);
      setLoadingSplits(true);
      void ipc.getSplits(selected.id).then((rows) => {
        if (loadRequestRef.current !== requestId) return;
        setSplits(rows);
        setLoadingSplits(false);
      });
    } else {
      setSplits([]);
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

  const apartmentConfigById = new Map(
    apartmentsConfig.map((apartment) => [apartment.id, apartment]),
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

  return (
    <BillingPageShell
      title="UPN Preview"
      subtitle={null}
      actions={
        <>
          <Button
            variant="outline"
            onClick={downloadAll}
            disabled={!selected || splits.length === 0 || downloading}
          >
            <Download className="size-4 mr-2" />
            {downloading ? "Saving..." : "Download All PDFs"}
          </Button>
          <Button
            onClick={sendEmails}
            disabled={!selected || splits.length === 0 || sending}
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

      {!loadingSplits && apartments.length > 0 && (
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

      {selected && splits.length === 0 && (
        <div className="rounded-lg border border-dashed border-border px-4 py-5 text-sm text-muted-foreground min-h-[132px] flex items-center justify-center">
          {loadingSplits ? (
            <div className="text-center">
              <div className="text-sm font-medium text-foreground">Loading UPN data...</div>
              <div className="text-sm text-muted-foreground">
                Preparing apartment packets for this period.
              </div>
            </div>
          ) : (
            <div className="flex flex-wrap items-center justify-between gap-3 w-full">
              <span>No splits found. Go to Splits and click Recalculate first.</span>
              <Link
                to="/splits"
                className="inline-flex h-9 items-center justify-center rounded-md border border-input bg-card px-4 text-sm font-medium shadow-card hover:bg-accent hover:text-accent-foreground"
              >
                Go to Splits
              </Link>
            </div>
          )}
        </div>
      )}

      {!loadingSplits && apartments.length > 0 && (
        <div className="overflow-hidden rounded-lg border border-border bg-card shadow-card">
          <table className="w-full text-sm">
            <thead>
              <tr className="bg-surface-2 text-left text-xs font-medium text-muted-foreground">
                <th className="px-3 py-2">Apartment</th>
                <th className="px-3 py-2">Recipient</th>
                <th className="px-3 py-2 text-right">Status</th>
                <th className="px-3 py-2 text-right">Total</th>
                <th className="w-32 px-3 py-2"></th>
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
                  emailResult={emailResults.find((r) => r.apartment_label === label)}
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
                  )} EUR
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
                {r.success ? (
                  <CheckCircle2 className="size-4 shrink-0 text-success" />
                ) : (
                  <XCircle className="size-4 shrink-0 text-danger" />
                )}
                <span className="font-medium">{r.apartment_label}</span>
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
