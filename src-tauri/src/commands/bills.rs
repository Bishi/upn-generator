use regex::Regex;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use tauri::State;

use super::config::{DbState, Provider};

// ─── Structs ───────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BillingPeriod {
    pub id: Option<i64>,
    pub building_id: i64,
    pub month: i32,
    pub year: i32,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Bill {
    pub id: Option<i64>,
    pub billing_period_id: i64,
    pub provider_id: Option<i64>,
    pub raw_text: String,
    pub amount_cents: i64,
    pub creditor_name: String,
    pub creditor_iban: String,
    pub creditor_address: String,
    pub creditor_city: String,
    pub creditor_postal_code: String,
    pub reference: String,
    pub due_date: String,
    pub purpose_code: String,
    pub purpose_text: String,
    pub invoice_number: String,
    pub status: String,
    pub source_filename: String,
    // Joined display fields (not stored)
    pub provider_name: Option<String>,
}

// ─── Helpers ───────────────────────────────────────────────────────────────

/// Parse a Slovenian-format amount string to cents.
/// Handles "1.234,56" → 123456, "123,45" → 12345, "123.45" → 12345
fn parse_amount_to_cents(s: &str) -> i64 {
    let trimmed = s.trim().replace('\u{a0}', ""); // remove nbsp
    // Detect if comma is decimal separator (Slovenian: "123,45" or "1.234,56")
    let normalized = if trimmed.contains(',') {
        trimmed.replace('.', "").replace(',', ".")
    } else {
        trimmed.replace(',', "")
    };
    (normalized.parse::<f64>().unwrap_or(0.0) * 100.0).round() as i64
}

fn first_capture(pattern: &str, text: &str) -> Option<String> {
    if pattern.is_empty() {
        return None;
    }
    let re = Regex::new(pattern).ok()?;
    re.captures(text)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().trim().to_string())
}

