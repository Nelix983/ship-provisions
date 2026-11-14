"""SQLite storage layer. The database lives in the user's app-data folder so the
installed .exe needs no write access to its own install directory."""

from __future__ import annotations

import os
import sqlite3
from datetime import date, datetime

from .categories import categorize, normalize_name
from .models import Invoice, ParsedPdf

SCHEMA = """
CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT
);
CREATE TABLE IF NOT EXISTS products (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT UNIQUE NOT NULL,
    category TEXT NOT NULL DEFAULT 'Other',
    default_unit TEXT NOT NULL DEFAULT ''
);
CREATE TABLE IF NOT EXISTS invoices (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source_file TEXT NOT NULL,
    invoice_date TEXT,
    delivery_date TEXT,
    order_no TEXT,
    packing_list TEXT,
    port TEXT,
    reference TEXT,
    your_order TEXT,
    debtor TEXT,
    ship TEXT,
    total_excl_vat REAL,
    grand_total REAL,
    imported_at TEXT NOT NULL,
    UNIQUE(order_no, reference, source_file)
);
CREATE TABLE IF NOT EXISTS invoice_lines (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    invoice_id INTEGER NOT NULL REFERENCES invoices(id) ON DELETE CASCADE,
    line_ref TEXT,
    description TEXT NOT NULL,
    quantity REAL NOT NULL,
    unit TEXT NOT NULL,
    unit_price REAL NOT NULL,
    total REAL NOT NULL,
    product_id INTEGER REFERENCES products(id)
);
CREATE TABLE IF NOT EXISTS crew (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    day TEXT UNIQUE NOT NULL,
    persons INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS stocktakes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    day TEXT NOT NULL,
    counted_by TEXT NOT NULL DEFAULT '',
    notes TEXT NOT NULL DEFAULT '',
    source_file TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS stocktake_lines (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    stocktake_id INTEGER NOT NULL REFERENCES stocktakes(id) ON DELETE CASCADE,
    product_id INTEGER NOT NULL REFERENCES products(id),
    counted_qty REAL NOT NULL,
    unit TEXT NOT NULL DEFAULT '',
    notes TEXT NOT NULL DEFAULT ''
);
CREATE INDEX IF NOT EXISTS idx_lines_invoice ON invoice_lines(invoice_id);
CREATE INDEX IF NOT EXISTS idx_lines_product ON invoice_lines(product_id);
CREATE INDEX IF NOT EXISTS idx_stock_lines ON stocktake_lines(stocktake_id);
"""


def default_db_path() -> str:
    if os.name == "nt":
        base = os.environ.get("APPDATA") or os.path.join(
            os.path.expanduser("~"), "AppData", "Roaming"
        )
    else:
        base = os.environ.get(
            "XDG_DATA_HOME", os.path.join(os.path.expanduser("~"), ".local", "share")
        )
    folder = os.path.join(base, "ShipProvisions")
    os.makedirs(folder, exist_ok=True)
    return os.path.join(folder, "provisions.db")


