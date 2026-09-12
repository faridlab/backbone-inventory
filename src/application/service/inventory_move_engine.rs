//! The converged stock-move engine (hand-authored, user-owned): `_action_confirm` /
//! `_action_assign` / `_action_done` / `_action_cancel` over quant-grain reservation.
//!
//! Spec: `docs/odoo/inventory/stock/stock-business-logic.md` §1 (7-state lifecycle), §2 (T1
//! picking projection), §3 (reservation triangle), §4 (`_action_done` pipeline); the declared
//! contract is `schema/hooks/stock.hook.yaml`. The MOVE owns the lifecycle
//! (draft/waiting/confirmed/partially_available/assigned/done/cancel); the transfer/picking state
//! is a PROJECTION re-derived on every move change (never its own state machine, ADR-0016).
//!
//! **The reservation triangle** (§3): `stock_quants.reserved_quantity` is AUTHORITATIVE — written
//! only under the quant row's `FOR UPDATE` (`QuantRepository`); each move LINE mirrors the grain it
//! reserved (item x src x dest x lot x package x owner x qty); the move STATE aggregates the mirror
//! (`assigned` when the mirror meets demand, `partially_available` when positive but short).
//! `available = quantity - reserved` is a READ everywhere (T2), never a second writer.
//!
//! **`_action_done`** (§4): guards R22 (physical stock suffices for every draw) + R23 (positive
//! qty) + R24-at-move-grain (done needs lines) + R9 (src != dest); then per line the TWO-STEP
//! `_synchronize_quant` — the reserved step releases the line's reservation, the available step
//! moves the physical quantity (src quant decremented, dest quant incremented); done-qty propagates
//! along `move_orig_ids`/`move_dest_ids` (a `waiting` child releases to `confirmed` once ALL its
//! parents are done); a partial done (qty < demand) mints the BACKORDER move per policy; `date` is
//! stamped to the processing instant and `state='done'`; the transfer reprojects (T1).
//!
//! **Valuation (money path — unchanged contract).** The pipeline mints Stock Ledger Entries through
//! the EXISTING `StockLedgerEntryRepository` + the Bin moving-average reblende, and posts the GL
//! legs through the existing `AccountingPost` seam (`inventory_gl.rs`). There is NO
//! stock.valuation.layer and no second ledger: receipt blends the rate (`value += qty·price; rate =
//! value/qty`), delivery consumes the current average (`cogs = qty·rate; rate unchanged`),
//! transfer is value-neutral at the carried rate — the W1-proven legs. The V7 ordering invariant
//! (stock-account §2.1: OUT valued BEFORE, IN valued AFTER) is structural here: the OUT leg's value
//! is computed from the src Bin snapshot taken under the canonical FOR UPDATE locks BEFORE the dest
//! reblend runs; the IN leg then reblends the destination with the carried value.
//!
//! **Voucher identity note.** Move-minted SLE rows ride `voucher_type='stock_entry'` (the generic
//! stock-operation door) with `voucher_id = move_id` — the `voucher_type` enum has no
//! `stock_move` variant, and none is needed: consumers discriminate by the id columns, not the
//! type string. GL legs are emitted with `idempotency_key = move_id` under
//! `source_type='inventory'`, `source_id = move_id`, `posting_type='original'` — the same
//! producer name the voucher doors use, disambiguated by the source id (each post's source is
//! exactly one document: a voucher OR a move, never both). The move's GL settlement is recorded
//! on the move itself: `_action_done` arms `posting_state='pending'` in the movement's own
//! transaction when it built an envelope, and reconciles to `posted`/`failed` once the sink
//! answers. A rejected post does NOT roll the physical movement back — the move parks in
//! `failed` (a durable, sweepable record of the missing GL leg) and `repost_move_gl` re-drives
//! it; accounting's dedupe on `(company, source_type, source_id, posting_type)` makes the
//! re-drive idempotent. Moves that post no GL of their own (voucher-door legs whose GL is owned
//! by the voucher, value-neutral warehouse-to-warehouse shapes, directives without the needed
//! accounts) stay `not_applicable`.
//!
//! Per the module's 4-layer rule this file holds no SQL — the statements live on
//! `QuantRepository` / `StockMoveRepository` / `StockMoveLineRepository` / `BinRepository` /
//! `StockLedgerEntryRepository`, whose write methods take THIS service's transaction so the quant
//! flips, the Bin reblende, the SLE rows and the move state commit as one unit.

use async_trait::async_trait;
use chrono::Utc;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::domain::entity::MoveState;
use crate::infrastructure::persistence::{MoveRow, NewMoveLineRow, NewMoveRow, QuantDims};

use super::inventory_events::{
    BackorderCreated, InventoryEvent, MoveAssigned, MoveCancelled, MoveConfirmed, MoveDone,
    TransferProjected,
};
use super::inventory_gl::{AccountingPostEnvelope, GlPostLine, GlPostSink};
use super::inventory_posture::Posture;
use super::inventory_write_service::{
    legacy_company_echo, money, rate6, relay_ambient_scope, InventoryError, InventoryWriteService,
    SubmitOutcome,
};
use super::procurement_service::{MovePipeline, MovePipelineError};

// --- input vocabulary ---------------------------------------------------------

/// The move-creation input (draft insert; the pipeline advances state).
#[derive(Debug, Clone)]
pub struct NewStockMove {
    pub name: String,
    pub item_id: Uuid,
    pub demand_qty: Decimal,
    /// Unit valuation price the SLE mint uses on the IN leg (external receipts). 0 = carry the
    /// current average (value-neutral receive).
    pub price_unit: Decimal,
    /// "make_to_stock" | "make_to_order" | "mts_else_mto".
    pub procure_method: String,
    pub picking_id: Option<Uuid>,
    pub origin: Option<String>,
    pub location_id: Uuid,
    pub location_dest_id: Uuid,
    pub partner_id: Option<Uuid>,
    pub warehouse_id: Option<Uuid>,
    pub orderpoint_id: Option<Uuid>,
    pub move_orig_ids: Vec<Uuid>,
    pub move_dest_ids: Vec<Uuid>,
    pub is_inventory: bool,
    pub scrapped: bool,
    /// Explicit total value the move carries — a document reversal valued at its original
    /// amount. When set, the valuation core uses it on the value-bearing leg(s) instead of the
    /// average/price-derived carry, so reversing a voucher restores the estate to exactly its
    /// pre-movement state. `None` on ordinary moves.
    pub forced_value: Option<Decimal>,
}

/// Backorder policy on partial validate (spec §4 step 4: `create_backorder ∈ {always, never,
/// delayed}` — the operation type's vocabulary). `Always` mints the backorder CONFIRMED and
/// reserves it right away (reserve on mint); `Delayed` mints it CONFIRMED but defers the
/// reservation to the scheduler's assign sweep; `Never` leaves the remainder unbackordered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackorderPolicy {
    Always,
    Never,
    Delayed,
}

/// GL accounts for the legs `_action_done` may post. Accounts are inventory/composition config —
/// the same posture as the intake contract (the caller supplies them; they are not move columns).
/// A leg whose accounts are absent is simply not posted (`gl_posted=false`), the physical movement
/// is unaffected.
#[derive(Debug, Clone, Default)]
pub struct MoveGlDirective {
    /// COGS debit account for OUT legs (internal -> customer).
    pub cogs_account_id: Option<Uuid>,
    /// Inventory valuation account (credit side of OUT, debit side of IN).
    pub inventory_account_id: Option<Uuid>,
    /// GR/IR clearing account (credit side of external IN legs).
    pub grir_account_id: Option<Uuid>,
    /// Adjustment account for `is_inventory` moves (the value-diff counterleg).
    pub adjustment_account_id: Option<Uuid>,
    /// Ledger currency of the post.
    pub currency: String,
}

