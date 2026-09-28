use regex::Regex;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::panic::{catch_unwind, UnwindSafe};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::Duration;
use tauri::{AppHandle, Manager, State};

use super::config::{DbState, Provider};

#[cfg(target_os = "windows")]
use windows::{
    Data::Pdf::{PdfDocument, PdfPageRenderOptions},
    Graphics::Imaging::{BitmapDecoder, SoftwareBitmap},
    Media::Ocr::OcrEngine,
    Storage::{FileAccessMode, StorageFile, Streams::InMemoryRandomAccessStream},
};

// ─── Structs ───────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BillingPeriod {
    pub id: Option<i64>,
    pub building_id: i64,
    pub month: i32,
    pub year: i32,
    pub status: String,
    pub closed_at: Option<String>,
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
    pub reviewed_at: Option<String>,
    pub review_note: String,
    pub identity_status: String,
    pub identity_rule_snapshot: String,
    pub identity_evidence: String,
    pub identity_exception_note: String,
    pub identity_exception_at: Option<String>,
    pub source_page_start: Option<i32>,
    pub source_page_end: Option<i32>,
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

#[derive(Debug, Clone)]
struct ExtractedPage {
    page_number: i32,
    native_text: String,
    ocr_text: String,
    diagnostics: Vec<String>,
}

impl ExtractedPage {
    fn combined_text(&self) -> String {
        match (
            self.native_text.trim().is_empty(),
            self.ocr_text.trim().is_empty(),
        ) {
            (false, false) => format!("{}\n{}", self.native_text.trim(), self.ocr_text.trim()),
            (false, true) => self.native_text.trim().to_string(),
            (true, false) => self.ocr_text.trim().to_string(),
            (true, true) => String::new(),
        }
    }
}

#[derive(Debug, Clone)]
struct DocumentExtraction {
    pages: Vec<ExtractedPage>,
    diagnostics: Vec<String>,
}

#[cfg(target_os = "windows")]
static PDF_OCR_ACTIVE: AtomicBool = AtomicBool::new(false);

#[cfg(target_os = "windows")]
struct PdfOcrWorkerGuard;

#[cfg(target_os = "windows")]
impl Drop for PdfOcrWorkerGuard {
    fn drop(&mut self) {
        PDF_OCR_ACTIVE.store(false, Ordering::Release);
    }
}

fn contain_extractor_unwind<T, F>(operation: F) -> Result<T, String>
where
    F: FnOnce() -> Result<T, pdf_extract::OutputError> + UnwindSafe,
{
    match catch_unwind(operation) {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => Err(format!("PDF text extraction failed: {error}")),
        Err(_) => Err(
            "PDF text extraction panicked; the document is unreadable by the text extractor."
                .to_string(),
        ),
    }
}

#[cfg(target_os = "windows")]
fn ocr_pdf_pages(file_path: &str, page_limit: usize) -> Result<Vec<String>, String> {
    PDF_OCR_ACTIVE
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .map_err(|_| "Another PDF rendering/OCR operation is still running.".to_string())?;
    let canonical_path = std::fs::canonicalize(file_path)
        .map_err(|e| format!("Could not resolve PDF path: {e}"))?
        .to_string_lossy()
        .to_string();
    // WinRT StorageFile rejects the extended-length prefix returned by
    // std::fs::canonicalize even though the same path is valid for Win32 APIs.
    let path = if let Some(path) = canonical_path.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{path}")
    } else {
        canonical_path
            .strip_prefix(r"\\?\")
            .unwrap_or(&canonical_path)
            .to_string()
    };
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _worker_guard = PdfOcrWorkerGuard;
        let result = (|| -> Result<Vec<String>, String> {
            let file = StorageFile::GetFileFromPathAsync(&path.into())
                .map_err(|e| format!("Could not open PDF for rendering: {e}"))?
                .get()
                .map_err(|e| format!("Could not open PDF for rendering: {e}"))?;
            let document = PdfDocument::LoadFromFileAsync(&file)
                .map_err(|e| format!("Could not start PDF renderer: {e}"))?
                .get()
                .map_err(|e| format!("Could not load PDF renderer: {e}"))?;
            let page_count = document.PageCount().map_err(|e| e.to_string())? as usize;
            if page_count > page_limit {
                return Err(format!(
                    "PDF has {page_count} pages; the verification limit is {page_limit} pages."
                ));
            }
            let engine = OcrEngine::TryCreateFromUserProfileLanguages()
                .map_err(|e| format!("Windows OCR is unavailable: {e}"))?;
            let mut texts = Vec::with_capacity(page_count);
            for index in 0..page_count {
                let page = document.GetPage(index as u32).map_err(|e| e.to_string())?;
                let size = page.Size().map_err(|e| e.to_string())?;
                let width = 1800u32;
                let height = ((size.Height / size.Width) * width as f32)
                    .round()
                    .clamp(1.0, 2600.0) as u32;
                let options = PdfPageRenderOptions::new().map_err(|e| e.to_string())?;
                options
                    .SetDestinationWidth(width)
                    .map_err(|e| e.to_string())?;
                options
                    .SetDestinationHeight(height)
                    .map_err(|e| e.to_string())?;
                let stream = InMemoryRandomAccessStream::new().map_err(|e| e.to_string())?;
                page.RenderWithOptionsToStreamAsync(&stream, &options)
                    .map_err(|e| format!("Could not render PDF page {}: {e}", index + 1))?
                    .get()
                    .map_err(|e| format!("Could not render PDF page {}: {e}", index + 1))?;
                stream.Seek(0).map_err(|e| e.to_string())?;
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
                let recognized = engine
                    .RecognizeAsync(&bitmap)
                    .map_err(|e| e.to_string())?
                    .get()
                    .map_err(|e| e.to_string())?;
                texts.push(
                    recognized
                        .Text()
                        .map_err(|e| e.to_string())?
                        .to_string_lossy()
                        .trim()
                        .to_string(),
                );
                let _ = page.Close();
            }
            Ok(texts)
        })();
        let _ = tx.send(result);
    });
    match rx.recv_timeout(Duration::from_secs(75)) {
        Ok(result) => result,
        Err(mpsc::RecvTimeoutError::Timeout) => {
            Err("PDF rendering/OCR timed out after 75 seconds.".to_string())
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            Err("PDF rendering/OCR worker stopped unexpectedly.".to_string())
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn ocr_pdf_pages(_file_path: &str, _page_limit: usize) -> Result<Vec<String>, String> {
    Err("PDF page rendering/OCR is only supported on Windows builds.".to_string())
}

fn extract_pdf_document(file_path: &str) -> Result<DocumentExtraction, String> {
    const MAX_PDF_BYTES: u64 = 25 * 1024 * 1024;
    const MAX_PDF_PAGES: usize = 30;
    let metadata = std::fs::metadata(file_path).map_err(|e| e.to_string())?;
    if metadata.len() > MAX_PDF_BYTES {
        return Err("PDF is larger than the 25 MB verification limit.".to_string());
    }
    let pdf_bytes = std::fs::read(file_path).map_err(|e| e.to_string())?;
    let native_result =
        contain_extractor_unwind(|| pdf_extract::extract_text_from_mem_by_pages(&pdf_bytes));
    let ocr_result = ocr_pdf_pages(file_path, MAX_PDF_PAGES);
    let mut diagnostics = Vec::new();
    let native_pages = match native_result {
        Ok(pages) => pages,
        Err(error) => {
            diagnostics.push(error);
            Vec::new()
        }
    };
    let ocr_pages = match ocr_result {
        Ok(pages) => pages,
        Err(error) => {
            diagnostics.push(error);
            Vec::new()
        }
    };
    let page_count = native_pages.len().max(ocr_pages.len());
    if page_count == 0 {
        return Err(if diagnostics.is_empty() {
            "PDF contains no readable pages.".to_string()
        } else {
            diagnostics.join(" ")
        });
    }
    if page_count > MAX_PDF_PAGES {
        return Err(format!(
            "PDF has {page_count} pages; the verification limit is {MAX_PDF_PAGES} pages."
        ));
    }
    let pages = (0..page_count)
        .map(|index| ExtractedPage {
            page_number: index as i32 + 1,
            native_text: native_pages.get(index).cloned().unwrap_or_default(),
            ocr_text: ocr_pages.get(index).cloned().unwrap_or_default(),
            diagnostics: Vec::new(),
        })
        .collect();
    Ok(DocumentExtraction { pages, diagnostics })
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
         split_basis, identity_rule_type, identity_rule_operator, identity_label,
         identity_value, identity_alternate_label, identity_alternate_value
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
            identity_rule_type: row.get(16)?,
            identity_rule_operator: row.get(17)?,
            identity_label: row.get(18)?,
            identity_value: row.get(19)?,
            identity_alternate_label: row.get(20)?,
            identity_alternate_value: row.get(21)?,
        })
    })
    .map(|rows| rows.filter_map(|r| r.ok()).collect())
    .unwrap_or_default()
}

// ─── Billing Period Commands ────────────────────────────────────────────────

pub(crate) const PERIOD_STATUS_DRAFT: &str = "draft";
pub(crate) const PERIOD_STATUS_CLOSED: &str = "closed";

pub(crate) fn billing_period_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<BillingPeriod> {
    Ok(BillingPeriod {
        id: Some(row.get(0)?),
        building_id: row.get(1)?,
        month: row.get(2)?,
        year: row.get(3)?,
        status: row.get(4)?,
        closed_at: row.get(5)?,
        created_at: row.get(6)?,
    })
}

pub(crate) fn load_billing_period_by_id(
    conn: &Connection,
    billing_period_id: i64,
) -> Result<BillingPeriod, String> {
    conn.query_row(
        "SELECT id, building_id, month, year, status, closed_at, created_at
         FROM billing_periods WHERE id=?1",
        [billing_period_id],
        billing_period_from_row,
    )
    .map_err(|e| e.to_string())
}

