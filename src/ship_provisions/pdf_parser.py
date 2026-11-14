"""Parse ship-supplier proforma invoice PDFs (SHIPKO layout, plus generic fallback).

A single PDF may contain several proforma invoices (e.g. PROVISIONS, CABIN,
BONDED). Each invoice starts with the heading "Proforma invoice". Line items
look like:

    12 BEEF TENDERLOIN 3/4 UP (NZ) 26,84 KGS 22,44 EUR 602,29

i.e. ref | description | quantity | unit | unit price | total, with Dutch
number formatting (comma decimals, dot thousands).
"""

from __future__ import annotations

import re
from datetime import datetime

from .models import Invoice, InvoiceLine, ParsedPdf


class PdfParseError(Exception):
    pass


_LINE_RE = re.compile(
    r"^\s*(\d{1,4})\s+(.+?)\s+(\d[\d.,]*)\s+([A-Za-z][A-Za-z0-9/]{0,11})\s+"
    r"(\d[\d.,]*)\s+(\d[\d.,]*)\s*$"
)
_DATE = r"\d{1,2}-\d{1,2}-\d{4}"
_RE_DATE_DEBTOR = re.compile(rf"^({_DATE})\s+(\d{{4,6}}D)\b", re.M)
_RE_DELIVERY_ROW = re.compile(
    rf"^({_DATE})\s+(\d{{6,8}})\s+(\d{{6,7}})\s+Your reference\s+(.+?)\s*$", re.M
)
_RE_PORT = re.compile(r"Port\s+([A-ZÀ-Þ][A-ZÀ-Þ ,]*?)(?:\s+Your reference|\n|$)", re.M)
_RE_ORDER = re.compile(r"Your order\s+([A-Z0-9][A-Z0-9\-]+)")
_RE_REF_ALT = re.compile(r"Your reference\s+([A-Za-z][A-Za-z ]*?)\s*(?:\n|$)", re.M)
_RE_TOTAL_EXCL = re.compile(r"Total excl\. VAT\s+([\d.,]+)")
_RE_TOTAL = re.compile(r"Total\s+(?:EUR\s+)?([\d.,]+)\s*$", re.M)
_RE_SHIP = re.compile(r"^\s*([A-Z][^\n(]+?)\s*\(IMO", re.M)


def parse_number(s: str) -> float:
    s = s.strip()
    if "," in s:
        s = s.replace(".", "").replace(",", ".")
    return float(s)


def parse_date(s: str) -> str:
    s = s.strip()
    return datetime.strptime(s, "%d-%m-%Y").date().isoformat()


def _extract_text_pdfplumber(path: str) -> str:
    import pdfplumber

    parts = []
    with pdfplumber.open(path) as pdf:
        for page in pdf.pages:
            parts.append(page.extract_text() or "")
    return "\n".join(parts)


def _extract_text_pymupdf(path: str) -> str:
    import pymupdf

    parts = []
    with pymupdf.open(path) as doc:
        for page in doc:
            parts.append(page.get_text())
    return "\n".join(parts)


def _extract_text_ocr(path: str) -> str:
    """Rasterize pages with PyMuPDF and OCR them with pytesseract (optional)."""
    try:
        import pytesseract
        import pymupdf
        from PIL import Image
        import io
    except ImportError as exc:
        raise PdfParseError(
            "This PDF appears to be scanned (no text layer) and OCR support is "
            "not installed. Install Tesseract OCR or use a text-based PDF."
        ) from exc
    parts = []
    with pymupdf.open(path) as doc:
        for page in doc:
            pix = page.get_pixmap(dpi=300)
            img = Image.open(io.BytesIO(pix.tobytes("png")))
            parts.append(pytesseract.image_to_string(img))
    return "\n".join(parts)


def extract_text(path: str) -> str:
    text = _extract_text_pdfplumber(path)
    if len(text.strip()) < 200:
        text = _extract_text_pymupdf(path)
    if len(text.strip()) < 200:
        text = _extract_text_ocr(path)
    return text


def _parse_line(line: str) -> InvoiceLine | None:
    line = re.sub(r"\bEUR\b", " ", line)
    m = _LINE_RE.match(line)
    if not m:
        return None
    ref, desc, qty, unit, price, total = m.groups()
    desc = re.sub(r"\s+", " ", desc).strip()
    if not desc:
        return None
    try:
        return InvoiceLine(
            ref=ref,
            description=desc,
            quantity=parse_number(qty),
            unit=unit.upper(),
            unit_price=parse_number(price),
            total=parse_number(total),
        )
    except ValueError:
        return None


def _parse_invoice_chunk(chunk: str) -> Invoice | None:
    lines: list[InvoiceLine] = []
    for raw in chunk.splitlines():
        item = _parse_line(raw)
        if item is not None:
            lines.append(item)
    if not lines:
        return None

    inv = Invoice(lines=lines)

    m = _RE_DATE_DEBTOR.search(chunk)
    if m:
        inv.invoice_date = parse_date(m.group(1))
        inv.debtor = m.group(2)

    m = _RE_DELIVERY_ROW.search(chunk)
    if m:
        inv.delivery_date = parse_date(m.group(1))
        inv.order_no = m.group(2)
        inv.packing_list = m.group(3)
        inv.reference = m.group(4).strip().upper()

    if not inv.reference:
        m = _RE_REF_ALT.search(chunk)
        if m:
            inv.reference = m.group(1).strip().upper()

    m = _RE_PORT.search(chunk)
    if m:
        inv.port = re.sub(r"\s+", " ", m.group(1)).strip()

    m = _RE_ORDER.search(chunk)
    if m:
        inv.your_order = m.group(1).strip()

    m = _RE_SHIP.search(chunk)
    if m:
        inv.ship = m.group(1).strip()

    m = _RE_TOTAL_EXCL.search(chunk)
    if m:
        inv.total_excl_vat = parse_number(m.group(1))

    totals = _RE_TOTAL.findall(chunk)
    if totals:
        inv.grand_total = parse_number(totals[-1])

    return inv


def parse_pdf(path: str) -> ParsedPdf:
    text = extract_text(path)
    chunks = re.split(r"(?i)proforma\s+invoice", text)
    invoices: list[Invoice] = []
    for chunk in chunks[1:]:
        inv = _parse_invoice_chunk(chunk)
        if inv is not None:
            invoices.append(inv)
    if not invoices:
        raise PdfParseError(
            f"No invoice line items found in '{path}'. "
            "The PDF format may not be supported."
        )
    return ParsedPdf(source_path=path, invoices=invoices)
