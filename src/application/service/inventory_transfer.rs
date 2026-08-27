//! The transfer/picking surface (hand-authored, user-owned).
//!
//! Two write paths live here, both converged onto the ONE stock-move engine
//! ([`super::inventory_move_engine`]):
//!
//! **Picking-as-projection (spec stock-business-logic.md §2 / §12 T1, ADR-0016).** The
//! transfer document is a PROJECTION: `transfers.state` is re-derived from its member move
//! states by the engine on EVERY move change — there is NO per-picking state mutation code in
//! this file (or anywhere in the lane): no hand-set transfer state machine, no transfer-state
//! column writes. `create_picking` mints the header + DRAFT moves through the engine and
//! confirms them; `validate_picking` is Odoo's `button_validate` — `_action_done` over the
//! transfer's moves; the probes READ the projected state. Guards: R3 name/company unique,
//! R24 done needs lines (engine-enforced at move grain).
//!
//! **The stock-entry voucher (warehouse-to-warehouse move), re-wired onto move minting.**
//! The voucher surface keeps its identity verbatim — header + item rows, the `StockMoved`
//! event, the value-neutral no-GL contract — but the physical/valuation legs are now minted
//! by the engine pipeline: one move per line, confirmed, reserved against the source quants,
//! and validated (`_action_done`), which flips the quants (two-step sync) and mints the V7
//!-ordered SLE pair through the moving-average Bin reblende (OUT valued before, IN after).
//!
//! Per the module's 4-layer rule this file holds no SQL — statements live on
//! [`super::super::super::infrastructure::persistence::StockPickingRepository`] (header mint,
//! probes, location resolution, the quant-surface heal) and the engine's repositories; every
//! engine verb runs its own guarded transaction, so a move's quant flips, SLE rows, and GL leg
//! commit as one unit.

use backbone_orm::company_scope;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::infrastructure::persistence::{
    NewMoveLineRow, NewPickingRow, NewStockEntryItemRow, NewTransferRow, ProcurementRepository,
};

use super::inventory_events::{InventoryEvent, StockMoved};
use super::inventory_gl::{AccountingPostEnvelope, GlPostAck, GlPostRejected, GlPostSink};
use super::inventory_move_engine::{BackorderPolicy, MoveGlDirective, NewStockMove};
use super::inventory_write_service::{
    is_dup, InventoryError, InventoryWriteService, NewTransfer,
};

/// The GL sink of the value-neutral transfer door. A warehouse-to-warehouse move posts NO GL
/// (the W1 contract), so the engine's directive carried into `_action_done` never produces an
/// envelope and this sink is never invoked. It exists so the door cannot SILENTLY post — if the
/// valuation shape ever changes to emit one, the loud rejection surfaces it instead of a phantom
/// journal entry nobody asked for.
struct ValueNeutralDoorSink;
#[async_trait::async_trait]
impl GlPostSink for ValueNeutralDoorSink {
    async fn post(&self, _e: &AccountingPostEnvelope) -> Result<GlPostAck, GlPostRejected> {
        Err(GlPostRejected {
            code: "no_gl_on_transfer_door".into(),
            message: "warehouse-to-warehouse moves are value-neutral and post no GL".into(),
        })
    }
}

/// One line of a picking (the member move's demand).
#[derive(Debug, Clone)]
pub struct PickingLine {
    pub item_id: Uuid,
    pub demand_qty: Decimal,
    /// Unit valuation price for external IN legs (0 = carry the current average).
    pub price_unit: Decimal,
}

/// The picking-mint input. `name` is the transfer reference (unique per company — R3).
#[derive(Debug, Clone)]
pub struct NewPicking {
    pub name: String,
    pub company_id: Uuid,
    pub picking_type_id: Uuid,
    pub location_id: Uuid,
    pub location_dest_id: Uuid,
    pub partner_id: Option<Uuid>,
    /// direct (ship as available) / one (ship all at once).
    pub move_type: String,
    pub origin: Option<String>,
    pub lines: Vec<PickingLine>,
}

