use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::State;

use crate::credentials::{self, MailCredentialKind};
use crate::db::migrations;

#[derive(Default)]
pub struct PeriodOperationState {
    active_email_sends: Mutex<HashMap<i64, usize>>,
}

pub struct PeriodEmailSendGuard {
    state: Arc<PeriodOperationState>,
    billing_period_id: i64,
}

impl Drop for PeriodEmailSendGuard {
    fn drop(&mut self) {
        let mut active = self
            .state
            .active_email_sends
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(count) = active.get_mut(&self.billing_period_id) {
            *count -= 1;
            if *count == 0 {
                active.remove(&self.billing_period_id);
            }
        }
    }
}

pub struct DbState(pub Arc<Mutex<Connection>>, pub Arc<PeriodOperationState>);

impl DbState {
    pub(crate) fn begin_period_email_send<F>(
        &self,
        billing_period_id: i64,
        ensure_open: F,
    ) -> Result<PeriodEmailSendGuard, String>
    where
        F: FnOnce(&Connection) -> Result<(), String>,
    {
        // Registration and close use the same operation-state -> database lock order.
        let mut active = self
            .1
            .active_email_sends
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let conn = self.0.lock().map_err(|e| e.to_string())?;
        ensure_open(&conn)?;
        *active.entry(billing_period_id).or_insert(0) += 1;
        Ok(PeriodEmailSendGuard {
            state: Arc::clone(&self.1),
            billing_period_id,
        })
    }

    pub(crate) fn while_period_email_sends_idle<T, F>(
        &self,
        billing_period_id: i64,
        action: F,
    ) -> Result<T, String>
    where
        F: FnOnce(&Connection) -> Result<T, String>,
    {
        // Keep the state lock through the status transition so a new send cannot register midway.
        let active = self
            .1
            .active_email_sends
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if active.get(&billing_period_id).copied().unwrap_or(0) > 0 {
            return Err(
                "Cannot close this billing month while an email send is in progress.".to_string(),
            );
        }
        let conn = self.0.lock().map_err(|e| e.to_string())?;
        action(&conn)
    }

    #[cfg(test)]
    fn active_email_send_count(&self, billing_period_id: i64) -> usize {
        self.1
            .active_email_sends
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(&billing_period_id)
            .copied()
            .unwrap_or(0)
    }
}

// --- Building ---

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Building {
    pub id: Option<i64>,
    pub name: String,
    pub address: String,
    pub city: String,
    pub postal_code: String,
}

#[tauri::command]
pub fn get_building(db: State<DbState>) -> Result<Building, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.query_row(
        "SELECT id, name, address, city, postal_code FROM building WHERE id = 1",
        [],
        |row| {
            Ok(Building {
                id: Some(row.get(0)?),
                name: row.get(1)?,
                address: row.get(2)?,
                city: row.get(3)?,
                postal_code: row.get(4)?,
            })
        },
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_building(db: State<DbState>, building: Building) -> Result<Building, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE building SET name=?1, address=?2, city=?3, postal_code=?4 WHERE id=1",
        rusqlite::params![
            building.name,
            building.address,
            building.city,
            building.postal_code
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(building)
}

// --- Apartments ---

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Apartment {
    pub id: Option<i64>,
    pub building_id: i64,
    pub label: String,
    pub unit_code: String,
    pub occupant_count: i32,
    pub contact_email: String,
    pub payer_name: String,
    pub payer_address: String,
    pub payer_city: String,
    pub payer_postal_code: String,
    pub m2_percentage: f64,
    pub is_active: bool,
}

#[tauri::command]
pub fn get_apartments(db: State<DbState>) -> Result<Vec<Apartment>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, building_id, label, unit_code, occupant_count, contact_email,
             payer_name, payer_address, payer_city, payer_postal_code, m2_percentage, is_active
             FROM apartments ORDER BY label",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(Apartment {
                id: Some(row.get(0)?),
                building_id: row.get(1)?,
                label: row.get(2)?,
                unit_code: row.get(3)?,
                occupant_count: row.get(4)?,
                contact_email: row.get(5)?,
                payer_name: row.get(6)?,
                payer_address: row.get(7)?,
                payer_city: row.get(8)?,
                payer_postal_code: row.get(9)?,
                m2_percentage: row.get(10)?,
                is_active: row.get::<_, i32>(11)? != 0,
            })
        })
        .map_err(|e| e.to_string())?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

