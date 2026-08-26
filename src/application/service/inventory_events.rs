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

// ---- stock-move pipeline events (schema/hooks/stock.hook.yaml `events:` block) -------------

/// A stock move hit `confirmed` (spec §1 `_action_confirm`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MoveConfirmed {
    pub move_id: Uuid,
    pub company_id: Uuid,
    pub item_id: Uuid,
    pub demand_qty: Decimal,
    pub picking_id: Option<Uuid>,
}

/// A stock move hit `assigned` — fully reserved against quants (spec §1 `_action_assign`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MoveAssigned {
    pub move_id: Uuid,
    pub company_id: Uuid,
    pub picking_id: Option<Uuid>,
}

/// A stock move hit `done` — quants flipped, SLE minted (spec §4 `_action_done`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MoveDone {
    pub move_id: Uuid,
    pub company_id: Uuid,
    pub item_id: Uuid,
    pub quantity: Decimal,
    pub price_unit: Decimal,
    pub is_inventory: bool,
}

/// A stock move hit `cancel` — its reservation was released (spec §1 `_action_cancel`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MoveCancelled {
    pub move_id: Uuid,
    pub company_id: Uuid,
    pub released_qty: Decimal,
}

/// A transfer's projected state changed (the T1 picking projection recompute — the picking has no
/// state machine of its own; its state is re-derived from its moves on every move change).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TransferProjected {
    pub transfer_id: Uuid,
    pub company_id: Uuid,
    pub state: String,
    pub previous_state: String,
}

/// A partial validate minted the backorder move (spec §4 step 4).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BackorderCreated {
    pub backorder_id: Uuid,
    pub company_id: Uuid,
    pub origin_id: Uuid,
}

/// The daily scheduler ordered replenishment on a reordering rule (spec §7 task 1; the T11
/// orderpoint computes decided the quantity).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OrderpointTriggered {
    pub orderpoint_id: Uuid,
    pub company_id: Uuid,
    pub item_id: Uuid,
    pub qty_to_order: Decimal,
    pub forecast_qty: Decimal,
}

/// A landed-cost document was validated: its cost lines were allocated over the target
/// receipt's DONE move lines and the remaining-share portion revalued the bins through the
/// move engine's adjustment verb.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LandedCostValidated {
    pub lc_id: Uuid,
    pub company_id: Uuid,
    pub target_receipt_id: Uuid,
    /// Σ cost line amounts (negative on a reversal document).
    pub amount_total: Decimal,
    /// The remaining-share value actually revalued onto the bins (the effective Σ of the
    /// minted landed-cost SLE rows — smaller than `amount_total` when part of the target stock
    /// was already consumed: the retroactive-revaluation asymmetry).
    pub revalued_value: Decimal,
}

/// The inventory domain-event union (discriminated) published on the module event bus.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum InventoryEvent {
    StockReceived(StockReceived),
    StockDelivered(StockDelivered),
    StockMoved(StockMoved),
    StockReconciled(StockReconciled),
    MoveConfirmed(MoveConfirmed),
    MoveAssigned(MoveAssigned),
    MoveDone(MoveDone),
    MoveCancelled(MoveCancelled),
    TransferProjected(TransferProjected),
    BackorderCreated(BackorderCreated),
    OrderpointTriggered(OrderpointTriggered),
    LandedCostValidated(LandedCostValidated),
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