/// The picking mint outcome: the header id, the member move ids, and the PROJECTED state
/// after every move minted + confirmed (a READ of the projection, never an assertion).
#[derive(Debug, Clone)]
pub struct PickingCreated {
    pub transfer_id: Uuid,
    pub move_ids: Vec<Uuid>,
    pub projected_state: String,
}

/// The `button_validate` outcome: per-move results plus the transfer's final projected state
/// (READ — the projection is what the moves made it, never what this method wanted).
#[derive(Debug, Clone)]
pub struct PickingValidated {
    pub transfer_id: Uuid,
    pub projected_state: String,
    pub validated_moves: Vec<super::inventory_move_engine::MoveDoneOutcome>,
}

/// The picking-assignment outcome: the transfer the move now belongs to, and whether this
/// call MINTED that transfer or JOINED an already-open one.
#[derive(Debug, Clone)]
pub struct PickingAssignment {
    pub transfer_id: Uuid,
    pub minted: bool,
}

impl InventoryWriteService {
    // ---- picking-as-projection: mint / probe / validate ------------------------

    /// Mint a picking: insert the transfer header, then mint one DRAFT move per line through
    /// the engine and confirm each (the operation type's reservation posture fires with
    /// confirm — `at_confirm` reserves immediately). Every mint/confirm REPROJECTS the
    /// transfer (the engine does it on every move change — this method never touches
    /// `transfers.state`). Guards: R3 (name/company unique — the typed duplicate error),
    /// R9 (source != destination), non-empty, non-negative demand.
    ///
    /// Not cross-move atomic: each engine verb commits its own transaction. A failure
    /// mid-way leaves the transfer projected at whatever its minted moves say (typically
    /// `draft`/`confirmed`) — a probe-able, retryable state, never a half-flipped stock
    /// position (stock only moves on `validate_picking`).
    pub async fn create_picking(&self, p: NewPicking) -> Result<PickingCreated, InventoryError> {
        if p.lines.is_empty() { return Err(InventoryError::EmptyDocument); }
        if p.location_id == p.location_dest_id {
            return Err(InventoryError::SameLocation { move_id: Uuid::new_v4(), location_id: p.location_id });
        }
        for l in &p.lines {
            if l.demand_qty < Decimal::ZERO || l.price_unit < Decimal::ZERO {
                return Err(InventoryError::NegativeQuantity);
            }
        }
        let id = Uuid::new_v4();
        let mut tx = self.db_pool.begin().await?;
        // RLS scope (ADR-0008): bound before any read — an unbound connection is fenced to
        // zero rows and the operation-type lookup below would read every type as absent.
        company_scope::bind_company_on(&mut tx, p.company_id).await?;
        let op = self.pickings.fetch_operation_type(&mut tx, p.picking_type_id, p.company_id).await?
            .ok_or(InventoryError::NotFound(p.picking_type_id))?;
        // Same structural pre-checks the engine's move mint applies (R13 view locations hold
        // no stock; R26 internal locations must belong to this company) — run them BEFORE the
        // header insert so a bad location pair cannot leave an orphaned transfer.
        let locs = self.moves.fetch_move_locations(&mut tx, p.location_id, p.location_dest_id).await?;
        let (src, dst) = match locs {
            (Some(s), Some(d)) => (s, d),
            (None, _) => return Err(InventoryError::LocationNotFound(p.location_id)),
            (_, None) => return Err(InventoryError::LocationNotFound(p.location_dest_id)),
        };
        for loc in [&src, &dst] {
            if loc.usage == "view" {
                return Err(InventoryError::ViewLocationHoldsNoStock { location_id: loc.id });
            }
            if loc.usage == "internal" && loc.company_id != Some(p.company_id) {
                return Err(InventoryError::QuantCompanyMismatch {
                    location_id: loc.id, location_company: loc.company_id, move_company: p.company_id,
                });
            }
        }
        let ins = self.pickings.insert_transfer(&mut tx, &NewPickingRow {
            id,
            name: &p.name,
            origin: p.origin.as_deref(),
            picking_type_id: p.picking_type_id,
            location_id: p.location_id,
            location_dest_id: p.location_dest_id,
            partner_id: p.partner_id,
            company_id: p.company_id,
            move_type: &p.move_type,
        }).await;
        if let Err(e) = ins {
            return Err(if is_dup(&e) { InventoryError::DuplicateNumber(p.name) } else { e.into() });
        }
        tx.commit().await?;

        let mut move_ids = Vec::with_capacity(p.lines.len());
        for (idx, l) in p.lines.iter().enumerate() {
            let mid = self.create_move(NewStockMove {
                name: format!("{}/{}", p.name, idx + 1),
                company_id: p.company_id,
                item_id: l.item_id,
                demand_qty: l.demand_qty,
                price_unit: l.price_unit,
                procure_method: "make_to_stock".into(),
                picking_id: Some(id),
                origin: p.origin.clone(),
                location_id: p.location_id,
                location_dest_id: p.location_dest_id,
                partner_id: p.partner_id,
                warehouse_id: src.warehouse_id,
                orderpoint_id: None,
                move_orig_ids: vec![],
                move_dest_ids: vec![],
                is_inventory: false,
                scrapped: false,
                forced_value: None, // ordinary demand: the valuation core derives the carry
            }).await?;
            self.action_confirm(p.company_id, mid).await?;
            // The operation type's reservation posture: `at_confirm` (the generated default)
            // reserves what is available right away — the picking projects to
            // assigned / partially_available / confirmed accordingly. `manual` / `by_date`
            // leave reservation to the scheduler or the operator.
            if op.reservation_method == "at_confirm" {
                let _ = self.action_assign(p.company_id, mid).await?;
            }
            move_ids.push(mid);
        }
        let projected = self.fetch_picking(p.company_id, id).await?;
        Ok(PickingCreated { transfer_id: id, move_ids, projected_state: projected.0.state })
    }

