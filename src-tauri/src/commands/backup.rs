use rusqlite::{Connection, DatabaseName, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tauri::State;

#[cfg(target_os = "windows")]
use std::os::windows::ffi::OsStrExt;
#[cfg(target_os = "windows")]
use windows::{
    core::PCWSTR,
    Win32::Storage::FileSystem::{MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH},
};

use super::config::DbState;

const REQUIRED_TABLES: &[&str] = &[
    "building",
    "apartments",
    "providers",
    "billing_periods",
    "bills",
    "bill_splits",
    "smtp_config",
];

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BackupFileInfo {
    pub path: String,
}

fn ensure_required_tables(conn: &Connection) -> Result<(), String> {
    for table in REQUIRED_TABLES {
        let exists = conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1",
                [table],
                |_| Ok(()),
            )
            .optional()
            .map_err(|e| e.to_string())?
            .is_some();

        if !exists {
            return Err(format!(
                "Backup file is missing required table '{}'.",
                table
            ));
        }
    }

    Ok(())
}

fn backup_output_path(output_path: &str) -> Result<&Path, String> {
    let path = Path::new(output_path);
    let parent = path
        .parent()
        .ok_or_else(|| "Backup path must include a parent folder.".to_string())?;

    if !parent.exists() {
        return Err("Selected backup folder does not exist.".to_string());
    }

    Ok(path)
}

