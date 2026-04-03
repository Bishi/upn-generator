import { useMutation, useQueryClient } from "@tanstack/react-query";
import { confirm, message, open, save } from "@tauri-apps/plugin-dialog";
import { DatabaseBackup, RotateCcw } from "lucide-react";
import { ipc } from "@/lib/ipc";
import { setStoredBillingPeriod } from "@/lib/billing-period-selection";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";

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
    <Card className="max-w-2xl">
      <CardHeader>
        <CardTitle>Data Backup</CardTitle>
        <CardDescription>
          Create a manual SQLite backup of the app database or restore from a previous backup
          file. Backup files include billing and tenant data, but the SMTP password is excluded on
          purpose.
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-6">
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

        {(backupMutation.error || restoreMutation.error) && (
          <div className="rounded-md border border-destructive/40 bg-destructive/5 px-4 py-3 text-sm text-destructive">
            {backupMutation.error?.message ?? restoreMutation.error?.message}
          </div>
        )}
      </CardContent>
    </Card>
  );
}
