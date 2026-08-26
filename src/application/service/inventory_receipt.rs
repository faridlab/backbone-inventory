//! The Purchase Receipt path: draft → submit → repost (hand-authored, user-owned).
//!
//! An `impl InventoryWriteService` chunk over the vocabulary in [`super::inventory_write_service`]:
//! the goods-in voucher. `create_purchase_receipt` opens a draft; `submit_purchase_receipt` mints
//! ONE stock move per line through the converged engine (supplier location → the warehouse's
//! stock location) — move application runs the **inflow** half of the moving-average valuation
//! engine (`value += qty·rate; qty += qty; rate = value/qty`): the quant materializes, the Bin
//! reblends, the SLE row mints — then the voucher's ONE balanced `AccountingPost` (`Dr Inventory ·
//! Cr GR/IR`) is emitted and eventually reconciled; `repost_purchase_receipt` is the exit from a
//! stuck `failed` GL post. The moves post no GL of their own: the voucher owns the single
//! envelope its posting_state/repost/reversal machinery keys on.
//!
//! Per the module's 4-layer rule this file holds no SQL — the statements live on
//! `PurchaseReceiptRepository` / `PurchaseReceiptItemRepository` and the engine's repositories.

use backbone_orm::company_scope;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::infrastructure::persistence::{GlVoucher, NewReceiptItemRow, NewReceiptRow};

