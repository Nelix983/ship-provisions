from __future__ import annotations

from dataclasses import dataclass, field


@dataclass
class InvoiceLine:
    ref: str
    description: str
    quantity: float
    unit: str
    unit_price: float
    total: float


@dataclass
class Invoice:
    invoice_date: str = ""        # ISO yyyy-mm-dd
    delivery_date: str = ""       # ISO yyyy-mm-dd
    order_no: str = ""
    packing_list: str = ""
    port: str = ""
    reference: str = ""           # e.g. PROVISIONS / CABIN / BONDED
    your_order: str = ""
    debtor: str = ""
    ship: str = ""
    total_excl_vat: float | None = None
    grand_total: float | None = None
    lines: list[InvoiceLine] = field(default_factory=list)

    @property
    def lines_total(self) -> float:
        return round(sum(l.total for l in self.lines), 2)

    @property
    def total(self) -> float:
        if self.grand_total is not None:
            return self.grand_total
        return self.lines_total


@dataclass
class ParsedPdf:
    source_path: str
    invoices: list[Invoice] = field(default_factory=list)
