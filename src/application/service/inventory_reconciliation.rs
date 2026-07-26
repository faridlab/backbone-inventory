//! The Stock Reconciliation path (hand-authored, user-owned).
//!
//! An `impl InventoryWriteService` chunk over the vocabulary in [`super::inventory_write_service`]:
//! the physical-count voucher. Sets each (item, warehouse) bin to the counted qty/value; the delta
//! vs. the running balance is the posted difference. A balanced `AccountingPost` follows when the
//! net difference is nonzero (`Dr Inventory · Cr Adjustment` for a positive count, reversed for a
//! negative one); a zero net is `not_applicable` and emits no GL.
//!
//! Per the module's 4-layer rule this file holds no SQL — the statements live on
//! `StockReconciliationRepository` / `StockReconciliationItemRepository` / `BinRepository` /
//! `StockLedgerEntryRepository`, whose write methods take THIS service's transaction so the SLE + Bin
//! + recon lines + header status commit together.

use backbone_orm::company_scope;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::infrastructure::persistence::{
    GlVoucher, NewReconciliationItemRow, NewReconciliationRow, NewSleRow,
};

use super::inventory_events::{InventoryEvent, StockReconciled};
use super::inventory_gl::{AccountingPostEnvelope, GlPostLine, GlPostSink};
use super::inventory_write_service::{
    is_dup, money, InventoryError, InventoryWriteService, NewReconciliation,
};

impl InventoryWriteService {
    // ---- submit: Stock Reconciliation (value-difference post) ---------------

    pub async fn submit_reconciliation(&self, r: NewReconciliation, sink: &dyn GlPostSink) -> Result<Uuid, InventoryError> {
        if r.lines.is_empty() { return Err(InventoryError::EmptyDocument); }
        for l in &r.lines {
            if l.counted_qty < Decimal::ZERO || l.counted_rate < Decimal::ZERO { return Err(InventoryError::NegativeQuantity); }
        }
        let id = Uuid::new_v4();
        let mut tx = self.db_pool.begin().await?;
        // RLS scope (ADR-0008): MUST be bound before any bin read — an unbound connection is fenced
        // to zero rows, so the FOR UPDATE below would read every bin as empty and the count would
        // book the entire counted quantity as a difference.
        company_scope::bind_company_on(&mut tx, r.company_id).await?;
        let ins = self.recons.insert_submitted(&mut tx, &NewReconciliationRow {
            id,
            recon_number: &r.recon_number,
            company_id: r.company_id,
            warehouse_id: r.warehouse_id,
            posting_date: r.posting_date,
            inventory_account_id: r.inventory_account_id,
            adjustment_account_id: r.adjustment_account_id,
        }).await;
        if let Err(e) = ins {
            return Err(if is_dup(&e) { InventoryError::DuplicateNumber(r.recon_number) } else { e.into() });
        }
        let mut net = Decimal::ZERO;
        let mut sle_no = 0i32;
        for l in &r.lines {
            let bin = self.bins.lock_or_init(&mut tx, r.company_id, l.item_id, r.warehouse_id).await?;
            let target_rate = if l.counted_rate > Decimal::ZERO { l.counted_rate } else { bin.valuation_rate };
            let new_value = money(l.counted_qty * target_rate);
            let value_diff = new_value - bin.stock_value;
            let qty_diff = l.counted_qty - bin.actual_qty;
            self.bins.update_balance(&mut tx, r.company_id, l.item_id, r.warehouse_id, l.counted_qty, target_rate, new_value).await?;
            self.recon_items.insert_item(&mut tx, &NewReconciliationItemRow {
                id: Uuid::new_v4(),
                reconciliation_id: id,
                company_id: r.company_id,
                item_id: l.item_id,
                counted_qty: l.counted_qty,
                counted_rate: l.counted_rate,
                qty_difference: qty_diff,
                value_difference: value_diff,
            }).await?;
            sle_no += 1;
            self.sles.insert_sle(&mut tx, &NewSleRow {
                company_id: r.company_id, item_id: l.item_id, warehouse_id: r.warehouse_id,
                posting_date: r.posting_date, actual_qty: qty_diff, qty_after_txn: l.counted_qty,
                incoming_rate: Decimal::ZERO, valuation_rate: target_rate, stock_value: new_value,
                stock_value_difference: value_diff, voucher_type: "stock_reconciliation",
                voucher_id: id, voucher_no: &r.recon_number, sle_no,
            }).await?;
            net += value_diff;
        }
        self.recons.update_net_difference(&mut tx, id, net).await?;
        tx.commit().await?;

        if net != Decimal::ZERO {
            // net > 0: stock increased → Dr Inventory · Cr Adjustment; net < 0: the reverse.
            let (inv, adj) = (r.inventory_account_id, r.adjustment_account_id);
            let lines = if net > Decimal::ZERO {
                vec![GlPostLine::debit(inv, net).with_description("Inventory"), GlPostLine::credit(adj, net).with_description("Stock adjustment")]
            } else {
                let amt = -net;
                vec![GlPostLine::debit(adj, amt).with_description("Stock adjustment"), GlPostLine::credit(inv, amt).with_description("Inventory")]
            };
            let env = AccountingPostEnvelope {
                idempotency_key: id.to_string(), company_id: r.company_id, branch_id: None,
                source_type: "inventory".into(), source_id: id, source_reference: Some(r.recon_number.clone()),
                posting_date: r.posting_date, currency: "IDR".into(), posting_type: "original".into(), reverses_post_id: None,
                description: Some("Stock reconciliation".into()), lines,
            };
            self.emit_and_reconcile(GlVoucher::StockReconciliation, id, &env, sink, net.abs()).await?;
        } else {
            company_scope::with_company_scope(
                Some(r.company_id),
                self.recons.mark_not_applicable(&self.db_pool, id),
            ).await?;
        }
        self.sink.publish(InventoryEvent::StockReconciled(StockReconciled {
            reconciliation_id: id, company_id: r.company_id, warehouse_id: r.warehouse_id, net_difference: net,
        }));
        Ok(id)
    }
}
