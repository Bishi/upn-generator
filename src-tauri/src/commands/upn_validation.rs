use chrono::NaiveDate;
use lettre::message::Mailbox;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use tauri::State;

use super::config::DbState;

pub const ACTION_SEND_EMAILS: &str = "send_emails";
pub const ACTION_MARK_DELIVERED: &str = "mark_delivered";
pub const ACTION_DOWNLOAD_ALL: &str = "download_all";

const SEVERITY_ERROR: &str = "error";
const SEVERITY_WARNING: &str = "warning";
const ENTITY_PERIOD: &str = "period";
const ENTITY_BILL: &str = "bill";
const ENTITY_APARTMENT: &str = "apartment";
const ENTITY_PROVIDER: &str = "provider";
const ENTITY_SPLIT: &str = "split";
const M2_TOTAL_TOLERANCE: f64 = 0.01;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UpnValidationIssue {
    pub severity: String,
    pub code: String,
    pub message: String,
    pub entity_type: String,
    pub bill_id: Option<i64>,
    pub apartment_id: Option<i64>,
    pub provider_id: Option<i64>,
    pub label: String,
    pub blocks: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UpnPreSendValidation {
    pub billing_period_id: i64,
    pub error_count: i64,
    pub warning_count: i64,
    pub can_send_emails: bool,
    pub can_mark_delivered: bool,
    pub can_download_all: bool,
    pub issues: Vec<UpnValidationIssue>,
}

#[derive(Debug, Clone)]
struct BillForValidation {
    id: i64,
    provider_id: Option<i64>,
    provider_name: Option<String>,
    source_filename: String,
    amount_cents: i64,
    creditor_iban: String,
    reference: String,
    due_date: String,
    purpose_code: String,
    purpose_text: String,
    split_basis: String,
}

#[derive(Debug, Clone)]
struct ActiveApartment {
    id: i64,
    label: String,
    occupant_count: i32,
    m2_percentage: f64,
}

#[derive(Debug, Clone)]
struct PacketApartment {
    id: i64,
    label: String,
    contact_email: String,
}

fn normalize_email(email: &str) -> String {
    email.trim().to_lowercase()
}

fn parse_recipient_list(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
        .map(|item| item.to_string())
        .collect()
}

fn parse_allowlist(raw: &str) -> HashSet<String> {
    raw.split(',')
        .map(normalize_email)
        .filter(|item| !item.is_empty())
        .collect()
}

fn all_actions() -> Vec<String> {
    vec![
        ACTION_SEND_EMAILS.to_string(),
        ACTION_MARK_DELIVERED.to_string(),
        ACTION_DOWNLOAD_ALL.to_string(),
    ]
}

fn send_only() -> Vec<String> {
    vec![ACTION_SEND_EMAILS.to_string()]
}

fn issue(
    severity: &str,
    code: &str,
    entity_type: &str,
    message: String,
    label: String,
    blocks: Vec<String>,
) -> UpnValidationIssue {
    UpnValidationIssue {
        severity: severity.to_string(),
        code: code.to_string(),
        message,
        entity_type: entity_type.to_string(),
        bill_id: None,
        apartment_id: None,
        provider_id: None,
        label,
        blocks,
    }
}

fn bill_issue(
    severity: &str,
    code: &str,
    bill: &BillForValidation,
    message: String,
    blocks: Vec<String>,
) -> UpnValidationIssue {
    let mut issue = issue(
        severity,
        code,
        ENTITY_BILL,
        message,
        bill_label(bill),
        blocks,
    );
    issue.bill_id = Some(bill.id);
    issue.provider_id = bill.provider_id;
    issue
}

fn apartment_issue(
    severity: &str,
    code: &str,
    apartment_id: i64,
    apartment_label: &str,
    message: String,
    blocks: Vec<String>,
) -> UpnValidationIssue {
    let mut issue = issue(
        severity,
        code,
        ENTITY_APARTMENT,
        message,
        apartment_label.to_string(),
        blocks,
    );
    issue.apartment_id = Some(apartment_id);
    issue
}

fn split_issue(
    code: &str,
    bill: &BillForValidation,
    apartment_id: Option<i64>,
    apartment_label: Option<&str>,
    message: String,
) -> UpnValidationIssue {
    let mut issue = issue(
        SEVERITY_ERROR,
        code,
        ENTITY_SPLIT,
        message,
        apartment_label
            .map(|label| format!("{} / {}", bill_label(bill), label))
            .unwrap_or_else(|| bill_label(bill)),
        all_actions(),
    );
    issue.bill_id = Some(bill.id);
    issue.apartment_id = apartment_id;
    issue.provider_id = bill.provider_id;
    issue
}

fn provider_issue(
    code: &str,
    provider_id: i64,
    label: String,
    message: String,
) -> UpnValidationIssue {
    let mut issue = issue(
        SEVERITY_ERROR,
        code,
        ENTITY_PROVIDER,
        message,
        label,
        all_actions(),
    );
    issue.provider_id = Some(provider_id);
    issue
}

fn bill_label(bill: &BillForValidation) -> String {
    bill.provider_name
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(&bill.source_filename)
        .to_string()
}

fn normalize_iban(iban: &str) -> String {
    iban.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

fn iban_mod97_is_valid(iban: &str) -> bool {
    if iban.len() < 4 {
        return false;
    }

    let rearranged = format!("{}{}", &iban[4..], &iban[..4]);
    let mut remainder: u32 = 0;
    for ch in rearranged.chars() {
        if ch.is_ascii_digit() {
            remainder = (remainder * 10 + ch.to_digit(10).unwrap_or(0)) % 97;
        } else if ch.is_ascii_uppercase() {
            let value = ch as u32 - 'A' as u32 + 10;
            remainder = (remainder * 100 + value) % 97;
        } else {
            return false;
        }
    }

    remainder == 1
}

fn valid_slovenian_iban(iban: &str) -> bool {
    let normalized = normalize_iban(iban);
    normalized.len() == 19
        && normalized.starts_with("SI")
        && normalized.chars().skip(2).all(|ch| ch.is_ascii_digit())
        && iban_mod97_is_valid(&normalized)
}

fn valid_due_date(due_date: &str) -> bool {
    let compact: String = due_date.chars().filter(|ch| !ch.is_whitespace()).collect();
    NaiveDate::parse_from_str(&compact, "%d.%m.%Y").is_ok()
}

fn valid_purpose_code(purpose_code: &str) -> bool {
    purpose_code.len() == 4 && purpose_code.chars().all(|ch| ch.is_ascii_uppercase())
}

fn load_bills(conn: &Connection, billing_period_id: i64) -> Result<Vec<BillForValidation>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT b.id, b.provider_id, p.name, b.source_filename, b.amount_cents,
                    b.creditor_iban, b.reference, b.due_date, b.purpose_code,
                    b.purpose_text, COALESCE(p.split_basis, 'm2_percentage')
             FROM bills b
             LEFT JOIN providers p ON b.provider_id = p.id
             WHERE b.billing_period_id = ?1
             ORDER BY b.id",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([billing_period_id], |row| {
            Ok(BillForValidation {
                id: row.get(0)?,
                provider_id: row.get(1)?,
                provider_name: row.get(2)?,
                source_filename: row.get(3)?,
                amount_cents: row.get(4)?,
                creditor_iban: row.get(5)?,
                reference: row.get(6)?,
                due_date: row.get(7)?,
                purpose_code: row.get(8)?,
                purpose_text: row.get(9)?,
                split_basis: row.get(10)?,
            })
        })
        .map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

