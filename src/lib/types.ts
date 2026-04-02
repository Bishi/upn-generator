// ─── Settings ─────────────────────────────────────────────────────────────

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
  split_basis: "occupants" | "m2_percentage";
}

export interface SmtpConfig {
  host: string;
  port: number;
  username: string;
  from_email: string;
  use_tls: boolean;
}

// ─── Billing Periods ───────────────────────────────────────────────────────

export interface BillingPeriod {
  id: number | null;
  building_id: number;
  month: number;
  year: number;
  status: string;
  created_at: string;
}

// ─── Bills ─────────────────────────────────────────────────────────────────

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
  status: string;
  source_filename: string;
  provider_name: string | null;
}

// ─── Splits ────────────────────────────────────────────────────────────────

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
  split_basis: "occupants" | "m2_percentage";
}

// ─── UPN ───────────────────────────────────────────────────────────────────

export interface EmailResult {
  apartment_label: string;
  email: string;
  success: boolean;
  error: string | null;
}

// ─── Helpers ───────────────────────────────────────────────────────────────

/** Format cents as "12,34" Slovenian-style. */
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
