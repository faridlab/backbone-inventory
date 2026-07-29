//! The Purchase Receipt path: draft → submit → repost (hand-authored, user-owned).
//!
//! An `impl InventoryWriteService` chunk over the vocabulary in [`super::inventory_write_service`]:
//! the goods-in voucher. `create_purchase_receipt` opens a draft; `submit_purchase_receipt` runs the
//! **inflow** half of the moving-average valuation engine (`value += qty·rate; qty += qty; rate =
//! value/qty`) — the physical movement (SLE + Bin) commits first, then a balanced `AccountingPost`
//! (`Dr Inventory · Cr GR/IR`) is emitted and eventually reconciled; `repost_purchase_receipt` is the
//! exit from a stuck `failed` GL post.
//!
//! Per the module's 4-layer rule this file holds no SQL — the statements live on
//! `PurchaseReceiptRepository` / `PurchaseReceiptItemRepository` / `BinRepository` /
//! `StockLedgerEntryRepository`, whose write methods take THIS service's transaction so the SLE + Bin
//! + header status commit together with the receipt.

use backbone_orm::company_scope;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::infrastructure::persistence::{GlVoucher, NewReceiptItemRow, NewReceiptRow, NewSleRow};

use super::inventory_events::{InventoryEvent, StockReceived};
use super::inventory_gl::{AccountingPostEnvelope, GlPostLine, GlPostSink};
use super::inventory_write_service::{
    is_dup, money, rate6, InventoryError, InventoryWriteService, NewReceipt, SubmitOutcome,
};

impl InventoryWriteService {
    pub async fn create_purchase_receipt(&self, r: NewReceipt) -> Result<Uuid, InventoryError> {
        if r.lines.is_empty() { return Err(InventoryError::EmptyDocument); }
        for l in &r.lines {
            if l.quantity < Decimal::ZERO || l.rate < Decimal::ZERO { return Err(InventoryError::NegativeQuantity); }
        }
        let total: Decimal = r.lines.iter().map(|l| money(l.quantity * l.rate)).sum();
        let id = Uuid::new_v4();
        let mut tx = self.db_pool.begin().await?;
        company_scope::bind_company_on(&mut tx, r.company_id).await?;
        let ins = self.receipts.insert_draft(&mut tx, &NewReceiptRow {
            id,
            receipt_number: &r.receipt_number,
            company_id: r.company_id,
            branch_id: r.branch_id,
            supplier_id: r.supplier_id,
            source_po_id: r.source_po_id,
            warehouse_id: r.warehouse_id,
            posting_date: r.posting_date,
            currency: &r.currency,
            total_value: total,
            inventory_account_id: r.inventory_account_id,
            grir_account_id: r.grir_account_id,
        }).await;
        if let Err(e) = ins {
            return Err(if is_dup(&e) { InventoryError::DuplicateNumber(r.receipt_number) } else { e.into() });
        }
        for l in &r.lines {
            self.receipt_items.insert_item(&mut tx, &NewReceiptItemRow {
                id: Uuid::new_v4(),
                receipt_id: id,
                company_id: r.company_id,
                item_id: l.item_id,
                quantity: l.quantity,
                rate: l.rate,
                amount: money(l.quantity * l.rate),
            }).await?;
        }
        tx.commit().await?;
        Ok(id)
    }

    // ---- submit: Purchase Receipt (asset post) -----------------------------