class Database:
    def __init__(self, path: str | None = None):
        self.path = path or default_db_path()
        if self.path != ":memory:":
            os.makedirs(os.path.dirname(os.path.abspath(self.path)), exist_ok=True)
        self.conn = sqlite3.connect(self.path)
        self.conn.row_factory = sqlite3.Row
        self.conn.execute("PRAGMA foreign_keys = ON")
        self.conn.executescript(SCHEMA)
        self.conn.commit()

    def close(self) -> None:
        self.conn.close()

    # ------------------------------------------------------------------ settings
    def get_setting(self, key: str, default: str = "") -> str:
        row = self.conn.execute(
            "SELECT value FROM settings WHERE key=?", (key,)
        ).fetchone()
        return row["value"] if row else default

    def set_setting(self, key: str, value: str) -> None:
        self.conn.execute(
            "INSERT INTO settings(key, value) VALUES(?, ?) "
            "ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            (key, value),
        )
        self.conn.commit()

    # ------------------------------------------------------------------ products
    def get_or_create_product(self, name: str, unit: str = "", category: str = "") -> int:
        norm = normalize_name(name)
        row = self.conn.execute(
            "SELECT id, default_unit, category FROM products WHERE name=?", (norm,)
        ).fetchone()
        if row:
            updates, params = [], []
            if unit and not row["default_unit"]:
                updates.append("default_unit=?")
                params.append(unit.upper())
            if category and row["category"] == "Other":
                updates.append("category=?")
                params.append(category)
            if updates:
                params.append(row["id"])
                self.conn.execute(
                    f"UPDATE products SET {', '.join(updates)} WHERE id=?", params
                )
                self.conn.commit()
            return row["id"]
        cur = self.conn.execute(
            "INSERT INTO products(name, category, default_unit) VALUES(?, ?, ?)",
            (norm, category or "Other", unit.upper()),
        )
        self.conn.commit()
        return cur.lastrowid

    # ------------------------------------------------------------------ invoices
    def invoice_exists(self, source_file: str, order_no: str, reference: str) -> bool:
        row = self.conn.execute(
            "SELECT 1 FROM invoices WHERE source_file=? AND order_no=? AND reference=?",
            (os.path.basename(source_file), order_no, reference),
        ).fetchone()
        return row is not None

    def add_parsed_pdf(self, parsed: ParsedPdf) -> tuple[int, int]:
        """Store all invoices of a parsed PDF. Returns (added, skipped)."""
        added = skipped = 0
        src = os.path.basename(parsed.source_path)
        for inv in parsed.invoices:
            if self.invoice_exists(src, inv.order_no, inv.reference):
                skipped += 1
                continue
            cur = self.conn.execute(
                """INSERT INTO invoices(source_file, invoice_date, delivery_date,
                       order_no, packing_list, port, reference, your_order, debtor,
                       ship, total_excl_vat, grand_total, imported_at)
                   VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?)""",
                (
                    src, inv.invoice_date, inv.delivery_date, inv.order_no,
                    inv.packing_list, inv.port, inv.reference, inv.your_order,
                    inv.debtor, inv.ship, inv.total_excl_vat, inv.grand_total,
                    datetime.now().isoformat(timespec="seconds"),
                ),
            )
            invoice_id = cur.lastrowid
            for line in inv.lines:
                pid = self.get_or_create_product(
                    line.description, line.unit, categorize(line.description, inv.reference)
                )
                self.conn.execute(
                    """INSERT INTO invoice_lines(invoice_id, line_ref, description,
                           quantity, unit, unit_price, total, product_id)
                       VALUES(?,?,?,?,?,?,?,?)""",
                    (invoice_id, line.ref, line.description, line.quantity,
                     line.unit, line.unit_price, line.total, pid),
                )
            added += 1
        self.conn.commit()
        return added, skipped

    def list_invoices(self) -> list[sqlite3.Row]:
        return self.conn.execute(
            """SELECT i.*, (SELECT COUNT(*) FROM invoice_lines l
                            WHERE l.invoice_id = i.id) AS line_count
               FROM invoices i ORDER BY COALESCE(delivery_date, invoice_date) DESC, i.id DESC"""
        ).fetchall()

    def invoice_lines(self, invoice_id: int) -> list[sqlite3.Row]:
        return self.conn.execute(
            """SELECT l.*, p.name AS product_name, p.category
               FROM invoice_lines l LEFT JOIN products p ON p.id = l.product_id
               WHERE l.invoice_id=? ORDER BY CAST(l.line_ref AS INTEGER)""",
            (invoice_id,),
        ).fetchall()

    def list_products(self) -> list[sqlite3.Row]:
        return self.conn.execute(
            """SELECT p.*,
                      COALESCE((SELECT SUM(l.quantity) FROM invoice_lines l
                                WHERE l.product_id = p.id), 0) AS delivered_qty,
                      COALESCE((SELECT s.counted_qty FROM stocktake_lines s
                                JOIN stocktakes st ON st.id = s.stocktake_id
                                WHERE s.product_id = p.id
                                ORDER BY st.day DESC, st.id DESC LIMIT 1), NULL) AS last_counted,
                      (SELECT st.day FROM stocktake_lines s
                       JOIN stocktakes st ON st.id = s.stocktake_id
                       WHERE s.product_id = p.id
                       ORDER BY st.day DESC, st.id DESC LIMIT 1) AS last_count_date
               FROM products p ORDER BY p.category, p.name"""
        ).fetchall()

    def delivered_between(self, start: str | None, end: str | None) -> list[sqlite3.Row]:
        q = """SELECT p.id AS product_id, p.name, p.default_unit, l.unit,
                      SUM(l.quantity) AS qty, SUM(l.total) AS cost
               FROM invoice_lines l
               JOIN invoices i ON i.id = l.invoice_id
               JOIN products p ON p.id = l.product_id
               WHERE 1=1"""
        params: list = []
        if start:
            q += " AND COALESCE(i.delivery_date, i.invoice_date) >= ?"
            params.append(start)
        if end:
            q += " AND COALESCE(i.delivery_date, i.invoice_date) <= ?"
            params.append(end)
        q += " GROUP BY p.id ORDER BY p.name"
        return self.conn.execute(q, params).fetchall()

    def weighted_avg_price(self, product_id: int) -> float:
        row = self.conn.execute(
            """SELECT SUM(total) AS t, SUM(quantity) AS q FROM invoice_lines
               WHERE product_id=? AND quantity > 0""",
            (product_id,),
        ).fetchone()
        if row and row["q"]:
            return row["t"] / row["q"]
        row = self.conn.execute(
            "SELECT unit_price FROM invoice_lines WHERE product_id=? "
            "ORDER BY id DESC LIMIT 1",
            (product_id,),
        ).fetchone()
        return row["unit_price"] if row else 0.0

    # ------------------------------------------------------------------ crew
    def add_crew(self, day: str, persons: int) -> None:
        self.conn.execute(
            "INSERT INTO crew(day, persons) VALUES(?, ?) "
            "ON CONFLICT(day) DO UPDATE SET persons=excluded.persons",
            (day, int(persons)),
        )
        self.conn.commit()

    def delete_crew(self, day: str) -> None:
        self.conn.execute("DELETE FROM crew WHERE day=?", (day,))
        self.conn.commit()

    def list_crew(self) -> list[sqlite3.Row]:
        return self.conn.execute("SELECT * FROM crew ORDER BY day").fetchall()

    def persons_on(self, day: str) -> int | None:
        """Persons on board on a day: latest entry on/before that day, else earliest."""
        row = self.conn.execute(
            "SELECT persons FROM crew WHERE day <= ? ORDER BY day DESC LIMIT 1", (day,)
        ).fetchone()
        if row:
            return row["persons"]
        row = self.conn.execute(
            "SELECT persons FROM crew ORDER BY day LIMIT 1"
        ).fetchone()
        return row["persons"] if row else None

    def person_days(self, start: str, end: str) -> tuple[float, float]:
        """(person-days, avg persons) for the half-open interval [start, end)."""
        d0 = date.fromisoformat(start)
        d1 = date.fromisoformat(end)
        if d1 <= d0:
            return 0.0, 0.0
        days = (d1 - d0).days
        total = 0.0
        cur = d0
        from datetime import timedelta

        while cur < d1:
            p = self.persons_on(cur.isoformat())
            total += p if p else 0
            cur += timedelta(days=1)
        return total, (total / days if days else 0.0)

    # ------------------------------------------------------------------ stocktakes
    def add_stocktake(self, day: str, counted_by: str, notes: str, source_file: str,
                      rows: list[tuple[int, float, str, str]]) -> int:
        cur = self.conn.execute(
            """INSERT INTO stocktakes(day, counted_by, notes, source_file, created_at)
               VALUES(?,?,?,?,?)""",
            (day, counted_by, notes, source_file, datetime.now().isoformat(timespec="seconds")),
        )
        st_id = cur.lastrowid
        for product_id, qty, unit, note in rows:
            self.conn.execute(
                """INSERT INTO stocktake_lines(stocktake_id, product_id, counted_qty, unit, notes)
                   VALUES(?,?,?,?,?)""",
                (st_id, product_id, qty, unit, note),
            )
        self.conn.commit()
        return st_id

    def list_stocktakes(self) -> list[sqlite3.Row]:
        return self.conn.execute(
            """SELECT s.*, (SELECT COUNT(*) FROM stocktake_lines l
                            WHERE l.stocktake_id = s.id) AS line_count
               FROM stocktakes s ORDER BY s.day DESC, s.id DESC"""
        ).fetchall()

    def stock_at(self, product_id: int, day: str) -> float | None:
        row = self.conn.execute(
            """SELECT s.counted_qty FROM stocktake_lines s
               JOIN stocktakes st ON st.id = s.stocktake_id
               WHERE s.product_id=? AND st.day <= ?
               ORDER BY st.day DESC, st.id DESC LIMIT 1""",
            (product_id, day),
        ).fetchone()
        return row["counted_qty"] if row else None

    def stocktake_lines(self, stocktake_id: int) -> list[sqlite3.Row]:
        return self.conn.execute(
            """SELECT s.counted_qty, s.unit, s.notes, p.name, p.category, p.default_unit
               FROM stocktake_lines s JOIN products p ON p.id = s.product_id
               WHERE s.stocktake_id=? ORDER BY p.category, p.name""",
            (stocktake_id,),
        ).fetchall()

    def date_range_invoices(self) -> tuple[str | None, str | None]:
        row = self.conn.execute(
            "SELECT MIN(COALESCE(delivery_date, invoice_date)) AS a, "
            "MAX(COALESCE(delivery_date, invoice_date)) AS b FROM invoices"
        ).fetchone()
        return (row["a"], row["b"]) if row else (None, None)
