use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::State;

pub struct DbState(pub Mutex<Connection>);

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
    pub occupant_count: i32,
    pub contact_email: String,
    pub payer_name: String,
    pub payer_address: String,
    pub payer_city: String,
    pub payer_postal_code: String,
    pub is_active: bool,
}

#[tauri::command]
pub fn get_apartments(db: State<DbState>) -> Result<Vec<Apartment>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, building_id, label, occupant_count, contact_email,
             payer_name, payer_address, payer_city, payer_postal_code, is_active
             FROM apartments ORDER BY label",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(Apartment {
                id: Some(row.get(0)?),
                building_id: row.get(1)?,
                label: row.get(2)?,
                occupant_count: row.get(3)?,
                contact_email: row.get(4)?,
                payer_name: row.get(5)?,
                payer_address: row.get(6)?,
                payer_city: row.get(7)?,
                payer_postal_code: row.get(8)?,
                is_active: row.get::<_, i32>(9)? != 0,
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
                "UPDATE apartments SET label=?1, occupant_count=?2, contact_email=?3,
                 payer_name=?4, payer_address=?5, payer_city=?6, payer_postal_code=?7, is_active=?8
                 WHERE id=?9",
                rusqlite::params![
                    apartment.label,
                    apartment.occupant_count,
                    apartment.contact_email,
                    apartment.payer_name,
                    apartment.payer_address,
                    apartment.payer_city,
                    apartment.payer_postal_code,
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
                 (building_id, label, occupant_count, contact_email, payer_name, payer_address, payer_city, payer_postal_code, is_active)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
                rusqlite::params![
                    apartment.building_id,
                    apartment.label,
                    apartment.occupant_count,
                    apartment.contact_email,
                    apartment.payer_name,
                    apartment.payer_address,
                    apartment.payer_city,
                    apartment.payer_postal_code,
                    active
                ],
            )
            .map_err(|e| e.to_string())?;
            let id = conn.last_insert_rowid();
            Ok(Apartment { id: Some(id), ..apartment })
        }
    }
}

#[tauri::command]
pub fn delete_apartment(db: State<DbState>, id: i64) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM apartments WHERE id=?1", [id])
        .map_err(|e| e.to_string())?;
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
}

#[tauri::command]
pub fn get_providers(db: State<DbState>) -> Result<Vec<Provider>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, name, service_type, creditor_name, creditor_address, creditor_city,
             creditor_postal_code, creditor_iban, purpose_code, match_pattern, amount_pattern,
             reference_pattern, due_date_pattern, invoice_number_pattern, purpose_text_template
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
            })
        })
        .map_err(|e| e.to_string())?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

#[tauri::command]
pub fn save_provider(db: State<DbState>, provider: Provider) -> Result<Provider, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    match provider.id {
        Some(id) => {
            conn.execute(
                "UPDATE providers SET name=?1, service_type=?2, creditor_name=?3, creditor_address=?4,
                 creditor_city=?5, creditor_postal_code=?6, creditor_iban=?7, purpose_code=?8,
                 match_pattern=?9, amount_pattern=?10, reference_pattern=?11, due_date_pattern=?12,
                 invoice_number_pattern=?13, purpose_text_template=?14 WHERE id=?15",
                rusqlite::params![
                    provider.name, provider.service_type, provider.creditor_name,
                    provider.creditor_address, provider.creditor_city, provider.creditor_postal_code,
                    provider.creditor_iban, provider.purpose_code, provider.match_pattern,
                    provider.amount_pattern, provider.reference_pattern, provider.due_date_pattern,
                    provider.invoice_number_pattern, provider.purpose_text_template, id
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
                  reference_pattern, due_date_pattern, invoice_number_pattern, purpose_text_template)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
                rusqlite::params![
                    provider.name, provider.service_type, provider.creditor_name,
                    provider.creditor_address, provider.creditor_city, provider.creditor_postal_code,
                    provider.creditor_iban, provider.purpose_code, provider.match_pattern,
                    provider.amount_pattern, provider.reference_pattern, provider.due_date_pattern,
                    provider.invoice_number_pattern, provider.purpose_text_template
                ],
            )
            .map_err(|e| e.to_string())?;
            let id = conn.last_insert_rowid();
            Ok(Provider { id: Some(id), ..provider })
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
}

#[tauri::command]
pub fn get_smtp_config(db: State<DbState>) -> Result<SmtpConfig, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.query_row(
        "SELECT host, port, username, from_email, use_tls FROM smtp_config WHERE id=1",
        [],
        |row| {
            Ok(SmtpConfig {
                host: row.get(0)?,
                port: row.get(1)?,
                username: row.get(2)?,
                from_email: row.get(3)?,
                use_tls: row.get::<_, i32>(4)? != 0,
            })
        },
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_smtp_config(db: State<DbState>, config: SmtpConfig) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE smtp_config SET host=?1, port=?2, username=?3, from_email=?4, use_tls=?5 WHERE id=1",
        rusqlite::params![
            config.host, config.port, config.username, config.from_email,
            if config.use_tls { 1 } else { 0 }
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
