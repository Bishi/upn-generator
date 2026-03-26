mod commands;
mod db;

use commands::config::{
    delete_apartment, delete_provider, get_apartments, get_building, get_providers,
    get_smtp_config, save_apartment, save_building, save_provider, save_smtp_config, DbState,
};
use db::migrations;
use rusqlite::Connection;
use std::sync::Mutex;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_dir = dirs_next::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("si.upn-generator");
    std::fs::create_dir_all(&app_dir).expect("Failed to create app data directory");
    let db_path = app_dir.join("upn-generator.db");

    let conn = Connection::open(&db_path).expect("Failed to open database");
    migrations::run_migrations(&conn).expect("Failed to run migrations");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(DbState(Mutex::new(conn)))
        .invoke_handler(tauri::generate_handler![
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
