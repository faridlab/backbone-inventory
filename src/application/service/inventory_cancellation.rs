//! The cancellation/reversal path (hand-authored, user-owned) — council 2026-07-29, finding #3.
//!
//! An `impl InventoryWriteService` chunk: `cancel_purchase_receipt` and `cancel_delivery_note`. A
//! cancellation REVERSES a submitted voucher: per original line it mints ONE REVERSE stock move
//! through the converged engine (the mirror image of the line's forward move — the moves the
//! submit door minted), drives it confirm → assign → done, so the quant flips, the Bin reblende
//! and the compensating Stock Ledger Entry all come from MOVE APPLICATION — the cancel NEVER
//! writes Bins or SLEs directly and NEVER deletes rows. The header flips submitted→cancelled,
//! and a balanced `posting_type='reversal'` AccountingPost whose `reverses_post_id` references
//! the original post is emitted. The original `journal_id`/`accounting_post_id` stay intact; the
//! reversal's ids land in `reversal_journal_id`/`reversal_accounting_post_id`.
//!
//! **Valuation: `forced_value`.** Each reverse move carries `forced_value` = the value the
//! original line carried (the receipt line's `money(qty·rate)`; the delivery line's stored
//! `cogs_amount` — the whole remaining value for a drain-to-zero line). The valuation core uses
//! it instead of the average/price-derived carry, so the reversed estate returns to EXACTLY its
//! pre-movement state — the same arithmetic the direct-write cancellation produced.
//!
//! **GL legs.** The reverse moves post no GL of their own (empty directive + the door-owned-GL
//! tripwire sink); the voucher's ONE reversal envelope is unchanged — the net GL effect per door
//! is identical to the direct-write era.
//!
//! Idempotent + crash-recoverable: the reverse move names are deterministic
//! (`{voucher_no}/REV/{seq}`, `origin` = the voucher number), so a crash mid-way leaves the
//! voucher `submitted` with some reverse moves landed — a re-call RESUMES (a DONE reverse move
//! is skipped) instead of double-reversing. If the physical reversal committed but the GL
//! reversal failed (status='cancelled', `reversal_accounting_post_id` NULL), re-calling
//! re-emits ONLY the GL — it never re-mints moves. An already-fully-reversed voucher
//! short-circuits with the recorded ids.
//!
//! **Receipt cancel** reverses an INFLOW: push the received qty back out (stock location → the
//! supplier location) and remove the value it added. Requires the stock still hold the qty
//! (else `InsufficientStockToReverse` — the goods were issued), and that the source quant's
//! FREE availability cover the whole reversal (stock reserved for someone else is not
//! returnable stock). GL: swaps the original `Dr Inventory · Cr GR/IR` to `Dr GR/IR · Cr Inventory`.
//!
//! **Delivery cancel** reverses an OUTFLOW: push the delivered qty back in (the customer
//! location → the stock location) and restore the COGS consumed. Always safe (qty only
//! increases). GL: swaps `Dr COGS · Cr Inventory` to `Dr Inventory · Cr COGS`.
//!
//! Per the module's 4-layer rule this file holds no SQL — the statements live on the repositories
//! and the move engine; every engine verb runs its own guarded transaction.
//!
//! Tenancy (ADR-0029): the module is tenant-agnostic. Every write transaction this file opens
//! re-binds the caller's ambient org scope (`relay_ambient_scope`); the composing decorator owns
//! isolation. Cross-module wire fields that still carry a company id are legacy twins filled
//! from the ambient scope's company echo.

use rust_decimal::Decimal;
use uuid::Uuid;

use crate::infrastructure::persistence::{
    DeliveryCancelHeaderRow, GlVoucher, MoveRow, ReceiptCancelHeaderRow,
};

use super::inventory_gl::{AccountingPostEnvelope, GlPostLine, GlPostSink};
use super::inventory_move_engine::{BackorderPolicy, MoveGlDirective, NewStockMove};
use super::inventory_write_service::{
    legacy_company_echo, money, relay_ambient_scope, DoorOwnedGlSink, InventoryError,
    InventoryWriteService, SubmitOutcome,
};

impl InventoryWriteService {
    // ---- cancel: Purchase Receipt (reverse the inflow through the move engine) ------------

