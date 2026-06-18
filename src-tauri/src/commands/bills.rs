use regex::Regex;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;
use tauri::State;

use super::config::{DbState, Provider};

#[cfg(target_os = "windows")]
use windows::{
    Graphics::Imaging::{BitmapDecoder, SoftwareBitmap},
    Media::Ocr::OcrEngine,
    Storage::{FileAccessMode, StorageFile},
};

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
    pub parse_note: String,
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

fn normalize_ocr_alnum(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

fn supported_image_extensions() -> &'static [&'static str] {
    &["jpg", "jpeg", "png", "bmp", "tif", "tiff"]
}

fn is_supported_image_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            supported_image_extensions()
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(ext))
        })
        .unwrap_or(false)
}

fn extract_text_from_pdf(file_path: &str) -> Result<String, String> {
    let pdf_bytes = std::fs::read(file_path).map_err(|e| e.to_string())?;
    Ok(pdf_extract::extract_text_from_mem(&pdf_bytes)
        .unwrap_or_default()
        .trim()
        .to_string())
}

#[cfg(target_os = "windows")]
fn extract_text_from_image(file_path: &str) -> Result<String, String> {
    let path = file_path.to_string();
    let (tx, rx) = mpsc::channel();

    std::thread::spawn(move || {
        let result = (|| -> Result<String, String> {
            let file = StorageFile::GetFileFromPathAsync(&path.into())
                .map_err(|e| e.to_string())?
                .get()
                .map_err(|e| e.to_string())?;
            let stream = file
                .OpenAsync(FileAccessMode::Read)
                .map_err(|e| e.to_string())?
                .get()
                .map_err(|e| e.to_string())?;
            let decoder = BitmapDecoder::CreateAsync(&stream)
                .map_err(|e| e.to_string())?
                .get()
                .map_err(|e| e.to_string())?;
            let bitmap = decoder
                .GetSoftwareBitmapAsync()
                .map_err(|e| e.to_string())?
                .get()
                .map_err(|e| e.to_string())?;
            let bitmap = SoftwareBitmap::Convert(
                &bitmap,
                windows::Graphics::Imaging::BitmapPixelFormat::Bgra8,
            )
            .map_err(|e| e.to_string())?;
            let engine =
                OcrEngine::TryCreateFromUserProfileLanguages().map_err(|e| e.to_string())?;
            let result = engine
                .RecognizeAsync(&bitmap)
                .map_err(|e| e.to_string())?
                .get()
                .map_err(|e| e.to_string())?;
            let text = result.Text().map_err(|e| e.to_string())?.to_string_lossy();
            Ok(text.trim().to_string())
        })();

        let _ = tx.send(result);
    });

    match rx.recv_timeout(Duration::from_secs(20)) {
        Ok(result) => result,
        Err(mpsc::RecvTimeoutError::Timeout) => Err(
            "Image OCR timed out after 20 seconds. Try a smaller/clearer image or import a PDF."
                .to_string(),
        ),
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            Err("Image OCR worker stopped unexpectedly.".to_string())
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn extract_text_from_image(_file_path: &str) -> Result<String, String> {
    Err("Image bill import is only supported on Windows builds.".to_string())
}

fn extract_text_from_file(file_path: &str) -> Result<String, String> {
    let path = Path::new(file_path);
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .unwrap_or_default();

    match extension.as_str() {
        "pdf" => extract_text_from_pdf(file_path),
        _ if is_supported_image_file(path) => extract_text_from_image(file_path),
        _ => Err(format!(
            "Unsupported bill file type: {}. Supported files: PDF, JPG, JPEG, PNG, BMP, TIF, TIFF.",
            extension
        )),
    }
}

fn extract_upn_purpose_from_context(
    context: &str,
    stub_offset_in_context: usize,
    purpose_code_re: &Regex,
) -> Option<(String, String)> {
    let normalized_context = normalize_spaces(context);
    if let Ok(energetika_re) =
        Regex::new(r"(?i)ra\S*un\s+\S*t\.?\s*([A-Z0-9-]+)\s+([0-9]{5,})(?:/\d{2}\.\d{2}\.\d{4})?")
    {
        if let Some(caps) = energetika_re.captures(&normalized_context) {
            let invoice = caps.get(1).map(|m| m.as_str()).unwrap_or_default();
            let partner = caps.get(2).map(|m| m.as_str()).unwrap_or_default();
            if !invoice.is_empty() && !partner.is_empty() {
                return Some((
                    "ENRG".to_string(),
                    format!("RAČUN ŠT. {} {}", invoice, partner),
                ));
            }
        }
    }

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

fn first_date_in_text(text: &str) -> String {
    Regex::new(r"(\d{2}\.\d{2}\.\d{4})")
        .ok()
        .and_then(|re| {
            re.captures(text)
                .and_then(|caps| caps.get(1).map(|m| m.as_str().to_string()))
        })
        .unwrap_or_default()
}

fn find_source_period_month_year(text: &str) -> Option<(i32, i32)> {
    let compact = text.replace(' ', "");
    let range_re = Regex::new(r"(\d{2})\.(\d{2})\.(\d{4})[-–](\d{2})\.(\d{2})\.(\d{4})").ok()?;
    if let Some(caps) = range_re.captures(&compact) {
        let month = caps.get(2)?.as_str().parse::<i32>().ok()?;
        let year = caps.get(3)?.as_str().parse::<i32>().ok()?;
        return Some((month, year));
    }

    let month_word_re = Regex::new(
        r"(?i)\b(JANUAR|FEBRUAR|MAREC|APRIL|MAJ|JUNIJ|JULIJ|AVGUST|SEPTEMBER|OKTOBER|NOVEMBER|DECEMBER)\s+(\d{4})\b",
    )
    .ok()?;
    let caps = month_word_re.captures(text)?;
    let month_name = caps.get(1)?.as_str().to_uppercase();
    let year = caps.get(2)?.as_str().parse::<i32>().ok()?;
    let month = match month_name.as_str() {
        "JANUAR" => 1,
        "FEBRUAR" => 2,
        "MAREC" => 3,
        "APRIL" => 4,
        "MAJ" => 5,
        "JUNIJ" => 6,
        "JULIJ" => 7,
        "AVGUST" => 8,
        "SEPTEMBER" => 9,
        "OKTOBER" => 10,
        "NOVEMBER" => 11,
        "DECEMBER" => 12,
        _ => return None,
    };
    Some((month, year))
}

fn missing_payment_field_note(
    amount_cents: i64,
    creditor_iban: &str,
    reference: &str,
    due_date: &str,
) -> String {
    let mut missing = Vec::new();
    if amount_cents <= 0 {
        missing.push("amount");
    }
    if creditor_iban.trim().is_empty() {
        missing.push("creditor IBAN");
    }
    if reference.trim().is_empty() {
        missing.push("reference");
    }
    if due_date.trim().is_empty() {
        missing.push("due date");
    }

    if missing.is_empty() {
        return String::new();
    }

    format!(
        "Missing required payment field{}: {}. Review this import before calculating splits or generating UPNs.",
        if missing.len() == 1 { "" } else { "s" },
        missing.join(", ")
    )
}

fn append_parse_note(existing: &str, additional: &str) -> String {
    let existing = existing.trim();
    let additional = additional.trim();
    match (existing.is_empty(), additional.is_empty()) {
        (true, true) => String::new(),
        (false, true) => existing.to_string(),
        (true, false) => additional.to_string(),
        (false, false) => format!("{existing} {additional}"),
    }
}

fn import_review_parse_note(
    existing_note: &str,
    amount_cents: i64,
    creditor_iban: &str,
    reference: &str,
    due_date: &str,
) -> String {
    append_parse_note(
        existing_note,
        &missing_payment_field_note(amount_cents, creditor_iban, reference, due_date),
    )
}

fn get_providers_inner(conn: &rusqlite::Connection) -> Vec<Provider> {
    let mut stmt = match conn.prepare(
        "SELECT id, name, service_type, creditor_name, creditor_address, creditor_city,
         creditor_postal_code, creditor_iban, purpose_code, match_pattern, amount_pattern,
         reference_pattern, due_date_pattern, invoice_number_pattern, purpose_text_template,
         split_basis
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
            split_basis: row.get(15)?,
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
    conn.query_row(
        "SELECT id, building_id, month, year, status, created_at
         FROM billing_periods WHERE building_id=1 AND month=?1 AND year=?2",
        params![month, year],
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
        "DELETE FROM upn_delivery_events WHERE billing_period_id=?1",
        [id],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM bill_splits WHERE bill_id IN (SELECT id FROM bills WHERE billing_period_id=?1)",
        [id],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM inbox_bill_hashes WHERE billing_period_id=?1",
        [id],
    )
    .map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM bills WHERE billing_period_id=?1", [id])
        .map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM inbox_imports WHERE billing_period_id=?1", [id])
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
             b.invoice_number, b.parse_note, b.status, b.source_filename,
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
                parse_note: row.get(15)?,
                status: row.get(16)?,
                source_filename: row.get(17)?,
                provider_name: row.get(18)?,
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
                 invoice_number=?11, parse_note=?12, status=?13 WHERE id=?14",
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
                    bill.parse_note,
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
                  purpose_code, purpose_text, invoice_number, parse_note, status, source_filename)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)",
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
                    bill.parse_note,
                    bill.status,
                    bill.source_filename,
                ],
            )
            .map_err(|e| e.to_string())?;
            let id = conn.last_insert_rowid();
            Ok(Bill {
                id: Some(id),
                ..bill
            })
        }
    }
}