fn load_active_apartments(conn: &Connection) -> Result<Vec<ActiveApartment>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, label, occupant_count, m2_percentage
             FROM apartments
             WHERE building_id = 1 AND is_active = 1
             ORDER BY label",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(ActiveApartment {
                id: row.get(0)?,
                label: row.get(1)?,
                occupant_count: row.get(2)?,
                m2_percentage: row.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

fn load_packet_apartments(
    conn: &Connection,
    billing_period_id: i64,
) -> Result<Vec<PacketApartment>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT DISTINCT a.id, a.label, a.contact_email
             FROM bill_splits bs
             JOIN bills b ON bs.bill_id = b.id
             JOIN apartments a ON bs.apartment_id = a.id
             WHERE b.billing_period_id = ?1
             ORDER BY a.label",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([billing_period_id], |row| {
            Ok(PacketApartment {
                id: row.get(0)?,
                label: row.get(1)?,
                contact_email: row.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

fn load_smtp_allowlist(conn: &Connection) -> Result<(bool, HashSet<String>), String> {
    conn.query_row(
        "SELECT allowlist_enabled, recipient_allowlist FROM smtp_config WHERE id = 1",
        [],
        |row| {
            let enabled = row.get::<_, i32>(0)? != 0;
            let raw = row.get::<_, String>(1)?;
            Ok((enabled, parse_allowlist(&raw)))
        },
    )
    .map_err(|e| e.to_string())
}

fn validate_payment_fields(bills: &[BillForValidation], issues: &mut Vec<UpnValidationIssue>) {
    for bill in bills {
        if !valid_slovenian_iban(&bill.creditor_iban) {
            issues.push(bill_issue(
                SEVERITY_ERROR,
                "invalid_creditor_iban",
                bill,
                "Creditor IBAN is missing or invalid.".to_string(),
                all_actions(),
            ));
        }
        if bill.reference.trim().is_empty() {
            issues.push(bill_issue(
                SEVERITY_ERROR,
                "missing_payment_reference",
                bill,
                "Payment reference is missing.".to_string(),
                all_actions(),
            ));
        }
        if bill.due_date.trim().is_empty() || !valid_due_date(&bill.due_date) {
            issues.push(bill_issue(
                SEVERITY_ERROR,
                "invalid_due_date",
                bill,
                "Due date is missing or invalid.".to_string(),
                all_actions(),
            ));
        }
        if !valid_purpose_code(bill.purpose_code.trim()) {
            issues.push(bill_issue(
                SEVERITY_ERROR,
                "invalid_purpose_code",
                bill,
                "Purpose code must be exactly four uppercase letters, such as OTHR.".to_string(),
                all_actions(),
            ));
        }
        if bill.purpose_text.trim().is_empty() {
            issues.push(bill_issue(
                SEVERITY_ERROR,
                "missing_purpose_text",
                bill,
                "Purpose text is missing.".to_string(),
                all_actions(),
            ));
        }
    }
}

fn validate_duplicate_providers(bills: &[BillForValidation], issues: &mut Vec<UpnValidationIssue>) {
    let mut grouped: BTreeMap<i64, Vec<&BillForValidation>> = BTreeMap::new();
    for bill in bills {
        if let Some(provider_id) = bill.provider_id {
            grouped.entry(provider_id).or_default().push(bill);
        }
    }

    for (provider_id, provider_bills) in grouped {
        if provider_bills.len() <= 1 {
            continue;
        }
        let label = provider_bills
            .first()
            .and_then(|bill| bill.provider_name.clone())
            .unwrap_or_else(|| format!("Provider {provider_id}"));
        issues.push(provider_issue(
            "duplicate_provider_bills",
            provider_id,
            label.clone(),
            format!(
                "{} has {} bills in this billing period.",
                label,
                provider_bills.len()
            ),
        ));
    }
}

fn validate_split_rows(
    conn: &Connection,
    bills: &[BillForValidation],
    active_apartments: &[ActiveApartment],
    issues: &mut Vec<UpnValidationIssue>,
) -> Result<(), String> {
    for bill in bills {
        let rows = {
            let mut stmt = conn
                .prepare(
                    "SELECT bs.apartment_id, a.label, a.is_active, bs.amount_cents
                     FROM bill_splits bs
                     JOIN apartments a ON bs.apartment_id = a.id
                     WHERE bs.bill_id = ?1
                     ORDER BY a.label",
                )
                .map_err(|e| e.to_string())?;
            let mapped = stmt
                .query_map([bill.id], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i32>(2)? != 0,
                        row.get::<_, i64>(3)?,
                    ))
                })
                .map_err(|e| e.to_string())?;
            mapped
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?
        };

        if rows.is_empty() {
            issues.push(split_issue(
                "bill_without_splits",
                bill,
                None,
                None,
                "Bill has no split rows. Recalculate splits before sending.".to_string(),
            ));
            continue;
        }

        let split_sum = rows
            .iter()
            .map(|(_, _, _, amount_cents)| *amount_cents)
            .sum::<i64>();
        if split_sum != bill.amount_cents {
            issues.push(split_issue(
                "split_total_mismatch",
                bill,
                None,
                None,
                format!(
                    "Split total {} cents does not match bill total {} cents.",
                    split_sum, bill.amount_cents
                ),
            ));
        }

        for (apartment_id, apartment_label, is_active, _) in &rows {
            if !*is_active {
                issues.push(split_issue(
                    "inactive_apartment_split",
                    bill,
                    Some(*apartment_id),
                    Some(apartment_label),
                    "Split row belongs to an inactive apartment.".to_string(),
                ));
            }
        }

        let row_active_ids = rows
            .iter()
            .filter_map(|(apartment_id, _, is_active, _)| {
                if *is_active {
                    Some(*apartment_id)
                } else {
                    None
                }
            })
            .collect::<HashSet<_>>();

        for apartment in active_apartments {
            if !row_active_ids.contains(&apartment.id) {
                issues.push(split_issue(
                    "missing_active_apartment_split",
                    bill,
                    Some(apartment.id),
                    Some(&apartment.label),
                    "Active apartment is missing a split row for this bill.".to_string(),
                ));
            }
        }
    }

    Ok(())
}