#[tauri::command]
pub fn save_apartment(db: State<DbState>, apartment: Apartment) -> Result<Apartment, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let active = if apartment.is_active { 1 } else { 0 };
    match apartment.id {
        Some(id) => {
            conn.execute(
                "UPDATE apartments SET label=?1, unit_code=?2, occupant_count=?3, contact_email=?4,
                 payer_name=?5, payer_address=?6, payer_city=?7, payer_postal_code=?8, m2_percentage=?9, is_active=?10
                 WHERE id=?11",
                rusqlite::params![
                    apartment.label,
                    apartment.unit_code,
                    apartment.occupant_count,
                    apartment.contact_email,
                    apartment.payer_name,
                    apartment.payer_address,
                    apartment.payer_city,
                    apartment.payer_postal_code,
                    apartment.m2_percentage,
                    active,
                    id
                ],
            )
            .map_err(|e| e.to_string())?;
            Ok(apartment)
        }
        None => {
            conn.execute(
                "INSERT INTO apartments
                 (building_id, label, unit_code, occupant_count, contact_email, payer_name, payer_address, payer_city, payer_postal_code, m2_percentage, is_active)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
                rusqlite::params![
                    apartment.building_id,
                    apartment.label,
                    apartment.unit_code,
                    apartment.occupant_count,
                    apartment.contact_email,
                    apartment.payer_name,
                    apartment.payer_address,
                    apartment.payer_city,
                    apartment.payer_postal_code,
                    apartment.m2_percentage,
                    active
                ],
            )
            .map_err(|e| e.to_string())?;
            let id = conn.last_insert_rowid();
            Ok(Apartment {
                id: Some(id),
                ..apartment
            })
        }
    }
}

#[tauri::command]
pub fn delete_apartment(db: State<DbState>, id: i64) -> Result<(), String> {
    let mut conn = db.0.lock().map_err(|e| e.to_string())?;
    delete_apartment_inner(&mut conn, id)
}

