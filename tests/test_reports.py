from ship_provisions.db import Database
from ship_provisions.models import Invoice, InvoiceLine, ParsedPdf
from ship_provisions.reports import consumption_report, per_delivery_report


def make_db() -> Database:
    db = Database(":memory:")
    parsed = ParsedPdf(source_path="test.pdf", invoices=[
        Invoice(invoice_date="2026-09-01", delivery_date="2026-09-01",
                order_no="O1", reference="PROVISIONS", grand_total=300.0, lines=[
                    InvoiceLine("1", "BEEF TENDERLOIN", 10.0, "KGS", 20.0, 200.0),
                    InvoiceLine("2", "MINERAL WATER STILL", 100.0, "BOXES", 1.0, 100.0),
                ]),
        Invoice(invoice_date="2026-09-15", delivery_date="2026-09-15",
                order_no="O2", reference="PROVISIONS", grand_total=100.0, lines=[
                    InvoiceLine("1", "BEEF TENDERLOIN", 5.0, "KGS", 20.0, 100.0),
                ]),
    ])
    db.add_parsed_pdf(parsed)
    db.add_crew("2026-09-01", 10)
    db.add_crew("2026-09-16", 20)
    return db


def test_consumption_between_stocktakes():
    db = make_db()
    beef_id = db.conn.execute(
        "SELECT id FROM products WHERE name='BEEF TENDERLOIN'").fetchone()["id"]
    water_id = db.conn.execute(
        "SELECT id FROM products WHERE name='MINERAL WATER STILL'").fetchone()["id"]
    # stocktake 1 (Sep 10): 4 kg beef, 40 boxes water on board
    db.add_stocktake("2026-09-10", "Cook", "", "st1.xlsx", [
        (beef_id, 4.0, "KGS", ""), (water_id, 40.0, "BOXES", ""),
    ])
    # stocktake 2 (Sep 20): 1 kg beef, 10 boxes water remain
    db.add_stocktake("2026-09-20", "Cook", "", "st2.xlsx", [
        (beef_id, 1.0, "KGS", ""), (water_id, 10.0, "BOXES", ""),
    ])
    rep = consumption_report(db, "2026-09-10", "2026-09-20")
    beef = next(r for r in rep["rows"] if r["product_id"] == beef_id)
    water = next(r for r in rep["rows"] if r["product_id"] == water_id)
    # beef: start 4 + delivered 5 (Sep 15) - end 1 = 8 kg consumed
    assert beef["consumed"] == 8.0
    assert beef["consumed_cost"] == 160.0
    # water: 40 + 0 - 10 = 30 boxes consumed, valued at 1.0 EUR
    assert water["consumed"] == 30.0
    assert water["consumed_cost"] == 30.0
    # person-days Sep10..Sep20: Sep10-15 -> 10 persons (6 days), Sep16-19 -> 20 (4 days)
    assert rep["person_days"] == 6 * 10 + 4 * 20
    assert rep["avg_persons"] == 14.0
    assert rep["cost_per_person_day"] == round(190.0 / 140.0, 2)
    assert beef["consumed_per_person_day"] == round(8.0 / 140.0, 4)


def test_per_delivery_cost_per_person():
    db = make_db()
    rows = per_delivery_report(db)
    by_order = {r["order_no"]: r for r in rows}
    assert by_order["O1"]["total"] == 300.0
    assert by_order["O1"]["persons"] == 10
    assert by_order["O1"]["cost_per_person"] == 30.0
    assert by_order["O2"]["persons"] == 10  # Sep 15 -> latest entry <= Sep 15 is 10
    assert by_order["O2"]["cost_per_person"] == 10.0


def test_duplicate_import_skipped():
    db = make_db()
    parsed = ParsedPdf(source_path="test.pdf", invoices=[
        Invoice(invoice_date="2026-09-01", delivery_date="2026-09-01",
                order_no="O1", reference="PROVISIONS", grand_total=300.0, lines=[
                    InvoiceLine("1", "BEEF TENDERLOIN", 10.0, "KGS", 20.0, 200.0),
                ]),
    ])
    added, skipped = db.add_parsed_pdf(parsed)
    assert (added, skipped) == (0, 1)
    assert len(db.list_invoices()) == 2