use super::inventory_events::{InventoryEvent, StockReceived};
use super::inventory_gl::{AccountingPostEnvelope, GlPostLine, GlPostSink};
use super::inventory_move_engine::{BackorderPolicy, MoveGlDirective, NewStockMove};
use super::inventory_write_service::{
    is_dup, money, DoorOwnedGlSink, InventoryError, InventoryWriteService, NewReceipt, SubmitOutcome,
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

    /// Submit the goods-in voucher. Every line mints ONE stock move through the converged
    /// engine (supplier location → the warehouse's stock location): confirm → assign (an
    /// inbound move's supply is unconditionally available, so the execution line covers the
    /// full line) → done, which materializes the destination quant, reblends the Bin
    /// (`value += qty·rate; rate = value/qty`) and mints the SLE row. The voucher keeps its
    /// identity verbatim — header status, the ONE `Dr Inventory · Cr GR/IR` envelope, the
    /// posting_state/repost machinery, the `StockReceived` event; the moves post no GL (empty
    /// directive + the door-owned-GL tripwire sink).
    ///
    /// Not cross-move atomic (each engine verb commits its own transaction): a crash mid-way
    /// leaves the receipt `draft` with some line moves landed — the deterministic move names
    /// (`{receipt_number}/{seq}`, `origin` = the receipt number) make a re-submit RESUME
    /// instead of double-receiving.
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

        // The door's move endpoints: the company's supplier location (virtual source) and the
        // warehouse's stock location (internal destination) — resolve-or-bootstrap each.
        let (supplier_loc, stock_loc) = self.door_move_endpoints(company, warehouse, "supplier").await?;

        // The posting posture (per-company valuation settings; absent row = today's shapes).
        // The receipt side is unchanged in BOTH postures — the header `grir_account_id` IS the
        // interim-received leg (Assumption A1 of the valuation overlay); only the inventory leg
        // consults the location valuation-account override. `periodic` suppresses the
        // real-time post below (the voucher retires to `not_applicable`).
        let posture = self.posting_posture(company).await?;
        let inv_acct = self.inventory_leg_account(stock_loc, inv_acct).await?;

        // ---- physical movement: one minted move per line, engine-driven ----
        for (idx, it) in items.iter().enumerate() {
            if it.quantity.is_zero() { continue; } // a zero line moves nothing and mints nothing
            let name = format!("{}/{}", voucher_no, idx + 1);
            let mid = match self.mint_line_move(NewStockMove {
                name: name.clone(),
                company_id: company,
                item_id: it.item_id,
                demand_qty: it.quantity,
                price_unit: it.rate, // the IN leg values the inflow at the line's rate
                procure_method: "make_to_stock".into(),
                picking_id: None,
                origin: Some(voucher_no.clone()),
                location_id: supplier_loc,
                location_dest_id: stock_loc,
                partner_id: None,
                warehouse_id: Some(warehouse),
                orderpoint_id: None,
                move_orig_ids: vec![],
                move_dest_ids: vec![],
                is_inventory: false,
                scrapped: false,
                forced_value: None,
            }).await? {
                Some(mid) => mid,
                None => continue, // the line already landed in a prior (crashed) attempt
            };
            self.advance_move_to_assigned(mid).await?;
            self.action_done(mid, BackorderPolicy::Never, &MoveGlDirective::default(), &DoorOwnedGlSink).await?;
        }
        // The GL amount is the voucher's own arithmetic (Σ money(qty·rate)) — identical to the
        // Σ of the moves' IN-leg carries by construction.
        let total_debit: Decimal = items.iter().map(|l| money(l.quantity * l.rate)).sum();
        {
            let mut tx = self.db_pool.begin().await?;
            company_scope::bind_company_on(&mut tx, company).await?;
            self.receipts.mark_submitted(&mut tx, id).await?;
            tx.commit().await?;
        }

        // ---- GL post (eventually consistent) ----
        // The explicit account-move gate: a voucher that carries neither value nor quantity
        // posts nothing (stays `not_applicable`); a `periodic` company suppresses the
        // real-time post the same way (the closing flow — a later increment — owns those legs).
        let total_qty: Decimal = items.iter().map(|l| l.quantity).sum();
        if posture.periodic
            || !Self::should_create_account_move(total_debit, total_qty, true)
        {
            company_scope::with_company_scope(
                Some(company),
                self.gl.mark_not_applicable(&self.db_pool, GlVoucher::PurchaseReceipt, id),
            ).await?;
            self.sink.publish(InventoryEvent::StockReceived(StockReceived {
                receipt_id: id, company_id: company, warehouse_id: warehouse, source_po_id: source_po,
                total_value: total_debit,
            }));
            return Ok(SubmitOutcome {
                voucher_id: id, posted: false, journal_id: None, post_id: None,
                gl_amount: Decimal::ZERO,
            });
        }
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
        // The SAME posture the submit ran under (absent row = defaults): a `periodic`
        // company suppresses the real-time post — the unsettled leg retires to
        // `not_applicable` — and the inventory leg resolves the same location
        // valuation-account override the submit used, so the rebuilt envelope is identical.
        let posture = self.posting_posture(h.company_id).await?;
        if posture.periodic {
            company_scope::with_company_scope(
                Some(h.company_id),
                self.gl.mark_not_applicable(&self.db_pool, GlVoucher::PurchaseReceipt, id),
            ).await?;
            return Ok(SubmitOutcome {
                voucher_id: id, posted: false, journal_id: None, post_id: None,
                gl_amount: Decimal::ZERO,
            });
        }
        let (_, stock_loc) = self.door_move_endpoints(h.company_id, h.warehouse_id, "supplier").await?;
        let inv_acct = self.inventory_leg_account(stock_loc, h.inventory_account_id).await?;
        let amt = h.total_value;
        let env = AccountingPostEnvelope {
            idempotency_key: id.to_string(), company_id: h.company_id, branch_id: h.branch_id,
            source_type: "inventory".into(), source_id: id, source_reference: Some(h.receipt_number),
            posting_date: h.posting_date, currency: h.currency.clone(), posting_type: "original".into(), reverses_post_id: None,
            description: Some("Goods receipt (repost)".into()),
            lines: vec![
                GlPostLine::debit(inv_acct, amt).with_description("Inventory"),
                GlPostLine::credit(h.grir_account_id, amt).with_description("GR/IR clearing"),
            ],
        };
        self.emit_and_reconcile(GlVoucher::PurchaseReceipt, id, &env, sink, amt).await
    }
}
