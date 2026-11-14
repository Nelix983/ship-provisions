"""Inventory checklist: export a fillable Excel/CSV for the crew, and import the
filled-in file back as a stocktake."""

from __future__ import annotations

import csv
import os
import re
from datetime import date

from .categories import normalize_name
from .db import Database

CHECKLIST_HEADERS = [
    "Product", "Category", "Unit", "Delivered to date", "On board (system est.)",
    "Counted qty", "Counted unit", "Checked by", "Notes",
]


def export_checklist(db: Database, path: str) -> int:
    """Write the checklist to .xlsx or .csv (chosen by extension). Returns row count."""
    products = db.list_products()
    rows = []
    for p in products:
        rows.append([
            p["name"], p["category"], p["default_unit"] or "",
            round(p["delivered_qty"], 3),
            (p["last_counted"] if p["last_counted"] is not None else ""),
            "", "", "", "",
        ])
    ext = os.path.splitext(path)[1].lower()
    if ext == ".csv":
        with open(path, "w", newline="", encoding="utf-8-sig") as fh:
            w = csv.writer(fh)
            w.writerow(CHECKLIST_HEADERS)
            w.writerows(rows)
    else:
        from openpyxl import Workbook
        from openpyxl.styles import Font, PatternFill
        from openpyxl.utils import get_column_letter

        wb = Workbook()
        ws = wb.active
        ws.title = "Checklist"
        ws.append(CHECKLIST_HEADERS)
        for c in ws[1]:
            c.font = Font(bold=True, color="FFFFFF")
            c.fill = PatternFill("solid", fgColor="1F4E78")
        for r in rows:
            ws.append(r)
        widths = [42, 24, 10, 18, 20, 14, 14, 14, 30]
        for i, w in enumerate(widths, start=1):
            ws.column_dimensions[get_column_letter(i)].width = w
        ws.freeze_panes = "A2"
        wb.save(path)
    return len(rows)


def _to_float(value) -> float | None:
    if value is None:
        return None
    if isinstance(value, (int, float)):
        return float(value)
    s = str(value).strip().replace(" ", "")
    if not s:
        return None
    s = s.replace(".", "").replace(",", ".") if ("," in s and "." in s) else s.replace(",", ".")
    try:
        return float(s)
    except ValueError:
        return None


def import_stocktake(db: Database, path: str, day: str | None = None,
                     counted_by: str = "", notes: str = "") -> tuple[int, int]:
    """Read a filled checklist (.xlsx or .csv). Returns (stocktake_id, line count).

    Rows are matched to products by the Product column (case-insensitive); unknown
    products are created. Only rows with a numeric Counted qty are recorded.
    """
    if day is None:
        day = date.today().isoformat()
    date.fromisoformat(day)  # validate

    headers, rows = _read_table(path)
    col = {h.strip().lower(): i for i, h in enumerate(headers)}
    required = {"product", "counted qty"}
    missing = required - set(col)
    if missing:
        raise ValueError(
            f"File '{os.path.basename(path)}' is missing column(s): {', '.join(sorted(missing))}. "
            f"Expected headers: {', '.join(CHECKLIST_HEADERS)}"
        )

    parsed_rows: list[tuple[int, float, str, str]] = []
    created = 0
    for r in rows:
        name = str(r[col["product"]]).strip() if col["product"] < len(r) and r[col["product"]] else ""
        if not name:
            continue
        qty = _to_float(r[col["counted qty"]] if col["counted qty"] < len(r) else None)
        if qty is None:
            continue
        unit = ""
        if "counted unit" in col and col["counted unit"] < len(r) and r[col["counted unit"]]:
            unit = str(r[col["counted unit"]]).strip().upper()
        note = ""
        if "notes" in col and col["notes"] < len(r) and r[col["notes"]]:
            note = str(r[col["notes"]]).strip()
        norm = normalize_name(name)
        existing = db.conn.execute(
            "SELECT id FROM products WHERE name=?", (norm,)
        ).fetchone()
        if existing:
            pid = existing["id"]
        else:
            pid = db.get_or_create_product(name, unit)
            created += 1
        if not unit:
            row = db.conn.execute(
                "SELECT default_unit FROM products WHERE id=?", (pid,)
            ).fetchone()
            unit = row["default_unit"] if row else ""
        parsed_rows.append((pid, qty, unit, note))

    if not parsed_rows:
        raise ValueError("No rows with a filled 'Counted qty' were found in the file.")
    st_id = db.add_stocktake(day, counted_by, notes, os.path.basename(path), parsed_rows)
    return st_id, len(parsed_rows)


def _read_table(path: str) -> tuple[list[str], list[list]]:
    ext = os.path.splitext(path)[1].lower()
    if ext == ".csv":
        with open(path, newline="", encoding="utf-8-sig") as fh:
            data = list(csv.reader(fh))
        if not data:
            raise ValueError("Empty CSV file.")
        return data[0], data[1:]
    from openpyxl import load_workbook

    wb = load_workbook(path, data_only=True)
    ws = wb.active
    data = [[("" if c is None else c) for c in row] for row in ws.iter_rows(values_only=True)]
    if not data:
        raise ValueError("Empty Excel file.")
    return [str(h) for h in data[0]], data[1:]