pub(crate) fn ensure_period_open(conn: &Connection, billing_period_id: i64) -> Result<(), String> {
    let status: String = conn
        .query_row(
            "SELECT status FROM billing_periods WHERE id=?1",
            [billing_period_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    if status == PERIOD_STATUS_CLOSED {
        return Err(
            "This billing month is closed. Reopen the month before changing bills, splits, or delivery."
                .to_string(),
        );
    }
    Ok(())
}

#[tauri::command]
pub fn get_billing_periods(db: State<DbState>) -> Result<Vec<BillingPeriod>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, building_id, month, year, status, closed_at, created_at
             FROM billing_periods ORDER BY year DESC, month DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], billing_period_from_row)
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
        "SELECT id, building_id, month, year, status, closed_at, created_at
         FROM billing_periods WHERE building_id=1 AND month=?1 AND year=?2",
        params![month, year],
        billing_period_from_row,
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_billing_period(db: State<DbState>, id: i64) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    delete_billing_period_inner(&conn, id)
}

fn delete_billing_period_inner(conn: &Connection, id: i64) -> Result<(), String> {
    ensure_period_open(conn, id)?;
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
            "SELECT id, building_id, month, year, status, closed_at, created_at
             FROM billing_periods WHERE year=?1 ORDER BY month ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([year], billing_period_from_row)
        .map_err(|e| e.to_string())?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

// ─── Bill Commands ──────────────────────────────────────────────────────────

fn bill_has_review_warning(parse_note: &str, status: &str) -> bool {
    !parse_note.trim().is_empty() || status == "needs_review"
}

fn persisted_bill_fields_changed(previous: &Bill, next: &Bill) -> bool {
    previous.amount_cents != next.amount_cents
        || previous.creditor_name != next.creditor_name
        || previous.creditor_iban != next.creditor_iban
        || previous.creditor_address != next.creditor_address
        || previous.creditor_city != next.creditor_city
        || previous.creditor_postal_code != next.creditor_postal_code
        || previous.reference != next.reference
        || previous.due_date != next.due_date
        || previous.purpose_code != next.purpose_code
        || previous.purpose_text != next.purpose_text
        || previous.invoice_number != next.invoice_number
        || previous.parse_note != next.parse_note
        || previous.status != next.status
}

fn bill_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Bill> {
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
        reviewed_at: row.get(18)?,
        review_note: row.get(19)?,
        identity_status: row.get(20)?,
        identity_rule_snapshot: row.get(21)?,
        identity_evidence: row.get(22)?,
        identity_exception_note: row.get(23)?,
        identity_exception_at: row.get(24)?,
        source_page_start: row.get(25)?,
        source_page_end: row.get(26)?,
        provider_name: row.get(27)?,
    })
}

fn load_bill_by_id(conn: &Connection, id: i64) -> Result<Bill, String> {
    conn.query_row(
        "SELECT b.id, b.billing_period_id, b.provider_id, b.raw_text, b.amount_cents,
         b.creditor_name, b.creditor_iban, b.creditor_address, b.creditor_city,
         b.creditor_postal_code, b.reference, b.due_date, b.purpose_code, b.purpose_text,
         b.invoice_number, b.parse_note, b.status, b.source_filename,
         b.reviewed_at, b.review_note, b.identity_status, b.identity_rule_snapshot,
         b.identity_evidence, b.identity_exception_note, b.identity_exception_at,
         b.source_page_start, b.source_page_end, p.name as provider_name
         FROM bills b
         LEFT JOIN providers p ON b.provider_id = p.id
         WHERE b.id = ?1",
        [id],
        bill_from_row,
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_bills(db: State<DbState>, billing_period_id: i64) -> Result<Vec<Bill>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT b.id, b.billing_period_id, b.provider_id, b.raw_text, b.amount_cents,
             b.creditor_name, b.creditor_iban, b.creditor_address, b.creditor_city,
             b.creditor_postal_code, b.reference, b.due_date, b.purpose_code, b.purpose_text,
             b.invoice_number, b.parse_note, b.status, b.source_filename,
             b.reviewed_at, b.review_note, b.identity_status, b.identity_rule_snapshot,
             b.identity_evidence, b.identity_exception_note, b.identity_exception_at,
             b.source_page_start, b.source_page_end, p.name as provider_name
             FROM bills b
             LEFT JOIN providers p ON b.provider_id = p.id
             WHERE b.billing_period_id = ?1
             ORDER BY b.id",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([billing_period_id], bill_from_row)
        .map_err(|e| e.to_string())?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

#[tauri::command]
pub fn save_bill(db: State<DbState>, bill: Bill) -> Result<Bill, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    save_bill_inner(&conn, bill)
}

fn save_bill_inner(conn: &Connection, bill: Bill) -> Result<Bill, String> {
    match bill.id {
        Some(id) => {
            let previous = load_bill_by_id(conn, id)?;
            ensure_period_open(conn, previous.billing_period_id)?;
            let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
            let meaningful_change = persisted_bill_fields_changed(&previous, &bill);
            tx.execute(
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
            if meaningful_change && bill_has_review_warning(&bill.parse_note, &bill.status) {
                tx.execute(
                    "UPDATE bills SET reviewed_at=NULL, review_note='' WHERE id=?1",
                    [id],
                )
                .map_err(|e| e.to_string())?;
            }
            if meaningful_change && previous.identity_status == "exception" {
                tx.execute(
                    "UPDATE bills
                     SET identity_status='not_checked', identity_exception_note='',
                         identity_exception_at=NULL
                     WHERE id=?1",
                    [id],
                )
                .map_err(|e| e.to_string())?;
            }
            let saved = load_bill_by_id(&tx, id)?;
            tx.commit().map_err(|e| e.to_string())?;
            Ok(saved)
        }
        None => {
            ensure_period_open(conn, bill.billing_period_id)?;
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
            load_bill_by_id(conn, id)
        }
    }
}

#[tauri::command]
pub fn mark_bill_reviewed(
    db: State<DbState>,
    bill_id: i64,
    review_note: String,
) -> Result<Bill, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    mark_bill_reviewed_inner(&conn, bill_id, &review_note)
}

fn mark_bill_reviewed_inner(
    conn: &Connection,
    bill_id: i64,
    review_note: &str,
) -> Result<Bill, String> {
    let bill = load_bill_by_id(conn, bill_id)?;
    ensure_period_open(conn, bill.billing_period_id)?;
    if bill_has_review_warning(&bill.parse_note, &bill.status) {
        conn.execute(
            "UPDATE bills SET reviewed_at=datetime('now'), review_note=?1 WHERE id=?2",
            params![review_note.trim(), bill_id],
        )
        .map_err(|e| e.to_string())?;
    }
    load_bill_by_id(conn, bill_id)
}

#[tauri::command]
pub fn mark_bill_unreviewed(db: State<DbState>, bill_id: i64) -> Result<Bill, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    mark_bill_unreviewed_inner(&conn, bill_id)
}

fn mark_bill_unreviewed_inner(conn: &Connection, bill_id: i64) -> Result<Bill, String> {
    let bill = load_bill_by_id(conn, bill_id)?;
    ensure_period_open(conn, bill.billing_period_id)?;
    conn.execute(
        "UPDATE bills SET reviewed_at=NULL, review_note='' WHERE id=?1",
        [bill_id],
    )
    .map_err(|e| e.to_string())?;
    load_bill_by_id(conn, bill_id)
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
    let billing_period_id: i64 = conn
        .query_row(
            "SELECT billing_period_id FROM bills WHERE id=?1",
            [id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    ensure_period_open(&conn, billing_period_id)?;
    conn.execute("DELETE FROM bill_splits WHERE bill_id=?1", [id])
        .map_err(|e| e.to_string())?;
    delete_inbox_imports_for_bill(&conn, id)?;
    conn.execute("DELETE FROM inbox_bill_hashes WHERE bill_id=?1", [id])
        .map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM bills WHERE id=?1", [id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

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
    source_page_start: Option<i32>,
    source_page_end: Option<i32>,
    identity: Option<IdentityVerification>,
    segment_text: String,
}

pub(crate) fn ensure_period_bills_identity_eligible(
    conn: &Connection,
    billing_period_id: i64,
) -> Result<(), String> {
    let blocked: Option<(String, String)> = conn
        .query_row(
            "SELECT COALESCE(p.name, b.source_filename), b.identity_status
             FROM bills b
             LEFT JOIN providers p ON p.id=b.provider_id
             WHERE b.billing_period_id=?1
               AND NOT (
                    b.identity_status='matched'
                    OR (b.identity_status='exception' AND trim(b.identity_exception_note) != '')
               )
             ORDER BY b.id LIMIT 1",
            [billing_period_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    if let Some((label, status)) = blocked {
        return Err(format!(
            "Cannot calculate or edit splits because {label} has building identity status '{status}'. Verify it or approve a noted exception first."
        ));
    }
    Ok(())
}

fn extract_document_from_file(file_path: &str) -> Result<DocumentExtraction, String> {
    let path = Path::new(file_path);
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .unwrap_or_default();
    match extension.as_str() {
        "pdf" => extract_pdf_document(file_path),
        _ if is_supported_image_file(path) => {
            let text = extract_text_from_image(file_path)?;
            Ok(DocumentExtraction {
                pages: vec![ExtractedPage {
                    page_number: 1,
                    native_text: String::new(),
                    ocr_text: text,
                    diagnostics: Vec::new(),
                }],
                diagnostics: Vec::new(),
            })
        }
        _ => Err(format!(
            "Unsupported bill file type: {}. Supported files: PDF, JPG, JPEG, PNG, BMP, TIF, TIFF.",
            extension
        )),
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IdentityVerification {
    pub status: String,
    pub explanation: String,
    pub expected_values: Vec<String>,
    pub found_values: Vec<String>,
    pub page_start: Option<i32>,
    pub page_end: Option<i32>,
    pub rule_snapshot: String,
}

fn fold_identity_char(ch: char) -> char {
    match ch.to_uppercase().next().unwrap_or(ch) {
        'Č' | 'Ć' => 'C',
        'Š' => 'S',
        'Ž' => 'Z',
        other => other,
    }
}

fn compact_identity_text(value: &str) -> String {
    value
        .chars()
        .map(fold_identity_char)
        .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '/' | '-'))
        .collect()
}

fn edit_distance(left: &str, right: &str) -> usize {
    let right: Vec<char> = right.chars().collect();
    let mut previous: Vec<usize> = (0..=right.len()).collect();
    for (left_index, left_char) in left.chars().enumerate() {
        let mut current = vec![left_index + 1];
        for (right_index, right_char) in right.iter().enumerate() {
            current.push(
                (previous[right_index + 1] + 1)
                    .min(current[right_index] + 1)
                    .min(previous[right_index] + usize::from(left_char != *right_char)),
            );
        }
        previous = current;
    }
    previous[right.len()]
}

fn identity_label_end(text: &str, label: &str) -> Option<usize> {
    if let Some(offset) = text.find(label) {
        return Some(offset + label.len());
    }
    // OCR commonly drops or substitutes a few characters in human-readable
    // labels. Only the label is matched approximately; the configured account
    // value must still occur exactly (after separator/spacing normalization).
    let tolerance = (label.len() / 5).clamp(1, 3);
    let min_len = label.len().saturating_sub(tolerance);
    let max_len = label.len() + tolerance;
    for start in 0..text.len() {
        for length in min_len..=max_len {
            let end = start + length;
            if end <= text.len() && edit_distance(&text[start..end], label) <= tolerance {
                return Some(end);
            }
        }
    }
    None
}

fn provider_rule_snapshot(provider: &Provider) -> String {
    serde_json::json!({
        "type": provider.identity_rule_type,
        "operator": provider.identity_rule_operator,
        "label": provider.identity_label,
        "value": provider.identity_value,
        "alternate_label": provider.identity_alternate_label,
        "alternate_value": provider.identity_alternate_value,
    })
    .to_string()
}

fn compact_identity_projection(text: &str) -> (String, Vec<usize>, Vec<char>) {
    let folded: Vec<char> = text.chars().map(fold_identity_char).collect();
    let mut compact = String::new();
    let mut positions = Vec::new();
    for (index, ch) in folded.iter().copied().enumerate() {
        if ch.is_ascii_alphanumeric() {
            compact.push(ch.to_ascii_uppercase());
            positions.push(index);
        }
    }
    (compact, positions, folded)
}

fn normalize_identity_value(value: &str) -> String {
    value
        .chars()
        .map(fold_identity_char)
        .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '/' | '-'))
        .map(|ch| ch.to_ascii_uppercase())
        .collect()
}

fn contains_complete_normalized_value(text: &str, expected: &str) -> bool {
    let expected = normalize_identity_value(expected);
    if expected.is_empty() {
        return false;
    }
    let mut value_pattern = String::new();
    for (index, ch) in expected.chars().enumerate() {
        if index > 0 {
            value_pattern.push_str(r"\s*");
        }
        value_pattern.push_str(&regex::escape(&ch.to_string()));
    }
    let folded_text: String = text.chars().map(fold_identity_char).collect();
    let Ok(pattern) = Regex::new(&value_pattern) else {
        return false;
    };
    let found = pattern.find_iter(&folded_text).any(|matched| {
        let previous = folded_text[..matched.start()].chars().next_back();
        if previous.is_some_and(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '/' | '-')) {
            return false;
        }
        let following = &folded_text[matched.end()..];
        let immediate = following.chars().next();
        if immediate.is_some_and(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '/' | '-')) {
            return false;
        }
        let continuation = following
            .chars()
            .skip_while(|ch| matches!(ch, ' ' | '\t' | '\u{00a0}'))
            .next();
        !continuation.is_some_and(|ch| ch.is_ascii_digit() || matches!(ch, '/' | '-'))
    });
    found
}

