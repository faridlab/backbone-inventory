//! Quant-driven adjustment SQL (hand-authored, user-owned).
//!
//! The physical half of the ONE adjustment door (spec stock-business-logic.md §5.2):
//! there is no `stock.inventory` model — a counted quantity STAGES on the quant row
//! (`inventory_quantity`), the diff is a stored compute (`inventory_diff_quantity`,
//! T4), and applying flips the quant to the count while the service mints the
//! `is_inventory` move through the move mint in [`super::stock_picking_repository`].
//!
//! Guards carried here: R24 (no staging/applying while the quant holds
//! reservations — checked by the service from the locked row this file returns),
//! R13 (only internal locations hold countable stock).
//!
//! Per the module's 4-layer rule the statements live here and take the caller's
//! connection, so the stage/apply writes commit as one unit with the move mint,
//! the SLE append, and the Bin reblance.

use rust_decimal::Decimal;
use sqlx::{PgConnection, Row};
use uuid::Uuid;

/// A locked quant + its location facts (the guard inputs the service checks).
pub struct QuantCountRow {
    pub id: Uuid,
    pub item_id: Uuid,
    pub location_id: Uuid,
    pub quantity: Decimal,
    pub reserved_quantity: Decimal,
    pub inventory_quantity: Option<Decimal>,
    pub inventory_diff_quantity: Option<Decimal>,
    pub inventory_quantity_set: bool,
    pub inventory_date: Option<chrono::NaiveDate>,
    /// Location usage as text (`internal` / `view` / ... — R13 check).
    pub location_usage: String,
    /// The location's warehouse — the grain the valuation engine (Bin/SLE) keys on.
    pub location_warehouse_id: Option<Uuid>,
}

/// How a count addresses a quant: by id, or by the (item, location) pair with all
/// optional dimensions NULL — the bin-compatible quant grain.
#[derive(Debug, Clone, Copy)]
pub enum QuantSelector {
    ById(Uuid),
    ItemLocation { item_id: Uuid, location_id: Uuid },
}

impl QuantSelector {
    /// (where-fragment, binds) for the selector. Returns the SQL predicate and the
    /// one-or-two bound values.
    fn where_clause(&self) -> (&'static str, Vec<Uuid>) {
        match self {
            QuantSelector::ById(id) => ("q.id = $1", vec![*id]),
            QuantSelector::ItemLocation { item_id, location_id } => (
                "q.item_id = $1 AND q.location_id = $2 AND q.lot_id IS NULL AND q.package_id IS NULL AND q.owner_id IS NULL",
                vec![*item_id, *location_id],
            ),
        }
    }
}

/// The quant staging/apply SQL. Stateless; every method takes the caller's connection.
#[derive(Default)]
pub struct StockAdjustmentRepository;

impl StockAdjustmentRepository {
    pub fn new() -> Self {
        Self
    }

    /// Lock the addressed quant `FOR UPDATE` (the reservation-apex row — every
    /// count stage/apply serializes here). `None` when no quant matches; the
    /// service then either errors (apply) or initializes one (stage).
    pub async fn lock_quant(
        &self,
        conn: &mut PgConnection,
        selector: QuantSelector,
    ) -> Result<Option<QuantCountRow>, sqlx::Error> {
        let (clause, binds) = selector.where_clause();
        let sql = format!(
            r#"SELECT q.id, q.item_id, q.location_id, q.quantity, q.reserved_quantity,
                      q.inventory_quantity, q.inventory_diff_quantity, q.inventory_quantity_set,
                      q.inventory_date,
                      l.usage::text AS location_usage,
                      l.warehouse_id AS location_warehouse_id
               FROM inventory.stock_quants q
               JOIN inventory.locations l ON l.id = q.location_id
               WHERE {clause} AND (q.metadata->>'deleted_at') IS NULL
               FOR UPDATE OF q"#
        );
        let mut q = sqlx::query(&sql);
        for b in binds {
            q = q.bind(b);
        }
        let row = q.fetch_optional(conn).await?;
        Ok(row.map(|r| QuantCountRow {
            id: r.get("id"),
            item_id: r.get("item_id"),
            location_id: r.get("location_id"),
            quantity: r.get("quantity"),
            reserved_quantity: r.get("reserved_quantity"),
            inventory_quantity: r.get("inventory_quantity"),
            inventory_diff_quantity: r.get("inventory_diff_quantity"),
            inventory_quantity_set: r.get("inventory_quantity_set"),
            inventory_date: r.get("inventory_date"),
            location_usage: r.get("location_usage"),
            location_warehouse_id: r.get("location_warehouse_id"),
        }))
    }

