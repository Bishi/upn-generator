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

const FORM_WIDTH_MM: f32 = 210.0;
const FORM_HEIGHT_MM: f32 = 99.0;
const LEFT_WIDTH_MM: f32 = 60.0;
const TOP_RIGHT_HEIGHT_MM: f32 = 50.0;

fn format_iban(iban: &str) -> String {
    iban.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<Vec<_>>()
        .chunks(4)
        .map(|chunk| chunk.iter().collect::<String>())
        .collect::<Vec<_>>()
        .join(" ")
}

fn normalize_spaces(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

fn format_amount_display(cents: i64) -> String {
    let euros = cents / 100;
    let cents_part = (cents % 100).unsigned_abs();
    format!("***{},{}", euros, format!("{:02}", cents_part))
}

fn split_reference(reference: &str) -> (String, String) {
    let compact = normalize_spaces(reference);
    let mut parts = compact.splitn(2, ' ');
    let head = parts.next().unwrap_or("").trim().to_string();
    let tail = parts.next().unwrap_or("").trim().to_string();

    if head.len() == 4 && head.starts_with("SI") {
        let digits = tail
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>();
        (head, digits)
    } else {
        let stripped = compact
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>();
        let model = truncate_chars(&stripped, 4);
        let rest = stripped
            .chars()
            .skip(model.chars().count())
            .collect::<String>();
        (model, rest)
    }
}

fn pt_to_mm(points: f32) -> f32 {
    points * 0.352_778
}

fn y_from_top(y_top: f32) -> f32 {
    FORM_HEIGHT_MM - y_top
}

fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb(Rgb::new(
        r as f32 / 255.0,
        g as f32 / 255.0,
        b as f32 / 255.0,
        None,
    ))
}

fn line_top(layer: &PdfLayerReference, x1: f32, y1_top: f32, x2: f32, y2_top: f32) {
    let line = Line {
        points: vec![
            (Point::new(Mm(x1), Mm(y_from_top(y1_top))), false),
            (Point::new(Mm(x2), Mm(y_from_top(y2_top))), false),
        ],
        is_closed: false,
    };
    layer.add_line(line);
}

fn stroke_rect_top(layer: &PdfLayerReference, x1: f32, y1_top: f32, x2: f32, y2_top: f32) {
    let poly = Polygon {
        rings: vec![vec![
            (Point::new(Mm(x1), Mm(y_from_top(y1_top))), false),
            (Point::new(Mm(x2), Mm(y_from_top(y1_top))), false),
            (Point::new(Mm(x2), Mm(y_from_top(y2_top))), false),
            (Point::new(Mm(x1), Mm(y_from_top(y2_top))), false),
        ]],
        mode: PolygonMode::Stroke,
        winding_order: WindingOrder::NonZero,
    };
    layer.add_polygon(poly);
}

fn fill_rect_top(layer: &PdfLayerReference, x1: f32, y1_top: f32, x2: f32, y2_top: f32) {
    let poly = Polygon {
        rings: vec![vec![
            (Point::new(Mm(x1), Mm(y_from_top(y1_top))), false),
            (Point::new(Mm(x2), Mm(y_from_top(y1_top))), false),
            (Point::new(Mm(x2), Mm(y_from_top(y2_top))), false),
            (Point::new(Mm(x1), Mm(y_from_top(y2_top))), false),
        ]],
        mode: PolygonMode::Fill,
        winding_order: WindingOrder::NonZero,
    };
    layer.add_polygon(poly);
}

fn text_top(
    layer: &PdfLayerReference,
    text: &str,
    size_pt: f32,
    x: f32,
    y_top: f32,
    font: &IndirectFontRef,
) {
    let baseline = y_from_top(y_top + pt_to_mm(size_pt) * 0.8);
    layer.use_text(text, size_pt, Mm(x), Mm(baseline), font);
}

fn draw_grid_field(
    layer: &PdfLayerReference,
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    cell_width: Option<f32>,
) {
    stroke_rect_top(layer, x1, y1, x2, y2);
    if let Some(step) = cell_width {
        let mut x = x1 + step;
        while x < x2 - 0.15 {
            line_top(layer, x, y1, x, y2);
            x += step;
        }
    }
}

fn draw_three_line_box(layer: &PdfLayerReference, x1: f32, y1: f32, x2: f32, y2: f32) {
    stroke_rect_top(layer, x1, y1, x2, y2);
    let h = (y2 - y1) / 3.0;
    line_top(layer, x1, y1 + h, x2, y1 + h);
    line_top(layer, x1, y1 + 2.0 * h, x2, y1 + 2.0 * h);
}