fn identity_label_value_start(line: &str, label: &str) -> Option<usize> {
    let (compact_text, compact_positions, _) = compact_identity_projection(line);
    let compact_label = compact_identity_text(label);
    let bookend_label = || {
        let tokens: Vec<String> = label
            .split_whitespace()
            .map(compact_identity_text)
            .filter(|token| token.len() >= 3)
            .collect();
        let first = tokens.first()?;
        let last = tokens.last()?;
        if first == last {
            return None;
        }
        let start = compact_text.find(first)? + first.len();
        let bounded_end = (start + 80).min(compact_text.len());
        compact_text[start..bounded_end]
            .find(last)
            .map(|offset| start + offset + last.len())
    };
    let value_start = identity_label_end(&compact_text, &compact_label).or_else(bookend_label)?;
    Some(
        value_start
            .checked_sub(1)
            .and_then(|index| compact_positions.get(index))
            .map(|index| index + 1)
            .unwrap_or(0),
    )
}

fn complete_identity_value(value_text: &str, expected: &str) -> String {
    let chars: Vec<char> = value_text.chars().map(fold_identity_char).collect();
    let Some(start) = chars.iter().position(|ch| ch.is_ascii_alphanumeric()) else {
        return String::new();
    };
    let expected_is_numeric = normalize_identity_value(expected)
        .chars()
        .all(|ch| ch.is_ascii_digit() || matches!(ch, '/' | '-'));
    let mut value = String::new();
    let mut index = start;
    while let Some(ch) = chars.get(index).copied() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '/' | '-') {
            value.push(ch);
            index += 1;
            continue;
        }
        if ch.is_whitespace() {
            let next = chars[index + 1..]
                .iter()
                .copied()
                .find(|next| !next.is_whitespace());
            let can_join_numeric_piece = expected_is_numeric
                && next.is_some_and(|next| next.is_ascii_digit() || matches!(next, '/' | '-'))
                && value
                    .chars()
                    .all(|current| current.is_ascii_digit() || matches!(current, '/' | '-'));
            if can_join_numeric_piece {
                index += 1;
                continue;
            }
        }
        break;
    }
    normalize_identity_value(&value)
}

fn labeled_value_result(text: &str, label: &str, expected: &str) -> (bool, bool, String) {
    let lines: Vec<&str> = text.lines().collect();
    let expected = normalize_identity_value(expected);
    let mut label_found = false;
    let mut first_candidate = String::new();
    for (line_index, line) in lines.iter().enumerate() {
        let Some(value_start) = identity_label_value_start(line, label) else {
            continue;
        };
        label_found = true;
        let line_chars: Vec<char> = line.chars().collect();
        let mut candidate = complete_identity_value(
            &line_chars[value_start.min(line_chars.len())..]
                .iter()
                .collect::<String>(),
            &expected,
        );
        if candidate.is_empty() {
            candidate = lines[line_index + 1..]
                .iter()
                .find(|next| !next.trim().is_empty())
                .map(|next| complete_identity_value(next, &expected))
                .unwrap_or_default();
        }
        if candidate == expected && !expected.is_empty() {
            return (true, true, candidate);
        }
        if first_candidate.is_empty() && !candidate.is_empty() {
            first_candidate = candidate;
        }
    }
    (label_found, false, first_candidate)
}

fn address_result(text: &str, expected: &str) -> bool {
    let expected_compact = compact_identity_text(expected);
    let Some(house_start) = expected_compact.find(|ch: char| ch.is_ascii_digit()) else {
        return false;
    };
    let (street, house) = expected_compact.split_at(house_start);
    if street.is_empty() || house.is_empty() {
        return false;
    }
    let folded: Vec<char> = text.chars().map(fold_identity_char).collect();
    let mut compact = String::new();
    let mut positions = Vec::new();
    for (index, ch) in folded.iter().copied().enumerate() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '/' | '-') {
            compact.push(ch);
            positions.push(index);
        }
    }
    ["", "ULICA", "CESTA"].iter().any(|middle| {
        let phrase = format!("{street}{middle}{house}");
        compact.match_indices(&phrase).any(|(offset, _)| {
            let start = positions[offset];
            let end = positions[offset + phrase.len() - 1] + 1;
            let starts_at_boundary = start == 0 || !folded[start - 1].is_ascii_alphanumeric();
            let ends_at_boundary = folded
                .get(end)
                .is_none_or(|next| !next.is_ascii_alphanumeric() && !matches!(next, '/' | '-'));
            // OCR may space out another house-number digit; a separated single
            // letter can likewise be a suffix. Neither may extend the match.
            let separator_count = folded[end..]
                .iter()
                .copied()
                .take_while(|ch| ch.is_whitespace() || *ch == '.')
                .count();
            let continuation = &folded[end + separator_count..];
            let has_separated_house_continuation = separator_count > 0
                && (continuation.first().is_some_and(|ch| ch.is_ascii_digit())
                    || (continuation
                        .first()
                        .is_some_and(|ch| ch.is_ascii_alphabetic())
                        && continuation
                            .get(1)
                            .is_none_or(|ch| !ch.is_ascii_alphanumeric())));
            starts_at_boundary && ends_at_boundary && !has_separated_house_continuation
        })
    })
}