/// Outcome of `_action_done`.
#[derive(Debug, Clone)]
pub struct MoveDoneOutcome {
    pub move_id: Uuid,
    pub done_qty: Decimal,
    /// The minted backorder move (remaining demand), if any.
    pub backorder_move_id: Option<Uuid>,
    /// SLE rows minted by the valuation core (0 for value-neutral same-warehouse moves).
    pub sle_count: i32,
    /// The value the OUT leg carried off the source bin (COGS for a delivery shape).
    pub out_value: Decimal,
    /// The value the IN leg blended into the destination bin (the line amount for a receipt
    /// shape).
    pub in_value: Decimal,
    pub gl_posted: bool,
    pub gl_amount: Decimal,
}

/// Outcome of `_action_assign`.
#[derive(Debug, Clone)]
pub struct MoveAssignOutcome {
    pub move_id: Uuid,
    /// The move's post-assign state: `assigned` (fully reserved), `partially_available` (some
    /// reserved), `confirmed` (nothing reservable at the source location).
    pub state: String,
    pub reserved_qty: Decimal,
}

/// The move lifecycle engine. An `impl InventoryWriteService` chunk over the vocabulary in
/// [`super::inventory_write_service`] — the converged stock pipeline the routes/scheduler surfaces
/// drive.
impl InventoryWriteService {
    // ---- create -------------------------------------------------------------

    /// Create a DRAFT move (spec §1: `draft → waiting/confirmed` happens on confirm, never at
    /// insert). Guards: R9 (src != dest), non-negative demand (R23 at the door), R13 (a quant
    /// surface can never be a view location — checked for both endpoints that hold stock).
    pub async fn create_move(&self, m: NewStockMove) -> Result<Uuid, InventoryError> {
        if m.demand_qty < Decimal::ZERO || m.price_unit < Decimal::ZERO {
            return Err(InventoryError::NegativeQuantity);
        }
        if m.location_id == m.location_dest_id {
            return Err(InventoryError::SameLocation { move_id: Uuid::new_v4(), location_id: m.location_id });
        }
        let id = Uuid::new_v4();
        let mut tx = self.db_pool.begin().await?;
        // Re-bind the caller's ambient org scope onto this transaction before the location
        // reads (ADR-0029) — the scope is task-local and a fresh pool transaction carries none
        // of it. Under the composed shape the decorator's fence bounds every read here;
        // undecorated (module tests, jobs) the transaction stays plain.
        relay_ambient_scope(&mut tx).await?;
        let locs = self.moves.fetch_move_locations(&mut tx, m.location_id, m.location_dest_id).await?;
        let (src, dst) = match locs {
            (Some(s), Some(d)) => (s, d),
            (None, _) => return Err(InventoryError::LocationNotFound(m.location_id)),
            (_, None) => return Err(InventoryError::LocationNotFound(m.location_dest_id)),
        };
        for loc in [&src, &dst] {
            if loc.usage == "view" {
                return Err(InventoryError::ViewLocationHoldsNoStock { location_id: loc.id });
            }
        }
        self.moves.insert_move(&mut tx, &NewMoveRow {
            id,
            name: &m.name,
            item_id: m.item_id,
            demand_qty: m.demand_qty,
            price_unit: m.price_unit,
            procure_method: &m.procure_method,
            picking_id: m.picking_id,
            origin: m.origin.as_deref(),
            location_id: m.location_id,
            location_dest_id: m.location_dest_id,
            partner_id: m.partner_id,
            warehouse_id: m.warehouse_id,
            orderpoint_id: m.orderpoint_id,
            move_orig_ids: m.move_orig_ids.clone(),
            move_dest_ids: m.move_dest_ids.clone(),
            is_inventory: m.is_inventory,
            scrapped: m.scrapped,
            forced_value: m.forced_value,
        }).await?;
        // Chain bookkeeping: a move naming its `move_orig_ids` parents gets the REVERSE link
        // written too (each parent's move_dest_ids gains this move) — the done-qty propagation
        // and waiting-release walk the parent's `move_dest_ids`, so both directions must exist
        // (`link_chain` is idempotent per pair).
        for parent in &m.move_orig_ids {
            self.moves.link_chain(&mut tx, *parent, id).await?;
        }
        if let Some(picking) = m.picking_id {
            self.moves.reproject_picking(&mut tx, picking).await?;
        }
        tx.commit().await?;
        Ok(id)
    }

    // ---- _action_confirm ------------------------------------------------------

    /// `draft → confirmed`, or `draft → waiting` while any `move_orig_ids` parent is not done
    /// (spec §1: `waiting` is the state a chained move sits in until its parents land).
    /// `procure_method` gates supply: `make_to_order` / `mts_else_mto` moves are minted by the
    /// procurement (rule) engine, not here — this method only owns the state gate.
    ///
    /// The move read rides the caller's ambient org scope (ADR-0029): under the composed shape
    /// the decorator's fence bounds it; undecorated (module tests, jobs) it is plain.
    pub async fn action_confirm(&self, move_id: Uuid) -> Result<String, InventoryError> {
        let mut tx = self.db_pool.begin().await?;
        relay_ambient_scope(&mut tx).await?;
        let mv = self.moves.fetch_move(&mut tx, move_id).await?
            .ok_or(InventoryError::NotFound(move_id))?;
        if mv.state != "draft" {
            return Err(InventoryError::WrongMoveState { move_id, action: "confirm", current: mv.state });
        }
        let to = self.confirm_core(&mut tx, &mv).await?;
        tx.commit().await?;
        if to == "confirmed" {
            self.sink.publish(InventoryEvent::MoveConfirmed(MoveConfirmed {
                move_id, company_id: legacy_company_echo(), item_id: mv.item_id,
                demand_qty: mv.demand_qty, picking_id: mv.picking_id,
            }));
        }
        Ok(to)
    }

    /// The confirm verb's core on the caller's connection (the ambient org scope already bound
    /// — ADR-0029): the parents-done check, the guarded transition, and the picking reproject.
    /// Shared by the public verb (own transaction) and the [`MovePipeline`] port the scheduler
    /// drives on its per-batch connection.
    async fn confirm_core(
        &self,
        conn: &mut sqlx::PgConnection,
        mv: &MoveRow,
    ) -> Result<String, InventoryError> {
        let parents_done = if mv.move_orig_ids.is_empty() {
            true
        } else {
            let parents = self.moves.fetch_moves(conn, &mv.move_orig_ids).await?;
            parents.iter().all(|p| p.state == "done")
        };
        let to = if parents_done { "confirmed" } else { "waiting" };
        let ok = self.moves.transition_state(conn, mv.id, "draft", to).await?;
        if !ok {
            return Err(InventoryError::WrongMoveState { move_id: mv.id, action: "confirm", current: "raced".into() });
        }
        if let Some(picking) = mv.picking_id {
            self.moves.reproject_picking(conn, picking).await?;
        }
        Ok(to.into())
    }

    // ---- _action_assign -------------------------------------------------------

