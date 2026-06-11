import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { Check, CheckCircle2, Eye, EyeOff, Loader2, Save, ShieldAlert, Wifi } from "lucide-react";
import { ipc } from "@/lib/ipc";
import type { InboxConfig } from "@/lib/types";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Card, CardContent } from "@/components/ui/card";
import { SettingsLoadingCard } from "@/components/settings/SettingsLoadingCard";
import { SETTINGS_PANEL_WIDTH } from "@/components/settings/layout";

const emptyConfig: InboxConfig = {
  host: "",
  port: 993,
  username: "",
  use_tls: true,
  folder: "INBOX",
  days_to_scan: 45,
  sender_allowlist: "",
  password_configured: false,
};

function normalizeConfig(config: InboxConfig): InboxConfig {
  return {
    ...config,
    port: Number.isFinite(config.port) ? config.port : 993,
    days_to_scan: Number.isFinite(config.days_to_scan) ? config.days_to_scan : 45,
  };
}

export function InboxSection() {
  const queryClient = useQueryClient();
  const { data, isLoading } = useQuery({
    queryKey: ["inbox_config"],
    queryFn: ipc.getInboxConfig,
  });

  const [form, setForm] = useState<InboxConfig>(emptyConfig);
  const [password, setPassword] = useState("");
  const [showPassword, setShowPassword] = useState(false);
  const [saved, setSaved] = useState(false);
  const [testStatus, setTestStatus] = useState<string | null>(null);

  useEffect(() => {
    if (data) setForm(normalizeConfig(data));
  }, [data]);

  const saveMutation = useMutation({
    mutationFn: async (cfg: InboxConfig) => {
      await ipc.saveInboxConfig(cfg);
      if (password.trim()) await ipc.saveInboxPassword(password);
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["inbox_config"] });
      setPassword("");
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
    },
  });

  const testMutation = useMutation({
    mutationFn: () => ipc.testInboxConnection(form, password),
    onSuccess: () => {
      setTestStatus("Connection OK");
      queryClient.invalidateQueries({ queryKey: ["inbox_config"] });
    },
    onError: (error) => {
      setTestStatus(error instanceof Error ? error.message : String(error));
    },
  });

  const handleSubmit = (event: React.FormEvent) => {
    event.preventDefault();
    saveMutation.mutate(form);
  };

  if (isLoading) return <SettingsLoadingCard rows={6} />;

  return (
    <Card className={`${SETTINGS_PANEL_WIDTH} overflow-hidden`}>
      <div className="border-b border-border px-5 py-4">
        <h3 className="font-head text-lg font-semibold">Inbox (IMAP) Settings</h3>
        <p className="mt-1 text-sm text-muted-foreground">
          Used to read incoming bill attachments for configured providers that are still missing in the selected billing month.
          The password is stored in Windows Credential Manager.
        </p>
      </div>
      <CardContent className="p-5">
        <form onSubmit={handleSubmit} className="space-y-4">
          <div className="grid grid-cols-3 gap-3">
            <div className="col-span-2 space-y-1.5">
              <Label htmlFor="inbox_host">IMAP host</Label>
              <Input
                id="inbox_host"
                value={form.host}
                onChange={(event) => setForm({ ...form, host: event.target.value })}
                placeholder="imap.gmail.com"
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="inbox_port">Port</Label>
              <Input
                id="inbox_port"
                type="number"
                min={1}
                max={65535}
                value={form.port}
                onChange={(event) =>
                  setForm({ ...form, port: parseInt(event.target.value, 10) || 993 })
                }
              />
            </div>
          </div>

          <div className="grid grid-cols-3 gap-3">
            <div className="col-span-2 space-y-1.5">
              <Label htmlFor="inbox_username">Username</Label>
              <Input
                id="inbox_username"
                value={form.username}
                onChange={(event) => setForm({ ...form, username: event.target.value })}
                placeholder="you@gmail.com"
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="inbox_folder">Folder</Label>
              <Input
                id="inbox_folder"
                value={form.folder}
                onChange={(event) => setForm({ ...form, folder: event.target.value })}
                placeholder="INBOX"
              />
            </div>
          </div>

          <div className="grid grid-cols-3 gap-3">
            <div className="col-span-2 space-y-1.5">
              <Label htmlFor="inbox_password">Password</Label>
              <div className="relative">
                <Input
                  id="inbox_password"
                  type={showPassword ? "text" : "password"}
                  value={password}
                  onChange={(event) => setPassword(event.target.value)}
                  placeholder={
                    form.password_configured ? "Password saved" : "IMAP app password"
                  }
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
            <div className="space-y-1.5">
              <Label htmlFor="days_to_scan">Mailbox scan window</Label>
              <Input
                id="days_to_scan"
                type="number"
                min={1}
                max={90}
                value={form.days_to_scan}
                onChange={(event) =>
                  setForm({
                    ...form,
                    days_to_scan: Math.min(90, Math.max(1, parseInt(event.target.value, 10) || 45)),
                  })
                }
              />
              <p className="text-xs text-muted-foreground">
                Looks back this many days, then imports only configured missing providers for the selected billing month.
              </p>
            </div>
          </div>

          <div className="space-y-1.5">
            <Label htmlFor="sender_allowlist">Sender allowlist</Label>
            <Input
              id="sender_allowlist"
              value={form.sender_allowlist}
              onChange={(event) =>
                setForm({ ...form, sender_allowlist: event.target.value })
              }
              placeholder="billing@example.com, invoices@example.com"
            />
            <p className="text-xs text-muted-foreground">
              Optional comma-separated email addresses. Blank imports supported attachments from all senders.
            </p>
          </div>

          <div className="flex flex-wrap items-center gap-3">
            <label className="flex items-center gap-2 text-sm">
              <input
                type="checkbox"
                checked={form.use_tls}
                onChange={(event) => setForm({ ...form, use_tls: event.target.checked })}
                className="size-4 accent-primary"
              />
              Use TLS
            </label>
            {!form.use_tls && (
              <span className="inline-flex items-center gap-1.5 rounded-md bg-warning-soft px-2 py-1 text-xs font-semibold text-warning">
                <ShieldAlert className="size-3.5" />
                Plain IMAP is for local test servers only
              </span>
            )}
            {form.password_configured && !password && (
              <span className="inline-flex items-center gap-1.5 rounded-md bg-success-soft px-2 py-1 text-xs font-semibold text-success">
                <CheckCircle2 className="size-3.5" />
                Password saved
              </span>
            )}
          </div>

          <div className="flex flex-wrap items-center gap-2">
            <Button type="submit" disabled={saveMutation.isPending} className="gap-2">
              {saveMutation.isPending ? (
                <Loader2 className="size-4 animate-spin" />
              ) : saved ? (
                <Check className="size-4" />
              ) : (
                <Save className="size-4" />
              )}
              Save
            </Button>
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
                <Wifi className="size-4" />
              )}
              Test Connection
            </Button>
          </div>
          {(saveMutation.error || testStatus) && (
            <div className="rounded-md border border-border bg-surface-2 px-3 py-2 text-sm text-muted-foreground">
              {saveMutation.error
                ? saveMutation.error instanceof Error
                  ? saveMutation.error.message
                  : String(saveMutation.error)
                : testStatus}
            </div>
          )}
        </form>
      </CardContent>
    </Card>
  );
}