    pub async fn submit_purchase_receipt(&self, id: Uuid, sink: &dyn GlPostSink) -> Result<SubmitOutcome, InventoryError> {
        // RLS scope (ADR-0008), ID-only: fenced by the request/inherited scope.
        let hdr = self.receipts.fetch_submit_header(&self.db_pool, id).await?
            .ok_or(InventoryError::NotFound(id))?;
        if hdr.status != "draft" {
            return Err(InventoryError::NotDraft(id.to_string()));
        }
        let company = hdr.company_id;
        let branch = hdr.branch_id;
        let warehouse = hdr.warehouse_id;
        let posting_date = hdr.posting_date;
        let voucher_no = hdr.receipt_number;
        let inv_acct = hdr.inventory_account_id;
        let grir_acct = hdr.grir_account_id;
        let source_po = hdr.source_po_id;

        let items = company_scope::with_company_scope(
            Some(company),
            self.receipt_items.fetch_items(&self.db_pool, id),
        ).await?;

        // ---- physical movement: SLE + Bin, one transaction ----
        let mut tx = self.db_pool.begin().await?;
        // RLS scope (ADR-0008): MUST be bound before any bin read — an unbound connection is fenced
        // to zero rows, so the FOR UPDATE below would read every bin as empty.
        company_scope::bind_company_on(&mut tx, company).await?;
        let mut total_debit = Decimal::ZERO;
        let mut sle_no = 0i32;
        for it in &items {
            let (item, qty, in_rate) = (it.item_id, it.quantity, it.rate);
            let bin = self.bins.lock_or_init(&mut tx, company, item, warehouse).await?;
            let in_amount = money(qty * in_rate);
            let new_qty = bin.actual_qty + qty;
            let new_value = bin.stock_value + in_amount;
            let new_rate = if new_qty > Decimal::ZERO { rate6(new_value / new_qty) } else { Decimal::ZERO };
            self.bins.update_balance(&mut tx, company, item, warehouse, new_qty, new_rate, new_value).await?;
            sle_no += 1;
            self.sles.insert_sle(&mut tx, &NewSleRow {
                company_id: company, item_id: item, warehouse_id: warehouse, posting_date,
                actual_qty: qty, qty_after_txn: new_qty, incoming_rate: in_rate,
                valuation_rate: new_rate, stock_value: new_value, stock_value_difference: in_amount,
                voucher_type: "purchase_receipt", voucher_id: id, voucher_no: &voucher_no, sle_no,
            }).await?;
            total_debit += in_amount;
        }
        self.receipts.mark_submitted(&mut tx, id).await?;
        tx.commit().await?;

        // ---- GL post (eventually consistent) ----
        let env = AccountingPostEnvelope {
            idempotency_key: id.to_string(), company_id: company, branch_id: branch,
            source_type: "inventory".into(), source_id: id, source_reference: Some(voucher_no.clone()),
            posting_date, currency: hdr.currency.clone(), posting_type: "original".into(), reverses_post_id: None,
            description: Some("Goods receipt".into()),
            lines: vec![
                GlPostLine::debit(inv_acct, total_debit).with_description("Inventory"),
                GlPostLine::credit(grir_acct, total_debit).with_description("GR/IR clearing"),
            ],
        };
        let outcome = self.emit_and_reconcile(GlVoucher::PurchaseReceipt, id, &env, sink, total_debit).await?;
        self.sink.publish(InventoryEvent::StockReceived(StockReceived {
            receipt_id: id, company_id: company, warehouse_id: warehouse, source_po_id: source_po,
            total_value: total_debit,
        }));
        Ok(outcome)
    }

    // ---- repost: re-drive a stuck GL post (pending/failed) ------------------

    /// Re-emit the GL post for a voucher whose physical movement committed but whose post is
    /// `pending` or `failed` (a transient accounting outage, or a crash between commit and the
    /// status update). This is the exit from a stuck `failed` voucher (council 2026-07-04) — without
    /// it, `(submitted, failed)` is terminal and the subledger silently stops tying to the GL.
    ///
    /// **Idempotent** against the "physically-posted-but-status-not-updated" crash window: it rebuilds
    /// the SAME envelope (`source_id = voucher_id`), and accounting dedupes on
    /// `(company, source_type, source_id, posting_type)` — so a re-emit of an already-posted voucher
    /// returns the original journal, never a second one. An already-`posted` voucher short-circuits
    /// (returns the recorded ids); `not_applicable` is a no-op. Rebuilds the envelope from the stored
    /// header (never re-touches the SLE/Bin — the physical movement already happened).
    pub async fn repost_purchase_receipt(&self, id: Uuid, sink: &dyn GlPostSink) -> Result<SubmitOutcome, InventoryError> {
        // RLS scope (ADR-0008), ID-only: fenced by the request/inherited scope.
        let h = self.receipts.fetch_repost_header(&self.db_pool, id).await?
            .ok_or(InventoryError::NotFound(id))?;
        if let Some(o) = Self::already_settled(&h.gl, id) { return Ok(o); }
        let amt = h.total_value;
        let env = AccountingPostEnvelope {
            idempotency_key: id.to_string(), company_id: h.company_id, branch_id: h.branch_id,
            source_type: "inventory".into(), source_id: id, source_reference: Some(h.receipt_number),
            posting_date: h.posting_date, currency: h.currency.clone(), posting_type: "original".into(), reverses_post_id: None,
            description: Some("Goods receipt (repost)".into()),
            lines: vec![
                GlPostLine::debit(h.inventory_account_id, amt).with_description("Inventory"),
                GlPostLine::credit(h.grir_account_id, amt).with_description("GR/IR clearing"),
            ],
        };
        self.emit_and_reconcile(GlVoucher::PurchaseReceipt, id, &env, sink, amt).await
    }
}
