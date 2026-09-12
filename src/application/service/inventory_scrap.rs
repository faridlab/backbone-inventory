//! The scrap door (hand-authored, user-owned) — the stock.scrap satellite (spec
//! stock-business-logic.md §8).
//!
//! A scrap is an OPERATOR DOCUMENT with a genuine hand-set state (`draft` → `done` —
//! unlike the picking-batch projection), but the physical/valuation work rides the ONE
//! stock-move engine: `process_scrap` mints a single move (`scrapped = true`,
//! `is_inventory = true`) from the stock location to the scrap location, drives it
//! confirm → assign → done through the engine verbs, and stamps the header `done` with
//! the move id. No second estate: the quants, the SLE pair, the Bin reblende, and the GL
//! leg are all the engine's, exactly as for every other door.
//!
//! The scrap location defaults to the inventory-loss location (the same sink the
//! reconciliation door moves through), overridable per scrap. The GL shape is the
//! adjustment shape (the engine's `is_inventory` leg): the directive's
//! `adjustment_account_id` carries the scrap expense account, `inventory_account_id` the
//! stock account. The HTTP surface gets the DEFERRED form (the module's standing posture:
//! the composing service's sink is not available on a bare route); a service-driven
//! process posts through its sink, and `repost_move_gl` re-drives a stuck leg.
//!
//! Tenancy (ADR-0029): the module is tenant-agnostic. Every write transaction this file
//! opens re-binds the caller's ambient org scope (`relay_ambient_scope`); the composing
//! decorator owns isolation.
//!
//! Per the module's 4-layer rule this file holds no SQL — statements live on
//! [`super::super::super::infrastructure::persistence::ScrapDoorRepository`].

use rust_decimal::Decimal;
use uuid::Uuid;

use crate::infrastructure::persistence::{NewScrapRow, ScrapRow};

use super::inventory_gl::GlPostSink;
use super::inventory_move_engine::{BackorderPolicy, MoveGlDirective, NewStockMove};
use super::inventory_write_service::{
    is_dup, relay_ambient_scope, InventoryError, InventoryWriteService,
};

/// The scrap-mint input. `scrap_location_id` defaults to the inventory-loss
/// location when `None`. Guards on mint: strictly positive quantity (R-shaped), a
/// stockable internal source location (R13).
#[derive(Debug, Clone)]
pub struct NewScrap {
    pub item_id: Uuid,
    pub scrap_qty: Decimal,
    /// Where the scrapped stock currently sits (internal usage).
    pub location_id: Uuid,
    /// Where the scrap lands (the loss sink); `None` resolves the default loss sink.
    pub scrap_location_id: Option<Uuid>,
    pub lot_id: Option<Uuid>,
    pub package_id: Option<Uuid>,
    pub owner_id: Option<Uuid>,
    pub picking_id: Option<Uuid>,
    pub origin: Option<String>,
    pub scrap_reason_tag_ids: Vec<Uuid>,
}

/// The scrap process outcome: the closed header and the engine move it landed through.
#[derive(Debug, Clone)]
pub struct ScrapProcessed {
    pub scrap_id: Uuid,
    pub move_id: Uuid,
}

impl InventoryWriteService {
    // ---- scrap: mint / process / probe ---------------------------------------------

