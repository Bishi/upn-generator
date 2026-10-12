use super::config::DbState;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tauri::State;

pub const DEFAULT_SUBJECT: &str = "Položnice za {month}/{year}";
pub const DEFAULT_BODY: &str = "Spoštovani,\n\nv priponki najdete UPN položnice za {month}/{year}.{originals_note}\n\nLep pozdrav";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailContent {
    pub subject: String,
    pub body: String,
}

pub fn validate(content: &EmailContent) -> Result<(), String> {
    if content.subject.trim().is_empty() || content.subject.contains(['\r', '\n']) {
        return Err("Email subject must be nonempty and on one line.".into());
    }
    if content.subject.len() > 998
        || content.body.len() > 100_000
        || content.subject.contains('\0')
        || content.body.contains('\0')
    {
        return Err("Email content is too long or contains invalid characters.".into());
    }
    Ok(())
}

pub fn load(conn: &Connection) -> Result<EmailContent, String> {
    conn.query_row(
        "SELECT email_subject, email_body FROM app_settings WHERE id=1",
        [],
        |r| {
            Ok(EmailContent {
                subject: r.get(0)?,
                body: r.get(1)?,
            })
        },
    )
    .map_err(|e| e.to_string())
}

pub fn render(template: &EmailContent, month: i32, year: i32, originals: bool) -> EmailContent {
    let (month, year) = if month == 1 {
        (12, year - 1)
    } else {
        (month - 1, year)
    };
    let replace = |text: &str| {
        text.replace("{month}", &format!("{month:02}"))
            .replace("{year}", &year.to_string())
            .replace(
                "{originals_note}",
                if originals {
                    "\n\nPriloženi so tudi razpoložljivi izvirni računi dobaviteljev."
                } else {
                    ""
                },
            )
    };
    EmailContent {
        subject: replace(&template.subject),
        body: replace(&template.body),
    }
}

#[tauri::command]
pub fn get_email_content(db: State<DbState>) -> Result<EmailContent, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    load(&conn)
}

#[tauri::command]
pub fn save_email_content(
    db: State<DbState>,
    content: EmailContent,
) -> Result<EmailContent, String> {
    validate(&content)?;
    db.0.lock()
        .map_err(|e| e.to_string())?
        .execute(
            "UPDATE app_settings SET email_subject=?1, email_body=?2 WHERE id=1",
            rusqlite::params![content.subject, content.body],
        )
        .map_err(|e| e.to_string())?;
    Ok(content)
}

#[tauri::command]
pub fn preview_email_content(
    db: State<DbState>,
    billing_period_id: i64,
    include_original_bills: bool,
) -> Result<EmailContent, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let (month, year) = conn
        .query_row(
            "SELECT month, year FROM billing_periods WHERE id=?1",
            [billing_period_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|e| e.to_string())?;
    let originals = include_original_bills && conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM bills b JOIN bill_splits s ON s.bill_id=b.id JOIN apartments a ON a.id=s.apartment_id WHERE b.billing_period_id=?1 AND b.source_document_id IS NOT NULL AND trim(a.contact_email) != '')",
        [billing_period_id], |r| r.get::<_, bool>(0),
    ).map_err(|e| e.to_string())?;
    let content = render(&load(&conn)?, month, year, originals);
    validate(&content)?;
    Ok(content)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn templates_render_service_month_and_optional_originals() {
        let t = EmailContent {
            subject: DEFAULT_SUBJECT.into(),
            body: DEFAULT_BODY.into(),
        };
        let rendered = render(&t, 1, 2027, false);
        assert_eq!(rendered.subject, "Položnice za 12/2026");
        assert!(!rendered.body.contains('{'));
        assert!(!rendered.body.contains("izvirni"));
        assert!(render(&t, 10, 2026, true)
            .body
            .contains("razpoložljivi izvirni"));
    }
    #[test]
    fn legacy_settings_migrate_without_overwriting_saved_content() {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::migrations::run_migrations(&conn).unwrap();
        conn.execute_batch("UPDATE app_settings SET theme='crisp' WHERE id=1; ALTER TABLE app_settings DROP COLUMN email_subject; ALTER TABLE app_settings DROP COLUMN email_body;").unwrap();
        crate::db::migrations::run_migrations(&conn).unwrap();
        assert_eq!(load(&conn).unwrap().body, DEFAULT_BODY);
        conn.execute(
            "UPDATE app_settings SET email_subject='My subject', email_body=''",
            [],
        )
        .unwrap();
        crate::db::migrations::run_migrations(&conn).unwrap();
        assert_eq!(load(&conn).unwrap().subject, "My subject");
        assert!(load(&conn).unwrap().body.is_empty());
        let theme: String = conn
            .query_row("SELECT theme FROM app_settings WHERE id=1", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(theme, "crisp");
        conn.execute(
            "UPDATE app_settings SET email_subject='Poloznice za {month}/{year}' WHERE id=1",
            [],
        )
        .unwrap();
        crate::db::migrations::run_migrations(&conn).unwrap();
        assert_eq!(load(&conn).unwrap().subject, DEFAULT_SUBJECT);
        assert!(load(&conn).unwrap().body.is_empty());
    }

    #[test]
    fn subject_validation_and_empty_body() {
        for subject in ["", "  ", "hello\rBCC: x", "hello\nworld", "bad\0"] {
            assert!(validate(&EmailContent {
                subject: subject.into(),
                body: String::new()
            })
            .is_err());
        }
        assert!(validate(&EmailContent {
            subject: "Custom subject".into(),
            body: String::new()
        })
        .is_ok());
    }
}