fn validate_split_inputs(
    bills: &[BillForValidation],
    active_apartments: &[ActiveApartment],
    issues: &mut Vec<UpnValidationIssue>,
) {
    let has_occupant_bill = bills.iter().any(|bill| bill.split_basis == "occupants");
    let has_m2_bill = bills.iter().any(|bill| bill.split_basis == "m2_percentage");

    if has_occupant_bill {
        let occupant_total = active_apartments
            .iter()
            .map(|apartment| apartment.occupant_count)
            .sum::<i32>();
        if occupant_total <= 0 {
            issues.push(issue(
                SEVERITY_ERROR,
                "zero_active_occupants",
                ENTITY_PERIOD,
                "An occupant-based bill exists, but all active apartments have zero occupants."
                    .to_string(),
                "Active apartments".to_string(),
                all_actions(),
            ));
        }
    }

    if has_m2_bill {
        let m2_total = active_apartments
            .iter()
            .map(|apartment| apartment.m2_percentage)
            .sum::<f64>();
        if m2_total <= 0.0 {
            issues.push(issue(
                SEVERITY_ERROR,
                "zero_active_m2_percentage",
                ENTITY_PERIOD,
                "An m2-based bill exists, but active apartment m2 percentages total zero."
                    .to_string(),
                "Active apartments".to_string(),
                all_actions(),
            ));
        } else if (m2_total - 100.0).abs() > M2_TOTAL_TOLERANCE {
            issues.push(issue(
                SEVERITY_WARNING,
                "m2_percentage_total_not_100",
                ENTITY_PERIOD,
                format!(
                    "Active apartment m2 percentages total {:.2}% instead of 100.00%.",
                    m2_total
                ),
                "Active apartments".to_string(),
                vec![],
            ));
        }
    }
}

