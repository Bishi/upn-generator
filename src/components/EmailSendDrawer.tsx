import { useEffect, useRef } from "react";
import { createPortal } from "react-dom";
import { AlertTriangle, Mail, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { formatEur } from "@/lib/types";

export interface EmailPacketSummary {
  apartmentId: number;
  label: string;
  recipients: string[];
  slipCount: number;
  totalCents: number;
}

export function EmailSendDrawer({
  periodLabel,
  packets,
  warningCount,
  includeOriginalBills,
  originalFileCount,
  missingOriginalLabels,
  canSend,
  onClose,
  onConfirm,
}: {
  periodLabel: string;
  packets: EmailPacketSummary[];
  warningCount: number;
  includeOriginalBills: boolean;
  originalFileCount: number;
  missingOriginalLabels: string[];
  canSend: boolean;
  onClose: () => void;
  onConfirm: () => void;
}) {
  const dialogRef = useRef<HTMLDialogElement>(null);

  useEffect(() => {
    const dialog = dialogRef.current;
    dialog?.showModal();
    return () => dialog?.close();
  }, []);

  const slipCount = packets.reduce((sum, packet) => sum + packet.slipCount, 0);

  return createPortal(
    <dialog
      ref={dialogRef}
      aria-labelledby="email-send-title"
      aria-describedby="email-send-description"
      onCancel={(event) => {
        event.preventDefault();
        onClose();
      }}
      className="fixed inset-0 m-0 h-dvh max-h-none w-screen max-w-none border-0 bg-transparent p-0 text-foreground backdrop:bg-overlay"
    >
      <button
        type="button"
        tabIndex={-1}
        aria-label="Close email summary"
        onClick={onClose}
        className="absolute inset-0"
      />
      <div className="absolute right-0 top-0 flex h-full w-full max-w-full flex-col border-l border-border bg-card shadow-pop animate-drawer-slide-in sm:w-[600px]">
        <header className="border-b border-border px-5 py-4">
          <div className="flex items-center justify-between gap-4">
            <h2 id="email-send-title" className="font-head text-lg font-semibold">Send all emails</h2>
            <Button variant="ghost" size="icon" onClick={onClose} aria-label="Close email summary">
              <X className="size-4" />
            </Button>
          </div>
          <p id="email-send-description" className="mt-2 text-sm text-muted-foreground">
            Review the UPN packets and recipients before sending.
          </p>
        </header>

        <div className="min-h-0 flex-1 space-y-5 overflow-y-auto px-5 py-5">
          <div className="rounded-md border border-border bg-surface-2 p-4">
            <div className="text-xs text-muted-foreground">Billing month</div>
            <div className="mt-1 text-lg font-semibold">{periodLabel}</div>
            <p className="mt-2 text-sm text-muted-foreground">
              {packets.length} apartment packet{packets.length === 1 ? "" : "s"} · {slipCount} UPN slip{slipCount === 1 ? "" : "s"}
            </p>
          </div>

          <section aria-label="Attachments" className="rounded-md border border-border p-4 text-sm">
            <h3 className="font-semibold">Attachments</h3>
            <p className="mt-2 text-muted-foreground">
              {includeOriginalBills
                ? `UPN slips and available original bills. ${originalFileCount} original file${originalFileCount === 1 ? "" : "s"} available across the batch. Full PDFs/images are attached once per email for the apartment's bills.`
                : "UPN slips only. Original bills will not be attached."}
            </p>
            {includeOriginalBills && missingOriginalLabels.length > 0 && (
              <p className="mt-2 text-warning">Original unavailable for: {missingOriginalLabels.join(", ")}. UPN slips and any available originals will still be sent.</p>
            )}
          </section>

          <section aria-label="Packets and recipients" className="space-y-3">
            {packets.map((packet) => (
              <div key={packet.apartmentId} className="rounded-md border border-border p-4">
                <div className="flex items-baseline justify-between gap-3">
                  <h3 className="font-semibold">{packet.label}</h3>
                  <span className="shrink-0 font-mono text-sm">{formatEur(packet.totalCents)} €</span>
                </div>
                <p className="mt-1 text-xs text-muted-foreground">
                  {packet.slipCount} UPN slip{packet.slipCount === 1 ? "" : "s"} in one PDF packet
                </p>
                <ul className="mt-3 space-y-1 text-sm">
                  {packet.recipients.map((recipient, index) => (
                    <li key={`${recipient}-${index}`} className="break-all">{recipient}</li>
                  ))}
                </ul>
              </div>
            ))}
          </section>

          <div className="flex gap-3 rounded-md border border-warning/30 bg-warning-soft p-4 text-sm text-warning">
            <AlertTriangle className="mt-0.5 size-4 shrink-0" />
            <div className="space-y-2">
              <p>All listed packets will be sent, including any previously emailed packets. Sent emails cannot be recalled.</p>
              {warningCount > 0 && (
                <p>There {warningCount === 1 ? "is" : "are"} {warningCount} validation warning{warningCount === 1 ? "" : "s"} to review before sending.</p>
              )}
            </div>
          </div>
          {!canSend && (
            <p role="status" className="text-sm text-danger">Sending is unavailable. Close this summary and review the billing month's validation issues.</p>
          )}
        </div>

        <footer className="flex justify-end gap-2 border-t border-border px-5 py-4">
          <Button variant="outline" onClick={onClose} autoFocus>Cancel</Button>
          <Button onClick={onConfirm} disabled={!canSend}>
            <Mail className="size-4" />
            Confirm send
          </Button>
        </footer>
      </div>
    </dialog>,
    document.body,
  );
}