    /// Cancel the goods-in voucher: per line, ONE reverse move (stock location → supplier
    /// location) minted with `forced_value = money(qty·rate)` — the exact value the inflow
    /// added — confirm → assign (reserves against the stock quants; the door refuses a line
    /// whose free availability cannot cover the WHOLE reversal) → done, which draws the source
    /// quant, reblends the Bin back to its pre-receipt state and mints the compensating SLE row.
    /// The voucher's ONE reversal envelope (`Dr GR/IR · Cr Inventory`) is unchanged.
    ///
    /// Not cross-move atomic (each engine verb commits its own transaction): a crash mid-way
    /// leaves the voucher `submitted` with some reverse moves landed — the deterministic names
    /// (`{receipt_number}/REV/{seq}`) make a re-cancel RESUME instead of double-reversing.
    pub async fn cancel_purchase_receipt(&self, id: Uuid, sink: &dyn GlPostSink) -> Result<SubmitOutcome, InventoryError> {
        let h = self.receipts.fetch_cancel_header(&self.db_pool, id).await?
            .ok_or(InventoryError::NotFound(id))?;

        // Recovery / idempotency: the physical reversal already committed. Re-emit ONLY the GL leg —
        // never re-mint reverse moves. An already-recorded reversal short-circuits with its ids.
        if h.status == "cancelled" {
            return self.receipt_reversal_gl(id, h, sink).await;
        }
        if h.status != "submitted" {
            return Err(InventoryError::NotSubmitted(id));
        }
        // A reversal references the original post; the original must be posted.
        let orig_post_id = h.gl.accounting_post_id.ok_or(InventoryError::GlNotPosted(id))?;

        let items = self.receipt_items.fetch_items(&self.db_pool, id).await?;

        // The door's move endpoints — the mirror image of the submit door's pair: the warehouse's
        // stock location is now the SOURCE, the supplier location the destination the received
        // goods return to.
        let (supplier_loc, stock_loc) = self.door_move_endpoints(h.warehouse_id, "supplier").await?;

        // The reversal legs already landed in a prior (crashed) attempt: a DONE move under the
        // line's reverse name means the bin draw already happened — the pre-check below skips it.
        let prior = self.door_moves_by_origin(&h.receipt_number).await?;
        let rev_name = |idx: usize| format!("{}/REV/{}", h.receipt_number, idx + 1);

        // ---- all-or-nothing availability pre-check, under the Bin locks -----------------------
        // Same posture the direct-write cancellation always had: EVERY un-landed line's qty must
        // still be on the bin before anything mints (the engine's quant draw guard re-checks at
        // the quant grain). A landed line is skipped — its bin draw already committed.
        {
            let mut tx = self.db_pool.begin().await?;
            // Re-bind the caller's ambient org scope before any bin read (ADR-0029) — the scope
            // is task-local and a fresh pool transaction carries none of it; undecorated
            // (module tests, jobs) the transaction stays plain.
            relay_ambient_scope(&mut tx).await?;
            for (idx, it) in items.iter().enumerate() {
                if it.quantity.is_zero() { continue; }
                // A landed-cost service line minted no stock — there is nothing of it on the
                // bin to check and nothing to reverse.
                if it.is_landed_costs_line { continue; }
                let already_reversed = prior.iter()
                    .any(|m| m.name == rev_name(idx) && m.state == "done");
                if already_reversed { continue; }
                let bin = self.bins.lock_or_init(&mut tx, it.item_id, h.warehouse_id).await?;
                if bin.actual_qty < it.quantity {
                    return Err(InventoryError::InsufficientStockToReverse {
                        item_id: it.item_id, warehouse_id: h.warehouse_id,
                        available: bin.actual_qty, requested: it.quantity,
                    });
                }
                // The quant-surface heal: stock received through the legacy direct-write paths
                // wrote Bins without quants — seed the quant from the (locked) Bin once, so the
                // reverse move's reservation has a surface to reserve against.
                self.pickings.ensure_quant_surface(&mut tx, it.item_id, stock_loc, Some(h.warehouse_id)).await?;
            }
            tx.commit().await?;
        }

        // ---- physical reversal: one reverse move per line, engine-driven -----------------------
        for (idx, it) in items.iter().enumerate() {
            if it.quantity.is_zero() { continue; } // a zero line moved nothing and reverses nothing
            // The landed-cost seam: a flagged line minted no move, so it has no reverse leg
            // either (its cost is recovered by a negative landed cost, not by un-receiving).
            if it.is_landed_costs_line { continue; }
            // The exact value the original inflow added — negating it restores the bin precisely.
            let reverse_value = money(it.quantity * it.rate);
            let mid = match self.mint_line_move(NewStockMove {
                name: rev_name(idx),
                item_id: it.item_id,
                demand_qty: it.quantity,
                price_unit: Decimal::ZERO, // the reversal is valued by its forced_value, never a price
                procure_method: "make_to_stock".into(),
                picking_id: None,
                origin: Some(h.receipt_number.clone()),
                location_id: stock_loc,
                location_dest_id: supplier_loc,
                partner_id: None,
                warehouse_id: Some(h.warehouse_id),
                orderpoint_id: None,
                move_orig_ids: vec![],
                move_dest_ids: vec![],
                is_inventory: false,
                scrapped: false,
                forced_value: Some(reverse_value),
            }).await? {
                Some(mid) => mid,
                None => continue, // the line already reversed in a prior (crashed) attempt
            };
            let state = self.advance_move_to_assigned(mid).await?;
            if state != "assigned" {
                // The stock quant's free availability cannot cover the WHOLE reversal — the
                // received goods are (partly) reserved for someone else. Release whatever the
                // partial assign took; no stock moved, no reservation held, the voucher stays
                // submitted (retryable once the reservation clears).
                self.unreserve_move(mid).await?;
                let on_hand = self.quants.fetch_on_hand(&self.db_pool, it.item_id, stock_loc).await?;
                return Err(InventoryError::InsufficientStockToReverse {
                    item_id: it.item_id, warehouse_id: h.warehouse_id,
                    available: on_hand.on_hand_qty - on_hand.reserved_qty,
                    requested: it.quantity,
                });
            }
            self.action_done(mid, BackorderPolicy::Never, &MoveGlDirective::default(), &DoorOwnedGlSink).await?;
        }
        {
            let mut tx = self.db_pool.begin().await?;
            relay_ambient_scope(&mut tx).await?;
            self.receipts.mark_cancelled(&mut tx, id).await?;
            tx.commit().await?;
        }

        // GL reversal: swap the original Dr Inventory · Cr GR/IR. The amount is the voucher's own
        // arithmetic (Σ money(qty·rate) over the STOCK lines) — identical to the Σ of the reverse
        // moves' forced values; a landed-cost service line contributed nothing on the way in and
        // reverses nothing on the way out.
        // The inventory leg resolves the SAME location valuation-account override the submit
        // used, so the compensation mirrors the original post exactly (posture symmetry).
        let inv_acct = self.inventory_leg_account(stock_loc, h.inventory_account_id).await?;
        let total: Decimal = items.iter()
            .filter(|l| !l.is_landed_costs_line)
            .map(|l| money(l.quantity * l.rate))
            .sum();
        let env = AccountingPostEnvelope {
            // Legacy twin (ADR-0029): filled from the ambient org scope's company echo for
            // consumers that still read a tenant off the wire. No module statement keys on it.
            idempotency_key: format!("{id}-reversal"), company_id: legacy_company_echo(), branch_id: h.branch_id,
            source_type: "inventory".into(), source_id: id, source_reference: Some(h.receipt_number.clone()),
            posting_date: h.posting_date, currency: h.currency.clone(), posting_type: "reversal".into(),
            reverses_post_id: Some(orig_post_id),
            description: Some("Goods receipt cancellation".into()),
            lines: vec![
                GlPostLine::debit(h.grir_account_id, total).with_description("GR/IR clearing"),
                GlPostLine::credit(inv_acct, total).with_description("Inventory"),
            ],
        };
        self.emit_reversal_and_reconcile(GlVoucher::PurchaseReceipt, id, &env, sink, total).await
    }

