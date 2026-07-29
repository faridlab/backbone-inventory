//! The Delivery Note path: draft → submit → repost (hand-authored, user-owned).
//!
//! An `impl InventoryWriteService` chunk over the vocabulary in [`super::inventory_write_service`]:
//! the goods-out voucher. `create_delivery_note` opens a draft; `submit_delivery_note` runs the
//! **outflow** half of the moving-average valuation engine (`cogs = qty·rate; value -= cogs; qty -=
//! qty; rate unchanged by an outflow`) with availability check + the residual-flush rule (draining a
//! bin to 0 carries its entire remaining value so stock_value returns to exactly 0); a balanced
//! `AccountingPost` (`Dr COGS · Cr Inventory`) follows. `repost_delivery_note` is the exit from a
//! stuck `failed` GL post.
//!
//! Per the module's 4-layer rule this file holds no SQL — the statements live on
//! `DeliveryNoteRepository` / `DeliveryNoteItemRepository` / `BinRepository` /
//! `StockLedgerEntryRepository`, whose write methods take THIS service's transaction so the SLE + Bin
//! + line valuation + header status commit together with the delivery.

use backbone_orm::company_scope;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::infrastructure::persistence::{GlVoucher, NewDeliveryItemRow, NewDeliveryRow, NewSleRow};

use super::inventory_events::{InventoryEvent, StockDelivered};
use super::inventory_gl::{AccountingPostEnvelope, GlPostLine, GlPostSink};
use super::inventory_write_service::{
    is_dup, money, InventoryError, InventoryWriteService, NewDelivery, SubmitOutcome,
};

impl InventoryWriteService {
    pub async fn create_delivery_note(&self, d: NewDelivery) -> Result<Uuid, InventoryError> {
        if d.lines.is_empty() { return Err(InventoryError::EmptyDocument); }
        for l in &d.lines {
            if l.quantity < Decimal::ZERO { return Err(InventoryError::NegativeQuantity); }
        }
        let id = Uuid::new_v4();
        let mut tx = self.db_pool.begin().await?;
        company_scope::bind_company_on(&mut tx, d.company_id).await?;
        let ins = self.deliveries.insert_draft(&mut tx, &NewDeliveryRow {
            id,
            delivery_number: &d.delivery_number,
            company_id: d.company_id,
            branch_id: d.branch_id,
            customer_id: d.customer_id,
            source_so_id: d.source_so_id,
            warehouse_id: d.warehouse_id,
            posting_date: d.posting_date,
            currency: &d.currency,
            cogs_account_id: d.cogs_account_id,
            inventory_account_id: d.inventory_account_id,
        }).await;
        if let Err(e) = ins {
            return Err(if is_dup(&e) { InventoryError::DuplicateNumber(d.delivery_number) } else { e.into() });
        }
        for l in &d.lines {
            self.delivery_items.insert_item(&mut tx, &NewDeliveryItemRow {
                id: Uuid::new_v4(),
                delivery_id: id,
                company_id: d.company_id,
                item_id: l.item_id,
                quantity: l.quantity,
            }).await?;
        }
        tx.commit().await?;
        Ok(id)
    }

    // ---- submit: Delivery Note (COGS post) ---------------------------------

