"""Insights: consumption and cost per person, and Excel report export."""

from __future__ import annotations

from datetime import date, timedelta

from .db import Database


def _fmt_qty(q: float) -> float:
    return round(q, 3)


def consumption_report(db: Database, start: str | None, end: str | None) -> dict:
    """Consumption between two stocktake dates (or overall if None).

    consumed(product) = stock_at_start + delivered_in_period - stock_at_end
    cost valued at the product's weighted-average invoice price.
    """
    products = db.list_products()
    delivered_map = {d["product_id"]: d for d in db.delivered_between(start, end)}
    rows = []
    total_consumed_cost = 0.0
    total_delivered_qty = 0.0
    total_delivered_cost = 0.0
    for p in products:
        pid = p["id"]
        d = delivered_map.get(pid)
        d_qty = (d["qty"] or 0.0) if d else 0.0
        d_cost = (d["cost"] or 0.0) if d else 0.0
        s_start = db.stock_at(pid, start) if start else None
        s_end = db.stock_at(pid, end) if end else None
        if s_start is None and start:
            s_start = 0.0  # no earlier count: assume we started from zero
        if s_end is None and end:
            s_end = 0.0
        if start is None and end is None:
            s_start = 0.0
            s_end = db.stock_at(pid, date.today().isoformat()) or 0.0
        consumed = (s_start or 0.0) + d_qty - (s_end or 0.0)
        avg_price = db.weighted_avg_price(pid)
        cost = consumed * avg_price
        total_consumed_cost += cost
        total_delivered_qty += d_qty
        total_delivered_cost += d_cost
        rows.append({
            "product_id": pid,
            "product": p["name"],
            "category": p["category"],
            "unit": p["default_unit"],
            "start_stock": _fmt_qty(s_start or 0.0),
            "delivered": _fmt_qty(d_qty),
            "end_stock": _fmt_qty(s_end or 0.0),
            "consumed": _fmt_qty(consumed),
            "avg_price": round(avg_price, 4),
            "consumed_cost": round(cost, 2),
            "delivered_cost": round(d_cost, 2),
        })
    rows.sort(key=lambda r: r["consumed_cost"], reverse=True)

    if start and end:
        person_days, avg_persons = db.person_days(start, end)
        days = (date.fromisoformat(end) - date.fromisoformat(start)).days
    else:
        a, b = db.date_range_invoices()
        if not a:
            a, b = date.today().isoformat(), date.today().isoformat()
        first_st = db.list_stocktakes()
        if first_st:
            a = min(a, first_st[-1]["day"])
        person_days, avg_persons = db.person_days(a, b) if a < b else (0.0, 0.0)
        days = (date.fromisoformat(b) - date.fromisoformat(a)).days if a < b else 1

    for r in rows:
        r["consumed_per_person_day"] = (
            round(r["consumed"] / person_days, 4) if person_days else None
        )
        r["cost_per_person_day"] = (
            round(r["consumed_cost"] / person_days, 4) if person_days else None
        )
        r["consumed_per_person"] = (
            round(r["consumed"] / avg_persons, 4) if avg_persons else None
        )
        r["cost_per_person"] = (
            round(r["consumed_cost"] / avg_persons, 2) if avg_persons else None
        )

    return {
        "start": start,
        "end": end,
        "days": days,
        "person_days": round(person_days, 2),
        "avg_persons": round(avg_persons, 2),
        "total_delivered_cost": round(total_delivered_cost, 2),
        "total_consumed_cost": round(total_consumed_cost, 2),
        "cost_per_person_day": (
            round(total_consumed_cost / person_days, 2) if person_days else None
        ),
        "purchased_per_person_day": (
            round(total_delivered_cost / person_days, 2) if person_days else None
        ),
        "cost_per_person": (
            round(total_consumed_cost / avg_persons, 2) if avg_persons else None
        ),
        "rows": rows,
    }


