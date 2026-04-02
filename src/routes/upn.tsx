import { createFileRoute, Link } from "@tanstack/react-router";
import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useRef, useState } from "react";
import { Mail, Download, Eye, CheckCircle2, XCircle, Loader2, Files } from "lucide-react";
import { ipc } from "@/lib/ipc";
import { useBillingPeriodSelection } from "@/lib/billing-period-selection";
import type { EmailResult, SplitRow } from "@/lib/types";
import { formatEur } from "@/lib/types";
import { BillingPageShell } from "@/components/BillingPageShell";
import { Button } from "@/components/ui/button";

export const Route = createFileRoute("/upn")({
  component: UpnPage,
});

function ApartmentCard({
  billingPeriodId,
  apartmentId,
  apartmentLabel,
  splits,
  emailResult,
  onPreviewError,
}: {
  billingPeriodId: number;
  apartmentId: number;
  apartmentLabel: string;
  splits: SplitRow[];
  emailResult?: EmailResult;
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

  return (
    <div className="rounded-lg border border-border bg-card p-4 flex flex-col gap-3">
      <div className="flex items-start justify-between gap-3">
        <div>
          <h3 className="font-semibold">{apartmentLabel}</h3>
          <p className="text-xs text-muted-foreground">
            {splits.length} UPN{splits.length === 1 ? "" : "s"} in one apartment packet
          </p>
        </div>
        <div className="flex flex-col items-end gap-2">
          <Button
            variant="outline"
            size="sm"
            onClick={previewAll}
            disabled={previewingAll || splits.length === 0}
          >
            {previewingAll ? (
              <Loader2 className="size-3.5 animate-spin" />
            ) : (
              <Files className="size-3.5" />
            )}
            Preview All
          </Button>
          {emailResult && (
            <span
              className={`flex items-center gap-1 text-xs ${
                emailResult.success ? "text-green-600" : "text-destructive"
              }`}
            >
              {emailResult.success ? (
                <CheckCircle2 className="size-3.5" />
              ) : (
                <XCircle className="size-3.5" />
              )}
              {emailResult.success ? "Sent" : emailResult.error ?? "Failed"}
            </span>
          )}
        </div>
      </div>

      <div className="flex flex-col gap-1.5">
        {splits.map((s) => (
          <div
            key={s.bill_id}
            className="flex items-center justify-between text-sm"
          >
            <span className="text-muted-foreground truncate max-w-40">
              {s.provider_name ?? s.bill_source_filename}
            </span>
            <div className="flex items-center gap-2">
              <span className="font-mono font-medium">
                {formatEur(s.split_amount_cents)} EUR
              </span>
              <Button
                variant="ghost"
                size="sm"
                onClick={() => previewUpn(s.bill_id)}
                disabled={loadingPreview === s.bill_id}
                title="Preview UPN"
              >
                {loadingPreview === s.bill_id ? (
                  <Loader2 className="size-3.5 animate-spin" />
                ) : (
                  <Eye className="size-3.5" />
                )}
                Preview
              </Button>
            </div>
          </div>
        ))}
      </div>

      <div className="border-t border-border pt-2 flex justify-between text-sm font-semibold">
        <span>Total</span>
        <span className="font-mono">{formatEur(total)} EUR</span>
      </div>
    </div>
  );
}

function UpnPage() {
  const [splits, setSplits] = useState<SplitRow[]>([]);
  const [loadingSplits, setLoadingSplits] = useState(false);
  const [sending, setSending] = useState(false);
  const [downloading, setDownloading] = useState(false);
  const [emailResults, setEmailResults] = useState<EmailResult[]>([]);
  const [pageMessage, setPageMessage] = useState<string | null>(null);
  const loadRequestRef = useRef(0);
  const {
    years,
    yearPeriods,
    selectedYear,
    selected,
    setSelectedYear,
    setSelected,
  } = useBillingPeriodSelection();

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
      const results = await ipc.sendEmails(selected.id);
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
      const folder = await open({ directory: true, title: "Choose folder to save UPN PDFs" });
      if (!folder || typeof folder !== "string") return;
      const saved = await ipc.saveAllUpns(selected.id, folder);
      setPageMessage(`Saved ${saved.length} PDF(s) to ${folder}`);
    } catch (e) {
      setPageMessage(String(e));
    } finally {
      setDownloading(false);
    }
  };

  const byApartment = new Map<number, { label: string; splits: SplitRow[] }>();
  for (const s of splits) {
    if (!byApartment.has(s.apartment_id)) {
      byApartment.set(s.apartment_id, { label: s.apartment_label, splits: [] });
    }
    byApartment.get(s.apartment_id)!.splits.push(s);
  }
  const apartments = [...byApartment.entries()].sort((a, b) =>
    a[1].label.localeCompare(b[1].label)
  );

  return (
    <BillingPageShell
      title="UPN Preview"
      subtitle={null}
      years={years}
      selectedYear={selectedYear}
      onSelectYear={setSelectedYear}
      yearPeriods={yearPeriods}
      selected={selected}
      onSelectPeriod={(period) => {
        setSelected(period);
        setSplits([]);
        setEmailResults([]);
      }}
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
        <div className="rounded-md bg-destructive/10 border border-destructive/30 px-4 py-3 text-sm text-destructive">
          {pageMessage}
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
                className="inline-flex h-9 items-center justify-center rounded-md border border-input bg-background px-4 text-sm font-medium shadow-sm hover:bg-accent hover:text-accent-foreground"
              >
                Go to Splits
              </Link>
            </div>
          )}
        </div>
      )}

      {!loadingSplits && apartments.length > 0 && (
        <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4">
          {apartments.map(([aptId, { label, splits: aptSplits }]) => (
            <ApartmentCard
              key={aptId}
              billingPeriodId={selected!.id!}
              apartmentId={aptId}
              apartmentLabel={label}
              splits={aptSplits}
              emailResult={emailResults.find((r) => r.apartment_label === label)}
              onPreviewError={setPageMessage}
            />
          ))}
        </div>
      )}

      {emailResults.length > 0 && (
        <div className="rounded-lg border border-border p-4">
          <h3 className="font-semibold mb-3 text-sm">Email Results</h3>
          <div className="flex flex-col gap-1.5">
            {emailResults.map((r, i) => (
              <div key={i} className="flex items-center gap-2 text-sm">
                {r.success ? (
                  <CheckCircle2 className="size-4 text-green-600 shrink-0" />
                ) : (
                  <XCircle className="size-4 text-destructive shrink-0" />
                )}
                <span className="font-medium">{r.apartment_label}</span>
                <span className="text-muted-foreground">{r.email}</span>
                {r.error && (
                  <span className="text-destructive text-xs">{r.error}</span>
                )}
              </div>
            ))}
          </div>
        </div>
      )}
    </BillingPageShell>
  );
}
