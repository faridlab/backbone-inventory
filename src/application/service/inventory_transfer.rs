//! The Stock Entry (transfer) path (hand-authored, user-owned).
//!
//! An `impl InventoryWriteService` chunk over the vocabulary in [`super::inventory_write_service`]:
//! the warehouse-to-warehouse move. Value-neutral — no GL — but still drives the moving-average
//! engine: paired out/in SLE at the source rate, BOTH bins locked in canonical `warehouse_id` order
//! before either leg runs (the lock order that stops two transfers touching the same pair of
//! warehouses — including opposing A→B / B→A — from deadlocking), and the same residual-flush rule
//! as delivery (draining the source to 0 carries its entire remaining value).
//!
//! Per the module's 4-layer rule this file holds no SQL — the statements live on
//! `StockEntryRepository` / `StockEntryItemRepository` / `BinRepository` /
//! `StockLedgerEntryRepository`, whose write methods take THIS service's transaction so the source
//! and target legs of the move commit together with the entry header.

use backbone_orm::company_scope;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::infrastructure::persistence::{
    NewSleRow, NewStockEntryItemRow, NewTransferRow,
};

use super::inventory_events::{InventoryEvent, StockMoved};
use super::inventory_write_service::{
    is_dup, money, rate6, InventoryError, InventoryWriteService, NewTransfer,
};

impl InventoryWriteService {
    // ---- submit: Stock Entry (transfer — value-neutral, no GL) --------------

    pub async fn submit_transfer(&self, t: NewTransfer) -> Result<Uuid, InventoryError> {
        if t.lines.is_empty() { return Err(InventoryError::EmptyDocument); }
        if t.from_warehouse_id == t.to_warehouse_id { return Err(InventoryError::SameWarehouse); }
        for l in &t.lines { if l.quantity < Decimal::ZERO { return Err(InventoryError::NegativeQuantity); } }

        let id = Uuid::new_v4();
        let mut tx = self.db_pool.begin().await?;
        // RLS scope (ADR-0008): MUST be bound before any bin read — an unbound connection is fenced
        // to zero rows, so the FOR UPDATE below would read every bin as empty.
        company_scope::bind_company_on(&mut tx, t.company_id).await?;
        let ins = self.entries.insert_transfer(&mut tx, &NewTransferRow {
            id,
            entry_number: &t.entry_number,
            company_id: t.company_id,
            from_warehouse_id: t.from_warehouse_id,
            to_warehouse_id: t.to_warehouse_id,
            posting_date: t.posting_date,
        }).await;
        if let Err(e) = ins {
            return Err(if is_dup(&e) { InventoryError::DuplicateNumber(t.entry_number) } else { e.into() });
        }
        let mut sle_no = 0i32;
        for l in &t.lines {
            self.entry_items.insert_item(&mut tx, &NewStockEntryItemRow {
                id: Uuid::new_v4(),
                entry_id: id,
                company_id: t.company_id,
                item_id: l.item_id,
                quantity: l.quantity,
            }).await?;
            // Lock BOTH bins in CANONICAL warehouse_id order before either leg runs. The earlier
            // "source before target" rule only ordered locks WITHIN one voucher — two concurrent
            // transfers A→B and B→A on the same item each took their own source first and deadlocked
            // at the Postgres row lock. Locking min(warehouse_id) first makes every transfer on the
            // pair take locks in the same order, so they serialize instead of deadlocking
            // (council 2026-07-29, parking-lot item).
            let (from_wh, to_wh) = (t.from_warehouse_id, t.to_warehouse_id);
            let (first_wh, second_wh) = if from_wh < to_wh { (from_wh, to_wh) } else { (to_wh, from_wh) };
            let first = self.bins.lock_or_init(&mut tx, t.company_id, l.item_id, first_wh).await?;
            let second = self.bins.lock_or_init(&mut tx, t.company_id, l.item_id, second_wh).await?;
            let (from, to) = if from_wh == first_wh { (first, second) } else { (second, first) };
            if from.actual_qty < l.quantity {
                return Err(InventoryError::InsufficientStock { item_id: l.item_id, warehouse_id: t.from_warehouse_id, available: from.actual_qty, requested: l.quantity });
            }
            let from_qty = from.actual_qty - l.quantity;
            // Same residual flush as delivery: when the source bin drains to 0, the move carries
            // its entire remaining value so the source ends at exactly 0 (and the target receives
            // the exact value moved — total value conserved to the cent).
            let move_value = if from_qty.is_zero() { from.stock_value } else { money(l.quantity * from.valuation_rate) };
            let from_value = from.stock_value - move_value;
            let from_rate = if from_qty > Decimal::ZERO { from.valuation_rate } else { Decimal::ZERO };
            self.bins.update_balance(&mut tx, t.company_id, l.item_id, t.from_warehouse_id, from_qty, from_rate, from_value).await?;
            sle_no += 1;
            self.sles.insert_sle(&mut tx, &NewSleRow {
                company_id: t.company_id, item_id: l.item_id, warehouse_id: t.from_warehouse_id,
                posting_date: t.posting_date, actual_qty: -l.quantity, qty_after_txn: from_qty,
                incoming_rate: Decimal::ZERO, valuation_rate: from_rate, stock_value: from_value,
                stock_value_difference: -move_value, voucher_type: "stock_entry", voucher_id: id,
                voucher_no: &t.entry_number, sle_no,
            }).await?;
            // IN to target at the source rate (value conserved). `to` was captured above (locked in
            // canonical order, before the OUT leg ran) — the OUT leg only touches the source bin, so
            // this snapshot is the target's correct starting balance.
            let to_qty = to.actual_qty + l.quantity;
            let to_value = to.stock_value + move_value;
            let to_rate = if to_qty > Decimal::ZERO { rate6(to_value / to_qty) } else { Decimal::ZERO };
            self.bins.update_balance(&mut tx, t.company_id, l.item_id, t.to_warehouse_id, to_qty, to_rate, to_value).await?;
            sle_no += 1;
            self.sles.insert_sle(&mut tx, &NewSleRow {
                company_id: t.company_id, item_id: l.item_id, warehouse_id: t.to_warehouse_id,
                posting_date: t.posting_date, actual_qty: l.quantity, qty_after_txn: to_qty,
                incoming_rate: from.valuation_rate, valuation_rate: to_rate, stock_value: to_value,
                stock_value_difference: move_value, voucher_type: "stock_entry", voucher_id: id,
                voucher_no: &t.entry_number, sle_no,
            }).await?;
        }
        tx.commit().await?;
        self.sink.publish(InventoryEvent::StockMoved(StockMoved {
            entry_id: id, company_id: t.company_id, from_warehouse_id: Some(t.from_warehouse_id), to_warehouse_id: Some(t.to_warehouse_id),
        }));
        Ok(id)
    }
}