fn draw_perforation(layer: &PdfLayerReference) {
    let mut y = 0.0;
    while y < FORM_HEIGHT_MM {
        line_top(
            layer,
            LEFT_WIDTH_MM,
            y,
            LEFT_WIDTH_MM,
            (y + 1.2).min(FORM_HEIGHT_MM),
        );
        y += 2.0;
    }
}

fn load_system_font(preferred_names: &[&str]) -> Option<Vec<u8>> {
    let win_dir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".to_string());
    for name in preferred_names {
        let path = format!("{}\\Fonts\\{}", win_dir, name);
        if let Ok(bytes) = std::fs::read(&path) {
            return Some(bytes);
        }
    }
    None
}

fn to_iso8859_2(s: &str) -> Vec<u8> {
    s.chars()
        .map(|c| match c {
            'Š' => 0xA9,
            'š' => 0xB9,
            'Č' => 0xC8,
            'č' => 0xE8,
            'Ž' => 0xAE,
            'ž' => 0xBE,
            'Ć' => 0xC6,
            'ć' => 0xE6,
            'Đ' => 0xD0,
            'đ' => 0xF0,
            other => {
                let n = other as u32;
                if n < 256 {
                    n as u8
                } else {
                    b'?'
                }
            }
        })
        .collect()
}

fn build_upnqr_string(data: &UpnData) -> String {
    let iban: String = data
        .creditor_iban
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect();
    let reference: String = data
        .creditor_reference
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let amount = format!("{:011}", data.amount_cents);
    let tr = |s: &str, n: usize| -> String { s.chars().take(n).collect() };

    let fields: Vec<String> = vec![
        "UPNQR".into(),
        "".into(),
        "".into(),
        "".into(),
        "".into(),
        tr(&data.payer_name, 33),
        tr(&data.payer_address, 33),
        tr(
            &format!("{} {}", data.payer_postal_code, data.payer_city),
            33,
        ),
        amount,
        "".into(),
        "".into(),
        tr(&data.purpose_code, 4),
        tr(&data.purpose_text, 42),
        tr(&data.due_date, 10),
        tr(&iban, 19),
        tr(&reference, 26),
        tr(&data.creditor_name, 33),
        tr(&data.creditor_address, 33),
        tr(&data.creditor_city, 33),
        "".into(),
    ];

    let checksum: usize = fields.iter().map(|f| f.chars().count()).sum::<usize>() + 19;
    format!("{}\n{:03}", fields.join("\n"), checksum)
}

fn render_upnqr_pixels(data: &UpnData) -> Result<(Vec<u8>, usize), String> {
    use qrcodegen::{Mask, QrCode, QrCodeEcc, QrSegment, Version};

    let qr_string = build_upnqr_string(data);
    let iso_bytes = to_iso8859_2(&qr_string);

    let eci_seg = QrSegment::make_eci(4);
    let data_seg = QrSegment::make_bytes(&iso_bytes);

    let qr = QrCode::encode_segments_advanced(
        &[eci_seg, data_seg],
        QrCodeEcc::Medium,
        Version::new(15),
        Version::new(15),
        Some(Mask::new(2)),
        false,
    )
    .map_err(|e| format!("QR encode error: {:?}", e))?;

    let quiet = 4i32;
    let scale = 4i32;
    let total = qr.size() + 2 * quiet;
    let img_size = (total * scale) as usize;

    let mut pixels = vec![255u8; img_size * img_size];
    for y in 0..qr.size() {
        for x in 0..qr.size() {
            if qr.get_module(x, y) {
                let ox = ((x + quiet) * scale) as usize;
                let oy = ((y + quiet) * scale) as usize;
                for dy in 0..scale as usize {
                    for dx in 0..scale as usize {
                        pixels[(oy + dy) * img_size + (ox + dx)] = 0u8;
                    }
                }
            }
        }
    }
    Ok((pixels, img_size))
}