    /// Reserve against quants at the source location (spec §1 `_action_assign`): the authoritative
    /// `reserved_quantity` is written under each quant's `FOR UPDATE` (competing reservations
    /// serialize there — one winner per unit of stock, R22/R25), a mirror move-line is minted per
    /// reserved quant grain, and the move state aggregates the mirror.
    ///
    /// The move read rides the caller's ambient org scope (ADR-0029): under the composed shape
    /// the decorator's fence bounds it; undecorated (module tests, jobs) it is plain.
    pub async fn action_assign(&self, move_id: Uuid) -> Result<MoveAssignOutcome, InventoryError> {
        let mut tx = self.db_pool.begin().await?;
        relay_ambient_scope(&mut tx).await?;
        let mv = self.moves.fetch_move(&mut tx, move_id).await?
            .ok_or(InventoryError::NotFound(move_id))?;
        if mv.state != "confirmed" && mv.state != "partially_available" {
            return Err(InventoryError::WrongMoveState { move_id, action: "assign", current: mv.state });
        }
        let (to, total) = self.assign_core(&mut tx, &mv).await?;
        let picking = mv.picking_id;
        tx.commit().await?;
        if to == "assigned" {
            self.sink.publish(InventoryEvent::MoveAssigned(MoveAssigned {
                move_id, company_id: legacy_company_echo(), picking_id: picking,
            }));
        }
        Ok(MoveAssignOutcome { move_id, state: to, reserved_qty: total })
    }

    /// The assign verb's core on the caller's connection (the ambient org scope already bound —
    /// ADR-0029): the reservation loop, the mirror mint, the aggregate state transition, the
    /// line re-mirror and the picking reproject. Returns `(new_state, reserved_qty)`. Shared by
    /// the public verb and the [`MovePipeline`] port.
    async fn assign_core(
        &self,
        conn: &mut sqlx::PgConnection,
        mv: &MoveRow,
    ) -> Result<(String, Decimal), InventoryError> {
        let locs = self.moves.fetch_move_locations(conn, mv.location_id, mv.location_dest_id).await?;
        let src = locs.0.ok_or(InventoryError::LocationNotFound(mv.location_id))?;
        // Reservation only draws from INTERNAL stock; an inbound move (supplier source) has nothing
        // to reserve and stays `confirmed`.
        let mut reserved = Decimal::ZERO;
        if src.usage == "internal" {
            let already = self.move_lines.sum_mirror_qty(conn, mv.id).await?;
            let mut remaining = mv.demand_qty - already;
            let candidates = self.quants.fetch_reservation_candidates(conn, mv.item_id, mv.location_id).await?;
            for cand in candidates {
                if remaining <= Decimal::ZERO { break; }
                let dims = QuantDims {
                    item_id: cand.item_id, location_id: cand.location_id,
                    lot_id: cand.lot_id, package_id: cand.package_id, owner_id: cand.owner_id,
                };
                // Lock the quant row, re-read the free availability under the lock (READ COMMITTED
                // sees the competitor's committed reservation), then reserve the min(free, need).
                let locked = self.quants.lock_or_init(conn, dims).await?;
                let free = locked.available();
                if free <= Decimal::ZERO { continue; }
                let take = if free < remaining { free } else { remaining };
                self.quants.adjust_reserved(conn, locked.id, take).await?;
                self.move_lines.insert_line(conn, &NewMoveLineRow {
                    id: Uuid::new_v4(),
                    quantity: take,
                    lot_id: cand.lot_id,
                    package_id: cand.package_id,
                    result_package_id: None,
                    owner_id: cand.owner_id,
                    move_id: mv.id,
                    picking_id: mv.picking_id,
                    location_id: mv.location_id,
                    location_dest_id: mv.location_dest_id,
                    item_id: mv.item_id,
                    state: mv.state.as_str(), // transient: re-mirrored to the post-assign state below
                }).await?;
                reserved += take;
                remaining -= take;
            }
            reserved += already;
        } else {
            // An INBOUND move (non-internal source — supplier/production) has no stock to
            // reserve: its supply is unconditionally available, so `_action_assign` mints the
            // execution line for the full remaining demand (Odoo's incoming-move readiness) —
            // the line records what will land, with no quant touched at the source.
            let already = self.move_lines.sum_mirror_qty(conn, mv.id).await?;
            if already < mv.demand_qty {
                self.move_lines.insert_line(conn, &NewMoveLineRow {
                    id: Uuid::new_v4(),
                    quantity: mv.demand_qty - already,
                    lot_id: None,
                    package_id: None,
                    result_package_id: None,
                    owner_id: None,
                    move_id: mv.id,
                    picking_id: mv.picking_id,
                    location_id: mv.location_id,
                    location_dest_id: mv.location_dest_id,
                    item_id: mv.item_id,
                    state: mv.state.as_str(), // transient: re-mirrored to the post-assign state below
                }).await?;
            }
            reserved = mv.demand_qty;
        }
        let total = if mv.demand_qty < reserved { mv.demand_qty } else { reserved };
        let to = if total >= mv.demand_qty && mv.demand_qty > Decimal::ZERO {
            "assigned"
        } else if total > Decimal::ZERO {
            "partially_available"
        } else {
            "confirmed"
        };
        let ok = self.moves.transition_state(conn, mv.id, mv.state.as_str(), to).await?;
        if !ok {
            return Err(InventoryError::WrongMoveState { move_id: mv.id, action: "assign", current: "raced".into() });
        }
        self.move_lines.mirror_state(conn, mv.id, to).await?;
        if let Some(picking) = mv.picking_id {
            self.moves.reproject_picking(conn, picking).await?;
        }
        Ok((to.into(), total))
    }

    // ---- unreserve (the triangle's release arm) --------------------------------

    /// Release every reservation the move holds (spec §1: cancel frees reservations first; assign
    /// retries also release-then-re.reserve). The mirror lines zero out; the authoritative
    /// `reserved_quantity` drops by exactly what the lines held.
    ///
    /// The move read rides the caller's ambient org scope (ADR-0029): under the composed shape
    /// the decorator's fence bounds it; undecorated (module tests, jobs) it is plain.
    pub async fn unreserve_move(&self, move_id: Uuid) -> Result<Decimal, InventoryError> {
        let mut tx = self.db_pool.begin().await?;
        relay_ambient_scope(&mut tx).await?;
        let mv = self.moves.fetch_move(&mut tx, move_id).await?
            .ok_or(InventoryError::NotFound(move_id))?;
        let released = self.unreserve_lines(&mut tx, &mv).await?;
        tx.commit().await?;
        Ok(released)
    }

    /// Internal: release all live reservations of a move inside the caller's transaction, by
    /// zeroing the mirror lines and re-deriving each touched quant's authoritative counter from
    /// the live mirror (the triangle's self-heal — a quant can never hold a reservation no live
    /// line claims). Returns the quantity the lines held. The zeroed lines stay as the record of
    /// what the move had reserved.
    async fn unreserve_lines(
        &self,
        tx: &mut sqlx::PgConnection,
        mv: &MoveRow,
    ) -> Result<Decimal, InventoryError> {
        let lines = self.move_lines.fetch_lines_for_move(tx, mv.id).await?;
        let mut released = Decimal::ZERO;
        let mut seen_dims: Vec<QuantDims> = Vec::new();
        for line in lines {
            if line.quantity > Decimal::ZERO {
                released += line.quantity;
            }
            self.move_lines.set_quantity(tx, line.id, Decimal::ZERO).await?;
            let dims = QuantDims {
                item_id: line.item_id, location_id: line.location_id,
                lot_id: line.lot_id, package_id: line.package_id, owner_id: line.owner_id,
            };
            if !seen_dims.contains(&dims) { seen_dims.push(dims); }
        }
        for dims in seen_dims {
            let quant = self.quants.lock_or_init(tx, dims).await?;
            self.quants.recompute_reserved_from_mirror(tx, quant.id, dims).await?;
        }
        Ok(released)
    }

    // ---- _action_done ----------------------------------------------------------

