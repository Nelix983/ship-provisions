import os

from ship_provisions.checklist import export_checklist, import_stocktake
from ship_provisions.db import Database
from ship_provisions.models import Invoice, InvoiceLine, ParsedPdf


def make_db() -> Database:
    db = Database(":memory:")
    db.add_parsed_pdf(ParsedPdf(source_path="t.pdf", invoices=[
        Invoice(invoice_date="2026-09-01", delivery_date="2026-09-01",
                order_no="O1", reference="PROVISIONS", grand_total=100.0, lines=[
                    InvoiceLine("1", "BEEF TENDERLOIN", 10.0, "KGS", 20.0, 200.0),
                    InvoiceLine("2", "MINERAL WATER STILL", 100.0, "BOXES", 1.0, 100.0),
                ]),
    ]))
    return db


def test_checklist_roundtrip_xlsx(tmp_path):
    db = make_db()
    path = str(tmp_path / "checklist.xlsx")
    n = export_checklist(db, path)
    assert n == 2
    assert os.path.exists(path)

    from openpyxl import load_workbook

    wb = load_workbook(path)
    ws = wb.active
    rows = list(ws.iter_rows(min_row=2, values_only=True))
    assert rows[0][0] == "BEEF TENDERLOIN"
    assert rows[0][5] in (None, "")  # Counted qty empty
    # crew fills in counts
    ws.cell(row=2, column=6, value=3.5)   # counted qty beef
    ws.cell(row=3, column=6, value="60")  # counted qty water (string ok)
    wb.save(path)

    st_id, count = import_stocktake(db, path, day="2026-09-30", counted_by="Cook")
    assert count == 2
    lines = db.stocktake_lines(st_id)
    beef = next(l for l in lines if l["name"] == "BEEF TENDERLOIN")
    assert beef["counted_qty"] == 3.5
    assert db.stock_at(beef_id := db.conn.execute(
        "SELECT id FROM products WHERE name='BEEF TENDERLOIN'").fetchone()["id"],
        "2026-09-30") == 3.5


def test_checklist_roundtrip_csv(tmp_path):
    db = make_db()
    path = str(tmp_path / "checklist.csv")
    export_checklist(db, path)
    with open(path, "a", encoding="utf-8-sig", newline="") as fh:
        fh.write("BEEF TENDERLOIN,Meat & poultry,KGS,10,4,4,KGS,Cook,ok\n")
    st_id, count = import_stocktake(db, path, day="2026-10-01", counted_by="Cook")
    assert count >= 1
