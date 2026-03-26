use rusqlite::params;
use serde::{Deserialize, Serialize};
use tauri::State;

use super::config::DbState;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BillSplit {
    pub id: Option<i64>,
    pub bill_id: i64,
    pub apartment_id: i64,
    pub amount_cents: i64,
}

/// One row in the splits view: a bill split enriched with names for display.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SplitRow {
    pub split_id: Option<i64>,
    pub bill_id: i64,
    pub apartment_id: i64,
    pub apartment_label: String,
    pub bill_source_filename: String,
    pub provider_name: Option<String>,
    pub bill_amount_cents: i64,
    pub split_amount_cents: i64,
    pub occupant_count: i32,
}

/// Recalculate splits for every bill in the given billing period.
/// Splits are proportional to occupant_count of active apartments.
/// Existing splits for these bills are replaced.
#[tauri::command]
pub fn calculate_splits(
    db: State<DbState>,
    billing_period_id: i64,
) -> Result<Vec<SplitRow>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;

    // Get all active apartments
    let mut stmt = conn
        .prepare(
            "SELECT id, label, occupant_count FROM apartments
             WHERE building_id=1 AND is_active=1 ORDER BY label",
        )
        .map_err(|e| e.to_string())?;
    let apartments: Vec<(i64, String, i32)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    if apartments.is_empty() {
        return Err("No active apartments configured.".to_string());
    }

    let total_occupants: i32 = apartments.iter().map(|(_, _, c)| c).sum();
    if total_occupants == 0 {
        return Err("Total occupant count is zero.".to_string());
    }

    // Get all bills for this period
    let mut stmt = conn
        .prepare(
            "SELECT b.id, b.amount_cents, b.source_filename, p.name
             FROM bills b
             LEFT JOIN providers p ON b.provider_id = p.id
             WHERE b.billing_period_id = ?1",
        )
        .map_err(|e| e.to_string())?;
    let bills: Vec<(i64, i64, String, Option<String>)> = stmt
        .query_map([billing_period_id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    if bills.is_empty() {
        return Err("No bills in this billing period.".to_string());
    }

    let mut result_rows: Vec<SplitRow> = Vec::new();

    for (bill_id, bill_amount, source_filename, provider_name) in &bills {
        // Delete existing splits for this bill
        conn.execute("DELETE FROM bill_splits WHERE bill_id=?1", [bill_id])
            .map_err(|e| e.to_string())?;

        // Calculate split per apartment
        let mut remaining = *bill_amount;
        let mut splits: Vec<(i64, i64)> = Vec::new(); // (apartment_id, amount_cents)

        for (i, (apt_id, _, occupants)) in apartments.iter().enumerate() {
            let share = if i == apartments.len() - 1 {
                // Last apartment gets the remainder to avoid rounding drift
                remaining
            } else {
                let s =
                    ((*bill_amount as f64 * *occupants as f64) / total_occupants as f64).round()
                        as i64;
                remaining -= s;
                s
            };
            splits.push((*apt_id, share));
        }

        // Insert splits
        for (apt_id, share) in &splits {
            conn.execute(
                "INSERT OR REPLACE INTO bill_splits (bill_id, apartment_id, amount_cents)
                 VALUES (?1, ?2, ?3)",
                params![bill_id, apt_id, share],
            )
            .map_err(|e| e.to_string())?;
            let split_id = conn.last_insert_rowid();

            let apt = apartments.iter().find(|(id, _, _)| id == apt_id).unwrap();
            result_rows.push(SplitRow {
                split_id: Some(split_id),
                bill_id: *bill_id,
                apartment_id: *apt_id,
                apartment_label: apt.1.clone(),
                bill_source_filename: source_filename.clone(),
                provider_name: provider_name.clone(),
                bill_amount_cents: *bill_amount,
                split_amount_cents: *share,
                occupant_count: apt.2,
            });
        }
    }

    Ok(result_rows)
}

/// Fetch existing splits for a billing period (without recalculating).
#[tauri::command]
pub fn get_splits(
    db: State<DbState>,
    billing_period_id: i64,
) -> Result<Vec<SplitRow>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT bs.id, bs.bill_id, bs.apartment_id, a.label,
             b.source_filename, p.name, b.amount_cents, bs.amount_cents, a.occupant_count
             FROM bill_splits bs
             JOIN bills b ON bs.bill_id = b.id
             JOIN apartments a ON bs.apartment_id = a.id
             LEFT JOIN providers p ON b.provider_id = p.id
             WHERE b.billing_period_id = ?1
             ORDER BY a.label, b.id",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([billing_period_id], |r| {
            Ok(SplitRow {
                split_id: Some(r.get(0)?),
                bill_id: r.get(1)?,
                apartment_id: r.get(2)?,
                apartment_label: r.get(3)?,
                bill_source_filename: r.get(4)?,
                provider_name: r.get(5)?,
                bill_amount_cents: r.get(6)?,
                split_amount_cents: r.get(7)?,
                occupant_count: r.get(8)?,
            })
        })
        .map_err(|e| e.to_string())?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

/// Manually override a single split amount.
#[tauri::command]
pub fn save_split(db: State<DbState>, split: BillSplit) -> Result<BillSplit, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    match split.id {
        Some(id) => {
            conn.execute(
                "UPDATE bill_splits SET amount_cents=?1 WHERE id=?2",
                params![split.amount_cents, id],
            )
            .map_err(|e| e.to_string())?;
            Ok(split)
        }
        None => Err("Cannot update split without id".to_string()),
    }
}