    pub async fn submit_delivery_note(&self, id: Uuid, sink: &dyn GlPostSink) -> Result<SubmitOutcome, InventoryError> {
        // RLS scope (ADR-0008), ID-only: fenced by the request/inherited scope.
        let hdr = self.deliveries.fetch_submit_header(&self.db_pool, id).await?
            .ok_or(InventoryError::NotFound(id))?;
        if hdr.status != "draft" {
            return Err(InventoryError::NotDraft(id.to_string()));
        }
        let company = hdr.company_id;
        let branch = hdr.branch_id;
        let warehouse = hdr.warehouse_id;
        let posting_date = hdr.posting_date;
        let voucher_no = hdr.delivery_number;
        let cogs_acct = hdr.cogs_account_id;
        let inv_acct = hdr.inventory_account_id;
        let source_so = hdr.source_so_id;

        let items = company_scope::with_company_scope(
            Some(company),
            self.delivery_items.fetch_items(&self.db_pool, id),
        ).await?;

        let mut tx = self.db_pool.begin().await?;
        // RLS scope (ADR-0008): MUST be bound before any bin read — an unbound connection is fenced
        // to zero rows, so the FOR UPDATE below would read every bin as empty (and every delivery
        // would then fail the availability check).
        company_scope::bind_company_on(&mut tx, company).await?;
        let mut total_cogs = Decimal::ZERO;
        let mut sle_no = 0i32;
        for it in &items {
            let (line_id, item, qty) = (it.id, it.item_id, it.quantity);
            let bin = self.bins.lock_or_init(&mut tx, company, item, warehouse).await?;
            if bin.actual_qty < qty {
                return Err(InventoryError::InsufficientStock { item_id: item, warehouse_id: warehouse, available: bin.actual_qty, requested: qty });
            }
            let new_qty = bin.actual_qty - qty;
            // On the FINAL outflow (bin drained to 0), the last units absorb the moving-average
            // rounding residual: COGS = the entire remaining value, so stock_value returns to
            // EXACTLY 0 and the Inventory subledger ties out with the GL at zero stock (council
            // 2026-07-04). Otherwise COGS consumes the current 2dp-rounded average.
            let cogs = if new_qty.is_zero() { bin.stock_value } else { money(qty * bin.valuation_rate) };
            let new_value = bin.stock_value - cogs;
            // Moving-average: rate is unchanged by an outflow.
            let new_rate = if new_qty > Decimal::ZERO { bin.valuation_rate } else { Decimal::ZERO };
            self.bins.update_balance(&mut tx, company, item, warehouse, new_qty, new_rate, new_value).await?;
            self.delivery_items.update_valuation(&mut tx, line_id, bin.valuation_rate, cogs).await?;
            sle_no += 1;
            self.sles.insert_sle(&mut tx, &NewSleRow {
                company_id: company, item_id: item, warehouse_id: warehouse, posting_date,
                actual_qty: -qty, qty_after_txn: new_qty, incoming_rate: Decimal::ZERO,
                valuation_rate: new_rate, stock_value: new_value, stock_value_difference: -cogs,
                voucher_type: "delivery_note", voucher_id: id, voucher_no: &voucher_no, sle_no,
            }).await?;
            total_cogs += cogs;
        }
        self.deliveries.mark_submitted_with_cogs(&mut tx, id, total_cogs).await?;
        tx.commit().await?;

        let env = AccountingPostEnvelope {
            idempotency_key: id.to_string(), company_id: company, branch_id: branch,
            source_type: "inventory".into(), source_id: id, source_reference: Some(voucher_no.clone()),
            posting_date, currency: hdr.currency.clone(), posting_type: "original".into(), reverses_post_id: None,
            description: Some("Delivery COGS".into()),
            lines: vec![
                GlPostLine::debit(cogs_acct, total_cogs).with_description("COGS"),
                GlPostLine::credit(inv_acct, total_cogs).with_description("Inventory"),
            ],
        };
        let outcome = self.emit_and_reconcile(GlVoucher::DeliveryNote, id, &env, sink, total_cogs).await?;
        self.sink.publish(InventoryEvent::StockDelivered(StockDelivered {
            delivery_id: id, company_id: company, warehouse_id: warehouse, source_so_id: source_so,
            total_cogs,
        }));
        Ok(outcome)
    }

    // ---- repost: re-drive a stuck GL post (pending/failed) ------------------

    pub async fn repost_delivery_note(&self, id: Uuid, sink: &dyn GlPostSink) -> Result<SubmitOutcome, InventoryError> {
        // RLS scope (ADR-0008), ID-only: fenced by the request/inherited scope.
        let h = self.deliveries.fetch_repost_header(&self.db_pool, id).await?
            .ok_or(InventoryError::NotFound(id))?;
        if let Some(o) = Self::already_settled(&h.gl, id) { return Ok(o); }
        let amt = h.total_cogs;
        let env = AccountingPostEnvelope {
            idempotency_key: id.to_string(), company_id: h.company_id, branch_id: h.branch_id,
            source_type: "inventory".into(), source_id: id, source_reference: Some(h.delivery_number),
            posting_date: h.posting_date, currency: h.currency.clone(), posting_type: "original".into(), reverses_post_id: None,
            description: Some("Delivery COGS (repost)".into()),
            lines: vec![
                GlPostLine::debit(h.cogs_account_id, amt).with_description("COGS"),
                GlPostLine::credit(h.inventory_account_id, amt).with_description("Inventory"),
            ],
        };
        self.emit_and_reconcile(GlVoucher::DeliveryNote, id, &env, sink, amt).await
    }
}
