//! Picking-batch projection SQL (hand-authored, user-owned) — the stock.picking.batch
//! satellite (SB-1).
//!
//! Distinct from the generated `picking_batch_repository.rs` (the GenericCrud newtype the
//! read routes use): this file owns the BATCH SURFACE'S OWN SQL that is not generic CRUD —
//!   - the batch header mint (`state` starts at `draft` and is from then on ONLY ever
//!     written by [`Self::reproject_batch`] — the SB-1 stored compute, spec
//!     misc-features.md SB / ADR-0016 `projection`),
//!   - the membership writes (`transfers.batch_id` set/clear + the `had_members`
//!     discriminator stamp — the only two columns the membership path owns),
//!   - the SB-1 projection recompute: the batch's state derives from its member
//!     pickings' states exactly the way the transfer's derives from its moves
//!     (LEAST-advanced live member; cancelled members drop out; no live member and
//!     nothing left to pick → `cancel`). The picking-projection cascade in
//!     `StockMoveRepository::reproject_picking` calls back into
//!     [`Self::reproject_batch_of_picking`] after every member-state change, so the
//!     recompute has exactly ONE implementation and ONE writer family — never a second
//!     writer beside the move engine.
//!   - the projection PROBE reads (header + member picking states): the projected
//!     `picking_batches.state` is READ here, never asserted or hand-set.
//!
//! State values are snake_case; enum parameters are bound as text with explicit casts and
//! read back with `::text` (the module-wide enum lesson). Statements live here and take
//! the caller's connection; services orchestrate.

use sqlx::{PgConnection, Row};
use uuid::Uuid;

/// Header fields of one picking batch (the projection probe surface).
#[derive(Debug, Clone)]
pub struct BatchHeaderRow {
    pub id: Uuid,
    /// The batch's owning company (strict fence) — the caller binds the RLS scope from it.
    pub company_id: Uuid,
    pub name: String,
    pub state: String,
    pub is_wave: bool,
    pub user_id: Option<Uuid>,
    pub scheduled_date: chrono::DateTime<chrono::Utc>,
    /// True once the batch has ever grouped a picking — the SB-1 discriminator.
    pub had_members: bool,
}

/// State of one member picking, as the projection probe reads it.
#[derive(Debug, Clone)]
pub struct BatchMemberRow {
    pub id: Uuid,
    pub name: String,
    /// draft / waiting / confirmed / assigned / done / cancel (as text).
    pub state: String,
}

/// A batch header insert. `state` starts at `draft`; membership writes and move
/// transitions reproject — the stored value is always the projection of the member
/// picking states.
pub struct NewBatchRow<'a> {
    pub id: Uuid,
    pub name: &'a str,
    pub company_id: Uuid,
    pub is_wave: bool,
    pub user_id: Option<Uuid>,
}

/// The batch mint/membership/probe SQL. Stateless — every method takes the caller's
/// connection so the header insert, the membership write, and the projection recompute
/// commit as one unit.
#[derive(Default)]
pub struct PickingBatchProjectionRepository;

impl PickingBatchProjectionRepository {
    pub fn new() -> Self {
        Self
    }

