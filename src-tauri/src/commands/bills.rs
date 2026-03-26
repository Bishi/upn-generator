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

fn interpolate_template(template: &str, invoice_number: &str, month: i32, year: i32) -> String {
    template
        .replace("{invoice_number}", invoice_number)
        .replace("{month}", &format!("{:02}", month))
        .replace("{year}", &year.to_string())
        .replace("{MM}", &format!("{:02}", month))
        .replace("{YYYY}", &year.to_string())
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

/// Import a PDF that may contain multiple bills. Splits by provider match patterns.
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

    // Find all provider match positions in the text
    let mut matches: Vec<(usize, usize)> = Vec::new(); // (position, provider_index)
    for (idx, provider) in providers.iter().enumerate() {
        if provider.match_pattern.is_empty() {
            continue;
        }
        if let Ok(re) = Regex::new(&provider.match_pattern) {
            for m in re.find_iter(&raw_text) {
                matches.push((m.start(), idx));
            }
        }
    }
    matches.sort_by_key(|(pos, _)| *pos);

    // Deduplicate: if two providers match at overlapping positions, keep the first
    matches.dedup_by(|a, b| a.0 == b.0);

    // If no matches, create one unmatched bill with the full text
    if matches.is_empty() {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO bills
             (billing_period_id, provider_id, raw_text, amount_cents, creditor_name, creditor_iban,
              creditor_address, creditor_city, creditor_postal_code, reference, due_date,
              purpose_code, purpose_text, invoice_number, status, source_filename)
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

    // Split text into segments by match positions and parse each
    let mut results: Vec<Bill> = Vec::new();
    let conn = db.0.lock().map_err(|e| e.to_string())?;

    for (i, &(start_pos, provider_idx)) in matches.iter().enumerate() {
        let end_pos = if i + 1 < matches.len() {
            matches[i + 1].0
        } else {
            raw_text.len()
        };
        let segment = &raw_text[start_pos..end_pos];
        let provider = &providers[provider_idx];

        let amount_str = first_capture(&provider.amount_pattern, segment).unwrap_or_default();
        let amount_cents = parse_amount_to_cents(&amount_str);
        let reference = first_capture(&provider.reference_pattern, segment).unwrap_or_default();
        let due_date = first_capture(&provider.due_date_pattern, segment).unwrap_or_default();
        let invoice_number =
            first_capture(&provider.invoice_number_pattern, segment).unwrap_or_default();
        let purpose_text =
            interpolate_template(&provider.purpose_text_template, &invoice_number, month, year);

        conn.execute(
            "INSERT INTO bills
             (billing_period_id, provider_id, raw_text, amount_cents, creditor_name, creditor_iban,
              creditor_address, creditor_city, creditor_postal_code, reference, due_date,
              purpose_code, purpose_text, invoice_number, status, source_filename)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,'draft',?15)",
            params![
                billing_period_id,
                provider.id,
                segment,
                amount_cents,
                provider.creditor_name,
                provider.creditor_iban,
                provider.creditor_address,
                provider.creditor_city,
                provider.creditor_postal_code,
                reference,
                due_date,
                provider.purpose_code,
                purpose_text,
                invoice_number,
                filename,
            ],
        )
        .map_err(|e| e.to_string())?;

        let id = conn.last_insert_rowid();
        results.push(Bill {
            id: Some(id),
            billing_period_id,
            provider_id: provider.id,
            raw_text: String::new(),
            amount_cents,
            creditor_name: provider.creditor_name.clone(),
            creditor_iban: provider.creditor_iban.clone(),
            creditor_address: provider.creditor_address.clone(),
            creditor_city: provider.creditor_city.clone(),
            creditor_postal_code: provider.creditor_postal_code.clone(),
            reference,
            due_date,
            purpose_code: provider.purpose_code.clone(),
            purpose_text,
            invoice_number,
            status: "draft".to_string(),
            source_filename: filename.clone(),
            provider_name: Some(provider.name.clone()),
        });
    }

    Ok(results)
}