    /// The mutation core (spec §4). See the module docs above for the full pipeline; the guards,
    /// the two-step quant sync, the V7-ordered valuation, the chain propagation, the backorder
    /// split and the projection all live here.
    ///
    /// The move read rides the caller's ambient org scope (ADR-0029): under the composed shape
    /// the decorator's fence bounds it; undecorated (module tests, jobs) it is plain.
    pub async fn action_done(
        &self,
        move_id: Uuid,
        backorder: BackorderPolicy,
        gl: &MoveGlDirective,
        sink: &dyn GlPostSink,
    ) -> Result<MoveDoneOutcome, InventoryError> {
        let mut tx = self.db_pool.begin().await?;
        relay_ambient_scope(&mut tx).await?;
        let mv = self.moves.fetch_move(&mut tx, move_id).await?
            .ok_or(InventoryError::NotFound(move_id))?;
        match mv.state.as_str() {
            "assigned" | "partially_available" | "confirmed" => {}
            other => return Err(InventoryError::WrongMoveState { move_id, action: "done", current: other.into() }),
        }

        let lines = self.move_lines.fetch_lines_for_move(&mut tx, move_id).await?;
        if lines.is_empty() {
            return Err(InventoryError::MoveLinesRequired { move_id }); // R24 at move grain
        }
        let mut done_qty = Decimal::ZERO;
        for l in &lines {
            if l.quantity < Decimal::ZERO {
                return Err(InventoryError::NegativeQuantity); // R23
            }
            done_qty += l.quantity;
        }
        if done_qty <= Decimal::ZERO && mv.demand_qty > Decimal::ZERO {
            return Err(InventoryError::MoveLinesRequired { move_id }); // nothing to validate
        }

        let locs = self.moves.fetch_move_locations(&mut tx, mv.location_id, mv.location_dest_id).await?;
        let src = locs.0.ok_or(InventoryError::LocationNotFound(mv.location_id))?;
        let dst = locs.1.ok_or(InventoryError::LocationNotFound(mv.location_dest_id))?;

        // -- the done write FIRST, so the lines drop out of the live-reservation mirror before
        //    the reserved step re-derives the quant's counter (the two steps below are ordered
        //    by exactly this dependency) -------------------------------------------------------
        let now = Utc::now();
        let ok = self.moves.mark_done(&mut tx, move_id, mv.state.as_str(), done_qty, now).await?;
        if !ok {
            return Err(InventoryError::WrongMoveState { move_id, action: "done", current: "raced".into() });
        }
        self.move_lines.mirror_state(&mut tx, move_id, "done").await?;

        // -- Step 1 of _synchronize_quant (the RESERVED step): re-derive each touched quant's
        //    authoritative reserved_quantity from the live mirror. Flipping this move's lines to
        //    `done` dropped them out of the SUM, so the reservation releases here — including any
        //    residual a shrunk line would otherwise strand. One recompute per DISTINCT source
        //    dimension tuple; quants that flip to zero on-hand keep their zeroed counter.
        let mut seen_dims: Vec<QuantDims> = Vec::new();
        for line in &lines {
            if line.quantity <= Decimal::ZERO { continue; }
            let src_dims = QuantDims {
                item_id: line.item_id, location_id: line.location_id,
                lot_id: line.lot_id, package_id: line.package_id, owner_id: line.owner_id,
            };
            if seen_dims.contains(&src_dims) { continue; }
            seen_dims.push(src_dims);
            let quant = self.quants.lock_or_init(&mut tx, src_dims).await?;
            self.quants.recompute_reserved_from_mirror(&mut tx, quant.id, src_dims).await?;
        }

        // -- Step 2 of _synchronize_quant (the AVAILABLE step): the physical flip, guarded by R22
        //    at quant grain — the PHYSICAL on-hand must cover the draw (the reserved <= available
        //    invariant itself is held by the row lock + the CHECK backstop). Only an INTERNAL
        //    source is drawn from: the quant estate tracks our own stock, so an inbound move
        //    (supplier/production source) has no source quant to decrement — its supply materializes
        //    at the destination quant below.
        let draw_from_src = src.usage == "internal";
        for line in &lines {
            if line.quantity <= Decimal::ZERO { continue; }
            if draw_from_src {
                let src_dims = QuantDims {
                    item_id: line.item_id, location_id: line.location_id,
                    lot_id: line.lot_id, package_id: line.package_id, owner_id: line.owner_id,
                };
                let src_quant = self.quants.lock_or_init(&mut tx, src_dims).await?;
                if src_quant.quantity < line.quantity {
                    return Err(InventoryError::InsufficientStock {
                        item_id: mv.item_id,
                        warehouse_id: mv.warehouse_id.unwrap_or(mv.location_id),
                        available: src_quant.quantity,
                        requested: line.quantity,
                    });
                }
                self.quants.apply_qty(&mut tx, src_quant.id, -line.quantity, Decimal::ZERO).await?;
            }
            let dst_dims = QuantDims {
                item_id: line.item_id, location_id: line.location_dest_id,
                lot_id: line.lot_id, package_id: line.result_package_id.or(line.package_id), owner_id: line.owner_id,
            };
            let dst_quant = self.quants.lock_or_init(&mut tx, dst_dims).await?;
            self.quants.apply_qty(&mut tx, dst_quant.id, line.quantity, Decimal::ZERO).await?;
        }

        // -- valuation core (V7: OUT valued before, IN after) --------------------------------
        let (sle_count, out_value, in_value) = self
            .mint_move_valuation(&mut tx, &mv, &src, &dst, done_qty)
            .await?;

        // -- GL arming (inside the movement's transaction) -----------------------------------
        // Build the envelope BEFORE commit and arm the GL leg in the SAME transaction as the
        // physical movement: a move either lands done with its GL leg `pending`, or not at all.
        // A move that builds no envelope (voucher-door legs whose GL is owned by the voucher,
        // value-neutral shapes, a directive without the accounts this leg needs, a `periodic`
        // posture whose real-time posts are suppressed) stays `not_applicable` — the engine
        // posts nothing on its behalf.
        let posture = self.posting_posture_on(&mut tx).await?;
        let envelope = self.move_gl_envelope(&mv, &src, &dst, gl, out_value, in_value, &posture);
        if envelope.is_some() {
            self.moves.set_posting_pending(&mut tx, move_id).await?;
        }

        // -- backorder split (partial validate) ----------------------------------------------
        // Policy vocabulary (the operation type's `create_backorder`): the remainder becomes its
        // own move on the SAME picking (the transfer stays open below `done` while it lives).
        // Both minting policies CONFIRM the backorder — a confirmed move is what the
        // scheduler's assign sweep and a re-validate can drive; a draft one is invisible to
        // both. `Always` then reserves right away (reserve on mint); `Delayed` leaves the
        // reservation to the scheduler's assign sweep (deferred reservation).
        let mut backorder_move_id = None;
        if done_qty < mv.demand_qty && backorder != BackorderPolicy::Never {
            let remaining = mv.demand_qty - done_qty;
            let child = Uuid::new_v4();
            let backorder_name = format!("{}/BO", mv.name);
            self.moves.insert_move(&mut tx, &NewMoveRow {
                id: child,
                name: &backorder_name,
                item_id: mv.item_id,
                demand_qty: remaining,
                price_unit: mv.price_unit,
                procure_method: &mv.procure_method,
                picking_id: mv.picking_id,
                origin: mv.origin.as_deref(),
                location_id: mv.location_id,
                location_dest_id: mv.location_dest_id,
                partner_id: mv.partner_id,
                warehouse_id: mv.warehouse_id,
                orderpoint_id: mv.orderpoint_id,
                move_orig_ids: vec![move_id],
                move_dest_ids: mv.move_dest_ids.clone(),
                is_inventory: mv.is_inventory,
                scrapped: mv.scrapped,
                forced_value: None, // fresh demand: ordinary valuation, never the parent's reversal carry
            }).await?;
            self.moves.link_chain(&mut tx, move_id, child).await?;
            let child_row = self.moves.fetch_move(&mut tx, child).await?
                .ok_or(InventoryError::NotFound(child))?;
            self.confirm_core(&mut tx, &child_row).await?;
            if backorder == BackorderPolicy::Always {
                let fresh = self.moves.fetch_move(&mut tx, child).await?
                    .ok_or(InventoryError::NotFound(child))?;
                if matches!(fresh.state.as_str(), "confirmed" | "partially_available") {
                    self.assign_core(&mut tx, &fresh).await?;
                }
            }
            backorder_move_id = Some(child);
        }

        // -- done-qty propagation: release waiting children whose parents are all done --------
        let mut released_children: Vec<crate::infrastructure::persistence::MoveRow> = Vec::new();
        if !mv.move_dest_ids.is_empty() {
            let children = self.moves.fetch_moves(&mut tx, &mv.move_dest_ids).await?;
            for ch in &children {
                if ch.state != "waiting" { continue; }
                let parents = self.moves.fetch_moves(&mut tx, &ch.move_orig_ids).await?;
                if parents.iter().all(|p| p.state == "done") {
                    let ok = self.moves.transition_state(&mut tx, ch.id, "waiting", "confirmed").await?;
                    if ok {
                        released_children.push(ch.clone());
                    }
                }
            }
        }

        let mut projected_state: Option<String> = None;
        if let Some(picking) = mv.picking_id {
            projected_state = self.moves.reproject_picking(&mut tx, picking).await?;
        }
        let picking = mv.picking_id;
        tx.commit().await?;

        // -- GL post (eventually consistent — the physical movement already committed) ---------
        // Failure posture: a rejection does NOT roll the physical movement back and does NOT
        // strand the hole silently — the move is marked `failed` (the durable, sweepable record
        // of the missing GL leg) while the error still surfaces to the caller, and
        // `repost_move_gl` re-drives the leg. A crash between this commit and the reconcile
        // leaves the move `pending`; the same repost heals it. Accounting dedupes on the
        // envelope's source identity, so a re-drive can never double post.
        let mut gl_posted = false;
        let mut gl_amount = Decimal::ZERO;
        if let Some(env) = envelope {
            debug_assert!(env.is_balanced());
            gl_amount = env.lines.iter().map(|l| l.debit).sum();
            match sink.post(&env).await {
                Ok(_) => {
                    self.moves.mark_posting_posted(&self.db_pool, move_id).await?;
                    gl_posted = true;
                }
                Err(rej) => {
                    let _ = self.moves.mark_posting_failed(&self.db_pool, move_id).await;
                    return Err(InventoryError::GlRejected { code: rej.code, message: rej.message });
                }
            }
        }

        // -- events ----------------------------------------------------------------------------
        self.sink.publish(InventoryEvent::MoveDone(MoveDone {
            move_id, company_id: legacy_company_echo(), item_id: mv.item_id,
            quantity: done_qty, price_unit: mv.price_unit, is_inventory: mv.is_inventory,
        }));
        if let Some(child) = backorder_move_id {
            self.sink.publish(InventoryEvent::BackorderCreated(BackorderCreated {
                backorder_id: child, company_id: legacy_company_echo(), origin_id: move_id,
            }));
        }
        for ch in released_children {
            self.sink.publish(InventoryEvent::MoveConfirmed(MoveConfirmed {
                move_id: ch.id, company_id: legacy_company_echo(), item_id: ch.item_id,
                demand_qty: ch.demand_qty, picking_id: ch.picking_id,
            }));
        }
        if let Some(p) = picking {
            if let Some(state) = projected_state {
                self.sink.publish(InventoryEvent::TransferProjected(TransferProjected {
                    transfer_id: p, company_id: legacy_company_echo(), state, previous_state: mv.state.clone(),
                }));
            }
        }
        Ok(MoveDoneOutcome { move_id, done_qty, backorder_move_id, sle_count, out_value, in_value, gl_posted, gl_amount })
    }

