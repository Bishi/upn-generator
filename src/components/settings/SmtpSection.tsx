import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { confirm } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useState } from "react";
import { CheckCircle2, Loader2, Save, Eye, EyeOff, Send, ShieldCheck, X } from "lucide-react";
import { ipc } from "@/lib/ipc";
import { useWorkflowSnapshotContext } from "@/lib/workflow-snapshot";
import type { SmtpConfig } from "@/lib/types";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Card, CardContent } from "@/components/ui/card";
import { SettingsLoadingCard } from "@/components/settings/SettingsLoadingCard";
import { SETTINGS_PANEL_WIDTH } from "@/components/settings/layout";
import type { SettingsDirtyRegistrar } from "@/components/settings/dirty-state";

const emptyConfig: SmtpConfig = {
  host: "",
  port: 587,
  username: "",
  from_email: "",
  use_tls: true,
  allowlist_enabled: true,
  recipient_allowlist: "",
  password_configured: false,
};

function sameSmtpConfig(left: SmtpConfig | undefined, right: SmtpConfig) {
  return (
    !!left &&
    left.host === right.host &&
    left.port === right.port &&
    left.username === right.username &&
    left.from_email === right.from_email &&
    left.use_tls === right.use_tls &&
    left.allowlist_enabled === right.allowlist_enabled &&
    left.recipient_allowlist === right.recipient_allowlist
  );
}