#[cfg(target_os = "windows")]
fn replace_backup_file(source: &Path, destination: &Path) -> Result<(), String> {
    let source_wide = source
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let destination_wide = destination
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    unsafe {
        MoveFileExW(
            PCWSTR(source_wide.as_ptr()),
            PCWSTR(destination_wide.as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    }
    .map_err(|error| format!("Could not replace the selected backup file: {error}"))
}

#[cfg(not(target_os = "windows"))]
fn replace_backup_file(source: &Path, destination: &Path) -> Result<(), String> {
    if destination.exists() {
        return Err("Replacing an existing backup is only supported on Windows.".to_string());
    }
    std::fs::rename(source, destination).map_err(|error| error.to_string())
}

fn table_exists(conn: &Connection, table: &str) -> Result<bool, String> {
    conn.query_row(
        "SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1",
        [table],
        |_| Ok(()),
    )
    .optional()
    .map_err(|e| e.to_string())
    .map(|row| row.is_some())
}

fn attached_table_exists(conn: &Connection, table: &str) -> Result<bool, String> {
    conn.query_row(
        "SELECT 1 FROM restore_db.sqlite_master WHERE type='table' AND name=?1",
        [table],
        |_| Ok(()),
    )
    .optional()
    .map_err(|e| e.to_string())
    .map(|row| row.is_some())
}

fn attached_column_exists(conn: &Connection, table: &str, column: &str) -> Result<bool, String> {
    let mut stmt = conn
        .prepare(&format!("PRAGMA restore_db.table_info({})", table))
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| e.to_string())?;

    for row in rows {
        if row.map_err(|e| e.to_string())? == column {
            return Ok(true);
        }
    }
    Ok(false)
}

#[tauri::command]
pub fn create_db_backup(db: State<DbState>, output_path: String) -> Result<BackupFileInfo, String> {
    create_db_backup_inner(&db, output_path)
}

fn create_db_backup_inner(db: &DbState, output_path: String) -> Result<BackupFileInfo, String> {
    let backup_path = backup_output_path(&output_path)?;
    let parent = backup_path
        .parent()
        .ok_or_else(|| "Backup path must include a parent folder.".to_string())?;
    let temporary = tempfile::Builder::new()
        .prefix(".upn-backup-")
        .suffix(".sqlite3")
        .tempfile_in(parent)
        .map_err(|error| format!("Could not create a temporary backup file: {error}"))?;
    let temporary_path = temporary.into_temp_path();
    let create_result = (|| -> Result<(), String> {
        {
            let conn = db.0.lock().map_err(|e| e.to_string())?;
            conn.backup(DatabaseName::Main, &temporary_path, None)
                .map_err(|e| e.to_string())?;
        }

        let backup_conn = Connection::open(&temporary_path).map_err(|e| e.to_string())?;
        ensure_required_tables(&backup_conn)?;
        backup_conn
            .execute("UPDATE smtp_config SET password='' WHERE id=1", [])
            .map_err(|e| e.to_string())?;
        if table_exists(&backup_conn, "inbox_config")? {
            backup_conn
                .execute("UPDATE inbox_config SET password='' WHERE id=1", [])
                .map_err(|e| e.to_string())?;
        }
        backup_conn
            .execute_batch("VACUUM")
            .map_err(|e| e.to_string())?;
        drop(backup_conn);
        replace_backup_file(&temporary_path, backup_path)?;
        Ok(())
    })();
    if let Err(error) = create_result {
        return Err(error);
    }

    Ok(BackupFileInfo { path: output_path })
}

#[tauri::command]
pub fn restore_db_backup(db: State<DbState>, input_path: String) -> Result<(), String> {
    restore_db_backup_inner(&db, input_path)
}

fn restore_db_backup_inner(db: &DbState, input_path: String) -> Result<(), String> {
    let restore_path = Path::new(&input_path);
    if !restore_path.exists() {
        return Err("Selected backup file does not exist.".to_string());
    }

    let source =
        Connection::open(restore_path).map_err(|e| format!("Could not open backup file: {}", e))?;
    ensure_required_tables(&source)?;
    drop(source);

    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute("DETACH DATABASE restore_db", []).ok();
    conn.execute("ATTACH DATABASE ?1 AS restore_db", [&input_path])
        .map_err(|e| format!("Could not attach backup file: {}", e))?;

    let restore_result = (|| -> Result<(), String> {
        ensure_required_tables(&conn)?;
        ensure_required_tables_on_attached(&conn)?;
        let has_inbox_config = attached_table_exists(&conn, "inbox_config")?;
        let has_inbox_imports = attached_table_exists(&conn, "inbox_imports")?;
        let has_inbox_bill_hashes = attached_table_exists(&conn, "inbox_bill_hashes")?;
        let has_app_settings = attached_table_exists(&conn, "app_settings")?;
        let has_upn_delivery_events = attached_table_exists(&conn, "upn_delivery_events")?;
        let has_bills_reviewed_at = attached_column_exists(&conn, "bills", "reviewed_at")?;
        let has_bills_review_note = attached_column_exists(&conn, "bills", "review_note")?;
        let has_provider_identity_rules =
            attached_column_exists(&conn, "providers", "identity_rule_type")?;
        let has_bill_identity_status = attached_column_exists(&conn, "bills", "identity_status")?;
        let has_source_documents = attached_table_exists(&conn, "source_documents")?;
        let has_bill_source_document_id =
            attached_column_exists(&conn, "bills", "source_document_id")?;
        let has_billing_periods_closed_at =
            attached_column_exists(&conn, "billing_periods", "closed_at")?;
        let has_smtp_allowlist_enabled =
            attached_column_exists(&conn, "smtp_config", "allowlist_enabled")?;
        let has_smtp_recipient_allowlist =
            attached_column_exists(&conn, "smtp_config", "recipient_allowlist")?;

        let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;

        tx.execute_batch(
            "
            DELETE FROM bill_splits;
            DELETE FROM inbox_bill_hashes;
            DELETE FROM upn_delivery_events;
            DELETE FROM inbox_imports;
            DELETE FROM bills;
            DELETE FROM source_documents;
            DELETE FROM billing_periods;
            DELETE FROM apartments;
            DELETE FROM providers;
            DELETE FROM building;
            DELETE FROM smtp_config;
            DELETE FROM inbox_config;
            DELETE FROM app_settings;
            ",
        )
        .map_err(|e| e.to_string())?;

        tx.execute_batch(
            "
            INSERT INTO building (id, name, address, city, postal_code)
            SELECT id, name, address, city, postal_code FROM restore_db.building;

            INSERT INTO apartments (
                id, building_id, label, unit_code, occupant_count, contact_email,
                payer_name, payer_address, payer_city, payer_postal_code,
                m2_percentage, is_active
            )
            SELECT
                id, building_id, label, unit_code, occupant_count, contact_email,
                payer_name, payer_address, payer_city, payer_postal_code,
                m2_percentage, is_active
            FROM restore_db.apartments;

            INSERT INTO providers (
                id, name, service_type, creditor_name, creditor_address, creditor_city,
                creditor_postal_code, creditor_iban, purpose_code, match_pattern,
                amount_pattern, reference_pattern, due_date_pattern,
                invoice_number_pattern, purpose_text_template, split_basis
            )
            SELECT
                id, name, service_type, creditor_name, creditor_address, creditor_city,
                creditor_postal_code, creditor_iban, purpose_code, match_pattern,
                amount_pattern, reference_pattern, due_date_pattern,
                invoice_number_pattern, purpose_text_template, split_basis
            FROM restore_db.providers;

            INSERT INTO billing_periods (id, building_id, month, year, status, created_at)
            SELECT id, building_id, month, year, status, created_at
            FROM restore_db.billing_periods;

            ",
        )
        .map_err(|e| e.to_string())?;

        if has_source_documents {
            tx.execute_batch(
                "INSERT INTO source_documents (
                    id, original_name, media_type, byte_size, content_sha256, content, created_at
                 )
                 SELECT id, original_name, media_type, byte_size, content_sha256, content, created_at
                 FROM restore_db.source_documents;",
            )
            .map_err(|e| e.to_string())?;
        }

        tx.execute_batch(
            "

            INSERT INTO bills (
                id, billing_period_id, provider_id, raw_text, amount_cents,
                creditor_name, creditor_iban, reference, due_date, purpose_code,
                purpose_text, parse_note, status, source_filename, creditor_address,
                creditor_city, creditor_postal_code, invoice_number
            )
            SELECT
                id, billing_period_id, provider_id, raw_text, amount_cents,
                creditor_name, creditor_iban, reference, due_date, purpose_code,
                purpose_text, parse_note, status, source_filename, creditor_address,
                creditor_city, creditor_postal_code, invoice_number
            FROM restore_db.bills;

            INSERT INTO bill_splits (id, bill_id, apartment_id, amount_cents)
            SELECT id, bill_id, apartment_id, amount_cents FROM restore_db.bill_splits;

            INSERT INTO smtp_config (id, host, port, username, from_email, use_tls, password)
            SELECT id, host, port, username, from_email, use_tls, ''
            FROM restore_db.smtp_config;
            ",
        )
        .map_err(|e| e.to_string())?;

        if has_billing_periods_closed_at {
            tx.execute(
                "UPDATE billing_periods
                 SET closed_at = (
                    SELECT closed_at
                    FROM restore_db.billing_periods
                    WHERE restore_db.billing_periods.id = billing_periods.id
                 )",
                [],
            )
            .map_err(|e| e.to_string())?;
        }

        if has_bills_reviewed_at {
            tx.execute(
                "UPDATE bills
                 SET reviewed_at = (
                    SELECT reviewed_at FROM restore_db.bills WHERE restore_db.bills.id = bills.id
                 )",
                [],
            )
            .map_err(|e| e.to_string())?;
        }

        if has_bills_review_note {
            tx.execute(
                "UPDATE bills
                 SET review_note = COALESCE((
                    SELECT review_note FROM restore_db.bills WHERE restore_db.bills.id = bills.id
                 ), '')",
                [],
            )
            .map_err(|e| e.to_string())?;
        }

        if has_provider_identity_rules {
            tx.execute_batch(
                "UPDATE providers SET
                    identity_rule_type=(SELECT identity_rule_type FROM restore_db.providers r WHERE r.id=providers.id),
                    identity_rule_operator=(SELECT identity_rule_operator FROM restore_db.providers r WHERE r.id=providers.id),
                    identity_label=(SELECT identity_label FROM restore_db.providers r WHERE r.id=providers.id),
                    identity_value=(SELECT identity_value FROM restore_db.providers r WHERE r.id=providers.id),
                    identity_alternate_label=(SELECT identity_alternate_label FROM restore_db.providers r WHERE r.id=providers.id),
                    identity_alternate_value=(SELECT identity_alternate_value FROM restore_db.providers r WHERE r.id=providers.id);",
            )
            .map_err(|e| e.to_string())?;
        }

        if has_bill_identity_status {
            tx.execute_batch(
                "UPDATE bills SET
                    identity_status=(SELECT identity_status FROM restore_db.bills r WHERE r.id=bills.id),
                    identity_rule_snapshot=(SELECT identity_rule_snapshot FROM restore_db.bills r WHERE r.id=bills.id),
                    identity_evidence=(SELECT identity_evidence FROM restore_db.bills r WHERE r.id=bills.id),
                    identity_exception_note=(SELECT identity_exception_note FROM restore_db.bills r WHERE r.id=bills.id),
                    identity_exception_at=(SELECT identity_exception_at FROM restore_db.bills r WHERE r.id=bills.id),
                    source_page_start=(SELECT source_page_start FROM restore_db.bills r WHERE r.id=bills.id),
                    source_page_end=(SELECT source_page_end FROM restore_db.bills r WHERE r.id=bills.id);",
            )
            .map_err(|e| e.to_string())?;
        }

        if has_source_documents && has_bill_source_document_id {
            tx.execute_batch(
                "UPDATE bills SET source_document_id=(
                    SELECT r.source_document_id FROM restore_db.bills r
                    JOIN source_documents d ON d.id=r.source_document_id
                    WHERE r.id=bills.id
                 );",
            )
            .map_err(|e| e.to_string())?;
        }

        if has_smtp_allowlist_enabled {
            tx.execute(
                "UPDATE smtp_config
                 SET allowlist_enabled = (
                    SELECT allowlist_enabled FROM restore_db.smtp_config WHERE id=1
                 )
                 WHERE id=1",
                [],
            )
            .map_err(|e| e.to_string())?;
        }

        if has_smtp_recipient_allowlist {
            tx.execute(
                "UPDATE smtp_config
                 SET recipient_allowlist = (
                    SELECT recipient_allowlist FROM restore_db.smtp_config WHERE id=1
                 )
                 WHERE id=1",
                [],
            )
            .map_err(|e| e.to_string())?;
        }

        if has_inbox_config {
            tx.execute(
                "INSERT INTO inbox_config (
                    id, host, port, username, password, use_tls, folder,
                    days_to_scan, sender_allowlist
                 )
                 SELECT
                    id, host, port, username, '', use_tls, folder,
                    days_to_scan, sender_allowlist
                 FROM restore_db.inbox_config WHERE id=1",
                [],
            )
            .map_err(|e| e.to_string())?;
        }
        tx.execute(
            "INSERT OR IGNORE INTO inbox_config (
                id, host, port, username, password, use_tls, folder,
                days_to_scan, sender_allowlist
             ) VALUES (1, '', 993, '', '', 1, 'INBOX', 45, '')",
            [],
        )
        .map_err(|e| e.to_string())?;

        if has_inbox_imports {
            tx.execute_batch(
                "
                INSERT INTO inbox_imports (
                    id, billing_period_id, folder, uid_validity, message_uid,
                    message_id, sender, subject, attachment_filename,
                    attachment_sha256, bill_ids, bill_count, status,
                    error_text, imported_at
                )
                SELECT
                    id, billing_period_id, folder, uid_validity, message_uid,
                    message_id, sender, subject, attachment_filename,
                    attachment_sha256, bill_ids, bill_count, status,
                    error_text, imported_at
                FROM restore_db.inbox_imports;
                ",
            )
            .map_err(|e| e.to_string())?;
        }

        if has_inbox_bill_hashes {
            tx.execute_batch(
                "
                INSERT INTO inbox_bill_hashes (
                    id, billing_period_id, inbox_import_id, bill_id,
                    bill_hash, created_at
                )
                SELECT
                    id, billing_period_id, inbox_import_id, bill_id,
                    bill_hash, created_at
                FROM restore_db.inbox_bill_hashes;
                ",
            )
            .map_err(|e| e.to_string())?;
        }

        if has_upn_delivery_events {
            tx.execute_batch(
                "
                INSERT INTO upn_delivery_events (
                    id, attempt_id, billing_period_id, apartment_id,
                    delivery_type, status, recipient, original_recipient,
                    attachment_sha256, error, created_at
                )
                SELECT
                    id, attempt_id, billing_period_id, apartment_id,
                    delivery_type, status, recipient, original_recipient,
                    attachment_sha256, error, created_at
                FROM restore_db.upn_delivery_events;
                ",
            )
            .map_err(|e| e.to_string())?;
        }

        if has_app_settings {
            tx.execute(
                "INSERT INTO app_settings (id, theme)
                 SELECT id, theme FROM restore_db.app_settings WHERE id=1",
                [],
            )
            .map_err(|e| e.to_string())?;
        } else {
            tx.execute(
                "INSERT INTO app_settings (id, theme) VALUES (1, 'refined')",
                [],
            )
            .map_err(|e| e.to_string())?;
        }
        tx.execute(
            "INSERT OR IGNORE INTO app_settings (id, theme) VALUES (1, 'refined')",
            [],
        )
        .map_err(|e| e.to_string())?;

        tx.execute(
            "DELETE FROM source_documents
             WHERE NOT EXISTS (
                 SELECT 1 FROM bills WHERE bills.source_document_id=source_documents.id
             )",
            [],
        )
        .map_err(|e| e.to_string())?;

        tx.commit().map_err(|e| e.to_string())
    })();

    let detach_result = conn.execute("DETACH DATABASE restore_db", []);
    match (restore_result, detach_result) {
        (Ok(()), Ok(_)) => Ok(()),
        (Err(err), _) => Err(err),
        (Ok(()), Err(err)) => Err(err.to_string()),
    }
}

