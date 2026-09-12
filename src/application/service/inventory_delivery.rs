//! The Delivery Note path: draft → submit → repost (hand-authored, user-owned).
//!
//! An `impl InventoryWriteService` chunk over the vocabulary in [`super::inventory_write_service`]:
//! the goods-out voucher. `create_delivery_note` opens a draft; `submit_delivery_note` mints ONE
//! stock move per line through the converged engine (the warehouse's stock location → the
//! customer location) — move application runs the **outflow** half of the moving-average
//! valuation engine (`cogs = qty·rate; value -= cogs; qty -= qty; rate unchanged by an
//! outflow`, with the residual-flush rule when the bin drains to 0): the source quant draws,
//! the Bin reblends, the SLE row mints; then the voucher's ONE balanced `AccountingPost`
//! (`Dr COGS · Cr Inventory`) is emitted and eventually reconciled. `repost_delivery_note` is
//! the exit from a stuck `failed` GL post.
//!
//! Per the module's 4-layer rule this file holds no SQL — the statements live on
//! `DeliveryNoteRepository` / `DeliveryNoteItemRepository` and the engine's repositories.
//!
//! Tenancy (ADR-0029): the module is tenant-agnostic. Every write transaction this file opens
//! re-binds the caller's ambient org scope (`relay_ambient_scope`); the composing decorator owns
//! isolation. Cross-module wire fields that still carry a company id are legacy twins filled
//! from the ambient scope's company echo.

use rust_decimal::Decimal;
use uuid::Uuid;

use crate::infrastructure::persistence::{GlVoucher, NewDeliveryItemRow, NewDeliveryRow};

use super::inventory_events::{InventoryEvent, StockDelivered};
use super::inventory_gl::{AccountingPostEnvelope, GlPostLine, GlPostSink};
use super::inventory_move_engine::{BackorderPolicy, MoveGlDirective, NewStockMove};
use super::inventory_write_service::{
    is_dup, legacy_company_echo, money, relay_ambient_scope, DoorOwnedGlSink, InventoryError,
    InventoryWriteService, NewDelivery, SubmitOutcome,
};

