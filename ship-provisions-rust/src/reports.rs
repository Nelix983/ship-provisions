//! Insights: consumption and cost per person, and Excel report export.

use anyhow::{Context, Result};
use chrono::Local;
use rust_xlsxwriter::{Color, Format, Workbook, XlsxError};

use crate::db::{Database, DeliveredRow};
use crate::models::round2;

#[derive(Debug, Clone)]
pub struct ConsumptionRow {
    pub product_id: i64,
    pub product: String,
    pub category: String,
    pub unit: String,
    pub start_stock: f64,
    pub delivered: f64,
    pub end_stock: f64,
    pub consumed: f64,
    pub avg_price: f64,
    pub consumed_cost: f64,
    pub delivered_cost: f64,
    pub consumed_per_person_day: Option<f64>,
    pub cost_per_person_day: Option<f64>,
    pub consumed_per_person: Option<f64>,
    pub cost_per_person: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct Report {
    pub start: Option<String>,
    pub end: Option<String>,
    pub days: i64,
    pub person_days: f64,
    pub avg_persons: f64,
    pub total_delivered_cost: f64,
    pub total_consumed_cost: f64,
    pub cost_per_person_day: Option<f64>,
    pub purchased_per_person_day: Option<f64>,
    pub cost_per_person: Option<f64>,
    pub rows: Vec<ConsumptionRow>,
}

pub fn consumption_report(db: &Database, start: Option<&str>, end: Option<&str>) -> Result<Report> {
    let products = db.list_products()?;
    let delivered = db.delivered_between(start, end)?;
    let delivered_map: std::collections::HashMap<i64, DeliveredRow> =
        delivered.into_iter().map(|d| (d.product_id, d)).collect();

    let mut rows: Vec<ConsumptionRow> = Vec::new();
    let mut total_consumed_cost = 0.0;
    let mut total_delivered_cost = 0.0;
    let today = Local::now().format("%Y-%m-%d").to_string();
    for p in &products {
        let (d_qty, d_cost) = delivered_map
            .get(&p.id)
            .map(|d| (d.qty, d.cost))
            .unwrap_or((0.0, 0.0));
        let mut s_start = start.and_then(|s| db.stock_at(p.id, s));
        let mut s_end = end.and_then(|e| db.stock_at(p.id, e));
        if start.is_some() && s_start.is_none() {
            s_start = Some(0.0);
        }
        if end.is_some() && s_end.is_none() {
            s_end = Some(0.0);
        }
        if start.is_none() && end.is_none() {
            s_start = Some(0.0);
            s_end = db.stock_at(p.id, &today).or(Some(0.0));
        }
        let s_start = s_start.unwrap_or(0.0);
        let s_end = s_end.unwrap_or(0.0);
        let consumed = s_start + d_qty - s_end;
        let avg_price = db.weighted_avg_price(p.id);
        let cost = consumed * avg_price;
        total_consumed_cost += cost;
        total_delivered_cost += d_cost;
        rows.push(ConsumptionRow {
            product_id: p.id,
            product: p.name.clone(),
            category: p.category.clone(),
            unit: p.default_unit.clone(),
            start_stock: round3(s_start),
            delivered: round3(d_qty),
            end_stock: round3(s_end),
            consumed: round3(consumed),
            avg_price: (avg_price * 10000.0).round() / 10000.0,
            consumed_cost: round2(cost),
            delivered_cost: round2(d_cost),
            consumed_per_person_day: None,
            cost_per_person_day: None,
            consumed_per_person: None,
            cost_per_person: None,
        });
    }
    rows.sort_by(|a, b| b.consumed_cost.partial_cmp(&a.consumed_cost).unwrap());

    let (person_days, avg_persons, days) = match (start, end) {
        (Some(s), Some(e)) => {
            let (pd, ap) = db.person_days(s, e);
            let d = chrono::NaiveDate::parse_from_str(e, "%Y-%m-%d")
                .ok()
                .and_then(|e| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok().map(|s| (e - s).num_days()))
                .unwrap_or(0);
            (pd, ap, d)
        }
        _ => {
            let (mut a, mut b) = db.date_range_invoices();
            if a.is_none() {
                let t = Local::now().format("%Y-%m-%d").to_string();
                a = Some(t.clone());
                b = Some(t);
            }
            if let Ok(sts) = db.list_stocktakes() {
                if let Some(first) = sts.last() {
                    match &a {
                        Some(a0) if first.day < *a0 => a = Some(first.day.clone()),
                        None => a = Some(first.day.clone()),
                        _ => {}
                    }
                }
            }
            let (a, b) = (a.unwrap(), b.unwrap());
            let (pd, ap) = if a < b { db.person_days(&a, &b) } else { (0.0, 0.0) };
            let days = chrono::NaiveDate::parse_from_str(&b, "%Y-%m-%d")
                .ok()
                .and_then(|b| chrono::NaiveDate::parse_from_str(&a, "%Y-%m-%d").ok().map(|a| (b - a).num_days()))
                .unwrap_or(1);
            (pd, ap, days.max(1))
        }
    };

    for r in &mut rows {
        r.consumed_per_person_day = if person_days > 0.0 { Some((r.consumed / person_days * 10000.0).round() / 10000.0) } else { None };
        r.cost_per_person_day = if person_days > 0.0 { Some((r.consumed_cost / person_days * 10000.0).round() / 10000.0) } else { None };
        r.consumed_per_person = if avg_persons > 0.0 { Some((r.consumed / avg_persons * 10000.0).round() / 10000.0) } else { None };
        r.cost_per_person = if avg_persons > 0.0 { Some(round2(r.consumed_cost / avg_persons)) } else { None };
    }

    Ok(Report {
        start: start.map(|s| s.to_string()),
        end: end.map(|e| e.to_string()),
        days,
        person_days: (person_days * 100.0).round() / 100.0,
        avg_persons: (avg_persons * 100.0).round() / 100.0,
        total_delivered_cost: round2(total_delivered_cost),
        total_consumed_cost: round2(total_consumed_cost),
        cost_per_person_day: if person_days > 0.0 { Some(round2(total_consumed_cost / person_days)) } else { None },
        purchased_per_person_day: if person_days > 0.0 { Some(round2(total_delivered_cost / person_days)) } else { None },
        cost_per_person: if avg_persons > 0.0 { Some(round2(total_consumed_cost / avg_persons)) } else { None },
        rows,
    })
}

fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

#[derive(Debug, Clone)]
pub struct DeliveryReportRow {
    pub date: String,
    pub order_no: String,
    pub reference: String,
    pub port: String,
    pub lines: i64,
    pub total: f64,
    pub persons: Option<i64>,
    pub cost_per_person: Option<f64>,
}

pub fn per_delivery_report(db: &Database) -> Result<Vec<DeliveryReportRow>> {
    let mut out = Vec::new();
    for inv in db.list_invoices()? {
        let day = inv.delivery_date.clone().or(inv.invoice_date.clone()).unwrap_or_default();
        let persons = if day.is_empty() { None } else { db.persons_on(&day) };
        let total = inv.grand_total.unwrap_or(0.0);
        out.push(DeliveryReportRow {
            date: day,
            order_no: inv.order_no,
            reference: inv.reference,
            port: inv.port,
            lines: inv.line_count,
            total: round2(total),
            persons,
            cost_per_person: persons.filter(|p| *p > 0).map(|p| round2(total / p as f64)),
        });
    }
    Ok(out)
}

// --------------------------------------------------------------------- Excel
pub fn export_report(db: &Database, path: &str, start: Option<&str>, end: Option<&str>) -> Result<()> {
    let ship = db.get_setting("ship_name");
    let rep = consumption_report(db, start, end)?;
    let per_delivery = per_delivery_report(db)?;
    let invoices = db.list_invoices()?;

    let mut wb = Workbook::new();
    let header = Format::new()
        .set_bold()
        .set_font_color(Color::RGB(0xFFFFFF))
        .set_background_color(Color::RGB(0x1F4E78));
    let title = Format::new().set_bold().set_font_size(14.0);
    let bold = Format::new().set_bold();
    let money = Format::new().set_num_format("#,##0.00");

    fn sheet<'a>(wb: &'a mut Workbook, name: &str) -> Result<&'a mut rust_xlsxwriter::Worksheet> {
    let ws = wb.add_worksheet();
    ws.set_name(name)?;
    Ok(ws)
}

    // ---- Summary
    let ws = sheet(&mut wb, "Summary")?;
    ws.write_string_with_format(0, 0, "Ship provisions report", &title)?;
    let rows_kv: [(&str, String); 11] = [
        ("Ship", ship),
        ("Period", format!("{}  to  {}", rep.start.clone().unwrap_or_else(|| "beginning".into()), rep.end.clone().unwrap_or_else(|| "today".into()))),
        ("Days in period", rep.days.to_string()),
        ("Person-days on board", format!("{:.2}", rep.person_days)),
        ("Average persons on board", format!("{:.2}", rep.avg_persons)),
        ("Purchased (deliveries) EUR", rep.total_delivered_cost.to_string()),
        ("Consumed value EUR", rep.total_consumed_cost.to_string()),
        ("Cost per person (period) EUR", rep.cost_per_person.map(|v| v.to_string()).unwrap_or_default()),
        ("Cost per person-day EUR", rep.cost_per_person_day.map(|v| v.to_string()).unwrap_or_default()),
        ("Purchased per person-day EUR", rep.purchased_per_person_day.map(|v| v.to_string()).unwrap_or_default()),
        ("", String::new()),
    ];
    for (i, (k, v)) in rows_kv.iter().enumerate() {
        let r = (i + 2) as u32;
        ws.write_string_with_format(r, 0, *k, &bold)?;
        ws.write_string(r, 1, v)?;
    }
    ws.set_column_width(0, 32.0)?;
    ws.set_column_width(1, 40.0)?;

    // ---- Consumption per person
    let ws = sheet(&mut wb, "Consumption per person")?;
    let heads = ["Product", "Category", "Unit", "Start stock", "Delivered", "End stock",
        "Consumed", "Consumed / person-day", "Consumed / person", "Avg price EUR",
        "Consumed cost EUR", "Cost / person EUR", "Cost / person-day EUR", "Delivered cost EUR"];
    for (c, h) in heads.iter().enumerate() {
        ws.write_string_with_format(0, c as u16, *h, &header)?;
    }
    for (i, r) in rep.rows.iter().enumerate() {
        let row = (i + 1) as u32;
        ws.write_string(row, 0, &r.product)?;
        ws.write_string(row, 1, &r.category)?;
        ws.write_string(row, 2, &r.unit)?;
        ws.write_number_with_format(row, 3, r.start_stock, &money)?;
        ws.write_number_with_format(row, 4, r.delivered, &money)?;
        ws.write_number_with_format(row, 5, r.end_stock, &money)?;
        ws.write_number_with_format(row, 6, r.consumed, &money)?;
        if let Some(v) = r.consumed_per_person_day { ws.write_number(row, 7, v)?; }
        if let Some(v) = r.consumed_per_person { ws.write_number(row, 8, v)?; }
        ws.write_number_with_format(row, 9, r.avg_price, &money)?;
        ws.write_number_with_format(row, 10, r.consumed_cost, &money)?;
        if let Some(v) = r.cost_per_person { ws.write_number_with_format(row, 11, v, &money)?; }
        if let Some(v) = r.cost_per_person_day { ws.write_number_with_format(row, 12, v, &money)?; }
        ws.write_number_with_format(row, 13, r.delivered_cost, &money)?;
    }
    let widths = [40.0, 22.0, 8.0, 12.0, 12.0, 12.0, 12.0, 18.0, 16.0, 13.0, 17.0, 15.0, 18.0, 16.0];
    for (c, w) in widths.iter().enumerate() {
        ws.set_column_width(c as u16, *w)?;
    }

    // ---- Deliveries
    let ws = sheet(&mut wb, "Deliveries")?;
    let heads = ["Delivery date", "Invoice date", "Order no", "Packing list", "Port", "Reference",
        "Your order", "Lines", "Total EUR", "Persons on board", "Cost per person EUR", "Source file"];
    for (c, h) in heads.iter().enumerate() {
        ws.write_string_with_format(0, c as u16, *h, &header)?;
    }
    for (i, d) in per_delivery.iter().enumerate() {
        let row = (i + 1) as u32;
        let inv = invoices.iter().find(|x| x.order_no == d.order_no && x.reference == d.reference);
        ws.write_string(row, 0, &d.date)?;
        ws.write_string(row, 1, inv.and_then(|x| x.invoice_date.clone()).unwrap_or_default())?;
        ws.write_string(row, 2, &d.order_no)?;
        ws.write_string(row, 3, inv.map(|x| x.packing_list.clone()).unwrap_or_default())?;
        ws.write_string(row, 4, &d.port)?;
        ws.write_string(row, 5, &d.reference)?;
        ws.write_string(row, 6, inv.map(|x| x.your_order.clone()).unwrap_or_default())?;
        ws.write_number(row, 7, d.lines as f64)?;
        ws.write_number_with_format(row, 8, d.total, &money)?;
        if let Some(p) = d.persons { ws.write_number(row, 9, p as f64)?; }
        if let Some(v) = d.cost_per_person { ws.write_number_with_format(row, 10, v, &money)?; }
        ws.write_string(row, 11, inv.map(|x| x.source_file.clone()).unwrap_or_default())?;
    }
    for (c, w) in [14.0, 13.0, 12.0, 13.0, 20.0, 16.0, 22.0, 8.0, 12.0, 16.0, 18.0, 40.0].iter().enumerate() {
        ws.set_column_width(c as u16, *w)?;
    }

    // ---- Delivery lines
    let ws = sheet(&mut wb, "Delivery lines")?;
    let heads = ["Delivery date", "Order no", "Reference", "Line", "Description", "Category",
        "Qty", "Unit", "Unit price EUR", "Total EUR"];
    for (c, h) in heads.iter().enumerate() {
        ws.write_string_with_format(0, c as u16, *h, &header)?;
    }
    let mut row = 1u32;
    for inv in &invoices {
        let day = inv.delivery_date.clone().or(inv.invoice_date.clone()).unwrap_or_default();
        for l in db.invoice_lines(inv.id)? {
            ws.write_string(row, 0, &day)?;
            ws.write_string(row, 1, &inv.order_no)?;
            ws.write_string(row, 2, &inv.reference)?;
            ws.write_string(row, 3, l.line_ref.clone().unwrap_or_default())?;
            ws.write_string(row, 4, &l.description)?;
            ws.write_string(row, 5, &l.category)?;
            ws.write_number_with_format(row, 6, l.quantity, &money)?;
            ws.write_string(row, 7, &l.unit)?;
            ws.write_number_with_format(row, 8, l.unit_price, &money)?;
            ws.write_number_with_format(row, 9, l.total, &money)?;
            row += 1;
        }
    }
    for (c, w) in [14.0, 12.0, 14.0, 7.0, 55.0, 22.0, 10.0, 9.0, 14.0, 12.0].iter().enumerate() {
        ws.set_column_width(c as u16, *w)?;
    }

    // ---- Stocktakes
    let ws = sheet(&mut wb, "Stocktakes")?;
    let heads = ["Date", "Counted by", "Lines", "Source file", "Notes"];
    for (c, h) in heads.iter().enumerate() {
        ws.write_string_with_format(0, c as u16, *h, &header)?;
    }
    for (i, st) in db.list_stocktakes()?.iter().enumerate() {
        let row = (i + 1) as u32;
        ws.write_string(row, 0, &st.day)?;
        ws.write_string(row, 1, &st.counted_by)?;
        ws.write_number(row, 2, st.line_count as f64)?;
        ws.write_string(row, 3, &st.source_file)?;
        ws.write_string(row, 4, &st.notes)?;
    }
    for (c, w) in [12.0, 18.0, 8.0, 40.0, 30.0].iter().enumerate() {
        ws.set_column_width(c as u16, *w)?;
    }

    // ---- Stocktake lines
    let ws = sheet(&mut wb, "Stocktake lines")?;
    let heads = ["Stocktake date", "Product", "Category", "Counted qty", "Unit", "Notes"];
    for (c, h) in heads.iter().enumerate() {
        ws.write_string_with_format(0, c as u16, *h, &header)?;
    }
    let mut row = 1u32;
    for st in db.list_stocktakes()? {
        for l in db.stocktake_lines(st.id)? {
            ws.write_string(row, 0, &st.day)?;
            ws.write_string(row, 1, &l.name)?;
            ws.write_string(row, 2, &l.category)?;
            ws.write_number_with_format(row, 3, l.counted_qty, &money)?;
            ws.write_string(row, 4, &l.unit)?;
            ws.write_string(row, 5, &l.notes)?;
            row += 1;
        }
    }
    for (c, w) in [14.0, 45.0, 22.0, 13.0, 9.0, 30.0].iter().enumerate() {
        ws.set_column_width(c as u16, *w)?;
    }

    // ---- Crew
    let ws = sheet(&mut wb, "Crew on board")?;
    ws.write_string_with_format(0, 0, "Date", &header)?;
    ws.write_string_with_format(0, 1, "Persons on board", &header)?;
    for (i, c) in db.list_crew()?.iter().enumerate() {
        let row = (i + 1) as u32;
        ws.write_string(row, 0, &c.day)?;
        ws.write_number(row, 1, c.persons as f64)?;
    }
    ws.set_column_width(0, 14.0)?;
    ws.set_column_width(1, 18.0)?;

    // ---- Current inventory
    let ws = sheet(&mut wb, "Current inventory")?;
    let heads = ["Product", "Category", "Unit", "Delivered to date", "Last counted", "Last count date"];
    for (c, h) in heads.iter().enumerate() {
        ws.write_string_with_format(0, c as u16, *h, &header)?;
    }
    for (i, p) in db.list_products()?.iter().enumerate() {
        let row = (i + 1) as u32;
        ws.write_string(row, 0, &p.name)?;
        ws.write_string(row, 1, &p.category)?;
        ws.write_string(row, 2, &p.default_unit)?;
        ws.write_number_with_format(row, 3, p.delivered_qty, &money)?;
        if let Some(v) = p.last_counted {
            ws.write_number_with_format(row, 4, v, &money)?;
        }
        ws.write_string(row, 5, p.last_count_date.clone().unwrap_or_default())?;
    }
    for (c, w) in [45.0, 22.0, 9.0, 17.0, 13.0, 15.0].iter().enumerate() {
        ws.set_column_width(c as u16, *w)?;
    }

    wb.save(path).map_err(|e: XlsxError| anyhow::anyhow!("could not save '{path}': {e}"))
        .with_context(|| format!("could not save '{path}'"))?;
    Ok(())
}
