use base64::Engine;
use lettre::message::header::ContentType;
use lettre::message::{Attachment, MultiPart, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};
use ::lopdf::{Document as LoDocument, Object, ObjectId};
use printpdf::*;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::BufWriter;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use super::config::DbState;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EmailResult {
    pub apartment_label: String,
    pub email: String,
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Clone)]
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
const MONO_WIDTH_FACTOR: f32 = 0.62;
const MIN_FONT_SIZE_PT: f32 = 7.5;

#[derive(Clone, Copy)]
struct Rect {
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
}

impl Rect {
    fn width(self) -> f32 {
        self.x2 - self.x1
    }

    fn height(self) -> f32 {
        self.y2 - self.y1
    }
}

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

fn mm_to_pt(mm: f32) -> f32 {
    mm / 0.352_778
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

fn mono_text_width_mm(text: &str, size_pt: f32) -> f32 {
    text.chars().count() as f32 * pt_to_mm(size_pt * MONO_WIDTH_FACTOR)
}

fn fit_mono_text_to_width(text: &str, width_mm: f32, preferred_pt: f32, min_pt: f32) -> (String, f32) {
    let compact = normalize_spaces(text);
    if compact.is_empty() {
        return (compact, preferred_pt);
    }

    let mut size_pt = preferred_pt;
    while size_pt > min_pt && mono_text_width_mm(&compact, size_pt) > width_mm {
        size_pt -= 0.25;
    }

    if mono_text_width_mm(&compact, size_pt) <= width_mm {
        return (compact, size_pt.max(min_pt));
    }

    let max_chars = ((width_mm / pt_to_mm(size_pt * MONO_WIDTH_FACTOR)).floor() as usize).max(1);
    (truncate_chars(&compact, max_chars), size_pt.max(min_pt))
}

fn draw_fitted_single_line(
    layer: &PdfLayerReference,
    text: &str,
    rect: Rect,
    preferred_pt: f32,
    min_pt: f32,
    font: &IndirectFontRef,
) {
    let horizontal_padding = 1.4f32;
    let available_width = (rect.width() - horizontal_padding * 2.0).max(1.0);
    let available_height = rect.height().max(1.0);
    let max_height_pt = (mm_to_pt(available_height) * 0.76).max(min_pt);
    let preferred_pt = preferred_pt.min(max_height_pt);
    let min_pt = min_pt.min(preferred_pt);
    let (fitted, size_pt) = fit_mono_text_to_width(text, available_width, preferred_pt, min_pt);
    let text_height_mm = pt_to_mm(size_pt) * 0.92;
    let top = rect.y1 + ((available_height - text_height_mm).max(0.0) / 2.0);
    text_top(layer, &fitted, size_pt, rect.x1 + horizontal_padding, top, font);
}

fn draw_fitted_multi_line(
    layer: &PdfLayerReference,
    lines: &[String],
    rect: Rect,
    preferred_pt: f32,
    min_pt: f32,
    font: &IndirectFontRef,
) {
    if lines.is_empty() {
        return;
    }

    let row_height = rect.height() / lines.len() as f32;
    for (index, line) in lines.iter().enumerate() {
        let row = Rect {
            x1: rect.x1,
            y1: rect.y1 + row_height * index as f32,
            x2: rect.x2,
            y2: rect.y1 + row_height * (index as f32 + 1.0),
        };
        draw_fitted_single_line(layer, line, row, preferred_pt, min_pt, font);
    }
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
        load_system_font(&["courbd.ttf", "cour.ttf", "lucon.ttf", "consola.ttf"])
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

    let receipt_payer_box = Rect {
        x1: 4.0,
        y1: 6.0,
        x2: 56.5,
        y2: 19.5,
    };
    let receipt_purpose_box = Rect {
        x1: 4.0,
        y1: 22.5,
        x2: 56.5,
        y2: 31.5,
    };
    let receipt_amount_box = Rect {
        x1: 16.5,
        y1: 34.5,
        x2: 56.5,
        y2: 39.5,
    };
    let receipt_iban_ref_box = Rect {
        x1: 4.0,
        y1: 42.0,
        x2: 56.5,
        y2: 56.0,
    };
    let receipt_creditor_box = Rect {
        x1: 4.0,
        y1: 59.0,
        x2: 56.5,
        y2: 72.5,
    };
    let qr_box = Rect {
        x1: 63.5,
        y1: 6.0,
        x2: 103.5,
        y2: 45.5,
    };
    let main_payer_box = Rect {
        x1: 106.5,
        y1: 22.0,
        x2: 206.0,
        y2: 37.0,
    };
    let amount_box = Rect {
        x1: 114.2,
        y1: 40.5,
        x2: 155.5,
        y2: 45.5,
    };
    let purpose_code_box = Rect {
        x1: 63.5,
        y1: 49.0,
        x2: 78.5,
        y2: 54.0,
    };
    let purpose_box = Rect {
        x1: 80.5,
        y1: 49.0,
        x2: 174.2,
        y2: 54.0,
    };
    let date_box = Rect {
        x1: 176.2,
        y1: 49.0,
        x2: 206.0,
        y2: 54.0,
    };
    let creditor_iban_box = Rect {
        x1: 63.5,
        y1: 58.0,
        x2: 191.0,
        y2: 63.0,
    };
    let creditor_reference_model_box = Rect {
        x1: 63.5,
        y1: 66.0,
        x2: 78.5,
        y2: 71.0,
    };
    let creditor_reference_body_box = Rect {
        x1: 80.5,
        y1: 66.0,
        x2: 163.0,
        y2: 71.0,
    };
    let creditor_box = Rect {
        x1: 63.5,
        y1: 74.0,
        x2: 163.0,
        y2: 89.0,
    };

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

    stroke_rect_top(
        &layer,
        receipt_payer_box.x1,
        receipt_payer_box.y1,
        receipt_payer_box.x2,
        receipt_payer_box.y2,
    );
    stroke_rect_top(
        &layer,
        receipt_purpose_box.x1,
        receipt_purpose_box.y1,
        receipt_purpose_box.x2,
        receipt_purpose_box.y2,
    );
    stroke_rect_top(
        &layer,
        receipt_amount_box.x1,
        receipt_amount_box.y1,
        receipt_amount_box.x2,
        receipt_amount_box.y2,
    );
    stroke_rect_top(
        &layer,
        receipt_iban_ref_box.x1,
        receipt_iban_ref_box.y1,
        receipt_iban_ref_box.x2,
        receipt_iban_ref_box.y2,
    );
    stroke_rect_top(
        &layer,
        receipt_creditor_box.x1,
        receipt_creditor_box.y1,
        receipt_creditor_box.x2,
        receipt_creditor_box.y2,
    );

    draw_grid_field(&layer, 106.5, 6.0, 177.7, 11.0, Some(3.75));
    draw_grid_field(&layer, 185.2, 6.5, 189.2, 10.5, None);
    draw_grid_field(&layer, 196.5, 6.5, 200.5, 10.5, None);
    stroke_rect_top(&layer, qr_box.x1, qr_box.y1, qr_box.x2, qr_box.y2);
    draw_grid_field(&layer, 106.5, 14.0, 121.5, 19.0, Some(3.75));
    draw_grid_field(&layer, 123.5, 14.0, 206.0, 19.0, Some(3.75));
    draw_three_line_box(&layer, main_payer_box.x1, main_payer_box.y1, main_payer_box.x2, main_payer_box.y2);
    draw_grid_field(&layer, amount_box.x1, amount_box.y1, amount_box.x2, amount_box.y2, Some(3.75));
    draw_grid_field(&layer, 161.2, 40.5, 191.2, 45.5, Some(3.75));
    draw_grid_field(&layer, 196.5, 41.0, 200.5, 45.0, None);
    draw_grid_field(
        &layer,
        purpose_code_box.x1,
        purpose_code_box.y1,
        purpose_code_box.x2,
        purpose_code_box.y2,
        Some(3.75),
    );
    draw_grid_field(&layer, purpose_box.x1, purpose_box.y1, purpose_box.x2, purpose_box.y2, Some(3.75));
    draw_grid_field(&layer, date_box.x1, date_box.y1, date_box.x2, date_box.y2, Some(3.72));
    draw_grid_field(
        &layer,
        creditor_iban_box.x1,
        creditor_iban_box.y1,
        creditor_iban_box.x2,
        creditor_iban_box.y2,
        Some(3.75),
    );
    draw_grid_field(
        &layer,
        creditor_reference_model_box.x1,
        creditor_reference_model_box.y1,
        creditor_reference_model_box.x2,
        creditor_reference_model_box.y2,
        Some(3.75),
    );
    draw_grid_field(
        &layer,
        creditor_reference_body_box.x1,
        creditor_reference_body_box.y1,
        creditor_reference_body_box.x2,
        creditor_reference_body_box.y2,
        Some(3.75),
    );
    draw_three_line_box(&layer, creditor_box.x1, creditor_box.y1, creditor_box.x2, creditor_box.y2);
    stroke_rect_top(&layer, 168.6, 71.0, 203.3, 89.0);
    line_top(&layer, 172.0, 85.6, 200.0, 85.6);

    layer.set_fill_color(black.clone());
    fill_rect_top(&layer, 61.0, 1.0, 62.5, 2.5);
    fill_rect_top(&layer, 207.5, 1.0, 209.0, 2.5);
    fill_rect_top(&layer, 207.5, 96.5, 209.0, 98.0);

    layer.set_fill_color(orange.clone());
    text_top(&layer, "Ime plačnika", 7.0, 4.0, 3.5, &label_font);
    text_top(&layer, "UPN QR - potrdilo", 10.0, 28.0, 2.0, &label_font);
    text_top(&layer, "Namen in rok plačila", 7.0, 4.0, 20.0, &label_font);
    text_top(&layer, "Znesek", 7.0, 16.5, 32.0, &label_font);
    text_top(&layer, "IBAN in referenca prejemnika", 7.0, 4.0, 40.0, &label_font);
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
    text_top(&layer, "Ime, ulica in kraj plačnika", 7.0, 106.5, 19.5, &label_font);
    text_top(&layer, "EUR", 11.0, 7.8, 35.6, &label_font);
    text_top(&layer, "EUR", 11.0, 104.5, 41.6, &label_font);
    text_top(&layer, "Znesek", 7.0, 114.2, 38.0, &label_font);
    text_top(&layer, "Datum plačila", 7.0, 161.2, 38.0, &label_font);
    text_top(&layer, "Nujno", 7.0, 195.3, 38.0, &label_font);
    text_top(&layer, "Koda namena", 7.0, 63.5, 46.5, &label_font);
    text_top(&layer, "Namen plačila", 7.0, 80.5, 46.5, &label_font);
    text_top(&layer, "Rok plačila", 7.0, 176.2, 46.5, &label_font);
    text_top(&layer, "IBAN prejemnika", 7.0, 63.5, 55.5, &label_font);
    text_top(&layer, "UPN QR", 10.0, 194.3, 59.1, &label_font);
    text_top(&layer, "Referenca prejemnika", 7.0, 63.5, 63.5, &label_font);
    text_top(&layer, "Ime, ulica in kraj prejemnika", 7.0, 63.5, 71.5, &label_font);
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

    draw_fitted_multi_line(
        &layer,
        &[
            data.payer_name.clone(),
            data.payer_address.clone(),
            payer_city_line.clone(),
        ],
        receipt_payer_box,
        10.8,
        MIN_FONT_SIZE_PT,
        &mono_font,
    );
    draw_fitted_multi_line(
        &layer,
        &[purpose_line.clone(), receipt_detail],
        receipt_purpose_box,
        9.6,
        7.8,
        &mono_font,
    );
    draw_fitted_single_line(&layer, &amount_display, receipt_amount_box, 11.5, 9.0, &mono_font);
    draw_fitted_multi_line(
        &layer,
        &[creditor_iban.clone(), format!("{} {}", reference_model, reference_body)],
        receipt_iban_ref_box,
        10.0,
        7.8,
        &mono_font,
    );
    draw_fitted_multi_line(
        &layer,
        &[
            data.creditor_name.clone(),
            data.creditor_address.clone(),
            creditor_city_line.clone(),
        ],
        receipt_creditor_box,
        10.6,
        MIN_FONT_SIZE_PT,
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

    draw_fitted_multi_line(
        &layer,
        &[
            data.payer_name.clone(),
            data.payer_address.clone(),
            payer_city_line,
        ],
        main_payer_box,
        13.2,
        9.0,
        &mono_font,
    );
    draw_fitted_single_line(&layer, &amount_display, amount_box, 11.5, 8.5, &mono_font);
    draw_fitted_single_line(
        &layer,
        &data.purpose_code.to_uppercase(),
        purpose_code_box,
        10.5,
        8.0,
        &mono_font,
    );
    draw_fitted_single_line(&layer, &purpose_line, purpose_box, 10.0, 7.8, &mono_font);
    draw_fitted_single_line(&layer, &data.due_date, date_box, 11.0, 8.5, &mono_font);
    draw_fitted_single_line(&layer, &creditor_iban, creditor_iban_box, 10.4, 8.0, &mono_font);
    draw_fitted_single_line(
        &layer,
        &reference_model,
        creditor_reference_model_box,
        10.0,
        8.0,
        &mono_font,
    );
    draw_fitted_single_line(
        &layer,
        &reference_body,
        creditor_reference_body_box,
        10.0,
        7.8,
        &mono_font,
    );
    draw_fitted_multi_line(
        &layer,
        &[
            data.creditor_name.clone(),
            data.creditor_address.clone(),
            creditor_city_line,
        ],
        creditor_box,
        12.2,
        8.8,
        &mono_font,
    );

    let mut buf: Vec<u8> = Vec::new();
    doc.save(&mut BufWriter::new(std::io::Cursor::new(&mut buf)))
        .map_err(|e| e.to_string())?;
    Ok(buf)
}

fn merge_pdf_documents(documents: Vec<Vec<u8>>) -> Result<Vec<u8>, String> {
    if documents.is_empty() {
        return Err("No UPN PDFs to merge.".to_string());
    }
    if documents.len() == 1 {
        return Ok(documents.into_iter().next().unwrap());
    }

    let mut max_id = 1;
    let mut pages = BTreeMap::<ObjectId, Object>::new();
    let mut objects = BTreeMap::<ObjectId, Object>::new();
    let mut merged = LoDocument::with_version("1.5");
    let mut catalog_object: Option<(ObjectId, Object)> = None;
    let mut pages_object: Option<(ObjectId, Object)> = None;

    for bytes in documents {
        let mut document = LoDocument::load_mem(&bytes).map_err(|e| e.to_string())?;
        document.renumber_objects_with(max_id);
        max_id = document.max_id + 1;

        for (object_id, object) in document.get_pages().into_values().map(|object_id| {
            let object = document
                .get_object(object_id)
                .map(|obj| obj.to_owned())
                .unwrap_or(Object::Null);
            (object_id, object)
        }) {
            pages.insert(object_id, object);
        }

        objects.extend(document.objects);
    }

    for (object_id, object) in objects {
        match object.type_name().unwrap_or("") {
            "Catalog" => {
                catalog_object = Some((
                    catalog_object.map(|(id, _)| id).unwrap_or(object_id),
                    object,
                ));
            }
            "Pages" => {
                if let Ok(dictionary) = object.as_dict() {
                    let mut dictionary = dictionary.clone();
                    if let Some((_, ref existing)) = pages_object {
                        if let Ok(existing_dictionary) = existing.as_dict() {
                            dictionary.extend(existing_dictionary);
                        }
                    }
                    pages_object = Some((
                        pages_object.map(|(id, _)| id).unwrap_or(object_id),
                        Object::Dictionary(dictionary),
                    ));
                }
            }
            "Page" | "Outlines" | "Outline" => {}
            _ => {
                merged.objects.insert(object_id, object);
            }
        }
    }

    let (pages_id, pages_root) = pages_object.ok_or_else(|| "Pages root not found.".to_string())?;
    let (catalog_id, catalog_root) =
        catalog_object.ok_or_else(|| "Catalog root not found.".to_string())?;

    for (object_id, object) in &pages {
        if let Ok(dictionary) = object.as_dict() {
            let mut dictionary = dictionary.clone();
            dictionary.set("Parent", pages_id);
            merged.objects.insert(*object_id, Object::Dictionary(dictionary));
        }
    }

    if let Ok(dictionary) = pages_root.as_dict() {
        let mut dictionary = dictionary.clone();
        dictionary.set("Count", pages.len() as u32);
        dictionary.set(
            "Kids",
            pages
                .keys()
                .copied()
                .map(Object::Reference)
                .collect::<Vec<_>>(),
        );
        merged.objects.insert(pages_id, Object::Dictionary(dictionary));
    }

    if let Ok(dictionary) = catalog_root.as_dict() {
        let mut dictionary = dictionary.clone();
        dictionary.set("Pages", pages_id);
        dictionary.remove(b"Outlines");
        merged.objects.insert(catalog_id, Object::Dictionary(dictionary));
    }

    merged.trailer.set("Root", catalog_id);
    merged.max_id = merged.objects.len() as u32;
    merged.renumber_objects();

    let mut output = Vec::new();
    merged.save_to(&mut output).map_err(|e| e.to_string())?;
    Ok(output)
}

fn render_upn_pdf_batch(items: &[UpnData]) -> Result<Vec<u8>, String> {
    let pdfs = items
        .iter()
        .map(render_upn_pdf)
        .collect::<Result<Vec<_>, _>>()?;
    merge_pdf_documents(pdfs)
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

fn load_apartment_upn_data(
    conn: &rusqlite::Connection,
    billing_period_id: i64,
    apartment_id: i64,
) -> Result<Vec<UpnData>, String> {
    let bill_ids = query_vec(
        conn,
        "SELECT bs.bill_id
         FROM bill_splits bs
         JOIN bills b ON bs.bill_id = b.id
         WHERE b.billing_period_id = ?1 AND bs.apartment_id = ?2
         ORDER BY b.id",
        &[&billing_period_id, &apartment_id],
        |r| r.get::<_, i64>(0),
    )?;

    bill_ids
        .into_iter()
        .map(|bill_id| load_upn_data(conn, bill_id, apartment_id))
        .collect()
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

fn write_preview_pdf(data: &UpnData, bill_id: i64, apartment_id: i64) -> Result<String, String> {
    let pdf_bytes = render_upn_pdf(data)?;
    let temp_path = std::env::temp_dir().join(format!("upn_{}_{}.pdf", bill_id, apartment_id));
    std::fs::write(&temp_path, &pdf_bytes).map_err(|e| e.to_string())?;
    temp_path
        .to_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "Invalid temp path".to_string())
}

fn write_batch_preview_pdf(
    items: &[UpnData],
    billing_period_id: i64,
    apartment_id: i64,
) -> Result<String, String> {
    let pdf_bytes = render_upn_pdf_batch(items)?;
    let temp_path =
        std::env::temp_dir().join(format!("upn_batch_{}_{}.pdf", billing_period_id, apartment_id));
    std::fs::write(&temp_path, &pdf_bytes).map_err(|e| e.to_string())?;
    temp_path
        .to_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "Invalid temp path".to_string())
}

#[tauri::command]
pub fn preview_upn(db: State<DbState>, bill_id: i64, apartment_id: i64) -> Result<String, String> {
    let data = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        load_upn_data(&conn, bill_id, apartment_id)?
    };
    write_preview_pdf(&data, bill_id, apartment_id)
}

#[tauri::command]
pub fn open_preview_upn(
    app: AppHandle,
    db: State<DbState>,
    bill_id: i64,
    apartment_id: i64,
) -> Result<String, String> {
    let data = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        load_upn_data(&conn, bill_id, apartment_id)?
    };
    let path = write_preview_pdf(&data, bill_id, apartment_id)?;
    app.opener()
        .open_path(&path, None::<String>)
        .map_err(|e| format!("System PDF opener failed for {}: {}", path, e))?;
    Ok(path)
}