    // ---- _action_cancel --------------------------------------------------------

    /// `* → cancel` (spec §1): frees the reservation first, then propagates to the chained
    /// children (`move_dest_ids`) unless `propagate_cancel=false` — never into a done move.
    ///
    /// The move read rides the caller's ambient org scope (ADR-0029): under the composed shape
    /// the decorator's fence bounds it; undecorated (module tests, jobs) it is plain.
    pub async fn action_cancel(&self, move_id: Uuid) -> Result<Decimal, InventoryError> {
        let mut tx = self.db_pool.begin().await?;
        relay_ambient_scope(&mut tx).await?;
        let mv = self.moves.fetch_move(&mut tx, move_id).await?
            .ok_or(InventoryError::NotFound(move_id))?;
        match mv.state.as_str() {
            "draft" | "waiting" | "confirmed" | "partially_available" | "assigned" => {}
            other => return Err(InventoryError::WrongMoveState { move_id, action: "cancel", current: other.into() }),
        }
        let released = self.unreserve_lines(&mut tx, &mv).await?;
        let ok = self.moves.transition_state(&mut tx, move_id, mv.state.as_str(), "cancel").await?;
        if !ok {
            return Err(InventoryError::WrongMoveState { move_id, action: "cancel", current: "raced".into() });
        }
        self.move_lines.mirror_state(&mut tx, move_id, "cancel").await?;
        if mv.propagate_cancel && !mv.move_dest_ids.is_empty() {
            let children = self.moves.fetch_moves(&mut tx, &mv.move_dest_ids).await?;
            for ch in children {
                if matches!(ch.state.as_str(), "done" | "cancel") { continue; }
                let _ = self.moves.transition_state(&mut tx, ch.id, ch.state.as_str(), "cancel").await?;
                self.move_lines.mirror_state(&mut tx, ch.id, "cancel").await?;
            }
        }
        if let Some(picking) = mv.picking_id {
            self.moves.reproject_picking(&mut tx, picking).await?;
        }
        tx.commit().await?;
        self.sink.publish(InventoryEvent::MoveCancelled(MoveCancelled {
            move_id, company_id: legacy_company_echo(), released_qty: released,
        }));
        Ok(released)
    }

    // ---- GL repost: re-drive a stuck move GL leg ---------------------------------

