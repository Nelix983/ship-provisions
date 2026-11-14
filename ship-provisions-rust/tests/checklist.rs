use ship_provisions::checklist::{export_checklist, import_stocktake};
use ship_provisions::db::Database;
use ship_provisions::models::{Invoice, InvoiceLine, ParsedPdf};

fn make_db() -> Database {
    let db = Database::open_memory().unwrap();
    db.add_parsed_pdf(&ParsedPdf {
        source_path: "t.pdf".into(),
        invoices: vec![Invoice {
            invoice_date: "2026-09-01".into(),
            delivery_date: "2026-09-01".into(),
            order_no: "O1".into(),
            reference: "PROVISIONS".into(),
            grand_total: Some(300.0),
            lines: vec![
                InvoiceLine { line_ref: "1".into(), description: "BEEF TENDERLOIN".into(), quantity: 10.0, unit: "KGS".into(), unit_price: 20.0, total: 200.0 },
                InvoiceLine { line_ref: "2".into(), description: "MINERAL WATER STILL".into(), quantity: 100.0, unit: "BOXES".into(), unit_price: 1.0, total: 100.0 },
            ],
            ..Default::default()
        }],
    })
    .unwrap();
    db
}

#[test]
fn checklist_roundtrip_xlsx() {
    let db = make_db();
    let dir = std::env::temp_dir().join(format!("sp-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("checklist.xlsx");
    let path_s = path.to_string_lossy().to_string();
    let n = export_checklist(&db, &path_s).unwrap();
    assert_eq!(n, 2);

    // simulate the crew filling in "Counted qty" (column F = index 5) by rewriting
    // the file through calamine is read-only, so write a CSV instead for the import test
    let csv_path = dir.join("filled.csv");
    let csv_s = csv_path.to_string_lossy().to_string();
    export_checklist(&db, &csv_s).unwrap();
    let content = std::fs::read_to_string(&csv_s).unwrap();
    let filled = content.replacen("BEEF TENDERLOIN,Meat & poultry,KGS,10.000,,,,,", "BEEF TENDERLOIN,Meat & poultry,KGS,10.000,,3.5,KGS,Cook,ok", 1);
    std::fs::write(&csv_s, filled).unwrap();

    let (st_id, count) = import_stocktake(&db, &csv_s, "2026-09-30", "Cook", "").unwrap();
    assert_eq!(count, 1);
    let lines = db.stocktake_lines(st_id).unwrap();
    let beef = lines.iter().find(|l| l.name == "BEEF TENDERLOIN").unwrap();
    assert_eq!(beef.counted_qty, 3.5);
    let beef_id: i64 = db.conn.query_row("SELECT id FROM products WHERE name='BEEF TENDERLOIN'", [], |r| r.get(0)).unwrap();
    assert_eq!(db.stock_at(beef_id, "2026-09-30"), Some(3.5));
}

#[test]
fn checklist_roundtrip_csv() {
    let db = make_db();
    let dir = std::env::temp_dir().join(format!("sp-test-csv-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let csv_path = dir.join("checklist.csv");
    let csv_s = csv_path.to_string_lossy().to_string();
    export_checklist(&db, &csv_s).unwrap();
    let mut content = std::fs::read_to_string(&csv_s).unwrap();
    content.push_str("MINERAL WATER STILL,Dairy & eggs,BOXES,100.000,,60,BOXES,Cook,\n");
    std::fs::write(&csv_s, content).unwrap();
    let (_st, count) = import_stocktake(&db, &csv_s, "2026-10-01", "Cook", "").unwrap();
    assert!(count >= 1);
}
