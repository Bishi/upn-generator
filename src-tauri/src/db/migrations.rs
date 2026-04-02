use rusqlite::{params, Connection};

const DEFAULT_BUILDING_NAME: &str = "Skupnost stanovalcev Kamniska 36";
const DEFAULT_BUILDING_ADDRESS: &str = "Kamniska ulica 36";
const DEFAULT_BUILDING_CITY: &str = "Ljubljana";
const DEFAULT_BUILDING_POSTAL_CODE: &str = "1000";

type ApartmentSeed<'a> = (&'a str, &'a str, i32, &'a str, &'a str, &'a str, &'a str, &'a str, f64, bool);
type ProviderSeed<'a> = (
    &'a str,
    &'a str,
    &'a str,
    &'a str,
    &'a str,
    &'a str,
    &'a str,
    &'a str,
    &'a str,
    &'a str,
    &'a str,
    &'a str,
    &'a str,
    &'a str,
    &'a str,
);

const APARTMENT_SEEDS: [ApartmentSeed<'static>; 6] = [
    (
        "Andreja Vidonja",
        "1287/6",
        1,
        "amadeja.vidonja@gmail.com",
        "Andreja Vidonja",
        "Kamniska ulica 36",
        "Ljubljana",
        "1000",
        3.78,
        true,
    ),
    (
        "Mrvar Jernej\\Dusan",
        "1287/1",
        1,
        "ikiasdu@gmail.com,jernej.mrvar@gmail.com",
        "Mrvar Jernej\\Dusan",
        "Kamniska ulica 36",
        "Ljubljana",
        "1000",
        8.58,
        true,
    ),
    (
        "Stojic Milutin\\Goran",
        "1287/2",
        1,
        "goran.stojic@gmail.com",
        "Stojic Milutin\\Goran",
        "Kamniska ulica 36",
        "Ljubljana",
        "1000",
        9.65,
        true,
    ),
    (
        "Gabrijel Ales, Ines Hikl",
        "1287/3",
        3,
        "ales.gabrijel@gmail.com",
        "Gabrijel Ales, Ines Hikl",
        "Kamniska ulica 36",
        "Ljubljana",
        "1000",
        25.02,
        true,
    ),
    (
        "Risto Pecev, Tjasa Rant",
        "1287/4",
        4,
        "tjasarant@gmail.com,risto@artbread.si",
        "Risto Pecev, Tjasa Rant",
        "Kamniska ulica 36",
        "Ljubljana",
        "1000",
        25.02,
        true,
    ),
    (
        "Godnic Luka",
        "1287/5",
        2,
        "lgodnic@gmail.com",
        "Godnic Luka",
        "Kamniska ulica 36",
        "Ljubljana",
        "1000",
        27.95,
        true,
    ),
];

