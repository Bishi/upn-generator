import { useMutation, useQueryClient } from "@tanstack/react-query";
import { confirm, message, open, save } from "@tauri-apps/plugin-dialog";
import { DatabaseBackup, RotateCcw, ShieldCheck } from "lucide-react";
import { useState } from "react";
import { ipc } from "@/lib/ipc";
import { setStoredBillingPeriod } from "@/lib/billing-period-selection";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

function backupFilename() {
  const now = new Date();
  const date = [
    now.getFullYear(),
    String(now.getMonth() + 1).padStart(2, "0"),
    String(now.getDate()).padStart(2, "0"),
  ].join("-");
  const time = [
    String(now.getHours()).padStart(2, "0"),
    String(now.getMinutes()).padStart(2, "0"),
  ].join("-");

  return `upn-generator-backup-${date}_${time}.sqlite3`;
}

export function DataSection() {
  const queryClient = useQueryClient();
  const [resetConfirm, setResetConfirm] = useState("");

  const backupMutation = useMutation({
    mutationFn: ipc.createDbBackup,
    onSuccess: async ({ path }) => {
      await message(`Backup saved to:\n${path}`, {
        title: "Backup Created",
        kind: "info",
      });
    },
  });

  const restoreMutation = useMutation({
    mutationFn: ipc.restoreDbBackup,
    onSuccess: async () => {
      setStoredBillingPeriod(null);
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["building"] }),
        queryClient.invalidateQueries({ queryKey: ["apartments"] }),
        queryClient.invalidateQueries({ queryKey: ["providers"] }),
        queryClient.invalidateQueries({ queryKey: ["smtp_config"] }),
        queryClient.invalidateQueries({ queryKey: ["bills"] }),
        queryClient.invalidateQueries({ queryKey: ["splits"] }),
        queryClient.invalidateQueries({ queryKey: ["workflow-status"] }),
      ]);
      await message(
        "Backup restored. SMTP password was not included and must be entered again before sending email.",
        {
          title: "Restore Complete",
          kind: "info",
        }
      );
      window.location.reload();
    },
  });

  const resetMutation = useMutation({
    mutationFn: ipc.resetAllData,
    onSuccess: async () => {
      setStoredBillingPeriod(null);
      setResetConfirm("");
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["building"] }),
        queryClient.invalidateQueries({ queryKey: ["apartments"] }),
        queryClient.invalidateQueries({ queryKey: ["providers"] }),
        queryClient.invalidateQueries({ queryKey: ["smtp_config"] }),
        queryClient.invalidateQueries({ queryKey: ["bills"] }),
        queryClient.invalidateQueries({ queryKey: ["splits"] }),
        queryClient.invalidateQueries({ queryKey: ["workflow-status"] }),
      ]);
      window.location.reload();
    },
  });

  const handleCreateBackup = async () => {
    const outputPath = await save({
      title: "Save Backup",
      defaultPath: backupFilename(),
      filters: [{ name: "SQLite Backup", extensions: ["sqlite3", "db"] }],
    });

    if (!outputPath) return;
    backupMutation.mutate(outputPath);
  };

  const handleRestoreBackup = async () => {
    const confirmed = await confirm(
      "Restore will replace the current building, apartments, providers, billing periods, bills, splits, and SMTP settings. The SMTP password will stay blank after restore.",
      {
        title: "Restore Backup",
        kind: "warning",
        okLabel: "Restore",
        cancelLabel: "Cancel",
      }
    );

    if (!confirmed) return;

    const selected = await open({
      title: "Select Backup File",
      multiple: false,
      filters: [{ name: "SQLite Backup", extensions: ["sqlite3", "db"] }],
    });

    if (!selected || Array.isArray(selected)) return;
    restoreMutation.mutate(selected);
  };

  return (
    <div className="grid max-w-4xl gap-4 xl:grid-cols-[1fr_0.9fr]">
      <Card className="overflow-hidden">
        <div className="border-b border-border px-5 py-4">
          <h3 className="font-head text-lg font-semibold">Data Backup</h3>
          <p className="mt-1 text-sm text-muted-foreground">
            Create a manual SQLite backup or restore from a previous backup file.
          </p>
        </div>
        <CardContent className="space-y-5 p-5">
          <div className="flex items-center gap-3 rounded-lg border border-border bg-surface-2 p-4">
            <span className="grid size-10 place-items-center rounded-md bg-success-soft text-success">
              <ShieldCheck className="size-5" />
            </span>
            <div className="min-w-0 flex-1">
              <div className="text-sm font-semibold">Backed up locally</div>
              <div className="text-xs text-muted-foreground">
                SQLite snapshots include app data and selected appearance theme.
              </div>
            </div>
          </div>

          <div className="flex flex-wrap gap-3">
            <Button
              type="button"
              className="gap-2"
              disabled={backupMutation.isPending || restoreMutation.isPending}
              onClick={handleCreateBackup}
            >
              <DatabaseBackup className="size-4" />
              {backupMutation.isPending ? "Creating Backup..." : "Create Backup"}
            </Button>
            <Button
              type="button"
              variant="outline"
              className="gap-2"
              disabled={backupMutation.isPending || restoreMutation.isPending}
              onClick={handleRestoreBackup}
            >
              <RotateCcw className="size-4" />
              {restoreMutation.isPending ? "Restoring..." : "Restore Backup"}
            </Button>
          </div>

          <div className="space-y-2 text-sm text-muted-foreground">
            <p>Backups are saved wherever you choose in the file dialog.</p>
            <p>
              Suggested file format: <span className="font-mono">.sqlite3</span>
            </p>
            <p>Restore fully replaces current app data with the selected backup.</p>
            <p>After restore, re-enter the SMTP password before sending emails.</p>
          </div>
        </CardContent>
      </Card>

      <Card className="overflow-hidden">
        <div className="border-b border-border px-5 py-4">
          <h3 className="font-head text-lg font-semibold text-danger">Dev Factory Reset</h3>
          <p className="mt-1 text-sm text-muted-foreground">
            Reset seeded data for local testing.
          </p>
        </div>
        <CardContent className="p-5">
          <div className="space-y-3">
          <div>
            <p className="text-sm text-muted-foreground">
              Resets building, apartments, providers, SMTP, billing periods, bills, and splits
              back to the seeded defaults.
            </p>
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="reset-confirm">Type RESET ALL DATA to confirm</Label>
            <Input
              id="reset-confirm"
              value={resetConfirm}
              onChange={(e) => setResetConfirm(e.target.value)}
              placeholder="RESET ALL DATA"
            />
          </div>
          <Button
            type="button"
            variant="destructive"
            className="gap-2"
            disabled={
              resetConfirm !== "RESET ALL DATA" ||
              backupMutation.isPending ||
              restoreMutation.isPending ||
              resetMutation.isPending
            }
            onClick={() => resetMutation.mutate()}
          >
            <RotateCcw className="size-4" />
            {resetMutation.isPending ? "Resetting..." : "Reset All Data"}
          </Button>
        </div>

        {(backupMutation.error || restoreMutation.error || resetMutation.error) && (
          <div className="rounded-md border border-danger/40 bg-danger-soft px-4 py-3 text-sm text-danger">
            {backupMutation.error?.message ??
              restoreMutation.error?.message ??
              resetMutation.error?.message}
          </div>
        )}
        </CardContent>
      </Card>
    </div>
  );
}
