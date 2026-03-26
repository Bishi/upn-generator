use rusqlite::Connection;

pub fn run_migrations(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS building (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL DEFAULT '',
            address TEXT NOT NULL DEFAULT '',
            city TEXT NOT NULL DEFAULT '',
            postal_code TEXT NOT NULL DEFAULT ''
        );

        -- Ensure there is always exactly one building row
        INSERT OR IGNORE INTO building (id, name, address, city, postal_code)
        VALUES (1, '', '', '', '');

        CREATE TABLE IF NOT EXISTS apartments (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            building_id INTEGER NOT NULL REFERENCES building(id),
            label TEXT NOT NULL,
            occupant_count INTEGER NOT NULL DEFAULT 1,
            contact_email TEXT NOT NULL DEFAULT '',
            payer_name TEXT NOT NULL DEFAULT '',
            payer_address TEXT NOT NULL DEFAULT '',
            payer_city TEXT NOT NULL DEFAULT '',
            payer_postal_code TEXT NOT NULL DEFAULT '',
            is_active INTEGER NOT NULL DEFAULT 1
        );

        CREATE TABLE IF NOT EXISTS providers (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            service_type TEXT NOT NULL DEFAULT '',
            creditor_name TEXT NOT NULL DEFAULT '',
            creditor_address TEXT NOT NULL DEFAULT '',
            creditor_city TEXT NOT NULL DEFAULT '',
            creditor_postal_code TEXT NOT NULL DEFAULT '',
            creditor_iban TEXT NOT NULL DEFAULT '',
            purpose_code TEXT NOT NULL DEFAULT 'OTHR',
            match_pattern TEXT NOT NULL DEFAULT '',
            amount_pattern TEXT NOT NULL DEFAULT '',
            reference_pattern TEXT NOT NULL DEFAULT '',
            due_date_pattern TEXT NOT NULL DEFAULT '',
            invoice_number_pattern TEXT NOT NULL DEFAULT '',
            purpose_text_template TEXT NOT NULL DEFAULT ''
        );

        CREATE TABLE IF NOT EXISTS billing_periods (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            building_id INTEGER NOT NULL REFERENCES building(id),
            month INTEGER NOT NULL,
            year INTEGER NOT NULL,
            status TEXT NOT NULL DEFAULT 'draft',
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE(building_id, month, year)
        );

        CREATE TABLE IF NOT EXISTS bills (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            billing_period_id INTEGER NOT NULL REFERENCES billing_periods(id),
            provider_id INTEGER REFERENCES providers(id),
            raw_text TEXT NOT NULL DEFAULT '',
            amount_cents INTEGER NOT NULL DEFAULT 0,
            creditor_name TEXT NOT NULL DEFAULT '',
            creditor_iban TEXT NOT NULL DEFAULT '',
            reference TEXT NOT NULL DEFAULT '',
            due_date TEXT NOT NULL DEFAULT '',
            purpose_code TEXT NOT NULL DEFAULT 'OTHR',
            purpose_text TEXT NOT NULL DEFAULT '',
            status TEXT NOT NULL DEFAULT 'draft',
            source_filename TEXT NOT NULL DEFAULT ''
        );

        CREATE TABLE IF NOT EXISTS bill_splits (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            bill_id INTEGER NOT NULL REFERENCES bills(id),
            apartment_id INTEGER NOT NULL REFERENCES apartments(id),
            amount_cents INTEGER NOT NULL DEFAULT 0,
            UNIQUE(bill_id, apartment_id)
        );

        CREATE TABLE IF NOT EXISTS smtp_config (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            host TEXT NOT NULL DEFAULT '',
            port INTEGER NOT NULL DEFAULT 587,
            username TEXT NOT NULL DEFAULT '',
            from_email TEXT NOT NULL DEFAULT '',
            use_tls INTEGER NOT NULL DEFAULT 1
        );

        INSERT OR IGNORE INTO smtp_config (id) VALUES (1);
        ",
    )
    .map_err(|e| e.to_string())
}