impl InventoryWriteService {
    pub async fn create_delivery_note(&self, d: NewDelivery) -> Result<Uuid, InventoryError> {
        if d.lines.is_empty() { return Err(InventoryError::EmptyDocument); }
        for l in &d.lines {
            if l.quantity < Decimal::ZERO { return Err(InventoryError::NegativeQuantity); }
        }
        let id = Uuid::new_v4();
        let mut tx = self.db_pool.begin().await?;
        relay_ambient_scope(&mut tx).await?;
        let ins = self.deliveries.insert_draft(&mut tx, &NewDeliveryRow {
            id,
            delivery_number: &d.delivery_number,
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
                item_id: l.item_id,
                quantity: l.quantity,
            }).await?;
        }
        tx.commit().await?;
        Ok(id)
    }

    // ---- submit: Delivery Note (COGS post) ---------------------------------

    /// Submit the goods-out voucher. Every line mints ONE stock move through the converged
    /// engine (the warehouse's stock location → the customer location): confirm → assign
    /// (reserves against the source quants — the door refuses a line the reservation cannot
    /// cover WHOLE, preserving the voucher's all-or-nothing availability posture) → done,
    /// which releases the reservation, draws the source quant, reblends the Bin (`cogs =
    /// qty·rate; rate unchanged`; residual flush when the bin drains to 0) and mints the SLE
    /// row. The voucher keeps its identity verbatim — header status + per-line COGS snapshot,
    /// the ONE `Dr COGS · Cr Inventory` envelope, the posting_state/repost machinery, the
    /// `StockDelivered` event; the moves post no GL (empty directive + the door-owned-GL
    /// tripwire sink).
    ///
    /// Not cross-move atomic (each engine verb commits its own transaction): a crash mid-way
    /// leaves the delivery `draft` with some line moves landed — the deterministic move names
    /// (`{delivery_number}/{seq}`, `origin` = the delivery number) make a re-submit RESUME
    /// instead of double-shipping. A refusal mid-way (a line the reservation cannot cover)
    /// unreserves that line's move and leaves the voucher `draft`: no stock moved, no
    /// reservation held.
    pub async fn submit_delivery_note(&self, id: Uuid, sink: &dyn GlPostSink) -> Result<SubmitOutcome, InventoryError> {
        // RLS scope (ADR-0008), ID-only: fenced by the request/inherited scope.
        let hdr = self.deliveries.fetch_submit_header(&self.db_pool, id).await?
            .ok_or(InventoryError::NotFound(id))?;
        if hdr.status != "draft" {
            return Err(InventoryError::NotDraft(id.to_string()));
        }
        let branch = hdr.branch_id;
        let warehouse = hdr.warehouse_id;
        let posting_date = hdr.posting_date;
        let voucher_no = hdr.delivery_number;
        let cogs_acct = hdr.cogs_account_id;
        let inv_acct = hdr.inventory_account_id;
        let source_so = hdr.source_so_id;

        // The posting posture (the valuation settings row; absent row = today's shapes).
        // Resolved BEFORE anything mints so the anglo-saxon posture's fail-closed account
        // check refuses the delivery with NOTHING moved and nothing posted: under the
        // posture the debit leg is the interim-delivered account, and an unconfigured one
        // must not fall back to COGS silently.
        let posture = self.posting_posture().await?;
        let debit_acct = self.delivery_debit_account(&posture, cogs_acct)?;

        let items = self.delivery_items.fetch_items(&self.db_pool, id).await?;

        // The door's move endpoints: the warehouse's stock location (internal source) and the
        // customer location (virtual destination) — resolve-or-bootstrap each.
        let (customer_loc, stock_loc) = self.door_move_endpoints(warehouse, "customer").await?;
        // The inventory credit leg resolves the same location valuation-account override the
        // receipt path uses (the chain: location override → header account).
        let inv_acct = self.inventory_leg_account(stock_loc, inv_acct).await?;

        // ---- all-or-nothing availability pre-check, under the Bin locks -----------------------
        // Same posture the voucher path always had: EVERY line's demand must be coverable
        // before anything mints (the engine's draw guard re-checks at the quant grain). Also
        // captures each line's valuation-rate snapshot — the per-line COGS copy the voucher
        // rows carry.
        let mut line_rates: std::collections::HashMap<Uuid, Decimal> = std::collections::HashMap::new();
        {
            let mut tx = self.db_pool.begin().await?;
            // Re-bind the caller's ambient org scope before any bin read (ADR-0029) — the
            // scope is task-local and a fresh pool transaction carries none of it;
            // undecorated (module tests, jobs) the transaction stays plain.
            relay_ambient_scope(&mut tx).await?;
            for it in &items {
                if it.quantity.is_zero() { continue; }
                let bin = self.bins.lock_or_init(&mut tx, it.item_id, warehouse).await?;
                line_rates.insert(it.item_id, bin.valuation_rate);
                if bin.actual_qty < it.quantity {
                    return Err(InventoryError::InsufficientStock {
                        item_id: it.item_id, warehouse_id: warehouse,
                        available: bin.actual_qty, requested: it.quantity,
                    });
                }
                // The quant-surface heal: stock seeded through the legacy direct-write paths
                // wrote Bins without quants — seed the quant from the (locked) Bin once, so the
                // line move's reservation has a surface to reserve against.
                self.pickings.ensure_quant_surface(&mut tx, it.item_id, stock_loc, Some(warehouse)).await?;
            }
            tx.commit().await?;
        }

        // ---- physical movement: one minted move per line, engine-driven ----
        let mut total_cogs = Decimal::ZERO;
        for (idx, it) in items.iter().enumerate() {
            if it.quantity.is_zero() { continue; } // a zero line moves nothing and mints nothing
            let name = format!("{}/{}", voucher_no, idx + 1);
            let mid = match self.mint_line_move(NewStockMove {
                name: name.clone(),
                item_id: it.item_id,
                demand_qty: it.quantity,
                price_unit: Decimal::ZERO, // an outflow is valued at the source average, never priced
                procure_method: "make_to_stock".into(),
                picking_id: None,
                origin: Some(voucher_no.clone()),
                location_id: stock_loc,
                location_dest_id: customer_loc,
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
                None => {
                    // The line already landed in a prior (crashed) attempt: its COGS is the
                    // value its move carried — recover it from the move's SLE leg.
                    let prior_cogs = self.sles.sum_move_value(&self.db_pool, &name).await?;
                    total_cogs += prior_cogs;
                    continue;
                }
            };
            let state = self.advance_move_to_assigned(mid).await?;
            if state != "assigned" {
                // The all-or-nothing posture: a line the reservation cannot cover WHOLE
                // refuses the delivery. Release whatever the partial assign took; no stock
                // moved, no reservation held, the voucher stays draft (retryable).
                self.unreserve_move(mid).await?;
                let on_hand = self.quants.fetch_on_hand(&self.db_pool, it.item_id, stock_loc).await?;
                return Err(InventoryError::InsufficientStock {
                    item_id: it.item_id, warehouse_id: warehouse,
                    available: on_hand.on_hand_qty, requested: it.quantity,
                });
            }
            let outcome = self.action_done(mid, BackorderPolicy::Never, &MoveGlDirective::default(), &DoorOwnedGlSink).await?;
            // The engine's OUT leg IS the line's COGS (average or residual-flush — the same
            // arithmetic the voucher path always used); snapshot it on the voucher row.
            let cogs = outcome.out_value;
            let rate = line_rates.get(&it.item_id).copied().unwrap_or(Decimal::ZERO);
            {
                let mut tx = self.db_pool.begin().await?;
                relay_ambient_scope(&mut tx).await?;
                self.delivery_items.update_valuation(&mut tx, it.id, rate, cogs).await?;
                tx.commit().await?;
            }
            total_cogs += cogs;
        }
        {
            let mut tx = self.db_pool.begin().await?;
            relay_ambient_scope(&mut tx).await?;
            self.deliveries.mark_submitted_with_cogs(&mut tx, id, total_cogs).await?;
            tx.commit().await?;
        }

        // The explicit account-move gate: a voucher that carries neither value nor quantity
        // posts nothing (stays `not_applicable`); a `periodic` valuation policy suppresses the
        // real-time post the same way (the closing flow — a later increment — owns those legs).
        let total_qty: Decimal = items.iter().map(|l| l.quantity).sum();
        if posture.periodic
            || !Self::should_create_account_move(total_cogs, total_qty, true)
        {
            self.gl.mark_not_applicable(&self.db_pool, GlVoucher::DeliveryNote, id).await?;
            self.sink.publish(InventoryEvent::StockDelivered(StockDelivered {
                delivery_id: id, company_id: legacy_company_echo(), warehouse_id: warehouse, source_so_id: source_so,
                total_cogs,
            }));
            return Ok(SubmitOutcome {
                voucher_id: id, posted: false, journal_id: None, post_id: None,
                gl_amount: Decimal::ZERO,
            });
        }
        let env = AccountingPostEnvelope {
            // Legacy twin (ADR-0029): filled from the ambient org scope's company echo for
            // consumers that still read a tenant off the wire. No module statement keys on it.
            idempotency_key: id.to_string(), company_id: legacy_company_echo(), branch_id: branch,
            source_type: "inventory".into(), source_id: id, source_reference: Some(voucher_no.clone()),
            posting_date, currency: hdr.currency.clone(), posting_type: "original".into(), reverses_post_id: None,
            description: Some("Delivery COGS".into()),
            lines: vec![
                // Under the anglo-saxon posture `debit_acct` is the interim-delivered leg
                // (COGS recognition deferred to the invoice side); else today's COGS debit.
                GlPostLine::debit(debit_acct, total_cogs).with_description("COGS"),
                GlPostLine::credit(inv_acct, total_cogs).with_description("Inventory"),
            ],
        };
        let outcome = self.emit_and_reconcile(GlVoucher::DeliveryNote, id, &env, sink, total_cogs).await?;
        self.sink.publish(InventoryEvent::StockDelivered(StockDelivered {
            delivery_id: id, company_id: legacy_company_echo(), warehouse_id: warehouse, source_so_id: source_so,
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
        // The SAME posture the submit ran under (absent row = defaults): the anglo-saxon
        // debit swap applies to the rebuilt envelope identically (fail-closed on an
        // unconfigured interim account — a repost must not silently diverge from its
        // original), a `periodic` valuation policy retires the unsettled leg to
        // `not_applicable`, and the inventory leg resolves the same location override the
        // submit used.
        let posture = self.posting_posture().await?;
        if posture.periodic {
            self.gl.mark_not_applicable(&self.db_pool, GlVoucher::DeliveryNote, id).await?;
            return Ok(SubmitOutcome {
                voucher_id: id, posted: false, journal_id: None, post_id: None,
                gl_amount: Decimal::ZERO,
            });
        }
        let debit_acct = self.delivery_debit_account(&posture, h.cogs_account_id)?;
        let (_, stock_loc) = self.door_move_endpoints(h.warehouse_id, "customer").await?;
        let inv_acct = self.inventory_leg_account(stock_loc, h.inventory_account_id).await?;
        let amt = h.total_cogs;
        let env = AccountingPostEnvelope {
            // Legacy twin (ADR-0029): filled from the ambient org scope's company echo for
            // consumers that still read a tenant off the wire. No module statement keys on it.
            idempotency_key: id.to_string(), company_id: legacy_company_echo(), branch_id: h.branch_id,
            source_type: "inventory".into(), source_id: id, source_reference: Some(h.delivery_number),
            posting_date: h.posting_date, currency: h.currency.clone(), posting_type: "original".into(), reverses_post_id: None,
            description: Some("Delivery COGS (repost)".into()),
            lines: vec![
                GlPostLine::debit(debit_acct, amt).with_description("COGS"),
                GlPostLine::credit(inv_acct, amt).with_description("Inventory"),
            ],
        };
        self.emit_and_reconcile(GlVoucher::DeliveryNote, id, &env, sink, amt).await
    }
}