fn delete_inbox_imports_for_bill(conn: &Connection, bill_id: i64) -> Result<(), String> {
    let import_ids = {
        let mut stmt = conn
            .prepare("SELECT id, bill_ids FROM inbox_imports WHERE status='imported'")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| e.to_string())?;
        let mut import_ids = Vec::new();
        for row in rows {
            let (import_id, bill_ids_json) = row.map_err(|e| e.to_string())?;
            if serde_json::from_str::<Vec<i64>>(&bill_ids_json)
                .map(|bill_ids| bill_ids.contains(&bill_id))
                .unwrap_or(false)
            {
                import_ids.push(import_id);
            }
        }
        import_ids
    };

    for import_id in import_ids {
        conn.execute("DELETE FROM inbox_imports WHERE id=?1", [import_id])
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn delete_bill(db: State<DbState>, id: i64) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM bill_splits WHERE bill_id=?1", [id])
        .map_err(|e| e.to_string())?;
    delete_inbox_imports_for_bill(&conn, id)?;
    conn.execute("DELETE FROM inbox_bill_hashes WHERE bill_id=?1", [id])
        .map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM bills WHERE id=?1", [id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Parse a bill file and try to match it against configured providers.
/// Returns a partially-filled Bill that the user can review before saving.
#[tauri::command]
pub fn import_bill(
    db: State<DbState>,
    file_path: String,
    billing_period_id: i64,
) -> Result<Bill, String> {
    let raw_text = extract_text_from_file(&file_path)?;

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
    let (source_month, source_year) =
        find_source_period_month_year(&raw_text).unwrap_or((month, year));

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
        let purpose_text = interpolate_template(
            &p.purpose_text_template,
            &invoice_number,
            source_month,
            source_year,
        );
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
        (
            None,
            0,
            String::new(),
            String::new(),
            String::new(),
            "OTHR".to_string(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
        )
    };

    let parse_note =
        import_review_parse_note("", amount_cents, &creditor_iban, &reference, &due_date);
    let status = if parse_note.is_empty() {
        "draft".to_string()
    } else {
        "needs_review".to_string()
    };

    // Insert into DB
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO bills
         (billing_period_id, provider_id, raw_text, amount_cents, creditor_name, creditor_iban,
          creditor_address, creditor_city, creditor_postal_code, reference, due_date,
          purpose_code, purpose_text, invoice_number, parse_note, status, source_filename)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)",
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
            parse_note,
            status,
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
        parse_note,
        status,
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
    parse_note: String,
}