    // ---- cancel: Delivery Note (reverse the outflow through the move engine) --------------

    /// Cancel the goods-out voucher: per line, ONE reverse move (the customer location → the
    /// stock location) minted with `forced_value = cogs_amount` — the EXACT value the outflow
    /// consumed (the whole remaining value for a drain-to-zero line) — confirm → assign (an
    /// external source's supply is unconditionally available, so the full line is covered) →
    /// done, which materializes the stock quant, reblends the Bin back to its pre-delivery
    /// state and mints the compensating SLE row. The voucher's ONE reversal envelope
    /// (`Dr Inventory · Cr COGS`) is unchanged. Always safe — qty only increases.
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

        let items = self.delivery_items.fetch_cancel_items(&self.db_pool, id).await?;

        // The door's move endpoints — the mirror image of the submit door's pair: the customer
        // location is now the SOURCE, the warehouse's stock location the destination
        // the delivered goods return to.
        let (customer_loc, stock_loc) = self.door_move_endpoints(h.warehouse_id, "customer").await?;

        // ---- physical reversal: one reverse move per line, engine-driven -----------------------
        for (idx, it) in items.iter().enumerate() {
            if it.quantity.is_zero() { continue; } // a zero line moved nothing and reverses nothing
            let name = format!("{}/REV/{}", h.delivery_number, idx + 1);
            let mid = match self.mint_line_move(NewStockMove {
                name,
                item_id: it.item_id,
                demand_qty: it.quantity,
                price_unit: Decimal::ZERO, // the reversal is valued by its forced_value, never a price
                procure_method: "make_to_stock".into(),
                picking_id: None,
                origin: Some(h.delivery_number.clone()),
                location_id: customer_loc,
                location_dest_id: stock_loc,
                partner_id: None,
                warehouse_id: Some(h.warehouse_id),
                orderpoint_id: None,
                move_orig_ids: vec![],
                move_dest_ids: vec![],
                is_inventory: false,
                scrapped: false,
                // The exact COGS the outflow consumed — restoring it reblends the bin exactly
                // (a drain-to-zero line carried the whole remaining value).
                forced_value: Some(it.cogs_amount),
            }).await? {
                Some(mid) => mid,
                None => continue, // the line already reversed in a prior (crashed) attempt
            };
            let state = self.advance_move_to_assigned(mid).await?;
            if state != "assigned" {
                // Unreachable in practice (an external source's supply is unconditionally
                // available, so assign always covers the full demand) — but a delivery cancel
                // must restore the WHOLE line, so a partial advance fails loudly instead of
                // silently short-restoring the estate.
                return Err(InventoryError::WrongMoveState {
                    move_id: mid, action: "cancel", current: state,
                });
            }
            self.action_done(mid, BackorderPolicy::Never, &MoveGlDirective::default(), &DoorOwnedGlSink).await?;
        }
        {
            let mut tx = self.db_pool.begin().await?;
            relay_ambient_scope(&mut tx).await?;
            self.deliveries.mark_cancelled(&mut tx, id).await?;
            tx.commit().await?;
        }