fn validate_recipients(
    conn: &Connection,
    billing_period_id: i64,
    issues: &mut Vec<UpnValidationIssue>,
) -> Result<(), String> {
    let packets = load_packet_apartments(conn, billing_period_id)?;
    let (allowlist_enabled, allowlist) = load_smtp_allowlist(conn)?;

    for packet in packets {
        let recipients = parse_recipient_list(&packet.contact_email);
        if recipients.is_empty() {
            issues.push(apartment_issue(
                SEVERITY_ERROR,
                "missing_recipient",
                packet.id,
                &packet.label,
                "Apartment has UPN packets but no email recipient.".to_string(),
                send_only(),
            ));
            continue;
        }

        for original in recipients {
            let normalized = normalize_email(&original);
            if normalized.parse::<Mailbox>().is_err() {
                issues.push(apartment_issue(
                    SEVERITY_ERROR,
                    "invalid_recipient",
                    packet.id,
                    &packet.label,
                    format!("Invalid email recipient: {original}"),
                    send_only(),
                ));
                continue;
            }

            if allowlist_enabled && !allowlist.contains(&normalized) {
                issues.push(apartment_issue(
                    SEVERITY_ERROR,
                    "recipient_not_allowlisted",
                    packet.id,
                    &packet.label,
                    format!("Recipient is outside the enabled email safety allowlist: {original}"),
                    send_only(),
                ));
            }
        }
    }

    Ok(())
}

