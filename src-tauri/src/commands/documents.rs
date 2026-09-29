use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{ipc::Response, State};

use super::config::DbState;

const LOCAL_PREVIEW_IDLE_TTL: Duration = Duration::from_secs(30 * 60);
const LOCAL_PREVIEW_HARD_TTL: Duration = Duration::from_secs(60 * 60);
static LOCAL_PREVIEW_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone)]
struct LocalPreviewSource {
    canonical_path: PathBuf,
    sha256: String,
    original_name: String,
    media_type: String,
    created_at: Instant,
    last_accessed: Instant,
}

#[derive(Default)]
pub struct LocalPreviewState {
    sources: Mutex<HashMap<String, LocalPreviewSource>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SourceDocumentInfo {
    pub original_name: String,
    pub media_type: String,
    pub byte_size: i64,
    pub page_start: Option<i32>,
    pub page_end: Option<i32>,
}

#[derive(Debug, Clone)]
pub(crate) struct ResolvedSource {
    pub path: PathBuf,
    pub bytes: Vec<u8>,
    pub sha256: String,
    pub original_name: String,
    pub media_type: String,
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub(crate) fn media_type_for_path(path: &Path) -> Result<String, String> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match extension.as_str() {
        "pdf" => Ok("application/pdf".to_string()),
        "jpg" | "jpeg" => Ok("image/jpeg".to_string()),
        "png" => Ok("image/png".to_string()),
        "bmp" => Ok("image/bmp".to_string()),
        "tif" | "tiff" => Ok("image/tiff".to_string()),
        _ => Err("Unsupported source document type.".to_string()),
    }
}

fn sweep_local_sources(sources: &mut HashMap<String, LocalPreviewSource>) {
    let now = Instant::now();
    sources.retain(|_, source| {
        now.duration_since(source.last_accessed) <= LOCAL_PREVIEW_IDLE_TTL
            && now.duration_since(source.created_at) <= LOCAL_PREVIEW_HARD_TTL
    });
}

pub(crate) fn register_local_source(
    state: &LocalPreviewState,
    file_path: &str,
    expected_sha256: &str,
    original_name: String,
) -> Result<String, String> {
    let canonical_path = std::fs::canonicalize(file_path)
        .map_err(|error| format!("Could not resolve the selected source file: {error}"))?;
    let media_type = media_type_for_path(&canonical_path)?;
    let bytes = std::fs::read(&canonical_path).map_err(|error| error.to_string())?;
    if sha256_hex(&bytes) != expected_sha256 {
        return Err("The source file changed while its preview handle was created.".to_string());
    }
    let counter = LOCAL_PREVIEW_COUNTER.fetch_add(1, Ordering::Relaxed);
    let issued_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let handle_seed = format!(
        "{}:{}:{}:{}",
        expected_sha256,
        canonical_path.display(),
        counter,
        issued_at
    );
    let handle = sha256_hex(handle_seed.as_bytes());
    let now = Instant::now();
    let mut sources = state.sources.lock().map_err(|error| error.to_string())?;
    sweep_local_sources(&mut sources);
    sources.insert(
        handle.clone(),
        LocalPreviewSource {
            canonical_path,
            sha256: expected_sha256.to_string(),
            original_name,
            media_type,
            created_at: now,
            last_accessed: now,
        },
    );
    Ok(handle)
}

pub(crate) fn resolve_local_source(
    state: &LocalPreviewState,
    source_handle: &str,
) -> Result<ResolvedSource, String> {
    let source = {
        let mut sources = state.sources.lock().map_err(|error| error.to_string())?;
        sweep_local_sources(&mut sources);
        let source = sources.get_mut(source_handle).ok_or_else(|| {
            "The local source preview expired. Select the file again.".to_string()
        })?;
        source.last_accessed = Instant::now();
        source.clone()
    };
    let bytes = match std::fs::read(&source.canonical_path) {
        Ok(bytes) => bytes,
        Err(error) => {
            clear_local_source_inner(state, source_handle);
            return Err(format!(
                "The selected source file is no longer available: {error}"
            ));
        }
    };
    if sha256_hex(&bytes) != source.sha256 {
        clear_local_source_inner(state, source_handle);
        return Err("The selected source file changed after preview. Select it again.".to_string());
    }
    Ok(ResolvedSource {
        path: source.canonical_path,
        bytes,
        sha256: source.sha256,
        original_name: source.original_name,
        media_type: source.media_type,
    })
}

fn clear_local_source_inner(state: &LocalPreviewState, source_handle: &str) {
    if let Ok(mut sources) = state.sources.lock() {
        sources.remove(source_handle);
    }
}

pub(crate) fn clear_local_source_handle(state: &LocalPreviewState, source_handle: &str) {
    clear_local_source_inner(state, source_handle);
}

#[tauri::command]
pub fn clear_local_preview_source(
    state: State<LocalPreviewState>,
    source_handle: String,
) -> Result<(), String> {
    let mut sources = state.sources.lock().map_err(|error| error.to_string())?;
    sweep_local_sources(&mut sources);
    sources.remove(&source_handle);
    Ok(())
}

#[tauri::command]
pub fn get_local_preview_source_info(
    state: State<LocalPreviewState>,
    source_handle: String,
) -> Result<SourceDocumentInfo, String> {
    let source = resolve_local_source(&state, &source_handle)?;
    Ok(SourceDocumentInfo {
        original_name: source.original_name,
        media_type: source.media_type,
        byte_size: source.bytes.len() as i64,
        page_start: None,
        page_end: None,
    })
}

#[tauri::command]
pub fn read_local_preview_source(
    state: State<LocalPreviewState>,
    source_handle: String,
) -> Result<Response, String> {
    Ok(Response::new(
        resolve_local_source(&state, &source_handle)?.bytes,
    ))
}

fn stored_document_info(
    conn: &Connection,
    bill_id: i64,
) -> Result<Option<SourceDocumentInfo>, String> {
    conn.query_row(
        "SELECT d.original_name, d.media_type, d.byte_size,
                b.source_page_start, b.source_page_end
         FROM bills b
         JOIN source_documents d ON d.id=b.source_document_id
         WHERE b.id=?1",
        [bill_id],
        |row| {
            Ok(SourceDocumentInfo {
                original_name: row.get(0)?,
                media_type: row.get(1)?,
                byte_size: row.get(2)?,
                page_start: row.get(3)?,
                page_end: row.get(4)?,
            })
        },
    )
    .optional()
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn get_bill_source_document_info(
    db: State<DbState>,
    bill_id: i64,
) -> Result<Option<SourceDocumentInfo>, String> {
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    stored_document_info(&conn, bill_id)
}

#[tauri::command]
pub fn read_bill_source_document(db: State<DbState>, bill_id: i64) -> Result<Response, String> {
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    let (bytes, expected_sha256, expected_size) = conn
        .query_row(
            "SELECT d.content, d.content_sha256, d.byte_size
             FROM bills b
             JOIN source_documents d ON d.id=b.source_document_id
             WHERE b.id=?1",
            [bill_id],
            |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )
        .optional()
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "Original document is unavailable for this bill.".to_string())?;
    if bytes.len() as i64 != expected_size || sha256_hex(&bytes) != expected_sha256 {
        return Err("The retained original document is corrupt or incomplete.".to_string());
    }
    Ok(Response::new(bytes))
}

pub(crate) fn persist_source_document(
    conn: &Connection,
    original_name: &str,
    media_type: &str,
    bytes: &[u8],
    content_sha256: &str,
) -> Result<i64, String> {
    conn.execute(
        "INSERT OR IGNORE INTO source_documents
         (original_name, media_type, byte_size, content_sha256, content)
         VALUES (?1,?2,?3,?4,?5)",
        params![
            original_name,
            media_type,
            bytes.len() as i64,
            content_sha256,
            bytes
        ],
    )
    .map_err(|error| error.to_string())?;
    conn.query_row(
        "SELECT id FROM source_documents WHERE content_sha256=?1",
        [content_sha256],
        |row| row.get(0),
    )
    .map_err(|error| error.to_string())
}

pub(crate) fn link_bills_to_document(
    conn: &Connection,
    bill_ids: &[i64],
    source_document_id: i64,
) -> Result<(), String> {
    for bill_id in bill_ids {
        conn.execute(
            "UPDATE bills SET source_document_id=?1 WHERE id=?2",
            params![source_document_id, bill_id],
        )
        .map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub(crate) fn delete_orphan_source_documents(conn: &Connection) -> Result<(), String> {
    conn.execute(
        "DELETE FROM source_documents
         WHERE NOT EXISTS (
             SELECT 1 FROM bills WHERE bills.source_document_id=source_documents.id
         )",
        [],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations;
    use std::io::Write;

    #[test]
    fn exact_source_bytes_are_deduplicated_and_shared_until_last_link_is_deleted() {
        let conn = Connection::open_in_memory().unwrap();
        migrations::run_migrations(&conn).unwrap();
        let period_id: i64 = conn
            .query_row("SELECT id FROM billing_periods LIMIT 1", [], |row| {
                row.get(0)
            })
            .unwrap_or_else(|_| {
                conn.execute(
                    "INSERT INTO billing_periods (building_id, month, year) VALUES (1, 1, 2026)",
                    [],
                )
                .unwrap();
                conn.last_insert_rowid()
            });
        for id in [1001_i64, 1002_i64] {
            conn.execute(
                "INSERT INTO bills (id, billing_period_id, source_filename) VALUES (?1,?2,'combined.pdf')",
                params![id, period_id],
            )
            .unwrap();
        }
        let bytes = b"%PDF-1.7 exact original";
        let hash = sha256_hex(bytes);
        let first = persist_source_document(&conn, "combined.pdf", "application/pdf", bytes, &hash)
            .unwrap();
        let second =
            persist_source_document(&conn, "renamed.pdf", "application/pdf", bytes, &hash).unwrap();
        assert_eq!(first, second);
        link_bills_to_document(&conn, &[1001, 1002], first).unwrap();

        conn.execute("DELETE FROM bills WHERE id=1001", []).unwrap();
        delete_orphan_source_documents(&conn).unwrap();
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM source_documents", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            1
        );

        conn.execute("DELETE FROM bills WHERE id=1002", []).unwrap();
        delete_orphan_source_documents(&conn).unwrap();
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM source_documents", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }

    #[test]
    fn local_preview_handle_rejects_changed_source_and_clear_invalidates_it() {
        let mut file = tempfile::Builder::new().suffix(".pdf").tempfile().unwrap();
        file.write_all(b"%PDF-1.7 original").unwrap();
        file.flush().unwrap();
        let bytes = std::fs::read(file.path()).unwrap();
        let state = LocalPreviewState::default();
        let handle = register_local_source(
            &state,
            &file.path().to_string_lossy(),
            &sha256_hex(&bytes),
            "invoice.pdf".to_string(),
        )
        .unwrap();
        assert_eq!(resolve_local_source(&state, &handle).unwrap().bytes, bytes);

        std::fs::write(file.path(), b"%PDF-1.7 changed").unwrap();
        assert!(resolve_local_source(&state, &handle)
            .unwrap_err()
            .contains("changed"));
        assert!(resolve_local_source(&state, &handle).is_err());

        let bytes = std::fs::read(file.path()).unwrap();
        let replacement = register_local_source(
            &state,
            &file.path().to_string_lossy(),
            &sha256_hex(&bytes),
            "invoice.pdf".to_string(),
        )
        .unwrap();
        clear_local_source_handle(&state, &replacement);
        assert!(resolve_local_source(&state, &replacement).is_err());
    }
}
