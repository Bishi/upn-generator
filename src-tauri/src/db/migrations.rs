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
    .map_err(|e| e.to_string())?;

    // Unique indexes for idempotent seed data
    let _ = conn.execute(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_provider_name_iban ON providers(name, creditor_iban)",
        [],
    );
    let _ = conn.execute(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_apartment_label ON apartments(building_id, label)",
        [],
    );

    // Seed building data (only on fresh DB where name is empty)
    let _ = conn.execute(
        "UPDATE building SET name='Skupnost stanovalcev Kamniška 36', address='Kamniška ulica 36', city='Ljubljana', postal_code='1000' WHERE id=1 AND name=''",
        [],
    );

    // Seed providers
    let _ = conn.execute_batch(
        "
        INSERT OR IGNORE INTO providers (name, service_type, creditor_name, creditor_address, creditor_city, creditor_postal_code, creditor_iban, purpose_code, match_pattern, amount_pattern, reference_pattern, due_date_pattern, invoice_number_pattern, purpose_text_template)
        VALUES ('Elektro energija d.o.o.', 'Electricity', 'Elektro energija d.o.o.', 'Dunajska cesta 119', 'Ljubljana', '1000', 'SI56 0400 1004 8988 093', 'ENRG', 'Elektro energija', 'ZA PLA.+?([\\d.,]+)\\s*.', '(SI\\d{2}\\s+[\\d\\s]+)', 'Rok pla.ila:\\s*(\\d{2}\\.\\s?\\d{2}\\.\\s?\\d{4})', 'Ra.un.*?([A-Z0-9\\-]+)', 'rn. {invoice} ({month}-{year})');

        INSERT OR IGNORE INTO providers (name, service_type, creditor_name, creditor_address, creditor_city, creditor_postal_code, creditor_iban, purpose_code, match_pattern, amount_pattern, reference_pattern, due_date_pattern, invoice_number_pattern, purpose_text_template)
        VALUES ('JP VOKA SNAGA d.o.o.', 'Waste (MKO/BIO)', 'JP VOKA SNAGA d.o.o.', 'Vodovodna cesta 90', 'Ljubljana', '1000', 'SI56 0400 1004 9142 226', 'SCVE', 'VOKA SNAGA.*(?:MKO|BIO|odpad)', 'ZA PLA.+?([\\d.,]+)\\s*.', '(SI\\d{2}\\s+[\\d\\s]+)', 'Rok pla.ila:\\s*(\\d{2}\\.\\s?\\d{2}\\.\\s?\\d{4})', '', 'Komunalne stor. {invoice} ({month}-{year})');

        INSERT OR IGNORE INTO providers (name, service_type, creditor_name, creditor_address, creditor_city, creditor_postal_code, creditor_iban, purpose_code, match_pattern, amount_pattern, reference_pattern, due_date_pattern, invoice_number_pattern, purpose_text_template)
        VALUES ('Energetika Ljubljana d.o.o.', 'Gas/Heating', 'Energetika Ljubljana d.o.o.', 'Verovškova ulica 62', 'Ljubljana', '1000', 'SI56 0292 4025 3764 022', 'ENRG', 'Energetika Ljubljana', 'ZA PLA.+?([\\d.,]+)\\s*.', '(SI\\d{2}\\s+[\\d\\s]+)', 'Rok pla.ila:\\s*(\\d{2}\\.\\s?\\d{2}\\.\\s?\\d{4})', '', 'rn. {invoice} ({month}-{year})');

        INSERT OR IGNORE INTO providers (name, service_type, creditor_name, creditor_address, creditor_city, creditor_postal_code, creditor_iban, purpose_code, match_pattern, amount_pattern, reference_pattern, due_date_pattern, invoice_number_pattern, purpose_text_template)
        VALUES ('ZLM d.o.o.', 'Cleaning', 'ZLM d.o.o.', '', 'Ljubljana', '1000', 'SI56 0201 1025 7890 131', 'OTHR', 'ZLM', 'ZA PLA.+?([\\d.,]+)\\s*.', '(SI\\d{2}\\s+[\\d\\s]+)', 'Rok pla.ila:\\s*(\\d{2}\\.\\s?\\d{2}\\.\\s?\\d{4})', 'RN\\.\\s*([A-Z0-9\\-]+)', 'RN. {invoice} ({month}-{year})');

        INSERT OR IGNORE INTO providers (name, service_type, creditor_name, creditor_address, creditor_city, creditor_postal_code, creditor_iban, purpose_code, match_pattern, amount_pattern, reference_pattern, due_date_pattern, invoice_number_pattern, purpose_text_template)
        VALUES ('JP VOKA SNAGA d.o.o.', 'Water/Sewage', 'JP VOKA SNAGA d.o.o.', 'Vodovodna cesta 90', 'Ljubljana', '1000', 'SI56 2900 0000 3057 588', 'WTER', 'VOKA SNAGA.*(?:vod|kanal)', 'ZA PLA.+?([\\d.,]+)\\s*.', '(SI\\d{2}\\s+[\\d\\s]+)', 'Rok pla.ila:\\s*(\\d{2}\\.\\s?\\d{2}\\.\\s?\\d{4})', '', 'Komunalne stor. {invoice} ({month}-{year})');
        ",
    );

    // Seed apartments (5 apartments, 12 total occupants)
    let _ = conn.execute_batch(
        "
        INSERT OR IGNORE INTO apartments (building_id, label, occupant_count, contact_email, payer_name, payer_address, payer_city, payer_postal_code, is_active)
        VALUES (1, 'Stanovanje 1', 3, '', 'Stanovalec 1', 'Kamniška ulica 36', 'Ljubljana', '1000', 1);

        INSERT OR IGNORE INTO apartments (building_id, label, occupant_count, contact_email, payer_name, payer_address, payer_city, payer_postal_code, is_active)
        VALUES (1, 'Stanovanje 2', 2, '', 'Stanovalec 2', 'Kamniška ulica 36', 'Ljubljana', '1000', 1);

        INSERT OR IGNORE INTO apartments (building_id, label, occupant_count, contact_email, payer_name, payer_address, payer_city, payer_postal_code, is_active)
        VALUES (1, 'Stanovanje 3', 3, '', 'Stanovalec 3', 'Kamniška ulica 36', 'Ljubljana', '1000', 1);

        INSERT OR IGNORE INTO apartments (building_id, label, occupant_count, contact_email, payer_name, payer_address, payer_city, payer_postal_code, is_active)
        VALUES (1, 'Stanovanje 4', 2, '', 'Stanovalec 4', 'Kamniška ulica 36', 'Ljubljana', '1000', 1);

        INSERT OR IGNORE INTO apartments (building_id, label, occupant_count, contact_email, payer_name, payer_address, payer_city, payer_postal_code, is_active)
        VALUES (1, 'Stanovanje 5', 2, '', 'Stanovalec 5', 'Kamniška ulica 36', 'Ljubljana', '1000', 1);
        ",
    );

    // Additive migrations — silently ignored if column already exists
    let _ = conn.execute(
        "ALTER TABLE smtp_config ADD COLUMN password TEXT NOT NULL DEFAULT ''",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE bills ADD COLUMN creditor_address TEXT NOT NULL DEFAULT ''",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE bills ADD COLUMN creditor_city TEXT NOT NULL DEFAULT ''",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE bills ADD COLUMN creditor_postal_code TEXT NOT NULL DEFAULT ''",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE bills ADD COLUMN invoice_number TEXT NOT NULL DEFAULT ''",
        [],
    );

    Ok(())
}
