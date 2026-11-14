"""Tkinter desktop GUI for the ship provisions tracker."""

from __future__ import annotations

import os
import tkinter as tk
from datetime import date
from tkinter import filedialog, messagebox, ttk

from . import __version__
from .checklist import export_checklist, import_stocktake
from .db import Database, default_db_path
from .models import ParsedPdf
from .pdf_parser import PdfParseError, parse_pdf
from .reports import consumption_report, export_report, per_delivery_report


def _fmt(v, nd=2):
    if v is None:
        return ""
    return f"{v:,.{nd}f}"


class ReviewDialog(tk.Toplevel):
    """Show parsed invoices so the user can pick which to import."""

    def __init__(self, parent, parsed: ParsedPdf):
        super().__init__(parent)
        self.title("Review parsed invoices")
        self.parsed = parsed
        self.selected = {i: True for i in range(len(parsed.invoices))}
        self.result: list[int] | None = None

        cols = ("sel", "inv_date", "deliv", "order", "ref", "port", "lines", "total")
        self.tree = ttk.Treeview(self, columns=cols, show="headings", height=10)
        for c, w in zip(cols, (40, 110, 110, 100, 130, 160, 60, 110)):
            self.tree.heading(c, text={
                "sel": "Import?", "inv_date": "Invoice date", "deliv": "Delivery date",
                "order": "Order no", "ref": "Reference", "port": "Port",
                "lines": "Lines", "total": "Total EUR",
            }[c])
            self.tree.column(c, width=w, anchor="w" if c in ("port", "ref") else "e")
        for i, inv in enumerate(parsed.invoices):
            self.tree.insert("", "end", iid=str(i), values=(
                "yes", inv.invoice_date, inv.delivery_date, inv.order_no,
                inv.reference, inv.port, len(inv.lines), f"{inv.total:,.2f}",
            ))
        self.tree.bind("<Button-1>", self._toggle)
        self.tree.pack(fill="both", expand=True, padx=8, pady=8)

        btns = ttk.Frame(self)
        btns.pack(fill="x", padx=8, pady=(0, 8))
        ttk.Button(btns, text="Import selected", command=self._ok).pack(side="right")
        ttk.Button(btns, text="Cancel", command=self._cancel).pack(side="right", padx=6)
        self.transient(parent)
        self.grab_set()
        self.wait_window()

    def _toggle(self, event):
        item = self.tree.identify_row(event.y)
        col = self.tree.identify_column(event.x)
        if item and col == "#1":
            self.selected[int(item)] = not self.selected[int(item)]
            self.tree.set(item, "sel", "yes" if self.selected[int(item)] else "no")

    def _ok(self):
        self.result = [i for i, v in self.selected.items() if v]
        self.destroy()

    def _cancel(self):
        self.result = None
        self.destroy()


class LinesDialog(tk.Toplevel):
    """Generic read-only table of dict rows: columns = [(key, header, width, fmt)]."""

    def __init__(self, parent, title: str, rows, columns):
        super().__init__(parent)
        self.title(title)
        ids = [c[0] for c in columns]
        tree = ttk.Treeview(self, columns=ids, show="headings")
        for key, header, width, fmt in columns:
            tree.heading(key, text=header)
            tree.column(key, width=width, anchor="w" if fmt is None else "e")
        for r in rows:
            tree.insert("", "end", values=[
                fmt(r[key]) if fmt else r[key] for key, _, _, fmt in columns
            ])
        tree.pack(fill="both", expand=True)
        self.geometry(f"{min(1100, sum(c[2] for c in columns) + 40)}x500")


INVOICE_LINE_COLS = [
    ("line_ref", "Line", 50, None),
    ("description", "Description", 380, None),
    ("quantity", "Qty", 80, lambda v: _fmt(v, 3)),
    ("unit", "Unit", 80, None),
    ("unit_price", "Price EUR", 90, _fmt),
    ("total", "Total EUR", 100, _fmt),
    ("category", "Category", 170, None),
]

STOCKTAKE_LINE_COLS = [
    ("name", "Product", 380, None),
    ("category", "Category", 170, None),
    ("counted_qty", "Counted qty", 100, lambda v: _fmt(v, 3)),
    ("unit", "Unit", 80, None),
    ("notes", "Notes", 200, None),
]