    /// Insert the batch header (a draft grouping point). Leaks the raw `sqlx::Error`
    /// deliberately so the service can turn a unique violation on (name, company_id)
    /// into the typed duplicate-name error.
    pub async fn insert_batch(
        &self,
        conn: &mut PgConnection,
        b: &NewBatchRow<'_>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"INSERT INTO inventory.picking_batches
                 (id, name, company_id, is_wave, user_id, scheduled_date, state, had_members)
               VALUES ($1, $2, $3, $4, $5, NOW(), 'draft'::picking_batch_state, FALSE)"#,
        )
        .bind(b.id)
        .bind(b.name)
        .bind(b.company_id)
        .bind(b.is_wave)
        .bind(b.user_id)
        .execute(conn)
        .await?;
        Ok(())
    }

    /// Read one batch header (probe surface — the projected state is READ here, never
    /// asserted by callers).
    pub async fn fetch_batch(
        &self,
        conn: &mut PgConnection,
        batch_id: Uuid,
    ) -> Result<Option<BatchHeaderRow>, sqlx::Error> {
        let row = sqlx::query(
            r#"SELECT id, company_id, name, state::text AS state, is_wave, user_id,
                      scheduled_date, had_members
               FROM inventory.picking_batches
               WHERE id = $1 AND (metadata->>'deleted_at') IS NULL"#,
        )
        .bind(batch_id)
        .fetch_optional(conn)
        .await?;
        Ok(row.map(|r| BatchHeaderRow {
            id: r.get("id"),
            company_id: r.get("company_id"),
            name: r.get("name"),
            state: r.get("state"),
            is_wave: r.get("is_wave"),
            user_id: r.get("user_id"),
            scheduled_date: r.get("scheduled_date"),
            had_members: r.get("had_members"),
        }))
    }

    /// Read a batch's member pickings (the projection inputs, for the probe surface).
    pub async fn fetch_members(
        &self,
        conn: &mut PgConnection,
        batch_id: Uuid,
    ) -> Result<Vec<BatchMemberRow>, sqlx::Error> {
        let rows = sqlx::query(
            r#"SELECT id, name, state::text AS state
               FROM inventory.transfers
               WHERE batch_id = $1 AND (metadata->>'deleted_at') IS NULL
               ORDER BY scheduled_date, id"#,
        )
        .bind(batch_id)
        .fetch_all(conn)
        .await?;
        Ok(rows
            .iter()
            .map(|r| BatchMemberRow {
                id: r.get("id"),
                name: r.get("name"),
                state: r.get("state"),
            })
            .collect())
    }

    /// Attach a picking to its batch: set the membership pointer and stamp the SB-1
    /// discriminator (`had_members` — once true, an empty batch auto-cancels instead of
    /// falling back to `draft`). The caller reprojects right after; this write only
    /// moves the pointer. The `batch_member_company_guard` trigger backstops the
    /// same-company requirement (the service checks first, loudly).
    pub async fn add_member(
        &self,
        conn: &mut PgConnection,
        batch_id: Uuid,
        picking_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"UPDATE inventory.transfers
               SET batch_id = $2
               WHERE id = $1 AND (metadata->>'deleted_at') IS NULL"#,
        )
        .bind(picking_id)
        .bind(batch_id)
        .execute(&mut *conn)
        .await?;
        sqlx::query(
            r#"UPDATE inventory.picking_batches
               SET had_members = TRUE
               WHERE id = $1"#,
        )
        .bind(batch_id)
        .execute(conn)
        .await?;
        Ok(())
    }

    /// Detach a picking from its batch (removal is always allowed — a done or cancelled
    /// picking still leaves its batch). Returns false when the picking was not a member.
    pub async fn remove_member(
        &self,
        conn: &mut PgConnection,
        batch_id: Uuid,
        picking_id: Uuid,
    ) -> Result<bool, sqlx::Error> {
        let res = sqlx::query(
            r#"UPDATE inventory.transfers
               SET batch_id = NULL
               WHERE id = $1 AND batch_id = $2 AND (metadata->>'deleted_at') IS NULL"#,
        )
        .bind(picking_id)
        .bind(batch_id)
        .execute(conn)
        .await?;
        Ok(res.rows_affected() == 1)
    }

    /// The SB-1 projection recompute (spec misc-features.md SB, ADR-0016 `projection`
    /// shape). `picking_batches.state` is a STORED COMPUTE over its member pickings:
    /// the LEAST-advanced live member band, with cancelled members dropping out of the
    /// aggregation entirely —
    ///   live member draft/confirmed → batch `draft` (nothing ready to wave-pick yet)
    ///   every live member ≥ waiting → `waiting`
    ///   every live member ≥ assigned → `ready`
    ///   every live member done     → `done`
    ///   no live member, members exist (all cancelled) → `cancel`
    ///   no member at all, the batch HAS grouped one before → `cancel` (auto-cancel on
    ///       empty — SB-1: an emptied batch must not linger as a live work item)
    ///   no member at all, never grouped one → `draft` (a freshly minted batch the
    ///       operator is about to fill)
    /// A member's `confirmed` counts at the draft band (Odoo's `_compute_state` has no
    /// `confirmed` in any prefix: a confirmed-but-unreserved picking is not wave-ready).
    /// Returns the new projected state (text), or `None` when the batch row is gone.
    pub async fn reproject_batch(
        &self,
        conn: &mut PgConnection,
        batch_id: Uuid,
    ) -> Result<Option<String>, sqlx::Error> {
        let row = sqlx::query(
            r#"WITH agg AS (
                   SELECT
                       COUNT(*) FILTER (WHERE state <> 'cancel'::transfer_state) AS live_count,
                       COUNT(*) AS total_count,
                       MIN(CASE state WHEN 'draft'::transfer_state THEN 0
                                      WHEN 'confirmed'::transfer_state THEN 0
                                      WHEN 'waiting'::transfer_state THEN 1
                                      WHEN 'assigned'::transfer_state THEN 2
                                      WHEN 'done'::transfer_state THEN 3
                                      ELSE 4 END)
                           FILTER (WHERE state <> 'cancel'::transfer_state) AS rank
                   FROM inventory.transfers
                   WHERE batch_id = $1 AND (metadata->>'deleted_at') IS NULL
               )
               UPDATE inventory.picking_batches b
               SET state = CASE
                       WHEN agg.live_count > 0 THEN CASE agg.rank
                           WHEN 0 THEN 'draft'::picking_batch_state
                           WHEN 1 THEN 'waiting'::picking_batch_state
                           WHEN 2 THEN 'ready'::picking_batch_state
                           WHEN 3 THEN 'done'::picking_batch_state
                           ELSE 'draft'::picking_batch_state END
                       WHEN agg.total_count > 0 THEN 'cancel'::picking_batch_state
                       WHEN b.had_members THEN 'cancel'::picking_batch_state
                       ELSE 'draft'::picking_batch_state END
               FROM agg
               WHERE b.id = $1
               RETURNING b.state::text"#,
        )
        .bind(batch_id)
        .fetch_optional(&mut *conn)
        .await?;
        Ok(row.map(|r| r.get::<String, _>("state")))
    }

    /// Read one picking's batch membership. Outer `None` = the transfer row is absent
    /// (or fenced out — the caller has bound the company scope); inner `None` = the
    /// picking belongs to no batch. The membership-guard input for the typed
    /// already-batched refusal.
    pub async fn picking_membership(
        &self,
        conn: &mut PgConnection,
        picking_id: Uuid,
    ) -> Result<Option<Option<Uuid>>, sqlx::Error> {
        let row = sqlx::query_scalar::<_, Option<Uuid>>(
            r#"SELECT batch_id FROM inventory.transfers
               WHERE id = $1 AND (metadata->>'deleted_at') IS NULL"#,
        )
        .bind(picking_id)
        .fetch_optional(&mut *conn)
        .await?;
        Ok(row)
    }

    /// The picking-projection cascade entry: after a picking's own state was re-derived,
    /// re-derive its batch's (if the picking belongs to one). Called by
    /// `StockMoveRepository::reproject_picking` on every member-state change so the
    /// batch never lags its members — the picking and its batch roll-up commit in the
    /// same transaction.
    pub async fn reproject_batch_of_picking(
        &self,
        conn: &mut PgConnection,
        picking_id: Uuid,
    ) -> Result<Option<String>, sqlx::Error> {
        let batch_id: Option<Uuid> = sqlx::query_scalar(
            r#"SELECT batch_id FROM inventory.transfers
               WHERE id = $1 AND (metadata->>'deleted_at') IS NULL"#,
        )
        .bind(picking_id)
        .fetch_optional(&mut *conn)
        .await?
        .flatten();
        match batch_id {
            Some(b) => self.reproject_batch(conn, b).await,
            None => Ok(None),
        }
    }
}