    /// Re-drive the GL leg of a done move whose `posting_state` is `pending` or `failed` — a
    /// rejected or interrupted AccountingPost from `_action_done`. The physical movement
    /// (quants, Bin, SLE rows) already happened and is never re-touched: the envelope is rebuilt
    /// from the move's committed ledger legs plus the caller's account directive, re-emitted
    /// under the move's ORIGINAL source identity (`source_type='inventory'`, `source_id` = move
    /// id, `posting_type='original'`), and accounting's dedupe on that identity makes the
    /// re-drive idempotent — a leg that actually landed before the status write was lost returns
    /// the original journal instead of a second one.
    ///
    /// Short-circuits exactly like the voucher doors' repost verbs: `posted` → nothing to drive;
    /// `not_applicable` → this move owns no GL leg (a voucher door posted its voucher's
    /// envelope; a value-neutral shape posts nothing), so a sweep can call this blindly. If the
    /// rebuilt envelope is `None` (the directive no longer supplies the accounts this leg shape
    /// needs), the unsettled leg retires to `not_applicable` — the GL is genuinely not postable
    /// under the current configuration and the move leaves the sweep worklist.
    ///
    /// On a fresh rejection the move parks in `failed` again and the error surfaces, as in
    /// `_action_done`.
    ///
    /// The move read rides the caller's ambient org scope (ADR-0029): under the composed shape
    /// the decorator's fence bounds it; undecorated (module tests, jobs) it is plain.
    pub async fn repost_move_gl(
        &self,
        move_id: Uuid,
        gl: &MoveGlDirective,
        sink: &dyn GlPostSink,
    ) -> Result<SubmitOutcome, InventoryError> {
        let mv = {
            let mut tx = self.db_pool.begin().await?;
            relay_ambient_scope(&mut tx).await?;
            let mv = self.moves.fetch_move(&mut tx, move_id).await?
                .ok_or(InventoryError::NotFound(move_id))?;
            tx.commit().await?;
            mv
        };
        match mv.posting_state.as_str() {
            "posted" => return Ok(SubmitOutcome {
                voucher_id: move_id, posted: true, journal_id: None, post_id: None,
                gl_amount: Decimal::ZERO,
            }),
            "not_applicable" => return Ok(SubmitOutcome {
                voucher_id: move_id, posted: false, journal_id: None, post_id: None,
                gl_amount: Decimal::ZERO,
            }),
            _ => {} // pending | failed → re-drive below
        }
        if mv.state != "done" {
            // posting_state only leaves not_applicable inside a done transaction, so this is a
            // defensive guard — but a hand-mangled row should fail loudly, not re-post.
            return Err(InventoryError::WrongMoveState { move_id, action: "repost_gl", current: mv.state });
        }
        let locs = {
            let mut tx = self.db_pool.begin().await?;
            relay_ambient_scope(&mut tx).await?;
            let locs = self.moves.fetch_move_locations(&mut tx, mv.location_id, mv.location_dest_id).await?;
            tx.commit().await?;
            locs
        };
        let src = locs.0.ok_or(InventoryError::LocationNotFound(mv.location_id))?;
        let dst = locs.1.ok_or(InventoryError::LocationNotFound(mv.location_dest_id))?;
        // The SAME posture the original done-transaction consulted: a `periodic` posture's
        // move legs retire to `not_applicable` (the real-time post is genuinely not wanted
        // under the current configuration), exactly like a directive that no longer supplies
        // the accounts.
        let posture = self.posting_posture().await?;
        if posture.periodic {
            self.moves.mark_posting_not_applicable(&self.db_pool, move_id).await?;
            return Ok(SubmitOutcome {
                voucher_id: move_id, posted: false, journal_id: None, post_id: None,
                gl_amount: Decimal::ZERO,
            });
        }
        let (out_value, in_value) = self.sles.move_leg_values(&self.db_pool, move_id).await?;
        let envelope = self.move_gl_envelope(&mv, &src, &dst, gl, out_value, in_value, &posture);
        let Some(env) = envelope else {
            self.moves.mark_posting_not_applicable(&self.db_pool, move_id).await?;
            return Ok(SubmitOutcome {
                voucher_id: move_id, posted: false, journal_id: None, post_id: None,
                gl_amount: Decimal::ZERO,
            });
        };
        debug_assert!(env.is_balanced());
        let gl_amount = env.lines.iter().map(|l| l.debit).sum();
        match sink.post(&env).await {
            Ok(ack) => {
                self.moves.mark_posting_posted(&self.db_pool, move_id).await?;
                Ok(SubmitOutcome {
                    voucher_id: move_id, posted: true,
                    journal_id: Some(ack.journal_id), post_id: Some(ack.post_id),
                    gl_amount,
                })
            }
            Err(rej) => {
                let _ = self.moves.mark_posting_failed(&self.db_pool, move_id).await;
                Err(InventoryError::GlRejected { code: rej.code, message: rej.message })
            }
        }
    }

    // ---- valuation core (private) -----------------------------------------------

    /// The Bin/SLE half of `_action_done` — the moving-average engine over the move's legs, with
    /// the W1-proven arithmetic (receipt blends, delivery consumes, transfer carries; residual
    /// flush when a bin drains to zero) and the V7 ordering (OUT valued from the locked src
    /// snapshot BEFORE the dest reblend). Returns `(sle_count, out_value, in_value)`.
    ///
    /// Skips valuation entirely (returns zeros) when neither side resolves to an internal
    /// warehouse bin, and for same-warehouse moves (the bin grain is (item, warehouse): a move
    /// inside one warehouse changes no balance — the quant flips above carry the physical truth).
    async fn mint_move_valuation(
        &self,
        tx: &mut sqlx::PgConnection,
        mv: &crate::infrastructure::persistence::MoveRow,
        src: &crate::infrastructure::persistence::LocationFacts,
        dst: &crate::infrastructure::persistence::LocationFacts,
        qty: Decimal,
    ) -> Result<(i32, Decimal, Decimal), InventoryError> {
        let src_wh = if src.usage == "internal" { src.warehouse_id } else { None };
        let dst_wh = if dst.usage == "internal" { dst.warehouse_id } else { None };
        if src_wh.is_none() && dst_wh.is_none() { return Ok((0, Decimal::ZERO, Decimal::ZERO)); }
        if let (Some(a), Some(b)) = (src_wh, dst_wh) {
            if a == b { return Ok((0, Decimal::ZERO, Decimal::ZERO)); }
        }
        let posting_date = Utc::now().date_naive();
        let mut sle_no = self.sles.fetch_max_sle_no(tx, "stock_entry", mv.id).await?;

        // Lock BOTH bins in canonical warehouse order (the deadlock rule proven by the transfer
        // path: opposing moves on the same pair serialize instead of deadlocking).
        let mut legs: Vec<(Uuid, crate::infrastructure::persistence::BinBalanceRow)> = Vec::new();
        for wh in [src_wh, dst_wh].into_iter().flatten() {
            let bal = self.bins.lock_or_init(tx, mv.item_id, wh).await?;
            legs.push((wh, bal));
        }
        // `BinBalanceRow` is not Clone; `Decimal` is Copy, so reconstruct on lookup.
        let balance_of = |wh: Uuid| {
            legs.iter().find(|(w, _)| *w == wh).map(|(_, b)| crate::infrastructure::persistence::BinBalanceRow {
                actual_qty: b.actual_qty, valuation_rate: b.valuation_rate, stock_value: b.stock_value,
            })
        };

        let mut out_value = Decimal::ZERO;
        let mut in_value = Decimal::ZERO;

        // OUT leg (V7: valued BEFORE the IN reblend, at the src bin's pre-move average).
        if let (Some(wh), Some(bal)) = (src_wh, src_wh.and_then(balance_of)) {
            let out_qty = bal.actual_qty - qty;
            if out_qty < Decimal::ZERO {
                return Err(InventoryError::InsufficientStock {
                    item_id: mv.item_id, warehouse_id: wh,
                    available: bal.actual_qty, requested: qty,
                });
            }
            // A forced value (a document reversal at its original amount) overrides the
            // average/drain-derived carry — and reblends the remaining rate, because the
            // remaining estate's value changed by an amount the old average does not describe.
            // Ordinary moves: residual-flush rule (draining the bin to 0 carries its entire
            // remaining value) or the current 2dp-rounded average.
            let forced = mv.forced_value.is_some();
            out_value = match mv.forced_value {
                Some(fv) => fv,
                None if out_qty.is_zero() => bal.stock_value,
                None => money(qty * bal.valuation_rate),
            };
            let out_stock_value = bal.stock_value - out_value;
            let out_rate = if out_qty > Decimal::ZERO {
                if forced { rate6(out_stock_value / out_qty) } else { bal.valuation_rate }
            } else { Decimal::ZERO };
            self.bins.update_balance(tx, mv.item_id, wh, out_qty, out_rate, out_stock_value).await?;
            sle_no += 1;
            self.sles.insert_sle(tx, &crate::infrastructure::persistence::NewSleRow {
                item_id: mv.item_id, warehouse_id: wh, posting_date,
                actual_qty: -qty, qty_after_txn: out_qty, incoming_rate: Decimal::ZERO,
                valuation_rate: out_rate, stock_value: out_stock_value,
                stock_value_difference: -out_value,
                voucher_type: "stock_entry", voucher_id: mv.id, voucher_no: &mv.name, sle_no,
            }).await?;
        }

        // IN leg (V7: valued AFTER — reblends the destination with the carried value).
        if let (Some(wh), Some(bal)) = (dst_wh, dst_wh.and_then(balance_of)) {
            // Carried value: a forced value (a document reversal at its original amount)
            // overrides everything; an external source (receipt) values the inflow at the
            // move's unit price; an internal transfer carries the OUT value. A zero price on
            // an external receive carries the destination's current average (value-neutral
            // receive).
            let carried = if let Some(fv) = mv.forced_value {
                fv
            } else if src.usage != "internal" {
                if mv.price_unit > Decimal::ZERO { money(qty * mv.price_unit) } else { money(qty * bal.valuation_rate) }
            } else {
                out_value
            };
            in_value = carried;
            let in_qty = bal.actual_qty + qty;
            let in_stock_value = bal.stock_value + carried;
            let in_rate = if in_qty > Decimal::ZERO { rate6(in_stock_value / in_qty) } else { Decimal::ZERO };
            self.bins.update_balance(tx, mv.item_id, wh, in_qty, in_rate, in_stock_value).await?;
            sle_no += 1;
            self.sles.insert_sle(tx, &crate::infrastructure::persistence::NewSleRow {
                item_id: mv.item_id, warehouse_id: wh, posting_date,
                actual_qty: qty, qty_after_txn: in_qty, incoming_rate: mv.price_unit,
                valuation_rate: in_rate, stock_value: in_stock_value,
                stock_value_difference: carried,
                voucher_type: "stock_entry", voucher_id: mv.id, voucher_no: &mv.name, sle_no,
            }).await?;
        }

        Ok((sle_no, out_value, in_value))
    }

