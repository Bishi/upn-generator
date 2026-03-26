use base64::Engine;
use lettre::message::header::ContentType;
use lettre::message::{Attachment, MultiPart, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};
use printpdf::*;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::io::BufWriter;
use tauri::State;

use super::config::DbState;

// ─── Structs ───────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EmailResult {
    pub apartment_label: String,
    pub email: String,
    pub success: bool,
    pub error: Option<String>,
}

struct UpnData {
    payer_name: String,
    payer_address: String,
    payer_city: String,
    payer_postal_code: String,
    amount_cents: i64,
    purpose_code: String,
    purpose_text: String,
    due_date: String,
    creditor_iban: String,
    creditor_reference: String,
    creditor_name: String,
    creditor_address: String,
    creditor_city: String,
}

// ─── UPN PDF helpers ────────────────────────────────────────────────────────

fn format_amount(cents: i64) -> String {
    let euros = cents / 100;
    let c = (cents % 100).unsigned_abs();
    format!("{}.{:02}", euros, c)
}

fn format_iban(iban: &str) -> String {
    iban.chars()
        .collect::<Vec<_>>()
        .chunks(4)
        .map(|c| c.iter().collect::<String>())
        .collect::<Vec<_>>()
        .join(" ")
}

fn wrap_text(text: &str, max_chars: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        if current.is_empty() {
            current = word.to_string();
        } else if current.len() + 1 + word.len() <= max_chars {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(current.clone());
            current = word.to_string();
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

/// Draw a stroked line segment (open path).
fn hline(layer: &PdfLayerReference, x1: f32, y: f32, x2: f32) {
    let l = Line {
        points: vec![
            (Point::new(Mm(x1), Mm(y)), false),
            (Point::new(Mm(x2), Mm(y)), false),
        ],
        is_closed: false,
    };
    layer.add_line(l);
}

fn vline(layer: &PdfLayerReference, x: f32, y1: f32, y2: f32) {
    let l = Line {
        points: vec![
            (Point::new(Mm(x), Mm(y1)), false),
            (Point::new(Mm(x), Mm(y2)), false),
        ],
        is_closed: false,
    };
    layer.add_line(l);
}

/// Draw a stroked open line (any two points).
fn draw_line(layer: &PdfLayerReference, x1: f32, y1: f32, x2: f32, y2: f32) {
    let l = Line {
        points: vec![
            (Point::new(Mm(x1), Mm(y1)), false),
            (Point::new(Mm(x2), Mm(y2)), false),
        ],
        is_closed: false,
    };
    layer.add_line(l);
}

/// Draw a stroked rectangle.
fn draw_rect(layer: &PdfLayerReference, x: f32, y: f32, w: f32, h: f32) {
    let poly = Polygon {
        rings: vec![vec![
            (Point::new(Mm(x), Mm(y)), false),
            (Point::new(Mm(x + w), Mm(y)), false),
            (Point::new(Mm(x + w), Mm(y + h)), false),
            (Point::new(Mm(x), Mm(y + h)), false),
        ]],
        mode: PolygonMode::Stroke,
        winding_order: WindingOrder::NonZero,
    };
    layer.add_polygon(poly);
}

fn t(layer: &PdfLayerReference, text: &str, size: f32, x: f32, y: f32, font: &IndirectFontRef) {
    layer.use_text(text, size, Mm(x), Mm(y), font);
}

/// Load a TTF font from Windows system fonts for Slovenian character support.
fn load_system_font() -> Option<Vec<u8>> {
    let win_dir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".to_string());
    for name in &["arial.ttf", "calibri.ttf", "verdana.ttf", "tahoma.ttf"] {
        let path = format!("{}\\Fonts\\{}", win_dir, name);
        if let Ok(bytes) = std::fs::read(&path) {
            return Some(bytes);
        }
    }
    None
}

fn render_upn_pdf(data: &UpnData) -> Result<Vec<u8>, String> {
    let (doc, page1, layer1) =
        PdfDocument::new("Nalog za placilo", Mm(210.0), Mm(297.0), "Layer 1");
    let layer = doc.get_page(page1).get_layer(layer1);

    let (font, font_bold) = if let Some(font_bytes) = load_system_font() {
        let f = doc
            .add_external_font(std::io::Cursor::new(font_bytes.clone()))
            .map_err(|e| e.to_string())?;
        let fb = doc
            .add_external_font(std::io::Cursor::new(font_bytes))
            .map_err(|e| e.to_string())?;
        (f, fb)
    } else {
        let f = doc
            .add_builtin_font(BuiltinFont::Helvetica)
            .map_err(|e| e.to_string())?;
        let fb = doc
            .add_builtin_font(BuiltinFont::HelveticaBold)
            .map_err(|e| e.to_string())?;
        (f, fb)
    };

    // ─── Layout constants (all mm, f32) ────────────────────────────────
    let sl: f32 = 6.0; // slip left
    let sr: f32 = 204.0; // slip right
    let sb: f32 = 6.0; // slip bottom
    let st: f32 = 111.0; // slip top

    let c1: f32 = sl + 72.0; // col divider 1 (x≈78)
    let c2: f32 = sl + 132.0; // col divider 2 (x≈138)
    let hb: f32 = st - 7.0; // header bottom (y≈104)

    // Horizontal row dividers
    let hy1: f32 = hb - 31.0; // y≈73 — below payer name
    let hy2: f32 = hy1 - 16.0; // y≈57 — below IBAN plačnika
    let hy3: f32 = hy2 - 20.0; // y≈37 — barcode zone top

    // Right-column sub-dividers
    let ry1: f32 = hb - 13.0; // y≈91 — below IBAN prejemnika
    let ry2: f32 = ry1 - 11.0; // y≈80 — below BIC
    let ry3: f32 = ry2 - 13.0; // y≈67 — below referenca

    let my1: f32 = hb - 15.0; // y≈89 — below kod namena (middle col)

    // ─── Borders ────────────────────────────────────────────────────────
    layer.set_outline_thickness(0.3);
    draw_rect(&layer, sl, sb, sr - sl, st - sb);
    hline(&layer, sl, hb, sr);
    t(&layer, "NALOG ZA PLACILO / UPN", 8.0, sl + 2.0, hb + 2.0, &font_bold);
    t(&layer, "Univerzalni placilni nalog", 5.5, sl + 2.0, hb + 8.5, &font);

    // Column dividers
    vline(&layer, c1, sb, hb);
    vline(&layer, c2, sb, hb);

    // ─── Left column: Payer ─────────────────────────────────────────────
    let lx = sl + 1.5;
    t(&layer, "PLACNIK", 6.0, lx, hb - 4.0, &font_bold);
    t(&layer, &data.payer_name, 8.5, lx, hb - 10.5, &font);
    t(&layer, &data.payer_address, 8.0, lx, hb - 17.5, &font);
    t(
        &layer,
        &format!("{} {}", data.payer_postal_code, data.payer_city),
        8.0,
        lx,
        hb - 24.5,
        &font,
    );

    hline(&layer, sl, hy1, c1);
    t(&layer, "IBAN PLACNIKA", 6.0, lx, hy1 - 4.0, &font_bold);
    // Payer IBAN left blank for tenants

    hline(&layer, sl, hy2, c2); // spans left + middle
    t(&layer, "ROK PLACILA", 6.0, lx, hy2 - 4.0, &font_bold);
    t(&layer, &data.due_date, 9.0, lx, hy2 - 12.0, &font_bold);

    hline(&layer, sl, hy3, c2); // barcode zone separator
    t(&layer, "QR/OCR koda", 5.5, lx, sb + 3.5, &font);

    // ─── Middle column: Purpose + Amount ───────────────────────────────
    let mx = c1 + 1.5;
    t(&layer, "KOD NAMENA", 6.0, mx, hb - 4.0, &font_bold);
    t(&layer, &data.purpose_code, 8.5, mx, hb - 10.5, &font);

    hline(&layer, c1, my1, c2);
    t(&layer, "NAMEN PLACILA", 6.0, mx, my1 - 4.0, &font_bold);
    for (i, l) in wrap_text(&data.purpose_text, 26).iter().take(3).enumerate() {
        t(&layer, l, 8.0, mx, my1 - 11.0 - (i as f32 * 7.0), &font);
    }

    t(&layer, "ZNESEK EUR", 6.0, mx, hy2 - 4.0, &font_bold);
    t(
        &layer,
        &format_amount(data.amount_cents),
        10.0,
        mx,
        hy2 - 13.0,
        &font_bold,
    );

    // ─── Right column: Creditor ─────────────────────────────────────────
    let rx = c2 + 1.5;
    t(&layer, "IBAN PREJEMNIKA", 6.0, rx, hb - 4.0, &font_bold);
    t(
        &layer,
        &format_iban(&data.creditor_iban),
        8.0,
        rx,
        hb - 10.5,
        &font,
    );

    hline(&layer, c2, ry1, sr);
    t(&layer, "BIC BANKE PREJEMNIKA", 6.0, rx, ry1 - 4.0, &font_bold);

    hline(&layer, c2, ry2, sr);
    t(&layer, "REFERENCA PREJEMNIKA", 6.0, rx, ry2 - 4.0, &font_bold);
    t(&layer, &data.creditor_reference, 8.5, rx, ry2 - 12.0, &font);

    hline(&layer, c2, ry3, sr);
    t(&layer, "PREJEMNIK", 6.0, rx, ry3 - 4.0, &font_bold);
    t(&layer, &data.creditor_name, 8.5, rx, ry3 - 11.0, &font);
    t(&layer, &data.creditor_address, 8.0, rx, ry3 - 18.0, &font);
    t(&layer, &data.creditor_city, 8.0, rx, ry3 - 25.0, &font);

    // ─── Upper portion: payment notice ─────────────────────────────────
    let ny = st + 5.0;
    draw_line(&layer, sl, st + 3.0, sr, st + 3.0);
    t(&layer, "OBVESTILO O PLACILU", 11.0, sl, ny + 170.0, &font_bold);
    t(
        &layer,
        &format!("Prejemnik:  {}", data.creditor_name),
        9.0,
        sl,
        ny + 162.0,
        &font,
    );
    t(
        &layer,
        &format!("Znesek:  {} EUR", format_amount(data.amount_cents)),
        9.0,
        sl,
        ny + 154.0,
        &font,
    );
    t(
        &layer,
        &format!("Namen:  {}", data.purpose_text),
        9.0,
        sl,
        ny + 146.0,
        &font,
    );
    t(
        &layer,
        &format!("Rok placila:  {}", data.due_date),
        9.0,
        sl,
        ny + 138.0,
        &font,
    );

    // ─── Save to bytes ──────────────────────────────────────────────────
    let mut buf: Vec<u8> = Vec::new();
    doc.save(&mut BufWriter::new(std::io::Cursor::new(&mut buf)))
        .map_err(|e| e.to_string())?;
    Ok(buf)
}

// ─── DB helper ─────────────────────────────────────────────────────────────

fn load_upn_data(
    conn: &rusqlite::Connection,
    bill_id: i64,
    apartment_id: i64,
) -> Result<UpnData, String> {
    let split_amount: i64 = conn
        .query_row(
            "SELECT amount_cents FROM bill_splits WHERE bill_id=?1 AND apartment_id=?2",
            params![bill_id, apartment_id],
            |r| r.get(0),
        )
        .map_err(|e| format!("No split for bill {} apt {}: {}", bill_id, apartment_id, e))?;

    let (reference, due_date, purpose_code, purpose_text, creditor_name, creditor_iban, creditor_address, creditor_city):
        (String, String, String, String, String, String, String, String) = conn
        .query_row(
            "SELECT reference, due_date, purpose_code, purpose_text,
             creditor_name, creditor_iban, creditor_address, creditor_city
             FROM bills WHERE id=?1",
            [bill_id],
            |r| {
                Ok((
                    r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?,
                    r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?,
                ))
            },
        )
        .map_err(|e| e.to_string())?;

    let (payer_name, payer_address, payer_city, payer_postal_code): (
        String,
        String,
        String,
        String,
    ) = conn
        .query_row(
            "SELECT payer_name, payer_address, payer_city, payer_postal_code
             FROM apartments WHERE id=?1",
            [apartment_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .map_err(|e| e.to_string())?;

    Ok(UpnData {
        payer_name,
        payer_address,
        payer_city,
        payer_postal_code,
        amount_cents: split_amount,
        purpose_code,
        purpose_text,
        due_date,
        creditor_iban,
        creditor_reference: reference,
        creditor_name,
        creditor_address,
        creditor_city,
    })
}

fn query_vec<T, F>(
    conn: &rusqlite::Connection,
    sql: &str,
    params: &[&dyn rusqlite::types::ToSql],
    map: F,
) -> Result<Vec<T>, String>
where
    F: Fn(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
{
    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
    let rows: Vec<T> = stmt
        .query_map(params, map)
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

// ─── Commands ──────────────────────────────────────────────────────────────

#[tauri::command]
pub fn generate_upn_pdf(
    db: State<DbState>,
    bill_id: i64,
    apartment_id: i64,
) -> Result<String, String> {
    let data = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        load_upn_data(&conn, bill_id, apartment_id)?
    };
    let pdf_bytes = render_upn_pdf(&data)?;
    Ok(base64::engine::general_purpose::STANDARD.encode(&pdf_bytes))
}

#[tauri::command]
pub fn save_smtp_password(db: State<DbState>, password: String) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute("UPDATE smtp_config SET password=?1 WHERE id=1", [&password])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn get_smtp_password(db: State<DbState>) -> Result<String, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.query_row("SELECT password FROM smtp_config WHERE id=1", [], |r| {
        r.get(0)
    })
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn send_emails(
    db: State<DbState>,
    billing_period_id: i64,
) -> Result<Vec<EmailResult>, String> {
    let (smtp_host, smtp_port, smtp_user, smtp_from, use_tls, smtp_pass) = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        conn.query_row(
            "SELECT host, port, username, from_email, use_tls, password
             FROM smtp_config WHERE id=1",
            [],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i32>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, i32>(4)? != 0,
                    r.get::<_, String>(5)?,
                ))
            },
        )
        .map_err(|e| e.to_string())?
    };

    if smtp_host.is_empty() {
        return Err("SMTP host not configured.".to_string());
    }

    let (month, year) = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        conn.query_row(
            "SELECT month, year FROM billing_periods WHERE id=?1",
            [billing_period_id],
            |r| Ok((r.get::<_, i32>(0)?, r.get::<_, i32>(1)?)),
        )
        .map_err(|e| e.to_string())?
    };

    let apartments: Vec<(i64, String, String)> = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        query_vec(
            &conn,
            "SELECT DISTINCT a.id, a.label, a.contact_email
             FROM bill_splits bs
             JOIN bills b ON bs.bill_id = b.id
             JOIN apartments a ON bs.apartment_id = a.id
             WHERE b.billing_period_id = ?1 AND a.contact_email != ''
             ORDER BY a.label",
            &[&billing_period_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?
    };

    if apartments.is_empty() {
        return Err(
            "No apartments with email addresses have splits in this period.".to_string(),
        );
    }

    let creds = Credentials::new(smtp_user, smtp_pass);
    let mailer: SmtpTransport = if use_tls {
        SmtpTransport::relay(&smtp_host)
            .map_err(|e| e.to_string())?
            .port(smtp_port as u16)
            .credentials(creds)
            .build()
    } else {
        SmtpTransport::starttls_relay(&smtp_host)
            .map_err(|e| e.to_string())?
            .port(smtp_port as u16)
            .credentials(creds)
            .build()
    };

    let mut results: Vec<EmailResult> = Vec::new();

    for (apt_id, apt_label, apt_email) in &apartments {
        let bill_ids: Vec<i64> = {
            let conn = db.0.lock().map_err(|e| e.to_string())?;
            query_vec(
                &conn,
                "SELECT bs.bill_id
                 FROM bill_splits bs
                 JOIN bills b ON bs.bill_id = b.id
                 WHERE b.billing_period_id = ?1 AND bs.apartment_id = ?2",
                &[&billing_period_id, apt_id],
                |r| r.get(0),
            )?
        };

        let mut attachments: Vec<(String, Vec<u8>)> = Vec::new();
        let mut pdf_error: Option<String> = None;

        for bill_id in &bill_ids {
            let result = {
                let conn = db.0.lock().map_err(|e| e.to_string())?;
                load_upn_data(&conn, *bill_id, *apt_id)
            }
            .and_then(|d| render_upn_pdf(&d));

            match result {
                Ok(bytes) => {
                    attachments.push((format!("UPN_{}_{}.pdf", apt_label, bill_id), bytes));
                }
                Err(e) => {
                    pdf_error = Some(format!("PDF error bill {}: {}", bill_id, e));
                    break;
                }
            }
        }

        if let Some(e) = pdf_error {
            results.push(EmailResult {
                apartment_label: apt_label.clone(),
                email: apt_email.clone(),
                success: false,
                error: Some(e),
            });
            continue;
        }

        let subject = format!("Poloznice za {}/{}", month, year);
        let body = format!(
            "Spoštovani,\n\nv priponki najdete UPN položnice za {:02}/{}.\n\nLep pozdrav",
            month, year
        );

        let from_result = smtp_from.parse();
        let to_result = apt_email.parse();
        let (from_addr, to_addr) = match (from_result, to_result) {
            (Ok(f), Ok(t)) => (f, t),
            _ => {
                results.push(EmailResult {
                    apartment_label: apt_label.clone(),
                    email: apt_email.clone(),
                    success: false,
                    error: Some("Invalid email address".to_string()),
                });
                continue;
            }
        };

        let mut mp = MultiPart::mixed().singlepart(SinglePart::plain(body));
        for (filename, bytes) in attachments {
            mp = mp.singlepart(
                Attachment::new(filename)
                    .body(bytes, ContentType::parse("application/pdf").unwrap()),
            );
        }

        match Message::builder()
            .from(from_addr)
            .to(to_addr)
            .subject(&subject)
            .multipart(mp)
        {
            Ok(msg) => match mailer.send(&msg) {
                Ok(_) => results.push(EmailResult {
                    apartment_label: apt_label.clone(),
                    email: apt_email.clone(),
                    success: true,
                    error: None,
                }),
                Err(e) => results.push(EmailResult {
                    apartment_label: apt_label.clone(),
                    email: apt_email.clone(),
                    success: false,
                    error: Some(e.to_string()),
                }),
            },
            Err(e) => results.push(EmailResult {
                apartment_label: apt_label.clone(),
                email: apt_email.clone(),
                success: false,
                error: Some(e.to_string()),
            }),
        }
    }

    Ok(results)
}

#[tauri::command]
pub fn save_all_upns(
    db: State<DbState>,
    billing_period_id: i64,
    folder_path: String,
) -> Result<Vec<String>, String> {
    let splits: Vec<(i64, i64, String)> = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        query_vec(
            &conn,
            "SELECT bs.bill_id, bs.apartment_id, a.label
             FROM bill_splits bs
             JOIN bills b ON bs.bill_id = b.id
             JOIN apartments a ON bs.apartment_id = a.id
             WHERE b.billing_period_id = ?1
             ORDER BY a.label, b.id",
            &[&billing_period_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?
    };

    let mut saved: Vec<String> = Vec::new();
    for (bill_id, apt_id, apt_label) in &splits {
        let data = {
            let conn = db.0.lock().map_err(|e| e.to_string())?;
            load_upn_data(&conn, *bill_id, *apt_id)?
        };
        let pdf_bytes = render_upn_pdf(&data)?;
        let filename = format!("UPN_{}_{}.pdf", apt_label.replace(' ', "_"), bill_id);
        let full_path = format!("{}\\{}", folder_path, filename);
        std::fs::write(&full_path, &pdf_bytes).map_err(|e| e.to_string())?;
        saved.push(filename);
    }
    Ok(saved)
}