fn render_upn_pdf(data: &UpnData) -> Result<Vec<u8>, String> {
    let (doc, page1, layer1) = PdfDocument::new(
        "Obrazec UPN QR",
        Mm(FORM_WIDTH_MM),
        Mm(FORM_HEIGHT_MM),
        "Layer 1",
    );
    let layer = doc.get_page(page1).get_layer(layer1);

    let label_font = if let Some(bytes) =
        load_system_font(&["arialbd.ttf", "calibrib.ttf", "seguisb.ttf", "verdanab.ttf"])
    {
        doc.add_external_font(std::io::Cursor::new(bytes))
            .map_err(|e| e.to_string())?
    } else {
        doc.add_builtin_font(BuiltinFont::HelveticaBold)
            .map_err(|e| e.to_string())?
    };

    let mono_font = if let Some(bytes) =
        load_system_font(&["courbd.ttf", "lucon.ttf", "consolab.ttf", "couri.ttf"])
    {
        doc.add_external_font(std::io::Cursor::new(bytes))
            .map_err(|e| e.to_string())?
    } else {
        doc.add_builtin_font(BuiltinFont::CourierBold)
            .map_err(|e| e.to_string())?
    };

    let orange = rgb(244, 120, 54);
    let top_fill = rgb(254, 224, 205);
    let bottom_fill = rgb(255, 244, 210);
    let black = rgb(0, 0, 0);

    layer.set_fill_color(rgb(255, 255, 255));
    fill_rect_top(&layer, 0.0, 0.0, FORM_WIDTH_MM, FORM_HEIGHT_MM);
    layer.set_fill_color(top_fill);
    fill_rect_top(
        &layer,
        LEFT_WIDTH_MM,
        0.0,
        FORM_WIDTH_MM,
        TOP_RIGHT_HEIGHT_MM,
    );
    layer.set_fill_color(bottom_fill);
    fill_rect_top(
        &layer,
        LEFT_WIDTH_MM,
        TOP_RIGHT_HEIGHT_MM,
        FORM_WIDTH_MM,
        FORM_HEIGHT_MM,
    );

    layer.set_outline_color(orange.clone());
    layer.set_fill_color(orange.clone());
    layer.set_outline_thickness(0.25);
    draw_perforation(&layer);

    stroke_rect_top(&layer, 4.0, 6.0, 56.5, 19.5);
    stroke_rect_top(&layer, 4.0, 22.5, 56.5, 31.5);
    stroke_rect_top(&layer, 16.5, 34.5, 56.5, 39.5);
    stroke_rect_top(&layer, 4.0, 42.0, 56.5, 56.0);
    stroke_rect_top(&layer, 4.0, 59.0, 56.5, 72.5);

    draw_grid_field(&layer, 106.5, 6.0, 177.7, 11.0, Some(3.75));
    draw_grid_field(&layer, 185.2, 6.5, 189.2, 10.5, None);
    draw_grid_field(&layer, 196.5, 6.5, 200.5, 10.5, None);
    stroke_rect_top(&layer, 63.5, 6.0, 103.5, 45.5);
    draw_grid_field(&layer, 106.5, 14.0, 121.5, 19.0, Some(3.75));
    draw_grid_field(&layer, 123.5, 14.0, 206.0, 19.0, Some(3.75));
    draw_three_line_box(&layer, 106.5, 22.0, 206.0, 37.0);
    draw_grid_field(&layer, 114.2, 40.5, 155.5, 45.5, Some(3.75));
    draw_grid_field(&layer, 161.2, 40.5, 191.2, 45.5, Some(3.75));
    draw_grid_field(&layer, 196.5, 41.0, 200.5, 45.0, None);
    draw_grid_field(&layer, 63.5, 49.0, 78.5, 54.0, Some(3.75));
    draw_grid_field(&layer, 80.5, 49.0, 174.2, 54.0, Some(3.75));
    draw_grid_field(&layer, 176.2, 49.0, 206.0, 54.0, Some(3.72));
    draw_grid_field(&layer, 63.5, 58.0, 191.0, 63.0, Some(3.75));
    draw_grid_field(&layer, 63.5, 66.0, 78.5, 71.0, Some(3.75));
    draw_grid_field(&layer, 80.5, 66.0, 163.0, 71.0, Some(3.75));
    draw_three_line_box(&layer, 63.5, 74.0, 163.0, 89.0);
    stroke_rect_top(&layer, 168.6, 71.0, 203.3, 89.0);
    line_top(&layer, 172.0, 85.6, 200.0, 85.6);

    layer.set_fill_color(black.clone());
    fill_rect_top(&layer, 61.0, 1.0, 62.5, 2.5);
    fill_rect_top(&layer, 207.5, 1.0, 209.0, 2.5);
    fill_rect_top(&layer, 207.5, 96.5, 209.0, 98.0);

    layer.set_fill_color(orange.clone());
    text_top(&layer, "Ime plačnika", 7.0, 4.0, 3.5, &label_font);
    text_top(&layer, "UPN QR - potrdilo", 10.0, 32.6, 2.0, &label_font);
    text_top(&layer, "Namen in rok plačila", 7.0, 4.0, 20.0, &label_font);
    text_top(&layer, "Znesek", 7.0, 16.5, 32.0, &label_font);
    text_top(
        &layer,
        "IBAN in referenca prejemnika",
        7.0,
        4.0,
        40.0,
        &label_font,
    );
    text_top(&layer, "Ime prejemnika", 7.0, 4.0, 56.5, &label_font);
    text_top(
        &layer,
        "Prostor za vpise ponudnika plačilnih storitev",
        6.0,
        13.8,
        97.0,
        &label_font,
    );
    text_top(&layer, "Koda QR", 7.0, 63.5, 3.5, &label_font);
    text_top(&layer, "IBAN plačnika", 7.0, 106.5, 3.5, &label_font);
    text_top(&layer, "Polog", 7.0, 184.3, 3.5, &label_font);
    text_top(&layer, "Dvig", 7.0, 196.1, 3.5, &label_font);
    text_top(&layer, "Referenca plačnika", 7.0, 106.5, 11.0, &label_font);
    text_top(
        &layer,
        "Ime, ulica in kraj plačnika",
        7.0,
        106.5,
        19.5,
        &label_font,
    );
    text_top(&layer, "EUR", 11.0, 7.8, 35.6, &label_font);
    text_top(&layer, "EUR", 11.0, 111.2, 41.6, &label_font);
    text_top(&layer, "Znesek", 7.0, 114.2, 38.0, &label_font);
    text_top(&layer, "Datum plačila", 7.0, 161.2, 38.0, &label_font);
    text_top(&layer, "Nujno", 7.0, 195.3, 38.0, &label_font);
    text_top(&layer, "Koda namena", 7.0, 63.5, 46.5, &label_font);
    text_top(&layer, "Namen plačila", 7.0, 80.5, 46.5, &label_font);
    text_top(&layer, "Rok plačila", 7.0, 176.2, 46.5, &label_font);
    text_top(&layer, "IBAN prejemnika", 7.0, 63.5, 55.5, &label_font);
    text_top(&layer, "UPN QR", 10.0, 194.3, 59.1, &label_font);
    text_top(&layer, "Referenca prejemnika", 7.0, 63.5, 63.5, &label_font);
    text_top(
        &layer,
        "Ime, ulica in kraj prejemnika",
        7.0,
        63.5,
        71.5,
        &label_font,
    );
    text_top(
        &layer,
        "Podpis plačnika (neobvezno žig)",
        6.0,
        172.0,
        89.0,
        &label_font,
    );
    text_top(
        &layer,
        "Prostor za vpise ponudnika plačilnih storitev",
        5.0,
        118.9,
        97.0,
        &label_font,
    );

    layer.set_fill_color(black);

    let payer_city_line = format!("{} {}", data.payer_postal_code, data.payer_city);
    let creditor_city_line = normalize_spaces(&data.creditor_city);
    let creditor_iban = format_iban(&data.creditor_iban);
    let (reference_model, reference_body) = split_reference(&data.creditor_reference);
    let purpose_line = truncate_chars(&normalize_spaces(&data.purpose_text), 42);
    let receipt_detail = truncate_chars(
        &format!("{}, {}", truncate_chars(&reference_body, 18), data.due_date),
        34,
    );
    let amount_display = format_amount_display(data.amount_cents);

    text_top(
        &layer,
        &truncate_chars(&data.payer_name, 24),
        12.0,
        7.0,
        8.4,
        &mono_font,
    );
    text_top(
        &layer,
        &truncate_chars(&data.payer_address, 24),
        12.0,
        7.0,
        14.2,
        &mono_font,
    );
    text_top(
        &layer,
        &truncate_chars(&payer_city_line, 24),
        12.0,
        7.0,
        20.0,
        &mono_font,
    );
    text_top(
        &layer,
        &truncate_chars(&purpose_line, 28),
        11.0,
        7.0,
        25.2,
        &mono_font,
    );
    text_top(&layer, &receipt_detail, 10.5, 7.0, 30.0, &mono_font);
    text_top(&layer, &amount_display, 12.0, 22.0, 35.3, &mono_font);
    text_top(&layer, &creditor_iban, 11.0, 7.0, 45.0, &mono_font);
    text_top(
        &layer,
        &truncate_chars(&format!("{} {}", reference_model, reference_body), 28),
        11.0,
        7.0,
        51.0,
        &mono_font,
    );
    text_top(
        &layer,
        &truncate_chars(&data.creditor_name, 24),
        12.0,
        7.0,
        61.8,
        &mono_font,
    );
    text_top(
        &layer,
        &truncate_chars(&data.creditor_address, 24),
        12.0,
        7.0,
        67.6,
        &mono_font,
    );
    text_top(
        &layer,
        &truncate_chars(&creditor_city_line, 24),
        12.0,
        7.0,
        73.4,
        &mono_font,
    );

    if let Ok((pixels, img_px)) = render_upnqr_pixels(data) {
        let qr_target_mm = 36.5f64;
        let dpi = img_px as f64 * 25.4 / qr_target_mm;
        let img_xobj = ImageXObject {
            width: Px(img_px),
            height: Px(img_px),
            color_space: ColorSpace::Greyscale,
            bits_per_component: ColorBits::Bit8,
            image_data: pixels,
            image_filter: None,
            clipping_bbox: None,
            interpolate: false,
        };
        Image::from(img_xobj).add_to_layer(
            layer.clone(),
            ImageTransform {
                translate_x: Some(Mm(65.2)),
                translate_y: Some(Mm(y_from_top(7.8 + qr_target_mm as f32))),
                dpi: Some(dpi as f32),
                ..Default::default()
            },
        );
    }

    text_top(
        &layer,
        &truncate_chars(&data.payer_name, 29),
        14.0,
        109.0,
        24.0,
        &mono_font,
    );
    text_top(
        &layer,
        &truncate_chars(&data.payer_address, 29),
        14.0,
        109.0,
        30.0,
        &mono_font,
    );
    text_top(
        &layer,
        &truncate_chars(&payer_city_line, 29),
        14.0,
        109.0,
        36.0,
        &mono_font,
    );
    text_top(&layer, &amount_display, 12.0, 118.0, 41.2, &mono_font);
    text_top(
        &layer,
        &truncate_chars(&data.purpose_code.to_uppercase(), 4),
        11.0,
        66.5,
        49.6,
        &mono_font,
    );
    text_top(
        &layer,
        &truncate_chars(&purpose_line, 42),
        11.0,
        82.5,
        49.6,
        &mono_font,
    );
    text_top(
        &layer,
        &truncate_chars(&data.due_date, 10),
        12.0,
        178.0,
        49.6,
        &mono_font,
    );
    text_top(&layer, &creditor_iban, 11.0, 65.5, 58.3, &mono_font);
    text_top(&layer, &reference_model, 11.0, 65.5, 66.4, &mono_font);
    text_top(
        &layer,
        &truncate_chars(&reference_body, 22),
        11.0,
        82.5,
        66.4,
        &mono_font,
    );
    text_top(
        &layer,
        &truncate_chars(&data.creditor_name, 30),
        14.0,
        66.5,
        76.0,
        &mono_font,
    );
    text_top(
        &layer,
        &truncate_chars(&data.creditor_address, 30),
        14.0,
        66.5,
        82.0,
        &mono_font,
    );
    text_top(
        &layer,
        &truncate_chars(&creditor_city_line, 30),
        14.0,
        66.5,
        88.0,
        &mono_font,
    );

    let mut buf: Vec<u8> = Vec::new();
    doc.save(&mut BufWriter::new(std::io::Cursor::new(&mut buf)))
        .map_err(|e| e.to_string())?;
    Ok(buf)
}

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

    let (
        reference,
        due_date,
        purpose_code,
        purpose_text,
        creditor_name,
        creditor_iban,
        creditor_address,
        creditor_city,
    ): (
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
    ) = conn
        .query_row(
            "SELECT reference, due_date, purpose_code, purpose_text,
             creditor_name, creditor_iban, creditor_address, creditor_city
             FROM bills WHERE id=?1",
            [bill_id],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                    r.get(7)?,
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
pub fn preview_upn(db: State<DbState>, bill_id: i64, apartment_id: i64) -> Result<String, String> {
    let data = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        load_upn_data(&conn, bill_id, apartment_id)?
    };
    let pdf_bytes = render_upn_pdf(&data)?;
    let temp_path = std::env::temp_dir().join(format!("upn_{}_{}.pdf", bill_id, apartment_id));
    std::fs::write(&temp_path, &pdf_bytes).map_err(|e| e.to_string())?;
    temp_path
        .to_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "Invalid temp path".to_string())
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
pub fn send_emails(db: State<DbState>, billing_period_id: i64) -> Result<Vec<EmailResult>, String> {
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
        return Err("No apartments with email addresses have splits in this period.".to_string());
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
                    attachments.push((format!("UPN_{}_{}.pdf", apt_label, bill_id), bytes))
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
