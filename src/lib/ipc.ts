import { invoke } from "@tauri-apps/api/core";
import type {
  Apartment,
  Bill,
  BillingPeriod,
  BillSplit,
  Building,
  EmailResult,
  Provider,
  SmtpConfig,
  SplitRow,
} from "./types";

export const ipc = {
  // ─── Config ───────────────────────────────────────────────────────────
  getBuilding: () => invoke<Building>("get_building"),
  saveBuilding: (building: Building) => invoke<Building>("save_building", { building }),

  getApartments: () => invoke<Apartment[]>("get_apartments"),
  saveApartment: (apartment: Apartment) =>
    invoke<Apartment>("save_apartment", { apartment }),
  deleteApartment: (id: number) => invoke<void>("delete_apartment", { id }),

  getProviders: () => invoke<Provider[]>("get_providers"),
  saveProvider: (provider: Provider) =>
    invoke<Provider>("save_provider", { provider }),
  deleteProvider: (id: number) => invoke<void>("delete_provider", { id }),

  getSmtpConfig: () => invoke<SmtpConfig>("get_smtp_config"),
  saveSmtpConfig: (config: SmtpConfig) =>
    invoke<void>("save_smtp_config", { config }),
  resetAllData: () => invoke<void>("reset_all_data"),

  // ─── Billing Periods ─────────────────────────────────────────────────
  getBillingPeriods: () => invoke<BillingPeriod[]>("get_billing_periods"),
  createBillingPeriod: (month: number, year: number) =>
    invoke<BillingPeriod>("create_billing_period", { month, year }),
  createYearPeriods: (year: number) =>
    invoke<BillingPeriod[]>("create_year_periods", { year }),

  // ─── Bills ───────────────────────────────────────────────────────────
  getBills: (billingPeriodId: number) =>
    invoke<Bill[]>("get_bills", { billingPeriodId }),
  importBill: (filePath: string, billingPeriodId: number) =>
    invoke<Bill>("import_bill", { filePath, billingPeriodId }),
  importBills: (filePath: string, billingPeriodId: number) =>
    invoke<Bill[]>("import_bills", { filePath, billingPeriodId }),
  saveBill: (bill: Bill) => invoke<Bill>("save_bill", { bill }),
  deleteBill: (id: number) => invoke<void>("delete_bill", { id }),

  // ─── Splits ──────────────────────────────────────────────────────────
  calculateSplits: (billingPeriodId: number) =>
    invoke<SplitRow[]>("calculate_splits", { billingPeriodId }),
  getSplits: (billingPeriodId: number) =>
    invoke<SplitRow[]>("get_splits", { billingPeriodId }),
  saveSplit: (split: BillSplit) => invoke<BillSplit>("save_split", { split }),

  // ─── UPN ─────────────────────────────────────────────────────────────
  generateUpnPdf: (billId: number, apartmentId: number) =>
    invoke<string>("generate_upn_pdf", { billId, apartmentId }),
  previewUpn: (billId: number, apartmentId: number) =>
    invoke<string>("preview_upn", { billId, apartmentId }),
  openPreviewUpn: (billId: number, apartmentId: number) =>
    invoke<string>("open_preview_upn", { billId, apartmentId }),
  openPreviewApartmentUpns: (billingPeriodId: number, apartmentId: number) =>
    invoke<string>("open_preview_apartment_upns", { billingPeriodId, apartmentId }),
  saveAllUpns: (billingPeriodId: number, folderPath: string) =>
    invoke<string[]>("save_all_upns", { billingPeriodId, folderPath }),
  sendEmails: (billingPeriodId: number) =>
    invoke<EmailResult[]>("send_emails", { billingPeriodId }),
  saveSmtpPassword: (password: string) =>
    invoke<void>("save_smtp_password", { password }),
  getSmtpPassword: () => invoke<string>("get_smtp_password"),
};
