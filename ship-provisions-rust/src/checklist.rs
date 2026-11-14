//! Inventory checklist: export a fillable Excel/CSV for the crew, and import the
//! filled-in file back as a stocktake.

use std::path::Path;

use anyhow::{Context, Result, bail};
use calamine::{Data, Reader};

use crate::categories::normalize_name;
use crate::db::Database;

pub const CHECKLIST_HEADERS: [&str; 9] = [
    "Product",
    "Category",
    "Unit",
    "Delivered to date",
    "On board (system est.)",
    "Counted qty",
    "Counted unit",
    "Checked by",
    "Notes",
];

pub fn export_checklist(db: &Database, path: &str) -> Result<usize> {
    let products = db.list_products()?;
    let ext = Path::new(path)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if ext == "csv" {
        let mut w = csv::Writer::from_path(path)?;
        w.write_record(CHECKLIST_HEADERS)?;
        for p in &products {
            w.write_record([
                p.name.clone(),
                p.category.clone(),
                p.default_unit.clone(),
                format!("{:.3}", p.delivered_qty),
                p.last_counted.map(|v| format!("{v:.3}")).unwrap_or_default(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
            ])?;
        }
        w.flush()?;
    } else {
        use rust_xlsxwriter::{Color, Format, Workbook};
        let mut wb = Workbook::new();
        let ws = wb.add_worksheet();
        let header = Format::new()
            .set_bold()
            .set_font_color(Color::RGB(0xFFFFFF))
            .set_background_color(Color::RGB(0x1F4E78));
        for (c, h) in CHECKLIST_HEADERS.iter().enumerate() {
            ws.write_string_with_format(0, c as u16, *h, &header)?;
        }
        let widths = [42.0, 24.0, 10.0, 18.0, 20.0, 14.0, 14.0, 14.0, 30.0];
        for (c, w) in widths.iter().enumerate() {
            ws.set_column_width(c as u16, *w)?;
        }
        for (i, p) in products.iter().enumerate() {
            let r = (i + 1) as u32;
            ws.write_string(r, 0, &p.name)?;
            ws.write_string(r, 1, &p.category)?;
            ws.write_string(r, 2, &p.default_unit)?;
            ws.write_number(r, 3, p.delivered_qty)?;
            if let Some(v) = p.last_counted {
                ws.write_number(r, 4, v)?;
            }
        }
        wb.save(path).with_context(|| format!("could not save '{path}'"))?;
    }
    Ok(products.len())
}

#[allow(dead_code)]
fn cell_f64(v: &Data) -> Option<f64> {
    match v {
        Data::Float(f) => Some(*f),
        Data::Int(i) => Some(*i as f64),
        Data::String(s) => to_float(s),
        _ => None,
    }
}

fn to_float(s: &str) -> Option<f64> {
    let t = s.trim().replace(' ', "");
    if t.is_empty() {
        return None;
    }
    let t = if t.contains(',') && t.contains('.') {
        t.replace('.', "").replace(',', ".")
    } else {
        t.replace(',', ".")
    };
    t.parse::<f64>().ok()
}

/// Reads a filled checklist (.xlsx or .csv). Returns (stocktake_id, line count).
pub fn import_stocktake(
    db: &Database,
    path: &str,
    day: &str,
    counted_by: &str,
    notes: &str,
) -> Result<(i64, usize)> {
    let ext = Path::new(path)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let table: Vec<Vec<String>> = if ext == "csv" {
        let mut r = csv::ReaderBuilder::new()
            .has_headers(false)
            .flexible(true)
            .from_path(path)?;
        r.records()
            .collect::<std::result::Result<Vec<_>, _>>()?
            .into_iter()
            .map(|rec| rec.iter().map(|s| s.to_string()).collect())
            .collect()
    } else {
        let mut wb: calamine::Xlsx<_> = calamine::open_workbook(path)
            .with_context(|| format!("could not open '{path}'"))?;
        let range = wb
            .worksheet_range_at(0)
            .ok_or_else(|| anyhow::anyhow!("workbook has no sheets"))?
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        range
            .rows()
            .map(|row| row.iter().map(|c| c.to_string()).collect())
            .collect()
    };
    if table.is_empty() {
        bail!("empty file");
    }
    let headers: Vec<String> = table[0].iter().map(|h| h.trim().to_lowercase()).collect();
    let col = |name: &str| headers.iter().position(|h| h == name);
    let c_product = col("product").ok_or_else(|| anyhow::anyhow!("missing 'Product' column"))?;
    let c_qty = col("counted qty").ok_or_else(|| anyhow::anyhow!("missing 'Counted qty' column"))?;
    let c_unit = col("counted unit");
    let c_notes = col("notes");

    let mut rows: Vec<(i64, f64, String, String)> = Vec::new();
    for r in &table[1..] {
        let name = r.get(c_product).cloned().unwrap_or_default();
        let name = name.trim().to_string();
        if name.is_empty() {
            continue;
        }
        let raw_qty = r.get(c_qty).map(|v| v.trim().to_string()).unwrap_or_default();
        let qty = match to_float(&raw_qty) {
            Some(q) => q,
            None if raw_qty.is_empty() => continue, // row not counted
            None => bail!("invalid Counted qty '{raw_qty}' for '{name}'"),
        };
        let unit = c_unit
            .and_then(|c| r.get(c))
            .map(|v| v.trim().to_uppercase())
            .unwrap_or_default();
        let note = c_notes
            .and_then(|c| r.get(c))
            .map(|v| v.trim().to_string())
            .unwrap_or_default();
        let pid = if let Some(id) = db
            .conn
            .query_row(
                "SELECT id FROM products WHERE name=?1",
                [normalize_name(&name)],
                |r| r.get::<_, i64>(0),
            )
            .ok()
        {
            id
        } else {
            db.get_or_create_product(&name, &unit, "")?
        };
        let unit = if unit.is_empty() {
            db.conn
                .query_row("SELECT default_unit FROM products WHERE id=?1", [pid], |r| {
                    r.get::<_, String>(0)
                })
                .unwrap_or_default()
        } else {
            unit
        };
        rows.push((pid, qty, unit, note));
    }
    if rows.is_empty() {
        bail!("No rows with a filled 'Counted qty' were found in the file.");
    }
    let file_name = Path::new(path)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let st_id = db.add_stocktake(day, counted_by, notes, &file_name, &rows)?;
    Ok((st_id, rows.len()))
}