    /// The projection PROBE: read the transfer header (with its projected state) and the
    /// member move states. The state is a stored compute the engine re-derived — callers read
    /// it, they never assert it. The read is fenced: the caller names the company and the RLS
    /// scope is bound on the reading transaction before any row is touched (an unbound
    /// connection is fenced to zero rows — the probe would see every transfer as absent).
    pub async fn fetch_picking(
        &self,
        company_id: Uuid,
        transfer_id: Uuid,
    ) -> Result<(crate::infrastructure::persistence::TransferHeaderRow,
                 Vec<crate::infrastructure::persistence::MoveStateRow>), InventoryError> {
        let mut tx = self.db_pool.begin().await?;
        company_scope::bind_company_on(&mut tx, company_id).await?;
        let header = self.pickings.fetch_transfer(&mut tx, transfer_id).await?
            .ok_or(InventoryError::NotFound(transfer_id))?;
        let moves = self.pickings.fetch_moves_of_transfer(&mut tx, transfer_id).await?;
        tx.commit().await?;
        Ok((header, moves))
    }

    /// `_assign_picking`: attach a rule-launched move to its grouping transfer — the hop that
    /// carries a demand launched through the procurement engine (`run_procurement` / `_run_pull`
    /// mint moves with no transfer) onto the picking surface an operator validates. The move's
    /// RULE names the operation type; the group key is (operation type, source, destination,
    /// partner, origin) with `origin` standing in for the procurement group — every move
    /// launched for one source document joins ONE transfer, and a transfer stays open for
    /// later lines of the same document. An open transfer that matches is joined; otherwise a
    /// header is minted (name from the operation type's reference prefix, unique per company;
    /// shipping policy inherited from the operation type). The projection then re-derives
    /// from the member move states, exactly as it does on every move change.
    ///
    /// Idempotent: a move that already belongs to a transfer returns it (`minted: false`).
    /// Fail-closed on a move with no procurement rule (voucher-door moves keep their voucher
    /// identity — their door mints the moves and owns the GL, and no picking is derived for
    /// them here) and on terminal states (`done` / `cancel` never re-group).
    pub async fn assign_picking(
        &self,
        company_id: Uuid,
        move_id: Uuid,
    ) -> Result<PickingAssignment, InventoryError> {
        let mut tx = self.db_pool.begin().await?;
        // RLS scope (ADR-0008), and the fence's fail-closed posture: the company is a
        // caller-supplied fact, bound before the move read — an unfenced read would see every
        // move as absent, and a wrong-company move reads as NotFound below.
        company_scope::bind_company_on(&mut tx, company_id).await?;
        let mv = self.moves.fetch_move(&mut tx, move_id).await?
            .ok_or(InventoryError::NotFound(move_id))?;
        if mv.company_id != company_id {
            return Err(InventoryError::NotFound(move_id));
        }
        match mv.state.as_str() {
            "draft" | "waiting" | "confirmed" | "partially_available" | "assigned" => {}
            other => return Err(InventoryError::WrongMoveState {
                move_id, action: "assign_picking", current: other.into(),
            }),
        }
        if let Some(existing) = mv.picking_id {
            tx.commit().await?;
            return Ok(PickingAssignment { transfer_id: existing, minted: false });
        }
        // The rule names the operation type; without one there is nothing to derive a
        // grouping transfer from (refuse rather than guess a type).
        let rule_id = mv.rule_id.ok_or(InventoryError::MoveHasNoRule { move_id })?;
        let picking_type_id = ProcurementRepository::rule_picking_type(&mut *tx, rule_id).await?
            .ok_or(InventoryError::NotFound(rule_id))?;
        let op = self.pickings.fetch_operation_type(&mut tx, picking_type_id, company_id).await?
            .ok_or(InventoryError::NotFound(picking_type_id))?;

        let (transfer_id, minted) = match self.pickings.find_open_group_picking(
            &mut tx, company_id, picking_type_id,
            mv.location_id, mv.location_dest_id, mv.partner_id, mv.origin.as_deref(),
        ).await? {
            Some(open) => (open, false),
            None => {
                let id = Uuid::new_v4();
                // The operation type's reference prefix (e.g. `IN/`, `WH/OUT/`) plus a short
                // unique suffix — (name, company) is unique; a collision is the typed
                // duplicate error the caller can retry.
                let prefix = op.sequence_code.trim_end_matches('/');
                let prefix = if prefix.is_empty() { "PICK" } else { prefix };
                let name = format!("{}/{}", prefix, &Uuid::new_v4().simple().to_string()[..8].to_uppercase());
                let ins = self.pickings.insert_transfer(&mut tx, &NewPickingRow {
                    id,
                    name: &name,
                    origin: mv.origin.as_deref(),
                    picking_type_id,
                    location_id: mv.location_id,
                    location_dest_id: mv.location_dest_id,
                    partner_id: mv.partner_id,
                    company_id,
                    move_type: &op.move_type,
                }).await;
                if let Err(e) = ins {
                    return Err(if is_dup(&e) { InventoryError::DuplicateNumber(name) } else { e.into() });
                }
                (id, true)
            }
        };
        self.moves.set_picking(&mut tx, move_id, transfer_id).await?;
        self.move_lines.retarget_picking(&mut tx, move_id, transfer_id).await?;
        self.moves.reproject_picking(&mut tx, transfer_id).await?;
        tx.commit().await?;
        Ok(PickingAssignment { transfer_id, minted })
    }