/// Parse all UPN payment stubs (***amount sections) from PDF text.
/// Each stub has: ***amount [PURPOSECODE text], then IBAN, then reference.
/// Bills with QR codes print this stub as human-readable text alongside the QR.
fn parse_upn_stubs(text: &str) -> Vec<ExtractedBill> {
    let stub_re = match Regex::new(r"\*{2,}(\d+[.,]\d{2})") {
        Ok(r) => r,
        Err(_) => return vec![],
    };
    let purpose_code_re = Regex::new(r"\b(ENRG|SCVE|WTER|OTHR|RENT|SALA|COST)\b").unwrap();
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
        let search_area2 = &after[..after
            .find('\n')
            .and_then(|i| after[i + 1..].find('\n').map(|j| i + 1 + j))
            .unwrap_or(after.len())
            .min(after.len())];

        let (purpose_code, purpose_text) = if let Some(caps) = purpose_code_re.captures(search_area)
        {
            let code = caps.get(1).unwrap().as_str().to_string();
            let rest = search_area[caps.get(1).unwrap().end()..].trim().to_string();
            (code, rest)
        } else if let Some(caps) = purpose_code_re.captures(search_area2) {
            let code = caps.get(1).unwrap().as_str().to_string();
            let rest = search_area2[caps.get(1).unwrap().end()..]
                .trim()
                .to_string();
            (code, rest)
        } else {
            ("OTHR".to_string(), String::new())
        };

        let before_start = m.start().saturating_sub(500);
        let context = &text[before_start..after_end];
        let stub_offset_in_context = m.start() - before_start;
        let parsed_from_context =
            extract_upn_purpose_from_context(context, stub_offset_in_context, &purpose_code_re);
        let mut due_date = parsed_from_context
            .as_ref()
            .map(|(_, context_text)| first_date_in_text(context_text))
            .unwrap_or_default();
        if due_date.is_empty() {
            due_date = find_due_date(context);
        }
        if due_date.is_empty() {
            due_date = first_date_in_text(&purpose_text);
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

        let (purpose_code, purpose_text) =
            parsed_from_context.unwrap_or((purpose_code, purpose_text));

        results.push(ExtractedBill {
            iban_norm,
            iban_raw,
            amount_cents,
            reference,
            due_date,
            purpose_code,
            purpose_text,
            invoice_number: String::new(),
            parse_note: String::new(),
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
    let invoice_number = inv_re
        .captures(text)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .unwrap_or_default();
    let invoice_number = if invoice_number.is_empty() {
        Regex::new(r"(?i)ra\S*un\s+\S*tevilka:\s*([A-Z0-9-]+)")
            .ok()
            .and_then(|re| re.captures(text))
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string())
            .unwrap_or_default()
    } else {
        invoice_number
    };

    Some(ExtractedBill {
        iban_norm,
        iban_raw,
        amount_cents,
        reference,
        due_date,
        purpose_code: "ENRG".to_string(),
        purpose_text: String::new(), // will use template
        invoice_number,
        parse_note: String::new(),
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
    let ref_norm: String = ref_dirty_line
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>()
        .to_uppercase();
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
    let invoice_number = inv_re
        .captures(text)
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
        parse_note: String::new(),
    })
}

/// Parse OCR'd Dimnikarstvo Energetski Servis bills.
/// These image imports often lose the exact UPN stub formatting, so we match
/// the provider-specific cues directly and fall back to the known provider IBAN.
fn parse_dimnikar_style(text: &str) -> Option<ExtractedBill> {
    let normalized = normalize_spaces(text);
    let normalized_ocr = normalize_ocr_alnum(&normalized);
    let looks_like_dimnikar = normalized_ocr.contains("DIMNIK")
        && (normalized_ocr.contains("SERVIS")
            || normalized_ocr.contains("SERV")
            || normalized_ocr.contains("SERVQS"))
        && (normalized_ocr.contains("11042026")
            || normalized_ocr.contains("5243585")
            || normalized_ocr.contains("ANDREJABITENCA"));
    if !looks_like_dimnikar {
        return None;
    }

    let amount_cents = if let Some(caps) = Regex::new(r"[*•·]{2,}\s*(\d+[.,]\d{2})")
        .ok()?
        .captures(text)
    {
        parse_amount_to_cents(caps.get(1)?.as_str())
    } else {
        let amount_re =
            Regex::new(r"(?i)(?:skup[a-z]*\s+za\s+pla[a-z]*\s*(?:eur)?|eur\s+c(?:ost|osc))\s*([0-9]+[.,][0-9]{2})")
                .ok()?;
        parse_amount_to_cents(amount_re.captures(&normalized)?.get(1)?.as_str())
    };

    let invoice_number = Regex::new(r"(\d{3,5}[-—–]\d{4})")
        .ok()?
        .captures(text)?
        .get(1)?
        .as_str()
        .replace(['—', '–'], "-")
        .to_string();

    let due_date = if let Some(caps) = Regex::new(r"(?i)rok[^0-9]{0,20}(\d{2}\.\d{2}\.\d{4})")
        .ok()?
        .captures(&normalized)
    {
        caps.get(1)?.as_str().to_string()
    } else {
        Regex::new(r"(\d{2}\.\d{2}\.\d{4})")
            .ok()?
            .captures_iter(text)
            .nth(1)
            .and_then(|caps| caps.get(1).map(|m| m.as_str().to_string()))
            .unwrap_or_default()
    };

    let invoice_digits = invoice_number.replace('-', "");
    let reference_digits = Regex::new(r"0{4,}\d{7,}")
        .ok()
        .and_then(|re| {
            let candidates: Vec<String> = re
                .find_iter(&normalized_ocr)
                .map(|m| m.as_str().to_string())
                .collect();

            candidates
                .iter()
                .filter(|candidate| candidate.contains(&invoice_digits))
                .min_by_key(|candidate| candidate.len())
                .cloned()
                .or_else(|| {
                    normalized_ocr.rfind("UPNQR").and_then(|idx| {
                        let after_marker = &normalized_ocr[idx..];
                        re.find_iter(after_marker)
                            .map(|m| m.as_str().to_string())
                            .min_by_key(|candidate| candidate.len())
                    })
                })
                .or_else(|| {
                    candidates
                        .into_iter()
                        .min_by_key(|candidate| candidate.len())
                })
        })
        .unwrap_or_else(|| format!("0000{}", invoice_digits));
    let reference_model = Regex::new(r"(?i)SI\s*([01][0-9])")
        .ok()
        .and_then(|re| re.captures(&normalized))
        .and_then(|caps| caps.get(1).map(|m| m.as_str().to_string()))
        .unwrap_or_else(|| "12".to_string());
    let reference = format!("SI{} {}", reference_model, reference_digits);
    let expected_reference_prefix = format!("0000{}", invoice_digits);
    let high_confidence_reference = reference_model == "12"
        && reference_digits.starts_with(&expected_reference_prefix)
        && reference_digits.len() <= expected_reference_prefix.len() + 2;
    let parse_note = if amount_cents > 0 && !due_date.is_empty() && high_confidence_reference {
        String::new()
    } else {
        "Parsed via OCR fallback parser. Review the imported fields before calculating splits or sending UPNs."
            .to_string()
    };

    let iban_raw = "SI56 6100 0000 5243 585".to_string();
    let iban_norm = normalize_iban(&iban_raw);

    Some(ExtractedBill {
        iban_norm,
        iban_raw,
        amount_cents,
        reference,
        due_date,
        purpose_code: "COST".to_string(),
        purpose_text: String::new(),
        invoice_number,
        parse_note,
    })
}

#[derive(Clone)]
pub(crate) struct BillImportContext {
    pub month: i32,
    pub year: i32,
    pub providers: Vec<Provider>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PreparedBillPreviewSummary {
    pub provider_id: Option<i64>,
    pub provider_name: Option<String>,
    pub creditor_name: String,
    pub amount_cents: i64,
    pub reference: String,
    pub due_date: String,
    pub invoice_number: String,
    pub purpose_text: String,
    pub parse_note: String,
    pub status: String,
}

pub(crate) struct PreparedBillImport {
    raw_text: String,
    filename: String,
    source_month: i32,
    source_year: i32,
    detected_source_period: Option<(i32, i32)>,
    extracted: Vec<ExtractedBill>,
    log: String,
    redact_details: bool,
}

pub(crate) struct ExpectedProviderFilterResult {
    pub skipped_status: Option<&'static str>,
    pub skipped_reason: Option<String>,
}

pub(crate) struct BillHashFilterResult {
    pub kept_hashes: Vec<String>,
    pub skipped_duplicate_count: usize,
}

impl PreparedBillImport {
    pub(crate) fn detected_source_period(&self) -> Option<(i32, i32)> {
        self.detected_source_period
    }

    pub(crate) fn has_extracted_bills(&self) -> bool {
        !self.extracted.is_empty()
    }
}

pub(crate) fn bill_content_hash(
    provider_id: Option<i64>,
    creditor_iban: &str,
    amount_cents: i64,
    reference: &str,
    due_date: &str,
    invoice_number: &str,
) -> String {
    let iban_norm = normalize_iban(creditor_iban);
    let provider_key = provider_id
        .map(|id| id.to_string())
        .unwrap_or_else(|| iban_norm.clone());
    let canonical = format!(
        "bill-v1|{}|{}|{}|{}|{}|{}",
        provider_key,
        iban_norm,
        amount_cents,
        reference.trim(),
        due_date.trim(),
        invoice_number.trim()
    );
    let digest = Sha256::digest(canonical.as_bytes());
    digest.iter().map(|b| format!("{:02x}", b)).collect()
}

fn bill_hash(provider: Option<&Provider>, bill: &ExtractedBill) -> String {
    bill_content_hash(
        provider.and_then(|p| p.id),
        &bill.iban_norm,
        bill.amount_cents,
        &bill.reference,
        &bill.due_date,
        &bill.invoice_number,
    )
}

pub(crate) fn retain_expected_provider_bills(
    prepared: &mut PreparedBillImport,
    providers: &[Provider],
    missing_provider_ids: &HashSet<i64>,
) -> ExpectedProviderFilterResult {
    let provider_by_iban: std::collections::HashMap<String, &Provider> = providers
        .iter()
        .filter(|p| p.id.is_some() && !p.creditor_iban.is_empty())
        .map(|p| (normalize_iban(&p.creditor_iban), p))
        .collect();

    let mut kept: Vec<ExtractedBill> = Vec::new();
    let mut unknown_count = 0;
    let mut already_present: Vec<String> = Vec::new();

    for bill in prepared.extracted.drain(..) {
        let Some(provider) = provider_by_iban.get(&bill.iban_norm).copied() else {
            unknown_count += 1;
            continue;
        };
        let Some(provider_id) = provider.id else {
            unknown_count += 1;
            continue;
        };
        if missing_provider_ids.contains(&provider_id) {
            kept.push(bill);
        } else {
            already_present.push(provider.name.clone());
        }
    }

    let kept_count = kept.len();
    prepared.extracted = kept;
    let partial = kept_count > 0;
    let skipped = if unknown_count > 0 && already_present.is_empty() {
        Some((
            "skipped_unknown_provider",
            if partial {
                "Some parsed bills were ignored because no configured provider matched them."
                    .to_string()
            } else {
                "No configured provider matched the parsed bill attachment.".to_string()
            },
        ))
    } else if !already_present.is_empty() && unknown_count == 0 {
        already_present.sort();
        already_present.dedup();
        Some((
            "skipped_already_present",
            if partial {
                format!(
                    "Some parsed bills were ignored because configured providers already have bills in this period: {}.",
                    already_present.join(", ")
                )
            } else {
                format!(
                    "Configured provider already has a bill in this period: {}.",
                    already_present.join(", ")
                )
            },
        ))
    } else if unknown_count > 0 || !already_present.is_empty() {
        Some((
            "skipped_not_expected",
            if partial {
                "Some parsed bills were ignored because they were unknown or already present for this period."
                    .to_string()
            } else {
                "No parsed bill matched a configured provider that is still missing for this period."
                    .to_string()
            },
        ))
    } else {
        None
    };

    if kept_count > 0 {
        prepared.log.push_str(&format!(
            "Inbox expected-provider filter: {} bill(s) kept\n",
            kept_count
        ));
        let (skipped_status, skipped_reason) = skipped
            .map(|(status, reason)| (Some(status), Some(reason)))
            .unwrap_or((None, None));
        return ExpectedProviderFilterResult {
            skipped_status,
            skipped_reason,
        };
    }

    if let Some((status, reason)) = skipped {
        return ExpectedProviderFilterResult {
            skipped_status: Some(status),
            skipped_reason: Some(reason),
        };
    }

    ExpectedProviderFilterResult {
        skipped_status: Some("skipped_not_expected"),
        skipped_reason: Some(
            "No parsed bill matched a configured provider that is still missing for this period."
                .to_string(),
        ),
    }
}

pub(crate) fn retain_new_bill_hashes(
    prepared: &mut PreparedBillImport,
    providers: &[Provider],
    existing_hashes: &HashSet<String>,
) -> BillHashFilterResult {
    let provider_by_iban: std::collections::HashMap<String, &Provider> = providers
        .iter()
        .filter(|p| !p.creditor_iban.is_empty())
        .map(|p| (normalize_iban(&p.creditor_iban), p))
        .collect();
    let mut seen_in_attachment = HashSet::new();
    let mut kept = Vec::new();
    let mut kept_hashes = Vec::new();
    let mut skipped_duplicate_count = 0;

    for bill in prepared.extracted.drain(..) {
        let provider = provider_by_iban.get(&bill.iban_norm).copied();
        let hash = bill_hash(provider, &bill);
        if existing_hashes.contains(&hash) || !seen_in_attachment.insert(hash.clone()) {
            skipped_duplicate_count += 1;
            continue;
        }
        kept.push(bill);
        kept_hashes.push(hash);
    }

    prepared.extracted = kept;
    BillHashFilterResult {
        kept_hashes,
        skipped_duplicate_count,
    }
}

pub(crate) fn preview_prepared_bills(
    prepared: &PreparedBillImport,
    providers: &[Provider],
) -> Vec<PreparedBillPreviewSummary> {
    let provider_by_iban: std::collections::HashMap<String, &Provider> = providers
        .iter()
        .filter(|p| !p.creditor_iban.is_empty())
        .map(|p| (normalize_iban(&p.creditor_iban), p))
        .collect();

    prepared
        .extracted
        .iter()
        .map(|eb| {
            let provider = provider_by_iban.get(&eb.iban_norm).copied();
            let parse_note = import_review_parse_note(
                &eb.parse_note,
                eb.amount_cents,
                &eb.iban_norm,
                &eb.reference,
                &eb.due_date,
            );
            let status = if parse_note.is_empty() {
                "draft".to_string()
            } else {
                "needs_review".to_string()
            };
            let (provider_id, provider_name, creditor_name, purpose_text) = match provider {
                Some(p) => (
                    p.id,
                    Some(p.name.clone()),
                    p.creditor_name.clone(),
                    if !eb.purpose_text.is_empty() {
                        eb.purpose_text.clone()
                    } else {
                        interpolate_template(
                            &p.purpose_text_template,
                            &eb.invoice_number,
                            prepared.source_month,
                            prepared.source_year,
                        )
                    },
                ),
                None => (None, None, String::new(), eb.purpose_text.clone()),
            };

            PreparedBillPreviewSummary {
                provider_id,
                provider_name,
                creditor_name,
                amount_cents: eb.amount_cents,
                reference: eb.reference.clone(),
                due_date: eb.due_date.clone(),
                invoice_number: eb.invoice_number.clone(),
                purpose_text,
                parse_note,
                status,
            }
        })
        .collect()
}

pub(crate) fn load_bill_import_context(
    conn: &Connection,
    billing_period_id: i64,
) -> Result<BillImportContext, String> {
    let (month, year) = conn
        .query_row(
            "SELECT month, year FROM billing_periods WHERE id=?1",
            [billing_period_id],
            |r| Ok((r.get::<_, i32>(0)?, r.get::<_, i32>(1)?)),
        )
        .map_err(|e| e.to_string())?;

    Ok(BillImportContext {
        month,
        year,
        providers: get_providers_inner(conn),
    })
}

pub(crate) fn prepare_multi_bill_import_from_path(
    file_path: &str,
    source_filename: String,
    context: &BillImportContext,
    include_raw_text_in_log: bool,
) -> Result<PreparedBillImport, String> {
    let raw_text = extract_text_from_file(file_path)?;
    Ok(prepare_multi_bill_import_from_text(
        raw_text,
        source_filename,
        context.month,
        context.year,
        include_raw_text_in_log,
    ))
}

fn prepare_multi_bill_import_from_text(
    raw_text: String,
    filename: String,
    month: i32,
    year: i32,
    include_raw_text_in_log: bool,
) -> PreparedBillImport {
    let detected_source_period = find_source_period_month_year(&raw_text);
    let (source_month, source_year) = detected_source_period.unwrap_or((month, year));
    let raw_log = if include_raw_text_in_log {
        raw_text.as_str()
    } else {
        "(redacted for inbox import)"
    };
    let redact_details = !include_raw_text_in_log;
    let mut log = format!(
        "=== import_bills: {} ===\n\n--- RAW TEXT ---\n{}\n\n--- PARSE RESULTS ---\n",
        filename, raw_log
    );

    let mut extracted: Vec<ExtractedBill> = Vec::new();
    let mut seen_ibans: std::collections::HashSet<String> = std::collections::HashSet::new();

    let stubs = parse_upn_stubs(&raw_text);
    log.push_str(&format!("Phase 1 (UPN stubs): {} found\n", stubs.len()));
    for bill in stubs {
        if !redact_details {
            log.push_str(&format!(
                "  IBAN={} amount={} ref={} due={}\n",
                bill.iban_raw, bill.amount_cents, bill.reference, bill.due_date
            ));
        }
        if seen_ibans.insert(bill.iban_norm.clone()) {
            extracted.push(bill);
        }
    }

    let elektro = parse_elektro_style(&raw_text);
    log.push_str(&format!(
        "Phase 2 (Elektro): {}\n",
        if elektro.is_some() {
            "found"
        } else {
            "NOT FOUND"
        }
    ));
    if let Some(bill) = elektro {
        if !redact_details {
            log.push_str(&format!(
                "  IBAN={} amount={} ref={} due={}\n",
                bill.iban_raw, bill.amount_cents, bill.reference, bill.due_date
            ));
        }
        if seen_ibans.insert(bill.iban_norm.clone()) {
            extracted.push(bill);
        }
    }

    let zlm = parse_zlm_style(&raw_text);
    log.push_str(&format!(
        "Phase 3 (ZLM): {}\n",
        if zlm.is_some() { "found" } else { "NOT FOUND" }
    ));
    if let Some(bill) = zlm {
        if !redact_details {
            log.push_str(&format!(
                "  IBAN={} amount={} ref={} due={}\n",
                bill.iban_raw, bill.amount_cents, bill.reference, bill.due_date
            ));
        }
        if seen_ibans.insert(bill.iban_norm.clone()) {
            extracted.push(bill);
        }
    }

    let dimnikar = parse_dimnikar_style(&raw_text);
    log.push_str(&format!(
        "Phase 4 (Dimnikar OCR): {}\n",
        if dimnikar.is_some() {
            "found"
        } else {
            "NOT FOUND"
        }
    ));
    if let Some(bill) = dimnikar {
        if !redact_details {
            log.push_str(&format!(
                "  IBAN={} amount={} ref={} due={}\n",
                bill.iban_raw, bill.amount_cents, bill.reference, bill.due_date
            ));
        }
        if seen_ibans.insert(bill.iban_norm.clone()) {
            extracted.push(bill);
        }
    }

    PreparedBillImport {
        raw_text,
        filename,
        source_month,
        source_year,
        detected_source_period,
        extracted,
        log,
        redact_details,
    }
}

fn write_import_debug_log(log: &str) {
    if let Some(path) =
        dirs_next::data_dir().map(|d| d.join("si.upn-generator").join("import_debug.log"))
    {
        let _ = std::fs::write(path, log);
    }
}

pub(crate) fn save_prepared_multi_bill_import(
    conn: &Connection,
    billing_period_id: i64,
    mut prepared: PreparedBillImport,
    providers: &[Provider],
    persist_fallback_raw_text: bool,
) -> Result<Vec<Bill>, String> {
    let provider_by_iban: std::collections::HashMap<String, &Provider> = providers
        .iter()
        .filter(|p| !p.creditor_iban.is_empty())
        .map(|p| (normalize_iban(&p.creditor_iban), p))
        .collect();

    if prepared.extracted.is_empty() {
        let raw_text = if persist_fallback_raw_text {
            prepared.raw_text.clone()
        } else {
            String::new()
        };
        conn.execute(
            "INSERT INTO bills (billing_period_id, provider_id, raw_text, amount_cents,
             creditor_name, creditor_iban, creditor_address, creditor_city,
             creditor_postal_code, reference, due_date, purpose_code, purpose_text,
             invoice_number, parse_note, status, source_filename)
             VALUES (?1,NULL,?2,0,'','','','','','','','OTHR','','',
                     'No bill data could be parsed automatically. Review this import manually.',
                     'needs_review',?3)",
            params![billing_period_id, raw_text, prepared.filename],
        )
        .map_err(|e| e.to_string())?;
        let id = conn.last_insert_rowid();
        prepared.log.push_str("--- SAVED BILLS ---\n");
        if prepared.redact_details {
            prepared
                .log
                .push_str("  status=needs_review details=(redacted for inbox import)\n");
        } else {
            prepared.log.push_str(
                "  status=needs_review parse_note=No bill data could be parsed automatically. Review this import manually.\n",
            );
        }
        write_import_debug_log(&prepared.log);
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
            parse_note: "No bill data could be parsed automatically. Review this import manually."
                .to_string(),
            status: "needs_review".to_string(),
            source_filename: prepared.filename,
            provider_name: None,
        }]);
    }

    let mut results: Vec<Bill> = Vec::new();
    prepared.log.push_str("--- SAVED BILLS ---\n");

    for eb in prepared.extracted {
        let provider = provider_by_iban.get(&eb.iban_norm).copied();

        let (
            provider_id,
            creditor_name,
            creditor_iban,
            creditor_address,
            creditor_city,
            creditor_postal_code,
            purpose_code,
        ) = match provider {
            Some(p) => (
                p.id,
                p.creditor_name.clone(),
                p.creditor_iban.clone(),
                p.creditor_address.clone(),
                p.creditor_city.clone(),
                p.creditor_postal_code.clone(),
                if eb.purpose_code != "OTHR" {
                    eb.purpose_code.clone()
                } else {
                    p.purpose_code.clone()
                },
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

        let purpose_text = if !eb.purpose_text.is_empty() {
            eb.purpose_text.clone()
        } else if let Some(p) = provider {
            interpolate_template(
                &p.purpose_text_template,
                &eb.invoice_number,
                prepared.source_month,
                prepared.source_year,
            )
        } else {
            String::new()
        };

        let parse_note = import_review_parse_note(
            &eb.parse_note,
            eb.amount_cents,
            &eb.iban_norm,
            &eb.reference,
            &eb.due_date,
        );
        let status = if parse_note.is_empty() {
            "draft".to_string()
        } else {
            "needs_review".to_string()
        };

        conn.execute(
            "INSERT INTO bills (billing_period_id, provider_id, raw_text, amount_cents,
             creditor_name, creditor_iban, creditor_address, creditor_city,
             creditor_postal_code, reference, due_date, purpose_code, purpose_text,
             invoice_number, parse_note, status, source_filename)
             VALUES (?1,?2,'',?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)",
            params![
                billing_period_id,
                provider_id,
                eb.amount_cents,
                creditor_name,
                creditor_iban,
                creditor_address,
                creditor_city,
                creditor_postal_code,
                eb.reference,
                eb.due_date,
                purpose_code,
                purpose_text,
                eb.invoice_number,
                parse_note,
                status,
                prepared.filename,
            ],
        )
        .map_err(|e| e.to_string())?;

        let id = conn.last_insert_rowid();
        if prepared.redact_details {
            prepared.log.push_str(&format!(
                "  provider={} status={} details=(redacted for inbox import)\n",
                provider.map(|p| p.name.as_str()).unwrap_or("(unmatched)"),
                status
            ));
        } else {
            prepared.log.push_str(&format!(
                "  provider={} amount={} ref={} status={} parse_note={}\n",
                provider.map(|p| p.name.as_str()).unwrap_or("(unmatched)"),
                eb.amount_cents,
                eb.reference,
                status,
                if parse_note.is_empty() {
                    "(empty)"
                } else {
                    parse_note.as_str()
                }
            ));
        }
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
            parse_note,
            status,
            source_filename: prepared.filename.clone(),
            provider_name: provider.map(|p| p.name.clone()),
        });
    }

    write_import_debug_log(&prepared.log);
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_provider(id: i64, iban: &str, name: &str) -> Provider {
        Provider {
            id: Some(id),
            name: name.to_string(),
            service_type: String::new(),
            creditor_name: name.to_string(),
            creditor_address: String::new(),
            creditor_city: String::new(),
            creditor_postal_code: String::new(),
            creditor_iban: iban.to_string(),
            purpose_code: "OTHR".to_string(),
            match_pattern: String::new(),
            amount_pattern: String::new(),
            reference_pattern: String::new(),
            due_date_pattern: String::new(),
            invoice_number_pattern: String::new(),
            purpose_text_template: String::new(),
            split_basis: "m2_percentage".to_string(),
        }
    }

    fn test_extracted_bill(iban: &str) -> ExtractedBill {
        ExtractedBill {
            iban_norm: normalize_iban(iban),
            iban_raw: iban.to_string(),
            amount_cents: 1234,
            reference: "SI12 123".to_string(),
            due_date: "01.04.2026".to_string(),
            purpose_code: "OTHR".to_string(),
            purpose_text: String::new(),
            invoice_number: String::new(),
            parse_note: String::new(),
        }
    }

    fn test_prepared(extracted: Vec<ExtractedBill>) -> PreparedBillImport {
        PreparedBillImport {
            raw_text: String::new(),
            filename: "invoice.pdf".to_string(),
            source_month: 4,
            source_year: 2026,
            detected_source_period: Some((4, 2026)),
            extracted,
            log: String::new(),
            redact_details: true,
        }
    }

    #[test]
    fn prepared_import_preserves_detected_source_period() {
        let prepared = prepare_multi_bill_import_from_text(
            "Obracun za MAREC 2026".to_string(),
            "invoice.pdf".to_string(),
            4,
            2026,
            false,
        );

        assert_eq!(prepared.detected_source_period(), Some((3, 2026)));
    }

    #[test]
    fn prepared_import_preserves_unknown_source_period() {
        let prepared = prepare_multi_bill_import_from_text(
            "Racun brez jasnega obdobja".to_string(),
            "invoice.pdf".to_string(),
            4,
            2026,
            false,
        );

        assert_eq!(prepared.detected_source_period(), None);
    }

    #[test]
    fn import_review_note_reports_missing_due_date() {
        let note = import_review_parse_note("", 9387, "SI560400100489142226", "SI12 123", "");

        assert!(note.contains("Missing required payment field: due date"));
        assert!(note.contains("Review this import"));
    }

    #[test]
    fn prepared_bill_preview_marks_missing_due_date_for_review() {
        let providers = vec![test_provider(
            7,
            "SI56 0400 1004 9142 226",
            "JP VOKA SNAGA d.o.o.",
        )];
        let mut bill = test_extracted_bill("SI56 0400 1004 9142 226");
        bill.due_date = String::new();
        let prepared = test_prepared(vec![bill]);

        let preview = preview_prepared_bills(&prepared, &providers);

        assert_eq!(preview.len(), 1);
        assert_eq!(preview[0].status, "needs_review");
        assert!(preview[0].parse_note.contains("due date"));
        assert!(preview[0].parse_note.contains("Review this import"));
    }

    #[test]
    fn upn_stub_uses_context_purpose_date_before_marker() {
        let text = "
Ravnanje z odpadki 05/2026
***93,87
SI56 0400 1004 9142 226
SI12 2000263445522
JP VOKA SNAGA d.o.o.

SCVE Ravnanje z odpadki 05/2026 0040113249 15.06.2026
***93,87
SI56 0400 1004 9142 226
SI12 2000263445522

Energetika Ljubljana
Rok plačila: 19.06.2026
***249,06
SI56 0292 4025 3764 022
SI12 6330017789210
";

        let bills = parse_upn_stubs(text);

        let waste = bills
            .iter()
            .find(|bill| bill.reference == "SI12 2000263445522")
            .expect("waste bill");
        assert_eq!(waste.purpose_text, "Ravnanje z odpadki 05/2026 0040113249 15.06.2026");
        assert_eq!(waste.due_date, "15.06.2026");
    }

    #[test]
    fn expected_provider_filter_keeps_missing_configured_provider() {
        let providers = vec![test_provider(7, "SI56 0400 1004 8988 093", "Elektro")];
        let mut missing = HashSet::new();
        missing.insert(7);
        let mut prepared = test_prepared(vec![test_extracted_bill("SI56 0400 1004 8988 093")]);

        let result = retain_expected_provider_bills(&mut prepared, &providers, &missing);

        assert_eq!(prepared.extracted.len(), 1);
        assert!(result.skipped_status.is_none());
    }

    #[test]
    fn expected_provider_filter_skips_unknown_provider() {
        let providers = vec![test_provider(7, "SI56 0400 1004 8988 093", "Elektro")];
        let mut missing = HashSet::new();
        missing.insert(7);
        let mut prepared = test_prepared(vec![test_extracted_bill("SI56 9999 0000 0000 000")]);

        let result = retain_expected_provider_bills(&mut prepared, &providers, &missing);

        assert!(prepared.extracted.is_empty());
        assert_eq!(result.skipped_status, Some("skipped_unknown_provider"));
    }

    #[test]
    fn expected_provider_filter_skips_already_present_provider() {
        let providers = vec![test_provider(7, "SI56 0400 1004 8988 093", "Elektro")];
        let missing = HashSet::new();
        let mut prepared = test_prepared(vec![test_extracted_bill("SI56 0400 1004 8988 093")]);

        let result = retain_expected_provider_bills(&mut prepared, &providers, &missing);

        assert!(prepared.extracted.is_empty());
        assert_eq!(result.skipped_status, Some("skipped_already_present"));
    }

    #[test]
    fn expected_provider_filter_reports_partial_unknown_skip() {
        let providers = vec![test_provider(7, "SI56 0400 1004 8988 093", "Elektro")];
        let mut missing = HashSet::new();
        missing.insert(7);
        let mut prepared = test_prepared(vec![
            test_extracted_bill("SI56 0400 1004 8988 093"),
            test_extracted_bill("SI56 9999 0000 0000 000"),
        ]);

        let result = retain_expected_provider_bills(&mut prepared, &providers, &missing);

        assert_eq!(prepared.extracted.len(), 1);
        assert_eq!(result.skipped_status, Some("skipped_unknown_provider"));
    }

    #[test]
    fn bill_hash_filter_skips_previously_imported_bill_content() {
        let providers = vec![test_provider(7, "SI56 0400 1004 8988 093", "Elektro")];
        let mut first = test_prepared(vec![test_extracted_bill("SI56 0400 1004 8988 093")]);
        let first_result = retain_new_bill_hashes(&mut first, &providers, &HashSet::new());
        let existing: HashSet<String> = first_result.kept_hashes.into_iter().collect();
        let mut second = test_prepared(vec![test_extracted_bill("SI56 0400 1004 8988 093")]);

        let second_result = retain_new_bill_hashes(&mut second, &providers, &existing);

        assert!(second.extracted.is_empty());
        assert_eq!(second_result.skipped_duplicate_count, 1);
    }

    #[test]
    fn bill_content_hash_matches_saved_bill_fields() {
        let provider = test_provider(7, "SI56 0400 1004 8988 093", "Elektro");
        let bill = test_extracted_bill("SI56 0400 1004 8988 093");

        let parsed_hash = bill_hash(Some(&provider), &bill);
        let saved_hash = bill_content_hash(
            Some(7),
            &bill.iban_norm,
            bill.amount_cents,
            &bill.reference,
            &bill.due_date,
            &bill.invoice_number,
        );

        assert_eq!(parsed_hash, saved_hash);
    }

    #[test]
    fn delete_inbox_imports_for_bill_matches_json_bill_id_exactly() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE inbox_imports (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                bill_ids TEXT NOT NULL DEFAULT '',
                status TEXT NOT NULL DEFAULT ''
            );
            INSERT INTO inbox_imports (bill_ids, status) VALUES ('[10]', 'imported');
            INSERT INTO inbox_imports (bill_ids, status) VALUES ('[1,11]', 'imported');
            INSERT INTO inbox_imports (bill_ids, status) VALUES ('[1]', 'failed');
            ",
        )
        .unwrap();

        delete_inbox_imports_for_bill(&conn, 1).unwrap();

        let remaining: Vec<String> = {
            let mut stmt = conn
                .prepare("SELECT bill_ids || ':' || status FROM inbox_imports ORDER BY id")
                .unwrap();
            stmt.query_map([], |row| row.get::<_, String>(0))
                .unwrap()
                .map(|row| row.unwrap())
                .collect()
        };
        assert_eq!(remaining, vec!["[10]:imported", "[1]:failed"]);
    }
}

