import { useEffect, useRef, useState } from "react";
import { Drawer } from "@/components/ui/drawer";
import { AlertTriangle, Loader2, Mail, Paperclip, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { ipc } from "@/lib/ipc";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import type { EmailContent, ExtraEmailAttachment } from "@/lib/types";
import { formatEur } from "@/lib/types";

export interface EmailPacketSummary {
  apartmentId: number;
  label: string;
  recipients: string[];
  slipCount: number;
  totalCents: number;
}

export function EmailSendDrawer({
  billingPeriodId,
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
  billingPeriodId: number;
  periodLabel: string;
  packets: EmailPacketSummary[];
  warningCount: number;
  includeOriginalBills: boolean;
  originalFileCount: number;
  missingOriginalLabels: string[];
  canSend: boolean;
  onClose: () => void;
  onConfirm: (content: EmailContent, extraAttachments: ExtraEmailAttachment[]) => void;
}) {
  const [extraFiles, setExtraFiles] = useState<{ attachment: ExtraEmailAttachment; size: number }[]>([]);
  const [readingFiles, setReadingFiles] = useState(false);
  const [attachmentError, setAttachmentError] = useState<string | null>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const attachmentRequestRef = useRef(0);
  const readingFilesRef = useRef(false);
  useEffect(() => () => { attachmentRequestRef.current += 1; }, []);

  const addFiles = async (files: File[]) => {
    if (!files.length || readingFilesRef.current) return;
    const totalSize = extraFiles.reduce((sum, file) => sum + file.size, 0) + files.reduce((sum, file) => sum + file.size, 0);
    if (extraFiles.length + files.length > 10 || totalSize > 15 * 1024 * 1024) {
      setAttachmentError("Choose at most 10 additional files, totaling at most 15 MiB. This selection was not added.");
      return;
    }
    const request = ++attachmentRequestRef.current;
    readingFilesRef.current = true;
    setReadingFiles(true);
    setAttachmentError(null);
    try {
      const added: { attachment: ExtraEmailAttachment; size: number }[] = [];
      for (const file of files) {
        const bytes = new Uint8Array(await file.arrayBuffer());
        if (request !== attachmentRequestRef.current) return;
        const chunks = [];
        for (let offset = 0; offset < bytes.length; offset += 32768) {
          chunks.push(String.fromCharCode(...bytes.subarray(offset, offset + 32768)));
        }
        added.push({
          attachment: { name: file.name, content_type: file.type || "application/octet-stream", data_base64: btoa(chunks.join("")) },
          size: bytes.length,
        });
      }
      if (request === attachmentRequestRef.current) setExtraFiles((current) => [...current, ...added]);
    } catch (error) {
      if (request === attachmentRequestRef.current) setAttachmentError(`Could not read the selected files. This selection was not added. ${String(error)}`);
    } finally {
      if (request === attachmentRequestRef.current) {
        readingFilesRef.current = false;
        setReadingFiles(false);
      }
    }
  };

  const [content, setContent] = useState<EmailContent | null>(null);
  const [contentError, setContentError] = useState<string | null>(null);
  useEffect(() => {
    let active = true;
    setContent(null);
    setContentError(null);
    ipc.previewEmailContent(billingPeriodId, includeOriginalBills).then(
      (value) => { if (active) setContent(value); },
      (error) => { if (active) setContentError(String(error)); },
    );
    return () => { active = false; };
  }, [billingPeriodId, includeOriginalBills]);
  const validContent = content != null && content.subject.trim().length > 0 && !/[\r\n\0]/.test(content.subject) && !content.body.includes("\0");
  const slipCount = packets.reduce((sum, packet) => sum + packet.slipCount, 0);

  return (
    <Drawer
      open
      onClose={onClose}
      title="Send all emails"
      description="Review the UPN packets and recipients before sending."
      width={720}
      closeLabel="Close email summary"
      bodyClassName="space-y-5 py-5"
      footerClassName="justify-end gap-2"
      footer={
        <>
          <Button variant="outline" onClick={onClose} data-drawer-autofocus>Cancel</Button>
          <Button onClick={() => { if (content && validContent && !readingFilesRef.current) onConfirm(content, extraFiles.map((file) => file.attachment)); }} disabled={!canSend || !validContent || readingFiles}>
            <Mail className="size-4" />
            Confirm send
          </Button>
        </>
      }
    >
          <div className="rounded-md border border-border bg-surface-2 p-4">
            <div className="text-xs text-muted-foreground">Billing month</div>
            <div className="mt-1 text-lg font-semibold">{periodLabel}</div>
            <p className="mt-2 text-sm text-muted-foreground">
              {packets.length} apartment packet{packets.length === 1 ? "" : "s"} · {slipCount} UPN slip{slipCount === 1 ? "" : "s"}
            </p>
          </div>

          <section aria-label="Email content" className="space-y-3">
            <h3 className="font-semibold">Email content</h3>
            {contentError ? <p role="alert" className="text-sm text-danger">{contentError} Close and reopen this drawer to retry.</p> : !content ? <p role="status" className="text-sm text-muted-foreground">Loading email content...</p> : <>
              <div className="space-y-1.5">
                <Label htmlFor="batch-email-subject">Subject</Label>
                <Input id="batch-email-subject" required maxLength={998} value={content.subject} onChange={(e) => setContent({ ...content, subject: e.target.value })} />
              </div>
              <div className="space-y-1.5">
                <Label htmlFor="batch-email-body">Body</Label>
                <textarea id="batch-email-body" rows={8} maxLength={100000} value={content.body} onChange={(e) => setContent({ ...content, body: e.target.value })} className="w-full rounded-md border border-input bg-white px-3 py-2 text-sm text-slate-900 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring" />
              </div>
              {!validContent && <p role="alert" className="text-sm text-danger">Enter a subject on one line.</p>}
            </>}
          </section>

          <section aria-label="Attachments" className="rounded-md border border-border p-4 text-sm">
            <h3 className="font-semibold">Attachments</h3>
            <p className="mt-2 text-muted-foreground">
              {includeOriginalBills
                ? `UPN slips + available original bills (${originalFileCount} file${originalFileCount === 1 ? "" : "s"}).`
                : "UPN slips only."}
            </p>
            {includeOriginalBills && missingOriginalLabels.length > 0 && (
              <p className="mt-2 text-warning">Originals unavailable: {missingOriginalLabels.join(", ")}.</p>
            )}
          </section>

          <section aria-label="Additional attachments" className="space-y-3 rounded-md border border-border p-4 text-sm">
            <h3 className="font-semibold">Additional attachments</h3>
            <p className="text-xs text-muted-foreground">Sent to everyone in this batch. Up to 10 files, 15 MiB total.</p>
            <input ref={fileInputRef} type="file" multiple className="hidden" aria-label="Choose additional attachments" onChange={(event) => {
              const files = Array.from(event.currentTarget.files ?? []);
              event.currentTarget.value = "";
              void addFiles(files);
            }} />
            <Button type="button" variant="outline" disabled={readingFiles || extraFiles.length >= 10} onClick={() => fileInputRef.current?.click()}>
              {readingFiles ? <Loader2 className="size-4 animate-spin" /> : <Paperclip className="size-4" />}
              {readingFiles ? "Reading files..." : "Add attachments"}
            </Button>
            {extraFiles.length > 0 && <ul className="space-y-2">
              {extraFiles.map((file, index) => <li key={index} className="flex min-w-0 items-center gap-3">
                <div className="min-w-0 flex-1">
                  <p className="break-all">{file.attachment.name}</p>
                  <p className="text-xs text-muted-foreground">{(file.size / 1024).toFixed(1)} KiB</p>
                </div>
                <Button type="button" variant="ghost" size="icon" disabled={readingFiles} aria-label={`Remove attachment ${file.attachment.name}`} onClick={() => { setExtraFiles((current) => current.filter((_, i) => i !== index)); setAttachmentError(null); }}><X className="size-4" /></Button>
              </li>)}
            </ul>}
            {attachmentError && <p role="alert" className="text-danger">{attachmentError}</p>}
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
    </Drawer>
  );
}
