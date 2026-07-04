//! Inventory domain events — the public extension surface (hand-authored, user-owned).
//!
//! Semantic events a consumer subscribes to (per the module brief), distinct from generated CRUD
//! events. Published through an `InventoryEventSink`. Notably `StockDelivered` lets `backbone-selling`
//! advance its `delivered_qty` watermark, and `StockReceived` lets buying reconcile a PO.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Goods received into stock (Purchase Receipt submitted).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StockReceived {
    pub receipt_id: Uuid,
    pub company_id: Uuid,
    pub warehouse_id: Uuid,
    pub source_po_id: Option<Uuid>,
    pub total_value: Decimal,
}

/// Stock delivered out (Delivery Note submitted) — carries the source SO so selling can advance
/// its delivered_qty watermark.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StockDelivered {
    pub delivery_id: Uuid,
    pub company_id: Uuid,
    pub warehouse_id: Uuid,
    pub source_so_id: Option<Uuid>,
    pub total_cogs: Decimal,
}

/// Stock moved between warehouses (Stock Entry submitted) — value-neutral.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StockMoved {
    pub entry_id: Uuid,
    pub company_id: Uuid,
    pub from_warehouse_id: Option<Uuid>,
    pub to_warehouse_id: Option<Uuid>,
}

/// Stock adjusted to a physical count (Stock Reconciliation submitted).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StockReconciled {
    pub reconciliation_id: Uuid,
    pub company_id: Uuid,
    pub warehouse_id: Uuid,
    pub net_difference: Decimal,
}

/// The inventory domain-event union (discriminated) published on the module event bus.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum InventoryEvent {
    StockReceived(StockReceived),
    StockDelivered(StockDelivered),
    StockMoved(StockMoved),
    StockReconciled(StockReconciled),
}

/// Sink for inventory domain events. Fire-and-forget; a real adapter wires a bus, tests record.
pub trait InventoryEventSink: Send + Sync {
    fn publish(&self, event: InventoryEvent);
}

/// Default sink — emits structured tracing events.
pub struct LoggingSink;

impl InventoryEventSink for LoggingSink {
    fn publish(&self, event: InventoryEvent) {
        tracing::info!(target: "inventory.events", ?event, "inventory domain event");
    }
}
