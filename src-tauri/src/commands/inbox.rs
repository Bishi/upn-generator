use chrono::{Duration as ChronoDuration, Local};
use mailparse::{addrparse_header, DispositionType, MailAddr, MailHeaderMap, ParsedMail};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration as StdDuration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager, State};
use tempfile::{Builder as TempFileBuilder, TempDir};

use crate::credentials::{self, MailCredentialKind};

use super::bills::{
    bill_content_hash, load_bill_import_context, prepare_multi_bill_import_from_path,
    preview_prepared_bills, retain_expected_provider_bills, retain_new_bill_hashes,
    save_prepared_multi_bill_import, PreparedBillImport, PreparedBillPreviewSummary,
};
use super::config::DbState;

const MAX_DAYS_TO_SCAN: i32 = 90;
const MAX_FOLDER_LEN: usize = 128;
const MAX_SUBJECT_LEN: usize = 240;
const MAX_ERROR_LEN: usize = 500;
const MAX_ATTACHMENT_BYTES: usize = 20 * 1024 * 1024;
const MAX_MESSAGE_BYTES: u32 = 30 * 1024 * 1024;
const MAX_SCAN_SUMMARY_ITEMS: usize = 8;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InboxConfig {
    pub host: String,
    pub port: i32,
    pub username: String,
    pub use_tls: bool,
    pub folder: String,
    pub days_to_scan: i32,
    pub sender_allowlist: String,
    pub password_configured: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InboxImportResult {
    pub sender: String,
    pub subject: String,
    pub attachment_filename: String,
    pub status: String,
    pub bill_ids: Vec<i64>,
    pub bill_count: i32,
    pub skipped_reason: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InboxPreviewNotice {
    pub status: String,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InboxPreviewBillSummary {
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InboxPreviewCandidate {
    pub id: String,
    pub sender: String,
    pub subject: String,
    pub received_date: Option<String>,
    pub attachment_filename: String,
    pub attachment_sha256: String,
    pub status: String,
    pub selectable: bool,
    pub importable_count: i32,
    pub skipped_reason: Option<String>,
    pub error: Option<String>,
    pub bills: Vec<InboxPreviewBillSummary>,
    pub notices: Vec<InboxPreviewNotice>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InboxPreviewSession {
    pub session_id: String,
    pub billing_period_id: i64,
    pub days_to_scan: i32,
    pub username: String,
    pub folder: String,
    pub sender_allowlist: String,
    pub received_date_source: String,
    pub scan_summary: InboxPreviewScanSummary,
    pub candidates: Vec<InboxPreviewCandidate>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct InboxPreviewScanSummary {
    pub messages_matched: i32,
    pub messages_fetched: i32,
    pub messages_skipped_sender: i32,
    pub messages_skipped_oversize: i32,
    pub messages_without_supported_attachments: i32,
    pub supported_attachments_found: i32,
    pub unsupported_attachments_found: i32,
    pub unsupported_attachment_names: Vec<String>,
    pub senders_seen: Vec<String>,
}

#[derive(Debug, Clone)]
struct InboxCredentials {
    config: InboxConfig,
    password: String,
}

#[derive(Debug, Clone)]
struct MailAttachment {
    filename: String,
    mime_type: String,
    bytes: Vec<u8>,
}

#[derive(Debug, Default)]
struct AttachmentScan {
    attachments: Vec<MailAttachment>,
    unsupported_filenames: Vec<String>,
}

#[derive(Debug, Clone)]
struct MessageContext {
    folder: String,
    uid_validity: Option<u32>,
    message_uid: Option<u32>,
    message_id: String,
    sender: String,
    subject: String,
    received_date: Option<String>,
}

#[derive(Debug)]
struct InboxPreviewSessionData {
    billing_period_id: i64,
    days_to_scan: i32,
    username: String,
    folder: String,
    sender_allowlist: String,
    created_at: Instant,
    last_accessed: Instant,
    temp_dir: TempDir,
    candidates: HashMap<String, InboxPreviewCandidateData>,
}

#[derive(Debug, Clone)]
struct InboxPreviewCandidateData {
    message: MessageContext,
    attachment_filename: String,
    attachment_sha256: String,
    file_path: Option<PathBuf>,
    status: String,
}

pub struct InboxPreviewState {
    sessions: Mutex<HashMap<String, InboxPreviewSessionData>>,
}

impl Default for InboxPreviewState {
    fn default() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
        }
    }
}

static PREVIEW_COUNTER: AtomicU64 = AtomicU64::new(1);
const PREVIEW_SESSION_TTL: StdDuration = StdDuration::from_secs(30 * 60);

fn normalize_email(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

fn validate_folder(folder: &str) -> Result<(), String> {
    if folder.trim().is_empty() {
        return Err("Inbox folder is required.".to_string());
    }
    if folder.len() > MAX_FOLDER_LEN
        || folder
            .chars()
            .any(|ch| ch == '\0' || ch == '\r' || ch == '\n' || ch.is_control())
    {
        return Err("Inbox folder contains unsupported characters.".to_string());
    }
    Ok(())
}

fn validate_config(
    config: &InboxConfig,
    require_password: bool,
    password: &str,
) -> Result<(), String> {
    if config.host.trim().is_empty() {
        return Err("IMAP host is required.".to_string());
    }
    if config.username.trim().is_empty() {
        return Err("IMAP username is required.".to_string());
    }
    if require_password && password.is_empty() {
        return Err("IMAP password is required.".to_string());
    }
    if !(1..=65535).contains(&config.port) {
        return Err("IMAP port must be between 1 and 65535.".to_string());
    }
    if !(0..=MAX_DAYS_TO_SCAN).contains(&config.days_to_scan) {
        return Err(format!(
            "Days to scan must be between 0 and {}.",
            MAX_DAYS_TO_SCAN
        ));
    }
    validate_sender_allowlist(&config.sender_allowlist)?;
    validate_folder(&config.folder)?;
    Ok(())
}

fn validate_sender_allowlist(raw: &str) -> Result<(), String> {
    for item in raw
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
    {
        if !item.contains('@') {
            return Err(format!(
                "Sender allowlist contains an invalid email address: {}",
                item
            ));
        }
    }
    Ok(())
}

fn parse_allowlist(raw: &str) -> HashSet<String> {
    raw.split(',')
        .map(normalize_email)
        .filter(|item| item.contains('@'))
        .collect()
}

fn supported_extension(filename: &str) -> Option<String> {
    let ext = std::path::Path::new(filename)
        .extension()
        .and_then(|ext| ext.to_str())?
        .to_ascii_lowercase();
    match ext.as_str() {
        "pdf" | "jpg" | "jpeg" | "png" | "bmp" | "tif" | "tiff" => Some(ext),
        _ => None,
    }
}

fn format_billing_period(month: i32, year: i32) -> String {
    let month_name = match month {
        1 => "Jan",
        2 => "Feb",
        3 => "Mar",
        4 => "Apr",
        5 => "May",
        6 => "Jun",
        7 => "Jul",
        8 => "Aug",
        9 => "Sep",
        10 => "Oct",
        11 => "Nov",
        12 => "Dec",
        _ => return format!("{:02}.{}", month, year),
    };
    format!("{} {}", month_name, year)
}

fn billing_month_for_source_period(source_month: i32, source_year: i32) -> (i32, i32) {
    if source_month == 12 {
        (1, source_year + 1)
    } else {
        (source_month + 1, source_year)
    }
}

fn source_period_matches_billing_month(
    source_month: i32,
    source_year: i32,
    billing_month: i32,
    billing_year: i32,
) -> bool {
    let (expected_month, expected_year) =
        billing_month_for_source_period(source_month, source_year);
    expected_month == billing_month && expected_year == billing_year
}

fn mime_matches_extension(mime_type: &str, ext: &str) -> bool {
    let mime = mime_type.to_ascii_lowercase();
    if mime.trim().is_empty() {
        return false;
    }
    match ext {
        "pdf" => mime == "application/pdf",
        "jpg" | "jpeg" => mime == "image/jpeg",
        "png" => mime == "image/png",
        "bmp" => mime == "image/bmp" || mime == "image/x-ms-bmp",
        "tif" | "tiff" => mime == "image/tiff",
        _ => false,
    }
}

fn magic_matches_extension(bytes: &[u8], ext: &str) -> bool {
    match ext {
        "pdf" => bytes.starts_with(b"%PDF-"),
        "jpg" | "jpeg" => bytes.starts_with(&[0xFF, 0xD8, 0xFF]),
        "png" => bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]),
        "bmp" => bytes.starts_with(b"BM"),
        "tif" | "tiff" => {
            bytes.starts_with(&[b'I', b'I', 0x2A, 0x00])
                || bytes.starts_with(&[b'M', b'M', 0x00, 0x2A])
        }
        _ => false,
    }
}

fn sanitize_filename(filename: &str) -> String {
    let cleaned: String = filename
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-' | ' ') {
                ch
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = cleaned.trim_matches([' ', '.']).trim();
    let safe = if trimmed.is_empty() {
        "attachment"
    } else {
        trimmed
    };
    truncate_chars(safe, 120)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|b| format!("{:02x}", b)).collect()
}

fn preview_token(prefix: &str) -> String {
    let counter = PREVIEW_COUNTER.fetch_add(1, Ordering::Relaxed);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let seed = format!(
        "{}|{}|{}|{}|{:?}",
        prefix,
        now,
        std::process::id(),
        counter,
        std::thread::current().id()
    );
    let digest = Sha256::digest(seed.as_bytes());
    digest.iter().map(|b| format!("{:02x}", b)).collect()
}

fn write_inbox_preview_debug_log(lines: &[String]) {
    if let Some(path) =
        dirs_next::data_dir().map(|d| d.join("si.upn-generator").join("inbox_preview_debug.log"))
    {
        let _ = std::fs::create_dir_all(path.parent().unwrap_or_else(|| std::path::Path::new(".")));
        let _ = std::fs::write(path, lines.join("\n"));
    }
}

fn header_value(mail: &ParsedMail<'_>, name: &str) -> String {
    mail.headers.get_first_value(name).unwrap_or_default()
}

fn parsed_sender(mail: &ParsedMail<'_>) -> String {
    let Some(header) = mail.headers.get_first_header("From") else {
        return String::new();
    };
    let Ok(addresses) = addrparse_header(header) else {
        return String::new();
    };
    for address in addresses.iter() {
        match address {
            MailAddr::Single(info) => return normalize_email(&info.addr),
            MailAddr::Group(group) => {
                if let Some(first) = group.addrs.first() {
                    return normalize_email(&first.addr);
                }
            }
        }
    }
    String::new()
}

fn attachment_filename(part: &ParsedMail<'_>) -> Option<String> {
    let disposition = part.get_content_disposition();
    disposition
        .params
        .get("filename")
        .cloned()
        .or_else(|| part.ctype.params.get("name").cloned())
}

fn collect_attachment_scan(mail: &ParsedMail<'_>) -> AttachmentScan {
    let mut scan = AttachmentScan::default();
    for part in mail.parts() {
        if !part.subparts.is_empty() {
            continue;
        }
        let disposition = part.get_content_disposition();
        let filename = attachment_filename(part);
        let is_attachment = matches!(disposition.disposition, DispositionType::Attachment);
        if !is_attachment && filename.is_none() {
            continue;
        }
        let Some(filename) = filename else {
            continue;
        };
        if supported_extension(&filename).is_none() {
            scan.unsupported_filenames
                .push(sanitize_filename(&filename));
            continue;
        }
        if let Ok(bytes) = part.get_body_raw() {
            scan.attachments.push(MailAttachment {
                filename,
                mime_type: part.ctype.mimetype.clone(),
                bytes,
            });
        }
    }
    scan
}

fn collect_attachments(mail: &ParsedMail<'_>) -> Vec<MailAttachment> {
    collect_attachment_scan(mail).attachments
}

fn load_credentials(
    db: &State<DbState>,
    require_password: bool,
) -> Result<InboxCredentials, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut credentials_row = conn
        .query_row(
            "SELECT host, port, username, password, use_tls, folder, days_to_scan,
                sender_allowlist
         FROM inbox_config WHERE id=1",
            [],
            |row| {
                let password: String = row.get(3)?;
                Ok(InboxCredentials {
                    config: InboxConfig {
                        host: row.get(0)?,
                        port: row.get(1)?,
                        username: row.get(2)?,
                        use_tls: row.get::<_, i32>(4)? != 0,
                        folder: row.get(5)?,
                        days_to_scan: row.get(6)?,
                        sender_allowlist: row.get(7)?,
                        password_configured: false,
                    },
                    password,
                })
            },
        )
        .map_err(|e| e.to_string())?;
    credentials_row.config.password_configured = credentials::password_configured(
        MailCredentialKind::Imap,
        &credentials_row.config.username,
    )?;
    let resolved = credentials::resolve_password(
        MailCredentialKind::Imap,
        &credentials_row.config.username,
        "",
    );
    credentials_row.password = if require_password {
        resolved?
    } else {
        resolved.unwrap_or_default()
    };
    Ok(credentials_row)
}

fn connect_tls(
    config: &InboxConfig,
    password: &str,
) -> Result<imap::Session<native_tls::TlsStream<std::net::TcpStream>>, String> {
    validate_config(config, true, password)?;
    if !config.use_tls {
        return Err("Plain IMAP is disabled. Enable TLS for inbox imports.".to_string());
    }
    let tls = native_tls::TlsConnector::builder()
        .build()
        .map_err(|e| e.to_string())?;
    let client = imap::connect(
        (config.host.as_str(), config.port as u16),
        config.host.as_str(),
        &tls,
    )
    .map_err(|e| e.to_string())?;
    client
        .login(config.username.as_str(), password)
        .map_err(|e| e.0.to_string())
}

fn insert_import_record(
    conn: &rusqlite::Connection,
    billing_period_id: i64,
    message: &MessageContext,
    attachment_filename: &str,
    attachment_sha256: &str,
    bill_ids: &[i64],
    status: &str,
    error_text: &str,
) -> Result<i64, String> {
    let bill_ids_json = serde_json::to_string(bill_ids).map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO inbox_imports (
            billing_period_id, folder, uid_validity, message_uid, message_id,
            sender, subject, attachment_filename, attachment_sha256, bill_ids,
            bill_count, status, error_text
         ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
        params![
            billing_period_id,
            message.folder,
            message.uid_validity,
            message.message_uid,
            truncate_chars(&message.message_id, MAX_SUBJECT_LEN),
            message.sender,
            truncate_chars(&message.subject, MAX_SUBJECT_LEN),
            attachment_filename,
            attachment_sha256,
            bill_ids_json,
            bill_ids.len() as i32,
            status,
            truncate_chars(error_text, MAX_ERROR_LEN),
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

fn has_imported_hash(
    conn: &rusqlite::Connection,
    billing_period_id: i64,
    attachment_sha256: &str,
) -> Result<bool, String> {
    conn.query_row(
        "SELECT 1 FROM inbox_imports
         WHERE billing_period_id=?1 AND attachment_sha256=?2 AND status='imported'
         LIMIT 1",
        params![billing_period_id, attachment_sha256],
        |_| Ok(()),
    )
    .optional()
    .map_err(|e| e.to_string())
    .map(|row| row.is_some())
}

fn missing_provider_ids(
    conn: &rusqlite::Connection,
    billing_period_id: i64,
    context: &super::bills::BillImportContext,
) -> Result<HashSet<i64>, String> {
    let mut expected: HashSet<i64> = context
        .providers
        .iter()
        .filter_map(|provider| provider.id)
        .collect();
    let mut stmt = conn
        .prepare(
            "SELECT DISTINCT provider_id FROM bills
             WHERE billing_period_id=?1 AND provider_id IS NOT NULL",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![billing_period_id], |row| row.get::<_, i64>(0))
        .map_err(|e| e.to_string())?;
    for row in rows {
        expected.remove(&row.map_err(|e| e.to_string())?);
    }
    Ok(expected)
}

fn existing_bill_hashes(
    conn: &rusqlite::Connection,
    billing_period_id: i64,
) -> Result<HashSet<String>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT bill_hash FROM inbox_bill_hashes
             WHERE billing_period_id=?1",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![billing_period_id], |row| row.get::<_, String>(0))
        .map_err(|e| e.to_string())?;
    let mut hashes = HashSet::new();
    for row in rows {
        hashes.insert(row.map_err(|e| e.to_string())?);
    }
    let mut stmt = conn
        .prepare(
            "SELECT provider_id, creditor_iban, amount_cents, reference, due_date, invoice_number
             FROM bills
             WHERE billing_period_id=?1",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![billing_period_id], |row| {
            Ok(bill_content_hash(
                row.get::<_, Option<i64>>(0)?,
                &row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                &row.get::<_, String>(3)?,
                &row.get::<_, String>(4)?,
                &row.get::<_, String>(5)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    for row in rows {
        hashes.insert(row.map_err(|e| e.to_string())?);
    }
    Ok(hashes)
}

fn insert_bill_hashes(
    conn: &rusqlite::Connection,
    billing_period_id: i64,
    import_id: i64,
    bill_ids: &[i64],
    bill_hashes: &[String],
) -> Result<(), String> {
    for (bill_id, bill_hash) in bill_ids.iter().zip(bill_hashes.iter()) {
        conn.execute(
            "INSERT OR IGNORE INTO inbox_bill_hashes (
                billing_period_id, inbox_import_id, bill_id, bill_hash
             ) VALUES (?1,?2,?3,?4)",
            params![billing_period_id, import_id, bill_id, bill_hash],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn validate_attachment(attachment: &MailAttachment) -> Result<String, String> {
    if attachment.bytes.len() > MAX_ATTACHMENT_BYTES {
        return Err("Attachment is larger than 20 MB.".to_string());
    }
    let ext = supported_extension(&attachment.filename)
        .ok_or_else(|| "Unsupported attachment file type.".to_string())?;
    if !magic_matches_extension(&attachment.bytes, &ext) {
        return Err("Attachment content does not match its file type.".to_string());
    }
    if !mime_matches_extension(&attachment.mime_type, &ext) {
        return Err("Attachment MIME type does not match its file type.".to_string());
    }
    Ok(ext)
}

fn failure_result(message: &MessageContext, filename: &str, error: String) -> InboxImportResult {
    InboxImportResult {
        sender: message.sender.clone(),
        subject: message.subject.clone(),
        attachment_filename: filename.to_string(),
        status: "failed".to_string(),
        bill_ids: Vec::new(),
        bill_count: 0,
        skipped_reason: None,
        error: Some(error),
    }
}

fn skipped_result(
    message: &MessageContext,
    filename: &str,
    status: &str,
    reason: &str,
) -> InboxImportResult {
    InboxImportResult {
        sender: message.sender.clone(),
        subject: message.subject.clone(),
        attachment_filename: filename.to_string(),
        status: status.to_string(),
        bill_ids: Vec::new(),
        bill_count: 0,
        skipped_reason: Some(reason.to_string()),
        error: None,
    }
}

struct AttachmentAnalysis {
    status: String,
    reason: Option<String>,
    error: Option<String>,
    prepared: Option<PreparedBillImport>,
    kept_bill_hashes: Vec<String>,
    bill_summaries: Vec<InboxPreviewBillSummary>,
    notices: Vec<InboxPreviewNotice>,
}

fn bill_summary_from_prepared(summary: PreparedBillPreviewSummary) -> InboxPreviewBillSummary {
    InboxPreviewBillSummary {
        provider_id: summary.provider_id,
        provider_name: summary.provider_name,
        creditor_name: summary.creditor_name,
        amount_cents: summary.amount_cents,
        reference: summary.reference,
        due_date: summary.due_date,
        invoice_number: summary.invoice_number,
        purpose_text: summary.purpose_text,
        parse_note: summary.parse_note,
        status: summary.status,
    }
}

fn skipped_notice(status: &str, message: &str) -> InboxPreviewNotice {
    InboxPreviewNotice {
        status: status.to_string(),
        message: message.to_string(),
    }
}

fn analyze_staged_attachment(
    conn: &rusqlite::Connection,
    billing_period_id: i64,
    context: &super::bills::BillImportContext,
    message: &MessageContext,
    safe_filename: &str,
    attachment_sha256: &str,
    staged_path: &str,
) -> AttachmentAnalysis {
    match has_imported_hash(conn, billing_period_id, attachment_sha256) {
        Ok(true) => {
            let reason =
                "Attachment hash was already imported for this billing period.".to_string();
            return AttachmentAnalysis {
                status: "skipped_duplicate".to_string(),
                reason: Some(reason),
                error: None,
                prepared: None,
                kept_bill_hashes: Vec::new(),
                bill_summaries: Vec::new(),
                notices: Vec::new(),
            };
        }
        Ok(false) => {}
        Err(error) => {
            return AttachmentAnalysis {
                status: "failed".to_string(),
                reason: None,
                error: Some(error),
                prepared: None,
                kept_bill_hashes: Vec::new(),
                bill_summaries: Vec::new(),
                notices: Vec::new(),
            };
        }
    }

    let source_label = format!("email: {} / {}", message.sender, safe_filename);
    let mut prepared =
        match prepare_multi_bill_import_from_path(staged_path, source_label, context, false) {
            Ok(prepared) => prepared,
            Err(error) => {
                return AttachmentAnalysis {
                    status: "failed".to_string(),
                    reason: None,
                    error: Some(error),
                    prepared: None,
                    kept_bill_hashes: Vec::new(),
                    bill_summaries: Vec::new(),
                    notices: Vec::new(),
                };
            }
        };

    let period_mismatch = match prepared.detected_source_period() {
        Some((source_month, source_year))
            if source_period_matches_billing_month(
                source_month,
                source_year,
                context.month,
                context.year,
            ) =>
        {
            None
        }
        Some((source_month, source_year)) => Some((
            "skipped_wrong_period",
            format!(
                "Attachment appears to be for {}, which belongs in billing month {}. The selected billing month is {}.",
                format_billing_period(source_month, source_year),
                {
                    let (month, year) =
                        billing_month_for_source_period(source_month, source_year);
                    format_billing_period(month, year)
                },
                format_billing_period(context.month, context.year)
            ),
        )),
        None => Some((
            "skipped_unknown_period",
            format!(
                "Attachment source period could not be detected for selected billing month {}.",
                format_billing_period(context.month, context.year)
            ),
        )),
    };
    if let Some((status, reason)) = period_mismatch {
        return AttachmentAnalysis {
            status: status.to_string(),
            reason: Some(reason),
            error: None,
            prepared: None,
            kept_bill_hashes: Vec::new(),
            bill_summaries: Vec::new(),
            notices: Vec::new(),
        };
    }

    let missing_provider_ids = match missing_provider_ids(conn, billing_period_id, context) {
        Ok(ids) => ids,
        Err(error) => {
            return AttachmentAnalysis {
                status: "failed".to_string(),
                reason: None,
                error: Some(error),
                prepared: None,
                kept_bill_hashes: Vec::new(),
                bill_summaries: Vec::new(),
                notices: Vec::new(),
            };
        }
    };
    let mut notices = Vec::new();
    let filter_result =
        retain_expected_provider_bills(&mut prepared, &context.providers, &missing_provider_ids);
    if let (Some(status), Some(reason)) = (
        filter_result.skipped_status,
        filter_result.skipped_reason.as_deref(),
    ) {
        notices.push(skipped_notice(status, reason));
    }

    if prepared.has_extracted_bills() {
        let existing_hashes = match existing_bill_hashes(conn, billing_period_id) {
            Ok(hashes) => hashes,
            Err(error) => {
                return AttachmentAnalysis {
                    status: "failed".to_string(),
                    reason: None,
                    error: Some(error),
                    prepared: None,
                    kept_bill_hashes: Vec::new(),
                    bill_summaries: Vec::new(),
                    notices,
                };
            }
        };
        let hash_filter =
            retain_new_bill_hashes(&mut prepared, &context.providers, &existing_hashes);
        if hash_filter.skipped_duplicate_count > 0 {
            let reason = if hash_filter.skipped_duplicate_count == 1 {
                "Parsed bill content was already imported for this billing period.".to_string()
            } else {
                format!(
                    "{} parsed bills were already imported for this billing period.",
                    hash_filter.skipped_duplicate_count
                )
            };
            notices.push(skipped_notice("skipped_duplicate_bill", &reason));
        }

        if prepared.has_extracted_bills() {
            let bill_summaries = preview_prepared_bills(&prepared, &context.providers)
                .into_iter()
                .map(bill_summary_from_prepared)
                .collect();
            return AttachmentAnalysis {
                status: "ready".to_string(),
                reason: None,
                error: None,
                prepared: Some(prepared),
                kept_bill_hashes: hash_filter.kept_hashes,
                bill_summaries,
                notices,
            };
        }
    }

    if let Some(last_notice) = notices.last() {
        return AttachmentAnalysis {
            status: last_notice.status.clone(),
            reason: Some(last_notice.message.clone()),
            error: None,
            prepared: None,
            kept_bill_hashes: Vec::new(),
            bill_summaries: Vec::new(),
            notices,
        };
    }

    AttachmentAnalysis {
        status: "empty".to_string(),
        reason: Some("Inbox import did not find any bill to save.".to_string()),
        error: None,
        prepared: None,
        kept_bill_hashes: Vec::new(),
        bill_summaries: Vec::new(),
        notices,
    }
}

fn persist_analysis(
    conn: &rusqlite::Connection,
    billing_period_id: i64,
    context: &super::bills::BillImportContext,
    message: &MessageContext,
    safe_filename: &str,
    attachment_sha256: &str,
    analysis: AttachmentAnalysis,
) -> Vec<InboxImportResult> {
    if analysis.status != "ready" {
        let text = analysis
            .error
            .clone()
            .or_else(|| analysis.reason.clone())
            .unwrap_or_default();
        if let Err(error) = insert_import_record(
            conn,
            billing_period_id,
            message,
            safe_filename,
            attachment_sha256,
            &[],
            &analysis.status,
            &text,
        ) {
            eprintln!("Failed to write inbox import audit record: {error}");
        }
        if analysis.status == "failed" {
            return vec![failure_result(message, safe_filename, text)];
        }
        return vec![skipped_result(
            message,
            safe_filename,
            &analysis.status,
            &text,
        )];
    }

    let prepared = match analysis.prepared {
        Some(prepared) => prepared,
        None => {
            return vec![failure_result(
                message,
                safe_filename,
                "Ready inbox attachment had no prepared bill data.".to_string(),
            )];
        }
    };
    let saved = match save_prepared_multi_bill_import(
        conn,
        billing_period_id,
        prepared,
        &context.providers,
        false,
    ) {
        Ok(saved) => saved,
        Err(error) => {
            let _ = insert_import_record(
                conn,
                billing_period_id,
                message,
                safe_filename,
                attachment_sha256,
                &[],
                "failed",
                &error,
            );
            return vec![failure_result(message, safe_filename, error)];
        }
    };
    let bill_ids: Vec<i64> = saved.iter().filter_map(|bill| bill.id).collect();
    let import_id = match insert_import_record(
        conn,
        billing_period_id,
        message,
        safe_filename,
        attachment_sha256,
        &bill_ids,
        "imported",
        "",
    ) {
        Ok(id) => id,
        Err(error) => return vec![failure_result(message, safe_filename, error)],
    };
    if let Err(error) = insert_bill_hashes(
        conn,
        billing_period_id,
        import_id,
        &bill_ids,
        &analysis.kept_bill_hashes,
    ) {
        return vec![failure_result(message, safe_filename, error)];
    }

    let mut results = vec![InboxImportResult {
        sender: message.sender.clone(),
        subject: message.subject.clone(),
        attachment_filename: safe_filename.to_string(),
        status: "imported".to_string(),
        bill_ids,
        bill_count: saved.len() as i32,
        skipped_reason: None,
        error: None,
    }];
    for notice in analysis.notices {
        if let Err(error) = insert_import_record(
            conn,
            billing_period_id,
            message,
            safe_filename,
            attachment_sha256,
            &[],
            &notice.status,
            &notice.message,
        ) {
            return vec![failure_result(message, safe_filename, error)];
        }
        results.push(skipped_result(
            message,
            safe_filename,
            &notice.status,
            &notice.message,
        ));
    }
    results
}

fn import_staged_attachment(
    db: &State<DbState>,
    billing_period_id: i64,
    context: &super::bills::BillImportContext,
    message: &MessageContext,
    safe_filename: &str,
    attachment_sha256: &str,
    staged_path: &str,
) -> Vec<InboxImportResult> {
    let conn = match db.0.lock() {
        Ok(conn) => conn,
        Err(e) => return vec![failure_result(message, safe_filename, e.to_string())],
    };
    let tx = match conn.unchecked_transaction() {
        Ok(tx) => tx,
        Err(e) => return vec![failure_result(message, safe_filename, e.to_string())],
    };
    let analysis = analyze_staged_attachment(
        &tx,
        billing_period_id,
        context,
        message,
        safe_filename,
        attachment_sha256,
        staged_path,
    );
    let was_ready = analysis.status == "ready";
    let results = persist_analysis(
        &tx,
        billing_period_id,
        context,
        message,
        safe_filename,
        attachment_sha256,
        analysis,
    );
    if was_ready && results.iter().any(|result| result.status == "failed") {
        let error = results
            .iter()
            .find(|result| result.status == "failed")
            .and_then(|result| result.error.clone())
            .unwrap_or_else(|| "Inbox import failed.".to_string());
        let _ = tx.rollback();
        drop(conn);
        if let Ok(conn) = db.0.lock() {
            let _ = insert_import_record(
                &conn,
                billing_period_id,
                message,
                safe_filename,
                attachment_sha256,
                &[],
                "failed",
                &error,
            );
        }
        return results;
    }
    if let Err(error) = tx.commit() {
        return vec![failure_result(message, safe_filename, error.to_string())];
    }
    results
}

fn import_attachment(
    db: &State<DbState>,
    billing_period_id: i64,
    context: &super::bills::BillImportContext,
    message: &MessageContext,
    attachment: MailAttachment,
) -> Vec<InboxImportResult> {
    let safe_filename = sanitize_filename(&attachment.filename);
    let ext = match validate_attachment(&attachment) {
        Ok(ext) => ext,
        Err(error) => {
            let hash = sha256_hex(&attachment.bytes);
            if let Ok(conn) = db.0.lock() {
                let _ = insert_import_record(
                    &conn,
                    billing_period_id,
                    message,
                    &safe_filename,
                    &hash,
                    &[],
                    "failed",
                    &error,
                );
            }
            return vec![failure_result(message, &safe_filename, error)];
        }
    };
    let hash = sha256_hex(&attachment.bytes);
    let mut temp_file = match TempFileBuilder::new()
        .prefix("upn-mail-")
        .suffix(&format!(".{}", ext))
        .tempfile()
    {
        Ok(file) => file,
        Err(e) => return vec![failure_result(message, &safe_filename, e.to_string())],
    };
    if let Err(e) = temp_file.write_all(&attachment.bytes) {
        return vec![failure_result(message, &safe_filename, e.to_string())];
    }
    let temp_path = temp_file.path().to_string_lossy().to_string();
    import_staged_attachment(
        db,
        billing_period_id,
        context,
        message,
        &safe_filename,
        &hash,
        &temp_path,
    )
}

fn sweep_preview_sessions(state: &State<InboxPreviewState>) -> Result<(), String> {
    let now = Instant::now();
    let mut sessions = state.sessions.lock().map_err(|e| e.to_string())?;
    sessions.retain(|_, session| {
        now.duration_since(session.last_accessed) <= PREVIEW_SESSION_TTL
            && now.duration_since(session.created_at) <= PREVIEW_SESSION_TTL * 2
    });
    Ok(())
}

fn preview_candidate_from_analysis(
    id: String,
    message: &MessageContext,
    safe_filename: &str,
    attachment_sha256: &str,
    analysis: AttachmentAnalysis,
) -> InboxPreviewCandidate {
    let selectable = analysis.status == "ready" && !analysis.bill_summaries.is_empty();
    InboxPreviewCandidate {
        id,
        sender: message.sender.clone(),
        subject: message.subject.clone(),
        received_date: message.received_date.clone(),
        attachment_filename: safe_filename.to_string(),
        attachment_sha256: attachment_sha256.to_string(),
        status: analysis.status,
        selectable,
        importable_count: analysis.bill_summaries.len() as i32,
        skipped_reason: analysis.reason,
        error: analysis.error,
        bills: analysis.bill_summaries,
        notices: analysis.notices,
    }
}

fn preview_failure_candidate(
    id: String,
    message: &MessageContext,
    safe_filename: &str,
    attachment_sha256: &str,
    error: String,
) -> InboxPreviewCandidate {
    InboxPreviewCandidate {
        id,
        sender: message.sender.clone(),
        subject: message.subject.clone(),
        received_date: message.received_date.clone(),
        attachment_filename: safe_filename.to_string(),
        attachment_sha256: attachment_sha256.to_string(),
        status: "failed".to_string(),
        selectable: false,
        importable_count: 0,
        skipped_reason: None,
        error: Some(error),
        bills: Vec::new(),
        notices: Vec::new(),
    }
}

fn session_response(
    session_id: String,
    data: &InboxPreviewSessionData,
    scan_summary: InboxPreviewScanSummary,
    candidates: Vec<InboxPreviewCandidate>,
) -> InboxPreviewSession {
    InboxPreviewSession {
        session_id,
        billing_period_id: data.billing_period_id,
        days_to_scan: data.days_to_scan,
        username: data.username.clone(),
        folder: data.folder.clone(),
        sender_allowlist: data.sender_allowlist.clone(),
        received_date_source: "imap_internal_date".to_string(),
        scan_summary,
        candidates,
    }
}

#[tauri::command]
pub fn get_inbox_config(db: State<DbState>) -> Result<InboxConfig, String> {
    Ok(load_credentials(&db, false)?.config)
}

#[tauri::command]
pub fn save_inbox_config(db: State<DbState>, config: InboxConfig) -> Result<(), String> {
    validate_config(&config, false, "")?;
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE inbox_config
         SET host=?1, port=?2, username=?3, use_tls=?4, folder=?5,
             days_to_scan=?6, sender_allowlist=?7
         WHERE id=1",
        params![
            config.host.trim(),
            config.port,
            config.username.trim(),
            if config.use_tls { 1 } else { 0 },
            config.folder.trim(),
            config.days_to_scan,
            config.sender_allowlist.trim(),
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn save_inbox_password(db: State<DbState>, password: String) -> Result<(), String> {
    if password.is_empty() {
        return Ok(());
    }
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let username = conn
        .query_row("SELECT username FROM inbox_config WHERE id=1", [], |row| {
            row.get::<_, String>(0)
        })
        .map_err(|e| e.to_string())?;
    credentials::save_password(&conn, MailCredentialKind::Imap, &username, &password)
}

#[tauri::command]
pub async fn test_inbox_connection(config: InboxConfig, password: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || test_inbox_connection_impl(config, password))
        .await
        .map_err(|e| format!("Inbox test task failed: {e}"))?
}

fn test_inbox_connection_impl(config: InboxConfig, password: String) -> Result<(), String> {
    let stored_password =
        credentials::resolve_password(MailCredentialKind::Imap, &config.username, &password)?;
    let mut session = connect_tls(&config, &stored_password)?;
    session
        .examine(config.folder.as_str())
        .map_err(|e| e.to_string())?;
    session.logout().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn preview_inbox_attachments(
    app: AppHandle,
    billing_period_id: i64,
    days_to_scan: i32,
) -> Result<InboxPreviewSession, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = app.state::<DbState>();
        let preview_state = app.state::<InboxPreviewState>();
        preview_inbox_attachments_impl(db, preview_state, billing_period_id, days_to_scan)
    })
    .await
    .map_err(|e| format!("Inbox preview task failed: {e}"))?
}

fn preview_inbox_attachments_impl(
    db: State<'_, DbState>,
    preview_state: State<'_, InboxPreviewState>,
    billing_period_id: i64,
    days_to_scan: i32,
) -> Result<InboxPreviewSession, String> {
    let overall_start = Instant::now();
    let mut debug_log = vec![
        format!(
            "=== preview_inbox_attachments {} ===",
            Local::now().to_rfc3339()
        ),
        format!("billing_period_id={billing_period_id}"),
        format!("days_to_scan={days_to_scan}"),
    ];

    sweep_preview_sessions(&preview_state)?;
    if !(0..=MAX_DAYS_TO_SCAN).contains(&days_to_scan) {
        return Err(format!(
            "Days to scan must be between 0 and {}.",
            MAX_DAYS_TO_SCAN
        ));
    }

    let mut credentials = load_credentials(&db, true)?;
    credentials.config.days_to_scan = days_to_scan;
    validate_config(&credentials.config, true, &credentials.password)?;
    let context = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        load_bill_import_context(&conn, billing_period_id)?
    };
    let allowlist = parse_allowlist(&credentials.config.sender_allowlist);
    let since = (Local::now().date_naive() - ChronoDuration::days(days_to_scan as i64))
        .format("%d-%b-%Y")
        .to_string();
    debug_log.push(format!("folder={}", credentials.config.folder));
    debug_log.push(format!("allowlist_count={}", allowlist.len()));
    debug_log.push(format!("since={since}"));

    let session_id = preview_token("session");
    let temp_dir = tempfile::Builder::new()
        .prefix("upn-inbox-preview-")
        .tempdir()
        .map_err(|e| e.to_string())?;
    let mut session_data = InboxPreviewSessionData {
        billing_period_id,
        days_to_scan,
        username: credentials.config.username.clone(),
        folder: credentials.config.folder.clone(),
        sender_allowlist: credentials.config.sender_allowlist.clone(),
        created_at: Instant::now(),
        last_accessed: Instant::now(),
        temp_dir,
        candidates: HashMap::new(),
    };
    let mut preview_candidates = Vec::new();
    let mut scan_summary = InboxPreviewScanSummary::default();

    let connect_start = Instant::now();
    let mut imap_session = match connect_tls(&credentials.config, &credentials.password) {
        Ok(session) => {
            debug_log.push(format!(
                "connect_ms={}",
                connect_start.elapsed().as_millis()
            ));
            session
        }
        Err(error) => {
            debug_log.push(format!("connect_error={error}"));
            debug_log.push(format!("total_ms={}", overall_start.elapsed().as_millis()));
            write_inbox_preview_debug_log(&debug_log);
            return Err(error);
        }
    };
    let scan_result = (|| -> Result<(), String> {
        let examine_start = Instant::now();
        let mailbox = imap_session
            .examine(credentials.config.folder.as_str())
            .map_err(|e| e.to_string())?;
        debug_log.push(format!(
            "examine_ms={}",
            examine_start.elapsed().as_millis()
        ));
        let uid_validity = mailbox.uid_validity;
        let search_start = Instant::now();
        let ids = imap_session
            .search(format!("SINCE {}", since))
            .map_err(|e| e.to_string())?;
        let mut ids: Vec<u32> = ids.into_iter().collect();
        ids.sort_unstable();
        scan_summary.messages_matched = ids.len() as i32;
        debug_log.push(format!(
            "search_ms={} matched={}",
            search_start.elapsed().as_millis(),
            ids.len()
        ));

        for id in ids {
            let message_start = Instant::now();
            let id_str = id.to_string();
            let metadata_start = Instant::now();
            let metadata = imap_session
                .fetch(&id_str, "(UID RFC822.SIZE INTERNALDATE)")
                .map_err(|e| e.to_string())?;
            let metadata_ms = metadata_start.elapsed().as_millis();
            let Some(meta) = metadata.iter().next() else {
                debug_log.push(format!(
                    "message_id={id} metadata_ms={metadata_ms} no_metadata=true"
                ));
                continue;
            };
            let message_uid = meta.uid;
            let received_date = meta.internal_date().map(|date| date.to_rfc3339());
            let message_size = meta.size.unwrap_or(0);
            if message_size > MAX_MESSAGE_BYTES {
                scan_summary.messages_skipped_oversize += 1;
                debug_log.push(format!(
                    "message_id={id} uid={:?} metadata_ms={metadata_ms} size_bytes={} skipped=oversize",
                    message_uid, message_size
                ));
                let message = MessageContext {
                    folder: credentials.config.folder.clone(),
                    uid_validity,
                    message_uid,
                    message_id: String::new(),
                    sender: String::new(),
                    subject: "Message skipped before fetch".to_string(),
                    received_date,
                };
                let candidate_id = preview_token("candidate");
                preview_candidates.push(preview_failure_candidate(
                    candidate_id,
                    &message,
                    "(message)",
                    "",
                    "Message is larger than 30 MB.".to_string(),
                ));
                continue;
            }

            let fetch_start = Instant::now();
            let messages = imap_session
                .fetch(&id_str, "(UID BODY.PEEK[])")
                .map_err(|e| e.to_string())?;
            let fetch_ms = fetch_start.elapsed().as_millis();
            let Some(message) = messages.iter().next() else {
                debug_log.push(format!(
                    "message_id={id} uid={:?} metadata_ms={metadata_ms} fetch_ms={fetch_ms} no_message=true",
                    message_uid
                ));
                continue;
            };
            let Some(body) = message.body() else {
                debug_log.push(format!(
                    "message_id={id} uid={:?} metadata_ms={metadata_ms} fetch_ms={fetch_ms} no_body=true",
                    message.uid.or(message_uid)
                ));
                continue;
            };
            scan_summary.messages_fetched += 1;
            let parse_start = Instant::now();
            let parsed = mailparse::parse_mail(body).map_err(|e| e.to_string())?;
            let parse_ms = parse_start.elapsed().as_millis();
            let sender = parsed_sender(&parsed);
            if !sender.is_empty()
                && !scan_summary.senders_seen.contains(&sender)
                && scan_summary.senders_seen.len() < MAX_SCAN_SUMMARY_ITEMS
            {
                scan_summary.senders_seen.push(sender.clone());
            }
            if !allowlist.is_empty() && !allowlist.contains(&sender) {
                scan_summary.messages_skipped_sender += 1;
                debug_log.push(format!(
                    "message_id={id} uid={:?} metadata_ms={metadata_ms} fetch_ms={fetch_ms} parse_ms={parse_ms} size_bytes={} skipped=sender",
                    message.uid.or(message_uid),
                    message_size
                ));
                continue;
            }
            let message_context = MessageContext {
                folder: credentials.config.folder.clone(),
                uid_validity,
                message_uid: message.uid.or(message_uid),
                message_id: header_value(&parsed, "Message-ID"),
                sender,
                subject: truncate_chars(&header_value(&parsed, "Subject"), MAX_SUBJECT_LEN),
                received_date,
            };
            let attachment_scan_start = Instant::now();
            let attachment_scan = collect_attachment_scan(&parsed);
            let attachment_scan_ms = attachment_scan_start.elapsed().as_millis();
            let supported_count = attachment_scan.attachments.len();
            let unsupported_count = attachment_scan.unsupported_filenames.len();
            scan_summary.supported_attachments_found += attachment_scan.attachments.len() as i32;
            scan_summary.unsupported_attachments_found +=
                attachment_scan.unsupported_filenames.len() as i32;
            for filename in attachment_scan.unsupported_filenames {
                if !scan_summary
                    .unsupported_attachment_names
                    .contains(&filename)
                    && scan_summary.unsupported_attachment_names.len() < MAX_SCAN_SUMMARY_ITEMS
                {
                    scan_summary.unsupported_attachment_names.push(filename);
                }
            }
            if attachment_scan.attachments.is_empty() {
                scan_summary.messages_without_supported_attachments += 1;
            }
            debug_log.push(format!(
                "message_id={id} uid={:?} metadata_ms={metadata_ms} fetch_ms={fetch_ms} parse_ms={parse_ms} attachment_scan_ms={attachment_scan_ms} size_bytes={} supported_attachments={} unsupported_attachments={}",
                message_context.message_uid,
                message_size,
                supported_count,
                unsupported_count
            ));
            for attachment in attachment_scan.attachments {
                let safe_filename = sanitize_filename(&attachment.filename);
                let attachment_sha256 = sha256_hex(&attachment.bytes);
                let candidate_id = preview_token("candidate");
                let ext = match validate_attachment(&attachment) {
                    Ok(ext) => ext,
                    Err(error) => {
                        preview_candidates.push(preview_failure_candidate(
                            candidate_id.clone(),
                            &message_context,
                            &safe_filename,
                            &attachment_sha256,
                            error,
                        ));
                        session_data.candidates.insert(
                            candidate_id,
                            InboxPreviewCandidateData {
                                message: message_context.clone(),
                                attachment_filename: safe_filename,
                                attachment_sha256,
                                file_path: None,
                                status: "failed".to_string(),
                            },
                        );
                        continue;
                    }
                };
                let staged_name = format!("{}-{}.{}", preview_candidates.len(), candidate_id, ext);
                let staged_path = session_data.temp_dir.path().join(staged_name);
                let stage_start = Instant::now();
                if let Err(error) = std::fs::write(&staged_path, &attachment.bytes) {
                    debug_log.push(format!(
                        "attachment={} size_bytes={} stage_ms={} status=failed error={}",
                        safe_filename,
                        attachment.bytes.len(),
                        stage_start.elapsed().as_millis(),
                        error
                    ));
                    preview_candidates.push(preview_failure_candidate(
                        candidate_id.clone(),
                        &message_context,
                        &safe_filename,
                        &attachment_sha256,
                        error.to_string(),
                    ));
                    session_data.candidates.insert(
                        candidate_id,
                        InboxPreviewCandidateData {
                            message: message_context.clone(),
                            attachment_filename: safe_filename,
                            attachment_sha256,
                            file_path: None,
                            status: "failed".to_string(),
                        },
                    );
                    continue;
                }
                let stage_ms = stage_start.elapsed().as_millis();
                let analysis_start = Instant::now();
                let analysis = {
                    let conn = db.0.lock().map_err(|e| e.to_string())?;
                    analyze_staged_attachment(
                        &conn,
                        billing_period_id,
                        &context,
                        &message_context,
                        &safe_filename,
                        &attachment_sha256,
                        &staged_path.to_string_lossy(),
                    )
                };
                let analysis_ms = analysis_start.elapsed().as_millis();
                let candidate = preview_candidate_from_analysis(
                    candidate_id.clone(),
                    &message_context,
                    &safe_filename,
                    &attachment_sha256,
                    analysis,
                );
                session_data.candidates.insert(
                    candidate_id,
                    InboxPreviewCandidateData {
                        message: message_context.clone(),
                        attachment_filename: safe_filename.clone(),
                        attachment_sha256,
                        file_path: Some(staged_path),
                        status: candidate.status.clone(),
                    },
                );
                debug_log.push(format!(
                    "attachment={} size_bytes={} stage_ms={stage_ms} analysis_ms={analysis_ms} status={} importable_count={} notices={}",
                    safe_filename,
                    attachment.bytes.len(),
                    candidate.status,
                    candidate.importable_count,
                    candidate.notices.len()
                ));
                preview_candidates.push(candidate);
            }
            debug_log.push(format!(
                "message_id={id} elapsed_ms={}",
                message_start.elapsed().as_millis()
            ));
        }
        Ok(())
    })();
    let logout_result = imap_session.logout().map_err(|e| e.to_string());
    if let Err(error) = scan_result {
        let _ = logout_result;
        debug_log.push(format!("scan_error={error}"));
        debug_log.push(format!("total_ms={}", overall_start.elapsed().as_millis()));
        write_inbox_preview_debug_log(&debug_log);
        return Err(error);
    }
    logout_result?;

    let response = session_response(
        session_id.clone(),
        &session_data,
        scan_summary,
        preview_candidates,
    );
    preview_state
        .sessions
        .lock()
        .map_err(|e| e.to_string())?
        .insert(session_id, session_data);
    debug_log.push(format!(
        "summary matched={} fetched={} skipped_sender={} skipped_oversize={} without_supported={} supported_attachments={} unsupported_attachments={} candidates={}",
        response.scan_summary.messages_matched,
        response.scan_summary.messages_fetched,
        response.scan_summary.messages_skipped_sender,
        response.scan_summary.messages_skipped_oversize,
        response.scan_summary.messages_without_supported_attachments,
        response.scan_summary.supported_attachments_found,
        response.scan_summary.unsupported_attachments_found,
        response.candidates.len()
    ));
    debug_log.push(format!("total_ms={}", overall_start.elapsed().as_millis()));
    write_inbox_preview_debug_log(&debug_log);
    Ok(response)
}

#[tauri::command]
pub fn import_inbox_preview_selection(
    db: State<DbState>,
    preview_state: State<InboxPreviewState>,
    session_id: String,
    candidate_ids: Vec<String>,
) -> Result<Vec<InboxImportResult>, String> {
    sweep_preview_sessions(&preview_state)?;
    if candidate_ids.is_empty() {
        return Ok(Vec::new());
    }

    let (billing_period_id, selected_candidates) = {
        let mut sessions = preview_state.sessions.lock().map_err(|e| e.to_string())?;
        let session = sessions.get_mut(&session_id).ok_or_else(|| {
            "Inbox preview session expired. Fetch inbox preview again.".to_string()
        })?;
        session.last_accessed = Instant::now();
        let selected = candidate_ids
            .iter()
            .filter_map(|id| {
                session
                    .candidates
                    .get(id)
                    .cloned()
                    .map(|candidate| (id.clone(), candidate))
            })
            .collect::<Vec<_>>();
        (session.billing_period_id, selected)
    };

    let context = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        load_bill_import_context(&conn, billing_period_id)?
    };
    let mut results = Vec::new();
    for (_, candidate) in &selected_candidates {
        if candidate.status != "ready" {
            results.push(skipped_result(
                &candidate.message,
                &candidate.attachment_filename,
                &candidate.status,
                "Preview candidate is not importable.",
            ));
            continue;
        }
        let Some(path) = &candidate.file_path else {
            results.push(failure_result(
                &candidate.message,
                &candidate.attachment_filename,
                "Preview candidate attachment is no longer available.".to_string(),
            ));
            continue;
        };
        results.extend(import_staged_attachment(
            &db,
            billing_period_id,
            &context,
            &candidate.message,
            &candidate.attachment_filename,
            &candidate.attachment_sha256,
            &path.to_string_lossy(),
        ));
    }

    let mut sessions = preview_state.sessions.lock().map_err(|e| e.to_string())?;
    if let Some(session) = sessions.get_mut(&session_id) {
        for (id, candidate) in selected_candidates {
            if let Some(path) = candidate.file_path {
                let _ = std::fs::remove_file(path);
            }
            session.candidates.remove(&id);
        }
        let has_importable = session
            .candidates
            .values()
            .any(|candidate| candidate.status == "ready");
        if !has_importable {
            sessions.remove(&session_id);
        }
    }
    Ok(results)
}

#[tauri::command]
pub fn clear_inbox_preview_session(
    preview_state: State<InboxPreviewState>,
    session_id: String,
) -> Result<(), String> {
    sweep_preview_sessions(&preview_state)?;
    preview_state
        .sessions
        .lock()
        .map_err(|e| e.to_string())?
        .remove(&session_id);
    Ok(())
}

#[tauri::command]
pub async fn import_inbox_attachments(
    app: AppHandle,
    billing_period_id: i64,
) -> Result<Vec<InboxImportResult>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = app.state::<DbState>();
        import_inbox_attachments_impl(db, billing_period_id)
    })
    .await
    .map_err(|e| format!("Inbox import task failed: {e}"))?
}

fn import_inbox_attachments_impl(
    db: State<'_, DbState>,
    billing_period_id: i64,
) -> Result<Vec<InboxImportResult>, String> {
    let credentials = load_credentials(&db, true)?;
    validate_config(&credentials.config, true, &credentials.password)?;
    let context = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        load_bill_import_context(&conn, billing_period_id)?
    };
    let allowlist = parse_allowlist(&credentials.config.sender_allowlist);
    let since = (Local::now().date_naive()
        - ChronoDuration::days(credentials.config.days_to_scan as i64))
    .format("%d-%b-%Y")
    .to_string();

    let mut session = connect_tls(&credentials.config, &credentials.password)?;
    let mut results = Vec::new();
    let scan_result = (|| -> Result<(), String> {
        let mailbox = session
            .examine(credentials.config.folder.as_str())
            .map_err(|e| e.to_string())?;
        let uid_validity = mailbox.uid_validity;
        let ids = session
            .search(format!("SINCE {}", since))
            .map_err(|e| e.to_string())?;
        let mut ids: Vec<u32> = ids.into_iter().collect();
        ids.sort_unstable();

        for id in ids {
            let id_str = id.to_string();
            let metadata = session
                .fetch(&id_str, "(UID RFC822.SIZE INTERNALDATE)")
                .map_err(|e| e.to_string())?;
            let Some(meta) = metadata.iter().next() else {
                continue;
            };
            let message_uid = meta.uid;
            let received_date = meta.internal_date().map(|date| date.to_rfc3339());
            if meta.size.unwrap_or(0) > MAX_MESSAGE_BYTES {
                let message = MessageContext {
                    folder: credentials.config.folder.clone(),
                    uid_validity,
                    message_uid,
                    message_id: String::new(),
                    sender: String::new(),
                    subject: "Message skipped before fetch".to_string(),
                    received_date,
                };
                results.push(failure_result(
                    &message,
                    "(message)",
                    "Message is larger than 30 MB.".to_string(),
                ));
                continue;
            }

            let messages = session
                .fetch(&id_str, "(UID BODY.PEEK[])")
                .map_err(|e| e.to_string())?;
            let Some(message) = messages.iter().next() else {
                continue;
            };
            let Some(body) = message.body() else {
                continue;
            };
            let parsed = mailparse::parse_mail(body).map_err(|e| e.to_string())?;
            let sender = parsed_sender(&parsed);
            if !allowlist.is_empty() && !allowlist.contains(&sender) {
                continue;
            }
            let message_context = MessageContext {
                folder: credentials.config.folder.clone(),
                uid_validity,
                message_uid: message.uid.or(message_uid),
                message_id: header_value(&parsed, "Message-ID"),
                sender,
                subject: truncate_chars(&header_value(&parsed, "Subject"), MAX_SUBJECT_LEN),
                received_date,
            };
            for attachment in collect_attachments(&parsed) {
                results.extend(import_attachment(
                    &db,
                    billing_period_id,
                    &context,
                    &message_context,
                    attachment,
                ));
            }
        }
        Ok(())
    })();
    let logout_result = session.logout().map_err(|e| e.to_string());
    if let Err(error) = scan_result {
        let _ = logout_result;
        return Err(error);
    }
    logout_result?;
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_config_bounds() {
        let config = InboxConfig {
            host: "imap.example.com".to_string(),
            port: 993,
            username: "user".to_string(),
            use_tls: true,
            folder: "INBOX".to_string(),
            days_to_scan: 45,
            sender_allowlist: String::new(),
            password_configured: false,
        };
        assert!(validate_config(&config, true, "secret").is_ok());

        let mut today_only = config.clone();
        today_only.days_to_scan = 0;
        assert!(validate_config(&today_only, true, "secret").is_ok());

        let mut bad = config.clone();
        bad.days_to_scan = -1;
        assert!(validate_config(&bad, true, "secret").is_err());

        let mut bad = config;
        bad.folder = "INBOX\r\nBAD".to_string();
        assert!(validate_config(&bad, true, "secret").is_err());
    }

    #[test]
    fn validates_magic_bytes() {
        assert!(magic_matches_extension(b"%PDF-1.7", "pdf"));
        assert!(magic_matches_extension(&[0xFF, 0xD8, 0xFF, 0xAA], "jpg"));
        assert!(!magic_matches_extension(b"not a pdf", "pdf"));
    }

    #[test]
    fn rejects_empty_attachment_mime_type() {
        assert!(mime_matches_extension("application/pdf", "pdf"));
        assert!(!mime_matches_extension("", "pdf"));
        assert!(!mime_matches_extension("text/plain", "pdf"));
    }

    #[test]
    fn parses_sender_allowlist() {
        let allowlist = parse_allowlist(" A@Example.com, invalid, b@example.com ");
        assert!(allowlist.contains("a@example.com"));
        assert!(allowlist.contains("b@example.com"));
        assert!(!allowlist.contains("invalid"));
    }

    #[test]
    fn rejects_malformed_sender_allowlist_entries() {
        assert!(validate_sender_allowlist("").is_ok());
        assert!(validate_sender_allowlist("sender@example.com").is_ok());
        assert!(validate_sender_allowlist("not-an-email").is_err());
        assert!(validate_sender_allowlist("sender@example.com, bad-entry").is_err());
    }

    #[test]
    fn source_period_matches_following_billing_month() {
        assert!(source_period_matches_billing_month(1, 2026, 2, 2026));
        assert!(source_period_matches_billing_month(12, 2025, 1, 2026));
        assert!(!source_period_matches_billing_month(1, 2026, 1, 2026));
        assert!(!source_period_matches_billing_month(1, 2026, 3, 2026));
    }

    fn test_message() -> MessageContext {
        MessageContext {
            folder: "INBOX".to_string(),
            uid_validity: Some(1),
            message_uid: Some(2),
            message_id: "message@example.com".to_string(),
            sender: "sender@example.com".to_string(),
            subject: "Bill".to_string(),
            received_date: None,
        }
    }

    #[test]
    fn preview_candidate_keeps_ready_candidate_selectable_with_notices() {
        let message = test_message();
        let analysis = AttachmentAnalysis {
            status: "ready".to_string(),
            reason: None,
            error: None,
            prepared: None,
            kept_bill_hashes: vec!["hash".to_string()],
            bill_summaries: vec![InboxPreviewBillSummary {
                provider_id: Some(7),
                provider_name: Some("Elektro".to_string()),
                creditor_name: "Elektro".to_string(),
                amount_cents: 1234,
                reference: "SI12 123".to_string(),
                due_date: "01.04.2026".to_string(),
                invoice_number: String::new(),
                purpose_text: String::new(),
                parse_note: String::new(),
                status: "draft".to_string(),
            }],
            notices: vec![skipped_notice(
                "skipped_unknown_provider",
                "Some parsed bills were ignored.",
            )],
        };

        let candidate = preview_candidate_from_analysis(
            "candidate".to_string(),
            &message,
            "bill.pdf",
            "abc",
            analysis,
        );

        assert!(candidate.selectable);
        assert_eq!(candidate.importable_count, 1);
        assert_eq!(candidate.notices.len(), 1);
    }

    #[test]
    fn duplicate_preview_analysis_writes_no_inbox_import_rows() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE inbox_imports (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                billing_period_id INTEGER NOT NULL,
                attachment_sha256 TEXT NOT NULL,
                status TEXT NOT NULL
            );
            INSERT INTO inbox_imports (billing_period_id, attachment_sha256, status)
            VALUES (10, 'abc', 'imported');
            ",
        )
        .unwrap();
        let context = super::super::bills::BillImportContext {
            month: 4,
            year: 2026,
            providers: Vec::new(),
        };
        let message = test_message();

        let analysis = analyze_staged_attachment(
            &conn,
            10,
            &context,
            &message,
            "bill.pdf",
            "abc",
            "unused.pdf",
        );

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM inbox_imports", [], |row| row.get(0))
            .unwrap();
        assert_eq!(analysis.status, "skipped_duplicate");
        assert_eq!(count, 1);
    }
}
