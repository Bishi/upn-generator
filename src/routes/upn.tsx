import { createFileRoute } from "@tanstack/react-router";
import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";
import { Mail, Download, Eye, CheckCircle2, XCircle, Loader2 } from "lucide-react";
import { ipc } from "@/lib/ipc";
import type { BillingPeriod, EmailResult, SplitRow } from "@/lib/types";
import { formatEur, MONTHS } from "@/lib/types";
import { Button } from "@/components/ui/button";

export const Route = createFileRoute("/upn")({
  component: UpnPage,
});

// ─── Period tabs ──────────────────────────────────────────────────────────

function PeriodTabs({
  periods,
  selected,
  onSelect,
}: {
  periods: BillingPeriod[];
  selected: BillingPeriod | null;
  onSelect: (p: BillingPeriod) => void;
}) {
  return (
    <div className="flex gap-1 flex-wrap">
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
    </div>
  );
}


// ─── Apartment card ───────────────────────────────────────────────────────

function ApartmentCard({
  apartmentId,
  apartmentLabel,
  splits,
  emailResult,
}: {
  apartmentId: number;
  apartmentLabel: string;
  splits: SplitRow[];
  emailResult?: EmailResult;
}) {
  const [loadingPreview, setLoadingPreview] = useState<number | null>(null);

  const previewUpn = async (billId: number) => {
    setLoadingPreview(billId);
    try {
      await ipc.previewUpn(billId, apartmentId);
    } finally {
      setLoadingPreview(null);
    }
  };

  const total = splits.reduce((s, r) => s + r.split_amount_cents, 0);

  return (
    <>
      <div className="rounded-lg border border-border bg-card p-4 flex flex-col gap-3">
        <div className="flex items-center justify-between">
          <h3 className="font-semibold">{apartmentLabel}</h3>
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
                  {formatEur(s.split_amount_cents)} €
                </span>
                <button
                  onClick={() => previewUpn(s.bill_id)}
                  disabled={loadingPreview === s.bill_id}
                  className="text-muted-foreground hover:text-primary transition-colors"
                  title="Preview UPN"
                >
                  {loadingPreview === s.bill_id ? (
                    <Loader2 className="size-3.5 animate-spin" />
                  ) : (
                    <Eye className="size-3.5" />
                  )}
                </button>
              </div>
            </div>
          ))}
        </div>

        <div className="border-t border-border pt-2 flex justify-between text-sm font-semibold">
          <span>Total</span>
          <span className="font-mono">{formatEur(total)} €</span>
        </div>
      </div>

    </>
  );
}

// ─── Main page ─────────────────────────────────────────────────────────────

function UpnPage() {
  const [periods, setPeriods] = useState<BillingPeriod[]>([]);
  const [selected, setSelected] = useState<BillingPeriod | null>(null);
  const [splits, setSplits] = useState<SplitRow[]>([]);
  const [sending, setSending] = useState(false);
  const [downloading, setDownloading] = useState(false);
  const [emailResults, setEmailResults] = useState<EmailResult[]>([]);
  const [sendError, setSendError] = useState<string | null>(null);

  const loadPeriods = async () => {
    const ps = await ipc.getBillingPeriods();
    setPeriods(ps);
    if (!selected && ps.length > 0) setSelected(ps[0]);
  };

  const loadSplits = async (id: number) => {
    const rows = await ipc.getSplits(id);
    setSplits(rows);
  };

  useEffect(() => { loadPeriods(); }, []);
  useEffect(() => {
    if (selected?.id) {
      setEmailResults([]);
      loadSplits(selected.id);
    }
  }, [selected]);

  const sendEmails = async () => {
    if (!selected?.id) return;
    setSendError(null);
    setSending(true);
    try {
      const results = await ipc.sendEmails(selected.id);
      setEmailResults(results);
    } catch (e) {
      setSendError(String(e));
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
      alert(`Saved ${saved.length} PDF(s) to ${folder}`);
    } catch (e) {
      setSendError(String(e));
    } finally {
      setDownloading(false);
    }
  };

  // Group splits by apartment
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
    <div className="flex flex-col gap-6">
      <div className="flex items-center justify-between flex-wrap gap-2">
        <h2 className="text-2xl font-bold">UPN Preview</h2>
        <div className="flex gap-2">
          <Button
            variant="outline"
            onClick={downloadAll}
            disabled={!selected || splits.length === 0 || downloading}
          >
            <Download className="size-4 mr-2" />
            {downloading ? "Saving…" : "Download All PDFs"}
          </Button>
          <Button
            onClick={sendEmails}
            disabled={!selected || splits.length === 0 || sending}
          >
            <Mail className="size-4 mr-2" />
            {sending ? "Sending…" : "Send Emails"}
          </Button>
        </div>
      </div>

      <PeriodTabs
        periods={periods}
        selected={selected}
        onSelect={(p) => { setSelected(p); setSplits([]); setEmailResults([]); }}
      />

      {sendError && (
        <div className="rounded-md bg-destructive/10 border border-destructive/30 px-4 py-3 text-sm text-destructive">
          {sendError}
        </div>
      )}

      {!selected && (
        <p className="text-muted-foreground text-sm">
          Select a billing period to view UPN forms.
        </p>
      )}

      {selected && splits.length === 0 && (
        <p className="text-muted-foreground text-sm">
          No splits found. Go to Splits and click Recalculate first.
        </p>
      )}

      {apartments.length > 0 && (
        <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4">
          {apartments.map(([aptId, { label, splits: aptSplits }]) => (
            <ApartmentCard
              key={aptId}
              apartmentId={aptId}
              apartmentLabel={label}
              splits={aptSplits}
              emailResult={emailResults.find((r) => r.apartment_label === label)}
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
    </div>
  );
}
