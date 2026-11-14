import os

import pytest

from ship_provisions.checklist import export_checklist, import_stocktake
from ship_provisions.db import Database
from ship_provisions.pdf_parser import parse_pdf
from ship_provisions.reports import consumption_report, export_report

UPLOADS = "/mnt/vibefs/uploads/02ea6c0a-5d64-48a8-9974-5be2823efa02"
PDFS = [
    os.path.join(UPLOADS, "jMPKtAqk-Aquarius-G Proforma invoices 09-10.pdf"),
    os.path.join(UPLOADS, "rOgCwhhE-Aquarius-G Proforma invoices 23-09.pdf"),
]

pytestmark = pytest.mark.skipif(
    not all(os.path.exists(p) for p in PDFS), reason="sample invoice PDFs not available"
)


def test_end_to_end(tmp_path):
    db = Database(str(tmp_path / "test.db"))
    db.set_setting("ship_name", "Aquarius G")
    total_lines = 0
    for pdf in PDFS:
        parsed = parse_pdf(pdf)
        added, skipped = db.add_parsed_pdf(parsed)
        assert added == len(parsed.invoices)
        assert skipped == 0
        total_lines += sum(len(i.lines) for i in parsed.invoices)
    assert len(db.list_invoices()) == 8
    assert total_lines == 260 + 12 + 2 + 284 + 1 + 2 + 39 + 4

    # crew on board for the whole period
    db.add_crew("2026-09-22", 18)
    db.add_crew("2026-10-08", 20)

    # crew counts stock after the second delivery
    checklist = str(tmp_path / "checklist.xlsx")
    export_checklist(db, checklist)
    from openpyxl import load_workbook

    wb = load_workbook(checklist)
    ws = wb.active
    counted = 0
    for row in range(2, ws.max_row + 1):
        delivered = ws.cell(row=row, column=4).value or 0
        ws.cell(row=row, column=6, value=max(0, float(delivered) * 0.5))
        counted += 1
    wb.save(checklist)
    st_id, n = import_stocktake(db, checklist, day="2026-10-12", counted_by="2nd Cook")
    assert n == counted

    rep = consumption_report(db, "2026-09-22", "2026-10-12")
    assert rep["person_days"] > 0
    assert rep["total_delivered_cost"] == pytest.approx(10009.80 + 13424.02, abs=0.05)
    assert rep["cost_per_person_day"] is not None

    report = str(tmp_path / "report.xlsx")
    export_report(db, report, "2026-09-22", "2026-10-12")
    assert os.path.exists(report)
    wb2 = load_workbook(report)
    assert wb2.sheetnames == ["Summary", "Consumption per person", "Deliveries",
                              "Delivery lines", "Stocktakes", "Stocktake lines",
                              "Crew on board", "Current inventory"]
    assert wb2["Deliveries"].max_row == 9  # header + 8 invoices
    db.close()
