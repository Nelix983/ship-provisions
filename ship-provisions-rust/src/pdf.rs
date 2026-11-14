//! Parse ship-supplier proforma invoice PDFs (SHIPKO layout, plus generic fallback).
//!
//! A single PDF may contain several proforma invoices (PROVISIONS, CABIN, BONDED, ...),
//! each starting with the heading "Proforma invoice". Line items look like:
//!   12 BEEF TENDERLOIN 3/4 UP (NZ) 26,84 KGS 22,44 EUR 602,29
//! i.e. ref | description | quantity | unit | unit price | total, with Dutch number
//! formatting (comma decimals, dot thousands).

use anyhow::{Context, Result, bail};
use chrono::NaiveDate;
use regex::Regex;

use crate::models::{Invoice, InvoiceLine, ParsedPdf};

fn line_re() -> Regex {
    Regex::new(
        r"(?m)^\s*(\d{1,4})\s+(.+?)\s+(\d[\d.,]*)\s+([A-Za-z][A-Za-z0-9/]{0,11})\s+(\d[\d.,]*)\s+(\d[\d.,]*)\s*$",
    )
    .unwrap()
}

fn eur_re() -> Regex {
    Regex::new(r"\bEUR\b").unwrap()
}

pub fn parse_number(s: &str) -> Result<f64> {
    let s = s.trim();
    if s.contains(',') {
        let t = s.replace('.', "").replace(',', ".");
        Ok(t.parse::<f64>()?)
    } else {
        Ok(s.parse::<f64>()?)
    }
}

pub fn parse_date(s: &str) -> Result<String> {
    let d = NaiveDate::parse_from_str(s.trim(), "%d-%m-%Y")?;
    Ok(d.format("%Y-%m-%d").to_string())
}

fn parse_line(line: &str) -> Option<InvoiceLine> {
    let cleaned = eur_re().replace_all(line, " ");
    let caps = line_re().captures(&cleaned)?;
    let desc = caps[2].split_whitespace().collect::<Vec<_>>().join(" ");
    if desc.is_empty() {
        return None;
    }
    let quantity = parse_number(&caps[3]).ok()?;
    let unit_price = parse_number(&caps[5]).ok()?;
    let total = parse_number(&caps[6]).ok()?;
    Some(InvoiceLine {
        line_ref: caps[1].to_string(),
        description: desc,
        quantity,
        unit: caps[4].to_uppercase(),
        unit_price,
        total,
    })
}

fn parse_invoice_chunk(chunk: &str) -> Option<Invoice> {
    let mut inv = Invoice::default();
    for line in chunk.lines() {
        if let Some(item) = parse_line(line) {
            inv.lines.push(item);
        }
    }
    if inv.lines.is_empty() {
        return None;
    }

    let date = r"\d{1,2}-\d{1,2}-\d{4}";
    let re_date_debtor = Regex::new(&format!(r"(?m)^({date})\s+(\d{{4,6}}D)\b")).unwrap();
    if let Some(c) = re_date_debtor.captures(chunk) {
        inv.invoice_date = parse_date(&c[1]).unwrap_or_default();
        inv.debtor = c[2].to_string();
    }

    let re_delivery = Regex::new(&format!(
        r"(?m)^({date})\s+(\d{{6,8}})\s+(\d{{6,7}})\s+Your reference\s+(.+?)\s*$"
    ))
    .unwrap();
    if let Some(c) = re_delivery.captures(chunk) {
        inv.delivery_date = parse_date(&c[1]).unwrap_or_default();
        inv.order_no = c[2].to_string();
        inv.packing_list = c[3].to_string();
        inv.reference = c[4].trim().to_uppercase();
    }
    if inv.reference.is_empty() {
        let re_ref = Regex::new(r"(?m)Your reference\s+([A-Za-z][A-Za-z ]*?)\s*(?:\n|$)").unwrap();
        if let Some(c) = re_ref.captures(chunk) {
            inv.reference = c[1].trim().to_uppercase();
        }
    }

    let re_port =
        Regex::new(r"(?m)Port\s+([A-ZÀ-Þ][A-ZÀ-Þ ,]*?)(?:\s+Your reference|\n|$)").unwrap();
    if let Some(c) = re_port.captures(chunk) {
        inv.port = c[1].split_whitespace().collect::<Vec<_>>().join(" ");
    }

    let re_order = Regex::new(r"Your order\s+([A-Z0-9][A-Z0-9\-]+)").unwrap();
    if let Some(c) = re_order.captures(chunk) {
        inv.your_order = c[1].trim().to_string();
    }

    let re_ship = Regex::new(r"(?m)^\s*([A-Z][^\n(]+?)\s*\(IMO").unwrap();
    if let Some(c) = re_ship.captures(chunk) {
        inv.ship = c[1].trim().to_string();
    }

    let re_excl = Regex::new(r"Total excl\. VAT\s+([\d.,]+)").unwrap();
    if let Some(c) = re_excl.captures(chunk) {
        inv.total_excl_vat = parse_number(&c[1]).ok();
    }

    let re_total = Regex::new(r"(?m)Total\s+(?:EUR\s+)?([\d.,]+)\s*$").unwrap();
    if let Some(c) = re_total.captures_iter(chunk).last() {
        inv.grand_total = parse_number(&c[1]).ok();
    }

    Some(inv)
}


