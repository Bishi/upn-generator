// Settings

export interface Building {
  id: number | null;
  name: string;
  address: string;
  city: string;
  postal_code: string;
}

export interface Apartment {
  id: number | null;
  building_id: number;
  label: string;
  unit_code: string;
  occupant_count: number;
  contact_email: string;
  payer_name: string;
  payer_address: string;
  payer_city: string;
  payer_postal_code: string;
  m2_percentage: number;
  is_active: boolean;
}

export interface Provider {
  id: number | null;
  name: string;
  service_type: string;
  creditor_name: string;
  creditor_address: string;
  creditor_city: string;
  creditor_postal_code: string;
  creditor_iban: string;
  purpose_code: string;
  match_pattern: string;
  amount_pattern: string;
  reference_pattern: string;
  due_date_pattern: string;
  invoice_number_pattern: string;
  purpose_text_template: string;
  split_basis: "occupants" | "m2_percentage" | "equal_apartments";
  identity_rule_type: "unconfigured" | "labeled_value" | "building_address";
  identity_rule_operator: "all" | "any";
  identity_label: string;
  identity_value: string;
  identity_alternate_label: string;
  identity_alternate_value: string;
}

export interface SmtpConfig {
  host: string;
  port: number;
  username: string;
  from_email: string;
  use_tls: boolean;
  allowlist_enabled: boolean;
  recipient_allowlist: string;
  password_configured: boolean;
}

export interface InboxConfig {
  host: string;
  port: number;
  username: string;
  use_tls: boolean;
  folder: string;
  days_to_scan: number;
  sender_allowlist: string;
  password_configured: boolean;
}

export interface AppSettings {
  theme: string;
}

export interface BackupFileInfo {
  path: string;
}

export interface ResetAllDataResult {
  credential_cleanup_warning: string | null;
}

// Billing periods

export interface BillingPeriod {
  id: number | null;
  building_id: number;
  month: number;
  year: number;
  status: "draft" | "closed";
  closed_at: string | null;
  created_at: string;
}

// Bills

export interface Bill {
  id: number | null;
  billing_period_id: number;
  provider_id: number | null;
  raw_text: string;
  amount_cents: number;
  creditor_name: string;
  creditor_iban: string;
  creditor_address: string;
  creditor_city: string;
  creditor_postal_code: string;
  reference: string;
  due_date: string;
  purpose_code: string;
  purpose_text: string;
  invoice_number: string;
  parse_note: string;
  status: string;
  source_filename: string;
  reviewed_at: string | null;
  review_note: string;
  identity_status:
    | "not_checked"
    | "matched"
    | "mismatched"
    | "missing"
    | "unreadable"
    | "unconfigured"
    | "exception";
  identity_rule_snapshot: string;
  identity_evidence: string;
  identity_exception_note: string;
  identity_exception_at: string | null;
  source_page_start: number | null;
  source_page_end: number | null;
  source_document_id: number | null;
  provider_name: string | null;
}

export interface IdentityVerification {
  status: "not_checked" | "matched" | "mismatched" | "missing" | "unreadable" | "unconfigured";
  explanation: string;
  expected_values: string[];
  found_values: string[];
  page_start: number | null;
  page_end: number | null;
  rule_snapshot: string;
}

export interface PreparedBillPreviewSummary {
  provider_id: number | null;
  provider_name: string | null;
  creditor_name: string;
  amount_cents: number;
  reference: string;
  due_date: string;
  invoice_number: string;
  purpose_text: string;
  parse_note: string;
  status: string;
  identity: IdentityVerification;
  content_hash: string;
  source_page_start: number | null;
  source_page_end: number | null;
}

export interface LocalBillImportPreview {
  source_filename: string;
  source_handle: string;
  bills: PreparedBillPreviewSummary[];
}

export interface IdentityExceptionInput {
  content_hash: string;
  note: string;
  identity_status: string;
  rule_snapshot: string;
}

export interface LocalBillImportFinalizeRequest {
  source_handle: string;
  selected_content_hashes: string[];
  exceptions: IdentityExceptionInput[];
}

export interface SourceDocumentInfo {
  original_name: string;
  media_type: string;
  byte_size: number;
  page_start: number | null;
  page_end: number | null;
}

// Splits

export interface BillSplit {
  id: number | null;
  bill_id: number;
  apartment_id: number;
  amount_cents: number;
}

export interface SplitRow {
  split_id: number | null;
  bill_id: number;
  apartment_id: number;
  apartment_label: string;
  apartment_unit_code: string;
  bill_source_filename: string;
  provider_name: string | null;
  bill_amount_cents: number;
  split_amount_cents: number;
  occupant_count: number;
  m2_percentage: number;
  split_basis: "occupants" | "m2_percentage" | "equal_apartments";
  bill_status: string;
  bill_parse_note: string;
  bill_reviewed_at: string | null;
}

// UPN

export interface EmailResult {
  apartment_id: number;
  apartment_label: string;
  email: string;
  status: "sent" | "failed" | "blocked" | "partial" | "changed";
  recipient: string;
  original_recipient: string;
  success: boolean;
  error: string | null;
}

export interface UpnDeliveryEvent {
  id: number;
  attempt_id: string;
  billing_period_id: number;
  apartment_id: number;
  delivery_type: "email" | "pdf" | "manual";
  status: "sent" | "saved" | "delivered" | "failed" | "blocked";
  recipient: string;
  original_recipient: string;
  attachment_sha256: string;
  error: string;
  created_at: string;
}

