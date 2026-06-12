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
  status: string;
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
  provider_name: string | null;
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
  delivery_type: "email" | "pdf";
  status: "sent" | "saved" | "failed" | "blocked";
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

export interface UpnDeliveryApartmentRollup {
  apartment_id: number;
  apartment_label: string;
  packet_hash: string;
  packet_error: string;
  delivered: boolean;
  email_sent: boolean;
  pdf_saved: boolean;
  current_failed_event_count: number;
  current_blocked_event_count: number;
  last_current_delivery_type: "email" | "pdf" | null;
  last_current_delivery_status: "sent" | "saved" | "failed" | "blocked" | null;
  last_current_delivery_at: string | null;
}

export interface UpnDeliveryRollup {
  billing_period_id: number;
  packet_count: number;
  current_delivered_count: number;
  email_sent_count: number;
  pdf_saved_count: number;
  current_failed_event_count: number;
  current_blocked_event_count: number;
  complete: boolean;
  last_delivery_at: string | null;
  apartments: UpnDeliveryApartmentRollup[];
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

export const MONTHS = [
  "Januar", "Februar", "Marec", "April", "Maj", "Junij",
  "Julij", "Avgust", "September", "Oktober", "November", "December",
];