    // ---- landed-cost revaluation (the one non-move valuation write) --------------------------

    /// Revalue a DONE receipt move's bin by `delta`: ONE value-only SLE row
    /// (`voucher_type='landed_cost'`, `voucher_id` = the landed cost, qty 0, value `delta`)
    /// plus the Bin value reblend (`value += delta; qty unchanged; rate = value/qty`).
    ///
    /// **RETROACTIVE-REVALUATION ASYMMETRY — deliberate, preserved.** `delta` is the
    /// REMAINING-share portion of the move's landed-cost allocation
    /// (`delta = allocation x remaining_qty / done_qty`, computed by the landed-cost door from a
    /// read-only FIFO attribution over the SLE history). The already-CONSUMED portion of the
    /// allocation produces NO correcting entry on this path — no COGS true-up, no journal leg,
    /// no negative-SLE compensation — even though the consumed units also "cost more" now. This
    /// one-sided revalue of already-done moves is the reference ERP's behavior and the
    /// valuation-overlay plan row records it as an explicit keep: the consumed share's cost is
    /// accepted as recognized-at-the-old-average at consumption time, and only the stock still
    /// on hand revalues. Both GL sides of the landed-cost post carry only the remaining-portion
    /// value so the post balances without inventing the missing COGS leg. Reversal is the
    /// negative-amount landed-cost pattern (swapped legs verbatim), never a cancel.
    ///
    /// The one-writer invariant holds: this verb is the ONLY non-engine-internal caller path
    /// that reaches `insert_sle`/`update_balance`, and it lives HERE, inside the engine, beside
    /// [`Self::mint_move_valuation`] — the landed-cost door calls the verb, never the writers.
    /// The engine posts NO GL for this leg (the landed-cost document owns its own envelope).
    ///
    /// Crash-resume: the SLE row's name is deterministic
    /// (`{lc_number}/{move_name}/{move_line_id}` — the move line id is the stable target grain;
    /// worksheet row ids are transient and must not appear in ledger names), so a re-drive of a
    /// crashed validation finds the row via [`Self::has_sle_named`] semantics and skips it,
    /// exactly like a door resuming a half-landed submit. Takes the CALLER'S connection: the
    /// revaluation SLE, the bin reblend and the landed-cost document's state flip commit as one
    /// unit. Returns `Ok(())` without minting when `delta` is zero (nothing to revalue) or the
    /// row already landed.
    pub(super) async fn adjust_move_value(
        &self,
        tx: &mut sqlx::PgConnection,
        mv: &crate::infrastructure::persistence::MoveRow,
        move_line_id: Uuid,
        delta: Decimal,
        lc_id: Uuid,
        lc_number: &str,
    ) -> Result<(), InventoryError> {
        if delta.is_zero() {
            return Ok(()); // an all-consumed target line revalues nothing
        }
        // The bin grain is (item, warehouse): resolve the warehouse the move's IN leg blended
        // into — the destination location's warehouse, the same resolution mint_move_valuation
        // used, so the reblend lands on the bin the original inflow created.
        let locs = self.moves.fetch_move_locations(tx, mv.location_id, mv.location_dest_id).await?;
        let dst = locs.1.ok_or(InventoryError::LocationNotFound(mv.location_dest_id))?;
        if dst.usage != "internal" {
            // The target set is the receipt's DONE moves that blended value into a bin; a move
            // whose destination is not an internal warehouse minted no valuation to revalue.
            return Err(InventoryError::LandedCostNoValuationAccount { move_id: mv.id });
        }
        let wh = dst.warehouse_id
            .ok_or(InventoryError::LandedCostNoValuationAccount { move_id: mv.id })?;
        let name = format!("{}/{}/{}", lc_number, mv.name, move_line_id);
        if self.sles.has_sle_named(tx, "landed_cost", lc_id, &name).await? {
            return Ok(()); // this leg already landed in a prior (crashed) attempt — resume, never double-mint
        }
        let bal = self.bins.lock_or_init(tx, mv.item_id, wh).await?;
        let new_value = bal.stock_value + delta;
        let new_qty = bal.actual_qty; // a landed cost moves no quantity, only value
        let new_rate = if new_qty > Decimal::ZERO {
            rate6(new_value / new_qty)
        } else {
            Decimal::ZERO
        };
        self.bins.update_balance(tx, mv.item_id, wh, new_qty, new_rate, new_value).await?;
        let sle_no = self.sles.fetch_max_sle_no(tx, "landed_cost", lc_id).await? + 1;
        self.sles.insert_sle(tx, &crate::infrastructure::persistence::NewSleRow {
            item_id: mv.item_id, warehouse_id: wh,
            posting_date: chrono::Utc::now().date_naive(),
            actual_qty: Decimal::ZERO, qty_after_txn: new_qty,
            incoming_rate: Decimal::ZERO, valuation_rate: new_rate,
            stock_value: new_value, stock_value_difference: delta,
            voucher_type: "landed_cost", voucher_id: lc_id, voucher_no: &name, sle_no,
        }).await?;
        Ok(())
    }

