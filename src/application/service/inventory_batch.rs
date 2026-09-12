//! The picking-batch surface (hand-authored, user-owned) — the stock.picking.batch
//! satellite (spec misc-features.md SB).
//!
//! A batch GROUPS pickings (a wave-picking work list). Its `state` is a PROJECTION
//! derived from its member pickings, exactly the way a transfer's state is derived from
//! its member moves (the T1 discipline): there is NO per-batch state mutation code in
//! this file — no hand-set batch state machine, no batch-state column writes. `create_batch`
//! mints a draft header; the membership verbs move the `transfers.batch_id` pointer and
//! then RE-DERIVE the projection; the engine's picking projection cascades the same
//! recompute on every member-state change (the batch never lags its members). The probes
//! READ the projected state.
//!
//! Membership is one-batch-per-picking (`transfers.batch_id`), the effective Odoo shape:
//! a picking leaves its batch by clearing the pointer — which re-derives both sides.
//!
//! Deliberate deviations from Odoo's `_sanitize` behavior, both documented here and in
//! the schema description: a picking may only JOIN while non-terminal (`done`/`cancel`
//! refuse with the typed error rather than being silently dragged in), and a batch in a
//! terminal state refuses membership CHANGES (`BatchTerminal`) rather than cancelling
//! its members out from under the operator. Removal is always allowed — detaching a done
//! or cancelled picking from its batch is how a list gets cleaned up.
//!
//! Per the module's 4-layer rule this file holds no SQL — statements live on
//! [`super::super::super::infrastructure::persistence::PickingBatchProjectionRepository`].
//!
//! Tenancy (ADR-0029): the module is tenant-agnostic. Every write transaction this file
//! opens re-binds the caller's ambient org scope (`relay_ambient_scope`); the composing
//! decorator owns isolation.

use uuid::Uuid;

use crate::infrastructure::persistence::{BatchHeaderRow, BatchMemberRow, NewBatchRow};

use super::inventory_write_service::{is_dup, relay_ambient_scope, InventoryError, InventoryWriteService};

impl InventoryWriteService {
    // ---- picking-batch: mint / membership / probe --------------------------------

    /// Mint a batch: insert the header (a draft grouping point). `state` starts at
    /// `draft` and is from here on ONLY ever written by the projection recompute — this
    /// method never touches it. Guards: R3-shaped org-scoped name unique (the typed
    /// duplicate error on collision).
    pub async fn create_batch(
        &self,
        name: String,
        is_wave: bool,
        user_id: Option<Uuid>,
    ) -> Result<BatchHeaderRow, InventoryError> {
        let id = Uuid::new_v4();
        let mut tx = self.db_pool.begin().await?;
        // Re-bind the caller's ambient org scope before the write (ADR-0029) — the
        // composing decorator's fence arbiter turns a collision into the typed duplicate.
        relay_ambient_scope(&mut tx).await?;
        let ins = self.batches.insert_batch(&mut tx, &NewBatchRow {
            id,
            name: &name,
            is_wave,
            user_id,
        }).await;
        if let Err(e) = ins {
            return Err(if is_dup(&e) { InventoryError::DuplicateNumber(name) } else { e.into() });
        }
        tx.commit().await?;
        let (header, _) = self.fetch_batch(id).await?;
        Ok(header)
    }

    /// The projection PROBE: read the batch header (with its projected state) and its
    /// member pickings' states, riding the caller's ambient org scope (ADR-0029) — an
    /// out-of-scope batch reads as absent under the composition's fence.
    pub async fn fetch_batch(
        &self,
        batch_id: Uuid,
    ) -> Result<(BatchHeaderRow, Vec<BatchMemberRow>), InventoryError> {
        let mut tx = self.db_pool.begin().await?;
        relay_ambient_scope(&mut tx).await?;
        let header = self.batches.fetch_batch(&mut tx, batch_id).await?
            .ok_or(InventoryError::NotFound(batch_id))?;
        let members = self.batches.fetch_members(&mut tx, batch_id).await?;
        tx.commit().await?;
        Ok((header, members))
    }

    /// Attach a picking to its batch. Guards (all typed, all service-side with the
    /// DB trigger as backstop — `enforcement: both`, ADR-0015):
    ///   - the batch exists (absent under the ambient scope reads as `NotFound`),
    ///   - the batch is non-terminal (`done`/`cancel` refuse — `BatchTerminal`),
    ///   - the picking exists and is non-terminal (`PickingTerminalForBatch`
    ///     — a done or cancelled picking cannot join a work list),
    ///   - the picking belongs to no other batch (`PickingAlreadyBatched` — membership is
    ///     one batch per picking).
    /// The write moves only the membership pointer and stamps the `had_members`
    /// discriminator; the projection recompute that follows derives the state. Returns
    /// the re-derived header (a READ of the projection).
    pub async fn add_picking_to_batch(
        &self,
        batch_id: Uuid,
        picking_id: Uuid,
    ) -> Result<BatchHeaderRow, InventoryError> {
        let mut tx = self.db_pool.begin().await?;
        relay_ambient_scope(&mut tx).await?;
        let batch = self.batches.fetch_batch(&mut tx, batch_id).await?
            .ok_or(InventoryError::NotFound(batch_id))?;
        match batch.state.as_str() {
            "draft" | "waiting" | "ready" => {}
            other => return Err(InventoryError::BatchTerminal { batch_id, state: other.into() }),
        }
        let picking = self.pickings.fetch_transfer(&mut tx, picking_id).await?
            .ok_or(InventoryError::NotFound(picking_id))?;
        match picking.state.as_str() {
            "draft" | "waiting" | "confirmed" | "partially_available" | "assigned" => {}
            other => return Err(InventoryError::PickingTerminalForBatch {
                picking_id, state: other.into(),
            }),
        }
        if let Some(other) = self.batches.picking_membership(&mut tx, picking_id).await?
            .flatten()
        {
            return Err(InventoryError::PickingAlreadyBatched { picking_id, batch_id: other });
        }
        self.batches.add_member(&mut tx, batch_id, picking_id).await?;
        self.batches.reproject_batch(&mut tx, batch_id).await?;
        tx.commit().await?;
        let (header, _) = self.fetch_batch(batch_id).await?;
        Ok(header)
    }

    /// Detach a picking from its batch. Removal is always allowed — including a done or
    /// cancelled picking (that is how a finished list gets cleaned up). Detaching the
    /// last member leaves an EMPTY batch, which the projection then auto-cancels (the
    /// SB-1 discriminator: a batch that has grouped work must not linger as a live work
    /// item once emptied). Returns the re-derived header after the removal.
    pub async fn remove_picking_from_batch(
        &self,
        batch_id: Uuid,
        picking_id: Uuid,
    ) -> Result<BatchHeaderRow, InventoryError> {
        let mut tx = self.db_pool.begin().await?;
        relay_ambient_scope(&mut tx).await?;
        self.batches.fetch_batch(&mut tx, batch_id).await?
            .ok_or(InventoryError::NotFound(batch_id))?;
        let removed = self.batches.remove_member(&mut tx, batch_id, picking_id).await?;
        if !removed {
            return Err(InventoryError::NotABatchMember { batch_id, picking_id });
        }
        self.batches.reproject_batch(&mut tx, batch_id).await?;
        tx.commit().await?;
        let (header, _) = self.fetch_batch(batch_id).await?;
        Ok(header)
    }
}