    /// Mint a scrap order in `draft`. Nothing moves yet — the header records WHAT will be
    /// scrapped, where from, and where to. Guards: strictly positive quantity, a live
    /// internal source location, a resolvable scrap location. The name is
    /// minted `SCRAP/<short-uuid>` (org-scoped unique — the typed duplicate error on the
    /// rare collision).
    pub async fn create_scrap(&self, s: NewScrap) -> Result<ScrapRow, InventoryError> {
        if s.scrap_qty <= Decimal::ZERO {
            return Err(InventoryError::NegativeQuantity);
        }
        let id = Uuid::new_v4();
        let name = format!("SCRAP/{}", &Uuid::new_v4().simple().to_string()[..8].to_uppercase());
        let mut tx = self.db_pool.begin().await?;
        // Re-bind the caller's ambient org scope before any read (ADR-0029) — the scope is
        // task-local and a fresh pool transaction carries none of it; undecorated (module
        // tests, jobs) the transaction stays plain.
        relay_ambient_scope(&mut tx).await?;
        // R13-shaped pre-check: the source must be a stockable INTERNAL location (a view
        // location holds no stock).
        let locs = self.moves.fetch_move_locations(&mut tx, s.location_id, s.location_id).await?;
        let src = locs.0.ok_or(InventoryError::LocationNotFound(s.location_id))?;
        if src.usage != "internal" {
            return Err(InventoryError::ViewLocationHoldsNoStock { location_id: s.location_id });
        }
        // The scrap sink: explicit when given, else the inventory-loss location
        // (bootstrapped on first use — the same sink the reconciliation door uses).
        let scrap_location_id = match s.scrap_location_id {
            Some(explicit) => {
                let dst = self.moves.fetch_move_locations(&mut tx, explicit, explicit).await?;
                match dst.1.or(dst.0) {
                    Some(facts) if facts.usage == "inventory" => explicit,
                    _ => return Err(InventoryError::ScrapLocationUnavailable),
                }
            }
            None => self.pickings.ensure_inventory_loss_location(&mut tx).await
                .map_err(|_| InventoryError::ScrapLocationUnavailable)?,
        };
        let ins = self.scraps.insert_scrap(&mut tx, &NewScrapRow {
            id,
            name: &name,
            origin: s.origin.as_deref(),
            item_id: s.item_id,
            scrap_qty: s.scrap_qty,
            location_id: s.location_id,
            scrap_location_id,
            lot_id: s.lot_id,
            package_id: s.package_id,
            owner_id: s.owner_id,
            picking_id: s.picking_id,
            scrap_reason_tag_ids: s.scrap_reason_tag_ids,
        }).await;
        if let Err(e) = ins {
            return Err(if is_dup(&e) { InventoryError::DuplicateNumber(name) } else { e.into() });
        }
        tx.commit().await?;
        self.fetch_scrap(id).await?
            .ok_or(InventoryError::NotFound(id))
    }

    /// The probe: read one scrap header, riding the caller's ambient org scope (ADR-0029).
    pub async fn fetch_scrap(&self, scrap_id: Uuid) -> Result<Option<ScrapRow>, InventoryError> {
        let mut tx = self.db_pool.begin().await?;
        relay_ambient_scope(&mut tx).await?;
        let row = self.scraps.fetch_scrap(&mut tx, scrap_id).await?;
        tx.commit().await?;
        Ok(row)
    }