#[tauri::command]
pub fn open_preview_apartment_upns(
    app: AppHandle,
    db: State<DbState>,
    billing_period_id: i64,
    apartment_id: i64,
) -> Result<String, String> {
    let items = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        load_apartment_upn_data(&conn, billing_period_id, apartment_id)?
    };
    let path = write_batch_preview_pdf(&items, billing_period_id, apartment_id)?;
    app.opener()
        .open_path(&path, None::<String>)
        .map_err(|e| format!("System PDF opener failed for {}: {}", path, e))?;
    Ok(path)
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
        let attachment_bytes = {
            let conn = db.0.lock().map_err(|e| e.to_string())?;
            load_apartment_upn_data(&conn, billing_period_id, *apt_id)
                .and_then(|items| render_upn_pdf_batch(&items))
        };

        let attachment_bytes = match attachment_bytes {
            Ok(bytes) => bytes,
            Err(e) => {
                results.push(EmailResult {
                    apartment_label: apt_label.clone(),
                    email: apt_email.clone(),
                    success: false,
                    error: Some(e),
                });
                continue;
            }
        };

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

        let filename = format!(
            "UPN_{}_{:02}_{}.pdf",
            apt_label.replace(' ', "_"),
            month,
            year
        );
        let mp = MultiPart::mixed()
            .singlepart(SinglePart::plain(body))
            .singlepart(
                Attachment::new(filename).body(
                    attachment_bytes,
                    ContentType::parse("application/pdf").unwrap(),
                ),
            );

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