fn verify_provider_identity(
    provider: &Provider,
    text: &str,
    page_start: Option<i32>,
    page_end: Option<i32>,
    extraction_unreadable: bool,
) -> IdentityVerification {
    let snapshot = provider_rule_snapshot(provider);
    if provider.identity_rule_type == "unconfigured" {
        return IdentityVerification {
            status: "unconfigured".to_string(),
            explanation: "This provider has no building identity rule configured.".to_string(),
            expected_values: Vec::new(),
            found_values: Vec::new(),
            page_start,
            page_end,
            rule_snapshot: snapshot,
        };
    }

    if provider.identity_rule_type == "building_address" {
        let matched = address_result(text, &provider.identity_value);
        return IdentityVerification {
            status: if matched {
                "matched"
            } else if extraction_unreadable {
                "unreadable"
            } else {
                "missing"
            }
            .to_string(),
            explanation: if matched {
                "Temporary weaker evidence: the configured building street and house number appear together in a relevant invoice segment.".to_string()
            } else {
                "The configured building street and house number were not found together in the invoice segment.".to_string()
            },
            expected_values: vec![provider.identity_value.clone()],
            found_values: Vec::new(),
            page_start,
            page_end,
            rule_snapshot: snapshot,
        };
    }

    let mut clauses = vec![(
        provider.identity_label.as_str(),
        provider.identity_value.as_str(),
    )];
    if !provider.identity_alternate_label.trim().is_empty()
        && !provider.identity_alternate_value.trim().is_empty()
    {
        clauses.push((
            provider.identity_alternate_label.as_str(),
            provider.identity_alternate_value.as_str(),
        ));
    }
    let results: Vec<_> = clauses
        .iter()
        .map(|(label, value)| labeled_value_result(text, label, value))
        .collect();
    let matched = if provider.identity_rule_operator == "any" {
        results.iter().any(|(_, matched, _)| *matched)
    } else {
        results.iter().all(|(_, matched, _)| *matched)
    };
    let any_label = results.iter().any(|(found, _, _)| *found);
    let all_labels = results.iter().all(|(found, _, _)| *found);
    let status = if matched {
        "matched"
    } else if provider.identity_rule_operator == "all" && !all_labels {
        if extraction_unreadable {
            "unreadable"
        } else {
            "missing"
        }
    } else if any_label {
        "mismatched"
    } else if extraction_unreadable {
        "unreadable"
    } else {
        "missing"
    };
    IdentityVerification {
        status: status.to_string(),
        explanation: match status {
            "matched" => format!(
                "Configured {} identity rule matched this invoice segment.",
                provider.identity_rule_operator
            ),
            "mismatched" => {
                "A configured identity label was found, but its value did not satisfy the rule."
                    .to_string()
            }
            "unreadable" => {
                "Identity evidence could not be read because document extraction or OCR failed."
                    .to_string()
            }
            _ => "No configured identity label was found in this invoice segment.".to_string(),
        },
        expected_values: clauses
            .iter()
            .map(|(_, value)| (*value).to_string())
            .collect(),
        found_values: results
            .iter()
            .filter(|(found, _, _)| *found)
            .map(|(_, _, window)| window.clone())
            .collect(),
        page_start,
        page_end,
        rule_snapshot: snapshot,
    }
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
            source_page_start: None,
            source_page_end: None,
            identity: None,
            segment_text: String::new(),
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
        source_page_start: None,
        source_page_end: None,
        identity: None,
        segment_text: String::new(),
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
        source_page_start: None,
        source_page_end: None,
        identity: None,
        segment_text: String::new(),
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
        source_page_start: None,
        source_page_end: None,
        identity: None,
        segment_text: String::new(),
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
    pub identity: IdentityVerification,
    pub content_hash: String,
    pub source_page_start: Option<i32>,
    pub source_page_end: Option<i32>,
}

pub(crate) struct PreparedBillImport {
    filename: String,
    source_month: i32,
    source_year: i32,
    detected_source_period: Option<(i32, i32)>,
    extracted: Vec<ExtractedBill>,
    log: String,
    redact_details: bool,
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
    let payment_hash = bill_content_hash(
        provider.and_then(|p| p.id),
        &bill.iban_norm,
        bill.amount_cents,
        &bill.reference,
        &bill.due_date,
        &bill.invoice_number,
    );
    let source_hash = Sha256::digest(bill.segment_text.as_bytes());
    format!(
        "{payment_hash}:{}",
        source_hash
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    )
}

fn associate_provider<'a>(providers: &'a [Provider], bill: &ExtractedBill) -> Option<&'a Provider> {
    if let Some(provider) = providers.iter().find(|provider| {
        !provider.creditor_iban.trim().is_empty()
            && normalize_iban(&provider.creditor_iban) == bill.iban_norm
    }) {
        return Some(provider);
    }
    let matches: Vec<&Provider> = providers
        .iter()
        .filter(|provider| {
            !provider.match_pattern.trim().is_empty()
                && Regex::new(&format!("(?s){}", provider.match_pattern))
                    .map(|pattern| pattern.is_match(&bill.segment_text))
                    .unwrap_or(false)
        })
        .collect();
    (matches.len() == 1).then(|| matches[0])
}

