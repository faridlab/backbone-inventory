//! The store-level reservation surface (hand-written; user-owned) — the
//! checkout-intent ledger the register row asked for: intent + release
//! verbs on the stock authority, checkout consumes them.
//!
//! Posture (the design recorded on the issue):
//!
//! - Holds NEVER write the quant counters. `stock_quants.reserved_quantity`
//!   mirrors open stock-move lines and is recomputed from them; a hold
//!   written there would be wiped by the next mirror pass. Holds live in
//!   `inventory.stock_reservations` and the AVAILABILITY READ subtracts
//!   them at the warehouse grain.
//! - Hold is idempotent per (ref, item): a re-hold REPLACES the line
//!   (release-then-insert in one transaction) so the cart's freeze step can
//!   run freely.
//! - Hold refuses typed when the fresh availability triangle minus held
//!   reservations cannot cover the ask — the advisory check-then-act race
//!   window stays (two carts can still both pass the read), but a passed
//!   hold is bounded by the ledger's own arithmetic.
//! - Consume is the bridge at order confirm: the hold's quantity is
//!   superseded by the real reservation the delivery door's move lines
//!   create (the mirror owns reserved_quantity from there).
//! - Expiry is the abandonment policy: holds past their TTL expire in
//!   bounded SKIP LOCKED batches (the same idiom the promo claim sweep
//!   uses).

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::infrastructure::persistence::stock_reservation_repository::{
    ReservationRepoError, StockReservationRepository as Repo,
};

#[derive(Debug, thiserror::Error)]
pub enum ReservationError {
    #[error("db: {0}")]
    Db(#[from] sqlx::Error),
    #[error("cannot hold {asked} of item {item}: availability after existing holds is {available}")]
    InsufficientForHold {
        item: Uuid,
        asked: Decimal,
        available: Decimal,
    },
    #[error("hold lines carry no quantities")]
    EmptyHold,
}

/// One line of a hold request.
#[derive(Debug, Clone)]
pub struct HoldLine {
    pub item_id: Uuid,
    pub qty: Decimal,
}

/// The hold outcome, per line: the reservation id and the availability
/// that remained after the hold landed.
#[derive(Debug, Clone)]
pub struct HoldAckLine {
    pub item_id: Uuid,
    pub reservation_id: Uuid,
    pub remaining_after_hold: Decimal,
}

/// The verbs. Stateless over a pool; each verb runs in one transaction with
/// the ambient org scope relayed (the module's uniform posture).
#[derive(Clone)]
pub struct ReservationService {
    pool: sqlx::PgPool,
}

impl ReservationService {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }

