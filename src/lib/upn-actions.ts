import { open } from "@tauri-apps/plugin-dialog";
import { ipc } from "@/lib/ipc";
import type { EmailResult } from "@/lib/types";

export type DownloadUpnResult = {
  folder: string;
  count: number;
  paths: string[];
};

export async function downloadPeriodUpnPdfs(
  billingPeriodId: number,
): Promise<DownloadUpnResult | null> {
  const folder = await open({
    directory: true,
    title: "Choose folder to save UPN PDFs",
  });
  if (!folder || typeof folder !== "string") return null;

  const paths = await ipc.saveAllUpns(billingPeriodId, folder);
  return {
    folder,
    count: paths.length,
    paths,
  };
}

export function sendPeriodEmails(
  billingPeriodId: number,
): Promise<EmailResult[]> {
  return ipc.sendEmails(billingPeriodId);
}
