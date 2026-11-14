use ship_provisions::db::Database;
use ship_provisions::models::{Invoice, InvoiceLine, ParsedPdf};
use ship_provisions::reports::{consumption_report, per_delivery_report};

fn make_db() -> Database {
    let db = Database::open_memory().unwrap();
    let parsed = ParsedPdf {
        source_path: "test.pdf".into(),
        invoices: vec![
            Invoice {
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
            },
            Invoice {
                invoice_date: "2026-09-15".into(),
                delivery_date: "2026-09-15".into(),
                order_no: "O2".into(),
                reference: "PROVISIONS".into(),
                grand_total: Some(100.0),
                lines: vec![
                    InvoiceLine { line_ref: "1".into(), description: "BEEF TENDERLOIN".into(), quantity: 5.0, unit: "KGS".into(), unit_price: 20.0, total: 100.0 },
                ],
                ..Default::default()
            },
        ],
    };
    db.add_parsed_pdf(&parsed).unwrap();
    db.add_crew("2026-09-01", 10).unwrap();
    db.add_crew("2026-09-16", 20).unwrap();
    db
}

fn product_id(db: &Database, name: &str) -> i64 {
    db.conn
        .query_row("SELECT id FROM products WHERE name=?1", [name], |r| r.get(0))
        .unwrap()
}

#[test]
fn consumption_between_stocktakes() {
    let db = make_db();
    let beef = product_id(&db, "BEEF TENDERLOIN");
    let water = product_id(&db, "MINERAL WATER STILL");
    db.add_stocktake("2026-09-10", "Cook", "", "st1.xlsx", &[(beef, 4.0, "KGS".into(), String::new()), (water, 40.0, "BOXES".into(), String::new())]).unwrap();
    db.add_stocktake("2026-09-20", "Cook", "", "st2.xlsx", &[(beef, 1.0, "KGS".into(), String::new()), (water, 10.0, "BOXES".into(), String::new())]).unwrap();
    let rep = consumption_report(&db, Some("2026-09-10"), Some("2026-09-20")).unwrap();
    let beef_r = rep.rows.iter().find(|r| r.product_id == beef).unwrap();
    let water_r = rep.rows.iter().find(|r| r.product_id == water).unwrap();
    assert_eq!(beef_r.consumed, 8.0);
    assert_eq!(beef_r.consumed_cost, 160.0);
    assert_eq!(water_r.consumed, 30.0);
    assert_eq!(water_r.consumed_cost, 30.0);
    assert_eq!(rep.person_days, 140.0);
    assert_eq!(rep.avg_persons, 14.0);
    assert_eq!(beef_r.consumed_per_person_day, Some(((8.0f64 / 140.0) * 10000.0).round() / 10000.0));
}

#[test]
fn per_delivery_cost_per_person() {
    let db = make_db();
    let rows = per_delivery_report(&db).unwrap();
    let o1 = rows.iter().find(|r| r.order_no == "O1").unwrap();
    let o2 = rows.iter().find(|r| r.order_no == "O2").unwrap();
    assert_eq!(o1.total, 300.0);
    assert_eq!(o1.persons, Some(10));
    assert_eq!(o1.cost_per_person, Some(30.0));
    assert_eq!(o2.persons, Some(10));
    assert_eq!(o2.cost_per_person, Some(10.0));
}

#[test]
fn duplicate_import_skipped() {
    let db = make_db();
    let parsed = ParsedPdf {
        source_path: "test.pdf".into(),
        invoices: vec![Invoice {
            invoice_date: "2026-09-01".into(),
            delivery_date: "2026-09-01".into(),
            order_no: "O1".into(),
            reference: "PROVISIONS".into(),
            grand_total: Some(300.0),
            lines: vec![InvoiceLine { line_ref: "1".into(), description: "BEEF TENDERLOIN".into(), quantity: 10.0, unit: "KGS".into(), unit_price: 20.0, total: 200.0 }],
            ..Default::default()
        }],
    };
    let (added, skipped) = db.add_parsed_pdf(&parsed).unwrap();
    assert_eq!((added, skipped), (0, 1));
    assert_eq!(db.list_invoices().unwrap().len(), 2);
}
