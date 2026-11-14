//! Ship Provisions Tracker - egui desktop GUI (Windows .exe via cargo build --release).

use std::path::PathBuf;

use eframe::egui;
use ship_provisions::checklist::{export_checklist, import_stocktake};
use ship_provisions::db::{Database, InvoiceRow, ProductRow, StocktakeRow};
use ship_provisions::models::ParsedPdf;
use ship_provisions::pdf::parse_pdf;
use ship_provisions::reports::{consumption_report, export_report, per_delivery_report, Report};

#[derive(PartialEq, Clone, Copy)]
enum Tab {
    Deliveries,
    Inventory,
    Crew,
    Insights,
}

struct ReviewState {
    parsed: ParsedPdf,
    selected: Vec<bool>,
}

struct ImportState {
    path: String,
    day: String,
    by: String,
    notes: String,
    open: bool,
}

struct App {
    db: Database,
    tab: Tab,
    status: String,
    ship_name: String,
    invoices: Vec<InvoiceRow>,
    products: Vec<ProductRow>,
    stocktakes: Vec<StocktakeRow>,
    crew: Vec<ship_provisions::db::CrewRow>,
    review: Option<ReviewState>,
    lines_title: String,
    lines: Vec<ship_provisions::db::InvoiceLineRow>,
    lines_open: bool,
    st_lines_title: String,
    st_lines: Vec<ship_provisions::db::StocktakeLineRow>,
    st_lines_open: bool,
    imp: ImportState,
    crew_day: String,
    crew_n: String,
    period_opts: Vec<String>,
    period_idx: usize,
    report: Option<Report>,
    deliveries: Vec<ship_provisions::reports::DeliveryReportRow>,
}

impl App {
    fn new() -> Self {
        let (db, err) = match Database::open(None) {
            Ok(db) => (db, String::new()),
            Err(e) => (
                Database::open_memory().expect("memory db"),
                format!("Could not open DB at default location ({e}); using in-memory DB."),
            ),
        };
        let ship_name = db.get_setting("ship_name");
        let mut app = App {
            db,
            tab: Tab::Deliveries,
            status: err,
            ship_name,
            invoices: vec![],
            products: vec![],
            stocktakes: vec![],
            crew: vec![],
            review: None,
            lines_title: String::new(),
            lines: vec![],
            lines_open: false,
            st_lines_title: String::new(),
            st_lines: vec![],
            st_lines_open: false,
            imp: ImportState {
                path: String::new(),
                day: chrono::Local::now().format("%Y-%m-%d").to_string(),
                by: String::new(),
                notes: String::new(),
                open: false,
            },
            crew_day: chrono::Local::now().format("%Y-%m-%d").to_string(),
            crew_n: "20".to_string(),
            period_opts: vec!["All time".to_string()],
            period_idx: 0,
            report: None,
            deliveries: vec![],
        };
        app.refresh_all();
        if app.status.is_empty() {
            app.status = format!(
                "Ready. DB: {}",
                app.db.path.display()
            );
        }
        app
    }

    fn refresh_all(&mut self) {
        if let Ok(v) = self.db.list_invoices() {
            self.invoices = v;
        }
        if let Ok(v) = self.db.list_products() {
            self.products = v;
        }
        if let Ok(v) = self.db.list_stocktakes() {
            self.period_opts = vec!["All time".to_string()];
            let mut sts = v.clone();
            sts.sort_by(|a, b| a.day.cmp(&b.day));
            for w in sts.windows(2) {
                self.period_opts.push(format!("{}  ->  {}", w[0].day, w[1].day));
            }
            if self.period_idx >= self.period_opts.len() {
                self.period_idx = 0;
            }
            self.stocktakes = v;
        }
        if let Ok(v) = self.db.list_crew() {
            self.crew = v;
        }
        if let Ok(v) = per_delivery_report(&self.db) {
            self.deliveries = v;
        }
    }

    fn selected_period(&self) -> (Option<String>, Option<String>) {
        let v = &self.period_opts[self.period_idx];
        if v == "All time" || !v.contains("->") {
            return (None, None);
        }
        let parts: Vec<&str> = v.split("->").map(|s| s.trim()).collect();
        (Some(parts[0].to_string()), Some(parts[1].to_string()))
    }

