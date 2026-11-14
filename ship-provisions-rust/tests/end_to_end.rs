use ship_provisions::checklist::{export_checklist, import_stocktake};
use ship_provisions::db::Database;
use ship_provisions::pdf::parse_pdf;
use ship_provisions::reports::{consumption_report, export_report};

fn samples() -> Option<Vec<String>> {
    let dir = std::env::var("SHIP_PROVISIONS_SAMPLES").ok()?;
    let d = std::path::Path::new(&dir);
    if !d.exists() {
        return None;
    }
    let mut v: Vec<String> = std::fs::read_dir(d)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path().to_string_lossy().to_string())
        .filter(|p| p.ends_with(".pdf"))
        .collect();
    v.sort();
    if v.is_empty() { None } else { Some(v) }
}

#[test]
fn end_to_end_with_real_pdfs() {
    let pdfs = match samples() {
        Some(p) => p,
        None => return,
    };
    let dir = std::env::temp_dir().join(format!("sp-e2e-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let db = Database::open(Some(dir.join("test.db"))).unwrap();
    db.set_setting("ship_name", "Aquarius G").unwrap();

    let mut total_lines = 0usize;
    for pdf in &pdfs {
        let parsed = parse_pdf(pdf).unwrap();
        let (added, skipped) = db.add_parsed_pdf(&parsed).unwrap();
        assert_eq!(added, parsed.invoices.len());
        assert_eq!(skipped, 0);
        total_lines += parsed.invoices.iter().map(|i| i.lines.len()).sum::<usize>();
    }
    assert_eq!(db.list_invoices().unwrap().len(), 8);
    assert_eq!(total_lines, 260 + 12 + 2 + 284 + 1 + 2 + 39 + 4);

    db.add_crew("2026-09-22", 18).unwrap();
    db.add_crew("2026-10-08", 20).unwrap();

    let cl = dir.join("checklist.csv");
    let cl_s = cl.to_string_lossy().to_string();
    export_checklist(&db, &cl_s).unwrap();
    // crew counts half of what was delivered for every product line
    let mut rdr = csv::ReaderBuilder::new().has_headers(false).from_path(&cl_s).unwrap();
    let mut rows: Vec<Vec<String>> = Vec::new();
    for rec in rdr.records() {
        rows.push(rec.unwrap().iter().map(|s| s.to_string()).collect());
    }
    drop(rdr);
    let mut wtr = csv::Writer::from_path(&cl_s).unwrap();
    for mut row in rows {
        while row.len() < 9 {
            row.push(String::new());
        }
        if row[0] != "Product" {
            let delivered: f64 = row[3].parse().unwrap_or(0.0);
            row[5] = format!("{:.3}", (delivered * 0.5).max(0.0)); // Counted qty
            row[6] = "PCS".to_string();
        }
        wtr.write_record(&row).unwrap();
    }
    wtr.flush().unwrap();
    let (_st, n) = import_stocktake(&db, &cl_s, "2026-10-12", "2nd Cook", "").unwrap();
    assert!(n > 400);

    let rep = consumption_report(&db, Some("2026-09-22"), Some("2026-10-12")).unwrap();
    assert!(rep.person_days > 0.0);
    assert!((rep.total_delivered_cost - (10009.80 + 13424.02)).abs() < 0.05);
    assert!(rep.cost_per_person_day.is_some());

    let report = dir.join("report.xlsx");
    export_report(&db, &report.to_string_lossy(), Some("2026-09-22"), Some("2026-10-12")).unwrap();
    assert!(report.exists());

    // verify the workbook has the 8 sheets
    let mut wb: calamine::Xlsx<_> = calamine::open_workbook(&report).unwrap();
    use calamine::Reader;
    let names: Vec<String> = wb.sheet_names().to_vec();
    assert_eq!(names, vec![
        "Summary", "Consumption per person", "Deliveries", "Delivery lines",
        "Stocktakes", "Stocktake lines", "Crew on board", "Current inventory"
    ]);
    let deliv = wb.worksheet_range("Deliveries").unwrap();
    assert_eq!(deliv.rows().count(), 9); // header + 8 invoices
}