    /// Initialize the zeroed quant for (item, location) — the count surface exists
    /// even before any stock does. Returns its id; the caller has already verified
    /// the location is internal (R13).
    pub async fn init_quant(
        &self,
        conn: &mut PgConnection,
        item_id: Uuid,
        location_id: Uuid,
    ) -> Result<Uuid, sqlx::Error> {
        let id = Uuid::new_v4();
        sqlx::query(
            r#"INSERT INTO inventory.stock_quants
                 (id, item_id, location_id, quantity, reserved_quantity, available_quantity,
                  inventory_quantity_set, sn_duplicated)
               VALUES ($1, $2, $3, 0, 0, 0, FALSE, FALSE)"#,
        )
        .bind(id)
        .bind(item_id)
        .bind(location_id)
        .execute(conn)
        .await?;
        Ok(id)
    }

    /// Stage a counted quantity on a quant (T4): record the count and its stored
    /// compute (`inventory_diff_quantity = counted - quantity`), and raise the
    /// `inventory_quantity_set` gate that Apply demands. The pending-count partial
    /// index (`WHERE inventory_quantity_set = true`) is the worklist this feeds.
    pub async fn stage_count(
        &self,
        conn: &mut PgConnection,
        quant_id: Uuid,
        counted: Decimal,
        diff: Decimal,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"UPDATE inventory.stock_quants SET
                 inventory_quantity = $2,
                 inventory_diff_quantity = $3,
                 inventory_quantity_set = TRUE,
                 inventory_date = CURRENT_DATE
               WHERE id = $1"#,
        )
        .bind(quant_id)
        .bind(counted)
        .bind(diff)
        .execute(conn)
        .await?;
        Ok(())
    }

    /// Consume the staged count: clear the count fields and drop the
    /// `inventory_quantity_set` gate. This deliberately does NOT write `quantity` — the
    /// `is_inventory` move the apply minted through the engine pipeline is the one writer
    /// that reconciled the quant to the count (its two-step sync maintains `quantity`,
    /// `reserved_quantity`, and the T2 stored compute together). Consuming the staging is
    /// what makes re-application a no-op: a second apply finds the gate down and mints
    /// nothing.
    pub async fn consume_staged_count(
        &self,
        conn: &mut PgConnection,
        quant_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"UPDATE inventory.stock_quants SET
                 inventory_quantity = NULL,
                 inventory_diff_quantity = NULL,
                 inventory_quantity_set = FALSE,
                 inventory_date = NULL
               WHERE id = $1"#,
        )
        .bind(quant_id)
        .execute(conn)
        .await?;
        Ok(())
    }

    /// All quants at a location with a staged-but-unapplied count (the pending-count
    /// worklist the partial index `WHERE inventory_quantity_set = true` serves).
    pub async fn staged_counts_at_location(
        &self,
        conn: &mut PgConnection,
        location_id: Uuid,
    ) -> Result<Vec<StagedCountRow>, sqlx::Error> {
        let rows = sqlx::query(
            r#"SELECT id, item_id, quantity, inventory_quantity, inventory_diff_quantity
               FROM inventory.stock_quants
               WHERE location_id = $1 AND inventory_quantity_set
                 AND (metadata->>'deleted_at') IS NULL
               ORDER BY id"#,
        )
        .bind(location_id)
        .fetch_all(conn)
        .await?;
        Ok(rows
            .iter()
            .map(|r| StagedCountRow {
                quant_id: r.get("id"),
                item_id: r.get("item_id"),
                on_hand_qty: r.get("quantity"),
                counted_qty: r.get("inventory_quantity"),
                diff_qty: r.get("inventory_diff_quantity"),
            })
            .collect())
    }
}

/// One row of the pending-count worklist (the staged counts at a location).
pub struct StagedCountRow {
    pub quant_id: Uuid,
    pub item_id: Uuid,
    pub on_hand_qty: Decimal,
    pub counted_qty: Option<Decimal>,
    pub diff_qty: Option<Decimal>,
}