    fn import_pdfs(&mut self) {
        let files = rfd::FileDialog::new()
            .add_filter("PDF files", &["pdf"])
            .set_title("Select proforma invoice PDF(s)")
            .pick_files();
        let Some(files) = files else { return };
        let mut added = 0;
        let mut skipped = 0;
        for f in files {
            let p = f.to_string_lossy().to_string();
            match parse_pdf(&p) {
                Ok(parsed) => {
                    if self.review.is_none() {
                        let n = parsed.invoices.len();
                        self.review = Some(ReviewState {
                            parsed,
                            selected: vec![true; n],
                        });
                    } else {
                        // import directly if a review is already open is confusing; queue not supported
                        let chosen: Vec<_> = parsed
                            .invoices
                            .iter()
                            .filter(|i| {
                                !self.db.invoice_exists(
                                    &PathBuf::from(&p)
                                        .file_name()
                                        .map(|s| s.to_string_lossy().to_string())
                                        .unwrap_or_default(),
                                    &i.order_no,
                                    &i.reference,
                                )
                            })
                            .cloned()
                            .collect();
                        let pp = ParsedPdf { source_path: p, invoices: chosen };
                        match self.db.add_parsed_pdf(&pp) {
                            Ok((a, s)) => {
                                added += a;
                                skipped += s;
                            }
                            Err(e) => self.status = format!("Import failed: {e}"),
                        }
                    }
                }
                Err(e) => self.status = format!("PDF not recognized: {e}"),
            }
        }
        if added + skipped > 0 {
            self.status = format!("Imported {added} invoice(s), skipped {skipped} duplicate(s).");
        }
        self.refresh_all();
    }

    fn do_review_import(&mut self) {
        if let Some(rev) = self.review.take() {
            let chosen = ParsedPdf {
                source_path: rev.parsed.source_path,
                invoices: rev
                    .parsed
                    .invoices
                    .into_iter()
                    .zip(rev.selected.iter())
                    .filter(|(_, s)| **s)
                    .map(|(i, _)| i)
                    .collect(),
            };
            match self.db.add_parsed_pdf(&chosen) {
                Ok((a, s)) => self.status = format!("Imported {a} invoice(s), skipped {s} duplicate(s)."),
                Err(e) => self.status = format!("Import failed: {e}"),
            }
            self.refresh_all();
        }
    }

    fn compute_insights(&mut self) {
        let (s, e) = self.selected_period();
        match consumption_report(&self.db, s.as_deref(), e.as_deref()) {
            Ok(r) => {
                self.status = format!(
                    "Insights: {} -> {}",
                    r.start.clone().unwrap_or_else(|| "beginning".into()),
                    r.end.clone().unwrap_or_else(|| "today".into())
                );
                self.report = Some(r);
            }
            Err(err) => self.status = format!("Insights failed: {err}"),
        }
    }

    fn export_excel(&mut self) {
        let (s, e) = self.selected_period();
        let default = format!("provisions_report_{}.xlsx", chrono::Local::now().format("%Y-%m-%d"));
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Excel workbook", &["xlsx"])
            .set_file_name(default)
            .save_file()
        else {
            return;
        };
        let p = path.to_string_lossy().to_string();
        match export_report(&self.db, &p, s.as_deref(), e.as_deref()) {
            Ok(()) => self.status = format!("Excel report exported to {p}"),
            Err(err) => self.status = format!("Export failed: {err}"),
        }
    }

    fn export_checklist_file(&mut self, ext: &str) {
        let default = format!("checklist_{}.{}", chrono::Local::now().format("%Y-%m-%d"), ext);
        let filter = if ext == "xlsx" { "Excel" } else { "CSV" };
        let Some(path) = rfd::FileDialog::new()
            .add_filter(filter, &[ext])
            .set_file_name(default)
            .save_file()
        else {
            return;
        };
        let p = path.to_string_lossy().to_string();
        match export_checklist(&self.db, &p) {
            Ok(n) => self.status = format!("Checklist with {n} products written to {p}"),
            Err(e) => self.status = format!("Checklist export failed: {e}"),
        }
    }
}