export function SmtpSection({
  onDirtyEntry,
}: {
  onDirtyEntry?: SettingsDirtyRegistrar;
}) {
  const queryClient = useQueryClient();
  const snapshot = useWorkflowSnapshotContext();
  const { data, isLoading } = useQuery({
    queryKey: ["smtp_config"],
    queryFn: ipc.getSmtpConfig,
  });

  const [form, setForm] = useState<SmtpConfig>(emptyConfig);
  const [password, setPassword] = useState("");
  const [showPassword, setShowPassword] = useState(false);
  const [testRecipient, setTestRecipient] = useState("");
  const [testStatus, setTestStatus] = useState<string | null>(null);

  useEffect(() => {
    if (data) setForm(data);
  }, [data]);

  const mutation = useMutation({
    mutationFn: async (cfg: SmtpConfig) => {
      await ipc.saveSmtpConfig(cfg);
      if (password) await ipc.saveSmtpPassword(password);
    },
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["smtp_config"] });
      await snapshot.refresh({ core: false, periods: false, selected: true, statuses: true });
      setPassword("");
    },
  });

  const testMutation = useMutation({
    mutationFn: () => ipc.testSmtpConnection(form, password, testRecipient),
    onSuccess: () => {
      setTestStatus("Test email sent.");
    },
    onError: (error) => {
      setTestStatus(String(error));
    },
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!isDirty || mutation.isPending) return;
    mutation.mutate(form);
  };

  const isDirty = !sameSmtpConfig(data, form) || password.trim().length > 0;

  const discardChanges = useCallback(() => {
    setForm(data ?? emptyConfig);
    setPassword("");
    setTestStatus(null);
  }, [data]);

  useEffect(() => {
    if (!onDirtyEntry) return undefined;
    return onDirtyEntry("smtp", {
      tab: "delivery",
      label: "Email settings",
      isDirty: !isLoading && isDirty,
      isBusy: mutation.isPending,
      discard: discardChanges,
    });
  }, [discardChanges, isDirty, isLoading, mutation.isPending, onDirtyEntry]);

  const handleDiscard = async () => {
    if (!isDirty || mutation.isPending) return;
    const confirmed = await confirm(
      "Discard unsaved email settings changes?",
      {
        title: "Discard Unsaved Changes",
        kind: "warning",
        okLabel: "Discard changes",
        cancelLabel: "Keep editing",
      },
    );
    if (confirmed) discardChanges();
  };

  if (isLoading) return <SettingsLoadingCard rows={5} />;

  return (
    <Card className={`${SETTINGS_PANEL_WIDTH} overflow-hidden`}>
      <div className="border-b border-border px-5 py-4">
        <h3 className="font-head text-lg font-semibold">Email (SMTP) Settings</h3>
        <p className="mt-1 text-sm text-muted-foreground">
          Used to send UPN PDFs to apartment tenants. The password is stored in
          Windows Credential Manager and excluded from manual backups.
        </p>
      </div>
      <CardContent className="p-5">
        <form onSubmit={handleSubmit} className="space-y-4">
          <div className="grid grid-cols-3 gap-3">
            <div className="col-span-2 space-y-1.5">
              <Label htmlFor="host">SMTP host</Label>
              <Input
                id="host"
                value={form.host}
                onChange={(e) => setForm({ ...form, host: e.target.value })}
                placeholder="smtp.gmail.com"
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="port">Port</Label>
              <Input
                id="port"
                type="number"
                value={form.port}
                onChange={(e) => setForm({ ...form, port: parseInt(e.target.value, 10) || 587 })}
              />
            </div>
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="username">Username</Label>
            <Input
              id="username"
              value={form.username}
              onChange={(e) => setForm({ ...form, username: e.target.value })}
              placeholder="you@gmail.com"
            />
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="from_email">From address</Label>
            <Input
              id="from_email"
              type="email"
              value={form.from_email}
              onChange={(e) => setForm({ ...form, from_email: e.target.value })}
              placeholder="building@example.com"
            />
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="password">Password</Label>
            <div className="relative">
              <Input
                id="password"
                type={showPassword ? "text" : "password"}
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                placeholder={form.password_configured ? "Password saved" : "SMTP password"}
                className="pr-10"
              />
              <button
                type="button"
                onClick={() => setShowPassword(!showPassword)}
                className="absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
              >
                {showPassword ? <EyeOff className="size-4" /> : <Eye className="size-4" />}
              </button>
            </div>
            <p className="text-xs text-muted-foreground">
              Leave blank to keep the existing password. The saved password is never loaded back into this form.
            </p>
          </div>
          <div className="flex flex-wrap items-center gap-3">
            <label className="flex items-center gap-2 text-sm">
              <input
                type="checkbox"
                id="use_tls"
                checked={form.use_tls}
                onChange={(e) => setForm({ ...form, use_tls: e.target.checked })}
                className="size-4 accent-primary"
              />
              Use STARTTLS/TLS
            </label>
            {form.password_configured && !password && (
              <span className="inline-flex items-center gap-1.5 rounded-md bg-success-soft px-2 py-1 text-xs font-semibold text-success">
                <CheckCircle2 className="size-3.5" />
                Password saved
              </span>
            )}
          </div>
          <div className="rounded-md border border-border bg-surface-2 p-4">
            <div className="mb-3 flex items-start gap-2">
              <ShieldCheck className="mt-0.5 size-4 text-success" />
              <div>
                <h4 className="text-sm font-semibold">Email safety</h4>
                <p className="mt-1 text-xs text-muted-foreground">
                  When enabled, UPN emails are sent only to listed test recipients.
                  Other recipients block the email send before any delivery event is recorded.
                </p>
              </div>
            </div>
            <div className="space-y-3">
              <div className="flex items-center gap-2">
                <input
                  type="checkbox"
                  id="allowlist_enabled"
                  checked={form.allowlist_enabled}
                  onChange={(e) =>
                    setForm({ ...form, allowlist_enabled: e.target.checked })
                  }
                  className="size-4 accent-primary"
                />
                <Label htmlFor="allowlist_enabled">Enable recipient allowlist</Label>
              </div>
              <div className="space-y-1.5">
                <Label htmlFor="recipient_allowlist">Allowed test recipients</Label>
                <Input
                  id="recipient_allowlist"
                  value={form.recipient_allowlist}
                  onChange={(e) =>
                    setForm({ ...form, recipient_allowlist: e.target.value })
                  }
                  placeholder="you@gmail.com, tester@example.com"
                />
                <p className="text-xs text-muted-foreground">
                  Empty list blocks all UPN email sends while the allowlist is enabled.
                </p>
              </div>
            </div>
          </div>
          <div className="rounded-md border border-border bg-card p-4">
            <div className="grid grid-cols-[minmax(0,1fr)_auto] items-end gap-3">
              <div className="space-y-1.5">
                <Label htmlFor="test_recipient">Test recipient</Label>
                <Input
                  id="test_recipient"
                  type="email"
                  value={testRecipient}
                  onChange={(e) => setTestRecipient(e.target.value)}
                  placeholder="you@gmail.com"
                />
              </div>
              <Button
                type="button"
                variant="outline"
                onClick={() => testMutation.mutate()}
                disabled={testMutation.isPending}
                className="gap-2"
              >
                {testMutation.isPending ? (
                  <Loader2 className="size-4 animate-spin" />
                ) : (
                  <Send className="size-4" />
                )}
                Test Email
              </Button>
            </div>
            {testStatus && (
              <p
                className={`mt-2 text-xs ${
                  testMutation.isError ? "text-danger" : "text-success"
                }`}
              >
                {testStatus}
              </p>
            )}
          </div>
          <div className="flex flex-wrap gap-2">
            <Button
              type="submit"
              disabled={!isDirty || mutation.isPending}
              className={mutation.isPending ? "gap-2 disabled:opacity-100" : "gap-2"}
            >
              {mutation.isPending ? (
                <Loader2 className="size-4 animate-spin" />
              ) : (
                <Save className="size-4" />
              )}
              Save changes
            </Button>
            <Button
              type="button"
              variant="outline"
              disabled={!isDirty || mutation.isPending}
              onClick={handleDiscard}
              className="gap-2"
            >
              <X className="size-4" />
              Discard
            </Button>
          </div>
        </form>
      </CardContent>
    </Card>
  );
}