fn ensure_required_tables_on_attached(conn: &Connection) -> Result<(), String> {
    for table in REQUIRED_TABLES {
        let exists = conn
            .query_row(
                "SELECT 1 FROM restore_db.sqlite_master WHERE type='table' AND name=?1",
                [table],
                |_| Ok(()),
            )
            .optional()
            .map_err(|e| e.to_string())?
            .is_some();

        if !exists {
            return Err(format!(
                "Backup file is missing required table '{}'.",
                table
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::config::PeriodOperationState;
    use crate::db::migrations;
    use std::sync::{Arc, Mutex};

    fn initialized_connection() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory database");
        migrations::run_migrations(&conn).expect("migrations");
        conn
    }

    fn backup_file(
        status: &str,
        closed_at: Option<&str>,
        legacy_without_closed_at: bool,
    ) -> tempfile::NamedTempFile {
        let file = tempfile::NamedTempFile::new().expect("backup file");
        let conn = Connection::open(file.path()).expect("backup database");
        migrations::run_migrations(&conn).expect("backup migrations");
        conn.execute(
            "INSERT INTO billing_periods
             (id, building_id, month, year, status, closed_at)
             VALUES (1, 1, 6, 2026, ?1, ?2)",
            rusqlite::params![status, closed_at],
        )
        .expect("backup period");
        if legacy_without_closed_at {
            conn.execute("ALTER TABLE billing_periods DROP COLUMN closed_at", [])
                .expect("remove newer backup column");
        }
        drop(conn);
        file
    }

    fn restore_backup(file: &tempfile::NamedTempFile) -> DbState {
        let state = DbState(
            Arc::new(Mutex::new(initialized_connection())),
            Arc::new(PeriodOperationState::default()),
        );
        restore_db_backup_inner(&state, file.path().to_string_lossy().into_owned())
            .expect("restore backup");
        state
    }

    #[test]
    fn restore_preserves_closed_status_and_timestamp() {
        let file = backup_file("closed", Some("2026-06-30 12:00:00"), false);
        let state = restore_backup(&file);
        let conn = state.0.lock().expect("database lock");

        let restored: (String, Option<String>) = conn
            .query_row(
                "SELECT status, closed_at FROM billing_periods WHERE id=1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("restored period");

        assert_eq!(restored.0, "closed");
        assert_eq!(restored.1.as_deref(), Some("2026-06-30 12:00:00"));
    }

    #[test]
    fn restore_accepts_legacy_backup_without_closed_at() {
        let file = backup_file("closed", None, true);
        let state = restore_backup(&file);
        let conn = state.0.lock().expect("database lock");

        let restored: (String, Option<String>) = conn
            .query_row(
                "SELECT status, closed_at FROM billing_periods WHERE id=1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("restored period");

        assert_eq!(restored.0, "closed");
        assert_eq!(restored.1, None);
    }

    #[test]
    fn restore_round_trips_identity_rules_and_exception_evidence() {
        let file = backup_file("draft", None, false);
        {
            let conn = Connection::open(file.path()).expect("open backup");
            conn.execute(
                "UPDATE providers SET identity_rule_type='labeled_value',
                 identity_rule_operator='any', identity_label='Account',
                 identity_value='00123', identity_alternate_label='Meter',
                 identity_alternate_value='4-56' WHERE id=1",
                [],
            )
            .expect("custom rule");
            conn.execute(
                "INSERT INTO bills (
                    id, billing_period_id, provider_id, amount_cents, creditor_name,
                    identity_status, identity_rule_snapshot, identity_evidence,
                    identity_exception_note, identity_exception_at,
                    source_page_start, source_page_end
                 ) VALUES (1, 1, 1, 1234, 'Provider', 'exception',
                    '{\"type\":\"labeled_value\"}', '{\"status\":\"missing\"}',
                    'Accepted after checking the paper copy', '2026-09-28 10:00:00', 2, 3)",
                [],
            )
            .expect("identity bill");
        }

        let state = restore_backup(&file);
        let conn = state.0.lock().expect("database lock");
        let rule: (String, String, String, String, String, String) = conn
            .query_row(
                "SELECT identity_rule_type, identity_rule_operator, identity_label,
                        identity_value, identity_alternate_label, identity_alternate_value
                 FROM providers WHERE id=1",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                    ))
                },
            )
            .expect("restored rule");
        assert_eq!(
            rule,
            (
                "labeled_value".into(),
                "any".into(),
                "Account".into(),
                "00123".into(),
                "Meter".into(),
                "4-56".into(),
            )
        );
        let evidence: (
            String,
            String,
            String,
            String,
            Option<String>,
            Option<i32>,
            Option<i32>,
        ) = conn
            .query_row(
                "SELECT identity_status, identity_rule_snapshot, identity_evidence,
                        identity_exception_note, identity_exception_at,
                        source_page_start, source_page_end FROM bills WHERE id=1",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                    ))
                },
            )
            .expect("restored evidence");
        assert_eq!(evidence.0, "exception");
        assert_eq!(evidence.1, "{\"type\":\"labeled_value\"}");
        assert_eq!(evidence.2, "{\"status\":\"missing\"}");
        assert_eq!(evidence.3, "Accepted after checking the paper copy");
        assert_eq!(evidence.4.as_deref(), Some("2026-09-28 10:00:00"));
        assert_eq!((evidence.5, evidence.6), (Some(2), Some(3)));
    }

    #[test]
    fn legacy_restore_does_not_invent_identity_rules_or_evidence() {
        let file = backup_file("draft", None, false);
        {
            let conn = Connection::open(file.path()).expect("open backup");
            conn.execute(
                "INSERT INTO bills (id, billing_period_id, provider_id, amount_cents, creditor_name)
                 VALUES (1, 1, 1, 1234, 'Legacy provider')",
                [],
            )
            .expect("legacy bill");
            for column in [
                "identity_rule_type",
                "identity_rule_operator",
                "identity_label",
                "identity_value",
                "identity_alternate_label",
                "identity_alternate_value",
            ] {
                conn.execute(&format!("ALTER TABLE providers DROP COLUMN {column}"), [])
                    .expect("drop provider identity column");
            }
            for column in [
                "identity_status",
                "identity_rule_snapshot",
                "identity_evidence",
                "identity_exception_note",
                "identity_exception_at",
                "source_page_start",
                "source_page_end",
            ] {
                conn.execute(&format!("ALTER TABLE bills DROP COLUMN {column}"), [])
                    .expect("drop bill identity column");
            }
        }

        let state = restore_backup(&file);
        let conn = state.0.lock().expect("database lock");
        let provider_rule: String = conn
            .query_row(
                "SELECT identity_rule_type FROM providers WHERE id=1",
                [],
                |row| row.get(0),
            )
            .expect("provider fallback");
        let bill_status: String = conn
            .query_row("SELECT identity_status FROM bills WHERE id=1", [], |row| {
                row.get(0)
            })
            .expect("bill fallback");
        assert_eq!(provider_rule, "unconfigured");
        assert_eq!(bill_status, "not_checked");
    }

    #[test]
    fn restore_round_trips_source_documents_and_bill_links() {
        let file = backup_file("draft", None, false);
        {
            let conn = Connection::open(file.path()).expect("open backup");
            conn.execute(
                "INSERT INTO source_documents
                 (id, original_name, media_type, byte_size, content_sha256, content)
                 VALUES (7, 'combined.pdf', 'application/pdf', 8, 'source-hash', X'255044462D312E37')",
                [],
            )
            .expect("source document");
            conn.execute(
                "INSERT INTO bills
                 (id, billing_period_id, provider_id, source_filename, source_document_id, source_page_start)
                 VALUES (9, 1, 1, 'combined.pdf', 7, 3)",
                [],
            )
            .expect("linked bill");
        }

        let state = restore_backup(&file);
        let conn = state.0.lock().expect("database lock");
        let restored: (Option<i64>, Vec<u8>, Option<i32>) = conn
            .query_row(
                "SELECT b.source_document_id, d.content, b.source_page_start
                 FROM bills b JOIN source_documents d ON d.id=b.source_document_id
                 WHERE b.id=9",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("restored document link");
        assert_eq!(restored.0, Some(7));
        assert_eq!(restored.1, b"%PDF-1.7");
        assert_eq!(restored.2, Some(3));
    }

    #[test]
    fn legacy_restore_clears_current_documents_and_uses_null_bill_links() {
        let file = backup_file("draft", None, false);
        {
            let conn = Connection::open(file.path()).expect("open backup");
            conn.execute(
                "INSERT INTO bills (id, billing_period_id, provider_id, source_filename)
                 VALUES (4, 1, 1, 'legacy.pdf')",
                [],
            )
            .expect("legacy bill");
            conn.execute("ALTER TABLE bills DROP COLUMN source_document_id", [])
                .expect("drop source link");
            conn.execute("DROP TABLE source_documents", [])
                .expect("drop source documents");
        }
        let state = DbState(
            Arc::new(Mutex::new(initialized_connection())),
            Arc::new(PeriodOperationState::default()),
        );
        {
            let conn = state.0.lock().unwrap();
            conn.execute(
                "INSERT INTO source_documents
                 (id, original_name, media_type, byte_size, content_sha256, content)
                 VALUES (99, 'unrelated.pdf', 'application/pdf', 1, 'current', X'00')",
                [],
            )
            .unwrap();
        }
        restore_db_backup_inner(&state, file.path().to_string_lossy().into_owned())
            .expect("legacy restore");
        let conn = state.0.lock().unwrap();
        let link: Option<i64> = conn
            .query_row(
                "SELECT source_document_id FROM bills WHERE id=4",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let document_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM source_documents", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(link, None);
        assert_eq!(document_count, 0);
    }

    #[test]
    fn backup_failure_preserves_existing_destination_and_cleans_temporary_file() {
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("existing.sqlite3");
        std::fs::write(&destination, b"existing backup").unwrap();
        let state = DbState(
            Arc::new(Mutex::new(Connection::open_in_memory().unwrap())),
            Arc::new(PeriodOperationState::default()),
        );

        let error =
            create_db_backup_inner(&state, destination.to_string_lossy().into_owned()).unwrap_err();

        assert!(error.contains("missing required table"));
        assert_eq!(std::fs::read(&destination).unwrap(), b"existing backup");
        let leftovers = std::fs::read_dir(directory.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".upn-backup-")
            })
            .count();
        assert_eq!(leftovers, 0);
    }

    #[test]
    fn backup_atomically_replaces_existing_destination_after_password_scrub() {
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("existing.sqlite3");
        std::fs::write(&destination, b"old backup").unwrap();
        let state = DbState(
            Arc::new(Mutex::new(initialized_connection())),
            Arc::new(PeriodOperationState::default()),
        );
        {
            let conn = state.0.lock().unwrap();
            conn.execute(
                "UPDATE smtp_config SET password='smtp-secret' WHERE id=1",
                [],
            )
            .unwrap();
            conn.execute(
                "UPDATE inbox_config SET password='imap-secret' WHERE id=1",
                [],
            )
            .unwrap();
        }

        create_db_backup_inner(&state, destination.to_string_lossy().into_owned()).unwrap();

        let backup = Connection::open(&destination).unwrap();
        let smtp_password: String = backup
            .query_row("SELECT password FROM smtp_config WHERE id=1", [], |row| {
                row.get(0)
            })
            .unwrap();
        let inbox_password: String = backup
            .query_row("SELECT password FROM inbox_config WHERE id=1", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert!(smtp_password.is_empty());
        assert!(inbox_password.is_empty());
    }
}
