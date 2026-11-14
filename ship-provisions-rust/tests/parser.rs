use ship_provisions::pdf::parse_pdf;

fn sample_dir() -> Option<String> {
    std::env::var("SHIP_PROVISIONS_SAMPLES").ok()
}

#[test]
fn parses_pdf_0910() {
    let dir = match sample_dir() {
        Some(d) => d,
        None => return, // samples not available (CI)
    };
    let p = format!("{dir}/jMPKtAqk-Aquarius-G Proforma invoices 09-10.pdf");
    if !std::path::Path::new(&p).exists() {
        return;
    }
    let parsed = parse_pdf(&p).unwrap();
    assert_eq!(parsed.invoices.len(), 3);
    let refs: Vec<&str> = parsed.invoices.iter().map(|i| i.reference.as_str()).collect();
    assert_eq!(refs, ["PROVISIONS", "CABIN", "CABIN"]);
    let totals: Vec<f64> = parsed.invoices.iter().map(|i| i.total()).collect();
    assert!((totals[0] - 9563.93).abs() < 0.01);
    assert!((totals[1] - 391.87).abs() < 0.01);
    assert!((totals[2] - 54.00).abs() < 0.01);
    for inv in &parsed.invoices {
        assert!((inv.lines_total() - inv.total()).abs() < 0.02);
        assert_eq!(inv.invoice_date, "2026-10-12");
        assert_eq!(inv.delivery_date, "2026-10-08");
    }
    assert_eq!(parsed.invoices[0].order_no, "26010879");
    assert_eq!(parsed.invoices[0].packing_list, "2648782");
    assert!(!parsed.invoices[0].port.is_empty());
    assert_eq!(parsed.invoices[0].lines.len(), 260);
}

#[test]
fn parses_pdf_2309() {
    let dir = match sample_dir() {
        Some(d) => d,
        None => return,
    };
    let p = format!("{dir}/rOgCwhhE-Aquarius-G Proforma invoices 23-09.pdf");
    if !std::path::Path::new(&p).exists() {
        return;
    }
    let parsed = parse_pdf(&p).unwrap();
    assert_eq!(parsed.invoices.len(), 5);
    let prov = &parsed.invoices[0];
    assert_eq!(prov.reference, "PROVISIONS");
    assert!((prov.total() - 10490.46).abs() < 0.01);
    assert_eq!(prov.invoice_date, "2026-09-25");
    assert_eq!(prov.delivery_date, "2026-09-22");
    assert_eq!(prov.order_no, "26010342");
    assert_eq!(prov.your_order, "ANT-ARG-2026-1263-1");
    assert_eq!(prov.lines.len(), 284);
    let transport = prov.lines.iter().find(|l| l.description == "TRANSPORT").unwrap();
    assert_eq!(transport.unit_price, 1200.0);
    assert_eq!(transport.total, 1200.0);
    let beef = prov
        .lines
        .iter()
        .find(|l| l.description.contains("BEEF TENDERLOIN"))
        .unwrap();
    assert_eq!(beef.quantity, 27.30);
    assert_eq!(beef.unit, "KGS");
    assert!((beef.total - 612.61).abs() < 0.01);
    let bonded = parsed.invoices.iter().find(|i| i.reference == "BONDED").unwrap();
    assert!((bonded.total() - 1310.40).abs() < 0.01);
}