fn eur(v: f64) -> String {
    let neg = v < 0.0;
    let s = format!("{:.2}", v.abs());
    let (int, frac) = s.split_once('.').unwrap();
    let mut out = String::new();
    for (i, c) in int.chars().enumerate() {
        if i > 0 && (int.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    format!("{}{}.{}", if neg { "-" } else { "" }, out, frac)
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        // top bar with tabs
        ui.horizontal(|ui| {
            ui.heading("Ship Provisions Tracker");
            ui.separator();
            for (t, label) in [
                (Tab::Deliveries, "Deliveries (PDF import)"),
                (Tab::Inventory, "Inventory & checklist"),
                (Tab::Crew, "Crew on board"),
                (Tab::Insights, "Insights & Excel export"),
            ] {
                ui.selectable_value(&mut self.tab, t, label);
            }
            ui.separator();
            ui.label("Ship name:");
            ui.add(egui::TextEdit::singleline(&mut self.ship_name).desired_width(180.0));
            if ui.button("Save").clicked() {
                let _ = self.db.set_setting("ship_name", &self.ship_name);
                self.status = "Ship name saved.".into();
            }
        });
        ui.separator();

        // tab content
        match self.tab {
            Tab::Deliveries => self.ui_deliveries(ui),
            Tab::Inventory => self.ui_inventory(ui),
            Tab::Crew => self.ui_crew(ui),
            Tab::Insights => self.ui_insights(ui),
        }

        // status bar at the bottom
        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Status:");
            ui.label(&self.status);
        });

        // ------------------------------------------------------ dialogs
        let mut import_clicked = false;
        let mut close_review = false;
        if let Some(rev) = &mut self.review {
            let mut open = true;
            egui::Window::new("Review parsed invoices")
                .open(&mut open)
                .collapsible(false)
                .show(&ctx, |ui| {
                    egui::ScrollArea::vertical().max_height(400.0).show(ui, |ui| {
                        egui::Grid::new("rev").num_columns(8).striped(true).show(ui, |ui| {
                            for h in ["Import?", "Invoice date", "Delivery", "Order", "Ref", "Port", "Lines", "Total EUR"] {
                                ui.label(egui::RichText::new(h).strong());
                            }
                            ui.end_row();
                            for (i, inv) in rev.parsed.invoices.iter().enumerate() {
                                ui.checkbox(&mut rev.selected[i], "");
                                ui.label(&inv.invoice_date);
                                ui.label(&inv.delivery_date);
                                ui.label(&inv.order_no);
                                ui.label(&inv.reference);
                                ui.label(&inv.port);
                                ui.label(inv.lines.len().to_string());
                                ui.label(eur(inv.total()));
                                ui.end_row();
                            }
                        });
                    });
                    ui.horizontal(|ui| {
                        if ui.button("Import selected").clicked() {
                            import_clicked = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close_review = true;
                        }
                    });
                });
            if !open {
                close_review = true;
            }
        }
        if import_clicked {
            self.do_review_import();
        } else if close_review {
            self.review = None;
        }

        if self.lines_open {
            let mut open = true;
            let title = self.lines_title.clone();
            let lines = std::mem::take(&mut self.lines);
            egui::Window::new(title)
                .open(&mut open)
                .default_size([900.0, 500.0])
                .show(&ctx, |ui| {
                    egui::ScrollArea::both().show(ui, |ui| {
                        egui::Grid::new("lines").num_columns(7).striped(true).show(ui, |ui| {
                            for h in ["Line", "Description", "Qty", "Unit", "Price EUR", "Total EUR", "Category"] {
                                ui.label(egui::RichText::new(h).strong());
                            }
                            ui.end_row();
                            for l in &lines {
                                ui.label(l.line_ref.clone().unwrap_or_default());
                                ui.label(&l.description);
                                ui.label(format!("{:.3}", l.quantity));
                                ui.label(&l.unit);
                                ui.label(eur(l.unit_price));
                                ui.label(eur(l.total));
                                ui.label(&l.category);
                                ui.end_row();
                            }
                        });
                    });
                });
            self.lines = lines;
            if !open {
                self.lines_open = false;
            }
        }

        if self.st_lines_open {
            let mut open = true;
            let title = self.st_lines_title.clone();
            let lines = std::mem::take(&mut self.st_lines);
            egui::Window::new(title)
                .open(&mut open)
                .default_size([800.0, 400.0])
                .show(&ctx, |ui| {
                    egui::ScrollArea::both().show(ui, |ui| {
                        egui::Grid::new("stlines").num_columns(5).striped(true).show(ui, |ui| {
                            for h in ["Product", "Category", "Counted qty", "Unit", "Notes"] {
                                ui.label(egui::RichText::new(h).strong());
                            }
                            ui.end_row();
                            for l in &lines {
                                ui.label(&l.name);
                                ui.label(&l.category);
                                ui.label(format!("{:.3}", l.counted_qty));
                                ui.label(&l.unit);
                                ui.label(&l.notes);
                                ui.end_row();
                            }
                        });
                    });
                });
            self.st_lines = lines;
            if !open {
                self.st_lines_open = false;
            }
        }

        if self.imp.open {
            let mut open = true;
            egui::Window::new("Import filled checklist (stocktake)")
                .open(&mut open)
                .collapsible(false)
                .show(&ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("File:");
                        ui.label(if self.imp.path.is_empty() { "(none)" } else { &self.imp.path });
                        if ui.button("Browse...").clicked() {
                            if let Some(f) = rfd::FileDialog::new()
                                .add_filter("Excel/CSV", &["xlsx", "csv"])
                                .set_title("Select filled checklist")
                                .pick_file()
                            {
                                self.imp.path = f.to_string_lossy().to_string();
                            }
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Stocktake date (YYYY-MM-DD):");
                        ui.add(egui::TextEdit::singleline(&mut self.imp.day).desired_width(120.0));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Counted by:");
                        ui.add(egui::TextEdit::singleline(&mut self.imp.by).desired_width(200.0));
                        ui.label("Notes:");
                        ui.add(egui::TextEdit::singleline(&mut self.imp.notes).desired_width(200.0));
                    });
                    ui.horizontal(|ui| {
                        if ui.button("Import stocktake").clicked() {
                            match import_stocktake(
                                &self.db,
                                &self.imp.path,
                                &self.imp.day,
                                &self.imp.by,
                                &self.imp.notes,
                            ) {
                                Ok((id, n)) => {
                                    self.status = format!(
                                        "Stocktake #{id} recorded: {n} products counted on {} by {}.",
                                        self.imp.day, self.imp.by
                                    );
                                    self.imp.open = false;
                                    self.refresh_all();
                                }
                                Err(e) => self.status = format!("Could not import stocktake: {e}"),
                            }
                        }
                        if ui.button("Cancel").clicked() {
                            self.imp.open = false;
                        }
                    });
                });
            if !open {
                self.imp.open = false;
            }
        }
    }
}