        // GL reversal: swap the original Dr COGS · Cr Inventory. The amount is the voucher's own
        // stored Σ COGS — identical to the Σ of the reverse moves' forced values. The credit leg
        // resolves the SAME posture the original debit used (the anglo-saxon interim-delivered
        // account when the posture is ON — a compensation must mirror what was actually posted)
        // and the inventory leg resolves the same location valuation-account override.
        let posture = self.posting_posture().await?;
        let credit_acct = self.delivery_debit_account(&posture, h.cogs_account_id)?;
        let inv_acct = self.inventory_leg_account(stock_loc, h.inventory_account_id).await?;
        let total: Decimal = items.iter().map(|l| l.cogs_amount).sum();
        let env = AccountingPostEnvelope {
            // Legacy twin (ADR-0029): filled from the ambient org scope's company echo for
            // consumers that still read a tenant off the wire. No module statement keys on it.
            idempotency_key: format!("{id}-reversal"), company_id: legacy_company_echo(), branch_id: h.branch_id,
            source_type: "inventory".into(), source_id: id, source_reference: Some(h.delivery_number.clone()),
            posting_date: h.posting_date, currency: h.currency.clone(), posting_type: "reversal".into(),
            reverses_post_id: Some(orig_post_id),
            description: Some("Delivery cancellation".into()),
            lines: vec![
                GlPostLine::debit(inv_acct, total).with_description("Inventory"),
                GlPostLine::credit(credit_acct, total).with_description("COGS"),
            ],
        };
        self.emit_reversal_and_reconcile(GlVoucher::DeliveryNote, id, &env, sink, total).await
    }

    // ---- shared: the door-minted moves of one voucher origin ------------------------------

    /// The moves minted under one voucher origin (voucher doors stamp the voucher number in
    /// `origin`). The cancel doors read them to tell an already-landed reverse leg from a
    /// fresh one (crash recovery) — the mapping is the deterministic move NAME. The read
    /// rides the caller's ambient org scope (ADR-0029).
    async fn door_moves_by_origin(
        &self,
        origin: &str,
    ) -> Result<Vec<MoveRow>, InventoryError> {
        let mut tx = self.db_pool.begin().await?;
        relay_ambient_scope(&mut tx).await?;
        let rows = self.moves.fetch_moves_by_origin(&mut tx, origin).await?;
        tx.commit().await?;
        Ok(rows)
    }

    // ---- shared: the GL-only recovery leg for an already-cancelled voucher ----------------
    //
    // Reached when status is already 'cancelled': the physical reversal (the reverse moves)
    // committed in a prior call. If the reversal GL also landed, short-circuit with the recorded
    // ids; if it didn't (crash window, or a transient accounting outage), rebuild the reversal
    // envelope from the stored header total and re-emit — never re-minting moves.

    async fn receipt_reversal_gl(
        &self, id: Uuid, h: ReceiptCancelHeaderRow, sink: &dyn GlPostSink,
    ) -> Result<SubmitOutcome, InventoryError> {
        if let Some(rid) = h.reversal_accounting_post_id {
            return Ok(SubmitOutcome {
                voucher_id: id, posted: true, journal_id: h.reversal_journal_id, post_id: Some(rid), gl_amount: Decimal::ZERO,
            });
        }
        let orig_post_id = h.gl.accounting_post_id.ok_or(InventoryError::GlNotPosted(id))?;
        // Same posture + location-override resolution as the forward cancellation, so the
        // recovered reversal mirrors the original post exactly.
        let (_, stock_loc) = self.door_move_endpoints(h.warehouse_id, "supplier").await?;
        let inv_acct = self.inventory_leg_account(stock_loc, h.inventory_account_id).await?;
        let amt = h.total_value;
        let env = AccountingPostEnvelope {
            // Legacy twin (ADR-0029): filled from the ambient org scope's company echo for
            // consumers that still read a tenant off the wire. No module statement keys on it.
            idempotency_key: format!("{id}-reversal"), company_id: legacy_company_echo(), branch_id: h.branch_id,
            source_type: "inventory".into(), source_id: id, source_reference: Some(h.receipt_number.clone()),
            posting_date: h.posting_date, currency: h.currency.clone(), posting_type: "reversal".into(),
            reverses_post_id: Some(orig_post_id),
            description: Some("Goods receipt cancellation (repost)".into()),
            lines: vec![
                GlPostLine::debit(h.grir_account_id, amt).with_description("GR/IR clearing"),
                GlPostLine::credit(inv_acct, amt).with_description("Inventory"),
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
        // Same posture + location-override resolution as the forward cancellation (the credit
        // leg mirrors the original debit — the anglo-saxon interim account when the posture
        // is ON — so the recovered reversal mirrors the original post exactly).
        let posture = self.posting_posture().await?;
        let credit_acct = self.delivery_debit_account(&posture, h.cogs_account_id)?;
        let (_, stock_loc) = self.door_move_endpoints(h.warehouse_id, "customer").await?;
        let inv_acct = self.inventory_leg_account(stock_loc, h.inventory_account_id).await?;
        let amt = h.total_cogs;
        let env = AccountingPostEnvelope {
            // Legacy twin (ADR-0029): filled from the ambient org scope's company echo for
            // consumers that still read a tenant off the wire. No module statement keys on it.
            idempotency_key: format!("{id}-reversal"), company_id: legacy_company_echo(), branch_id: h.branch_id,
            source_type: "inventory".into(), source_id: id, source_reference: Some(h.delivery_number.clone()),
            posting_date: h.posting_date, currency: h.currency.clone(), posting_type: "reversal".into(),
            reverses_post_id: Some(orig_post_id),
            description: Some("Delivery cancellation (repost)".into()),
            lines: vec![
                GlPostLine::debit(inv_acct, amt).with_description("Inventory"),
                GlPostLine::credit(credit_acct, amt).with_description("COGS"),
            ],
        };
        self.emit_reversal_and_reconcile(GlVoucher::DeliveryNote, id, &env, sink, amt).await
    }
}
