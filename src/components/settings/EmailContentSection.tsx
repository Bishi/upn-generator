import { useCallback, useEffect, useState } from "react";
import { confirm } from "@tauri-apps/plugin-dialog";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Save, Loader2 } from "lucide-react";
import { ipc } from "@/lib/ipc";
import type { EmailContent } from "@/lib/types";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Card, CardContent } from "@/components/ui/card";
import { SettingsLoadingCard } from "./SettingsLoadingCard";
import { SETTINGS_PANEL_WIDTH } from "./layout";
import type { SettingsDirtyRegistrar } from "./dirty-state";

export function EmailContentSection({ onDirtyEntry }: { onDirtyEntry?: SettingsDirtyRegistrar }) {
  const queryClient = useQueryClient();
  const { data, isLoading, error } = useQuery({ queryKey: ["email_content"], queryFn: ipc.getEmailContent });
  const [form, setForm] = useState<EmailContent>({ subject: "", body: "" });
  useEffect(() => { if (data) setForm(data); }, [data]);
  const mutation = useMutation({
    mutationFn: ipc.saveEmailContent,
    onSuccess: (saved) => { queryClient.setQueryData(["email_content"], saved); setForm(saved); },
  });
  const isDirty = !!data && (data.subject !== form.subject || data.body !== form.body);
  const discard = useCallback(() => { if (data) setForm(data); mutation.reset(); }, [data, mutation.reset]);
  useEffect(() => onDirtyEntry?.("email-content", {
    tab: "email-content", label: "Email content", isDirty, isBusy: mutation.isPending, discard,
  }), [onDirtyEntry, isDirty, mutation.isPending, discard]);
  const handleDiscard = async () => {
    if (!isDirty || mutation.isPending) return;
    const confirmed = await confirm("Discard unsaved email content changes?", {
      title: "Discard Unsaved Changes",
      kind: "warning",
      okLabel: "Discard changes",
      cancelLabel: "Keep editing",
    });
    if (confirmed) discard();
  };
  if (isLoading) return <SettingsLoadingCard rows={3} />;
  return (
    <Card className={`${SETTINGS_PANEL_WIDTH} overflow-hidden`}>
      <div className="border-b border-border px-5 py-4">
        <h3 className="font-head text-lg font-semibold">Default Email Content</h3>
      </div>
      <CardContent className="p-5">
        <form className="space-y-4" onSubmit={(e) => { e.preventDefault(); if (isDirty && !mutation.isPending) mutation.mutate(form); }}>
          <div className="space-y-1.5">
            <Label htmlFor="default-email-subject">Subject</Label>
            <Input id="default-email-subject" required maxLength={998} value={form.subject} disabled={!data || mutation.isPending} onChange={(e) => setForm({ ...form, subject: e.target.value })} />
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="default-email-body">Body</Label>
            <textarea id="default-email-body" rows={9} maxLength={100000} value={form.body} disabled={!data || mutation.isPending} onChange={(e) => setForm({ ...form, body: e.target.value })} className="w-full rounded-md border border-input bg-white px-3 py-2 text-sm text-slate-900 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring" />
          </div>
          <p className="text-xs text-muted-foreground">Body can be left blank.</p>
          <dl className="space-y-2 rounded-md border border-border p-3 text-xs">
            <div className="grid grid-cols-[9rem_minmax(0,1fr)] gap-3">
              <dt className="font-mono">{"{month}"}</dt>
              <dd className="text-muted-foreground">Service month, as two digits (e.g. 09). The month before the billing month.</dd>
            </div>
            <div className="grid grid-cols-[9rem_minmax(0,1fr)] gap-3">
              <dt className="font-mono">{"{year}"}</dt>
              <dd className="text-muted-foreground">Service year (e.g. 2026). January billing uses the previous year.</dd>
            </div>
            <div className="grid grid-cols-[9rem_minmax(0,1fr)] gap-3">
              <dt className="font-mono">{"{originals_note}"}</dt>
              <dd className="text-muted-foreground">Adds a body note when original bills are included and available. Otherwise left empty.</dd>
            </div>
          </dl>
          {(error || mutation.error) && <p role="alert" className="text-sm text-danger">{String(error || mutation.error)}</p>}
          {mutation.isSuccess && !isDirty && <p role="status" className="text-sm text-success">Email content saved.</p>}
          <div className="flex gap-2">
            <Button type="submit" disabled={!isDirty || mutation.isPending || !form.subject.trim()}>
              {mutation.isPending ? <Loader2 className="size-4 animate-spin" /> : <Save className="size-4" />} Save changes
            </Button>
            <Button type="button" variant="outline" disabled={!isDirty || mutation.isPending} onClick={handleDiscard}>Discard</Button>
          </div>
        </form>
      </CardContent>
    </Card>
  );
}