    /// Build the GL envelope for the done move's leg shape, or `None` when the shape posts no GL
    /// (cross-warehouse internal transfer: value-neutral), the directive lacks the accounts, the
    /// `periodic` posture suppresses real-time stock posts, or the move carries neither value
    /// nor quantity (the explicit account-move gate).
    fn move_gl_envelope(
        &self,
        mv: &crate::infrastructure::persistence::MoveRow,
        src: &crate::infrastructure::persistence::LocationFacts,
        dst: &crate::infrastructure::persistence::LocationFacts,
        gl: &MoveGlDirective,
        out_value: Decimal,
        in_value: Decimal,
        posture: &Posture,
    ) -> Option<AccountingPostEnvelope> {
        // A `periodic` posture posts no real-time stock GL — the closing flow (a later
        // increment) owns those legs; the move stays `not_applicable`.
        if posture.periodic {
            return None;
        }
        // The explicit account-move gate (`_should_create_account_move` port): no envelope
        // when the move carries zero value AND zero qty, or the accounts its leg shape
        // requires are unset — both stay `not_applicable`.
        let shape_accounts_set = if mv.is_inventory {
            gl.inventory_account_id.is_some() && gl.adjustment_account_id.is_some()
        } else if src.usage == "internal" && dst.usage != "internal" {
            gl.cogs_account_id.is_some() && gl.inventory_account_id.is_some()
        } else if src.usage != "internal" && dst.usage == "internal" {
            gl.inventory_account_id.is_some() && gl.grir_account_id.is_some()
        } else {
            false // internal cross-warehouse: value-neutral, never posts
        };
        if !Self::should_create_account_move(out_value + in_value, mv.quantity, shape_accounts_set) {
            return None;
        }
        // The account-resolution chain for the inventory legs: the location's
        // valuation-account override when set, else the directive's account (the
        // door-header equivalent). Smallest-first override: a location beats the directive.
        let (description, lines) = if mv.is_inventory {
            // Adjustment shape (reconciliation vocabulary): the signed value diff between the
            // inventory account and the adjustment counterleg. The inventory leg resolves
            // the override of whichever side is the internal (counted) location.
            let inv = if dst.usage == "internal" { dst.valuation_account_id } else { src.valuation_account_id }
                .or(gl.inventory_account_id)?;
            let adj = gl.adjustment_account_id?;
            if in_value > out_value {
                ("Inventory adjustment".to_string(), vec![
                    GlPostLine::debit(inv, in_value - out_value).with_description("Inventory"),
                    GlPostLine::credit(adj, in_value - out_value).with_description("Adjustment"),
                ])
            } else if out_value > in_value {
                ("Inventory adjustment".to_string(), vec![
                    GlPostLine::debit(adj, out_value - in_value).with_description("Adjustment"),
                    GlPostLine::credit(inv, out_value - in_value).with_description("Inventory"),
                ])
            } else {
                return None;
            }
        } else if src.usage == "internal" && dst.usage != "internal" {
            // OUT (delivery shape): Dr COGS / Cr Inventory — the W1-proven leg. The inventory
            // leg prefers the SOURCE location's valuation-account override.
            ("Stock issue".to_string(), vec![
                GlPostLine::debit(gl.cogs_account_id?, out_value).with_description("COGS"),
                GlPostLine::credit(src.valuation_account_id.or(gl.inventory_account_id)?, out_value).with_description("Inventory"),
            ])
        } else if src.usage != "internal" && dst.usage == "internal" {
            // IN (receipt shape): Dr Inventory / Cr GR/IR. The inventory leg prefers the
            // DESTINATION location's valuation-account override.
            ("Goods receipt".to_string(), vec![
                GlPostLine::debit(dst.valuation_account_id.or(gl.inventory_account_id)?, in_value).with_description("Inventory"),
                GlPostLine::credit(gl.grir_account_id?, in_value).with_description("GR/IR clearing"),
            ])
        } else {
            // Internal cross-warehouse: value-neutral, no GL (the transfer-path contract).
            return None;
        };
        Some(AccountingPostEnvelope {
            idempotency_key: mv.id.to_string(),
            // Legacy twin (ADR-0029): filled from the ambient org scope's company echo for
            // consumers that still read a tenant off the wire. No module statement keys on it.
            company_id: legacy_company_echo(),
            branch_id: None,
            source_type: "inventory".into(),
            source_id: mv.id,
            source_reference: Some(mv.name.clone()),
            posting_date: Utc::now().date_naive(),
            currency: if gl.currency.is_empty() { "IDR".into() } else { gl.currency.clone() },
            posting_type: "original".into(),
            reverses_post_id: None,
            description: Some(description),
            lines,
        })
    }
}

// --- the scheduler's MovePipeline port ------------------------------------------

/// The write service IS the move engine, so it implements the port the daily scheduler drives
/// (`procurement_service::MovePipeline`): the per-move confirm/assign verbs on the CALLER'S
/// connection — the scheduler runs them inside its per-batch transaction, so a batch's
/// transitions commit or roll back together. Failures cross the boundary as a flat
/// `{code, message}` (the engine's own error taxonomy stays on this side of the port). Events
/// publish immediately through the advisory sink; the batch commit boundary is the scheduler's
/// concern.
#[async_trait]
impl MovePipeline for InventoryWriteService {
    async fn confirm(
        &self,
        conn: &mut sqlx::PgConnection,
        move_id: Uuid,
    ) -> Result<MoveState, MovePipelineError> {
        let pipe = |e: InventoryError| MovePipelineError { code: e.code(), message: e.to_string() };
        let mv = self
            .moves
            .fetch_move(conn, move_id)
            .await
            .map_err(|e| pipe(InventoryError::Db(e)))?
            .ok_or_else(|| pipe(InventoryError::NotFound(move_id)))?;
        if mv.state != "draft" {
            return Err(pipe(InventoryError::WrongMoveState { move_id, action: "confirm", current: mv.state }));
        }
        relay_ambient_scope(conn).await.map_err(|e| pipe(InventoryError::Db(e)))?;
        let to = self.confirm_core(conn, &mv).await.map_err(pipe)?;
        if to == "confirmed" {
            self.sink.publish(InventoryEvent::MoveConfirmed(MoveConfirmed {
                move_id,
                company_id: legacy_company_echo(),
                item_id: mv.item_id,
                demand_qty: mv.demand_qty,
                picking_id: mv.picking_id,
            }));
        }
        to.parse::<MoveState>()
            .map_err(|e| MovePipelineError { code: "bad_move_state".into(), message: e })
    }

    async fn assign(
        &self,
        conn: &mut sqlx::PgConnection,
        move_id: Uuid,
    ) -> Result<MoveState, MovePipelineError> {
        let pipe = |e: InventoryError| MovePipelineError { code: e.code(), message: e.to_string() };
        let mv = self
            .moves
            .fetch_move(conn, move_id)
            .await
            .map_err(|e| pipe(InventoryError::Db(e)))?
            .ok_or_else(|| pipe(InventoryError::NotFound(move_id)))?;
        if mv.state != "confirmed" && mv.state != "partially_available" {
            return Err(pipe(InventoryError::WrongMoveState { move_id, action: "assign", current: mv.state }));
        }
        relay_ambient_scope(conn).await.map_err(|e| pipe(InventoryError::Db(e)))?;
        let (to, _reserved) = self.assign_core(conn, &mv).await.map_err(pipe)?;
        if to == "assigned" {
            self.sink.publish(InventoryEvent::MoveAssigned(MoveAssigned {
                move_id, company_id: legacy_company_echo(), picking_id: mv.picking_id,
            }));
        }
        to.parse::<MoveState>()
            .map_err(|e| MovePipelineError { code: "bad_move_state".into(), message: e })
    }
}