pub(crate) fn retain_new_bill_hashes(
    prepared: &mut PreparedBillImport,
    providers: &[Provider],
    existing_hashes: &HashSet<String>,
) -> BillHashFilterResult {
    let mut seen_in_attachment = HashSet::new();
    let mut kept = Vec::new();
    let mut kept_hashes = Vec::new();
    let mut skipped_duplicate_count = 0;

    for bill in prepared.extracted.drain(..) {
        let provider = associate_provider(providers, &bill);
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
    prepared
        .extracted
        .iter()
        .map(|eb| {
            let provider = associate_provider(providers, eb);
            let mut parse_note = import_review_parse_note(
                &eb.parse_note,
                eb.amount_cents,
                &eb.iban_norm,
                &eb.reference,
                &eb.due_date,
            );
            if provider.is_some_and(|provider| normalize_iban(&provider.creditor_iban) != eb.iban_norm) {
                parse_note = append_parse_note(
                    &parse_note,
                    "Invoice IBAN differs from the configured provider IBAN; verify bank details before payment.",
                );
            }
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
            let identity = eb.identity.clone().unwrap_or_else(|| IdentityVerification {
                status: "not_checked".to_string(),
                explanation:
                    "This candidate has not been checked against a building identity rule."
                        .to_string(),
                expected_values: Vec::new(),
                found_values: Vec::new(),
                page_start: eb.source_page_start,
                page_end: eb.source_page_end,
                rule_snapshot: String::new(),
            });

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
                content_hash: bill_hash(provider, eb),
                identity,
                source_page_start: eb.source_page_start,
                source_page_end: eb.source_page_end,
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
    let document = extract_document_from_file(file_path)?;
    Ok(prepare_multi_bill_import_from_document(
        document,
        source_filename,
        context.month,
        context.year,
        &context.providers,
        include_raw_text_in_log,
    ))
}

#[tauri::command]
pub fn approve_bill_identity_exception(
    db: State<DbState>,
    bill_id: i64,
    note: String,
) -> Result<Bill, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    approve_bill_identity_exception_inner(&conn, bill_id, &note)
}

fn approve_bill_identity_exception_inner(
    conn: &Connection,
    bill_id: i64,
    note: &str,
) -> Result<Bill, String> {
    let note = note.trim();
    if note.is_empty() {
        return Err("An identity exception requires a note.".to_string());
    }
    let bill = load_bill_by_id(conn, bill_id)?;
    ensure_period_open(conn, bill.billing_period_id)?;
    let snapshot = if let Some(provider_id) = bill.provider_id {
        let providers = get_providers_inner(conn);
        providers
            .iter()
            .find(|provider| provider.id == Some(provider_id))
            .map(provider_rule_snapshot)
            .unwrap_or_default()
    } else {
        String::new()
    };
    let evidence = serde_json::json!({
        "status": bill.identity_status,
        "explanation": "Explicit manual exception approved without changing the underlying verification result.",
        "previous_evidence": bill.identity_evidence,
    })
    .to_string();
    conn.execute(
        "UPDATE bills
         SET identity_status='exception', identity_rule_snapshot=?1,
             identity_evidence=?2, identity_exception_note=?3,
             identity_exception_at=datetime('now')
         WHERE id=?4",
        params![snapshot, evidence, note, bill_id],
    )
    .map_err(|e| e.to_string())?;
    load_bill_by_id(conn, bill_id)
}

fn parse_page_candidates(text: &str) -> Vec<ExtractedBill> {
    let mut candidates = parse_upn_stubs(text);
    let mut seen_stubs = HashSet::new();
    candidates.retain(|candidate| {
        seen_stubs.insert(bill_content_hash(
            None,
            &candidate.iban_norm,
            candidate.amount_cents,
            &candidate.reference,
            &candidate.due_date,
            &candidate.invoice_number,
        ))
    });
    for candidate in [
        parse_elektro_style(text),
        parse_zlm_style(text),
        parse_dimnikar_style(text),
    ]
    .into_iter()
    .flatten()
    {
        let fingerprint = bill_content_hash(
            None,
            &candidate.iban_norm,
            candidate.amount_cents,
            &candidate.reference,
            &candidate.due_date,
            &candidate.invoice_number,
        );
        let already_represented = candidates.iter().any(|existing| {
            bill_content_hash(
                None,
                &existing.iban_norm,
                existing.amount_cents,
                &existing.reference,
                &existing.due_date,
                &existing.invoice_number,
            ) == fingerprint
        });
        if !already_represented {
            candidates.push(candidate);
        }
    }
    candidates
}

fn same_payment_candidate(left: &ExtractedBill, right: &ExtractedBill) -> bool {
    if left.iban_norm != right.iban_norm || left.amount_cents != right.amount_cents {
        return false;
    }
    let left_reference = compact_identity_text(&left.reference);
    let right_reference = compact_identity_text(&right.reference);
    if !left_reference.is_empty() && left_reference == right_reference {
        return true;
    }
    let left_invoice = compact_identity_text(&left.invoice_number);
    let right_invoice = compact_identity_text(&right.invoice_number);
    !left_invoice.is_empty() && left_invoice == right_invoice
}

fn payment_candidate_usable(candidate: &ExtractedBill) -> bool {
    candidate.amount_cents > 0
        && !candidate.iban_norm.is_empty()
        && !compact_identity_text(&candidate.reference).is_empty()
}

fn reconcile_page_candidates(page: &ExtractedPage) -> Vec<ExtractedBill> {
    let mut candidates = parse_page_candidates(&page.native_text);
    let ocr_candidates = parse_page_candidates(&page.ocr_text);
    if !candidates.iter().any(payment_candidate_usable)
        && ocr_candidates.iter().any(payment_candidate_usable)
    {
        return ocr_candidates;
    }
    for mut ocr_candidate in ocr_candidates {
        if let Some(native_candidate) = candidates
            .iter_mut()
            .find(|candidate| same_payment_candidate(candidate, &ocr_candidate))
        {
            if native_candidate.reference.is_empty() {
                native_candidate.reference = std::mem::take(&mut ocr_candidate.reference);
            }
            if native_candidate.due_date.is_empty() {
                native_candidate.due_date = std::mem::take(&mut ocr_candidate.due_date);
            }
            if native_candidate.invoice_number.is_empty() {
                native_candidate.invoice_number = std::mem::take(&mut ocr_candidate.invoice_number);
            }
            if native_candidate.purpose_text.is_empty() {
                native_candidate.purpose_text = std::mem::take(&mut ocr_candidate.purpose_text);
            }
        } else {
            candidates.push(ocr_candidate);
        }
    }
    candidates
}

fn page_links_to_payment_candidate(text: &str, candidate: &ExtractedBill) -> bool {
    let reference = normalize_identity_value(&candidate.reference);
    let invoice_number = normalize_identity_value(&candidate.invoice_number);
    (reference.len() >= 6 && contains_complete_normalized_value(text, &reference))
        || (invoice_number.len() >= 5 && contains_complete_normalized_value(text, &invoice_number))
}

fn prepare_multi_bill_import_from_document(
    document: DocumentExtraction,
    filename: String,
    month: i32,
    year: i32,
    providers: &[Provider],
    include_raw_text_in_log: bool,
) -> PreparedBillImport {
    let evidence_texts: Vec<String> = document
        .pages
        .iter()
        .map(ExtractedPage::combined_text)
        .collect();
    let raw_text = evidence_texts.join("\n");
    let detected_source_period = find_source_period_month_year(&raw_text);
    let (source_month, source_year) = detected_source_period.unwrap_or((month, year));
    let mut anchors: Vec<(usize, ExtractedBill)> = Vec::new();
    for (page_index, page) in document.pages.iter().enumerate() {
        for mut candidate in reconcile_page_candidates(page) {
            candidate.source_page_start = Some(document.pages[page_index].page_number);
            candidate.source_page_end = Some(document.pages[page_index].page_number);
            anchors.push((page_index, candidate));
        }
    }
    anchors.sort_by_key(|(page, _)| *page);
    let anchor_pages: Vec<usize> = anchors.iter().map(|(page, _)| *page).collect();
    let anchor_starts: Vec<usize> = anchors
        .iter()
        .enumerate()
        .map(|(index, (page_index, candidate))| {
            let lower_bound = anchor_pages[..index]
                .iter()
                .rev()
                .copied()
                .find(|page| *page != *page_index)
                .map(|page| page + 1)
                .unwrap_or(0);
            (lower_bound..=*page_index)
                .find(|page| page_links_to_payment_candidate(&evidence_texts[*page], candidate))
                .unwrap_or(*page_index)
        })
        .collect();
    let mut extracted = Vec::new();
    let mut seen = HashSet::new();
    for (anchor_index, (page_index, mut candidate)) in anchors.into_iter().enumerate() {
        let same_page_count = anchor_pages
            .iter()
            .filter(|page| **page == page_index)
            .count();
        let start = anchor_starts[anchor_index];
        // A payment anchor proves ownership only through its own page. Trailing
        // pages may belong to an invoice whose payment fields were unreadable;
        // borrowing them would let that invoice verify the preceding bill.
        let end = page_index;
        let (start, end) = if same_page_count > 1 {
            (page_index, page_index)
        } else {
            (start.min(page_index), end.max(page_index))
        };
        candidate.source_page_start = Some(document.pages[start].page_number);
        candidate.source_page_end = Some(document.pages[end].page_number);
        let segment_text = evidence_texts[start..=end].join("\n");
        let extraction_unreadable = document
            .diagnostics
            .iter()
            .any(|diagnostic| !diagnostic.is_empty())
            || document.pages[start..=end]
                .iter()
                .any(|page| !page.diagnostics.is_empty())
            || segment_text.trim().is_empty();
        candidate.segment_text = segment_text.clone();
        candidate.identity = Some(if same_page_count > 1 {
            IdentityVerification {
                status: "unreadable".to_string(),
                explanation: "Multiple distinct invoices share one page and their identity evidence cannot be attributed safely.".to_string(),
                expected_values: Vec::new(),
                found_values: Vec::new(),
                page_start: candidate.source_page_start,
                page_end: candidate.source_page_end,
                rule_snapshot: associate_provider(providers, &candidate)
                    .map(provider_rule_snapshot)
                    .unwrap_or_default(),
            }
        } else if let Some(provider) = associate_provider(providers, &candidate) {
            verify_provider_identity(
                provider,
                &segment_text,
                candidate.source_page_start,
                candidate.source_page_end,
                extraction_unreadable,
            )
        } else {
            IdentityVerification {
                status: "unconfigured".to_string(),
                explanation: "No configured provider could be associated with this invoice."
                    .to_string(),
                expected_values: Vec::new(),
                found_values: Vec::new(),
                page_start: candidate.source_page_start,
                page_end: candidate.source_page_end,
                rule_snapshot: String::new(),
            }
        });
        let key = bill_hash(associate_provider(providers, &candidate), &candidate);
        if seen.insert(key) {
            extracted.push(candidate);
        }
    }
    let raw_log = if include_raw_text_in_log {
        raw_text.as_str()
    } else {
        "(redacted for inbox import)"
    };
    let mut log = format!(
        "=== import_bills: {} ===\n\n--- RAW TEXT ---\n{}\n\n--- PARSE RESULTS ---\n",
        filename, raw_log
    );
    log.push_str(&format!(
        "Page-aware extraction: {} page(s), {} candidate(s), {} document diagnostic(s)\n",
        document.pages.len(),
        extracted.len(),
        document.diagnostics.len()
    ));
    PreparedBillImport {
        filename,
        source_month,
        source_year,
        detected_source_period,
        extracted,
        log,
        redact_details: !include_raw_text_in_log,
    }
}

#[cfg(test)]
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

pub(crate) fn save_prepared_multi_bill_import_with_exceptions(
    conn: &Connection,
    billing_period_id: i64,
    mut prepared: PreparedBillImport,
    providers: &[Provider],
    persist_fallback_raw_text: bool,
    identity_exceptions: &HashMap<String, IdentityExceptionInput>,
) -> Result<Vec<Bill>, String> {
    ensure_period_open(conn, billing_period_id)?;

    for candidate in &mut prepared.extracted {
        if let Some(provider) = associate_provider(providers, candidate) {
            let ambiguous_same_page = candidate
                .identity
                .as_ref()
                .map(|identity| {
                    identity
                        .explanation
                        .starts_with("Multiple distinct invoices share one page")
                })
                .unwrap_or(false);
            if !ambiguous_same_page {
                let extraction_unreadable = candidate.segment_text.trim().is_empty()
                    || candidate
                        .identity
                        .as_ref()
                        .map(|identity| identity.status == "unreadable")
                        .unwrap_or(false);
                candidate.identity = Some(verify_provider_identity(
                    provider,
                    &candidate.segment_text,
                    candidate.source_page_start,
                    candidate.source_page_end,
                    extraction_unreadable,
                ));
            }
        }
    }

    let mut providers_in_batch = HashSet::new();
    for candidate in &prepared.extracted {
        let provider = associate_provider(providers, candidate).ok_or_else(|| {
            "Candidate has no configured provider and cannot be imported automatically.".to_string()
        })?;
        let provider_id = provider
            .id
            .ok_or_else(|| "Candidate provider is not persisted.".to_string())?;
        if !providers_in_batch.insert(provider_id) {
            return Err(format!(
                "Multiple distinct candidates target {} in this billing month. Resolve the conflict before importing.",
                provider.name
            ));
        }
        let existing_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM bills WHERE billing_period_id=?1 AND provider_id=?2",
                params![billing_period_id, provider_id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        if existing_count > 0 {
            return Err(format!(
                "{} already has a bill in this billing month. Remove the existing bill before importing this candidate.",
                provider.name
            ));
        }
        let identity = candidate.identity.as_ref().ok_or_else(|| {
            "Candidate identity was not checked; refresh the preview before importing.".to_string()
        })?;
        if identity.status != "matched" {
            let hash = bill_hash(Some(provider), candidate);
            let exception = identity_exceptions.get(&hash);
            let note = exception
                .map(|exception| exception.note.trim())
                .unwrap_or("");
            if note.is_empty() {
                return Err(format!(
                    "{} is {} and requires an explicit exception note before import.",
                    provider.name, identity.status
                ));
            }
            if exception.is_some_and(|exception| {
                exception.identity_status != identity.status
                    || exception.rule_snapshot != identity.rule_snapshot
            }) {
                return Err(format!(
                    "{} identity evidence changed after preview. Refresh the preview and review it again.",
                    provider.name
                ));
            }
        }
    }

    if prepared.extracted.is_empty() {
        let _ = persist_fallback_raw_text;
        return Err(
            "No invoice candidate could be parsed. Nothing was saved; inspect the source and add a manual bill with an explicit identity exception if appropriate."
                .to_string(),
        );
    }

    let mut results: Vec<Bill> = Vec::new();
    prepared.log.push_str("--- SAVED BILLS ---\n");

    for eb in prepared.extracted {
        let provider = associate_provider(providers, &eb);
        let changed_iban = provider
            .map(|provider| normalize_iban(&provider.creditor_iban) != eb.iban_norm)
            .unwrap_or(false);

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
                if changed_iban {
                    eb.iban_raw.clone()
                } else {
                    p.creditor_iban.clone()
                },
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

        let mut parse_note = import_review_parse_note(
            &eb.parse_note,
            eb.amount_cents,
            &eb.iban_norm,
            &eb.reference,
            &eb.due_date,
        );
        if changed_iban {
            parse_note = append_parse_note(
                &parse_note,
                "Invoice IBAN differs from the configured provider IBAN; verify bank details before payment.",
            );
        }
        let status = if parse_note.is_empty() {
            "draft".to_string()
        } else {
            "needs_review".to_string()
        };

        let identity = eb.identity.clone().unwrap_or_else(|| IdentityVerification {
            status: "not_checked".to_string(),
            explanation: "Identity was not checked.".to_string(),
            expected_values: Vec::new(),
            found_values: Vec::new(),
            page_start: eb.source_page_start,
            page_end: eb.source_page_end,
            rule_snapshot: String::new(),
        });
        let candidate_hash = bill_hash(provider, &eb);
        let exception_note = if identity.status == "matched" {
            String::new()
        } else {
            identity_exceptions
                .get(&candidate_hash)
                .map(|exception| exception.note.trim().to_string())
                .unwrap_or_default()
        };
        let persisted_identity_status = if exception_note.is_empty() {
            identity.status.clone()
        } else {
            "exception".to_string()
        };
        let identity_evidence = serde_json::to_string(&identity).map_err(|e| e.to_string())?;

        conn.execute(
            "INSERT INTO bills (billing_period_id, provider_id, raw_text, amount_cents,
             creditor_name, creditor_iban, creditor_address, creditor_city,
             creditor_postal_code, reference, due_date, purpose_code, purpose_text,
             invoice_number, parse_note, status, source_filename, identity_status,
             identity_rule_snapshot, identity_evidence, identity_exception_note,
             identity_exception_at, source_page_start, source_page_end)
             VALUES (?1,?2,'',?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,
                     ?17,?18,?19,?20,CASE WHEN ?20='' THEN NULL ELSE datetime('now') END,?21,?22)",
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
                persisted_identity_status,
                identity.rule_snapshot,
                identity_evidence,
                exception_note,
                eb.source_page_start,
                eb.source_page_end,
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
        results.push(load_bill_by_id(conn, id)?);
    }

    write_import_debug_log(&prepared.log);
    Ok(results)
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LocalBillImportPreview {
    pub source_filename: String,
    pub file_sha256: String,
    pub bills: Vec<PreparedBillPreviewSummary>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IdentityExceptionInput {
    pub content_hash: String,
    pub note: String,
    pub identity_status: String,
    pub rule_snapshot: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LocalBillImportFinalizeRequest {
    pub file_path: String,
    pub expected_file_sha256: String,
    pub selected_content_hashes: Vec<String>,
    pub exceptions: Vec<IdentityExceptionInput>,
}

fn file_sha256(file_path: &str) -> Result<String, String> {
    let bytes = std::fs::read(file_path).map_err(|e| e.to_string())?;
    let digest = Sha256::digest(bytes);
    Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
}

#[tauri::command]
pub async fn preview_bill_import(
    app: AppHandle,
    file_path: String,
    billing_period_id: i64,
) -> Result<LocalBillImportPreview, String> {
    tauri::async_runtime::spawn_blocking(move || {
        preview_bill_import_impl(app.state::<DbState>(), file_path, billing_period_id)
    })
    .await
    .map_err(|error| format!("Bill preview task failed: {error}"))?
}

fn preview_bill_import_impl(
    db: State<DbState>,
    file_path: String,
    billing_period_id: i64,
) -> Result<LocalBillImportPreview, String> {
    let context = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        ensure_period_open(&conn, billing_period_id)?;
        load_bill_import_context(&conn, billing_period_id)?
    };
    let source_filename = Path::new(&file_path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(&file_path)
        .to_string();
    let before_hash = file_sha256(&file_path)?;
    let prepared =
        prepare_multi_bill_import_from_path(&file_path, source_filename.clone(), &context, true)?;
    let after_hash = file_sha256(&file_path)?;
    if before_hash != after_hash {
        return Err("The source file changed while it was being inspected. Try again.".to_string());
    }
    let bills = preview_prepared_bills(&prepared, &context.providers);
    if bills.is_empty() {
        return Err("No invoice candidates could be parsed from this file.".to_string());
    }
    Ok(LocalBillImportPreview {
        source_filename,
        file_sha256: after_hash,
        bills,
    })
}

#[tauri::command]
pub async fn finalize_bill_import_batch(
    app: AppHandle,
    billing_period_id: i64,
    files: Vec<LocalBillImportFinalizeRequest>,
) -> Result<Vec<Bill>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        finalize_bill_import_batch_impl(app.state::<DbState>(), billing_period_id, files)
    })
    .await
    .map_err(|error| format!("Bill finalization task failed: {error}"))?
}

fn finalize_bill_import_batch_impl(
    db: State<DbState>,
    billing_period_id: i64,
    files: Vec<LocalBillImportFinalizeRequest>,
) -> Result<Vec<Bill>, String> {
    if files.is_empty()
        || files
            .iter()
            .all(|file| file.selected_content_hashes.is_empty())
    {
        return Err("Select at least one invoice candidate to import.".to_string());
    }

    // Extraction and OCR stay outside the database lock. All resulting inserts
    // are nevertheless committed in one transaction after current-rule checks.
    let mut extracted_files = Vec::with_capacity(files.len());
    for file in files {
        if file.selected_content_hashes.is_empty() {
            continue;
        }
        let before_hash = file_sha256(&file.file_path)?;
        if before_hash != file.expected_file_sha256 {
            return Err(
                "A source file changed after preview. Refresh the preview before importing."
                    .to_string(),
            );
        }
        let document = extract_document_from_file(&file.file_path)?;
        let after_hash = file_sha256(&file.file_path)?;
        if after_hash != before_hash {
            return Err("A source file changed during final verification. Refresh the preview before importing.".to_string());
        }
        let source_filename = Path::new(&file.file_path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(&file.file_path)
            .to_string();
        extracted_files.push((file, source_filename, document));
    }

    let mut conn = db.0.lock().map_err(|e| e.to_string())?;
    ensure_period_open(&conn, billing_period_id)?;
    let context = load_bill_import_context(&conn, billing_period_id)?;
    let mut prepared_files = Vec::new();
    for (file, source_filename, document) in extracted_files {
        let mut prepared = prepare_multi_bill_import_from_document(
            document,
            source_filename,
            context.month,
            context.year,
            &context.providers,
            true,
        );
        let selected: HashSet<String> = file.selected_content_hashes.into_iter().collect();
        let mut found = HashSet::new();
        prepared.extracted.retain(|candidate| {
            let hash = bill_hash(associate_provider(&context.providers, candidate), candidate);
            if !selected.contains(&hash) {
                return false;
            }
            found.insert(hash.clone());
            true
        });
        if found != selected {
            return Err(
                "One or more selected candidates changed after preview. Refresh the preview."
                    .to_string(),
            );
        }
        let exception_map = file
            .exceptions
            .into_iter()
            .map(|exception| (exception.content_hash.clone(), exception))
            .collect();
        prepared_files.push((prepared, exception_map));
    }

    let mut retained_hashes = HashSet::new();
    for (prepared, _) in &mut prepared_files {
        prepared.extracted.retain(|candidate| {
            retained_hashes.insert(bill_hash(
                associate_provider(&context.providers, candidate),
                candidate,
            ))
        });
    }
    if retained_hashes.is_empty() {
        return Err("No selected invoice candidates remained after duplicate checks.".to_string());
    }

    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let mut saved = Vec::new();
    for (prepared, exception_map) in prepared_files {
        if prepared.extracted.is_empty() {
            continue;
        }
        saved.extend(save_prepared_multi_bill_import_with_exceptions(
            &tx,
            billing_period_id,
            prepared,
            &context.providers,
            true,
            &exception_map,
        )?);
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(saved)
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
            identity_rule_type: "unconfigured".to_string(),
            identity_rule_operator: "all".to_string(),
            identity_label: String::new(),
            identity_value: String::new(),
            identity_alternate_label: String::new(),
            identity_alternate_value: String::new(),
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
            source_page_start: None,
            source_page_end: None,
            identity: None,
            segment_text: String::new(),
        }
    }

    fn test_prepared(extracted: Vec<ExtractedBill>) -> PreparedBillImport {
        PreparedBillImport {
            filename: "invoice.pdf".to_string(),
            source_month: 4,
            source_year: 2026,
            detected_source_period: Some((4, 2026)),
            extracted,
            log: String::new(),
            redact_details: true,
        }
    }

    fn setup_bill_command_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE providers (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL
            );
            CREATE TABLE billing_periods (
                id INTEGER PRIMARY KEY,
                building_id INTEGER NOT NULL,
                month INTEGER NOT NULL,
                year INTEGER NOT NULL,
                status TEXT NOT NULL DEFAULT 'draft',
                closed_at TEXT,
                created_at TEXT NOT NULL DEFAULT '2026-06-01 00:00:00'
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
                source_filename TEXT NOT NULL DEFAULT '',
                reviewed_at TEXT,
                review_note TEXT NOT NULL DEFAULT '',
                identity_status TEXT NOT NULL DEFAULT 'matched',
                identity_rule_snapshot TEXT NOT NULL DEFAULT '',
                identity_evidence TEXT NOT NULL DEFAULT '',
                identity_exception_note TEXT NOT NULL DEFAULT '',
                identity_exception_at TEXT,
                source_page_start INTEGER,
                source_page_end INTEGER
            );
            INSERT INTO providers (id, name) VALUES (1, 'Provider 1');
            INSERT INTO billing_periods (id, building_id, month, year, status)
            VALUES (1, 1, 6, 2026, 'draft');
            ",
        )
        .unwrap();
        conn
    }

    fn insert_review_test_bill(conn: &Connection, id: i64, parse_note: &str, reviewed: bool) {
        conn.execute(
            "INSERT INTO bills (
                id, billing_period_id, provider_id, amount_cents, creditor_name, creditor_iban,
                creditor_address, creditor_city, creditor_postal_code, reference, due_date,
                purpose_code, purpose_text, invoice_number, parse_note, status, source_filename,
                reviewed_at, review_note
             ) VALUES (
                ?1, 1, 1, 1000, 'Provider 1', 'SI56 0400 1004 8988 093',
                '', '', '', 'SI00 123', '30.06.2026', 'OTHR', 'Utilities', '',
                ?2, CASE WHEN ?2 = '' THEN 'draft' ELSE 'needs_review' END, 'bill.pdf',
                CASE WHEN ?3 THEN '2026-06-01 00:00:00' ELSE NULL END,
                CASE WHEN ?3 THEN 'Checked' ELSE '' END
             )",
            params![id, parse_note, if reviewed { 1 } else { 0 }],
        )
        .unwrap();
    }

    fn close_test_period(conn: &Connection) {
        conn.execute(
            "UPDATE billing_periods
             SET status='closed', closed_at='2026-06-30 12:00:00'
             WHERE id=1",
            [],
        )
        .unwrap();
    }

    #[test]
    fn mark_bill_reviewed_sets_review_fields_and_preserves_parse_note() {
        let conn = setup_bill_command_conn();
        insert_review_test_bill(&conn, 1, "Review imported amount.", false);

        let bill = mark_bill_reviewed_inner(&conn, 1, " Checked by accountant ").unwrap();

        assert_eq!(bill.parse_note, "Review imported amount.");
        assert_eq!(bill.review_note, "Checked by accountant");
        assert!(bill.reviewed_at.is_some());
    }

    #[test]
    fn mark_bill_reviewed_noops_for_clean_bill() {
        let conn = setup_bill_command_conn();
        insert_review_test_bill(&conn, 1, "", false);

        let bill = mark_bill_reviewed_inner(&conn, 1, "Checked").unwrap();

        assert!(bill.reviewed_at.is_none());
        assert_eq!(bill.review_note, "");
    }

    #[test]
    fn save_bill_preserves_review_on_noop_and_clears_after_meaningful_edit() {
        let conn = setup_bill_command_conn();
        insert_review_test_bill(&conn, 1, "Review imported amount.", true);

        let bill = load_bill_by_id(&conn, 1).unwrap();
        let no_op = save_bill_inner(&conn, bill.clone()).unwrap();
        assert!(no_op.reviewed_at.is_some());
        assert_eq!(no_op.review_note, "Checked");

        let mut edited = no_op;
        edited.amount_cents += 1;
        let saved = save_bill_inner(&conn, edited).unwrap();

        assert!(saved.reviewed_at.is_none());
        assert_eq!(saved.review_note, "");
    }

    #[test]
    fn identity_exception_requires_note_and_is_invalidated_by_edit() {
        let conn = setup_bill_command_conn();
        insert_review_test_bill(&conn, 1, "", false);

        let error = approve_bill_identity_exception_inner(&conn, 1, "   ").unwrap_err();
        assert!(error.contains("requires a note"));

        let approved =
            approve_bill_identity_exception_inner(&conn, 1, "Checked against the supplier portal")
                .unwrap();
        assert_eq!(approved.identity_status, "exception");
        assert_eq!(
            approved.identity_exception_note,
            "Checked against the supplier portal"
        );
        assert!(approved.identity_exception_at.is_some());

        let mut edited = approved;
        edited.amount_cents += 1;
        let saved = save_bill_inner(&conn, edited).unwrap();
        assert_eq!(saved.identity_status, "not_checked");
        assert_eq!(saved.identity_exception_note, "");
        assert!(saved.identity_exception_at.is_none());
    }

    #[test]
    fn save_bill_rejects_closed_period() {
        let conn = setup_bill_command_conn();
        insert_review_test_bill(&conn, 1, "", false);
        close_test_period(&conn);

        let mut bill = load_bill_by_id(&conn, 1).unwrap();
        bill.amount_cents += 1;
        let error = save_bill_inner(&conn, bill).unwrap_err();

        assert!(error.contains("billing month is closed"));
    }

    #[test]
    fn delete_billing_period_rejects_closed_period_before_removing_data() {
        let conn = setup_bill_command_conn();
        insert_review_test_bill(&conn, 1, "", false);
        close_test_period(&conn);

        let error = delete_billing_period_inner(&conn, 1).unwrap_err();

        assert!(error.contains("billing month is closed"));
        let period_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM billing_periods WHERE id=1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let bill_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM bills WHERE billing_period_id=1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(period_count, 1);
        assert_eq!(bill_count, 1);
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
        assert_eq!(
            waste.purpose_text,
            "Ravnanje z odpadki 05/2026 0040113249 15.06.2026"
        );
        assert_eq!(waste.due_date, "15.06.2026");
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
    fn source_aware_hash_keeps_payment_fingerprint_and_distinguishes_documents() {
        let provider = test_provider(7, "SI56 0400 1004 8988 093", "Elektro");
        let mut bill = test_extracted_bill("SI56 0400 1004 8988 093");

        let parsed_hash = bill_hash(Some(&provider), &bill);
        let saved_hash = bill_content_hash(
            Some(7),
            &bill.iban_norm,
            bill.amount_cents,
            &bill.reference,
            &bill.due_date,
            &bill.invoice_number,
        );

        assert!(parsed_hash.starts_with(&format!("{saved_hash}:")));
        bill.segment_text = "A different supplier document with the same payment tuple".to_string();
        assert_ne!(parsed_hash, bill_hash(Some(&provider), &bill));
    }

    fn identity_provider(
        rule_type: &str,
        operator: &str,
        label: &str,
        value: &str,
        alternate_label: &str,
        alternate_value: &str,
    ) -> Provider {
        let mut provider = test_provider(1, "SI56 0400 1004 8988 093", "Provider");
        provider.identity_rule_type = rule_type.to_string();
        provider.identity_rule_operator = operator.to_string();
        provider.identity_label = label.to_string();
        provider.identity_value = value.to_string();
        provider.identity_alternate_label = alternate_label.to_string();
        provider.identity_alternate_value = alternate_value.to_string();
        provider
    }

    #[test]
    fn electricity_any_rule_accepts_either_identifier_even_when_other_differs() {
        let provider = identity_provider(
            "labeled_value",
            "any",
            "Številka kupca",
            "C0367125",
            "Številka merilnega mesta",
            "3-82858",
        );
        for text in [
            "Številka kupca: C0367125",
            "Številka kupca: C0039985\nŠtevilka merilnega mesta: 3-82858",
            "Številka kupca: C0367125\nŠtevilka merilnega mesta: 9-99999",
        ] {
            assert_eq!(
                verify_provider_identity(&provider, text, Some(1), Some(2), false).status,
                "matched"
            );
        }
        assert_eq!(
            verify_provider_identity(
                &provider,
                "Številka kupca: C0000000\nŠtevilka merilnega mesta: 9-99999",
                Some(1),
                Some(2),
                false,
            )
            .status,
            "mismatched"
        );
    }

    #[test]
    fn all_rule_preserves_missing_clause_separately_from_mismatch() {
        let provider = identity_provider("labeled_value", "all", "Customer", "001", "Meter", "002");
        assert_eq!(
            verify_provider_identity(&provider, "Customer: 001", Some(1), Some(1), false).status,
            "missing"
        );
        assert_eq!(
            verify_provider_identity(
                &provider,
                "Customer: 001 Meter: 999",
                Some(1),
                Some(1),
                false,
            )
            .status,
            "mismatched"
        );
    }

    #[test]
    fn labeled_rules_preserve_zeroes_and_separators() {
        for (label, value) in [
            ("PLIN Odjemno mesto", "01505116659"),
            ("Šifra naročnika", "0040113249"),
            ("Št. odjemnega mesta", "5495/109463"),
        ] {
            let provider = identity_provider("labeled_value", "all", label, value, "", "");
            assert_eq!(
                verify_provider_identity(
                    &provider,
                    &format!("{label}: {value}"),
                    Some(1),
                    Some(1),
                    false,
                )
                .status,
                "matched"
            );
            assert_eq!(
                verify_provider_identity(
                    &provider,
                    &format!(
                        "{label}: {}",
                        if value.starts_with('0') {
                            value.trim_start_matches('0').to_string()
                        } else {
                            value.replace('/', "")
                        }
                    ),
                    Some(1),
                    Some(1),
                    false,
                )
                .status,
                "mismatched"
            );
        }
    }

    #[test]
    fn labeled_rule_rejects_extended_value_and_does_not_scan_later_fields() {
        let water = identity_provider(
            "labeled_value",
            "all",
            "Št. odjemnega mesta",
            "5495/109463",
            "",
            "",
        );
        for (text, found) in [
            (
                "Št. odjemnega mesta: 5495/1094630\nDrug podatek: 5495/109463",
                "5495/1094630",
            ),
            (
                "Št. odjemnega mesta: 5495/109463-0\nDrug podatek: 5495/109463",
                "5495/109463-0",
            ),
            (
                "Št. odjemnega mesta: napačno\nDrug podatek: 5495/109463",
                "NAPACNO",
            ),
        ] {
            let result = verify_provider_identity(&water, text, Some(1), Some(1), false);
            assert_eq!(result.status, "mismatched", "{text}");
            assert_eq!(result.found_values, vec![found], "{text}");
        }
    }

    #[test]
    fn labeled_rule_allows_spaces_around_identifier_separators() {
        for (label, expected, extracted) in [
            ("Št. odjemnega mesta", "5495/109463", "5495 / 109463"),
            ("Številka merilnega mesta", "3-82858", "3 - 82858"),
        ] {
            let provider = identity_provider("labeled_value", "all", label, expected, "", "");
            let result = verify_provider_identity(
                &provider,
                &format!("{label}: {extracted}"),
                Some(1),
                Some(1),
                false,
            );
            assert_eq!(result.status, "matched", "{extracted}");
            assert_eq!(result.found_values, vec![expected], "{extracted}");
        }
    }

    #[test]
    fn payment_link_values_require_complete_token_boundaries() {
        for (text, expected_match) in [
            ("Sklic: SI12 111", true),
            ("Sklic: SI12   111", true),
            ("Sklic: SI12 111\n100,00 EUR", true),
            ("Sklic: SI12 111\r\n100,00 EUR", true),
            ("Sklic: SI12 1110", false),
            ("Sklic: SI12 111-0", false),
            ("Sklic: SI12 111 0", false),
            ("Sklic: SI12 111 - 0", false),
            ("Sklic: SI12 111\u{00a0}0", false),
            ("Sklic: 0SI12 111", false),
        ] {
            assert_eq!(
                contains_complete_normalized_value(text, "SI12 111"),
                expected_match,
                "{text}"
            );
        }
    }

    #[test]
    fn ocr_damaged_labels_still_require_the_exact_configured_value() {
        let gas = identity_provider(
            "labeled_value",
            "all",
            "PLIN Odjemno mesto",
            "01505116659",
            "",
            "",
        );
        assert_eq!(
            verify_provider_identity(
                &gas,
                "PLIN j Od mesto: 01 505116659",
                Some(8),
                Some(8),
                false,
            )
            .status,
            "matched"
        );
        assert_eq!(
            verify_provider_identity(
                &gas,
                "PLIN j Od mesto: 01 505116658",
                Some(8),
                Some(8),
                false,
            )
            .status,
            "mismatched"
        );
    }

    #[test]
    fn temporary_zlm_address_rule_is_bounded_to_house_number() {
        let provider = identity_provider(
            "building_address",
            "all",
            "Relevant building address",
            "Kamniška 36",
            "",
            "",
        );
        for text in [
            "Etažni lastniki Kamniška cesta 36, Ljubljana",
            "Redno čiščenje: KAMNISKA\nULICA 3 6",
            "Kamniška 36",
            "Kamniška 36, 100 EUR",
        ] {
            assert_eq!(
                verify_provider_identity(&provider, text, Some(1), Some(1), false).status,
                "matched"
            );
        }
        for text in [
            "Postavka 36, naslov Kamniška 40",
            "Kamniška cesta 360",
            "Kamniška 3 6 0",
            "Kamniška 36\n0",
            "Kamniška 36A",
            "Kamniška 36 A",
            "Kamniška 36.a",
            "PredKamniška 36",
        ] {
            assert_ne!(
                verify_provider_identity(&provider, text, Some(1), Some(1), false).status,
                "matched"
            );
        }
        let suffixed_provider = identity_provider(
            "building_address",
            "all",
            "Relevant building address",
            "Kamniška 36A",
            "",
            "",
        );
        assert_eq!(
            verify_provider_identity(
                &suffixed_provider,
                "Kamniška cesta 36 A, Ljubljana",
                Some(1),
                Some(1),
                false
            )
            .status,
            "matched"
        );
        assert_ne!(
            verify_provider_identity(
                &suffixed_provider,
                "Kamniška cesta 36, Ljubljana",
                Some(1),
                Some(1),
                false
            )
            .status,
            "matched"
        );
    }

    #[test]
    fn same_iban_distinct_upn_stubs_remain_separate() {
        let text = "***10,00\nOTHR First\nSI56 0400 1004 8988 093\nSI12 111\n\n***20,00\nOTHR Second\nSI56 0400 1004 8988 093\nSI12 222";
        let parsed = parse_page_candidates(text);
        assert_eq!(parsed.len(), 2);
        assert_ne!(parsed[0].amount_cents, parsed[1].amount_cents);
    }

    #[test]
    fn identical_upn_stub_repetition_is_not_a_second_invoice() {
        let stub = "***10,00\nOTHR First\nSI56 0400 1004 8988 093\nSI12 111";
        let parsed = parse_page_candidates(&format!("{stub}\n\n{stub}"));
        assert_eq!(parsed.len(), 1);
    }

    #[test]
    fn provider_identity_can_be_evaluated_when_invoice_iban_changed() {
        let mut provider = identity_provider(
            "labeled_value",
            "all",
            "PLIN Odjemno mesto",
            "01505116659",
            "",
            "",
        );
        provider.match_pattern = "(?i)energetika\\s+ljubljana".to_string();
        let mut bill = test_extracted_bill("SI56 9999 9999 9999 999");
        bill.segment_text = "Energetika Ljubljana\nPLIN Odjemno mesto: 01505116659".to_string();

        let associated = associate_provider(std::slice::from_ref(&provider), &bill)
            .expect("supplier match should associate provider");
        assert_eq!(associated.id, provider.id);
        assert_ne!(normalize_iban(&associated.creditor_iban), bill.iban_norm);
        assert_eq!(
            verify_provider_identity(associated, &bill.segment_text, Some(1), Some(1), false)
                .status,
            "matched"
        );
    }

    #[test]
    fn trailing_unparsed_invoice_cannot_verify_preceding_candidate() {
        let provider = identity_provider(
            "labeled_value",
            "all",
            "PLIN Odjemno mesto",
            "01505116659",
            "",
            "",
        );
        let document = DocumentExtraction {
            pages: vec![
                ExtractedPage {
                    page_number: 1,
                    native_text: "***10,00\nENRG Prvi račun\nSI56 0400 1004 8988 093\nSI12 111"
                        .to_string(),
                    ocr_text: String::new(),
                    diagnostics: Vec::new(),
                },
                ExtractedPage {
                    page_number: 2,
                    native_text: "Drug neprepoznan račun\nPLIN Odjemno mesto: 01505116659"
                        .to_string(),
                    ocr_text: String::new(),
                    diagnostics: Vec::new(),
                },
            ],
            diagnostics: Vec::new(),
        };

        let prepared = prepare_multi_bill_import_from_document(
            document,
            "two-invoices.pdf".to_string(),
            8,
            2026,
            &[provider],
            true,
        );

        assert_eq!(prepared.extracted.len(), 1);
        let candidate = &prepared.extracted[0];
        assert_eq!(candidate.source_page_end, Some(1));
        assert_eq!(candidate.identity.as_ref().unwrap().status, "missing");
    }

    #[test]
    fn preceding_unparsed_invoice_cannot_verify_later_candidate() {
        let provider = identity_provider(
            "labeled_value",
            "all",
            "PLIN Odjemno mesto",
            "01505116659",
            "",
            "",
        );
        let document = DocumentExtraction {
            pages: vec![
                ExtractedPage {
                    page_number: 1,
                    native_text: "Drug neprepoznan račun\nPLIN Odjemno mesto: 01505116659\nSI56 0400 1004 8988 093\nSI12 111 0"
                        .to_string(),
                    ocr_text: String::new(),
                    diagnostics: Vec::new(),
                },
                ExtractedPage {
                    page_number: 2,
                    native_text: "***10,00\nENRG Poznejši račun\nSI56 0400 1004 8988 093\nSI12 111"
                        .to_string(),
                    ocr_text: String::new(),
                    diagnostics: Vec::new(),
                },
            ],
            diagnostics: Vec::new(),
        };

        let prepared = prepare_multi_bill_import_from_document(
            document,
            "reverse-two-invoices.pdf".to_string(),
            8,
            2026,
            &[provider],
            true,
        );

        assert_eq!(prepared.extracted.len(), 1);
        let candidate = &prepared.extracted[0];
        assert_eq!(candidate.source_page_start, Some(2));
        assert_eq!(candidate.identity.as_ref().unwrap().status, "missing");
    }

    #[test]
    fn preceding_identity_page_links_when_reference_ends_at_line_boundary() {
        let provider = identity_provider(
            "labeled_value",
            "all",
            "PLIN Odjemno mesto",
            "01505116659",
            "",
            "",
        );
        let document = DocumentExtraction {
            pages: vec![
                ExtractedPage {
                    page_number: 1,
                    native_text: "PLIN Odjemno mesto: 01505116659\nSklic: SI12 111\n100,00 EUR"
                        .to_string(),
                    ocr_text: String::new(),
                    diagnostics: Vec::new(),
                },
                ExtractedPage {
                    page_number: 2,
                    native_text: "***10,00\nENRG Račun za plin\nSI56 0400 1004 8988 093\nSI12 111"
                        .to_string(),
                    ocr_text: String::new(),
                    diagnostics: Vec::new(),
                },
            ],
            diagnostics: Vec::new(),
        };

        let prepared = prepare_multi_bill_import_from_document(
            document,
            "split-invoice.pdf".to_string(),
            8,
            2026,
            &[provider],
            true,
        );

        assert_eq!(prepared.extracted.len(), 1);
        let candidate = &prepared.extracted[0];
        assert_eq!(candidate.source_page_start, Some(1));
        assert_eq!(candidate.source_page_end, Some(2));
        assert_eq!(candidate.identity.as_ref().unwrap().status, "matched");
    }

    #[test]
    fn partial_native_page_falls_back_to_ocr_payment_candidate() {
        let provider = identity_provider(
            "labeled_value",
            "all",
            "PLIN Odjemno mesto",
            "01505116659",
            "",
            "",
        );
        let document = DocumentExtraction {
            pages: vec![ExtractedPage {
                page_number: 1,
                native_text: "Energetika Ljubljana — searchable footer".to_string(),
                ocr_text: "PLIN Odjemno mesto: 01505116659\n***10,00\nENRG Plin\nSI56 0400 1004 8988 093\nSI12 111"
                    .to_string(),
                diagnostics: Vec::new(),
            }],
            diagnostics: Vec::new(),
        };

        let prepared = prepare_multi_bill_import_from_document(
            document,
            "partial-native.pdf".to_string(),
            8,
            2026,
            &[provider],
            true,
        );

        assert_eq!(prepared.extracted.len(), 1);
        assert_eq!(
            prepared.extracted[0].identity.as_ref().unwrap().status,
            "matched"
        );
    }

    #[test]
    fn extractor_panic_is_contained() {
        let result: Result<Vec<String>, String> = contain_extractor_unwind(|| {
            panic!("synthetic extractor panic");
            #[allow(unreachable_code)]
            Ok(Vec::new())
        });
        assert!(result.unwrap_err().contains("panicked"));
    }

    #[test]
    #[ignore = "requires the local Git-ignored whole-house sample and Windows OCR"]
    fn local_august_sample_verifies_all_requested_services() {
        let path = Path::new("../file-examples/Racuni_09_2026_s.pdf");
        if !path.exists() {
            return;
        }
        let conn = Connection::open_in_memory().unwrap();
        crate::db::migrations::run_migrations(&conn).unwrap();
        conn.execute(
            "INSERT INTO billing_periods (building_id, month, year) VALUES (1, 9, 2026)",
            [],
        )
        .unwrap();
        let period_id = conn.last_insert_rowid();
        let context = load_bill_import_context(&conn, period_id).unwrap();
        let document = extract_document_from_file(&path.to_string_lossy()).unwrap();
        let prepared = prepare_multi_bill_import_from_document(
            document,
            "Racuni_09_2026_s.pdf".to_string(),
            context.month,
            context.year,
            &context.providers,
            true,
        );
        let previews = preview_prepared_bills(&prepared, &context.providers);
        assert_eq!(
            previews.len(),
            5,
            "native/OCR reconciliation created extra candidates"
        );
        for service in [
            "Electricity",
            "Gas/Heating",
            "VO-KA komunalne storitve",
            "Water/Sewage",
            "Cleaning",
        ] {
            let matches = previews.iter().any(|preview| {
                preview.identity.status == "matched"
                    && preview
                        .provider_id
                        .and_then(|provider_id| {
                            context
                                .providers
                                .iter()
                                .find(|provider| provider.id == Some(provider_id))
                        })
                        .map(|provider| provider.service_type == service)
                        .unwrap_or(false)
            });
            assert!(
                matches,
                "missing matched preview for {service}: {previews:#?}"
            );
        }
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
