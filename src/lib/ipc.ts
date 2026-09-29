import { invoke } from "@tauri-apps/api/core";
import type {
  AppSettings,
  Apartment,
  BackupFileInfo,
  Bill,
  BillingPeriod,
  BillSplit,
  Building,
  EmailResult,
  InboxConfig,
  InboxImportResult,
  InboxPreviewSession,
  IdentityExceptionInput,
  LocalBillImportFinalizeRequest,
  LocalBillImportPreview,
  Provider,
  ResetAllDataResult,
  SmtpConfig,
  SourceDocumentInfo,
  SplitRow,
  UpnDeliveryEvent,
  UpnDeliveryRollup,
  UpnPacketHash,
  UpnPreSendValidation,
  UpnZipExportResult,
} from "./types";

export const ipc = {
  createDbBackup: (outputPath: string) =>
    invoke<BackupFileInfo>("create_db_backup", { outputPath }),
  restoreDbBackup: (inputPath: string) =>
    invoke<void>("restore_db_backup", { inputPath }),

  // Config
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
  getInboxConfig: () => invoke<InboxConfig>("get_inbox_config"),
  saveInboxConfig: (config: InboxConfig) =>
    invoke<void>("save_inbox_config", { config }),
  saveInboxPassword: (password: string) =>
    invoke<void>("save_inbox_password", { password }),
  testInboxConnection: (config: InboxConfig, password: string) =>
    invoke<void>("test_inbox_connection", { config, password }),
  testSmtpConnection: (config: SmtpConfig, password: string, testRecipient: string) =>
    invoke<void>("test_smtp_connection", { config, password, testRecipient }),
  getAppSettings: () => invoke<AppSettings>("get_app_settings"),
  saveAppSettings: (settings: AppSettings) =>
    invoke<AppSettings>("save_app_settings", { settings }),
  resetAllData: () => invoke<ResetAllDataResult>("reset_all_data"),

  // Billing periods
  getBillingPeriods: () => invoke<BillingPeriod[]>("get_billing_periods"),
  createBillingPeriod: (month: number, year: number) =>
    invoke<BillingPeriod>("create_billing_period", { month, year }),
  createYearPeriods: (year: number) =>
    invoke<BillingPeriod[]>("create_year_periods", { year }),
  closeBillingPeriod: (billingPeriodId: number) =>
    invoke<BillingPeriod>("close_billing_period", { billingPeriodId }),
  reopenBillingPeriod: (billingPeriodId: number) =>
    invoke<BillingPeriod>("reopen_billing_period", { billingPeriodId }),

  // Bills
  getBills: (billingPeriodId: number) =>
    invoke<Bill[]>("get_bills", { billingPeriodId }),
  previewBillImport: (filePath: string, billingPeriodId: number) =>
    invoke<LocalBillImportPreview>("preview_bill_import", { filePath, billingPeriodId }),
  finalizeBillImportBatch: (
    billingPeriodId: number,
    files: LocalBillImportFinalizeRequest[],
  ) =>
    invoke<Bill[]>("finalize_bill_import_batch", {
      billingPeriodId,
      files,
    }),
  previewInboxAttachments: (billingPeriodId: number, daysToScan: number) =>
    invoke<InboxPreviewSession>("preview_inbox_attachments", {
      billingPeriodId,
      daysToScan,
    }),
  importInboxPreviewSelection: (
    sessionId: string,
    candidateIds: string[],
    exceptions: IdentityExceptionInput[],
  ) =>
    invoke<InboxImportResult[]>("import_inbox_preview_selection", {
      sessionId,
      candidateIds,
      exceptions,
    }),
  clearInboxPreviewSession: (sessionId: string) =>
    invoke<void>("clear_inbox_preview_session", { sessionId }),
  getInboxPreviewSourceInfo: (sessionId: string, candidateId: string) =>
    invoke<SourceDocumentInfo>("get_inbox_preview_source_info", { sessionId, candidateId }),
  readInboxPreviewSource: (sessionId: string, candidateId: string) =>
    invoke<ArrayBuffer>("read_inbox_preview_source", { sessionId, candidateId }),
  getLocalPreviewSourceInfo: (sourceHandle: string) =>
    invoke<SourceDocumentInfo>("get_local_preview_source_info", { sourceHandle }),
  readLocalPreviewSource: (sourceHandle: string) =>
    invoke<ArrayBuffer>("read_local_preview_source", { sourceHandle }),
  clearLocalPreviewSource: (sourceHandle: string) =>
    invoke<void>("clear_local_preview_source", { sourceHandle }),
  getBillSourceDocumentInfo: (billId: number) =>
    invoke<SourceDocumentInfo | null>("get_bill_source_document_info", { billId }),
  readBillSourceDocument: (billId: number) =>
    invoke<ArrayBuffer>("read_bill_source_document", { billId }),
  saveBill: (bill: Bill) => invoke<Bill>("save_bill", { bill }),
  markBillReviewed: (billId: number, reviewNote: string) =>
    invoke<Bill>("mark_bill_reviewed", { billId, reviewNote }),
  markBillUnreviewed: (billId: number) =>
    invoke<Bill>("mark_bill_unreviewed", { billId }),
  approveBillIdentityException: (billId: number, note: string) =>
    invoke<Bill>("approve_bill_identity_exception", { billId, note }),
  deleteBill: (id: number) => invoke<void>("delete_bill", { id }),

  // Splits
  calculateSplits: (billingPeriodId: number) =>
    invoke<SplitRow[]>("calculate_splits", { billingPeriodId }),
  getSplits: (billingPeriodId: number) =>
    invoke<SplitRow[]>("get_splits", { billingPeriodId }),
  saveSplit: (split: BillSplit) => invoke<BillSplit>("save_split", { split }),

  // UPN
  generateUpnPdf: (billId: number, apartmentId: number) =>
    invoke<string>("generate_upn_pdf", { billId, apartmentId }),
  previewUpn: (billId: number, apartmentId: number) =>
    invoke<string>("preview_upn", { billId, apartmentId }),
  openPreviewUpn: (billId: number, apartmentId: number) =>
    invoke<string>("open_preview_upn", { billId, apartmentId }),
  openPreviewApartmentUpns: (billingPeriodId: number, apartmentId: number) =>
    invoke<string>("open_preview_apartment_upns", { billingPeriodId, apartmentId }),
  saveAllUpnsZip: (billingPeriodId: number, outputPath: string) =>
    invoke<UpnZipExportResult>("save_all_upns_zip", { billingPeriodId, outputPath }),
  sendEmails: (billingPeriodId: number) =>
    invoke<EmailResult[]>("send_emails", { billingPeriodId }),
  markUpnPeriodDelivered: (billingPeriodId: number) =>
    invoke<UpnDeliveryRollup>("mark_upn_period_delivered", { billingPeriodId }),
  unmarkUpnPeriodDelivered: (billingPeriodId: number) =>
    invoke<UpnDeliveryRollup>("unmark_upn_period_delivered", { billingPeriodId }),
  getUpnDeliveryEvents: (billingPeriodId: number) =>
    invoke<UpnDeliveryEvent[]>("get_upn_delivery_events", { billingPeriodId }),
  getUpnDeliveryRollup: (billingPeriodId: number) =>
    invoke<UpnDeliveryRollup>("get_upn_delivery_rollup", { billingPeriodId }),
  getUpnPacketHashes: (billingPeriodId: number) =>
    invoke<UpnPacketHash[]>("get_upn_packet_hashes", { billingPeriodId }),
  validateUpnPreSend: (billingPeriodId: number) =>
    invoke<UpnPreSendValidation>("validate_upn_pre_send", { billingPeriodId }),
  saveSmtpPassword: (password: string) =>
    invoke<void>("save_smtp_password", { password }),
};