    /// `button_validate` (spec §2): `_action_done` over the transfer's moves — nothing else.
    /// Done/cancelled member moves are skipped (idempotent re-validate). For each live move:
    /// the quant surface at its source grain is healed if the stock predates the converged
    /// model (Bins without quants); unreserved internal moves get an `assign` pass first (it
    /// mints the reservation mirror lines), and a move still without lines — nothing
    /// reservable, or an inbound move whose source is virtual — gets its demand line minted
    /// as the physical detail row so `done` has lines (R24); `_action_done` then flips the
    /// quants, mints the V7-ordered SLE pair, and posts the GL leg the move's shape calls
    /// for. The partial-validate backorder policy comes from the operation type (`ask` maps
    /// to creating the backorder — the non-interactive default of Odoo's wizard; `delayed`
    /// creates it too but leaves its reservation to the scheduler's assign sweep).
    ///
    /// Posting a GL leg needs the composing service's `GlPostSink`; a leg whose accounts the
    /// directive lacks is simply not posted (the physical movement is unaffected).
    pub async fn validate_picking(
        &self,
        company_id: Uuid,
        transfer_id: Uuid,
        gl: &MoveGlDirective,
        sink: &dyn GlPostSink,
    ) -> Result<PickingValidated, InventoryError> {
        // The company is a caller-supplied fact (the fence's fail-closed posture means this
        // method cannot DISCOVER it from an unfenced read): bind it on the transaction before
        // the header/move reads, which both fences them and proves the transfer belongs to
        // the caller.
        let move_ids = {
            let mut tx = self.db_pool.begin().await?;
            company_scope::bind_company_on(&mut tx, company_id).await?;
            let header = self.pickings.fetch_transfer(&mut tx, transfer_id).await?
                .ok_or(InventoryError::NotFound(transfer_id))?;
            if header.company_id != company_id {
                return Err(InventoryError::NotFound(transfer_id));
            }
            let ids = self.pickings.move_ids_of_transfer(&mut tx, transfer_id).await?;
            tx.commit().await?;
            ids
        };

        let op_backorder = self.operation_backorder_policy(company_id, transfer_id).await?;
        let mut outcomes = Vec::new();
        for mid in move_ids {
            let mv = {
                let mut tx = self.db_pool.begin().await?;
                company_scope::bind_company_on(&mut tx, company_id).await?;
                let mv = self.moves.fetch_move(&mut tx, mid).await?.ok_or(InventoryError::NotFound(mid))?;
                tx.commit().await?;
                mv
            };
            if matches!(mv.state.as_str(), "done" | "cancel") { continue; }
            if mv.state == "draft" {
                return Err(InventoryError::WrongMoveState { move_id: mid, action: "validate", current: mv.state.clone() });
            }
            self.prepare_move_for_validate(&mv).await?;
            outcomes.push(self.action_done(company_id, mid, op_backorder, gl, sink).await?);
        }
        let header = self.fetch_picking(company_id, transfer_id).await?;
        Ok(PickingValidated {
            transfer_id,
            projected_state: header.0.state,
            validated_moves: outcomes,
        })
    }

