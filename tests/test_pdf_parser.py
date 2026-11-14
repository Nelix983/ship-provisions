import os

import pytest

from ship_provisions.pdf_parser import parse_pdf

UPLOADS = "/mnt/vibefs/uploads/02ea6c0a-5d64-48a8-9974-5be2823efa02"
PDF_0910 = os.path.join(UPLOADS, "jMPKtAqk-Aquarius-G Proforma invoices 09-10.pdf")
PDF_2309 = os.path.join(UPLOADS, "rOgCwhhE-Aquarius-G Proforma invoices 23-09.pdf")

pytestmark = pytest.mark.skipif(
    not (os.path.exists(PDF_0910) and os.path.exists(PDF_2309)),
    reason="sample invoice PDFs not available",
)


def test_pdf_0910_invoices_and_totals():
    parsed = parse_pdf(PDF_0910)
    assert len(parsed.invoices) == 3
    refs = [i.reference for i in parsed.invoices]
    assert refs == ["PROVISIONS", "CABIN", "CABIN"]
    totals = [round(i.total, 2) for i in parsed.invoices]
    assert totals == [9563.93, 391.87, 54.00]
    for inv in parsed.invoices:
        assert abs(inv.lines_total - inv.total) < 0.02
        assert inv.invoice_date == "2026-10-12"
        assert inv.delivery_date == "2026-10-08"
    assert parsed.invoices[0].order_no == "26010879"
    assert parsed.invoices[0].packing_list == "2648782"
    assert parsed.invoices[0].port
    assert len(parsed.invoices[0].lines) == 260


def test_pdf_2309_invoices_and_totals():
    parsed = parse_pdf(PDF_2309)
    assert len(parsed.invoices) == 5
    totals = {i.reference: round(i.total, 2) for i in parsed.invoices}
    assert totals == {
        "PROVISIONS": 10490.46,
        "COTTON RAGS": 67.00,
        "WORKING CLOTHES": 50.65,
        "CABIN": 1505.51,
        "BONDED": 1310.40,
    }
    prov = parsed.invoices[0]
    assert prov.invoice_date == "2026-09-25"
    assert prov.delivery_date == "2026-09-22"
    assert prov.order_no == "26010342"
    assert prov.your_order == "ANT-ARG-2026-1263-1"
    assert len(prov.lines) == 284
    # spot-check a line incl. Dutch thousand separator
    transport = next(l for l in prov.lines if l.description == "TRANSPORT")
    assert transport.quantity == 1.0
    assert transport.unit_price == 1200.0
    assert transport.total == 1200.0
    beef = next(l for l in prov.lines if "BEEF TENDERLOIN" in l.description)
    assert beef.quantity == 27.30
    assert beef.unit == "KGS"
    assert beef.unit_price == 22.44
    assert abs(beef.total - 612.61) < 0.01
