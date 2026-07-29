//! The cancellation/reversal path (hand-authored, user-owned) — council 2026-07-29, finding #3.
//!
//! An `impl InventoryWriteService` chunk: `cancel_purchase_receipt` and `cancel_delivery_note`. A
//! cancellation REVERSES a submitted voucher: per original line it appends a COMPENSATING SLE that is
//! the exact negation of the original SLE (so the bin reblends to its pre-movement state), flips the
//! header submitted→cancelled, and emits a balanced `posting_type='reversal'` AccountingPost whose
//! `reverses_post_id` references the original post. The original `journal_id`/`accounting_post_id`
//! stay intact; the reversal's ids land in `reversal_journal_id`/`reversal_accounting_post_id`.
//!
//! Idempotent + crash-recoverable: if the physical reversal committed but the GL reversal failed
//! (status='cancelled', `reversal_accounting_post_id` NULL), re-calling re-emits ONLY the GL — it
//! never re-appends SLEs. An already-fully-reversed voucher short-circuits with the recorded ids.
//!
//! **Receipt cancel** reverses an INFLOW: push the received qty back out and remove the value it
//! added. Requires the bin still hold the qty (else `InsufficientStockToReverse` — the stock was
//! issued). Uses the stored line amount (`money(qty·rate)`) so the bin returns to its exact
//! pre-receipt value. GL: swaps the original `Dr Inventory · Cr GR/IR` to `Dr GR/IR · Cr Inventory`.
//!
//! **Delivery cancel** reverses an OUTFLOW: push the delivered qty back in and restore the COGS
//! consumed. Always safe (qty only increases). Uses the stored `cogs_amount` (for a drain-to-zero
//! line, the whole remaining value) so the bin reblends exactly. GL: swaps `Dr COGS · Cr Inventory`
//! to `Dr Inventory · Cr COGS`.
//!
//! Per the module's 4-layer rule this file holds no SQL — the statements live on the repositories,
//! whose write methods take THIS service's transaction so the compensating SLE + Bin + header status
//! commit together.

use backbone_orm::company_scope;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::infrastructure::persistence::{
    DeliveryCancelHeaderRow, GlVoucher, NewSleRow, ReceiptCancelHeaderRow,
};

use super::inventory_gl::{AccountingPostEnvelope, GlPostLine, GlPostSink};
use super::inventory_write_service::{
    money, rate6, InventoryError, InventoryWriteService, SubmitOutcome,
};

impl InventoryWriteService {
    // ---- cancel: Purchase Receipt (reverse the inflow + reversal GL post) -----------------