fn delete_apartment_inner(conn: &mut Connection, id: i64) -> Result<(), String> {
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let has_closed_records: bool = tx
        .query_row(
            "SELECT EXISTS (
                SELECT 1
                FROM bill_splits bs
                JOIN bills b ON b.id = bs.bill_id
                JOIN billing_periods bp ON bp.id = b.billing_period_id
                WHERE bs.apartment_id=?1 AND bp.status='closed'
                UNION ALL
                SELECT 1
                FROM upn_delivery_events ude
                JOIN billing_periods bp ON bp.id = ude.billing_period_id
                WHERE ude.apartment_id=?1 AND bp.status='closed'
            )",
            [id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    if has_closed_records {
        return Err(
            "Cannot delete this apartment because it has records in a closed billing month. Reopen the affected month first."
                .to_string(),
        );
    }
    tx.execute(
        "DELETE FROM upn_delivery_events WHERE apartment_id=?1",
        [id],
    )
    .map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM bill_splits WHERE apartment_id=?1", [id])
        .map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM apartments WHERE id=?1", [id])
        .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

// --- Providers ---

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Provider {
    pub id: Option<i64>,
    pub name: String,
    pub service_type: String,
    pub creditor_name: String,
    pub creditor_address: String,
    pub creditor_city: String,
    pub creditor_postal_code: String,
    pub creditor_iban: String,
    pub purpose_code: String,
    pub match_pattern: String,
    pub amount_pattern: String,
    pub reference_pattern: String,
    pub due_date_pattern: String,
    pub invoice_number_pattern: String,
    pub purpose_text_template: String,
    pub split_basis: String,
    pub identity_rule_type: String,
    pub identity_rule_operator: String,
    pub identity_label: String,
    pub identity_value: String,
    pub identity_alternate_label: String,
    pub identity_alternate_value: String,
}

fn validate_identity_rule(provider: &Provider) -> Result<(), String> {
    match provider.identity_rule_type.as_str() {
        "unconfigured" => Ok(()),
        "building_address" => {
            if provider.identity_value.trim().is_empty() {
                Err("Building-address verification requires an address value.".to_string())
            } else {
                Ok(())
            }
        }
        "labeled_value" => {
            if !matches!(provider.identity_rule_operator.as_str(), "all" | "any") {
                return Err("Identity rule operator must be 'all' or 'any'.".to_string());
            }
            if provider.identity_label.trim().is_empty()
                || provider.identity_value.trim().is_empty()
            {
                return Err("Labeled-value verification requires a label and value.".to_string());
            }
            let has_alt_label = !provider.identity_alternate_label.trim().is_empty();
            let has_alt_value = !provider.identity_alternate_value.trim().is_empty();
            if has_alt_label != has_alt_value {
                return Err(
                    "The alternate identity label and value must both be filled or both be empty."
                        .to_string(),
                );
            }
            Ok(())
        }
        _ => Err("Unknown provider identity rule type.".to_string()),
    }
}

#[tauri::command]
pub fn get_providers(db: State<DbState>) -> Result<Vec<Provider>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, name, service_type, creditor_name, creditor_address, creditor_city,
             creditor_postal_code, creditor_iban, purpose_code, match_pattern, amount_pattern,
             reference_pattern, due_date_pattern, invoice_number_pattern, purpose_text_template,
             split_basis, identity_rule_type, identity_rule_operator, identity_label,
             identity_value, identity_alternate_label, identity_alternate_value
             FROM providers ORDER BY name",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
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
        .map_err(|e| e.to_string())?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

#[tauri::command]
pub fn save_provider(db: State<DbState>, provider: Provider) -> Result<Provider, String> {
    validate_identity_rule(&provider)?;
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    match provider.id {
        Some(id) => {
            conn.execute(
                "UPDATE providers SET name=?1, service_type=?2, creditor_name=?3, creditor_address=?4,
                 creditor_city=?5, creditor_postal_code=?6, creditor_iban=?7, purpose_code=?8,
                 match_pattern=?9, amount_pattern=?10, reference_pattern=?11, due_date_pattern=?12,
                 invoice_number_pattern=?13, purpose_text_template=?14, split_basis=?15,
                 identity_rule_type=?16, identity_rule_operator=?17, identity_label=?18,
                 identity_value=?19, identity_alternate_label=?20, identity_alternate_value=?21
                 WHERE id=?22",
                rusqlite::params![
                    provider.name, provider.service_type, provider.creditor_name,
                    provider.creditor_address, provider.creditor_city, provider.creditor_postal_code,
                    provider.creditor_iban, provider.purpose_code, provider.match_pattern,
                    provider.amount_pattern, provider.reference_pattern, provider.due_date_pattern,
                    provider.invoice_number_pattern, provider.purpose_text_template, provider.split_basis,
                    provider.identity_rule_type, provider.identity_rule_operator, provider.identity_label,
                    provider.identity_value, provider.identity_alternate_label,
                    provider.identity_alternate_value, id
                ],
            )
            .map_err(|e| e.to_string())?;
            Ok(provider)
        }
        None => {
            conn.execute(
                "INSERT INTO providers
                 (name, service_type, creditor_name, creditor_address, creditor_city,
                  creditor_postal_code, creditor_iban, purpose_code, match_pattern, amount_pattern,
                  reference_pattern, due_date_pattern, invoice_number_pattern, purpose_text_template, split_basis,
                  identity_rule_type, identity_rule_operator, identity_label, identity_value,
                  identity_alternate_label, identity_alternate_value)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21)",
                rusqlite::params![
                    provider.name, provider.service_type, provider.creditor_name,
                    provider.creditor_address, provider.creditor_city, provider.creditor_postal_code,
                    provider.creditor_iban, provider.purpose_code, provider.match_pattern,
                    provider.amount_pattern, provider.reference_pattern, provider.due_date_pattern,
                    provider.invoice_number_pattern, provider.purpose_text_template, provider.split_basis,
                    provider.identity_rule_type, provider.identity_rule_operator, provider.identity_label,
                    provider.identity_value, provider.identity_alternate_label,
                    provider.identity_alternate_value
                ],
            )
            .map_err(|e| e.to_string())?;
            let id = conn.last_insert_rowid();
            Ok(Provider {
                id: Some(id),
                ..provider
            })
        }
    }
}

#[tauri::command]
pub fn delete_provider(db: State<DbState>, id: i64) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM providers WHERE id=?1", [id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

// --- SMTP Config ---

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SmtpConfig {
    pub host: String,
    pub port: i32,
    pub username: String,
    pub from_email: String,
    pub use_tls: bool,
    pub allowlist_enabled: bool,
    pub recipient_allowlist: String,
    pub password_configured: bool,
}

#[tauri::command]
pub fn get_smtp_config(db: State<DbState>) -> Result<SmtpConfig, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut config = conn.query_row(
        "SELECT host, port, username, from_email, use_tls, allowlist_enabled, recipient_allowlist
         FROM smtp_config WHERE id=1",
        [],
        |row| {
            Ok(SmtpConfig {
                host: row.get(0)?,
                port: row.get(1)?,
                username: row.get(2)?,
                from_email: row.get(3)?,
                use_tls: row.get::<_, i32>(4)? != 0,
                allowlist_enabled: row.get::<_, i32>(5)? != 0,
                recipient_allowlist: row.get(6)?,
                password_configured: false,
            })
        },
    )
    .map_err(|e| e.to_string())?;
    config.password_configured =
        credentials::password_configured(MailCredentialKind::Smtp, &config.username)?;
    Ok(config)
}