    /// The partial-validate backorder policy of the transfer's operation type (`ask` maps to
    /// `Always` — the non-interactive default; a wizard is not available on this surface;
    /// `delayed` mints the backorder but defers its reservation to the scheduler's assign
    /// sweep).
    async fn operation_backorder_policy(
        &self,
        company_id: Uuid,
        transfer_id: Uuid,
    ) -> Result<BackorderPolicy, InventoryError> {
        let picking_type_id = {
            let mut tx = self.db_pool.begin().await?;
            company_scope::bind_company_on(&mut tx, company_id).await?;
            let header = self.pickings.fetch_transfer(&mut tx, transfer_id).await?
                .ok_or(InventoryError::NotFound(transfer_id))?;
            tx.commit().await?;
            header.picking_type_id
        };
        let facts = {
            let mut tx = self.db_pool.begin().await?;
            company_scope::bind_company_on(&mut tx, company_id).await?;
            let f = self.pickings.fetch_operation_type(&mut tx, picking_type_id, company_id).await
                .map_err(InventoryError::from)?
                .ok_or(InventoryError::NotFound(picking_type_id))?;
            tx.commit().await?;
            f
        };
        Ok(match facts.create_backorder.as_str() {
            "never" => BackorderPolicy::Never,
            "delayed" => BackorderPolicy::Delayed,
            _ => BackorderPolicy::Always,
        })
    }

