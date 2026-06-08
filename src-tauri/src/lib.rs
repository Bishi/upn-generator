mod commands;
mod db;

use commands::backup::{create_db_backup, restore_db_backup};
use commands::bills::{
    create_billing_period, create_year_periods, delete_bill, delete_billing_period,
    get_billing_periods, get_bills, import_bill, import_bills, save_bill,
};
use commands::config::{
    delete_apartment, delete_provider, get_apartments, get_building, get_providers,
    get_smtp_config, reset_all_data, save_apartment, save_building, save_provider,
    save_smtp_config, DbState,
};
use commands::splits::{calculate_splits, get_splits, save_split};
use commands::upn::{
    generate_upn_pdf, get_smtp_password, open_preview_apartment_upns, open_preview_upn,
    preview_upn, save_all_upns, save_smtp_password, send_emails,
};
use db::migrations;
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::Mutex;

fn db_path() -> PathBuf {
    if let Some(path) = std::env::var_os("UPN_GENERATOR_DB_PATH") {
        return PathBuf::from(path);
    }

    dirs_next::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("si.upn-generator")
        .join("upn-generator.db")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let db_path = db_path();
    if let Some(app_dir) = db_path.parent() {
        std::fs::create_dir_all(app_dir).expect("Failed to create app data directory");
    }
    println!("Using UPN Generator DB: {}", db_path.display());

    let conn = Connection::open(&db_path).expect("Failed to open database");
    migrations::run_migrations(&conn).expect("Failed to run migrations");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(DbState(Mutex::new(conn)))
        .invoke_handler(tauri::generate_handler![
            // Backup
            create_db_backup,
            restore_db_backup,
            // Config
            get_building,
            save_building,
            get_apartments,
            save_apartment,
            delete_apartment,
            get_providers,
            save_provider,
            delete_provider,
            get_smtp_config,
            save_smtp_config,
            reset_all_data,
            // Bills
            get_billing_periods,
            create_billing_period,
            create_year_periods,
            delete_billing_period,
            import_bill,
            import_bills,
            get_bills,
            save_bill,
            delete_bill,
            // Splits
            calculate_splits,
            get_splits,
            save_split,
            // UPN
            generate_upn_pdf,
            preview_upn,
            open_preview_upn,
            open_preview_apartment_upns,
            save_all_upns,
            send_emails,
            save_smtp_password,
            get_smtp_password,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
