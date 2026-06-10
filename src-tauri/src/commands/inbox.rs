use chrono::{Duration as ChronoDuration, Local};
use mailparse::{addrparse_header, DispositionType, MailAddr, MailHeaderMap, ParsedMail};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::io::Write;
use tauri::State;
use tempfile::Builder as TempFileBuilder;

use super::bills::{
    bill_content_hash, load_bill_import_context, prepare_multi_bill_import_from_path,
    reset_import_debug_log, retain_expected_provider_bills, retain_new_bill_hashes,
    save_prepared_multi_bill_import,
};
use super::config::DbState;

const MAX_DAYS_TO_SCAN: i32 = 90;
const MAX_FOLDER_LEN: usize = 128;
const MAX_SUBJECT_LEN: usize = 240;
const MAX_ERROR_LEN: usize = 500;
const MAX_ATTACHMENT_BYTES: usize = 20 * 1024 * 1024;
const MAX_MESSAGE_BYTES: u32 = 30 * 1024 * 1024;

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

#[derive(Debug, Clone)]
struct MessageContext {
    folder: String,
    uid_validity: Option<u32>,
    message_uid: Option<u32>,
    message_id: String,
    sender: String,
    subject: String,
}

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
    if !(1..=MAX_DAYS_TO_SCAN).contains(&config.days_to_scan) {
        return Err(format!(
            "Days to scan must be between 1 and {}.",
            MAX_DAYS_TO_SCAN
        ));
    }
    validate_folder(&config.folder)?;
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

fn mime_matches_extension(mime_type: &str, ext: &str) -> bool {
    let mime = mime_type.to_ascii_lowercase();
    if mime.trim().is_empty() {
        return false;
    }
    match ext {
        "pdf" => matches!(
            mime.as_str(),
            "application/pdf" | "application/x-pdf" | "application/octet-stream"
        ),
        "jpg" | "jpeg" => mime == "image/jpeg",
        "png" => mime == "image/png",
        "bmp" => mime == "image/bmp" || mime == "image/x-ms-bmp",
        "tif" | "tiff" => mime == "image/tiff",
        _ => false,
    }
}

fn is_generic_pdf_mime(mime_type: &str, ext: &str) -> bool {
    ext == "pdf"
        && mime_type
            .trim()
            .eq_ignore_ascii_case("application/octet-stream")
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

fn pdf_has_eof_marker(bytes: &[u8]) -> bool {
    let tail_start = bytes.len().saturating_sub(1024);
    bytes[tail_start..]
        .windows(b"%%EOF".len())
        .any(|window| window == b"%%EOF")
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

fn collect_attachments(mail: &ParsedMail<'_>) -> Vec<MailAttachment> {
    let mut attachments = Vec::new();
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
            continue;
        }
        if let Ok(bytes) = part.get_body_raw() {
            attachments.push(MailAttachment {
                filename,
                mime_type: part.ctype.mimetype.clone(),
                bytes,
            });
        }
    }
    attachments
}