impl App {
    fn ui_deliveries(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("Import PDF invoice(s)...").clicked() {
                self.import_pdfs();
            }
            if ui.button("Refresh").clicked() {
                self.refresh_all();
            }
            ui.label("Double-click a row to view its line items.");
        });
        ui.separator();
        ui.label(egui::RichText::new("Invoices (double-click a row for line items)").strong());
        let rows = self.invoices.clone();
        egui::ScrollArea::vertical().max_height(ui.available_height() - 160.0).show(ui, |ui| {
            egui::Grid::new("inv2").num_columns(8).striped(true).show(ui, |ui| {
                for h in ["Delivery date", "Invoice date", "Order no", "Reference", "Port", "Lines", "Total EUR", "Source file"] {
                    ui.label(egui::RichText::new(h).strong());
                }
                ui.end_row();
                for inv in &rows {
                    let resp = ui
                        .horizontal(|ui| {
                            ui.label(inv.delivery_date.clone().or(inv.invoice_date.clone()).unwrap_or_default());
                            ui.label(inv.invoice_date.clone().unwrap_or_default());
                            ui.label(&inv.order_no);
                            ui.label(&inv.reference);
                            ui.label(&inv.port);
                            ui.label(inv.line_count.to_string());
                            ui.label(eur(inv.grand_total.unwrap_or(0.0)));
                            ui.label(&inv.source_file);
                        })
                        .response;
                    ui.end_row();
                    if resp.double_clicked() {
                        if let Ok(l) = self.db.invoice_lines(inv.id) {
                            self.lines_title = format!(
                                "Order {} - {} ({})",
                                inv.order_no,
                                inv.reference,
                                inv.delivery_date.clone().or(inv.invoice_date.clone()).unwrap_or_default()
                            );
                            self.lines = l;
                            self.lines_open = true;
                        }
                    }
                }
            });
        });
    }

    fn ui_inventory(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("Export checklist (Excel)...").clicked() {
                self.export_checklist_file("xlsx");
            }
            if ui.button("Export checklist (CSV)...").clicked() {
                self.export_checklist_file("csv");
            }
            if ui.button("Import filled checklist (stocktake)...").clicked() {
                self.imp.open = true;
            }
            if ui.button("Refresh").clicked() {
                self.refresh_all();
            }
            ui.label("The crew fills 'Counted qty' in the exported file; import it back here.");
        });
        ui.add_space(6.0);
        ui.label(egui::RichText::new("Current inventory (products known from invoices)").strong());
        let products = self.products.clone();
        egui::ScrollArea::vertical().max_height(ui.available_height() * 0.55).show(ui, |ui| {
            egui::Grid::new("prod").num_columns(6).striped(true).show(ui, |ui| {
                for h in ["Product", "Category", "Unit", "Delivered to date", "Last counted", "Count date"] {
                    ui.label(egui::RichText::new(h).strong());
                }
                ui.end_row();
                for p in &products {
                    ui.label(&p.name);
                    ui.label(&p.category);
                    ui.label(&p.default_unit);
                    ui.label(format!("{:.3}", p.delivered_qty));
                    ui.label(p.last_counted.map(|v| format!("{v:.3}")).unwrap_or_default());
                    ui.label(p.last_count_date.clone().unwrap_or_default());
                    ui.end_row();
                }
            });
        });
        ui.add_space(6.0);
        ui.label(egui::RichText::new("Stocktake history (double-click for lines)").strong());
        let sts = self.stocktakes.clone();
        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("st").num_columns(5).striped(true).show(ui, |ui| {
                for h in ["Date", "Counted by", "Lines", "Source file", "Notes"] {
                    ui.label(egui::RichText::new(h).strong());
                }
                ui.end_row();
                for st in &sts {
                    let resp = ui
                        .horizontal(|ui| {
                            ui.label(&st.day);
                            ui.label(&st.counted_by);
                            ui.label(st.line_count.to_string());
                            ui.label(&st.source_file);
                            ui.label(&st.notes);
                        })
                        .response;
                    ui.end_row();
                    if resp.double_clicked() {
                        if let Ok(l) = self.db.stocktake_lines(st.id) {
                            self.st_lines_title = format!("Stocktake {} by {}", st.day, st.counted_by);
                            self.st_lines = l;
                            self.st_lines_open = true;
                        }
                    }
                }
            });
        });
    }

    fn ui_crew(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Date (YYYY-MM-DD):");
            ui.add(egui::TextEdit::singleline(&mut self.crew_day).desired_width(110.0));
            ui.label("Persons on board:");
            ui.add(egui::TextEdit::singleline(&mut self.crew_n).desired_width(60.0));
            if ui.button("Add / update").clicked() {
                match self.crew_n.trim().parse::<i64>() {
                    Ok(n) if n >= 1 => match self.db.add_crew(&self.crew_day, n) {
                        Ok(()) => {
                            self.status = format!("Crew log: {n} persons on {}.", self.crew_day);
                            self.refresh_all();
                        }
                        Err(e) => self.status = format!("Invalid date: {e}"),
                    },
                    _ => self.status = "Enter persons >= 1.".into(),
                }
            }
            if ui.button("Delete selected").clicked() {
                // delete the first crew row matching the date field if present
                if self.crew.iter().any(|c| c.day == self.crew_day) {
                    let _ = self.db.delete_crew(&self.crew_day);
                    self.status = format!("Deleted crew entry for {}.", self.crew_day);
                    self.refresh_all();
                }
            }
            ui.label("Used to compute consumption and cost per person.");
        });
        ui.add_space(6.0);
        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("crew").num_columns(2).striped(true).show(ui, |ui| {
                ui.label(egui::RichText::new("Date").strong());
                ui.label(egui::RichText::new("Persons on board").strong());
                ui.end_row();
                for c in &self.crew {
                    if ui.selectable_label(false, &c.day).clicked() {
                        self.crew_day = c.day.clone();
                    }
                    ui.label(c.persons.to_string());
                    ui.end_row();
                }
            });
        });
    }

    fn ui_insights(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Period:");
            let cur = self.period_opts[self.period_idx].clone();
            egui::ComboBox::from_id_salt("period")
                .selected_text(cur)
                .show_ui(ui, |ui| {
                    for (i, o) in self.period_opts.clone().iter().enumerate() {
                        ui.selectable_value(&mut self.period_idx, i, o);
                    }
                });
            if ui.button("Compute insights").clicked() {
                self.compute_insights();
            }
            if ui.button("Export Excel report...").clicked() {
                self.export_excel();
            }
        });
        ui.add_space(4.0);
        if let Some(rep) = &self.report {
            ui.label(format!(
                "Period {} -> {} | {} days | person-days: {:.1} | avg persons: {:.1} | purchased: EUR {} | consumed value: EUR {} | cost/person: EUR {} | cost/person-day: EUR {} | purchased/person-day: EUR {}",
                rep.start.clone().unwrap_or_else(|| "beginning".into()),
                rep.end.clone().unwrap_or_else(|| "today".into()),
                rep.days,
                rep.person_days,
                rep.avg_persons,
                eur(rep.total_delivered_cost),
                eur(rep.total_consumed_cost),
                rep.cost_per_person.map(|v| format!("{v:.2}")).unwrap_or_else(|| "n/a".into()),
                rep.cost_per_person_day.map(|v| format!("{v:.2}")).unwrap_or_else(|| "n/a".into()),
                rep.purchased_per_person_day.map(|v| format!("{v:.2}")).unwrap_or_else(|| "n/a".into()),
            ));
        }
        ui.add_space(6.0);
        if let Some(rep) = &self.report {
            let rows: Vec<_> = rep
                .rows
                .iter()
                .filter(|r| r.consumed != 0.0 || r.delivered != 0.0)
                .cloned()
                .collect();
            egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
                egui::Grid::new("ins").num_columns(9).striped(true).show(ui, |ui| {
                    for h in ["Product", "Category", "Unit", "Consumed", "Cons./person-day", "Cons./person", "Cost EUR", "Cost/person", "Cost/person-day"] {
                        ui.label(egui::RichText::new(h).strong());
                    }
                    ui.end_row();
                    for r in &rows {
                        ui.label(&r.product);
                        ui.label(&r.category);
                        ui.label(&r.unit);
                        ui.label(format!("{:.3}", r.consumed));
                        ui.label(r.consumed_per_person_day.map(|v| format!("{v:.4}")).unwrap_or_default());
                        ui.label(r.consumed_per_person.map(|v| format!("{v:.3}")).unwrap_or_default());
                        ui.label(eur(r.consumed_cost));
                        ui.label(r.cost_per_person.map(|v| format!("{v:.2}")).unwrap_or_default());
                        ui.label(r.cost_per_person_day.map(|v| format!("{v:.2}")).unwrap_or_default());
                        ui.end_row();
                    }
                });
            });
        } else {
            ui.label("Press 'Compute insights' to see consumption and cost per person.");
        }
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1180.0, 720.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Ship Provisions Tracker",
        options,
        Box::new(|_cc| Ok(Box::new(App::new()))),
    )
}
