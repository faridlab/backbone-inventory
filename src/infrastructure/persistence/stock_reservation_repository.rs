//! The stock-reservation ledger's persistence (hand-written; user-owned).
//!
//! Holds, releases, consumes, and sweeps over `inventory.stock_reservations`
//! — the checkout-intent ledger the availability read subtracts at the
//! warehouse grain. Nothing here ever writes the quant counters: the
//! reservation triangle's `reserved_quantity` is a mirror of open
//! stock-move lines and is recomputed from them, so a hold that wrote it
//! would be wiped by the next mirror pass.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use uuid::Uuid;

/// One held line's facts, as the availability subtraction and the acks
/// surface them.
#[derive(Debug, Clone)]
pub struct HeldReservation {
    pub id: Uuid,
    pub warehouse_id: Uuid,
    pub item_id: Uuid,
    pub qty: Decimal,
}

/// The per-(warehouse, item) held sum the availability read subtracts.
#[derive(Debug, Clone)]
pub struct HeldAtWarehouse {
    pub warehouse_id: Uuid,
    pub item_id: Uuid,
    pub held: Decimal,
}

#[derive(Debug, thiserror::Error)]
pub enum ReservationRepoError {
    #[error("db: {0}")]
    Db(#[from] sqlx::Error),
}

/// The reservation ledger repository. Every statement takes the caller's
/// connection or transaction so the verbs compose with the caller's fencing.
#[derive(Clone)]
pub struct StockReservationRepository;

impl StockReservationRepository {
    pub fn new() -> Self {
        Self
    }
}

impl Default for StockReservationRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl StockReservationRepository {
    /// Insert one held line. The unique partial index on
    /// `(ref_type, ref_id, item_id) WHERE status='held'` makes a re-hold of
    /// the same ref+item a caught conflict — the caller decides replace
    /// semantics on top.
    pub async fn insert_held(
        tx: &mut sqlx::PgConnection,
        ref_type: &str,
        ref_id: Uuid,
        warehouse_id: Uuid,
        item_id: Uuid,
        qty: Decimal,
        expires_at: DateTime<Utc>,
    ) -> Result<(), ReservationRepoError> {
        sqlx::query(
            r#"INSERT INTO inventory.stock_reservations
                 (ref_type, ref_id, warehouse_id, item_id, qty, status, expires_at)
               VALUES ($1, $2, $3, $4, $5, 'held', $6)"#,
        )
        .bind(ref_type)
        .bind(ref_id)
        .bind(warehouse_id)
        .bind(item_id)
        .bind(qty)
        .bind(expires_at)
        .execute(tx)
        .await?;
        Ok(())
    }

    /// Release the ref's held lines (idempotent): `held -> released`.
    /// Returns the released row ids, the caller publishes what it wants.
    pub async fn release_for_ref(
        tx: &mut sqlx::PgConnection,
        ref_type: &str,
        ref_id: Uuid,
        at: DateTime<Utc>,
    ) -> Result<Vec<Uuid>, ReservationRepoError> {
        let rows: Vec<(Uuid,)> = sqlx::query_as(
            r#"UPDATE inventory.stock_reservations
                  SET status = 'released', settled_at = $3
                WHERE ref_type = $1 AND ref_id = $2 AND status = 'held'
                RETURNING id"#,
        )
        .bind(ref_type)
        .bind(ref_id)
        .bind(at)
        .fetch_all(tx)
        .await?;
        Ok(rows.into_iter().map(|(id,)| id).collect())
    }

    /// Consume the ref's held lines (idempotent): `held -> consumed` — the
    /// real stock move's own reservation takes over from here.
    pub async fn consume_for_ref(
        tx: &mut sqlx::PgConnection,
        ref_type: &str,
        ref_id: Uuid,
        at: DateTime<Utc>,
    ) -> Result<Vec<Uuid>, ReservationRepoError> {
        let rows: Vec<(Uuid,)> = sqlx::query_as(
            r#"UPDATE inventory.stock_reservations
                  SET status = 'consumed', settled_at = $3
                WHERE ref_type = $1 AND ref_id = $2 AND status = 'held'
                RETURNING id"#,
        )
        .bind(ref_type)
        .bind(ref_id)
        .bind(at)
        .fetch_all(tx)
        .await?;
        Ok(rows.into_iter().map(|(id,)| id).collect())
    }

    /// The ref's current held lines (the inspect arm; also the re-hold
    /// decision's input).
    pub async fn held_for_ref(
        conn: &mut sqlx::PgConnection,
        ref_type: &str,
        ref_id: Uuid,
    ) -> Result<Vec<HeldReservation>, ReservationRepoError> {
        let rows: Vec<(Uuid, Uuid, Uuid, Decimal)> = sqlx::query_as(
            r#"SELECT id, warehouse_id, item_id, qty
                 FROM inventory.stock_reservations
                WHERE ref_type = $1 AND ref_id = $2 AND status = 'held'"#,
        )
        .bind(ref_type)
        .bind(ref_id)
        .fetch_all(conn)
        .await?;
        Ok(rows
            .into_iter()
            .map(|(id, warehouse_id, item_id, qty)| HeldReservation {
                id,
                warehouse_id,
                item_id,
                qty,
            })
            .collect())
    }

    /// The held sums per (warehouse, item) for one warehouse — the
    /// availability subtraction's read. Fresh, never cached.
    pub async fn held_at_warehouse(
        conn: &mut sqlx::PgConnection,
        warehouse_id: Uuid,
    ) -> Result<Vec<HeldAtWarehouse>, ReservationRepoError> {
        let rows: Vec<(Uuid, Uuid, Decimal)> = sqlx::query_as(
            r#"SELECT warehouse_id, item_id, COALESCE(SUM(qty), 0)
                 FROM inventory.stock_reservations
                WHERE warehouse_id = $1 AND status = 'held'
                  AND expires_at > now()
                GROUP BY warehouse_id, item_id"#,
        )
        .bind(warehouse_id)
        .fetch_all(conn)
        .await?;
        Ok(rows
            .into_iter()
            .map(|(warehouse_id, item_id, held)| HeldAtWarehouse {
                warehouse_id,
                item_id,
                held,
            })
            .collect())
    }

    /// The expiry sweep: `held -> expired` past the horizon, bounded batch,
    /// SKIP LOCKED so concurrent sweeps split the work.
    pub async fn expire_stale(
        tx: &mut sqlx::PgConnection,
        at: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<Uuid>, ReservationRepoError> {
        let rows: Vec<(Uuid,)> = sqlx::query_as(
            r#"UPDATE inventory.stock_reservations
                  SET status = 'expired', settled_at = $1
                WHERE id IN (
                    SELECT id FROM inventory.stock_reservations
                     WHERE status = 'held' AND expires_at <= $1
                     ORDER BY expires_at
                     LIMIT $2
                     FOR UPDATE SKIP LOCKED
                )
                RETURNING id"#,
        )
        .bind(at)
        .bind(limit)
        .fetch_all(tx)
        .await?;
        Ok(rows.into_iter().map(|(id,)| id).collect())
    }
}