fn load_credentials(db: &State<DbState>) -> Result<InboxCredentials, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.query_row(
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
                    password_configured: !password.is_empty(),
                },
                password,
            })
        },
    )
    .map_err(|e| e.to_string())
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
    if is_generic_pdf_mime(&attachment.mime_type, &ext) && !pdf_has_eof_marker(&attachment.bytes) {
        return Err("Generic PDF attachment is missing a PDF EOF marker.".to_string());
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

    {
        let conn = match db.0.lock() {
            Ok(conn) => conn,
            Err(e) => return vec![failure_result(message, &safe_filename, e.to_string())],
        };
        match has_imported_hash(&conn, billing_period_id, &hash) {
            Ok(true) => {
                let _ = insert_import_record(
                    &conn,
                    billing_period_id,
                    message,
                    &safe_filename,
                    &hash,
                    &[],
                    "skipped_duplicate",
                    "Attachment hash was already imported for this billing period.",
                );
                return vec![skipped_result(
                    message,
                    &safe_filename,
                    "skipped_duplicate",
                    "Attachment hash was already imported for this billing period.",
                )];
            }
            Ok(false) => {}
            Err(error) => return vec![failure_result(message, &safe_filename, error)],
        }
    }

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
    let source_label = format!("email: {} / {}", message.sender, safe_filename);
    let mut prepared =
        match prepare_multi_bill_import_from_path(&temp_path, source_label, context, false) {
            Ok(prepared) => prepared,
            Err(error) => {
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
    let period_mismatch = match prepared.detected_source_period() {
        Some((source_month, source_year))
            if source_month == context.month && source_year == context.year =>
        {
            None
        }
        Some((source_month, source_year)) => Some((
            "skipped_wrong_period",
            format!(
                "Attachment appears to be for {:02}.{}, but the selected billing period is {:02}.{}.",
                source_month, source_year, context.month, context.year
            ),
        )),
        None => Some((
            "skipped_unknown_period",
            format!(
                "Attachment billing period could not be detected for selected period {:02}.{}.",
                context.month, context.year
            ),
        )),
    };
    if let Some((status, reason)) = period_mismatch {
        if let Ok(conn) = db.0.lock() {
            let _ = insert_import_record(
                &conn,
                billing_period_id,
                message,
                &safe_filename,
                &hash,
                &[],
                status,
                &reason,
            );
        }
        return vec![skipped_result(message, &safe_filename, status, &reason)];
    }

    let conn = match db.0.lock() {
        Ok(conn) => conn,
        Err(e) => return vec![failure_result(message, &safe_filename, e.to_string())],
    };
    let tx = match conn.unchecked_transaction() {
        Ok(tx) => tx,
        Err(e) => return vec![failure_result(message, &safe_filename, e.to_string())],
    };
    let missing_provider_ids = match missing_provider_ids(&tx, billing_period_id, context) {
        Ok(ids) => ids,
        Err(error) => {
            let _ = tx.rollback();
            return vec![failure_result(message, &safe_filename, error)];
        }
    };
    let filter_result =
        retain_expected_provider_bills(&mut prepared, &context.providers, &missing_provider_ids);
    let mut skipped_results = Vec::new();
    if let (Some(status), Some(reason)) = (
        filter_result.skipped_status,
        filter_result.skipped_reason.as_deref(),
    ) {
        if let Err(error) = insert_import_record(
            &tx,
            billing_period_id,
            message,
            &safe_filename,
            &hash,
            &[],
            status,
            reason,
        ) {
            let _ = tx.rollback();
            return vec![failure_result(message, &safe_filename, error)];
        }
        skipped_results.push(skipped_result(message, &safe_filename, status, reason));
    }
    if prepared.has_extracted_bills() {
        let existing_hashes = match existing_bill_hashes(&tx, billing_period_id) {
            Ok(hashes) => hashes,
            Err(error) => {
                let _ = tx.rollback();
                return vec![failure_result(message, &safe_filename, error)];
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
            if let Err(error) = insert_import_record(
                &tx,
                billing_period_id,
                message,
                &safe_filename,
                &hash,
                &[],
                "skipped_duplicate_bill",
                &reason,
            ) {
                let _ = tx.rollback();
                return vec![failure_result(message, &safe_filename, error)];
            }
            skipped_results.push(skipped_result(
                message,
                &safe_filename,
                "skipped_duplicate_bill",
                &reason,
            ));
        }
        let kept_bill_hashes = hash_filter.kept_hashes;
        if !prepared.has_extracted_bills() {
            if let Err(error) = tx.commit() {
                return vec![failure_result(message, &safe_filename, error.to_string())];
            }
            return skipped_results;
        }
        let saved = match save_prepared_multi_bill_import(
            &tx,
            billing_period_id,
            prepared,
            &context.providers,
            false,
            true,
        ) {
            Ok(saved) => saved,
            Err(error) => {
                let _ = tx.rollback();
                drop(conn);
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
        let bill_ids: Vec<i64> = saved.iter().filter_map(|bill| bill.id).collect();
        let import_id = match insert_import_record(
            &tx,
            billing_period_id,
            message,
            &safe_filename,
            &hash,
            &bill_ids,
            "imported",
            "",
        ) {
            Ok(id) => id,
            Err(error) => {
                let _ = tx.rollback();
                return vec![failure_result(message, &safe_filename, error)];
            }
        };
        if let Err(error) = insert_bill_hashes(
            &tx,
            billing_period_id,
            import_id,
            &bill_ids,
            &kept_bill_hashes,
        ) {
            let _ = tx.rollback();
            return vec![failure_result(message, &safe_filename, error)];
        }
        if let Err(error) = tx.commit() {
            return vec![failure_result(message, &safe_filename, error.to_string())];
        }

        let mut results = vec![InboxImportResult {
            sender: message.sender.clone(),
            subject: message.subject.clone(),
            attachment_filename: safe_filename,
            status: "imported".to_string(),
            bill_ids,
            bill_count: saved.len() as i32,
            skipped_reason: None,
            error: None,
        }];
        results.extend(skipped_results);
        return results;
    }
    if !prepared.has_extracted_bills() {
        if let Err(error) = tx.commit() {
            return vec![failure_result(message, &safe_filename, error.to_string())];
        }
        return skipped_results;
    }
    vec![failure_result(
        message,
        &safe_filename,
        "Inbox import did not find any bill to save.".to_string(),
    )]
}

#[tauri::command]
pub fn get_inbox_config(db: State<DbState>) -> Result<InboxConfig, String> {
    Ok(load_credentials(&db)?.config)
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
    conn.execute(
        "UPDATE inbox_config SET password=?1 WHERE id=1",
        params![password],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn test_inbox_connection(
    db: State<DbState>,
    config: InboxConfig,
    password: String,
) -> Result<(), String> {
    let stored_password = if password.is_empty() {
        load_credentials(&db)?.password
    } else {
        password
    };
    let mut session = connect_tls(&config, &stored_password)?;
    session
        .examine(config.folder.as_str())
        .map_err(|e| e.to_string())?;
    session.logout().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn import_inbox_attachments(
    db: State<DbState>,
    billing_period_id: i64,
) -> Result<Vec<InboxImportResult>, String> {
    let credentials = load_credentials(&db)?;
    validate_config(&credentials.config, true, &credentials.password)?;
    let context = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        load_bill_import_context(&conn, billing_period_id)?
    };
    reset_import_debug_log();
    let allowlist = parse_allowlist(&credentials.config.sender_allowlist);
    let since = (Local::now().date_naive()
        - ChronoDuration::days(credentials.config.days_to_scan as i64))
    .format("%d-%b-%Y")
    .to_string();

    let mut session = connect_tls(&credentials.config, &credentials.password)?;
    let mailbox = session
        .examine(credentials.config.folder.as_str())
        .map_err(|e| e.to_string())?;
    let uid_validity = mailbox.uid_validity;
    let ids = session
        .uid_search(format!("SINCE {}", since))
        .map_err(|e| e.to_string())?;
    let mut ids: Vec<u32> = ids.into_iter().collect();
    ids.sort_unstable();

    let mut results = Vec::new();
    for id in ids {
        let id_str = id.to_string();
        let metadata = session
            .uid_fetch(&id_str, "UID RFC822.SIZE")
            .map_err(|e| e.to_string())?;
        let Some(meta) = metadata.iter().next() else {
            continue;
        };
        let message_uid = meta.uid;
        if meta.size.unwrap_or(0) > MAX_MESSAGE_BYTES {
            let message = MessageContext {
                folder: credentials.config.folder.clone(),
                uid_validity,
                message_uid,
                message_id: String::new(),
                sender: String::new(),
                subject: "Message skipped before fetch".to_string(),
            };
            results.push(failure_result(
                &message,
                "(message)",
                "Message is larger than 30 MB.".to_string(),
            ));
            continue;
        }

        let messages = session
            .uid_fetch(&id_str, "UID BODY.PEEK[]")
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
    let _ = session.logout();
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

        let mut bad = config.clone();
        bad.days_to_scan = 0;
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
    fn validates_generic_pdf_eof_marker() {
        assert!(pdf_has_eof_marker(b"%PDF-1.7\nbody\n%%EOF\n"));
        assert!(pdf_has_eof_marker(
            &[vec![b'a'; 1019], b"%%EOF".to_vec()].concat()
        ));
        assert!(!pdf_has_eof_marker(b"%PDF-1.7\nbody"));
        assert!(!pdf_has_eof_marker(
            &[b"%%EOF".to_vec(), vec![b'a'; 1025]].concat()
        ));
    }

    #[test]
    fn rejects_empty_attachment_mime_type() {
        assert!(mime_matches_extension("application/pdf", "pdf"));
        assert!(mime_matches_extension("application/x-pdf", "pdf"));
        assert!(mime_matches_extension("application/octet-stream", "pdf"));
        assert!(!mime_matches_extension("", "pdf"));
        assert!(!mime_matches_extension("text/plain", "pdf"));
    }

    #[test]
    fn validates_generic_pdf_fallback_content() {
        let valid = MailAttachment {
            filename: "bill.pdf".to_string(),
            mime_type: "application/octet-stream".to_string(),
            bytes: b"%PDF-1.7\nbody\n%%EOF\n".to_vec(),
        };
        assert_eq!(validate_attachment(&valid).unwrap(), "pdf");

        let missing_eof = MailAttachment {
            bytes: b"%PDF-1.7\nbody".to_vec(),
            ..valid
        };
        assert_eq!(
            validate_attachment(&missing_eof).unwrap_err(),
            "Generic PDF attachment is missing a PDF EOF marker."
        );
    }

    #[test]
    fn parses_sender_allowlist() {
        let allowlist = parse_allowlist(" A@Example.com, invalid, b@example.com ");
        assert!(allowlist.contains("a@example.com"));
        assert!(allowlist.contains("b@example.com"));
        assert!(!allowlist.contains("invalid"));
    }
}