/// Import a bill file that may contain multiple bills.
/// Uses smart parsing: finds UPN payment stubs (***amount), falls back to
/// Elektro narrative format and ZLM format. Matches providers by IBAN.
#[tauri::command]
pub fn import_bills(
    db: State<DbState>,
    file_path: String,
    billing_period_id: i64,
) -> Result<Vec<Bill>, String> {
    let raw_text = extract_text_from_file(&file_path)?;

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
    let (source_month, source_year) =
        find_source_period_month_year(&raw_text).unwrap_or((month, year));

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
    let log_path =
        dirs_next::data_dir().map(|d| d.join("si.upn-generator").join("import_debug.log"));
    let mut log = format!(
        "=== import_bills: {} ===\n\n--- RAW TEXT ---\n{}\n\n--- PARSE RESULTS ---\n",
        filename, raw_text
    );

    // --- Collect extracted bills ---
    let mut extracted: Vec<ExtractedBill> = Vec::new();
    let mut seen_ibans: std::collections::HashSet<String> = std::collections::HashSet::new();

    // Phase 1: UPN payment stubs (***amount) — covers VOKA ×2 and Energetika
    let stubs = parse_upn_stubs(&raw_text);
    log.push_str(&format!("Phase 1 (UPN stubs): {} found\n", stubs.len()));
    for bill in stubs {
        log.push_str(&format!(
            "  IBAN={} amount={} ref={} due={}\n",
            bill.iban_raw, bill.amount_cents, bill.reference, bill.due_date
        ));
        if seen_ibans.insert(bill.iban_norm.clone()) {
            extracted.push(bill);
        }
    }

    // Phase 2: Elektro narrative format (ZA PLACILO Z DDV:)
    let elektro = parse_elektro_style(&raw_text);
    log.push_str(&format!(
        "Phase 2 (Elektro): {}\n",
        if elektro.is_some() {
            "found"
        } else {
            "NOT FOUND"
        }
    ));
    if let Some(bill) = elektro {
        log.push_str(&format!(
            "  IBAN={} amount={} ref={} due={}\n",
            bill.iban_raw, bill.amount_cents, bill.reference, bill.due_date
        ));
        if seen_ibans.insert(bill.iban_norm.clone()) {
            extracted.push(bill);
        }
    }

    // Phase 3: ZLM format (Za placilo EUR: + TRR:)
    let zlm = parse_zlm_style(&raw_text);
    log.push_str(&format!(
        "Phase 3 (ZLM): {}\n",
        if zlm.is_some() { "found" } else { "NOT FOUND" }
    ));
    if let Some(bill) = zlm {
        log.push_str(&format!(
            "  IBAN={} amount={} ref={} due={}\n",
            bill.iban_raw, bill.amount_cents, bill.reference, bill.due_date
        ));
        if seen_ibans.insert(bill.iban_norm.clone()) {
            extracted.push(bill);
        }
    }

    // Phase 4: OCR-tolerant Dimnikar image format
    let dimnikar = parse_dimnikar_style(&raw_text);
    log.push_str(&format!(
        "Phase 4 (Dimnikar OCR): {}\n",
        if dimnikar.is_some() {
            "found"
        } else {
            "NOT FOUND"
        }
    ));
    if let Some(bill) = dimnikar {
        log.push_str(&format!(
            "  IBAN={} amount={} ref={} due={}\n",
            bill.iban_raw, bill.amount_cents, bill.reference, bill.due_date
        ));
        if seen_ibans.insert(bill.iban_norm.clone()) {
            extracted.push(bill);
        }
    }

    // Fallback: nothing found — create one blank bill
    if extracted.is_empty() {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO bills (billing_period_id, provider_id, raw_text, amount_cents,
             creditor_name, creditor_iban, creditor_address, creditor_city,
             creditor_postal_code, reference, due_date, purpose_code, purpose_text,
             invoice_number, parse_note, status, source_filename)
             VALUES (?1,NULL,?2,0,'','','','','','','','OTHR','','',
                     'No bill data could be parsed automatically. Review this import manually.',
                     'needs_review',?3)",
            params![billing_period_id, raw_text, filename],
        )
        .map_err(|e| e.to_string())?;
        let id = conn.last_insert_rowid();
        log.push_str("--- SAVED BILLS ---\n");
        log.push_str(
            "  status=needs_review parse_note=No bill data could be parsed automatically. Review this import manually.\n",
        );
        if let Some(ref path) = log_path {
            let _ = std::fs::write(path, &log);
        }
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
            parse_note: "No bill data could be parsed automatically. Review this import manually."
                .to_string(),
            status: "needs_review".to_string(),
            source_filename: filename,
            provider_name: None,
        }]);
    }

    // --- Match to providers and insert ---
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut results: Vec<Bill> = Vec::new();
    log.push_str("--- SAVED BILLS ---\n");

    for eb in extracted {
        let provider = provider_by_iban.get(&eb.iban_norm).copied();

        // Determine creditor info from provider (if matched) or from extracted IBAN
        let (
            provider_id,
            creditor_name,
            creditor_iban,
            creditor_address,
            creditor_city,
            creditor_postal_code,
            purpose_code,
        ) = match provider {
            Some(p) => (
                p.id,
                p.creditor_name.clone(),
                p.creditor_iban.clone(),
                p.creditor_address.clone(),
                p.creditor_city.clone(),
                p.creditor_postal_code.clone(),
                if eb.purpose_code != "OTHR" {
                    eb.purpose_code.clone()
                } else {
                    p.purpose_code.clone()
                },
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
            interpolate_template(
                &p.purpose_text_template,
                &eb.invoice_number,
                source_month,
                source_year,
            )
        } else {
            String::new()
        };

        let parse_note = import_review_parse_note(
            &eb.parse_note,
            eb.amount_cents,
            &eb.iban_norm,
            &eb.reference,
            &eb.due_date,
        );
        let status = if parse_note.is_empty() {
            "draft".to_string()
        } else {
            "needs_review".to_string()
        };

        conn.execute(
            "INSERT INTO bills (billing_period_id, provider_id, raw_text, amount_cents,
             creditor_name, creditor_iban, creditor_address, creditor_city,
             creditor_postal_code, reference, due_date, purpose_code, purpose_text,
             invoice_number, parse_note, status, source_filename)
             VALUES (?1,?2,'',?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)",
            params![
                billing_period_id,
                provider_id,
                eb.amount_cents,
                creditor_name,
                creditor_iban,
                creditor_address,
                creditor_city,
                creditor_postal_code,
                eb.reference,
                eb.due_date,
                purpose_code,
                purpose_text,
                eb.invoice_number,
                parse_note,
                status,
                filename,
            ],
        )
        .map_err(|e| e.to_string())?;

        let id = conn.last_insert_rowid();
        log.push_str(&format!(
            "  provider={} amount={} ref={} status={} parse_note={}\n",
            provider.map(|p| p.name.as_str()).unwrap_or("(unmatched)"),
            eb.amount_cents,
            eb.reference,
            status,
            if parse_note.is_empty() {
                "(empty)"
            } else {
                parse_note.as_str()
            }
        ));
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
            parse_note,
            status,
            source_filename: filename.clone(),
            provider_name: provider.map(|p| p.name.clone()),
        });
    }

    if let Some(ref path) = log_path {
        let _ = std::fs::write(path, &log);
    }

    Ok(results)
}