    async fn scoped_tx(
        &self,
    ) -> Result<sqlx::Transaction<'_, sqlx::Postgres>, sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        if let Some(scope) = backbone_orm::org_scope::current_org_scope() {
            backbone_orm::org_scope::bind_org_scope_on(&mut tx, &scope).await?;
        }
        Ok(tx)
    }

    /// Hold `lines` for `ref` at `warehouse` until `expires_at`. Idempotent
    /// per (ref, item): existing held lines for the ref are replaced. Every
    /// line must fit the fresh availability triangle minus held
    /// reservations, or the whole hold refuses (all-or-nothing).
    pub async fn hold(
        &self,
        ref_type: &str,
        ref_id: Uuid,
        warehouse_id: Uuid,
        lines: &[HoldLine],
        expires_at: DateTime<Utc>,
    ) -> Result<Vec<HoldAckLine>, ReservationError> {
        if lines.is_empty() || lines.iter().any(|l| l.qty <= Decimal::ZERO) {
            return Err(ReservationError::EmptyHold);
        }
        // The availability triangle this hold must fit inside: fresh
        // per call, held subtracted, at the warehouse grain.
        let availability = self
            .availability_after_holds(warehouse_id)
            .await?;
        let mut tx = self.scoped_tx().await?;
        // Replace semantics: drop the ref's held lines first (a no-op on
        // the first hold).
        Repo::release_for_ref(&mut tx, ref_type, ref_id, Utc::now()).await?;
        let mut acks = Vec::with_capacity(lines.len());
        for line in lines {
            let available = availability
                .iter()
                .find(|(item, _)| *item == line.item_id)
                .map(|(_, qty)| *qty)
                .unwrap_or(Decimal::ZERO);
            if line.qty > available {
                return Err(ReservationError::InsufficientForHold {
                    item: line.item_id,
                    asked: line.qty,
                    available,
                });
            }
            // The insert's own id is generated; read it back for the ack.
            let id: Uuid = sqlx::query_scalar(
                r#"INSERT INTO inventory.stock_reservations
                     (ref_type, ref_id, warehouse_id, item_id, qty, status, expires_at)
                   VALUES ($1, $2, $3, $4, $5, 'held', $6)
                   RETURNING id"#,
            )
            .bind(ref_type)
            .bind(ref_id)
            .bind(warehouse_id)
            .bind(line.item_id)
            .bind(line.qty)
            .bind(expires_at)
            .fetch_one(&mut *tx)
            .await?;
            acks.push(HoldAckLine {
                item_id: line.item_id,
                reservation_id: id,
                remaining_after_hold: available - line.qty,
            });
        }
        tx.commit().await?;
        Ok(acks)
    }

    /// Release the ref's held lines (the cart-close hook). Idempotent.
    pub async fn release(&self, ref_type: &str, ref_id: Uuid) -> Result<usize, ReservationError> {
        let mut tx = self.scoped_tx().await?;
        let released = Repo::release_for_ref(&mut tx, ref_type, ref_id, Utc::now()).await?;
        tx.commit().await?;
        Ok(released.len())
    }

    /// Consume the ref's held lines (the order-confirm bridge: the real
    /// move's reservation takes over). Idempotent.
    pub async fn consume(&self, ref_type: &str, ref_id: Uuid) -> Result<usize, ReservationError> {
        let mut tx = self.scoped_tx().await?;
        let consumed = Repo::consume_for_ref(&mut tx, ref_type, ref_id, Utc::now()).await?;
        tx.commit().await?;
        Ok(consumed.len())
    }

    /// The ref's held lines (inspect).
    pub async fn held_for_ref(
        &self,
        ref_type: &str,
        ref_id: Uuid,
    ) -> Result<
        Vec<crate::infrastructure::persistence::stock_reservation_repository::HeldReservation>,
        ReservationError,
    > {
        let mut conn = self.pool.acquire().await?;
        if let Some(scope) = backbone_orm::org_scope::current_org_scope() {
            // A short-lived read connection with the fence bound, the
            // module's uniform read posture.
            let _ = backbone_orm::org_scope::bind_org_scope_on(&mut conn, &scope).await;
        }
        Ok(Repo::held_for_ref(&mut conn, ref_type, ref_id).await?)
    }

    /// The expiry sweep — the abandonment policy. Bounded batches; the
    /// caller (a host job pass) loops until it answers zero.
    pub async fn sweep_expired(&self, limit: i64) -> Result<usize, ReservationError> {
        let mut tx = self.scoped_tx().await?;
        let expired = Repo::expire_stale(&mut tx, Utc::now(), limit).await?;
        tx.commit().await?;
        Ok(expired.len())
    }

    /// The fresh triangle minus held reservations per item at one warehouse:
    /// `(quantity - reserved_quantity)` summed over the warehouse's internal
    /// locations, then the held ledger subtracted at the warehouse grain.
    /// The availability read's own arithmetic, re-expressed here so the
    /// hold check and the read can never diverge.
    async fn availability_after_holds(
        &self,
        warehouse_id: Uuid,
    ) -> Result<Vec<(Uuid, Decimal)>, ReservationError> {
        let mut conn = self.pool.acquire().await?;
        if let Some(scope) = backbone_orm::org_scope::current_org_scope() {
            let _ = backbone_orm::org_scope::bind_org_scope_on(&mut conn, &scope).await;
        }
        let rows: Vec<(Uuid, Decimal)> = sqlx::query_as(
            r#"WITH triangle AS (
                   SELECT q.item_id,
                          COALESCE(SUM(q.quantity - q.reserved_quantity), 0) AS avail
                     FROM inventory.stock_quants q
                     JOIN inventory.locations l ON l.id = q.location_id
                    WHERE l.warehouse_id = $1
                      AND l.usage::text = 'internal'
                      AND (q.metadata->>'deleted_at') IS NULL
                      AND (l.metadata->>'deleted_at') IS NULL
                    GROUP BY q.item_id
               ),
               held AS (
                   SELECT item_id, COALESCE(SUM(qty), 0) AS held
                     FROM inventory.stock_reservations
                    WHERE warehouse_id = $1 AND status = 'held' AND expires_at > now()
                    GROUP BY item_id
               )
               SELECT t.item_id, t.avail - COALESCE(h.held, 0)
                 FROM triangle t LEFT JOIN held h ON h.item_id = t.item_id"#,
        )
        .bind(warehouse_id)
        .fetch_all(&mut *conn)
        .await?;
        Ok(rows)
    }
}

impl From<ReservationRepoError> for ReservationError {
    fn from(e: ReservationRepoError) -> Self {
        match e {
            ReservationRepoError::Db(db) => ReservationError::Db(db),
        }
    }
}
