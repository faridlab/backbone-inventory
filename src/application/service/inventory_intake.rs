//! Inbound intake (hand-authored, user-owned) — the `DeliveryRequested` port the selling↔inventory
//! delivery seam triggers on (`docs/erp/modules/backbone-inventory.md` §3, extension-guide).
//!
//! Selling, when a Sales Order is ready to ship, emits `DeliveryRequested`; inventory turns it into
//! a **draft** Delivery Note (the physical move is a separate `submit_delivery_note(sink)` step, so
//! the GL post + SLE happen under the composing service's control). The GL account ids are inventory
//! config (resolved by the composing service / an item-account map), not selling's concern, so they
//! ride on the intake DTO. This is the inventory-owned half of the seam — a consumer wires the event
//! bus to `DeliveryIntake::on_delivery_requested`.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use super::inventory_write_service::{
    DeliveryLine, InventoryError, InventoryWriteService, NewDelivery, NewReceipt, ReceiptLine,
};

/// Serde default for the optional `currency` field on inbound intake requests. Existing callers that
/// don't send a currency get "IDR" (the module's historical single-currency behavior).
fn default_currency() -> String { "IDR".into() }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeliveryRequestLine {
    pub item_id: Uuid,
    pub quantity: Decimal,
}

/// The inbound request selling emits to have stock shipped for a confirmed order.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeliveryRequested {
    pub delivery_number: String,
    pub company_id: Uuid,
    pub branch_id: Option<Uuid>,
    pub customer_id: Uuid,
    pub source_so_id: Option<Uuid>,
    pub warehouse_id: Uuid,
    pub posting_date: chrono::NaiveDate,
    /// Ledger currency of the GL post (defaults to "IDR" when omitted).
    #[serde(default = "default_currency")]
    pub currency: String,
    /// GL accounts are inventory config, supplied by the composing service (not by selling).
    pub cogs_account_id: Uuid,
    pub inventory_account_id: Uuid,
    pub lines: Vec<DeliveryRequestLine>,
}

/// One line of a receipt request (what buying asks inventory to receive; carries the unit cost).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReceiptRequestLine {
    pub item_id: Uuid,
    pub quantity: Decimal,
    pub rate: Decimal,
}

/// The inbound request buying emits to have goods received against a confirmed Purchase Order.
/// GL accounts (Inventory + GR/IR) are inventory config, supplied by the composing service.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReceiptExpected {
    pub receipt_number: String,
    pub company_id: Uuid,
    pub branch_id: Option<Uuid>,
    pub supplier_id: Uuid,
    pub source_po_id: Option<Uuid>,
    pub warehouse_id: Uuid,
    pub posting_date: chrono::NaiveDate,
    /// Ledger currency of the GL post (defaults to "IDR" when omitted).
    #[serde(default = "default_currency")]
    pub currency: String,
    pub inventory_account_id: Uuid,
    pub grir_account_id: Uuid,
    pub lines: Vec<ReceiptRequestLine>,
}

/// The intake handler — a consumer subscribes the event bus to these.
#[derive(Clone)]
pub struct DeliveryIntake {
    write: InventoryWriteService,
}

impl DeliveryIntake {
    pub fn new(pool: PgPool) -> Self {
        Self { write: InventoryWriteService::new(pool) }
    }

    /// Turn a `DeliveryRequested` into a DRAFT delivery note; returns its id. The caller then
    /// `submit_delivery_note(id, sink)` to perform the physical move + COGS post.
    pub async fn on_delivery_requested(&self, req: DeliveryRequested) -> Result<Uuid, InventoryError> {
        self.write.create_delivery_note(NewDelivery {
            delivery_number: req.delivery_number,
            company_id: req.company_id,
            branch_id: req.branch_id,
            customer_id: req.customer_id,
            source_so_id: req.source_so_id,
            warehouse_id: req.warehouse_id,
            posting_date: req.posting_date,
            currency: req.currency,
            cogs_account_id: req.cogs_account_id,
            inventory_account_id: req.inventory_account_id,
            lines: req.lines.into_iter().map(|l| DeliveryLine { item_id: l.item_id, quantity: l.quantity }).collect(),
        }).await
    }

    /// Turn a `ReceiptExpected` (from buying) into a DRAFT purchase receipt; returns its id. The
    /// caller then `submit_purchase_receipt(id, sink)` to perform the physical move + asset post.
    /// The receipt-side mirror of `on_delivery_requested`.
    pub async fn on_receipt_expected(&self, req: ReceiptExpected) -> Result<Uuid, InventoryError> {
        self.write.create_purchase_receipt(NewReceipt {
            receipt_number: req.receipt_number,
            company_id: req.company_id,
            branch_id: req.branch_id,
            supplier_id: req.supplier_id,
            source_po_id: req.source_po_id,
            warehouse_id: req.warehouse_id,
            posting_date: req.posting_date,
            currency: req.currency,
            inventory_account_id: req.inventory_account_id,
            grir_account_id: req.grir_account_id,
            lines: req.lines.into_iter().map(|l| ReceiptLine { item_id: l.item_id, quantity: l.quantity, rate: l.rate }).collect(),
        }).await
    }
}