pub fn validate_upn_pre_send_inner(
    conn: &Connection,
    billing_period_id: i64,
) -> Result<UpnPreSendValidation, String> {
    let period_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM billing_periods WHERE id = ?1",
            [billing_period_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    if period_count == 0 {
        return Err(format!(
            "Billing period {billing_period_id} does not exist."
        ));
    }

    let bills = load_bills(conn, billing_period_id)?;
    let active_apartments = load_active_apartments(conn)?;
    let mut issues = Vec::new();

    if bills.is_empty() {
        issues.push(issue(
            SEVERITY_ERROR,
            "no_bills",
            ENTITY_PERIOD,
            "No bills are imported for this billing period.".to_string(),
            "Billing period".to_string(),
            all_actions(),
        ));
    }
    if active_apartments.is_empty() {
        issues.push(issue(
            SEVERITY_ERROR,
            "no_active_apartments",
            ENTITY_PERIOD,
            "No active apartments are configured.".to_string(),
            "Active apartments".to_string(),
            all_actions(),
        ));
    }

    validate_payment_fields(&bills, &mut issues);
    validate_duplicate_providers(&bills, &mut issues);
    validate_split_inputs(&bills, &active_apartments, &mut issues);
    validate_split_rows(conn, &bills, &active_apartments, &mut issues)?;
    validate_recipients(conn, billing_period_id, &mut issues)?;

    let error_count = issues
        .iter()
        .filter(|issue| issue.severity == SEVERITY_ERROR)
        .count() as i64;
    let warning_count = issues
        .iter()
        .filter(|issue| issue.severity == SEVERITY_WARNING)
        .count() as i64;
    let can_send_emails = !validation_blocks_action_from_issues(&issues, ACTION_SEND_EMAILS);
    let can_mark_delivered = !validation_blocks_action_from_issues(&issues, ACTION_MARK_DELIVERED);
    let can_download_all = !validation_blocks_action_from_issues(&issues, ACTION_DOWNLOAD_ALL);

    Ok(UpnPreSendValidation {
        billing_period_id,
        error_count,
        warning_count,
        can_send_emails,
        can_mark_delivered,
        can_download_all,
        issues,
    })
}

fn validation_blocks_action_from_issues(issues: &[UpnValidationIssue], action: &str) -> bool {
    issues.iter().any(|issue| {
        issue.severity == SEVERITY_ERROR && issue.blocks.iter().any(|block| block == action)
    })
}

pub fn validation_blocks_action(validation: &UpnPreSendValidation, action: &str) -> bool {
    validation_blocks_action_from_issues(&validation.issues, action)
}

fn action_label(action: &str) -> &str {
    match action {
        ACTION_SEND_EMAILS => "Send Emails",
        ACTION_MARK_DELIVERED => "Mark Delivered",
        ACTION_DOWNLOAD_ALL => "Download All PDFs",
        _ => "UPN action",
    }
}

pub fn ensure_validation_allows(
    conn: &Connection,
    billing_period_id: i64,
    action: &str,
) -> Result<(), String> {
    let validation = validate_upn_pre_send_inner(conn, billing_period_id)?;
    if validation_blocks_action(&validation, action) {
        let count = validation
            .issues
            .iter()
            .filter(|issue| {
                issue.severity == SEVERITY_ERROR && issue.blocks.iter().any(|block| block == action)
            })
            .count();
        return Err(format!(
            "UPN validation failed for {}: {} blocking issue{}. Open UPN Preview for details.",
            action_label(action),
            count,
            if count == 1 { "" } else { "s" }
        ));
    }
    Ok(())
}

#[tauri::command]
pub fn validate_upn_pre_send(
    db: State<DbState>,
    billing_period_id: i64,
) -> Result<UpnPreSendValidation, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    validate_upn_pre_send_inner(&conn, billing_period_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;

    fn setup_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE billing_periods (
                id INTEGER PRIMARY KEY,
                building_id INTEGER NOT NULL,
                month INTEGER NOT NULL,
                year INTEGER NOT NULL,
                status TEXT NOT NULL DEFAULT 'draft',
                created_at TEXT NOT NULL DEFAULT ''
            );
            CREATE TABLE apartments (
                id INTEGER PRIMARY KEY,
                building_id INTEGER NOT NULL,
                label TEXT NOT NULL,
                unit_code TEXT NOT NULL DEFAULT '',
                occupant_count INTEGER NOT NULL DEFAULT 1,
                contact_email TEXT NOT NULL DEFAULT '',
                payer_name TEXT NOT NULL DEFAULT '',
                payer_address TEXT NOT NULL DEFAULT '',
                payer_city TEXT NOT NULL DEFAULT '',
                payer_postal_code TEXT NOT NULL DEFAULT '',
                m2_percentage REAL NOT NULL DEFAULT 0,
                is_active INTEGER NOT NULL DEFAULT 1
            );
            CREATE TABLE providers (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                service_type TEXT NOT NULL DEFAULT '',
                creditor_name TEXT NOT NULL DEFAULT '',
                creditor_address TEXT NOT NULL DEFAULT '',
                creditor_city TEXT NOT NULL DEFAULT '',
                creditor_postal_code TEXT NOT NULL DEFAULT '',
                creditor_iban TEXT NOT NULL DEFAULT '',
                purpose_code TEXT NOT NULL DEFAULT 'OTHR',
                match_pattern TEXT NOT NULL DEFAULT '',
                amount_pattern TEXT NOT NULL DEFAULT '',
                reference_pattern TEXT NOT NULL DEFAULT '',
                due_date_pattern TEXT NOT NULL DEFAULT '',
                invoice_number_pattern TEXT NOT NULL DEFAULT '',
                purpose_text_template TEXT NOT NULL DEFAULT '',
                split_basis TEXT NOT NULL DEFAULT 'm2_percentage'
            );
            CREATE TABLE bills (
                id INTEGER PRIMARY KEY,
                billing_period_id INTEGER NOT NULL,
                provider_id INTEGER,
                raw_text TEXT NOT NULL DEFAULT '',
                amount_cents INTEGER NOT NULL DEFAULT 0,
                creditor_name TEXT NOT NULL DEFAULT '',
                creditor_iban TEXT NOT NULL DEFAULT '',
                creditor_address TEXT NOT NULL DEFAULT '',
                creditor_city TEXT NOT NULL DEFAULT '',
                creditor_postal_code TEXT NOT NULL DEFAULT '',
                reference TEXT NOT NULL DEFAULT '',
                due_date TEXT NOT NULL DEFAULT '',
                purpose_code TEXT NOT NULL DEFAULT 'OTHR',
                purpose_text TEXT NOT NULL DEFAULT '',
                invoice_number TEXT NOT NULL DEFAULT '',
                parse_note TEXT NOT NULL DEFAULT '',
                status TEXT NOT NULL DEFAULT 'draft',
                source_filename TEXT NOT NULL DEFAULT ''
            );
            CREATE TABLE bill_splits (
                id INTEGER PRIMARY KEY,
                bill_id INTEGER NOT NULL,
                apartment_id INTEGER NOT NULL,
                amount_cents INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE smtp_config (
                id INTEGER PRIMARY KEY,
                allowlist_enabled INTEGER NOT NULL DEFAULT 0,
                recipient_allowlist TEXT NOT NULL DEFAULT ''
            );
            CREATE TABLE upn_delivery_events (
                id INTEGER PRIMARY KEY,
                attempt_id TEXT NOT NULL DEFAULT '',
                billing_period_id INTEGER NOT NULL,
                apartment_id INTEGER NOT NULL,
                delivery_type TEXT NOT NULL DEFAULT 'email',
                status TEXT NOT NULL DEFAULT '',
                recipient TEXT NOT NULL DEFAULT '',
                original_recipient TEXT NOT NULL DEFAULT '',
                attachment_sha256 TEXT NOT NULL DEFAULT '',
                error TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL DEFAULT ''
            );
            INSERT INTO billing_periods (id, building_id, month, year) VALUES (1, 1, 6, 2026);
            INSERT INTO smtp_config (id, allowlist_enabled, recipient_allowlist) VALUES (1, 0, '');
            ",
        )
        .unwrap();
        conn
    }

    fn insert_provider(conn: &Connection, id: i64, split_basis: &str) {
        conn.execute(
            "INSERT INTO providers (
                id, name, creditor_iban, purpose_code, purpose_text_template, split_basis
             ) VALUES (?1, ?2, 'SI56 0400 1004 8988 093', 'OTHR', 'Utilities', ?3)",
            params![id, format!("Provider {id}"), split_basis],
        )
        .unwrap();
    }

    fn insert_apartment(
        conn: &Connection,
        id: i64,
        label: &str,
        email: &str,
        occupants: i32,
        m2: f64,
        active: bool,
    ) {
        conn.execute(
            "INSERT INTO apartments (
                id, building_id, label, occupant_count, contact_email,
                payer_name, m2_percentage, is_active
             ) VALUES (?1, 1, ?2, ?3, ?4, ?2, ?5, ?6)",
            params![id, label, occupants, email, m2, if active { 1 } else { 0 }],
        )
        .unwrap();
    }

    fn insert_bill(conn: &Connection, id: i64, provider_id: Option<i64>, amount_cents: i64) {
        conn.execute(
            "INSERT INTO bills (
                id, billing_period_id, provider_id, amount_cents, creditor_iban,
                reference, due_date, purpose_code, purpose_text, source_filename
             ) VALUES (?1, 1, ?2, ?3, 'SI56 0400 1004 8988 093',
                'SI00 123', '30.06.2026', 'OTHR', 'Utilities', ?4)",
            params![id, provider_id, amount_cents, format!("bill-{id}.pdf")],
        )
        .unwrap();
    }

    fn insert_split(conn: &Connection, bill_id: i64, apartment_id: i64, amount_cents: i64) {
        conn.execute(
            "INSERT INTO bill_splits (bill_id, apartment_id, amount_cents)
             VALUES (?1, ?2, ?3)",
            params![bill_id, apartment_id, amount_cents],
        )
        .unwrap();
    }

    fn setup_valid_period() -> Connection {
        let conn = setup_conn();
        insert_provider(&conn, 1, "m2_percentage");
        insert_apartment(&conn, 1, "Apt 1", "one@example.com", 1, 50.0, true);
        insert_apartment(&conn, 2, "Apt 2", "two@example.com", 1, 50.0, true);
        insert_bill(&conn, 1, Some(1), 1000);
        insert_split(&conn, 1, 1, 500);
        insert_split(&conn, 1, 2, 500);
        conn
    }

    fn validation_codes(validation: &UpnPreSendValidation) -> Vec<String> {
        validation
            .issues
            .iter()
            .map(|issue| issue.code.clone())
            .collect()
    }

    #[test]
    fn happy_path_allows_actions() {
        let conn = setup_valid_period();

        let validation = validate_upn_pre_send_inner(&conn, 1).unwrap();

        assert_eq!(validation.error_count, 0);
        assert_eq!(validation.warning_count, 0);
        assert!(validation.can_send_emails);
        assert!(validation.can_mark_delivered);
        assert!(validation.can_download_all);
    }

    #[test]
    fn spaced_due_date_allows_actions() {
        let conn = setup_valid_period();
        conn.execute("UPDATE bills SET due_date='30. 06. 2026' WHERE id=1", [])
            .unwrap();

        let validation = validate_upn_pre_send_inner(&conn, 1).unwrap();

        assert!(!validation_codes(&validation).contains(&"invalid_due_date".to_string()));
        assert!(validation.can_send_emails);
        assert!(validation.can_mark_delivered);
        assert!(validation.can_download_all);
    }

    #[test]
    fn missing_recipient_blocks_send_only() {
        let conn = setup_valid_period();
        conn.execute("UPDATE apartments SET contact_email='' WHERE id=1", [])
            .unwrap();

        let validation = validate_upn_pre_send_inner(&conn, 1).unwrap();
        let codes = validation_codes(&validation);

        assert!(codes.contains(&"missing_recipient".to_string()));
        assert!(!validation.can_send_emails);
        assert!(validation.can_mark_delivered);
        assert!(validation.can_download_all);
    }

    #[test]
    fn invalid_recipient_blocks_send_only() {
        let conn = setup_valid_period();
        conn.execute(
            "UPDATE apartments SET contact_email='not an email' WHERE id=1",
            [],
        )
        .unwrap();

        let validation = validate_upn_pre_send_inner(&conn, 1).unwrap();

        assert!(validation_codes(&validation).contains(&"invalid_recipient".to_string()));
        assert!(!validation.can_send_emails);
        assert!(validation.can_mark_delivered);
    }

    #[test]
    fn allowlist_violation_blocks_send_before_events() {
        let conn = setup_valid_period();
        conn.execute(
            "UPDATE smtp_config SET allowlist_enabled=1, recipient_allowlist='one@example.com'",
            [],
        )
        .unwrap();

        let validation = validate_upn_pre_send_inner(&conn, 1).unwrap();

        assert!(validation_codes(&validation).contains(&"recipient_not_allowlisted".to_string()));
        assert!(!validation.can_send_emails);
        ensure_validation_allows(&conn, 1, ACTION_SEND_EMAILS).unwrap_err();
        let event_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM upn_delivery_events", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(event_count, 0);
    }

    #[test]
    fn invalid_bill_fields_block_all_actions() {
        let conn = setup_valid_period();
        conn.execute(
            "UPDATE bills SET creditor_iban='', reference='', due_date='31.02.2026',
             purpose_code='AB12', purpose_text='' WHERE id=1",
            [],
        )
        .unwrap();

        let validation = validate_upn_pre_send_inner(&conn, 1).unwrap();
        let codes = validation_codes(&validation);

        assert!(codes.contains(&"invalid_creditor_iban".to_string()));
        assert!(codes.contains(&"missing_payment_reference".to_string()));
        assert!(codes.contains(&"invalid_due_date".to_string()));
        assert!(codes.contains(&"invalid_purpose_code".to_string()));
        assert!(codes.contains(&"missing_purpose_text".to_string()));
        assert!(!validation.can_send_emails);
        assert!(!validation.can_mark_delivered);
        assert!(!validation.can_download_all);
    }

    #[test]
    fn duplicate_non_null_provider_blocks_but_null_provider_does_not() {
        let conn = setup_valid_period();
        insert_bill(&conn, 2, Some(1), 1000);
        insert_split(&conn, 2, 1, 500);
        insert_split(&conn, 2, 2, 500);
        insert_bill(&conn, 3, None, 200);
        insert_split(&conn, 3, 1, 100);
        insert_split(&conn, 3, 2, 100);
        insert_bill(&conn, 4, None, 300);
        insert_split(&conn, 4, 1, 150);
        insert_split(&conn, 4, 2, 150);

        let validation = validate_upn_pre_send_inner(&conn, 1).unwrap();
        let codes = validation_codes(&validation);

        assert_eq!(
            codes
                .iter()
                .filter(|code| *code == "duplicate_provider_bills")
                .count(),
            1
        );
        assert!(!validation.can_send_emails);
    }

    #[test]
    fn split_total_mismatch_blocks_all_actions() {
        let conn = setup_valid_period();
        conn.execute(
            "UPDATE bill_splits SET amount_cents=400 WHERE bill_id=1 AND apartment_id=1",
            [],
        )
        .unwrap();

        let validation = validate_upn_pre_send_inner(&conn, 1).unwrap();

        assert!(validation_codes(&validation).contains(&"split_total_mismatch".to_string()));
        assert!(!validation.can_download_all);
    }

    #[test]
    fn missing_active_and_inactive_split_rows_are_detected() {
        let conn = setup_valid_period();
        insert_apartment(&conn, 3, "Apt 3", "three@example.com", 1, 0.0, false);
        conn.execute(
            "DELETE FROM bill_splits WHERE bill_id=1 AND apartment_id=2",
            [],
        )
        .unwrap();
        insert_split(&conn, 1, 3, 500);

        let validation = validate_upn_pre_send_inner(&conn, 1).unwrap();
        let codes = validation_codes(&validation);

        assert!(codes.contains(&"missing_active_apartment_split".to_string()));
        assert!(codes.contains(&"inactive_apartment_split".to_string()));
    }

    #[test]
    fn zero_occupants_for_occupant_basis_blocks() {
        let conn = setup_conn();
        insert_provider(&conn, 1, "occupants");
        insert_apartment(&conn, 1, "Apt 1", "one@example.com", 0, 50.0, true);
        insert_apartment(&conn, 2, "Apt 2", "two@example.com", 0, 50.0, true);
        insert_bill(&conn, 1, Some(1), 1000);
        insert_split(&conn, 1, 1, 500);
        insert_split(&conn, 1, 2, 500);

        let validation = validate_upn_pre_send_inner(&conn, 1).unwrap();

        assert!(validation_codes(&validation).contains(&"zero_active_occupants".to_string()));
        assert!(!validation.can_mark_delivered);
    }

    #[test]
    fn m2_zero_blocks_but_non_100_warns_only() {
        let conn = setup_valid_period();
        conn.execute("UPDATE apartments SET m2_percentage=40 WHERE id=1", [])
            .unwrap();

        let validation = validate_upn_pre_send_inner(&conn, 1).unwrap();
        assert!(validation_codes(&validation).contains(&"m2_percentage_total_not_100".to_string()));
        assert_eq!(validation.error_count, 0);
        assert_eq!(validation.warning_count, 1);
        assert!(validation.can_send_emails);

        conn.execute("UPDATE apartments SET m2_percentage=0", [])
            .unwrap();
        let validation = validate_upn_pre_send_inner(&conn, 1).unwrap();
        assert!(validation_codes(&validation).contains(&"zero_active_m2_percentage".to_string()));
        assert!(!validation.can_download_all);
    }
}
