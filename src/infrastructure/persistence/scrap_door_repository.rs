//! Scrap-door SQL (hand-authored, user-owned) — the stock.scrap satellite.
//!
//! Distinct from the generated `scrap_repository.rs` (the GenericCrud newtype): this file
//! holds the scrap DOOR's own SQL — the header mint (a draft scrap order), the probe
//! read, and the done-stamp that closes the door's document state. The document state
//! (`draft` → `done`) is genuinely HAND-SET (unlike the picking-batch projection): a
//! scrap is an operator document, and `done` is stamped exactly once, after the ONE
//! stock-move engine validated the door's move (the physical/valuation/SLE legs all ride
//! that single move — `scrapped = true` — there is no second estate here).
//!
//! Statements live here and take the caller's connection; services orchestrate. State
//! values bind as text with explicit casts and read back with `::text`.

use rust_decimal::Decimal;
use sqlx::{PgConnection, Row};
use uuid::Uuid;

/// A scrap order insert. `state` starts at `draft`; the done-stamp is the door's
/// terminal close after the engine validated the move.
pub struct NewScrapRow<'a> {
    pub id: Uuid,
    pub name: &'a str,
    pub origin: Option<&'a str>,
    pub item_id: Uuid,
    pub scrap_qty: Decimal,
    pub location_id: Uuid,
    pub scrap_location_id: Uuid,
    pub lot_id: Option<Uuid>,
    pub package_id: Option<Uuid>,
    pub owner_id: Option<Uuid>,
    pub picking_id: Option<Uuid>,
    pub scrap_reason_tag_ids: Vec<Uuid>,
}

/// The scrap probe surface: the header as the door reads it back.
#[derive(Debug, Clone)]
pub struct ScrapRow {
    pub id: Uuid,
    pub name: String,
    /// draft / done (as text).
    pub state: String,
    pub origin: Option<String>,
    pub item_id: Uuid,
    pub scrap_qty: Decimal,
    pub location_id: Uuid,
    pub scrap_location_id: Uuid,
    pub lot_id: Option<Uuid>,
    pub package_id: Option<Uuid>,
    pub owner_id: Option<Uuid>,
    pub picking_id: Option<Uuid>,
    /// The engine move this scrap landed through (set by the done-stamp).
    pub move_id: Option<Uuid>,
    pub scrap_reason_tag_ids: Vec<Uuid>,
}

/// The scrap door's mint/probe/close SQL. Stateless — every method takes the caller's
/// connection so the header insert and the later done-stamp run under the caller's scope.
#[derive(Default)]
pub struct ScrapDoorRepository;

impl ScrapDoorRepository {
    pub fn new() -> Self {
        Self
    }

    /// Insert the scrap header (draft). Leaks the raw `sqlx::Error` deliberately so the
    /// service can turn a unique violation on the name — the composing decorator's org-scoped
    /// arbiter — into the typed duplicate error.
    pub async fn insert_scrap(
        &self,
        conn: &mut PgConnection,
        s: &NewScrapRow<'_>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"INSERT INTO inventory.scraps
                 (id, name, state, origin, item_id, scrap_qty, location_id,
                  scrap_location_id, lot_id, package_id, owner_id, picking_id,
                  scrap_reason_tag_ids)
               VALUES ($1, $2, 'draft'::scrap_state, $3, $4, $5, $6,
                       $7, $8, $9, $10, $11, $12)"#,
        )
        .bind(s.id)
        .bind(s.name)
        .bind(s.origin)
        .bind(s.item_id)
        .bind(s.scrap_qty)
        .bind(s.location_id)
        .bind(s.scrap_location_id)
        .bind(s.lot_id)
        .bind(s.package_id)
        .bind(s.owner_id)
        .bind(s.picking_id)
        .bind(&s.scrap_reason_tag_ids)
        .execute(&mut *conn)
        .await?;
        Ok(())
    }

    /// Read one scrap header (probe surface).
    pub async fn fetch_scrap(
        &self,
        conn: &mut PgConnection,
        scrap_id: Uuid,
    ) -> Result<Option<ScrapRow>, sqlx::Error> {
        let row = sqlx::query(
            r#"SELECT id, name, state::text AS state, origin, item_id, scrap_qty,
                      location_id, scrap_location_id, lot_id, package_id, owner_id, picking_id,
                      move_id, scrap_reason_tag_ids
               FROM inventory.scraps
               WHERE id = $1 AND (metadata->>'deleted_at') IS NULL"#,
        )
        .bind(scrap_id)
        .fetch_optional(&mut *conn)
        .await?;
        Ok(row.map(|r| ScrapRow {
            id: r.get("id"),
            name: r.get("name"),
            state: r.get("state"),
            origin: r.get("origin"),
            item_id: r.get("item_id"),
            scrap_qty: r.get("scrap_qty"),
            location_id: r.get("location_id"),
            scrap_location_id: r.get("scrap_location_id"),
            lot_id: r.get("lot_id"),
            package_id: r.get("package_id"),
            owner_id: r.get("owner_id"),
            picking_id: r.get("picking_id"),
            move_id: r.get("move_id"),
            scrap_reason_tag_ids: r.get("scrap_reason_tag_ids"),
        }))
    }

    /// The terminal close: stamp the scrap `done` and bind the engine move it landed
    /// through. The `AND state = 'draft'` guard is the concurrency backstop — exactly one
    /// racer transitions; the other sees `false` and re-reads the header as `done`.
    pub async fn mark_scrap_done(
        &self,
        conn: &mut PgConnection,
        scrap_id: Uuid,
        move_id: Uuid,
    ) -> Result<bool, sqlx::Error> {
        let res = sqlx::query(
            r#"UPDATE inventory.scraps
               SET state = 'done'::scrap_state, move_id = $2
               WHERE id = $1 AND state = 'draft'::scrap_state"#,
        )
        .bind(scrap_id)
        .bind(move_id)
        .execute(&mut *conn)
        .await?;
        Ok(res.rows_affected() == 1)
    }
}