const PROVIDER_SEEDS: [ProviderSeed<'static>; 5] = [
    (
        "Elektro energija d.o.o.",
        "Electricity",
        "Elektro energija d.o.o.",
        "Dunajska cesta 119",
        "Ljubljana",
        "1000",
        "SI56 0400 1004 8988 093",
        "ENRG",
        "Elektro energija",
        "ZA PLA.+?([\\d.,]+)\\s*.",
        "(SI\\d{2}\\s+[\\d\\s]+)",
        "Rok pla.ila:\\s*(\\d{2}\\.\\s?\\d{2}\\.\\s?\\d{4})",
        "Ra.un.*?([A-Z0-9\\-]+)",
        "rn. {invoice} ({month}-{year})",
        "m2_percentage",
    ),
    (
        "JP VOKA SNAGA d.o.o.",
        "VO-KA komunalne storitve",
        "JP VOKA SNAGA d.o.o.",
        "Vodovodna cesta 90",
        "Ljubljana",
        "1000",
        "SI56 0400 1004 9142 226",
        "SCVE",
        "VOKA SNAGA.*(?:MKO|BIO|odpad)",
        "ZA PLA.+?([\\d.,]+)\\s*.",
        "(SI\\d{2}\\s+[\\d\\s]+)",
        "Rok pla.ila:\\s*(\\d{2}\\.\\s?\\d{2}\\.\\s?\\d{4})",
        "",
        "Komunalne stor. {invoice} ({month}-{year})",
        "occupants",
    ),
    (
        "Energetika Ljubljana d.o.o.",
        "Gas/Heating",
        "Energetika Ljubljana d.o.o.",
        "Verovskova ulica 62",
        "Ljubljana",
        "1000",
        "SI56 0292 4025 3764 022",
        "ENRG",
        "Energetika Ljubljana",
        "ZA PLA.+?([\\d.,]+)\\s*.",
        "(SI\\d{2}\\s+[\\d\\s]+)",
        "Rok pla.ila:\\s*(\\d{2}\\.\\s?\\d{2}\\.\\s?\\d{4})",
        "",
        "rn. {invoice} ({month}-{year})",
        "m2_percentage",
    ),
    (
        "ZLM d.o.o.",
        "Cleaning",
        "ZLM d.o.o.",
        "",
        "Ljubljana",
        "1000",
        "SI56 0201 1025 7890 131",
        "OTHR",
        "ZLM",
        "ZA PLA.+?([\\d.,]+)\\s*.",
        "(SI\\d{2}\\s+[\\d\\s]+)",
        "Rok pla.ila:\\s*(\\d{2}\\.\\s?\\d{2}\\.\\s?\\d{4})",
        "RN\\.\\s*([A-Z0-9\\-]+)",
        "RN. {invoice} ({month}-{year})",
        "m2_percentage",
    ),
    (
        "JP VOKA SNAGA d.o.o.",
        "Water/Sewage",
        "JP VOKA SNAGA d.o.o.",
        "Vodovodna cesta 90",
        "Ljubljana",
        "1000",
        "SI56 2900 0000 3057 588",
        "WTER",
        "VOKA SNAGA.*(?:vod|kanal)",
        "ZA PLA.+?([\\d.,]+)\\s*.",
        "(SI\\d{2}\\s+[\\d\\s]+)",
        "Rok pla.ila:\\s*(\\d{2}\\.\\s?\\d{2}\\.\\s?\\d{4})",
        "",
        "Komunalne stor. {invoice} ({month}-{year})",
        "m2_percentage",
    ),
];

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

        INSERT OR IGNORE INTO building (id, name, address, city, postal_code)
        VALUES (1, '', '', '', '');

        CREATE TABLE IF NOT EXISTS apartments (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            building_id INTEGER NOT NULL REFERENCES building(id),
            label TEXT NOT NULL,
            unit_code TEXT NOT NULL DEFAULT '',
            occupant_count INTEGER NOT NULL DEFAULT 1,
            contact_email TEXT NOT NULL DEFAULT '',
            payer_name TEXT NOT NULL DEFAULT '',
            payer_address TEXT NOT NULL DEFAULT '',
            payer_city TEXT NOT NULL DEFAULT '',
            payer_postal_code TEXT NOT NULL DEFAULT '',
            m2_percentage REAL NOT NULL DEFAULT 0,
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
            purpose_text_template TEXT NOT NULL DEFAULT '',
            split_basis TEXT NOT NULL DEFAULT 'm2_percentage'
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

    let _ = conn.execute(
        "ALTER TABLE apartments ADD COLUMN unit_code TEXT NOT NULL DEFAULT ''",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE apartments ADD COLUMN m2_percentage REAL NOT NULL DEFAULT 0",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE providers ADD COLUMN split_basis TEXT NOT NULL DEFAULT 'm2_percentage'",
        [],
    );
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

    let _ = conn.execute(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_provider_name_iban ON providers(name, creditor_iban)",
        [],
    );
    let _ = conn.execute(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_apartment_label ON apartments(building_id, label)",
        [],
    );

    let building_name: String = conn
        .query_row("SELECT name FROM building WHERE id=1", [], |row| row.get(0))
        .unwrap_or_default();

    if building_name.trim().is_empty() {
        reset_to_defaults(conn)?;
    } else {
        let _ = conn.execute(
            "UPDATE providers SET split_basis='m2_percentage' WHERE split_basis=''",
            [],
        );
        let _ = conn.execute(
            "UPDATE providers SET split_basis='m2_percentage'
             WHERE split_basis NOT IN ('m2_percentage', 'occupants', 'equal_apartments')",
            [],
        );
        let _ = conn.execute(
            "UPDATE providers SET split_basis='occupants'
             WHERE creditor_iban='SI56 0400 1004 9142 226'",
            [],
        );
    }

    Ok(())
}

pub fn reset_to_defaults(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "
        DELETE FROM bill_splits;
        DELETE FROM bills;
        DELETE FROM billing_periods;
        DELETE FROM apartments;
        DELETE FROM providers;
        UPDATE building SET name='', address='', city='', postal_code='' WHERE id=1;
        UPDATE smtp_config
        SET host='', port=587, username='', from_email='', use_tls=1, password=''
        WHERE id=1;
        ",
    )
    .map_err(|e| e.to_string())?;

    seed_defaults(conn)
}

fn seed_defaults(conn: &Connection) -> Result<(), String> {
    conn.execute(
        "UPDATE building SET name=?1, address=?2, city=?3, postal_code=?4 WHERE id=1",
        params![
            DEFAULT_BUILDING_NAME,
            DEFAULT_BUILDING_ADDRESS,
            DEFAULT_BUILDING_CITY,
            DEFAULT_BUILDING_POSTAL_CODE
        ],
    )
    .map_err(|e| e.to_string())?;

    for provider in PROVIDER_SEEDS {
        conn.execute(
            "INSERT INTO providers (
                name, service_type, creditor_name, creditor_address, creditor_city,
                creditor_postal_code, creditor_iban, purpose_code, match_pattern,
                amount_pattern, reference_pattern, due_date_pattern,
                invoice_number_pattern, purpose_text_template, split_basis
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                provider.0,
                provider.1,
                provider.2,
                provider.3,
                provider.4,
                provider.5,
                provider.6,
                provider.7,
                provider.8,
                provider.9,
                provider.10,
                provider.11,
                provider.12,
                provider.13,
                provider.14,
            ],
        )
        .map_err(|e| e.to_string())?;
    }

    for apartment in APARTMENT_SEEDS {
        conn.execute(
            "INSERT INTO apartments (
                building_id, label, unit_code, occupant_count, contact_email,
                payer_name, payer_address, payer_city, payer_postal_code,
                m2_percentage, is_active
            ) VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                apartment.0,
                apartment.1,
                apartment.2,
                apartment.3,
                apartment.4,
                apartment.5,
                apartment.6,
                apartment.7,
                apartment.8,
                if apartment.9 { 1 } else { 0 }
            ],
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(())
}