    /// Get a live move ready for `_action_done`: heal the source quant surface when the
    /// stock predates the converged model, then make sure the move HAS lines (R24) — an
    /// `assign` pass for internal sources (the reservation mirror), a minted demand line for
    /// everything else. `pub(super)`: the scrap door drives the same preparation for its
    /// move (a scrap's source is always an internal stock location).
    pub(super) async fn prepare_move_for_validate(
        &self,
        mv: &crate::infrastructure::persistence::MoveRow,
    ) -> Result<(), InventoryError> {
        let mut tx = self.db_pool.begin().await?;
        company_scope::bind_company_on(&mut tx, mv.company_id).await?;
        let locs = self.moves.fetch_move_locations(&mut tx, mv.location_id, mv.location_dest_id).await?;
        let src = locs.0.ok_or(InventoryError::LocationNotFound(mv.location_id))?;
        // The quant-surface heal: legacy voucher paths wrote Bins without quants; the first
        // converged touch of the grain seeds the quant from the Bin (one time, idempotent).
        if src.usage == "internal" {
            self.pickings.ensure_quant_surface(&mut tx, mv.company_id, mv.item_id, src.id, src.warehouse_id).await?;
        }
        tx.commit().await?;

        let lines = {
            let mut tx = self.db_pool.begin().await?;
            company_scope::bind_company_on(&mut tx, mv.company_id).await?;
            let l = self.move_lines.fetch_lines_for_move(&mut tx, mv.id).await?;
            tx.commit().await?;
            l
        };
        if !lines.is_empty() { return Ok(()); }
        if src.usage == "internal" {
            // Reserve what is available — this mints the reservation mirror lines. Whatever
            // the reservation could NOT cover is the BACKORDER's demand, never a phantom
            // line here: the done verb's R22 draw guard refuses a draw the source quant
            // cannot cover (you cannot ship stock that is not there), and its backorder
            // split mints the unreserved remainder as its own draft move on the same
            // picking. A move with nothing reservable stays lineless and the done verb
            // refuses it loudly (R24) — the transfer stays open below `done`.
            self.action_assign(mv.company_id, mv.id).await?;
        } else {
            self.mint_demand_line(mv, mv.demand_qty).await?;
        }
        Ok(())
    }

    /// Mint the physical detail line for a move's demand (the R24 floor: a done move carries
    /// at least one line). Used for untracked inbound moves (virtual source — nothing to
    /// reserve) and for the unreservable remainder of an internal move; `_action_done`
    /// enforces the draw guards (R22/R23) on whatever the lines claim.
    async fn mint_demand_line(
        &self,
        mv: &crate::infrastructure::persistence::MoveRow,
        qty: Decimal,
    ) -> Result<(), InventoryError> {
        if qty <= Decimal::ZERO { return Ok(()); }
        let mut tx = self.db_pool.begin().await?;
        company_scope::bind_company_on(&mut tx, mv.company_id).await?;
        self.move_lines.insert_line(&mut tx, &NewMoveLineRow {
            id: Uuid::new_v4(),
            quantity: qty,
            lot_id: None,
            package_id: None,
            result_package_id: None,
            owner_id: None,
            move_id: mv.id,
            picking_id: mv.picking_id,
            location_id: mv.location_id,
            location_dest_id: mv.location_dest_id,
            item_id: mv.item_id,
            company_id: mv.company_id,
            state: mv.state.as_str(),
        }).await?;
        tx.commit().await?;
        Ok(())
    }

    // ---- submit: Stock Entry (transfer — value-neutral, no GL), engine-routed --------------

