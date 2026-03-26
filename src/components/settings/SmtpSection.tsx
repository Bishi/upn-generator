import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { useState, useEffect } from "react";
import { Save, Eye, EyeOff } from "lucide-react";
import { ipc } from "@/lib/ipc";
import type { SmtpConfig } from "@/lib/types";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";

const emptyConfig: SmtpConfig = {
  host: "",
  port: 587,
  username: "",
  from_email: "",
  use_tls: true,
};

export function SmtpSection() {
  const queryClient = useQueryClient();
  const { data, isLoading } = useQuery({
    queryKey: ["smtp_config"],
    queryFn: ipc.getSmtpConfig,
  });

  const [form, setForm] = useState<SmtpConfig>(emptyConfig);
  const [password, setPassword] = useState("");
  const [showPassword, setShowPassword] = useState(false);
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    if (data) setForm(data);
  }, [data]);

  // Load existing password on mount
  useEffect(() => {
    ipc.getSmtpPassword().then((p) => { if (p) setPassword(p); }).catch(() => {});
  }, []);

  const mutation = useMutation({
    mutationFn: async (cfg: SmtpConfig) => {
      await ipc.saveSmtpConfig(cfg);
      if (password) await ipc.saveSmtpPassword(password);
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["smtp_config"] });
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
    },
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    mutation.mutate(form);
  };

  if (isLoading) return <div className="text-muted-foreground text-sm">Loading...</div>;

  return (
    <Card className="max-w-lg">
      <CardHeader>
        <CardTitle>Email (SMTP) Settings</CardTitle>
        <CardDescription>
          Used to send UPN PDFs to apartment tenants. The password is stored in Windows Credential Manager.
        </CardDescription>
      </CardHeader>
      <CardContent>
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
                onChange={(e) => setForm({ ...form, port: parseInt(e.target.value) || 587 })}
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
                placeholder="SMTP password"
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
            <p className="text-xs text-muted-foreground">Leave blank to keep the existing password.</p>
          </div>
          <div className="flex items-center gap-2">
            <input
              type="checkbox"
              id="use_tls"
              checked={form.use_tls}
              onChange={(e) => setForm({ ...form, use_tls: e.target.checked })}
              className="size-4"
            />
            <Label htmlFor="use_tls">Use TLS/STARTTLS</Label>
          </div>
          <Button type="submit" disabled={mutation.isPending} className="gap-2">
            <Save className="size-4" />
            {saved ? "Saved!" : mutation.isPending ? "Saving..." : "Save"}
          </Button>
        </form>
      </CardContent>
    </Card>
  );
}
