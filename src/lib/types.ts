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
  occupant_count: number;
  contact_email: string;
  payer_name: string;
  payer_address: string;
  payer_city: string;
  payer_postal_code: string;
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
}

export interface SmtpConfig {
  host: string;
  port: number;
  username: string;
  from_email: string;
  use_tls: boolean;
}