export interface UpnPacketHash {
  apartment_id: number;
  attachment_sha256: string;
  error: string;
}

export interface UpnZipExportResult {
  path: string;
  count: number;
  filenames: string[];
}

export interface UpnDeliveryApartmentRollup {
  apartment_id: number;
  apartment_label: string;
  packet_hash: string;
  packet_error: string;
  delivered: boolean;
  email_sent: boolean;
  manual_delivered: boolean;
  current_failed_event_count: number;
  current_blocked_event_count: number;
  last_current_delivery_type: "email" | "pdf" | "manual" | null;
  last_current_delivery_status: "sent" | "saved" | "delivered" | "failed" | "blocked" | null;
  last_current_delivery_at: string | null;
}

export interface UpnDeliveryRollup {
  billing_period_id: number;
  packet_count: number;
  current_delivered_count: number;
  email_sent_count: number;
  manual_delivered_count: number;
  current_failed_event_count: number;
  current_blocked_event_count: number;
  complete: boolean;
  last_delivery_at: string | null;
  apartments: UpnDeliveryApartmentRollup[];
}

export type UpnValidationAction =
  | "send_emails"
  | "mark_delivered"
  | "download_all";

export type UpnValidationSeverity = "error" | "warning";

export type UpnValidationEntityType =
  | "period"
  | "bill"
  | "apartment"
  | "provider"
  | "split";

export interface UpnValidationIssue {
  severity: UpnValidationSeverity;
  code: string;
  message: string;
  entity_type: UpnValidationEntityType;
  bill_id: number | null;
  apartment_id: number | null;
  provider_id: number | null;
  label: string;
  blocks: UpnValidationAction[];
}

export interface UpnPreSendValidation {
  billing_period_id: number;
  error_count: number;
  warning_count: number;
  can_send_emails: boolean;
  can_mark_delivered: boolean;
  can_download_all: boolean;
  issues: UpnValidationIssue[];
}

export interface InboxImportResult {
  sender: string;
  subject: string;
  attachment_filename: string;
  status:
    | "imported"
    | "skipped_duplicate"
    | "skipped_duplicate_bill"
    | "skipped_wrong_period"
    | "skipped_unknown_period"
    | "skipped_unknown_provider"
    | "skipped_already_present"
    | "skipped_not_expected"
    | "failed";
  bill_ids: number[];
  bill_count: number;
  skipped_reason: string | null;
  error: string | null;
}

export interface InboxPreviewNotice {
  status:
    | "skipped_duplicate_bill"
    | "skipped_unknown_provider"
    | "skipped_already_present"
    | "skipped_not_expected"
    | string;
  message: string;
}

export interface InboxPreviewBillSummary {
  provider_id: number | null;
  provider_name: string | null;
  creditor_name: string;
  amount_cents: number;
  reference: string;
  due_date: string;
  invoice_number: string;
  purpose_text: string;
  parse_note: string;
  status: string;
  identity: IdentityVerification;
  content_hash: string;
  source_page_start: number | null;
  source_page_end: number | null;
}

export interface InboxPreviewCandidate {
  id: string;
  sender: string;
  subject: string;
  received_date: string | null;
  attachment_filename: string;
  attachment_sha256: string;
  status:
    | "ready"
    | "skipped_duplicate"
    | "skipped_duplicate_bill"
    | "skipped_wrong_period"
    | "skipped_unknown_period"
    | "skipped_unknown_provider"
    | "skipped_already_present"
    | "skipped_not_expected"
    | "empty"
    | "failed";
  selectable: boolean;
  importable_count: number;
  skipped_reason: string | null;
  error: string | null;
  bills: InboxPreviewBillSummary[];
  notices: InboxPreviewNotice[];
  source_available: boolean;
  source_unavailable_reason: string | null;
}

export interface InboxPreviewScanSummary {
  messages_matched: number;
  messages_fetched: number;
  messages_skipped_sender: number;
  messages_skipped_oversize: number;
  messages_without_supported_attachments: number;
  supported_attachments_found: number;
  unsupported_attachments_found: number;
  unsupported_attachment_names: string[];
  senders_seen: string[];
}

export interface InboxPreviewSession {
  session_id: string;
  billing_period_id: number;
  days_to_scan: number;
  username: string;
  folder: string;
  sender_allowlist: string;
  received_date_source: "imap_internal_date";
  scan_summary: InboxPreviewScanSummary;
  candidates: InboxPreviewCandidate[];
}

// Helpers

export function formatEur(cents: number): string {
  const euros = Math.floor(Math.abs(cents) / 100);
  const c = Math.abs(cents) % 100;
  const sign = cents < 0 ? "-" : "";
  return `${sign}${euros},${String(c).padStart(2, "0")}`;
}

export function formatClosedAt(value: string | null | undefined): string {
  if (!value) return "closed";
  const normalized = value.includes("T") ? value : `${value.replace(" ", "T")}Z`;
  const date = new Date(normalized);
  if (Number.isNaN(date.getTime())) return value.slice(0, 16);
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  const hours = String(date.getHours()).padStart(2, "0");
  const minutes = String(date.getMinutes()).padStart(2, "0");
  return `${year}-${month}-${day} ${hours}:${minutes}`;
}

export function parseEurInputCents(value: string): number {
  const normalized = value.trim().replace(",", ".");
  return Math.round((parseFloat(normalized) || 0) * 100);
}

export const MONTHS = [
  "Januar", "Februar", "Marec", "April", "Maj", "Junij",
  "Julij", "Avgust", "September", "Oktober", "November", "December",
];
