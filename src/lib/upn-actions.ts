import { save } from "@tauri-apps/plugin-dialog";
import { ipc } from "@/lib/ipc";
import type { EmailResult, UpnZipExportResult } from "@/lib/types";

function waitForPaint() {
  return new Promise<void>((resolve) => {
    requestAnimationFrame(() => resolve());
  });
}

function ensureZipExtension(path: string) {
  return path.toLowerCase().endsWith(".zip") ? path : `${path}.zip`;
}

export async function downloadPeriodUpnPdfs(
  billingPeriodId: number,
  defaultFilename: string,
): Promise<UpnZipExportResult | null> {
  const outputPath = await save({
    title: "Save UPN PDF ZIP",
    defaultPath: defaultFilename,
    filters: [{ name: "ZIP Archive", extensions: ["zip"] }],
  });
  if (!outputPath) return null;

  await waitForPaint();
  return ipc.saveAllUpnsZip(billingPeriodId, ensureZipExtension(outputPath));
}

export function sendPeriodEmails(
  billingPeriodId: number,
): Promise<EmailResult[]> {
  return ipc.sendEmails(billingPeriodId);
}