    /// Process a DRAFT scrap: mint the move, drive it to done through the engine, stamp
    /// the header. The full-GL form — the caller's directive carries the accounts and the
    /// sink posts the envelope. The move is `is_inventory` (the engine's adjustment leg
    /// shape: the directive's `adjustment_account_id` is the scrap expense account) and
    /// `scrapped` (the engine's marker for scrap-marked moves). A scrap lands WHOLE: the
    /// backorder policy is `Never` and a move that cannot fully reserve refuses loudly
    /// (you cannot scrap stock that is not there — R22).
    ///
    /// Crash-window safe: the done-stamp's `state = 'draft'` guard means exactly one
    /// process call closes the header; a crash after the move landed but before the stamp
    /// leaves the scrap re-processable, and the resume check below re-drives the SAME
    /// move instead of minting a second one.
    pub async fn process_scrap(
        &self,
        scrap_id: Uuid,
        gl: &MoveGlDirective,
        sink: &dyn GlPostSink,
    ) -> Result<ScrapProcessed, InventoryError> {
        let scrap = self.fetch_scrap(scrap_id).await?
            .ok_or(InventoryError::NotFound(scrap_id))?;
        match scrap.state.as_str() {
            "draft" => {}
            other => return Err(InventoryError::ScrapNotDraft { scrap_id, state: other.into() }),
        }
        let move_id = match scrap.move_id {
            // Resume: a prior process minted the move but did not close the header (crash
            // between done and stamp). Re-driving the same move keeps the estate single.
            Some(prior) => prior,
            None => self.create_move(NewStockMove {
                name: scrap.name.clone(),
                item_id: scrap.item_id,
                demand_qty: scrap.scrap_qty,
                price_unit: Decimal::ZERO, // the loss is valued at the current average
                procure_method: "make_to_stock".into(),
                picking_id: None,
                origin: scrap.origin.clone(),
                location_id: scrap.location_id,
                location_dest_id: scrap.scrap_location_id,
                partner_id: None,
                warehouse_id: None,
                orderpoint_id: None,
                move_orig_ids: vec![],
                move_dest_ids: vec![],
                is_inventory: true,
                scrapped: true,
                forced_value: None,
            }).await?,
        };
        // Drive the pipeline, re-reading the state between verbs (the engine owns the
        // state; this method never asserts it).
        let mv = {
            let mut tx = self.db_pool.begin().await?;
            relay_ambient_scope(&mut tx).await?;
            let mv = self.moves.fetch_move(&mut tx, move_id).await?
                .ok_or(InventoryError::NotFound(move_id))?;
            tx.commit().await?;
            mv
        };
        let mut state = mv.state.clone();
        if state == "draft" {
            state = self.action_confirm(move_id).await?;
        }
        if matches!(state.as_str(), "confirmed" | "partially_available") {
            self.action_assign(move_id).await?;
            state = self.move_state_of(move_id).await?;
        }
        if state == "assigned" || state == "waiting" {
            // The waiting case is the resume window's residue (a move confirmed against
            // nothing reservable); the assign pass above is what freed or refused it.
            let fresh = self.move_state_of(move_id).await?;
            if fresh == "waiting" {
                return Err(InventoryError::WrongMoveState {
                    move_id, action: "process_scrap", current: fresh,
                });
            }
            state = fresh;
        }
        if state != "assigned" {
            return Err(InventoryError::WrongMoveState {
                move_id, action: "process_scrap", current: state,
            });
        }
        let mv = {
            let mut tx = self.db_pool.begin().await?;
            relay_ambient_scope(&mut tx).await?;
            let mv = self.moves.fetch_move(&mut tx, move_id).await?
                .ok_or(InventoryError::NotFound(move_id))?;
            tx.commit().await?;
            mv
        };
        self.prepare_move_for_validate(&mv).await?;
        self.action_done(move_id, BackorderPolicy::Never, gl, sink).await?;
        {
            let mut tx = self.db_pool.begin().await?;
            relay_ambient_scope(&mut tx).await?;
            let closed = self.scraps.mark_scrap_done(&mut tx, scrap_id, move_id).await?;
            tx.commit().await?;
            if !closed {
                return Err(InventoryError::ScrapNotDraft { scrap_id, state: "done".into() });
            }
        }
        Ok(ScrapProcessed { scrap_id, move_id })
    }

    /// The HTTP-shaped process: the SAME physical movement (the move lands done, the
    /// quants flip, the SLEs mint), but the GL leg carries a no-accounts directive — the
    /// engine builds no envelope and the move's posting stays `not_applicable` until a
    /// service-driven [`Self::repost_move_gl`] drives it with real accounts (the module's
    /// standing deferred posture for GL-posting verbs on a bare route).
    pub async fn process_scrap_deferred(
        &self,
        scrap_id: Uuid,
    ) -> Result<ScrapProcessed, InventoryError> {
        let no_accounts = MoveGlDirective {
            cogs_account_id: None,
            inventory_account_id: None,
            grir_account_id: None,
            adjustment_account_id: None,
            currency: "IDR".into(),
        };
        struct NoGlSink;
        #[async_trait::async_trait]
        impl GlPostSink for NoGlSink {
            async fn post(
                &self,
                _e: &super::inventory_gl::AccountingPostEnvelope,
            ) -> Result<super::inventory_gl::GlPostAck, super::inventory_gl::GlPostRejected> {
                Err(super::inventory_gl::GlPostRejected {
                    code: "no_gl_on_scrap_deferred".into(),
                    message: "the deferred scrap process posts no GL; drive repost_move_gl".into(),
                })
            }
        }
        self.process_scrap(scrap_id, &no_accounts, &NoGlSink).await
    }
}