// ---------------------------------------------------------------- layout extractor
// pdf-extract returns text in reading order, which jumbles table columns. This
// extractor tracks the text matrix so we can rebuild visual lines (group by y,
// sort by x), which restores the invoice table layout.

fn obj_f64(o: &lopdf::Object) -> f64 {
    match o {
        lopdf::Object::Integer(i) => *i as f64,
        lopdf::Object::Real(f) => *f as f64,
        _ => 0.0,
    }
}

fn obj_string(o: &lopdf::Object) -> String {
    match o {
        lopdf::Object::String(bytes, _) => String::from_utf8_lossy(bytes).to_string(),
        _ => String::new(),
    }
}

fn extract_text_layout(path: &str) -> Result<String> {
    let doc = lopdf::Document::load(path)
        .with_context(|| format!("could not open PDF '{path}'"))?;
    let mut out = String::new();
    for (_page_no, obj_id) in doc.get_pages() {
        let content = match doc.get_and_decode_page_content(obj_id) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let mut tlm = (0.0f64, 0.0f64); // text line matrix (x, y)
        let mut tm = tlm;              // text matrix
        let mut font_size = 8.0f64;
        let mut leading = 0.0f64;
        let mut segs: Vec<(f64, f64, String)> = Vec::new(); // (x, y, text)
        for op in &content.operations {
            match op.operator.as_str() {
                "BT" => {
                    tlm = (0.0, 0.0);
                    tm = tlm;
                }
                "Tf" => {
                    if let Some(f) = op.operands.get(1) {
                        font_size = obj_f64(f).abs();
                        if font_size == 0.0 {
                            font_size = 8.0;
                        }
                    }
                }
                "Tm" => {
                    if op.operands.len() >= 6 {
                        tm = (obj_f64(&op.operands[4]), obj_f64(&op.operands[5]));
                        tlm = tm;
                    }
                }
                "Td" => {
                    if op.operands.len() >= 2 {
                        tlm = (tlm.0 + obj_f64(&op.operands[0]), tlm.1 + obj_f64(&op.operands[1]));
                        tm = tlm;
                    }
                }
                "TD" => {
                    if op.operands.len() >= 2 {
                        let ty = obj_f64(&op.operands[1]);
                        leading = -ty;
                        tlm = (tlm.0 + obj_f64(&op.operands[0]), tlm.1 + ty);
                        tm = tlm;
                    }
                }
                "TL" => {
                    if let Some(l) = op.operands.first() {
                        leading = obj_f64(l);
                    }
                }
                "T*" => {
                    tlm = (tlm.0, tlm.1 - leading);
                    tm = tlm;
                }
                "Tj" => {
                    if let Some(s) = op.operands.first() {
                        let txt = obj_string(s);
                        let w = 0.5 * font_size * txt.chars().count() as f64;
                        segs.push((tm.0, tm.1, txt));
                        tm.0 += w;
                    }
                }
                "TJ" => {
                    if let Some(lopdf::Object::Array(items)) = op.operands.first() {
                        let txt: String = items
                            .iter()
                            .filter(|i| matches!(i, lopdf::Object::String(..)))
                            .map(obj_string)
                            .collect();
                        let w = 0.5 * font_size * txt.chars().count() as f64;
                        segs.push((tm.0, tm.1, txt));
                        tm.0 += w;
                    }
                }
                "'" => {
                    tlm = (tlm.0, tlm.1 - leading);
                    tm = tlm;
                    if let Some(s) = op.operands.first() {
                        segs.push((tm.0, tm.1, obj_string(s)));
                    }
                }
                _ => {}
            }
        }
        // group segments into visual lines by y (page y grows upward), then sort
        // each line's segments by x so columns read left-to-right
        segs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let tol = 2.5f64;
        let mut lines: Vec<(f64, Vec<(f64, String)>)> = Vec::new();
        for (x, y, txt) in segs {
            if txt.trim().is_empty() {
                continue;
            }
            match lines.last_mut() {
                Some((ly, segs)) if (y - *ly).abs() <= tol => segs.push((x, txt.trim().to_string())),
                _ => lines.push((y, vec![(x, txt.trim().to_string())])),
            }
        }
        for (_, mut l) in lines {
            l.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
            let line: Vec<&str> = l.iter().map(|(_, t)| t.as_str()).collect();
            out.push_str(&line.join(" "));
            out.push('\n');
        }
        out.push('\n');
    }
    Ok(out)
}

pub fn parse_pdf(path: &str) -> Result<ParsedPdf> {
    let mut text = extract_text_layout(path)?;
    if text.trim().len() < 200 {
        // fallback: reading-order extraction (e.g. unusual content streams)
        text = pdf_extract::extract_text(path)
            .with_context(|| format!("Could not extract text from '{path}' (scanned PDFs need OCR)"))?;
    }
    let split_re = Regex::new(r"(?i)proforma\s+invoice").unwrap();
    let mut invoices = Vec::new();
    for chunk in split_re.split(&text).skip(1) {
        if let Some(inv) = parse_invoice_chunk(chunk) {
            invoices.push(inv);
        }
    }
    if invoices.is_empty() {
        bail!("No invoice line items found in '{path}'. The PDF format may not be supported.");
    }
    Ok(ParsedPdf {
        source_path: path.to_string(),
        invoices,
    })
}
