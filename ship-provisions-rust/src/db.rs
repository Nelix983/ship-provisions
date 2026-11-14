//! SQLite storage layer. The database lives in the user's app-data folder
//! (%APPDATA%\ShipProvisions\provisions.db on Windows).

use std::path::PathBuf;

use anyhow::Result;
use chrono::{NaiveDate, Local};
use rusqlite::{params, Connection, Row};

use crate::categories::{categorize, normalize_name};
use crate::models::ParsedPdf;

pub const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT);
CREATE TABLE IF NOT EXISTS products (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT UNIQUE NOT NULL,
    category TEXT NOT NULL DEFAULT 'Other',
    default_unit TEXT NOT NULL DEFAULT ''
);
CREATE TABLE IF NOT EXISTS invoices (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source_file TEXT NOT NULL,
    invoice_date TEXT, delivery_date TEXT, order_no TEXT, packing_list TEXT,
    port TEXT, reference TEXT, your_order TEXT, debtor TEXT, ship TEXT,
    total_excl_vat REAL, grand_total REAL, imported_at TEXT NOT NULL,
    UNIQUE(order_no, reference, source_file)
);
CREATE TABLE IF NOT EXISTS invoice_lines (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    invoice_id INTEGER NOT NULL REFERENCES invoices(id) ON DELETE CASCADE,
    line_ref TEXT, description TEXT NOT NULL, quantity REAL NOT NULL,
    unit TEXT NOT NULL, unit_price REAL NOT NULL, total REAL NOT NULL,
    product_id INTEGER REFERENCES products(id)
);
CREATE TABLE IF NOT EXISTS crew (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    day TEXT UNIQUE NOT NULL, persons INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS stocktakes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    day TEXT NOT NULL, counted_by TEXT NOT NULL DEFAULT '',
    notes TEXT NOT NULL DEFAULT '', source_file TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS stocktake_lines (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    stocktake_id INTEGER NOT NULL REFERENCES stocktakes(id) ON DELETE CASCADE,
    product_id INTEGER NOT NULL REFERENCES products(id),
    counted_qty REAL NOT NULL, unit TEXT NOT NULL DEFAULT '',
    notes TEXT NOT NULL DEFAULT ''
);
CREATE INDEX IF NOT EXISTS idx_lines_invoice ON invoice_lines(invoice_id);
CREATE INDEX IF NOT EXISTS idx_lines_product ON invoice_lines(product_id);
CREATE INDEX IF NOT EXISTS idx_stock_lines ON stocktake_lines(stocktake_id);
"#;

pub fn default_db_path() -> PathBuf {
    if cfg!(windows) {
        let base = std::env::var("APPDATA").unwrap_or_else(|_| {
            PathBuf::from(std::env::var("USERPROFILE").unwrap_or_default()).join("AppData").join("Roaming").to_string_lossy().to_string()
        });
        PathBuf::from(base).join("ShipProvisions").join("provisions.db")
    } else {
        let base = std::env::var("XDG_DATA_HOME")
            .unwrap_or_else(|_| format!("{}/.local/share", std::env::var("HOME").unwrap_or_default()));
        PathBuf::from(base).join("ShipProvisions").join("provisions.db")
    }
}

#[derive(Debug, Clone)]
pub struct InvoiceRow {
    pub id: i64,
    pub source_file: String,
    pub invoice_date: Option<String>,
    pub delivery_date: Option<String>,
    pub order_no: String,
    pub packing_list: String,
    pub port: String,
    pub reference: String,
    pub your_order: String,
    pub grand_total: Option<f64>,
    pub line_count: i64,
}

#[derive(Debug, Clone)]
pub struct InvoiceLineRow {
    pub line_ref: Option<String>,
    pub description: String,
    pub quantity: f64,
    pub unit: String,
    pub unit_price: f64,
    pub total: f64,
    pub category: String,
}

#[derive(Debug, Clone)]
pub struct ProductRow {
    pub id: i64,
    pub name: String,
    pub category: String,
    pub default_unit: String,
    pub delivered_qty: f64,
    pub last_counted: Option<f64>,
    pub last_count_date: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DeliveredRow {
    pub product_id: i64,
    pub qty: f64,
    pub cost: f64,
}

#[derive(Debug, Clone)]
pub struct CrewRow {
    pub day: String,
    pub persons: i64,
}

#[derive(Debug, Clone)]
pub struct StocktakeRow {
    pub id: i64,
    pub day: String,
    pub counted_by: String,
    pub line_count: i64,
    pub source_file: String,
    pub notes: String,
}

#[derive(Debug, Clone)]
pub struct StocktakeLineRow {
    pub name: String,
    pub category: String,
    pub counted_qty: f64,
    pub unit: String,
    pub notes: String,
}

pub struct Database {
    pub conn: Connection,
    pub path: PathBuf,
}

impl Database {
    pub fn open(path: Option<PathBuf>) -> Result<Database> {
        let path = path.unwrap_or_else(default_db_path);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let conn = Connection::open(&path)?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        conn.execute_batch(SCHEMA)?;
        Ok(Database { conn, path })
    }

    pub fn open_memory() -> Result<Database> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(SCHEMA)?;
        Ok(Database { conn, path: PathBuf::from(":memory:") })
    }

    // ------------------------------------------------------------- settings
    pub fn get_setting(&self, key: &str) -> String {
        self.conn
            .query_row("SELECT value FROM settings WHERE key=?1", params![key], |r| r.get(0))
            .unwrap_or_default()
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO settings(key, value) VALUES(?1, ?2) \
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    // ------------------------------------------------------------- products
    pub fn get_or_create_product(&self, name: &str, unit: &str, category: &str) -> Result<i64> {
        let norm = normalize_name(name);
        let existing: Option<(i64, String, String)> = self
            .conn
            .query_row(
                "SELECT id, default_unit, category FROM products WHERE name=?1",
                params![norm],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .ok();
        if let Some((id, def_unit, cat)) = existing {
            let mut updates: Vec<String> = Vec::new();
            let mut vals: Vec<String> = Vec::new();
            if !unit.is_empty() && def_unit.is_empty() {
                updates.push("default_unit=?".into());
                vals.push(unit.to_uppercase());
            }
            if !category.is_empty() && cat == "Other" {
                updates.push("category=?".into());
                vals.push(category.to_string());
            }
            if !updates.is_empty() {
                let sql = format!(
                    "UPDATE products SET {} WHERE id=?",
                    updates.join(", ")
                );
                let mut stmt = self.conn.prepare(&sql)?;
                let n = vals.len();
                let mut p: Vec<&dyn rusqlite::ToSql> =
                    vals.iter().map(|v| v as &dyn rusqlite::ToSql).collect();
                p.push(&id);
                let _ = n;
                stmt.execute(rusqlite::params_from_iter(p.iter()))?;
            }
            return Ok(id);
        }
        self.conn.execute(
            "INSERT INTO products(name, category, default_unit) VALUES(?1, ?2, ?3)",
            params![norm, if category.is_empty() { "Other" } else { category }, unit.to_uppercase()],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    // ------------------------------------------------------------- invoices
    pub fn invoice_exists(&self, source_file: &str, order_no: &str, reference: &str) -> bool {
        self.conn
            .query_row(
                "SELECT 1 FROM invoices WHERE source_file=?1 AND order_no=?2 AND reference=?3",
                params![source_file, order_no, reference],
                |_| Ok(()),
            )
            .is_ok()
    }

    /// Returns (added, skipped).
    pub fn add_parsed_pdf(&self, parsed: &ParsedPdf) -> Result<(usize, usize)> {
        let src = std::path::Path::new(&parsed.source_path)
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let mut added = 0;
        let mut skipped = 0;
        let now = Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
        for inv in &parsed.invoices {
            if self.invoice_exists(&src, &inv.order_no, &inv.reference) {
                skipped += 1;
                continue;
            }
            self.conn.execute(
                "INSERT INTO invoices(source_file, invoice_date, delivery_date, order_no, \
                 packing_list, port, reference, your_order, debtor, ship, total_excl_vat, \
                 grand_total, imported_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
                params![
                    src,
                    opt(&inv.invoice_date),
                    opt(&inv.delivery_date),
                    inv.order_no,
                    inv.packing_list,
                    inv.port,
                    inv.reference,
                    inv.your_order,
                    inv.debtor,
                    inv.ship,
                    inv.total_excl_vat,
                    inv.grand_total,
                    now
                ],
            )?;
            let invoice_id = self.conn.last_insert_rowid();
            for line in &inv.lines {
                let pid = self.get_or_create_product(
                    &line.description,
                    &line.unit,
                    &categorize(&line.description, &inv.reference),
                )?;
                self.conn.execute(
                    "INSERT INTO invoice_lines(invoice_id, line_ref, description, quantity, \
                     unit, unit_price, total, product_id) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
                    params![
                        invoice_id,
                        line.line_ref,
                        line.description,
                        line.quantity,
                        line.unit,
                        line.unit_price,
                        line.total,
                        pid
                    ],
                )?;
            }
            added += 1;
        }
        Ok((added, skipped))
    }

    pub fn list_invoices(&self) -> Result<Vec<InvoiceRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT i.id, i.source_file, i.invoice_date, i.delivery_date, i.order_no, \
             i.packing_list, i.port, i.reference, i.your_order, i.grand_total, \
             (SELECT COUNT(*) FROM invoice_lines l WHERE l.invoice_id = i.id) \
             FROM invoices i ORDER BY COALESCE(i.delivery_date, i.invoice_date) DESC, i.id DESC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(InvoiceRow {
                id: r.get(0)?,
                source_file: r.get(1)?,
                invoice_date: r.get(2)?,
                delivery_date: r.get(3)?,
                order_no: r.get(4)?,
                packing_list: r.get(5)?,
                port: r.get(6)?,
                reference: r.get(7)?,
                your_order: r.get(8)?,
                grand_total: r.get(9)?,
                line_count: r.get(10)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn invoice_lines(&self, invoice_id: i64) -> Result<Vec<InvoiceLineRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT l.line_ref, l.description, l.quantity, l.unit, l.unit_price, l.total, \
             p.category FROM invoice_lines l LEFT JOIN products p ON p.id = l.product_id \
             WHERE l.invoice_id=?1 ORDER BY CAST(l.line_ref AS INTEGER)",
        )?;
        let rows = stmt.query_map(params![invoice_id], |r| {
            Ok(InvoiceLineRow {
                line_ref: r.get(0)?,
                description: r.get(1)?,
                quantity: r.get(2)?,
                unit: r.get(3)?,
                unit_price: r.get(4)?,
                total: r.get(5)?,
                category: r.get(6)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn list_products(&self) -> Result<Vec<ProductRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT p.id, p.name, p.category, p.default_unit, \
             COALESCE((SELECT SUM(l.quantity) FROM invoice_lines l WHERE l.product_id = p.id), 0), \
             (SELECT s.counted_qty FROM stocktake_lines s JOIN stocktakes st ON st.id = s.stocktake_id \
              WHERE s.product_id = p.id ORDER BY st.day DESC, st.id DESC LIMIT 1), \
             (SELECT st.day FROM stocktake_lines s JOIN stocktakes st ON st.id = s.stocktake_id \
              WHERE s.product_id = p.id ORDER BY st.day DESC, st.id DESC LIMIT 1) \
             FROM products p ORDER BY p.category, p.name",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(ProductRow {
                id: r.get(0)?,
                name: r.get(1)?,
                category: r.get(2)?,
                default_unit: r.get(3)?,
                delivered_qty: r.get(4)?,
                last_counted: r.get(5)?,
                last_count_date: r.get(6)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn delivered_between(&self, start: Option<&str>, end: Option<&str>) -> Result<Vec<DeliveredRow>> {
        let mut q = String::from(
            "SELECT p.id, SUM(l.quantity), SUM(l.total) FROM invoice_lines l \
             JOIN invoices i ON i.id = l.invoice_id JOIN products p ON p.id = l.product_id WHERE 1=1",
        );
        if start.is_some() {
            q.push_str(" AND COALESCE(i.delivery_date, i.invoice_date) >= ?1");
        }
        if end.is_some() {
            q.push_str(if start.is_some() { " AND COALESCE(i.delivery_date, i.invoice_date) <= ?2" } else { " AND COALESCE(i.delivery_date, i.invoice_date) <= ?1" });
        }
        q.push_str(" GROUP BY p.id ORDER BY p.name");
        let mut stmt = self.conn.prepare(&q)?;
        let rows = match (start, end) {
            (Some(s), Some(e)) => stmt.query_map(params![s, e], map_delivered)?,
            (Some(s), None) => stmt.query_map(params![s], map_delivered)?,
            (None, Some(e)) => stmt.query_map(params![e], map_delivered)?,
            (None, None) => stmt.query_map([], map_delivered)?,
        };
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn weighted_avg_price(&self, product_id: i64) -> f64 {
        let r: Option<(Option<f64>, Option<f64>)> = self
            .conn
            .query_row(
                "SELECT SUM(total), SUM(quantity) FROM invoice_lines WHERE product_id=?1 AND quantity > 0",
                params![product_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .ok();
        if let Some((Some(t), Some(q))) = r {
            if q > 0.0 {
                return t / q;
            }
        }
        self.conn
            .query_row(
                "SELECT unit_price FROM invoice_lines WHERE product_id=?1 ORDER BY id DESC LIMIT 1",
                params![product_id],
                |r| r.get(0),
            )
            .unwrap_or(0.0)
    }

    // ------------------------------------------------------------- crew
    pub fn add_crew(&self, day: &str, persons: i64) -> Result<()> {
        NaiveDate::parse_from_str(day, "%Y-%m-%d").map_err(|e| anyhow::anyhow!("invalid date: {e}"))?;
        self.conn.execute(
            "INSERT INTO crew(day, persons) VALUES(?1, ?2) \
             ON CONFLICT(day) DO UPDATE SET persons=excluded.persons",
            params![day, persons],
        )?;
        Ok(())
    }

    pub fn delete_crew(&self, day: &str) -> Result<()> {
        self.conn.execute("DELETE FROM crew WHERE day=?1", params![day])?;
        Ok(())
    }

    pub fn list_crew(&self) -> Result<Vec<CrewRow>> {
        let mut stmt = self.conn.prepare("SELECT day, persons FROM crew ORDER BY day")?;
        let rows = stmt.query_map([], |r| Ok(CrewRow { day: r.get(0)?, persons: r.get(1)? }))?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Persons on board on a day: latest entry on/before that day, else earliest.
    pub fn persons_on(&self, day: &str) -> Option<i64> {
        if let Ok(p) = self.conn.query_row(
            "SELECT persons FROM crew WHERE day <= ?1 ORDER BY day DESC LIMIT 1",
            params![day],
            |r| r.get(0),
        ) {
            return Some(p);
        }
        self.conn
            .query_row("SELECT persons FROM crew ORDER BY day LIMIT 1", [], |r| r.get(0))
            .ok()
    }

    /// (person-days, avg persons) for the half-open interval [start, end).
    pub fn person_days(&self, start: &str, end: &str) -> (f64, f64) {
        let d0 = match NaiveDate::parse_from_str(start, "%Y-%m-%d") { Ok(d) => d, Err(_) => return (0.0, 0.0) };
        let d1 = match NaiveDate::parse_from_str(end, "%Y-%m-%d") { Ok(d) => d, Err(_) => return (0.0, 0.0) };
        let days = (d1 - d0).num_days();
        if days <= 0 {
            return (0.0, 0.0);
        }
        let mut total = 0.0f64;
        let mut cur = d0;
        for _ in 0..days {
            if let Some(p) = self.persons_on(&cur.format("%Y-%m-%d").to_string()) {
                total += p as f64;
            }
            cur = cur.succ_opt().unwrap();
        }
        (total, total / days as f64)
    }

    // ------------------------------------------------------------- stocktakes
    pub fn add_stocktake(
        &self,
        day: &str,
        counted_by: &str,
        notes: &str,
        source_file: &str,
        rows: &[(i64, f64, String, String)],
    ) -> Result<i64> {
        NaiveDate::parse_from_str(day, "%Y-%m-%d").map_err(|e| anyhow::anyhow!("invalid date: {e}"))?;
        let now = Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
        self.conn.execute(
            "INSERT INTO stocktakes(day, counted_by, notes, source_file, created_at) VALUES(?1,?2,?3,?4,?5)",
            params![day, counted_by, notes, source_file, now],
        )?;
        let st_id = self.conn.last_insert_rowid();
        for (pid, qty, unit, note) in rows {
            self.conn.execute(
                "INSERT INTO stocktake_lines(stocktake_id, product_id, counted_qty, unit, notes) VALUES(?1,?2,?3,?4,?5)",
                params![st_id, pid, qty, unit, note],
            )?;
        }
        Ok(st_id)
    }

    pub fn list_stocktakes(&self) -> Result<Vec<StocktakeRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT s.id, s.day, s.counted_by, s.source_file, s.notes, \
             (SELECT COUNT(*) FROM stocktake_lines l WHERE l.stocktake_id = s.id) \
             FROM stocktakes s ORDER BY s.day DESC, s.id DESC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(StocktakeRow {
                id: r.get(0)?,
                day: r.get(1)?,
                counted_by: r.get(2)?,
                source_file: r.get(3)?,
                notes: r.get(4)?,
                line_count: r.get(5)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn stock_at(&self, product_id: i64, day: &str) -> Option<f64> {
        self.conn
            .query_row(
                "SELECT s.counted_qty FROM stocktake_lines s JOIN stocktakes st ON st.id = s.stocktake_id \
                 WHERE s.product_id=?1 AND st.day <= ?2 ORDER BY st.day DESC, st.id DESC LIMIT 1",
                params![product_id, day],
                |r| r.get(0),
            )
            .ok()
    }

    pub fn stocktake_lines(&self, stocktake_id: i64) -> Result<Vec<StocktakeLineRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT p.name, p.category, s.counted_qty, s.unit, s.notes \
             FROM stocktake_lines s JOIN products p ON p.id = s.product_id \
             WHERE s.stocktake_id=?1 ORDER BY p.category, p.name",
        )?;
        let rows = stmt.query_map(params![stocktake_id], |r| {
            Ok(StocktakeLineRow {
                name: r.get(0)?,
                category: r.get(1)?,
                counted_qty: r.get(2)?,
                unit: r.get(3)?,
                notes: r.get(4)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn date_range_invoices(&self) -> (Option<String>, Option<String>) {
        self.conn
            .query_row(
                "SELECT MIN(COALESCE(delivery_date, invoice_date)), MAX(COALESCE(delivery_date, invoice_date)) FROM invoices",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap_or((None, None))
    }
}

fn opt(s: &str) -> Option<String> {
    if s.is_empty() { None } else { Some(s.to_string()) }
}

fn map_delivered(r: &Row) -> rusqlite::Result<DeliveredRow> {
    Ok(DeliveredRow {
        product_id: r.get(0)?,
        qty: r.get::<_, Option<f64>>(1)?.unwrap_or(0.0),
        cost: r.get::<_, Option<f64>>(2)?.unwrap_or(0.0),
    })
}