fn normalize_spaces(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn extract_upn_purpose_from_context(
    context: &str,
    stub_offset_in_context: usize,
    purpose_code_re: &Regex,
) -> Option<(String, String)> {
    let mut best: Option<(usize, String, String)> = None;

    for caps in purpose_code_re.captures_iter(context) {
        let code_match = match caps.get(1) {
            Some(m) => m,
            None => continue,
        };

        let line_end = context[code_match.start()..]
            .find('\n')
            .map(|idx| code_match.start() + idx)
            .unwrap_or(context.len());
        let raw_line = context[code_match.start()..line_end].trim();
        if raw_line.is_empty() {
            continue;
        }

        let candidate = normalize_spaces(raw_line);
        if candidate.contains("SI56")
            || candidate.contains("***")
            || candidate.contains("Referenca")
            || candidate.contains("IBAN")
        {
            continue;
        }

        let distance = code_match.start().abs_diff(stub_offset_in_context);
        let code = code_match.as_str().to_string();
        let text = candidate[code.len()..].trim().to_string();
        if text.is_empty() {
            continue;
        }

        match &best {
            Some((best_distance, _, _)) if distance >= *best_distance => {}
            _ => best = Some((distance, code, text)),
        }
    }

    best.map(|(_, code, text)| (code, text))
}

fn interpolate_template(template: &str, invoice_number: &str, month: i32, year: i32) -> String {
    template
        .replace("{invoice_number}", invoice_number)
        .replace("{invoice}", invoice_number) // alias
        .replace("{month}", &format!("{:02}", month))
        .replace("{year}", &year.to_string())
        .replace("{MM}", &format!("{:02}", month))
        .replace("{YYYY}", &year.to_string())
}

/// Remove spaces from IBAN and uppercase for comparison
fn normalize_iban(iban: &str) -> String {
    iban.chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>()
        .to_uppercase()
}

/// Find first IBAN (SI56...) in text, return raw form
fn find_iban(text: &str) -> Option<String> {
    let re = Regex::new(r"SI56[\s\d]{14,26}").ok()?;
    re.find(text).map(|m| m.as_str().trim().to_string())
}

/// Find payment reference (SI + 2 digits, but NOT SI56 which is IBAN)
fn find_payment_reference(text: &str) -> String {
    let re = Regex::new(r"SI(?:0[0-9]|1[0-2])\s*[\d\s]{4,}").unwrap();
    re.find(text)
        .map(|m| {
            // Collapse multiple spaces to single space
            let s = m.as_str().trim().to_string();
            let ws = Regex::new(r"\s+").unwrap();
            ws.replace_all(&s, " ").trim().to_string()
        })
        .unwrap_or_default()
}

/// Search text for a due date near payment labels
fn find_due_date(text: &str) -> String {
    let patterns = [
        // Elektro: "ROK PLAČILA:\n02. 03. 2026" (diacritic č/Č)
        r"(?i)rok\s+pla[čc]ila:\s*\n?\s*(\d{2}\.\s*\d{2}\.\s*\d{4})",
        r"(?i)zapadlost:\s*(\d{2}\.\d{2}\.\d{4})",
        // ZLM: "Zapade: 1 6 .0 2 .2 0 2 6" (space-separated chars)
        r"(?i)zapade:\s*\n?\s*(\d\s*\d\s*\.\s*\d\s*\d\s*\.\s*\d\s*\d\s*\d\s*\d)",
        r"(?i)datum:\s*(\d{2}\.\d{2}\.\d{4})",
    ];
    for p in &patterns {
        if let Ok(re) = Regex::new(p) {
            if let Some(caps) = re.captures(text) {
                if let Some(m) = caps.get(1) {
                    return m.as_str().replace(' ', "").trim().to_string();
                }
            }
        }
    }
    String::new()
}

fn get_providers_inner(conn: &rusqlite::Connection) -> Vec<Provider> {
    let mut stmt = match conn.prepare(
        "SELECT id, name, service_type, creditor_name, creditor_address, creditor_city,
         creditor_postal_code, creditor_iban, purpose_code, match_pattern, amount_pattern,
         reference_pattern, due_date_pattern, invoice_number_pattern, purpose_text_template
         FROM providers ORDER BY name",
    ) {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    stmt.query_map([], |row| {
        Ok(Provider {
            id: Some(row.get(0)?),
            name: row.get(1)?,
            service_type: row.get(2)?,
            creditor_name: row.get(3)?,
            creditor_address: row.get(4)?,
            creditor_city: row.get(5)?,
            creditor_postal_code: row.get(6)?,
            creditor_iban: row.get(7)?,
            purpose_code: row.get(8)?,
            match_pattern: row.get(9)?,
            amount_pattern: row.get(10)?,
            reference_pattern: row.get(11)?,
            due_date_pattern: row.get(12)?,
            invoice_number_pattern: row.get(13)?,
            purpose_text_template: row.get(14)?,
        })
    })
    .map(|rows| rows.filter_map(|r| r.ok()).collect())
    .unwrap_or_default()
}

// ─── Billing Period Commands ────────────────────────────────────────────────

#[tauri::command]
pub fn get_billing_periods(db: State<DbState>) -> Result<Vec<BillingPeriod>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, building_id, month, year, status, created_at
             FROM billing_periods ORDER BY year DESC, month DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(BillingPeriod {
                id: Some(row.get(0)?),
                building_id: row.get(1)?,
                month: row.get(2)?,
                year: row.get(3)?,
                status: row.get(4)?,
                created_at: row.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

#[tauri::command]
pub fn create_billing_period(
    db: State<DbState>,
    month: i32,
    year: i32,
) -> Result<BillingPeriod, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT OR IGNORE INTO billing_periods (building_id, month, year, status)
         VALUES (1, ?1, ?2, 'draft')",
        params![month, year],
    )
    .map_err(|e| e.to_string())?;
    let id = conn.last_insert_rowid();
    // If INSERT was ignored (duplicate), fetch existing
    let id = if id == 0 {
        conn.query_row(
            "SELECT id FROM billing_periods WHERE building_id=1 AND month=?1 AND year=?2",
            params![month, year],
            |r| r.get::<_, i64>(0),
        )
        .map_err(|e| e.to_string())?
    } else {
        id
    };
    conn.query_row(
        "SELECT id, building_id, month, year, status, created_at FROM billing_periods WHERE id=?1",
        [id],
        |row| {
            Ok(BillingPeriod {
                id: Some(row.get(0)?),
                building_id: row.get(1)?,
                month: row.get(2)?,
                year: row.get(3)?,
                status: row.get(4)?,
                created_at: row.get(5)?,
            })
        },
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_billing_period(db: State<DbState>, id: i64) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    // Cascade: delete splits → bills → period
    conn.execute(
        "DELETE FROM bill_splits WHERE bill_id IN (SELECT id FROM bills WHERE billing_period_id=?1)",
        [id],
    )
    .map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM bills WHERE billing_period_id=?1", [id])
        .map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM billing_periods WHERE id=?1", [id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn create_year_periods(db: State<DbState>, year: i32) -> Result<Vec<BillingPeriod>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    for month in 1..=12 {
        conn.execute(
            "INSERT OR IGNORE INTO billing_periods (building_id, month, year, status) VALUES (1, ?1, ?2, 'draft')",
            params![month, year],
        )
        .map_err(|e| e.to_string())?;
    }
    let mut stmt = conn
        .prepare(
            "SELECT id, building_id, month, year, status, created_at
             FROM billing_periods WHERE year=?1 ORDER BY month ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([year], |row| {
            Ok(BillingPeriod {
                id: Some(row.get(0)?),
                building_id: row.get(1)?,
                month: row.get(2)?,
                year: row.get(3)?,
                status: row.get(4)?,
                created_at: row.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

// ─── Bill Commands ──────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_bills(db: State<DbState>, billing_period_id: i64) -> Result<Vec<Bill>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT b.id, b.billing_period_id, b.provider_id, b.raw_text, b.amount_cents,
             b.creditor_name, b.creditor_iban, b.creditor_address, b.creditor_city,
             b.creditor_postal_code, b.reference, b.due_date, b.purpose_code, b.purpose_text,
             b.invoice_number, b.status, b.source_filename,
             p.name as provider_name
             FROM bills b
             LEFT JOIN providers p ON b.provider_id = p.id
             WHERE b.billing_period_id = ?1
             ORDER BY b.id",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([billing_period_id], |row| {
            Ok(Bill {
                id: Some(row.get(0)?),
                billing_period_id: row.get(1)?,
                provider_id: row.get(2)?,
                raw_text: row.get(3)?,
                amount_cents: row.get(4)?,
                creditor_name: row.get(5)?,
                creditor_iban: row.get(6)?,
                creditor_address: row.get(7)?,
                creditor_city: row.get(8)?,
                creditor_postal_code: row.get(9)?,
                reference: row.get(10)?,
                due_date: row.get(11)?,
                purpose_code: row.get(12)?,
                purpose_text: row.get(13)?,
                invoice_number: row.get(14)?,
                status: row.get(15)?,
                source_filename: row.get(16)?,
                provider_name: row.get(17)?,
            })
        })
        .map_err(|e| e.to_string())?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

#[tauri::command]
pub fn save_bill(db: State<DbState>, bill: Bill) -> Result<Bill, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    match bill.id {
        Some(id) => {
            conn.execute(
                "UPDATE bills SET amount_cents=?1, creditor_name=?2, creditor_iban=?3,
                 creditor_address=?4, creditor_city=?5, creditor_postal_code=?6,
                 reference=?7, due_date=?8, purpose_code=?9, purpose_text=?10,
                 invoice_number=?11, status=?12 WHERE id=?13",
                params![
                    bill.amount_cents,
                    bill.creditor_name,
                    bill.creditor_iban,
                    bill.creditor_address,
                    bill.creditor_city,
                    bill.creditor_postal_code,
                    bill.reference,
                    bill.due_date,
                    bill.purpose_code,
                    bill.purpose_text,
                    bill.invoice_number,
                    bill.status,
                    id
                ],
            )
            .map_err(|e| e.to_string())?;
            Ok(bill)
        }
        None => {
            conn.execute(
                "INSERT INTO bills
                 (billing_period_id, provider_id, raw_text, amount_cents, creditor_name, creditor_iban,
                  creditor_address, creditor_city, creditor_postal_code, reference, due_date,
                  purpose_code, purpose_text, invoice_number, status, source_filename)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)",
                params![
                    bill.billing_period_id,
                    bill.provider_id,
                    bill.raw_text,
                    bill.amount_cents,
                    bill.creditor_name,
                    bill.creditor_iban,
                    bill.creditor_address,
                    bill.creditor_city,
                    bill.creditor_postal_code,
                    bill.reference,
                    bill.due_date,
                    bill.purpose_code,
                    bill.purpose_text,
                    bill.invoice_number,
                    bill.status,
                    bill.source_filename,
                ],
            )
            .map_err(|e| e.to_string())?;
            let id = conn.last_insert_rowid();
            Ok(Bill { id: Some(id), ..bill })
        }
    }
}

#[tauri::command]
pub fn delete_bill(db: State<DbState>, id: i64) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM bill_splits WHERE bill_id=?1", [id])
        .map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM bills WHERE id=?1", [id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Parse a PDF file and try to match it against configured providers.
/// Returns a partially-filled Bill that the user can review before saving.
#[tauri::command]
pub fn import_bill(
    db: State<DbState>,
    file_path: String,
    billing_period_id: i64,
) -> Result<Bill, String> {
    // Extract text from PDF
    let pdf_bytes = std::fs::read(&file_path).map_err(|e| e.to_string())?;
    let raw_text = pdf_extract::extract_text_from_mem(&pdf_bytes)
        .unwrap_or_default()
        .trim()
        .to_string();

    let filename = std::path::Path::new(&file_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(&file_path)
        .to_string();

    // Get billing period for month/year interpolation
    let (month, year) = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        conn.query_row(
            "SELECT month, year FROM billing_periods WHERE id=?1",
            [billing_period_id],
            |r| Ok((r.get::<_, i32>(0)?, r.get::<_, i32>(1)?)),
        )
        .map_err(|e| e.to_string())?
    };

    // Try to match against providers
    let providers = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        get_providers_inner(&conn)
    };

    let mut matched_provider: Option<&Provider> = None;
    for provider in &providers {
        if provider.match_pattern.is_empty() {
            continue;
        }
        if let Ok(re) = Regex::new(&provider.match_pattern) {
            if re.is_match(&raw_text) {
                matched_provider = Some(provider);
                break;
            }
        }
    }

    let (
        provider_id,
        amount_cents,
        reference,
        due_date,
        invoice_number,
        purpose_code,
        purpose_text,
        creditor_name,
        creditor_iban,
        creditor_address,
        creditor_city,
        creditor_postal_code,
    ) = if let Some(p) = matched_provider {
        let amount_str = first_capture(&p.amount_pattern, &raw_text).unwrap_or_default();
        let amount_cents = parse_amount_to_cents(&amount_str);
        let reference = first_capture(&p.reference_pattern, &raw_text).unwrap_or_default();
        let due_date = first_capture(&p.due_date_pattern, &raw_text).unwrap_or_default();
        let invoice_number =
            first_capture(&p.invoice_number_pattern, &raw_text).unwrap_or_default();
        let purpose_text =
            interpolate_template(&p.purpose_text_template, &invoice_number, month, year);
        (
            p.id,
            amount_cents,
            reference,
            due_date,
            invoice_number,
            p.purpose_code.clone(),
            purpose_text,
            p.creditor_name.clone(),
            p.creditor_iban.clone(),
            p.creditor_address.clone(),
            p.creditor_city.clone(),
            p.creditor_postal_code.clone(),
        )
    } else {
        (None, 0, String::new(), String::new(), String::new(), "OTHR".to_string(), String::new(), String::new(), String::new(), String::new(), String::new(), String::new())
    };

    // Insert into DB
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO bills
         (billing_period_id, provider_id, raw_text, amount_cents, creditor_name, creditor_iban,
          creditor_address, creditor_city, creditor_postal_code, reference, due_date,
          purpose_code, purpose_text, invoice_number, status, source_filename)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,'draft',?15)",
        params![
            billing_period_id,
            provider_id,
            raw_text,
            amount_cents,
            creditor_name,
            creditor_iban,
            creditor_address,
            creditor_city,
            creditor_postal_code,
            reference,
            due_date,
            purpose_code,
            purpose_text,
            invoice_number,
            filename
        ],
    )
    .map_err(|e| e.to_string())?;

    let id = conn.last_insert_rowid();
    let provider_name = matched_provider.map(|p| p.name.clone());

    Ok(Bill {
        id: Some(id),
        billing_period_id,
        provider_id,
        raw_text: String::new(), // don't send raw text back
        amount_cents,
        creditor_name,
        creditor_iban,
        creditor_address,
        creditor_city,
        creditor_postal_code,
        reference,
        due_date,
        purpose_code,
        purpose_text,
        invoice_number,
        status: "draft".to_string(),
        source_filename: filename,
        provider_name,
    })
}

// ─── Smart multi-bill parser ───────────────────────────────────────────────

struct ExtractedBill {
    iban_norm: String,
    iban_raw: String,
    amount_cents: i64,
    reference: String,
    due_date: String,
    purpose_code: String,
    purpose_text: String,
    invoice_number: String,
}

/// Parse all UPN payment stubs (***amount sections) from PDF text.
/// Each stub has: ***amount [PURPOSECODE text], then IBAN, then reference.
/// Bills with QR codes print this stub as human-readable text alongside the QR.
fn parse_upn_stubs(text: &str) -> Vec<ExtractedBill> {
    let stub_re = match Regex::new(r"\*{2,}(\d+[.,]\d{2})") {
        Ok(r) => r,
        Err(_) => return vec![],
    };
    let purpose_code_re = Regex::new(r"\b(ENRG|SCVE|WTER|OTHR|RENT|SALA)\b").unwrap();
    let mut results: Vec<ExtractedBill> = Vec::new();
    let mut seen_ibans: std::collections::HashSet<String> = std::collections::HashSet::new();

    for m in stub_re.find_iter(text) {
        let amount_str = m.as_str().trim_matches('*');
        let amount_cents = parse_amount_to_cents(amount_str);

        // Window after stub: up to 600 chars for IBAN/reference/purpose
        let after_start = m.start();
        let after_end = (after_start + 600).min(text.len());
        let after = &text[after_start..after_end];

        // IBAN must appear after the stub marker
        let iban_raw = match find_iban(after) {
            Some(i) => i,
            None => continue,
        };
        let iban_norm = normalize_iban(&iban_raw);

        // Extract everything BEFORE dedup check so duplicate stubs can contribute data
        let reference = find_payment_reference(after);
        let stub_line_end = after.find('\n').unwrap_or(after.len());
        let search_area = &after[..stub_line_end.min(after.len())];
        let search_area2 = &after[..after.find('\n').and_then(|i| after[i+1..].find('\n').map(|j| i+1+j)).unwrap_or(after.len()).min(after.len())];

        let (purpose_code, purpose_text) = if let Some(caps) = purpose_code_re.captures(search_area) {
            let code = caps.get(1).unwrap().as_str().to_string();
            let rest = search_area[caps.get(1).unwrap().end()..].trim().to_string();
            (code, rest)
        } else if let Some(caps) = purpose_code_re.captures(search_area2) {
            let code = caps.get(1).unwrap().as_str().to_string();
            let rest = search_area2[caps.get(1).unwrap().end()..].trim().to_string();
            (code, rest)
        } else {
            ("OTHR".to_string(), String::new())
        };

        let before_start = m.start().saturating_sub(500);
        let context = &text[before_start..after_end];
        let stub_offset_in_context = m.start() - before_start;
        let parsed_from_context =
            extract_upn_purpose_from_context(context, stub_offset_in_context, &purpose_code_re);
        let mut due_date = find_due_date(context);
        // Fallback: date embedded in purpose text (e.g. "SCVE ... 16.02.2026")
        if due_date.is_empty() {
            if let Ok(date_re) = Regex::new(r"(\d{2}\.\d{2}\.\d{4})") {
                if let Some(caps) = date_re.captures(&purpose_text) {
                    due_date = caps.get(1).unwrap().as_str().to_string();
                }
            }
        }

        if !seen_ibans.insert(iban_norm.clone()) {
            // Duplicate stub: merge any better data into the existing entry
            if let Some(existing) = results.iter_mut().find(|b| b.iban_norm == iban_norm) {
                if existing.due_date.is_empty() && !due_date.is_empty() {
                    existing.due_date = due_date;
                }
                if let Some((context_code, context_text)) = &parsed_from_context {
                    existing.purpose_code = context_code.clone();
                    existing.purpose_text = context_text.clone();
                } else if existing.purpose_text.is_empty() && !purpose_text.is_empty() {
                    existing.purpose_text = purpose_text;
                }
                if existing.purpose_code == "OTHR" && purpose_code != "OTHR" {
                    existing.purpose_code = purpose_code;
                }
            }
            continue;
        }

        let (purpose_code, purpose_text) = parsed_from_context
            .unwrap_or((purpose_code, purpose_text));

        results.push(ExtractedBill {
            iban_norm,
            iban_raw,
            amount_cents,
            reference,
            due_date,
            purpose_code,
            purpose_text,
            invoice_number: String::new(),
        });
    }
    results
}

/// Parse Elektro energija-style bills (no QR code, narrative format).
fn parse_elektro_style(text: &str) -> Option<ExtractedBill> {
    // Amount on its own line after "ZA PLAČILO Z DDV:" (PDF preserves diacritic Č)
    let amount_re = Regex::new(r"ZA PLAČILO Z DDV:\s*\n\s*(\d+[.,]\d{2})").ok()?;
    let amount_cents = parse_amount_to_cents(amount_re.captures(text)?.get(1)?.as_str());

    // IBAN from "IBAN: SI56 ..." — take the first match (Elektro's own IBAN)
    let iban_re = Regex::new(r"IBAN:\s+(SI56[\s\d]+)").ok()?;
    let iban_raw = iban_re.captures(text)?.get(1)?.as_str().trim().to_string();
    let iban_norm = normalize_iban(&iban_raw);

    // Reference from "Referenca: SI12 ..."
    let ref_re = Regex::new(r"Referenca:\s+(SI\d{2}\s*\d+)").ok()?;
    let reference = ref_re.captures(text)?.get(1)?.as_str().trim().to_string();

    // Due date
    let due_date = find_due_date(text);

    // Invoice number from "Račun številka: IR..." (diacritics preserved)
    let inv_re = Regex::new(r"R[ae][čc]un [šs]tevilka:\s*(\S+)").ok()?;
    let invoice_number = inv_re.captures(text)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .unwrap_or_default();

    Some(ExtractedBill {
        iban_norm,
        iban_raw,
        amount_cents,
        reference,
        due_date,
        purpose_code: "ENRG".to_string(),
        purpose_text: String::new(), // will use template
        invoice_number,
    })
}

/// Parse ZLM-style bills (different layout, no *** stub, uses "Za plačilo EUR:").
fn parse_zlm_style(text: &str) -> Option<ExtractedBill> {
    // Amount from "Za plačilo EUR: 139,28" (PDF preserves diacritic č)
    let amount_re = Regex::new(r"Za plačilo EUR:\s*(\d+[.,]\d{2})").ok()?;
    let amount_cents = parse_amount_to_cents(amount_re.captures(text)?.get(1)?.as_str());

    // IBAN from "TRR:SI5 6  0 2 0 1 ..." — chars space-separated, grab to EOL and normalize
    let iban_re = Regex::new(r"TRR:([A-Z0-9][\sA-Z0-9]+)").ok()?;
    let iban_dirty = iban_re.captures(text)?.get(1)?.as_str();
    let iban_dirty_line = iban_dirty.lines().next().unwrap_or(iban_dirty);
    let iban_norm = normalize_iban(iban_dirty_line);
    let iban_raw = iban_norm.clone();

    // Reference from "Referenca: SI0 0  2 0 2 6 8 5" — space-separated chars.
    // Require a space within the SI model code (e.g. "SI0 0") to avoid matching
    // Elektro's "Referenca: SI12 9015175242273" which appears earlier in the PDF.
    let ref_re = Regex::new(r"Referenca:\s+(SI\d\s+\d[\s\d]*)").ok()?;
    let ref_dirty = ref_re.captures(text)?.get(1)?.as_str();
    let ref_dirty_line = ref_dirty.lines().next().unwrap_or(ref_dirty);
    let ref_norm: String = ref_dirty_line.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_uppercase();
    // Format as "SI00 202685"
    let reference = if ref_norm.len() > 4 {
        format!("{} {}", &ref_norm[..4], &ref_norm[4..])
    } else {
        ref_norm
    };

    // Due date
    let due_date = find_due_date(text);

    // Invoice from "Številka: 2026-85"
    let inv_re = Regex::new(r"[ŠS]tevilka:\s*(\d{4}-\d+)").ok()?;
    let invoice_number = inv_re.captures(text)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .unwrap_or_default();

    Some(ExtractedBill {
        iban_norm,
        iban_raw,
        amount_cents,
        reference,
        due_date,
        purpose_code: "OTHR".to_string(),
        purpose_text: String::new(), // will use template
        invoice_number,
    })
}

/// Import a PDF that may contain multiple bills.
/// Uses smart parsing: finds UPN payment stubs (***amount), falls back to
/// Elektro narrative format and ZLM format. Matches providers by IBAN.
#[tauri::command]
pub fn import_bills(
    db: State<DbState>,
    file_path: String,
    billing_period_id: i64,
) -> Result<Vec<Bill>, String> {
    let pdf_bytes = std::fs::read(&file_path).map_err(|e| e.to_string())?;
    let raw_text = pdf_extract::extract_text_from_mem(&pdf_bytes)
        .unwrap_or_default()
        .trim()
        .to_string();

    let filename = std::path::Path::new(&file_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(&file_path)
        .to_string();

    let (month, year) = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        conn.query_row(
            "SELECT month, year FROM billing_periods WHERE id=?1",
            [billing_period_id],
            |r| Ok((r.get::<_, i32>(0)?, r.get::<_, i32>(1)?)),
        )
        .map_err(|e| e.to_string())?
    };

    let providers = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        get_providers_inner(&conn)
    };

    // Build IBAN → provider map (normalized, no spaces)
    let provider_by_iban: std::collections::HashMap<String, &Provider> = providers
        .iter()
        .filter(|p| !p.creditor_iban.is_empty())
        .map(|p| (normalize_iban(&p.creditor_iban), p))
        .collect();

    // Write debug log: raw extracted text + parse results
    let log_path = dirs_next::data_dir()
        .map(|d| d.join("si.upn-generator").join("import_debug.log"));
    let mut log = format!("=== import_bills: {} ===\n\n--- RAW TEXT ---\n{}\n\n--- PARSE RESULTS ---\n", filename, raw_text);

    // --- Collect extracted bills ---
    let mut extracted: Vec<ExtractedBill> = Vec::new();
    let mut seen_ibans: std::collections::HashSet<String> = std::collections::HashSet::new();

    // Phase 1: UPN payment stubs (***amount) — covers VOKA ×2 and Energetika
    let stubs = parse_upn_stubs(&raw_text);
    log.push_str(&format!("Phase 1 (UPN stubs): {} found\n", stubs.len()));
    for bill in stubs {
        log.push_str(&format!("  IBAN={} amount={} ref={} due={}\n", bill.iban_raw, bill.amount_cents, bill.reference, bill.due_date));
        if seen_ibans.insert(bill.iban_norm.clone()) {
            extracted.push(bill);
        }
    }

    // Phase 2: Elektro narrative format (ZA PLACILO Z DDV:)
    let elektro = parse_elektro_style(&raw_text);
    log.push_str(&format!("Phase 2 (Elektro): {}\n", if elektro.is_some() { "found" } else { "NOT FOUND" }));
    if let Some(bill) = elektro {
        log.push_str(&format!("  IBAN={} amount={} ref={} due={}\n", bill.iban_raw, bill.amount_cents, bill.reference, bill.due_date));
        if seen_ibans.insert(bill.iban_norm.clone()) {
            extracted.push(bill);
        }
    }

    // Phase 3: ZLM format (Za placilo EUR: + TRR:)
    let zlm = parse_zlm_style(&raw_text);
    log.push_str(&format!("Phase 3 (ZLM): {}\n", if zlm.is_some() { "found" } else { "NOT FOUND" }));
    if let Some(bill) = zlm {
        log.push_str(&format!("  IBAN={} amount={} ref={} due={}\n", bill.iban_raw, bill.amount_cents, bill.reference, bill.due_date));
        if seen_ibans.insert(bill.iban_norm.clone()) {
            extracted.push(bill);
        }
    }

    if let Some(ref path) = log_path {
        let _ = std::fs::write(path, &log);
    }

    // Fallback: nothing found — create one blank bill
    if extracted.is_empty() {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO bills (billing_period_id, provider_id, raw_text, amount_cents,
             creditor_name, creditor_iban, creditor_address, creditor_city,
             creditor_postal_code, reference, due_date, purpose_code, purpose_text,
             invoice_number, status, source_filename)
             VALUES (?1,NULL,?2,0,'','','','','','','','OTHR','','','draft',?3)",
            params![billing_period_id, raw_text, filename],
        )
        .map_err(|e| e.to_string())?;
        let id = conn.last_insert_rowid();
        return Ok(vec![Bill {
            id: Some(id),
            billing_period_id,
            provider_id: None,
            raw_text: String::new(),
            amount_cents: 0,
            creditor_name: String::new(),
            creditor_iban: String::new(),
            creditor_address: String::new(),
            creditor_city: String::new(),
            creditor_postal_code: String::new(),
            reference: String::new(),
            due_date: String::new(),
            purpose_code: "OTHR".to_string(),
            purpose_text: String::new(),
            invoice_number: String::new(),
            status: "draft".to_string(),
            source_filename: filename,
            provider_name: None,
        }]);
    }

    // --- Match to providers and insert ---
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut results: Vec<Bill> = Vec::new();

    for eb in extracted {
        let provider = provider_by_iban.get(&eb.iban_norm).copied();

        // Determine creditor info from provider (if matched) or from extracted IBAN
        let (provider_id, creditor_name, creditor_iban, creditor_address,
             creditor_city, creditor_postal_code, purpose_code) = match provider {
            Some(p) => (
                p.id,
                p.creditor_name.clone(),
                p.creditor_iban.clone(),
                p.creditor_address.clone(),
                p.creditor_city.clone(),
                p.creditor_postal_code.clone(),
                if eb.purpose_code != "OTHR" { eb.purpose_code.clone() } else { p.purpose_code.clone() },
            ),
            None => (
                None,
                String::new(),
                eb.iban_raw.clone(),
                String::new(),
                String::new(),
                String::new(),
                eb.purpose_code.clone(),
            ),
        };

        // Purpose text: use extracted text if non-empty, else use provider template
        let purpose_text = if !eb.purpose_text.is_empty() {
            eb.purpose_text.clone()
        } else if let Some(p) = provider {
            interpolate_template(&p.purpose_text_template, &eb.invoice_number, month, year)
        } else {
            String::new()
        };

        conn.execute(
            "INSERT INTO bills (billing_period_id, provider_id, raw_text, amount_cents,
             creditor_name, creditor_iban, creditor_address, creditor_city,
             creditor_postal_code, reference, due_date, purpose_code, purpose_text,
             invoice_number, status, source_filename)
             VALUES (?1,?2,'',?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,'draft',?14)",
            params![
                billing_period_id, provider_id, eb.amount_cents,
                creditor_name, creditor_iban, creditor_address,
                creditor_city, creditor_postal_code,
                eb.reference, eb.due_date, purpose_code, purpose_text,
                eb.invoice_number, filename,
            ],
        )
        .map_err(|e| e.to_string())?;

        let id = conn.last_insert_rowid();
        results.push(Bill {
            id: Some(id),
            billing_period_id,
            provider_id,
            raw_text: String::new(),
            amount_cents: eb.amount_cents,
            creditor_name,
            creditor_iban,
            creditor_address,
            creditor_city,
            creditor_postal_code,
            reference: eb.reference,
            due_date: eb.due_date,
            purpose_code,
            purpose_text,
            invoice_number: eb.invoice_number,
            status: "draft".to_string(),
            source_filename: filename.clone(),
            provider_name: provider.map(|p| p.name.clone()),
        });
    }

    Ok(results)
}
