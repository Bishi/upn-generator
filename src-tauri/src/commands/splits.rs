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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SplitRow {
    pub split_id: Option<i64>,
    pub bill_id: i64,
    pub apartment_id: i64,
    pub apartment_label: String,
    pub apartment_unit_code: String,
    pub bill_source_filename: String,
    pub provider_name: Option<String>,
    pub bill_amount_cents: i64,
    pub split_amount_cents: i64,
    pub occupant_count: i32,
    pub m2_percentage: f64,
    pub split_basis: String,
}

#[derive(Clone)]
struct ApartmentWeight {
    id: i64,
    label: String,
    unit_code: String,
    occupant_count: i32,
    m2_percentage: f64,
}

fn calculate_weighted_shares(
    bill_amount: i64,
    apartments: &[ApartmentWeight],
    split_basis: &str,
) -> Result<Vec<(i64, i64)>, String> {
    let weights: Vec<f64> = apartments
        .iter()
        .map(|apt| match split_basis {
            "occupants" => apt.occupant_count as f64,
            _ => apt.m2_percentage,
        })
        .collect();

    let total_weight: f64 = weights.iter().sum();
    if total_weight <= 0.0 {
        return Err(match split_basis {
            "occupants" => "Total occupant count is zero.".to_string(),
            _ => "Total active apartment m2 percentage must be greater than zero.".to_string(),
        });
    }

    let mut remaining = bill_amount;
    let mut splits = Vec::with_capacity(apartments.len());

    for (index, apt) in apartments.iter().enumerate() {
        let share = if index == apartments.len() - 1 {
            remaining
        } else {
            let s = ((bill_amount as f64 * weights[index]) / total_weight).round() as i64;
            remaining -= s;
            s
        };
        splits.push((apt.id, share));
    }

    Ok(splits)
}

#[tauri::command]
pub fn calculate_splits(
    db: State<DbState>,
    billing_period_id: i64,
) -> Result<Vec<SplitRow>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare(
            "SELECT id, label, unit_code, occupant_count, m2_percentage
             FROM apartments
             WHERE building_id=1 AND is_active=1
             ORDER BY label",
        )
        .map_err(|e| e.to_string())?;
    let apartments: Vec<ApartmentWeight> = stmt
        .query_map([], |r| {
            Ok(ApartmentWeight {
                id: r.get(0)?,
                label: r.get(1)?,
                unit_code: r.get(2)?,
                occupant_count: r.get(3)?,
                m2_percentage: r.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    if apartments.is_empty() {
        return Err("No active apartments configured.".to_string());
    }

    let mut stmt = conn
        .prepare(
            "SELECT b.id, b.amount_cents, b.source_filename, p.name,
             COALESCE(p.split_basis, 'm2_percentage')
             FROM bills b
             LEFT JOIN providers p ON b.provider_id = p.id
             WHERE b.billing_period_id = ?1
             ORDER BY b.id",
        )
        .map_err(|e| e.to_string())?;
    let bills: Vec<(i64, i64, String, Option<String>, String)> = stmt
        .query_map([billing_period_id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    if bills.is_empty() {
        return Err("No bills in this billing period.".to_string());
    }

    let mut result_rows = Vec::new();

    for (bill_id, bill_amount, source_filename, provider_name, split_basis) in &bills {
        conn.execute("DELETE FROM bill_splits WHERE bill_id=?1", [bill_id])
            .map_err(|e| e.to_string())?;

        let normalized_basis = if split_basis == "occupants" {
            "occupants"
        } else {
            "m2_percentage"
        };
        let splits = calculate_weighted_shares(*bill_amount, &apartments, normalized_basis)?;

        for (apt_id, share) in &splits {
            conn.execute(
                "INSERT OR REPLACE INTO bill_splits (bill_id, apartment_id, amount_cents)
                 VALUES (?1, ?2, ?3)",
                params![bill_id, apt_id, share],
            )
            .map_err(|e| e.to_string())?;
            let split_id = conn.last_insert_rowid();

            let apt = apartments.iter().find(|apt| apt.id == *apt_id).unwrap();
            result_rows.push(SplitRow {
                split_id: Some(split_id),
                bill_id: *bill_id,
                apartment_id: *apt_id,
                apartment_label: apt.label.clone(),
                apartment_unit_code: apt.unit_code.clone(),
                bill_source_filename: source_filename.clone(),
                provider_name: provider_name.clone(),
                bill_amount_cents: *bill_amount,
                split_amount_cents: *share,
                occupant_count: apt.occupant_count,
                m2_percentage: apt.m2_percentage,
                split_basis: normalized_basis.to_string(),
            });
        }
    }

    Ok(result_rows)
}

#[tauri::command]
pub fn get_splits(
    db: State<DbState>,
    billing_period_id: i64,
) -> Result<Vec<SplitRow>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT bs.id, bs.bill_id, bs.apartment_id, a.label, a.unit_code,
             b.source_filename, p.name, b.amount_cents, bs.amount_cents,
             a.occupant_count, a.m2_percentage, COALESCE(p.split_basis, 'm2_percentage')
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
                apartment_unit_code: r.get(4)?,
                bill_source_filename: r.get(5)?,
                provider_name: r.get(6)?,
                bill_amount_cents: r.get(7)?,
                split_amount_cents: r.get(8)?,
                occupant_count: r.get(9)?,
                m2_percentage: r.get(10)?,
                split_basis: r.get(11)?,
            })
        })
        .map_err(|e| e.to_string())?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

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