def per_delivery_report(db: Database) -> list[dict]:
    """Cost per person for each delivered invoice."""
    out = []
    for inv in db.list_invoices():
        day = inv["delivery_date"] or inv["invoice_date"]
        persons = db.persons_on(day) if day else None
        total = inv["grand_total"] if inv["grand_total"] is not None else 0.0
        out.append({
            "date": day or "",
            "order_no": inv["order_no"],
            "reference": inv["reference"],
            "port": inv["port"],
            "lines": inv["line_count"],
            "total": round(total, 2),
            "persons": persons,
            "cost_per_person": round(total / persons, 2) if persons else None,
        })
    return out


# --------------------------------------------------------------------- Excel
def export_report(db: Database, path: str, start: str | None = None,
                  end: str | None = None) -> None:
    from openpyxl import Workbook
    from openpyxl.styles import Alignment, Font, PatternFill
    from openpyxl.utils import get_column_letter

    ship = db.get_setting("ship_name", "")
    wb = Workbook()

    header_fill = PatternFill("solid", fgColor="1F4E78")
    header_font = Font(bold=True, color="FFFFFF")
    title_font = Font(bold=True, size=14)
    bold = Font(bold=True)

    def style_header(ws, row=1):
        for c in ws[row]:
            if c.value is not None:
                c.font = header_font
                c.fill = header_fill
        ws.freeze_panes = ws.cell(row=row + 1, column=1).coordinate

    def autowidth(ws, widths):
        for i, w in enumerate(widths, start=1):
            ws.column_dimensions[get_column_letter(i)].width = w

    # ---- Summary sheet
    ws = wb.active
    ws.title = "Summary"
    ws["A1"] = "Ship provisions report"
    ws["A1"].font = title_font
    rep = consumption_report(db, start, end)
    per_delivery = per_delivery_report(db)
    ws["A3"] = "Ship"
    ws["B3"] = ship
    ws["A4"] = "Period"
    ws["B4"] = f"{rep['start'] or 'beginning'}  to  {rep['end'] or 'today'}"
    ws["A5"] = "Days in period"
    ws["B5"] = rep["days"]
    ws["A6"] = "Person-days on board"
    ws["B6"] = rep["person_days"]
    ws["A7"] = "Average persons on board"
    ws["B7"] = rep["avg_persons"]
    ws["A9"] = "Purchased (deliveries) EUR"
    ws["B9"] = rep["total_delivered_cost"]
    ws["A10"] = "Consumed value EUR"
    ws["B10"] = rep["total_consumed_cost"]
    ws["A11"] = "Cost per person (period) EUR"
    ws["B11"] = rep["cost_per_person"]
    ws["A12"] = "Cost per person-day EUR"
    ws["B12"] = rep["cost_per_person_day"]
    ws["A13"] = "Purchased per person-day EUR"
    ws["B13"] = rep["purchased_per_person_day"]
    for r in range(3, 14):
        ws.cell(row=r, column=1).font = bold
    for r in (9, 10, 11, 12, 13):
        ws.cell(row=r, column=2).number_format = "#,##0.00"
    autowidth(ws, [32, 40])

    # ---- Consumption per person
    ws = wb.create_sheet("Consumption per person")
    headers = ["Product", "Category", "Unit", "Start stock", "Delivered",
               "End stock", "Consumed", "Consumed / person-day",
               "Consumed / person", "Avg price EUR", "Consumed cost EUR",
               "Cost / person EUR", "Cost / person-day EUR", "Delivered cost EUR"]
    ws.append(headers)
    for r in rep["rows"]:
        ws.append([
            r["product"], r["category"], r["unit"], r["start_stock"], r["delivered"],
            r["end_stock"], r["consumed"], r["consumed_per_person_day"],
            r["consumed_per_person"], r["avg_price"], r["consumed_cost"],
            r["cost_per_person"], r["cost_per_person_day"], r["delivered_cost"],
        ])
    style_header(ws)
    autowidth(ws, [40, 22, 8, 12, 12, 12, 12, 18, 16, 13, 17, 15, 18, 16])
    for row in ws.iter_rows(min_row=2, min_col=4, max_col=14):
        for c in row:
            c.number_format = "#,##0.00"

    # ---- Deliveries
    ws = wb.create_sheet("Deliveries")
    ws.append(["Delivery date", "Invoice date", "Order no", "Packing list", "Port",
               "Reference", "Your order", "Lines", "Total EUR", "Persons on board",
               "Cost per person EUR", "Source file"])
    for d in per_delivery:
        inv = next(i for i in db.list_invoices() if i["order_no"] == d["order_no"]
                   and i["reference"] == d["reference"])
        ws.append([d["date"], inv["invoice_date"], d["order_no"], inv["packing_list"],
                   d["port"], d["reference"], inv["your_order"], d["lines"],
                   d["total"], d["persons"] if d["persons"] is not None else "",
                   d["cost_per_person"] if d["cost_per_person"] is not None else "",
                   inv["source_file"]])
    style_header(ws)
    autowidth(ws, [14, 13, 12, 13, 20, 16, 22, 8, 12, 16, 18, 40])
    for row in ws.iter_rows(min_row=2, min_col=9, max_col=11):
        for c in row:
            c.number_format = "#,##0.00"

    # ---- Delivery lines
    ws = wb.create_sheet("Delivery lines")
    ws.append(["Delivery date", "Order no", "Reference", "Line", "Description",
               "Category", "Qty", "Unit", "Unit price EUR", "Total EUR"])
    for inv in db.list_invoices():
        for l in db.invoice_lines(inv["id"]):
            ws.append([inv["delivery_date"] or inv["invoice_date"], inv["order_no"],
                       inv["reference"], l["line_ref"], l["description"], l["category"],
                       l["quantity"], l["unit"], l["unit_price"], l["total"]])
    style_header(ws)
    autowidth(ws, [14, 12, 14, 7, 55, 22, 10, 9, 14, 12])
    for row in ws.iter_rows(min_row=2, min_col=9, max_col=10):
        for c in row:
            c.number_format = "#,##0.00"

    # ---- Stocktakes
    ws = wb.create_sheet("Stocktakes")
    ws.append(["Date", "Counted by", "Lines", "Source file", "Notes"])
    for st in db.list_stocktakes():
        ws.append([st["day"], st["counted_by"], st["line_count"], st["source_file"],
                   st["notes"]])
    style_header(ws)
    autowidth(ws, [12, 18, 8, 40, 30])

    # ---- Stocktake lines
    ws = wb.create_sheet("Stocktake lines")
    ws.append(["Stocktake date", "Product", "Category", "Counted qty", "Unit", "Notes"])
    for st in db.list_stocktakes():
        for l in db.stocktake_lines(st["id"]):
            ws.append([st["day"], l["name"], l["category"], l["counted_qty"],
                       l["unit"], l["notes"]])
    style_header(ws)
    autowidth(ws, [14, 45, 22, 13, 9, 30])

    # ---- Crew
    ws = wb.create_sheet("Crew on board")
    ws.append(["Date", "Persons on board"])
    for c in db.list_crew():
        ws.append([c["day"], c["persons"]])
    style_header(ws)
    autowidth(ws, [14, 18])

    # ---- Current inventory
    ws = wb.create_sheet("Current inventory")
    ws.append(["Product", "Category", "Unit", "Delivered to date", "Last counted",
               "Last count date"])
    for p in db.list_products():
        ws.append([p["name"], p["category"], p["default_unit"],
                   round(p["delivered_qty"], 3),
                   p["last_counted"] if p["last_counted"] is not None else "",
                   p["last_count_date"] or ""])
    style_header(ws)
    autowidth(ws, [45, 22, 9, 17, 13, 15])

    wb.save(path)
