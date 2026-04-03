use rusqlite::{Connection, DatabaseName, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tauri::State;

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

#[tauri::command]
pub fn create_db_backup(db: State<DbState>, output_path: String) -> Result<BackupFileInfo, String> {
    let backup_path = backup_output_path(&output_path)?;

    {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        conn.backup(DatabaseName::Main, backup_path, None)
            .map_err(|e| e.to_string())?;
    }

    let backup_conn = Connection::open(backup_path).map_err(|e| e.to_string())?;
    ensure_required_tables(&backup_conn)?;
    backup_conn
        .execute("UPDATE smtp_config SET password='' WHERE id=1", [])
        .map_err(|e| e.to_string())?;

    Ok(BackupFileInfo { path: output_path })
}

#[tauri::command]
pub fn restore_db_backup(db: State<DbState>, input_path: String) -> Result<(), String> {
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

        let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;

        tx.execute_batch(
            "
            DELETE FROM bill_splits;
            DELETE FROM bills;
            DELETE FROM billing_periods;
            DELETE FROM apartments;
            DELETE FROM providers;
            DELETE FROM building;
            DELETE FROM smtp_config;
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
