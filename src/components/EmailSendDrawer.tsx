import { useEffect, useRef, useState } from "react";
import { Drawer } from "@/components/ui/drawer";
import { AlertTriangle, Loader2, Mail, Paperclip, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { ipc } from "@/lib/ipc";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import type { EmailContent, ExtraEmailAttachment } from "@/lib/types";
import { formatClosedAt, formatEur } from "@/lib/types";

export interface EmailPacketSummary {
  apartmentId: number;
  label: string;
  recipients: string[];
  slipCount: number;
  totalCents: number;
  /** Timestamp of the latest successful email send for this packet, if any. */
  lastEmailedAt: string | null;
}

function plural(count: number, noun: string, pluralNoun = `${noun}s`) {
  return `${count} ${count === 1 ? noun : pluralNoun}`;
}


export function EmailSendDrawer({
  billingPeriodId,
  periodLabel,
  packets,
  warningCount,
  includeOriginalBills,
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
  const recipientCount = packets.reduce((sum, packet) => sum + packet.recipients.length, 0);
  const previouslyEmailed = packets.filter((packet) => packet.lastEmailedAt != null);
  const sendLabel = `Send ${plural(packets.length, "email")}`;
  const attachmentSummary = includeOriginalBills ? "UPN slips + original bills" : "UPN slips only";
  const sameSendTime = previouslyEmailed.length > 0 && previouslyEmailed.every((packet) => packet.lastEmailedAt === previouslyEmailed[0].lastEmailedAt);
  const resendNotice = previouslyEmailed.length === 0
    ? null
    : previouslyEmailed.length === packets.length && sameSendTime
      ? `All ${packets.length} were already emailed on ${formatClosedAt(previouslyEmailed[0].lastEmailedAt)} and will be sent again.`
      : `Already emailed and will be sent again: ${previouslyEmailed.map((packet) => `${packet.label} (${formatClosedAt(packet.lastEmailedAt)})`).join(", ")}.`;

  return (
    <Drawer
      open
      onClose={onClose}
      title="Review and send emails"
      description={`Sends ${plural(packets.length, "email")} to ${plural(recipientCount, "address", "addresses")} for ${periodLabel} with ${attachmentSummary.toLowerCase()}. Nothing is sent until you press ${sendLabel}.`}
      width={720}
      closeLabel="Close email summary"
      bodyClassName="space-y-5 py-5"
      footerClassName="justify-end gap-2"
      footer={
        <>
          <Button variant="outline" onClick={onClose} data-drawer-autofocus>Cancel</Button>
          <Button onClick={() => { if (content && validContent && !readingFilesRef.current) onConfirm(content, extraFiles.map((file) => file.attachment)); }} disabled={!canSend || !validContent || readingFiles}>
            <Mail className="size-4" />
            {sendLabel}
          </Button>
        </>
      }
    >
          {(resendNotice || warningCount > 0) && (
            <div className="flex gap-3 rounded-md border border-warning/30 bg-warning-soft p-4 text-sm text-warning">
              <AlertTriangle className="mt-0.5 size-4 shrink-0" />
              <div className="space-y-2">
                {resendNotice && <p>{resendNotice}</p>}
                {warningCount > 0 && (
                  <p>There {warningCount === 1 ? "is" : "are"} {plural(warningCount, "validation warning")} to review before sending.</p>
                )}
              </div>
            </div>
          )}
          {!canSend && (
            <p role="status" className="text-sm text-danger">Sending is unavailable. Close this summary and review the billing month's validation issues.</p>
          )}

          <section aria-label="Email content" className="space-y-3">
            {contentError ? <p role="alert" className="text-sm text-danger">{contentError} Close and reopen this drawer to retry.</p> : !content ? <p role="status" className="text-sm text-muted-foreground">Loading email content...</p> : <>
              <div className="space-y-1.5">
                <Label htmlFor="batch-email-subject">Subject</Label>
                <Input id="batch-email-subject" required maxLength={998} value={content.subject} onChange={(e) => setContent({ ...content, subject: e.target.value })} />
              </div>
              <div className="space-y-1.5">
                <Label htmlFor="batch-email-body">Body</Label>
                <textarea id="batch-email-body" rows={3} maxLength={100000} value={content.body} onChange={(e) => setContent({ ...content, body: e.target.value })} className="field-sizing-content w-full resize-none overflow-hidden rounded-md border border-input bg-white px-3 py-2 text-sm text-slate-900 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring" />
              </div>
              {!validContent && <p role="alert" className="text-sm text-danger">Enter a subject on one line.</p>}
            </>}
          </section>

          <section aria-label="Attachments" className="space-y-3 rounded-md border border-border p-4 text-sm">
            <div className="flex flex-wrap items-center justify-between gap-3">
              <div>
                <h3 className="font-semibold">Attachments</h3>
                <p className="text-muted-foreground">
                  {attachmentSummary}
                  {includeOriginalBills && missingOriginalLabels.length > 0 && ` · originals unavailable: ${missingOriginalLabels.join(", ")}`}
                </p>
              </div>
              <input ref={fileInputRef} type="file" multiple className="hidden" aria-label="Choose additional attachments" onChange={(event) => {
                const files = Array.from(event.currentTarget.files ?? []);
                event.currentTarget.value = "";
                void addFiles(files);
              }} />
              <Button type="button" variant="outline" disabled={readingFiles || extraFiles.length >= 10} onClick={() => fileInputRef.current?.click()}>
                {readingFiles ? <Loader2 className="size-4 animate-spin" /> : <Paperclip className="size-4" />}
                {readingFiles ? "Reading files..." : "Add files for everyone"}
              </Button>
            </div>
            {extraFiles.length > 0 && <ul className="space-y-2">
              {extraFiles.map((file, index) => <li key={index} className="flex min-w-0 items-center gap-3">
                <div className="min-w-0 flex-1">
                  <p className="break-all">{file.attachment.name}</p>
                  <p className="text-xs text-muted-foreground">{(file.size / 1024).toFixed(1)} KiB · sent to everyone</p>
                </div>
                <Button type="button" variant="ghost" size="icon" disabled={readingFiles} aria-label={`Remove attachment ${file.attachment.name}`} onClick={() => { setExtraFiles((current) => current.filter((_, i) => i !== index)); setAttachmentError(null); }}><X className="size-4" /></Button>
              </li>)}
            </ul>}
            {attachmentError && <p role="alert" className="text-danger">{attachmentError}</p>}
          </section>

          <section aria-label="Recipients">
            <table className="w-full text-sm">
              <thead className="text-left text-xs text-muted-foreground">
                <tr className="border-b border-border">
                  <th scope="col" className="py-2 pr-3 font-normal">Apartment</th>
                  <th scope="col" className="py-2 pr-3 font-normal">To</th>
                  <th scope="col" className="py-2 pr-3 text-right font-normal">Amount</th>
                  <th scope="col" className="py-2 text-right font-normal">Status</th>
                </tr>
              </thead>
              <tbody>
                {packets.map((packet) => (
                  <tr key={packet.apartmentId} className="border-b border-border/60 align-top">
                    <td className="py-2 pr-3">
                      <div className="font-medium">{packet.label}</div>
                      <div className="text-xs text-muted-foreground">{plural(packet.slipCount, "slip")}</div>
                    </td>
                    <td className="py-2 pr-3">
                      {packet.recipients.map((recipient, index) => (
                        <div key={`${recipient}-${index}`} className="break-all">{recipient}</div>
                      ))}
                    </td>
                    <td className="py-2 pr-3 text-right font-mono whitespace-nowrap">{formatEur(packet.totalCents)} €</td>
                    <td className="py-2 text-right whitespace-nowrap">
                      {packet.lastEmailedAt != null
                        ? <span className="rounded-full bg-warning-soft px-2 py-0.5 text-xs text-warning">sent {formatClosedAt(packet.lastEmailedAt).slice(0, 10)}</span>
                        : <span className="text-xs text-muted-foreground">not sent</span>}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </section>
    </Drawer>
  );
}
