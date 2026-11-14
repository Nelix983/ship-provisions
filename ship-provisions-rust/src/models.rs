#[derive(Debug, Clone, Default)]
pub struct InvoiceLine {
    pub line_ref: String,
    pub description: String,
    pub quantity: f64,
    pub unit: String,
    pub unit_price: f64,
    pub total: f64,
}

#[derive(Debug, Clone, Default)]
pub struct Invoice {
    pub invoice_date: String,
    pub delivery_date: String,
    pub order_no: String,
    pub packing_list: String,
    pub port: String,
    pub reference: String,
    pub your_order: String,
    pub debtor: String,
    pub ship: String,
    pub total_excl_vat: Option<f64>,
    pub grand_total: Option<f64>,
    pub lines: Vec<InvoiceLine>,
}

impl Invoice {
    pub fn lines_total(&self) -> f64 {
        round2(self.lines.iter().map(|l| l.total).sum())
    }

    pub fn total(&self) -> f64 {
        self.grand_total.unwrap_or_else(|| self.lines_total())
    }
}

pub fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

pub struct ParsedPdf {
    pub source_path: String,
    pub invoices: Vec<Invoice>,
}