#[tauri::command]
pub fn save_smtp_config(db: State<DbState>, config: SmtpConfig) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE smtp_config
         SET host=?1, port=?2, username=?3, from_email=?4, use_tls=?5,
             allowlist_enabled=?6, recipient_allowlist=?7
         WHERE id=1",
        rusqlite::params![
            config.host,
            config.port,
            config.username,
            config.from_email,
            if config.use_tls { 1 } else { 0 },
            if config.allowlist_enabled { 1 } else { 0 },
            config.recipient_allowlist.trim()
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

// --- App Settings ---

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppSettings {
    pub theme: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ResetAllDataResult {
    pub credential_cleanup_warning: Option<String>,
}

fn is_valid_theme(theme: &str) -> bool {
    matches!(
        theme,
        "refined" | "crisp" | "official" | "dark-crisp" | "dark-mono" | "dark-shadow"
    )
}

#[tauri::command]
pub fn get_app_settings(db: State<DbState>) -> Result<AppSettings, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.query_row("SELECT theme FROM app_settings WHERE id=1", [], |row| {
        Ok(AppSettings { theme: row.get(0)? })
    })
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_app_settings(db: State<DbState>, settings: AppSettings) -> Result<AppSettings, String> {
    if !is_valid_theme(&settings.theme) {
        return Err("Invalid theme.".to_string());
    }

    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE app_settings SET theme=?1 WHERE id=1",
        rusqlite::params![&settings.theme],
    )
    .map_err(|e| e.to_string())?;
    Ok(settings)
}

#[tauri::command]
pub fn reset_all_data(db: State<DbState>) -> Result<ResetAllDataResult, String> {
    let compaction_warning = {
        let mut conn = db.0.lock().map_err(|e| e.to_string())?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        migrations::reset_to_defaults(&tx)?;
        tx.commit().map_err(|e| e.to_string())?;
        conn.execute_batch("VACUUM")
            .err()
            .map(|error| format!("Database compaction failed after the data reset: {error}"))
    };
    let credential_warning = credentials::delete_mail_credentials()
        .err()
        .map(|error| format!("Saved Windows mail credentials could not be deleted: {error}"));
    let credential_cleanup_warning = match (compaction_warning, credential_warning) {
        (None, None) => None,
        (Some(warning), None) | (None, Some(warning)) => Some(format!(
            "App data was reset, but cleanup needs attention. {warning}"
        )),
        (Some(first), Some(second)) => Some(format!(
            "App data was reset, but cleanup needs attention. {first} {second}"
        )),
    };
    Ok(ResetAllDataResult {
        credential_cleanup_warning,
    })
}

#[cfg(test)]
mod period_operation_tests {
    use super::*;
    use std::panic::{catch_unwind, AssertUnwindSafe};

    fn test_state() -> DbState {
        let conn = Connection::open_in_memory().expect("in-memory database");
        conn.execute_batch(
            "CREATE TABLE billing_periods (
                id INTEGER PRIMARY KEY,
                status TEXT NOT NULL
            );
            INSERT INTO billing_periods (id, status) VALUES (1, 'draft');",
        )
        .expect("test schema");
        DbState(
            Arc::new(Mutex::new(conn)),
            Arc::new(PeriodOperationState::default()),
        )
    }

    fn register_send(state: &DbState) -> Result<PeriodEmailSendGuard, String> {
        state.begin_period_email_send(1, |conn| {
            let status: String = conn
                .query_row("SELECT status FROM billing_periods WHERE id=1", [], |row| {
                    row.get(0)
                })
                .map_err(|error| error.to_string())?;
            if status == "closed" {
                return Err("closed".to_string());
            }
            Ok(())
        })
    }

    #[test]
    fn overlapping_email_send_guards_block_until_both_drop() {
        let state = test_state();
        let first = register_send(&state).expect("first guard");
        let second = register_send(&state).expect("second guard");

        assert_eq!(state.active_email_send_count(1), 2);
        assert!(state.while_period_email_sends_idle(1, |_| Ok(())).is_err());
        drop(first);
        assert_eq!(state.active_email_send_count(1), 1);
        assert!(state.while_period_email_sends_idle(1, |_| Ok(())).is_err());
        drop(second);
        assert_eq!(state.active_email_send_count(1), 0);
        assert!(state.while_period_email_sends_idle(1, |_| Ok(())).is_ok());
    }

    #[test]
    fn email_send_registration_failure_does_not_leak_counter() {
        let state = test_state();
        let result = state.begin_period_email_send(1, |_| Err("validation failed".to_string()));

        assert!(result.is_err());
        assert_eq!(state.active_email_send_count(1), 0);
    }

    #[test]
    fn email_send_guard_deregisters_during_unwind() {
        let state = test_state();
        let result = catch_unwind(AssertUnwindSafe(|| {
            let _guard = register_send(&state).expect("guard");
            panic!("simulated send panic");
        }));

        assert!(result.is_err());
        assert_eq!(state.active_email_send_count(1), 0);
        assert!(state.while_period_email_sends_idle(1, |_| Ok(())).is_ok());
    }

    fn apartment_delete_conn(period_status: &str) -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory database");
        conn.execute_batch(&format!(
            "CREATE TABLE apartments (id INTEGER PRIMARY KEY);
             CREATE TABLE billing_periods (id INTEGER PRIMARY KEY, status TEXT NOT NULL);
             CREATE TABLE bills (
                id INTEGER PRIMARY KEY,
                billing_period_id INTEGER NOT NULL
             );
             CREATE TABLE bill_splits (
                id INTEGER PRIMARY KEY,
                bill_id INTEGER NOT NULL,
                apartment_id INTEGER NOT NULL
             );
             CREATE TABLE upn_delivery_events (
                id INTEGER PRIMARY KEY,
                billing_period_id INTEGER NOT NULL,
                apartment_id INTEGER NOT NULL
             );
             INSERT INTO apartments (id) VALUES (1);
             INSERT INTO billing_periods (id, status) VALUES (1, '{period_status}');
             INSERT INTO bills (id, billing_period_id) VALUES (1, 1);
             INSERT INTO bill_splits (id, bill_id, apartment_id) VALUES (1, 1, 1);
             INSERT INTO upn_delivery_events
                (id, billing_period_id, apartment_id) VALUES (1, 1, 1);"
        ))
        .expect("apartment delete schema");
        conn
    }

    #[test]
    fn apartment_delete_rejects_closed_period_records_without_removing_data() {
        let mut conn = apartment_delete_conn("closed");

        let error = delete_apartment_inner(&mut conn, 1).unwrap_err();

        assert!(error.contains("closed billing month"));
        for table in ["apartments", "bill_splits", "upn_delivery_events"] {
            let count: i64 = conn
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .expect("row count");
            assert_eq!(count, 1, "{table} data should remain");
        }
    }

    #[test]
    fn apartment_delete_rejects_closed_delivery_history_without_splits() {
        let mut conn = apartment_delete_conn("closed");
        conn.execute("DELETE FROM bill_splits", [])
            .expect("remove split fixture");

        let error = delete_apartment_inner(&mut conn, 1).unwrap_err();

        assert!(error.contains("closed billing month"));
        let apartment_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM apartments", [], |row| row.get(0))
            .expect("apartment count");
        let event_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM upn_delivery_events", [], |row| {
                row.get(0)
            })
            .expect("event count");
        assert_eq!(apartment_count, 1);
        assert_eq!(event_count, 1);
    }

    #[test]
    fn apartment_delete_rejects_closed_splits_without_delivery_history() {
        let mut conn = apartment_delete_conn("closed");
        conn.execute("DELETE FROM upn_delivery_events", [])
            .expect("remove delivery fixture");

        let error = delete_apartment_inner(&mut conn, 1).unwrap_err();

        assert!(error.contains("closed billing month"));
        let apartment_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM apartments", [], |row| row.get(0))
            .expect("apartment count");
        let split_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM bill_splits", [], |row| row.get(0))
            .expect("split count");
        assert_eq!(apartment_count, 1);
        assert_eq!(split_count, 1);
    }

    #[test]
    fn apartment_delete_removes_records_when_period_is_open() {
        let mut conn = apartment_delete_conn("draft");

        delete_apartment_inner(&mut conn, 1).expect("delete apartment");

        for table in ["apartments", "bill_splits", "upn_delivery_events"] {
            let count: i64 = conn
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .expect("row count");
            assert_eq!(count, 0, "{table} data should be deleted");
        }
    }
}