    pub async fn cancel_purchase_receipt(&self, id: Uuid, sink: &dyn GlPostSink) -> Result<SubmitOutcome, InventoryError> {
        let h = self.receipts.fetch_cancel_header(&self.db_pool, id).await?
            .ok_or(InventoryError::NotFound(id))?;

        // Recovery / idempotency: the physical reversal already committed. Re-emit ONLY the GL leg —
        // never re-append SLEs. An already-recorded reversal short-circuits with its ids.
        if h.status == "cancelled" {
            return self.receipt_reversal_gl(id, h, sink).await;
        }
        if h.status != "submitted" {
            return Err(InventoryError::NotSubmitted(id));
        }
        // A reversal references the original post; the original must be posted.
        let orig_post_id = h.gl.accounting_post_id.ok_or(InventoryError::GlNotPosted(id))?;

        let items = company_scope::with_company_scope(
            Some(h.company_id),
            self.receipt_items.fetch_items(&self.db_pool, id),
        ).await?;

        let mut tx = self.db_pool.begin().await?;
        // RLS scope (ADR-0008): MUST be bound before any bin read.
        company_scope::bind_company_on(&mut tx, h.company_id).await?;
        let mut sle_no = self.sles.fetch_max_sle_no(&mut tx, "purchase_receipt", id).await?;
        let mut total = Decimal::ZERO;
        for it in &items {
            let (item, qty_in, rate) = (it.item_id, it.quantity, it.rate);
            // The exact value the original inflow added — negating it restores the bin precisely.
            let reverse_value = money(qty_in * rate);
            let bin = self.bins.lock_or_init(&mut tx, h.company_id, item, h.warehouse_id).await?;
            if bin.actual_qty < qty_in {
                return Err(InventoryError::InsufficientStockToReverse {
                    item_id: item, warehouse_id: h.warehouse_id, available: bin.actual_qty, requested: qty_in,
                });
            }
            let new_qty = bin.actual_qty - qty_in;
            let new_value = bin.stock_value - reverse_value;
            let new_rate = if new_qty > Decimal::ZERO { rate6(new_value / new_qty) } else { Decimal::ZERO };
            self.bins.update_balance(&mut tx, h.company_id, item, h.warehouse_id, new_qty, new_rate, new_value).await?;
            sle_no += 1;
            self.sles.insert_sle(&mut tx, &NewSleRow {
                company_id: h.company_id, item_id: item, warehouse_id: h.warehouse_id, posting_date: h.posting_date,
                actual_qty: -qty_in, qty_after_txn: new_qty, incoming_rate: Decimal::ZERO,
                valuation_rate: new_rate, stock_value: new_value, stock_value_difference: -reverse_value,
                voucher_type: "purchase_receipt", voucher_id: id, voucher_no: &h.receipt_number, sle_no,
            }).await?;
            total += reverse_value;
        }
        self.receipts.mark_cancelled(&mut tx, id).await?;
        tx.commit().await?;

        // GL reversal: swap the original Dr Inventory · Cr GR/IR.
        let env = AccountingPostEnvelope {
            idempotency_key: format!("{id}-reversal"), company_id: h.company_id, branch_id: h.branch_id,
            source_type: "inventory".into(), source_id: id, source_reference: Some(h.receipt_number.clone()),
            posting_date: h.posting_date, currency: h.currency.clone(), posting_type: "reversal".into(),
            reverses_post_id: Some(orig_post_id),
            description: Some("Goods receipt cancellation".into()),
            lines: vec![
                GlPostLine::debit(h.grir_account_id, total).with_description("GR/IR clearing"),
                GlPostLine::credit(h.inventory_account_id, total).with_description("Inventory"),
            ],
        };
        self.emit_reversal_and_reconcile(GlVoucher::PurchaseReceipt, id, &env, sink, total).await
    }

    // ---- cancel: Delivery Note (reverse the outflow + reversal GL post) -------------------

    pub async fn cancel_delivery_note(&self, id: Uuid, sink: &dyn GlPostSink) -> Result<SubmitOutcome, InventoryError> {
        let h = self.deliveries.fetch_cancel_header(&self.db_pool, id).await?
            .ok_or(InventoryError::NotFound(id))?;

        if h.status == "cancelled" {
            return self.delivery_reversal_gl(id, h, sink).await;
        }
        if h.status != "submitted" {
            return Err(InventoryError::NotSubmitted(id));
        }
        let orig_post_id = h.gl.accounting_post_id.ok_or(InventoryError::GlNotPosted(id))?;

        let items = company_scope::with_company_scope(
            Some(h.company_id),
            self.delivery_items.fetch_cancel_items(&self.db_pool, id),
        ).await?;

        let mut tx = self.db_pool.begin().await?;
        company_scope::bind_company_on(&mut tx, h.company_id).await?;
        let mut sle_no = self.sles.fetch_max_sle_no(&mut tx, "delivery_note", id).await?;
        let mut total = Decimal::ZERO;
        for it in &items {
            let (item, qty, cogs) = (it.item_id, it.quantity, it.cogs_amount);
            let bin = self.bins.lock_or_init(&mut tx, h.company_id, item, h.warehouse_id).await?;
            // Reverse the outflow: qty comes back, value restored by the exact COGS consumed.
            let new_qty = bin.actual_qty + qty;
            let new_value = bin.stock_value + cogs;
            let new_rate = if new_qty > Decimal::ZERO { rate6(new_value / new_qty) } else { Decimal::ZERO };
            self.bins.update_balance(&mut tx, h.company_id, item, h.warehouse_id, new_qty, new_rate, new_value).await?;
            sle_no += 1;
            self.sles.insert_sle(&mut tx, &NewSleRow {
                company_id: h.company_id, item_id: item, warehouse_id: h.warehouse_id, posting_date: h.posting_date,
                actual_qty: qty, qty_after_txn: new_qty, incoming_rate: Decimal::ZERO,
                valuation_rate: new_rate, stock_value: new_value, stock_value_difference: cogs,
                voucher_type: "delivery_note", voucher_id: id, voucher_no: &h.delivery_number, sle_no,
            }).await?;
            total += cogs;
        }
        self.deliveries.mark_cancelled(&mut tx, id).await?;
        tx.commit().await?;

        // GL reversal: swap the original Dr COGS · Cr Inventory.
        let env = AccountingPostEnvelope {
            idempotency_key: format!("{id}-reversal"), company_id: h.company_id, branch_id: h.branch_id,
            source_type: "inventory".into(), source_id: id, source_reference: Some(h.delivery_number.clone()),
            posting_date: h.posting_date, currency: h.currency.clone(), posting_type: "reversal".into(),
            reverses_post_id: Some(orig_post_id),
            description: Some("Delivery cancellation".into()),
            lines: vec![
                GlPostLine::debit(h.inventory_account_id, total).with_description("Inventory"),
                GlPostLine::credit(h.cogs_account_id, total).with_description("COGS"),
            ],
        };
        self.emit_reversal_and_reconcile(GlVoucher::DeliveryNote, id, &env, sink, total).await
    }