class App(tk.Tk):
    def __init__(self, db_path: str | None = None):
        super().__init__()
        self.title(f"Ship Provisions Tracker v{__version__}")
        self.geometry("1150x700")
        self.db = Database(db_path)

        self.status = ttk.Label(self, text="", anchor="w", relief="sunken")
        self.status.pack(fill="x", side="bottom")

        nb = ttk.Notebook(self)
        nb.pack(fill="both", expand=True, padx=6, pady=6)
        self.tab_deliveries = ttk.Frame(nb)
        self.tab_inventory = ttk.Frame(nb)
        self.tab_crew = ttk.Frame(nb)
        self.tab_insights = ttk.Frame(nb)
        nb.add(self.tab_deliveries, text="Deliveries (PDF import)")
        nb.add(self.tab_inventory, text="Inventory & checklist")
        nb.add(self.tab_crew, text="Crew on board")
        nb.add(self.tab_insights, text="Insights & Excel export")

        self._build_deliveries()
        self._build_inventory()
        self._build_crew()
        self._build_insights()
        self._refresh_all()

    # ------------------------------------------------------------- deliveries
    def _build_deliveries(self):
        top = ttk.Frame(self.tab_deliveries)
        top.pack(fill="x", padx=6, pady=6)
        ttk.Button(top, text="Import PDF invoice(s)...",
                   command=self.import_pdfs).pack(side="left")
        ttk.Label(top, text="  Ship name:").pack(side="left")
        self.ship_var = tk.StringVar(value=self.db.get_setting("ship_name", ""))
        ttk.Entry(top, textvariable=self.ship_var, width=24).pack(side="left")
        ttk.Button(top, text="Save", command=self._save_ship).pack(side="left", padx=4)
        ttk.Button(top, text="Refresh", command=self._refresh_invoices).pack(side="left", padx=4)

        cols = ("deliv", "inv_date", "order", "ref", "port", "lines", "total", "file")
        self.inv_tree = ttk.Treeview(self.tab_deliveries, columns=cols, show="headings")
        for c, w, t in (("deliv", 100, "Delivery date"), ("inv_date", 100, "Invoice date"),
                        ("order", 100, "Order no"), ("ref", 150, "Reference"),
                        ("port", 150, "Port"), ("lines", 55, "Lines"),
                        ("total", 110, "Total EUR"), ("file", 320, "Source file")):
            self.inv_tree.heading(c, text=t)
            self.inv_tree.column(c, width=w, anchor="w" if c in ("port", "ref", "file") else "e")
        self.inv_tree.pack(fill="both", expand=True, padx=6, pady=6)
        self.inv_tree.bind("<Double-1>", self._show_invoice_lines)

    def _save_ship(self):
        self.db.set_setting("ship_name", self.ship_var.get().strip())
        self._set_status("Ship name saved.")

    def import_pdfs(self):
        paths = filedialog.askopenfilenames(
            title="Select proforma invoice PDF(s)",
            filetypes=[("PDF files", "*.pdf"), ("All files", "*.*")],
        )
        if not paths:
            return
        added = skipped = 0
        for path in paths:
            try:
                parsed = parse_pdf(path)
            except PdfParseError as exc:
                messagebox.showerror("PDF not recognized", str(exc))
                continue
            dlg = ReviewDialog(self, parsed)
            if dlg.result is None:
                continue
            if not dlg.result:
                self._set_status("Nothing imported (all invoices deselected).")
                continue
            chosen = ParsedPdf(source_path=parsed.source_path,
                               invoices=[parsed.invoices[i] for i in dlg.result])
            a, s = self.db.add_parsed_pdf(chosen)
            added += a
            skipped += s
        self._set_status(f"Imported {added} invoice(s), skipped {skipped} duplicate(s).")
        self._refresh_all()

    def _show_invoice_lines(self, _event):
        sel = self.inv_tree.focus()
        if not sel or not sel.isdigit():
            return
        rows = self.db.list_invoices()
        inv = rows[int(sel)] if int(sel) < len(rows) else None
        if inv is None:
            return
        lines = self.db.invoice_lines(inv["id"])
        LinesDialog(self, f"Order {inv['order_no']} - {inv['reference']} "
                          f"({inv['delivery_date'] or inv['invoice_date']})",
                    lines, INVOICE_LINE_COLS)

    def _refresh_invoices(self):
        self.inv_tree.delete(*self.inv_tree.get_children())
        for i, inv in enumerate(self.db.list_invoices()):
            self.inv_tree.insert("", "end", iid=str(i), values=(
                inv["delivery_date"] or inv["invoice_date"] or "", inv["invoice_date"],
                inv["order_no"], inv["reference"], inv["port"], inv["line_count"],
                _fmt(inv["grand_total"]), inv["source_file"],
            ))

    # ------------------------------------------------------------- inventory
    def _build_inventory(self):
        top = ttk.Frame(self.tab_inventory)
        top.pack(fill="x", padx=6, pady=6)
        ttk.Button(top, text="Export checklist (Excel)...",
                   command=lambda: self._export_checklist(".xlsx")).pack(side="left")
        ttk.Button(top, text="Export checklist (CSV)...",
                   command=lambda: self._export_checklist(".csv")).pack(side="left", padx=4)
        ttk.Button(top, text="Import filled checklist (stocktake)...",
                   command=self._import_stocktake).pack(side="left", padx=4)
        ttk.Button(top, text="Refresh", command=self._refresh_all).pack(side="left", padx=4)
        ttk.Label(top, text="  The crew fills 'Counted qty' in the exported file; "
                            "import it back here to record a stocktake.").pack(side="left")

        paned = ttk.PanedWindow(self.tab_inventory, orient="vertical")
        paned.pack(fill="both", expand=True, padx=6, pady=6)

        f1 = ttk.Frame(paned)
        ttk.Label(f1, text="Current inventory (products known from invoices)").pack(anchor="w")
        cols = ("name", "cat", "unit", "delivered", "last_count", "last_date")
        self.prod_tree = ttk.Treeview(f1, columns=cols, show="headings")
        for c, w, t in (("name", 420, "Product"), ("cat", 190, "Category"),
                        ("unit", 80, "Unit"), ("delivered", 110, "Delivered to date"),
                        ("last_count", 110, "Last counted"), ("last_date", 110, "Count date")):
            self.prod_tree.heading(c, text=t)
            self.prod_tree.column(c, width=w, anchor="w" if c in ("name", "cat") else "e")
        self.prod_tree.pack(fill="both", expand=True)
        paned.add(f1, weight=3)

        f2 = ttk.Frame(paned)
        ttk.Label(f2, text="Stocktake history").pack(anchor="w")
        cols2 = ("day", "by", "lines", "file", "notes")
        self.st_tree = ttk.Treeview(f2, columns=cols2, show="headings", height=6)
        for c, w, t in (("day", 110, "Date"), ("by", 160, "Counted by"),
                        ("lines", 60, "Lines"), ("file", 320, "Source file"),
                        ("notes", 250, "Notes")):
            self.st_tree.heading(c, text=t)
            self.st_tree.column(c, width=w, anchor="w")
        self.st_tree.pack(fill="both", expand=True)
        self.st_tree.bind("<Double-1>", self._show_stocktake_lines)
        paned.add(f2, weight=1)

    def _export_checklist(self, ext: str):
        path = filedialog.asksaveasfilename(
            title="Save checklist for the crew",
            defaultextension=ext,
            initialfile=f"checklist_{date.today().isoformat()}{ext}",
            filetypes=[("Excel" if ext == ".xlsx" else "CSV", f"*{ext}")],
        )
        if not path:
            return
        n = export_checklist(self.db, path)
        self._set_status(f"Checklist with {n} products written to {os.path.basename(path)}.")
        messagebox.showinfo("Checklist exported",
                            f"Checklist with {n} products saved to:\n{path}\n\n"
                            "Give this file to the crew; they fill in 'Counted qty', "
                            "then use 'Import filled checklist' to record the stocktake.")

    def _import_stocktake(self):
        path = filedialog.askopenfilename(
            title="Select filled checklist (xlsx/csv)",
            filetypes=[("Excel", "*.xlsx"), ("CSV", "*.csv"), ("All files", "*.*")],
        )
        if not path:
            return
        day = self._ask_date("Stocktake date", date.today().isoformat())
        if day is None:
            return
        by = self._ask_text("Counted by (name/rank)", "")
        if by is None:
            return
        try:
            st_id, n = import_stocktake(self.db, path, day=day, counted_by=by)
        except ValueError as exc:
            messagebox.showerror("Could not import stocktake", str(exc))
            return
        self._set_status(f"Stocktake #{st_id} recorded: {n} products counted on {day} by {by}.")
        self._refresh_all()

    def _show_stocktake_lines(self, _event):
        sel = self.st_tree.focus()
        if not sel or not sel.isdigit():
            return
        rows = self.db.list_stocktakes()
        st = rows[int(sel)] if int(sel) < len(rows) else None
        if st is None:
            return
        LinesDialog(self, f"Stocktake {st['day']} by {st['counted_by']}",
                    self.db.stocktake_lines(st["id"]), STOCKTAKE_LINE_COLS)

    def _refresh_inventory(self):
        self.prod_tree.delete(*self.prod_tree.get_children())
        for p in self.db.list_products():
            self.prod_tree.insert("", "end", values=(
                p["name"], p["category"], p["default_unit"], _fmt(p["delivered_qty"], 3),
                _fmt(p["last_counted"], 3) if p["last_counted"] is not None else "",
                p["last_count_date"] or "",
            ))
        self.st_tree.delete(*self.st_tree.get_children())
        for i, st in enumerate(self.db.list_stocktakes()):
            self.st_tree.insert("", "end", iid=str(i), values=(
                st["day"], st["counted_by"], st["line_count"], st["source_file"], st["notes"],
            ))

    # ------------------------------------------------------------- crew
    def _build_crew(self):
        top = ttk.Frame(self.tab_crew)
        top.pack(fill="x", padx=6, pady=6)
        ttk.Label(top, text="Date (YYYY-MM-DD):").pack(side="left")
        self.crew_day = ttk.Entry(top, width=12)
        self.crew_day.insert(0, date.today().isoformat())
        self.crew_day.pack(side="left", padx=4)
        ttk.Label(top, text="Persons on board:").pack(side="left", padx=(10, 0))
        self.crew_n = ttk.Spinbox(top, from_=1, to=999, width=6)
        self.crew_n.set(20)
        self.crew_n.pack(side="left", padx=4)
        ttk.Button(top, text="Add / update", command=self._add_crew).pack(side="left", padx=4)
        ttk.Button(top, text="Delete selected", command=self._del_crew).pack(side="left", padx=4)
        ttk.Button(top, text="Refresh", command=self._refresh_crew).pack(side="left", padx=4)
        ttk.Label(top, text="  Used to compute consumption and cost per person.").pack(side="left")

        cols = ("day", "persons")
        self.crew_tree = ttk.Treeview(self.tab_crew, columns=cols, show="headings")
        self.crew_tree.heading("day", text="Date")
        self.crew_tree.heading("persons", text="Persons on board")
        self.crew_tree.column("day", width=150)
        self.crew_tree.column("persons", width=160, anchor="e")
        self.crew_tree.pack(fill="both", expand=True, padx=6, pady=6)

    def _add_crew(self):
        day = self.crew_day.get().strip()
        try:
            date.fromisoformat(day)
            n = int(self.crew_n.get())
            if n < 1:
                raise ValueError
        except ValueError:
            messagebox.showerror("Invalid input", "Enter a valid date (YYYY-MM-DD) and persons >= 1.")
            return
        self.db.add_crew(day, n)
        self._set_status(f"Crew log: {n} persons on {day}.")
        self._refresh_crew()

    def _del_crew(self):
        sel = self.crew_tree.focus()
        if not sel:
            return
        self.db.delete_crew(self.crew_tree.item(sel)["values"][0])
        self._refresh_crew()

    def _refresh_crew(self):
        self.crew_tree.delete(*self.crew_tree.get_children())
        for c in self.db.list_crew():
            self.crew_tree.insert("", "end", values=(c["day"], c["persons"]))

    # ------------------------------------------------------------- insights
    def _build_insights(self):
        top = ttk.Frame(self.tab_insights)
        top.pack(fill="x", padx=6, pady=6)
        ttk.Label(top, text="Period:").pack(side="left")
        self.period_var = tk.StringVar(value="All time")
        self.period_box = ttk.Combobox(top, textvariable=self.period_var, state="readonly", width=42)
        self.period_box.pack(side="left", padx=4)
        ttk.Button(top, text="Compute insights", command=self._compute_insights).pack(side="left", padx=4)
        ttk.Button(top, text="Export Excel report...", command=self._export_report).pack(side="left", padx=4)

        self.summary_var = tk.StringVar(value="")
        ttk.Label(self.tab_insights, textvariable=self.summary_var, justify="left",
                  relief="groove", padding=6).pack(fill="x", padx=6, pady=4)

        cols = ("product", "cat", "unit", "consumed", "per_pd", "per_p", "cost",
                "cost_p", "cost_pd")
        self.ins_tree = ttk.Treeview(self.tab_insights, columns=cols, show="headings")
        for c, w, t in (("product", 340, "Product"), ("cat", 170, "Category"),
                        ("unit", 70, "Unit"), ("consumed", 100, "Consumed"),
                        ("per_pd", 120, "Consumed/person-day"), ("per_p", 110, "Consumed/person"),
                        ("cost", 110, "Consumed cost EUR"), ("cost_p", 110, "Cost/person EUR"),
                        ("cost_pd", 120, "Cost/person-day EUR")):
            self.ins_tree.heading(c, text=t)
            self.ins_tree.column(c, width=w, anchor="w" if c in ("product", "cat") else "e")
        self.ins_tree.pack(fill="both", expand=True, padx=6, pady=6)

    def _period_options(self):
        opts = ["All time"]
        sts = sorted(self.db.list_stocktakes(), key=lambda s: s["day"])
        for a, b in zip(sts, sts[1:]):
            opts.append(f"{a['day']}  ->  {b['day']}")
        return opts

    def _selected_period(self) -> tuple[str | None, str | None]:
        v = self.period_var.get()
        if v == "All time" or "->" not in v:
            return None, None
        a, b = [x.strip() for x in v.split("->")]
        return a, b

    def _compute_insights(self):
        start, end = self._selected_period()
        rep = consumption_report(self.db, start, end)
        pd_ = rep["person_days"]
        self.summary_var.set(
            f"Period {rep['start'] or 'beginning'} -> {rep['end'] or 'today'} | "
            f"{rep['days']} days | person-days: {pd_} | avg persons: {rep['avg_persons']} | "
            f"purchased: EUR {_fmt(rep['total_delivered_cost'])} | "
            f"consumed value: EUR {_fmt(rep['total_consumed_cost'])} | "
            f"cost/person: EUR {_fmt(rep['cost_per_person'])} | "
            f"cost/person-day: EUR {_fmt(rep['cost_per_person_day'])} | "
            f"purchased/person-day: EUR {_fmt(rep['purchased_per_person_day'])}"
        )
        self.ins_tree.delete(*self.ins_tree.get_children())
        for r in rep["rows"]:
            if r["consumed"] == 0 and r["delivered"] == 0:
                continue
            self.ins_tree.insert("", "end", values=(
                r["product"], r["category"], r["unit"], _fmt(r["consumed"], 3),
                _fmt(r["consumed_per_person_day"], 4), _fmt(r["consumed_per_person"], 3),
                _fmt(r["consumed_cost"]), _fmt(r["cost_per_person"]), _fmt(r["cost_per_person_day"]),
            ))
        self._set_status(f"Insights computed for {rep['start'] or 'all time'} -> {rep['end'] or 'today'}.")

    def _export_report(self):
        start, end = self._selected_period()
        path = filedialog.asksaveasfilename(
            title="Export Excel report",
            defaultextension=".xlsx",
            initialfile=f"provisions_report_{date.today().isoformat()}.xlsx",
            filetypes=[("Excel workbook", "*.xlsx")],
        )
        if not path:
            return
        export_report(self.db, path, start, end)
        self._set_status(f"Excel report exported to {path}.")
        messagebox.showinfo("Report exported", f"Excel report saved to:\n{path}")

    # ------------------------------------------------------------- helpers
    def _ask_date(self, title, default):
        v = tk.simpledialog.askstring(title, "Date (YYYY-MM-DD):", initialvalue=default, parent=self)
        if v is None:
            return None
        try:
            return date.fromisoformat(v.strip()).isoformat()
        except ValueError:
            messagebox.showerror("Invalid date", "Use YYYY-MM-DD.")
            return None

    def _ask_text(self, title, default):
        return tk.simpledialog.askstring(title, title + ":", initialvalue=default, parent=self)

    def _set_status(self, msg):
        self.status.config(text=msg)

    def _refresh_all(self):
        self._refresh_invoices()
        self._refresh_inventory()
        self._refresh_crew()
        opts = self._period_options()
        self.period_box["values"] = opts
        if self.period_var.get() not in opts:
            self.period_var.set("All time")
        n_inv = len(self.db.list_invoices())
        n_prod = len(self.db.list_products())
        self._set_status(f"Ready. {n_inv} invoices, {n_prod} products. DB: {self.db.path}")

    def destroy(self):
        try:
            self.db.close()
        finally:
            super().destroy()


def main():
    import sys

    db_path = default_db_path()
    if len(sys.argv) > 1:
        db_path = sys.argv[1]
    app = App(db_path)
    app.mainloop()


if __name__ == "__main__":
    main()