    /// The warehouse-to-warehouse stock entry, converged: the voucher rows keep their exact
    /// identity (header, item lines, the `StockMoved` event, value-neutral no-GL posture),
    /// while each line's physical/valuation legs are minted by the move pipeline — one move
    /// per line, confirmed, reserved against the source quants, validated. The engine's
    /// valuation core produces the W1-proven arithmetic: the OUT leg valued at the source
    /// Bin's pre-move average BEFORE the destination reblende (V7 ordering), the residual
    /// flush when a Bin drains to zero, value conserved across the pair.
    pub async fn submit_transfer(&self, t: NewTransfer) -> Result<Uuid, InventoryError> {
        if t.lines.is_empty() { return Err(InventoryError::EmptyDocument); }
        if t.from_warehouse_id == t.to_warehouse_id { return Err(InventoryError::SameWarehouse); }
        for l in &t.lines { if l.quantity < Decimal::ZERO { return Err(InventoryError::NegativeQuantity); } }

        let id = Uuid::new_v4();
        let mut tx = self.db_pool.begin().await?;
        // RLS scope (ADR-0008): MUST be bound before any bin read — an unbound connection is
        // fenced to zero rows, so the availability pre-check below would read every bin as
        // empty and refuse every transfer.
        company_scope::bind_company_on(&mut tx, t.company_id).await?;
        // The warehouses' stock locations (bootstrapped per warehouse on first use — the
        // picking/move grain is the location, the voucher grain was the warehouse).
        let from_loc = self.pickings.ensure_internal_location(&mut tx, t.from_warehouse_id, t.company_id).await?;
        let to_loc = self.pickings.ensure_internal_location(&mut tx, t.to_warehouse_id, t.company_id).await?;
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
        for l in &t.lines {
            self.entry_items.insert_item(&mut tx, &NewStockEntryItemRow {
                id: Uuid::new_v4(),
                entry_id: id,
                company_id: t.company_id,
                item_id: l.item_id,
                quantity: l.quantity,
            }).await?;
            // The all-or-nothing pre-check of the voucher path, kept verbatim: the source Bin
            // must cover the line before anything mints (the engine's draw guard re-checks at
            // the quant grain).
            let from = self.bins.lock_or_init(&mut tx, t.company_id, l.item_id, t.from_warehouse_id).await?;
            if from.actual_qty < l.quantity {
                return Err(InventoryError::InsufficientStock {
                    item_id: l.item_id, warehouse_id: t.from_warehouse_id,
                    available: from.actual_qty, requested: l.quantity,
                });
            }
        }
        tx.commit().await?;

        for l in &t.lines {
            let mid = self.create_move(NewStockMove {
                name: format!("{}/{}", t.entry_number, l.item_id.simple()),
                company_id: t.company_id,
                item_id: l.item_id,
                demand_qty: l.quantity,
                price_unit: Decimal::ZERO, // internal move: value carried, not priced
                procure_method: "make_to_stock".into(),
                picking_id: None,
                origin: Some(t.entry_number.clone()),
                location_id: from_loc,
                location_dest_id: to_loc,
                partner_id: None,
                warehouse_id: Some(t.from_warehouse_id),
                orderpoint_id: None,
                move_orig_ids: vec![],
                move_dest_ids: vec![],
                is_inventory: false,
                scrapped: false,
                forced_value: None, // ordinary demand: the valuation core derives the carry
            }).await?;
            self.action_confirm(t.company_id, mid).await?;
            // Reserve from the (healed) source quant — mints the execution line — then
            // validate. The voucher door stays all-or-nothing per line: no backorder.
            self.prepare_move_for_validate(&(self.fetch_move_row(t.company_id, mid).await?)).await?;
            self.action_done(t.company_id, mid, BackorderPolicy::Never, &MoveGlDirective::default(), &ValueNeutralDoorSink).await?;
        }
        self.sink.publish(InventoryEvent::StockMoved(StockMoved {
            entry_id: id, company_id: t.company_id,
            from_warehouse_id: Some(t.from_warehouse_id), to_warehouse_id: Some(t.to_warehouse_id),
        }));
        Ok(id)
    }

    /// Fetch one move row (company-fenced) — a thin read for the voucher rewire's
    /// prepare step. Binds the RLS scope on its own transaction before the read: an
    /// unbound read is fenced to zero rows and would report the just-minted move absent.
    async fn fetch_move_row(
        &self,
        company_id: Uuid,
        move_id: Uuid,
    ) -> Result<crate::infrastructure::persistence::MoveRow, InventoryError> {
        let mut tx = self.db_pool.begin().await?;
        company_scope::bind_company_on(&mut tx, company_id).await?;
        let mv = self.moves.fetch_move(&mut tx, move_id).await?
            .ok_or(InventoryError::NotFound(move_id))?;
        tx.commit().await?;
        Ok(mv)
    }
}