    // ---- shared: the GL-only recovery leg for an already-cancelled voucher ----------------
    //
    // Reached when status is already 'cancelled': the physical reversal (SLE + Bin) committed in a
    // prior call. If the reversal GL also landed, short-circuit with the recorded ids; if it didn't
    // (crash window, or a transient accounting outage), rebuild the reversal envelope from the stored
    // header total and re-emit — never re-touching the SLE/Bin.

    async fn receipt_reversal_gl(
        &self, id: Uuid, h: ReceiptCancelHeaderRow, sink: &dyn GlPostSink,
    ) -> Result<SubmitOutcome, InventoryError> {
        if let Some(rid) = h.reversal_accounting_post_id {
            return Ok(SubmitOutcome {
                voucher_id: id, posted: true, journal_id: h.reversal_journal_id, post_id: Some(rid), gl_amount: Decimal::ZERO,
            });
        }
        let orig_post_id = h.gl.accounting_post_id.ok_or(InventoryError::GlNotPosted(id))?;
        let amt = h.total_value;
        let env = AccountingPostEnvelope {
            idempotency_key: format!("{id}-reversal"), company_id: h.company_id, branch_id: h.branch_id,
            source_type: "inventory".into(), source_id: id, source_reference: Some(h.receipt_number.clone()),
            posting_date: h.posting_date, currency: h.currency.clone(), posting_type: "reversal".into(),
            reverses_post_id: Some(orig_post_id),
            description: Some("Goods receipt cancellation (repost)".into()),
            lines: vec![
                GlPostLine::debit(h.grir_account_id, amt).with_description("GR/IR clearing"),
                GlPostLine::credit(h.inventory_account_id, amt).with_description("Inventory"),
            ],
        };
        self.emit_reversal_and_reconcile(GlVoucher::PurchaseReceipt, id, &env, sink, amt).await
    }

    async fn delivery_reversal_gl(
        &self, id: Uuid, h: DeliveryCancelHeaderRow, sink: &dyn GlPostSink,
    ) -> Result<SubmitOutcome, InventoryError> {
        if let Some(rid) = h.reversal_accounting_post_id {
            return Ok(SubmitOutcome {
                voucher_id: id, posted: true, journal_id: h.reversal_journal_id, post_id: Some(rid), gl_amount: Decimal::ZERO,
            });
        }
        let orig_post_id = h.gl.accounting_post_id.ok_or(InventoryError::GlNotPosted(id))?;
        let amt = h.total_cogs;
        let env = AccountingPostEnvelope {
            idempotency_key: format!("{id}-reversal"), company_id: h.company_id, branch_id: h.branch_id,
            source_type: "inventory".into(), source_id: id, source_reference: Some(h.delivery_number.clone()),
            posting_date: h.posting_date, currency: h.currency.clone(), posting_type: "reversal".into(),
            reverses_post_id: Some(orig_post_id),
            description: Some("Delivery cancellation (repost)".into()),
            lines: vec![
                GlPostLine::debit(h.inventory_account_id, amt).with_description("Inventory"),
                GlPostLine::credit(h.cogs_account_id, amt).with_description("COGS"),
            ],
        };
        self.emit_reversal_and_reconcile(GlVoucher::DeliveryNote, id, &env, sink, amt).await
    }
}
