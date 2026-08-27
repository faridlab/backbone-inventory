//! Picking-as-projection SQL (hand-authored, user-owned).
//!
//! Holds every statement for the converged transfer/picking surface that is NOT already owned by
//! the move engine's repositories:
//!   - the transfer (picking) header mint — the picking groups draft moves; it is NEVER a state
//!     machine of its own (spec stock-business-logic.md §2 / §12 T1, ADR-0016 `projection`),
//!   - the projection PROBE reads (header + member move states): the projected
//!     `transfers.state` is READ here, never asserted or written — the only writer in the
//!     module is the move engine's recompute, fired on every move-state change
//!     (`StockMoveRepository::reproject_picking`),
//!   - location resolution helpers the adjustment door shares (warehouse stock location,
//!     inventory-loss location),
//!   - the quant-surface backfill (a warehouse whose stock was seeded through the legacy
//!     voucher paths has Bin rows but no quant rows; converged moves draw from quants, so the
//!     first converged touch of a grain heals the quant from the Bin — a loud, one-time-per-
//!     grain catch-up, never an ongoing second writer),
//!   - adjustment-move detection: the ONE adjustment door's mint-once gate (an existing live
//!     `is_inventory` move for the grain is resumed, a done one newer than the count means the
//!     count already applied).
//!
//! State values are snake_case; enum parameters are bound as text with explicit casts and read
//! back with `::text` (the module-wide enum lesson). Per the module's 4-layer rule the
//! statements live here and take the caller's connection; services orchestrate.

use rust_decimal::Decimal;
use sqlx::{PgConnection, Row};
use uuid::Uuid;

/// State of one member move, as the projection probe reads it.
pub struct MoveStateRow {
    pub id: Uuid,
    pub item_id: Uuid,
    pub state: String,
    pub demand_qty: Decimal,
    pub quantity: Decimal,
    pub is_inventory: bool,
}

/// Header fields of one transfer (the projection probe surface).
pub struct TransferHeaderRow {
    pub id: Uuid,
    /// The transfer's owning company (strict fence) — the caller binds the RLS scope from it.
    pub company_id: Uuid,
    pub name: String,
    pub origin: Option<String>,
    pub picking_type_id: Uuid,
    pub location_id: Uuid,
    pub location_dest_id: Uuid,
    pub state: String,
    pub date_done: Option<chrono::DateTime<chrono::Utc>>,
}

/// Facts of an operation type the picking mint/validate needs: the picking code (which GL leg
/// shape its moves post), the reservation posture, the partial-validate backorder policy, the
/// reference prefix its transfer names mint from, and its default shipping policy.
pub struct OperationTypeFacts {
    pub id: Uuid,
    /// incoming / outgoing / internal (as text).
    pub code: String,
    /// at_confirm / manual / by_date (as text).
    pub reservation_method: String,
    /// ask / always / never (as text).
    pub create_backorder: String,
    /// Reference prefix for transfer names (e.g. `IN/`, `WH/OUT/`) — the vocabulary a minted
    /// picking's name starts from.
    pub sequence_code: String,
    /// direct (ship as available) / one (ship all at once) — the shipping policy a minted
    /// picking inherits from its operation type.
    pub move_type: String,
}

/// An existing `is_inventory` move addressing a quant grain, as the adjustment door's
/// mint-once gate reads it.
pub struct AdjustmentMoveRow {
    pub id: Uuid,
    /// draft / waiting / confirmed / partially_available / assigned / done / cancel (as text).
    pub state: String,
    /// Effective date — stamped to the processing instant when the move hit `done`.
    pub date: Option<chrono::DateTime<chrono::Utc>>,
}

/// A transfer header insert. `state` starts at `draft`; member moves are minted through the
/// engine right after, and every mint/transition reprojects — so the stored value is always
/// the projection of the member move states.
pub struct NewPickingRow<'a> {
    pub id: Uuid,
    pub name: &'a str,
    pub origin: Option<&'a str>,
    pub picking_type_id: Uuid,
    pub location_id: Uuid,
    pub location_dest_id: Uuid,
    pub partner_id: Option<Uuid>,
    pub company_id: Uuid,
    pub move_type: &'a str,
}

/// The picking mint/probe/heal SQL. Stateless — every method takes the caller's connection so
/// the header, its moves, and the projection commit as one unit (moves themselves are minted
/// through the engine's own guarded transactions).
#[derive(Default)]
pub struct StockPickingRepository;

impl StockPickingRepository {
    pub fn new() -> Self {
        Self
    }

    /// Insert the transfer header. Leaks the raw `sqlx::Error` deliberately so the service can
    /// turn a unique violation on (name, company_id) — guard R3 — into a typed
    /// duplicate-name error.
    pub async fn insert_transfer(
        &self,
        conn: &mut PgConnection,
        t: &NewPickingRow<'_>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"INSERT INTO inventory.transfers
                 (id, name, origin, priority, picking_type_id, location_id, location_dest_id,
                  partner_id, company_id, move_type, scheduled_date, state)
               VALUES ($1, $2, $3, 'normal', $4, $5, $6, $7, $8, $9::move_type, NOW(),
                       'draft'::transfer_state)"#,
        )
        .bind(t.id)
        .bind(t.name)
        .bind(t.origin)
        .bind(t.picking_type_id)
        .bind(t.location_id)
        .bind(t.location_dest_id)
        .bind(t.partner_id)
        .bind(t.company_id)
        .bind(t.move_type)
        .execute(conn)
        .await?;
        Ok(())
    }

    /// Read one transfer header (probe surface — the projected state is READ here, never
    /// asserted by callers).
    pub async fn fetch_transfer(
        &self,
        conn: &mut PgConnection,
        transfer_id: Uuid,
    ) -> Result<Option<TransferHeaderRow>, sqlx::Error> {
        let row = sqlx::query(
            r#"SELECT id, company_id, name, origin, picking_type_id, location_id, location_dest_id,
                      state::text AS state, date_done
               FROM inventory.transfers
               WHERE id = $1 AND (metadata->>'deleted_at') IS NULL"#,
        )
        .bind(transfer_id)
        .fetch_optional(conn)
        .await?;
        Ok(row.map(|r| TransferHeaderRow {
            id: r.get("id"),
            company_id: r.get("company_id"),
            name: r.get("name"),
            origin: r.get("origin"),
            picking_type_id: r.get("picking_type_id"),
            location_id: r.get("location_id"),
            location_dest_id: r.get("location_dest_id"),
            state: r.get("state"),
            date_done: r.get("date_done"),
        }))
    }

    /// Read a transfer's member moves (the projection inputs, for the probe surface).
    pub async fn fetch_moves_of_transfer(
        &self,
        conn: &mut PgConnection,
        transfer_id: Uuid,
    ) -> Result<Vec<MoveStateRow>, sqlx::Error> {
        let rows = sqlx::query(
            r#"SELECT id, item_id, state::text AS state, demand_qty, quantity, is_inventory
               FROM inventory.stock_moves
               WHERE picking_id = $1 AND (metadata->>'deleted_at') IS NULL
               ORDER BY create_date, id"#,
        )
        .bind(transfer_id)
        .fetch_all(conn)
        .await?;
        Ok(rows
            .iter()
            .map(|r| MoveStateRow {
                id: r.get("id"),
                item_id: r.get("item_id"),
                state: r.get("state"),
                demand_qty: r.get("demand_qty"),
                quantity: r.get("quantity"),
                is_inventory: r.get("is_inventory"),
            })
            .collect())
    }

    /// The ids of a transfer's member moves (the validate surface iterates these).
    pub async fn move_ids_of_transfer(
        &self,
        conn: &mut PgConnection,
        transfer_id: Uuid,
    ) -> Result<Vec<Uuid>, sqlx::Error> {
        let rows = sqlx::query(
            r#"SELECT id FROM inventory.stock_moves
               WHERE picking_id = $1 AND (metadata->>'deleted_at') IS NULL"#,
        )
        .bind(transfer_id)
        .fetch_all(conn)
        .await?;
        Ok(rows.iter().map(|r| r.get("id")).collect())
    }

    /// The operation type's facts (the picking mint refuses a type from another company).
    /// `None` when the type does not exist or is not usable by this company.
    pub async fn fetch_operation_type(
        &self,
        conn: &mut PgConnection,
        operation_type_id: Uuid,
        company_id: Uuid,
    ) -> Result<Option<OperationTypeFacts>, sqlx::Error> {
        let row = sqlx::query(
            r#"SELECT id, code::text AS code, reservation_method::text AS reservation_method,
                      create_backorder::text AS create_backorder, sequence_code,
                      move_type::text AS move_type
               FROM inventory.operation_types
               WHERE id = $1 AND active AND (metadata->>'deleted_at') IS NULL
                 AND (company_id = $2 OR company_id IS NULL)"#,
        )
        .bind(operation_type_id)
        .bind(company_id)
        .fetch_optional(conn)
        .await?;
        Ok(row.map(|r| OperationTypeFacts {
            id: r.get("id"),
            code: r.get("code"),
            reservation_method: r.get("reservation_method"),
            create_backorder: r.get("create_backorder"),
            sequence_code: r.get("sequence_code"),
            move_type: r.get("move_type"),
        }))
    }

    /// The OPEN transfer a move would join when it is grouped for fulfillment: the most
    /// recently scheduled transfer of the same company whose operation type, source,
    /// destination, partner, and origin (the procurement-group stand-in — moves launched for
    /// one source document share it) all match, and whose projection has not reached a
    /// terminal state (`done` picks nothing more up; a `cancel` never reopens). `None` when no
    /// open transfer matches — the caller mints one.
    pub async fn find_open_group_picking(
        &self,
        conn: &mut PgConnection,
        company_id: Uuid,
        picking_type_id: Uuid,
        location_id: Uuid,
        location_dest_id: Uuid,
        partner_id: Option<Uuid>,
        origin: Option<&str>,
    ) -> Result<Option<Uuid>, sqlx::Error> {
        let row = sqlx::query_scalar::<_, Uuid>(
            r#"SELECT id FROM inventory.transfers
               WHERE company_id = $1
                 AND picking_type_id = $2
                 AND location_id = $3
                 AND location_dest_id = $4
                 AND partner_id IS NOT DISTINCT FROM $5
                 AND origin IS NOT DISTINCT FROM $6
                 AND state NOT IN ('done'::transfer_state, 'cancel'::transfer_state)
                 AND (metadata->>'deleted_at') IS NULL
               ORDER BY scheduled_date DESC, id
               LIMIT 1"#,
        )
        .bind(company_id)
        .bind(picking_type_id)
        .bind(location_id)
        .bind(location_dest_id)
        .bind(partner_id)
        .bind(origin)
        .fetch_optional(conn)
        .await?;
        Ok(row)
    }

    /// The warehouse's stock location: the internal location the warehouse tree resolves to
    /// (deterministic: shallowest in the tree, then name). `None` when the warehouse has no
    /// internal location yet — the caller decides whether to bootstrap one.
    pub async fn resolve_internal_location(
        &self,
        conn: &mut PgConnection,
        warehouse_id: Uuid,
    ) -> Result<Option<Uuid>, sqlx::Error> {
        let row = sqlx::query(
            r#"SELECT id FROM inventory.locations
               WHERE warehouse_id = $1 AND usage = 'internal'::location_usage AND active
                 AND (metadata->>'deleted_at') IS NULL
               ORDER BY parent_path ASC NULLS FIRST, name ASC LIMIT 1"#,
        )
        .bind(warehouse_id)
        .fetch_optional(conn)
        .await?;
        Ok(row.map(|r| r.get("id")))
    }

    /// The inventory-loss location for a company (the far end of every adjustment move):
    /// company-owned first, shared (company NULL) as fallback. `None` when neither exists —
    /// the caller decides whether to bootstrap one.
    pub async fn resolve_inventory_loss_location(
        &self,
        conn: &mut PgConnection,
        company_id: Uuid,
    ) -> Result<Option<Uuid>, sqlx::Error> {
        let row = sqlx::query(
            r#"SELECT id FROM inventory.locations
               WHERE usage = 'inventory'::location_usage AND active
                 AND (metadata->>'deleted_at') IS NULL
                 AND (company_id = $1 OR company_id IS NULL)
               ORDER BY (company_id IS NOT NULL) DESC, name ASC LIMIT 1"#,
        )
        .bind(company_id)
        .fetch_optional(conn)
        .await?;
        Ok(row.map(|r| r.get("id")))
    }

    /// Bootstrap a warehouse's stock location (internal usage, company-fenced). The
    /// adjustment door and the picking mint use this the first time a warehouse without a
    /// location tree stages a count or mints a picking — a physical grain needs a location,
    /// and minting the warehouse's own stock location is the least-surprise resolution.
    pub async fn ensure_internal_location(
        &self,
        conn: &mut PgConnection,
        warehouse_id: Uuid,
        company_id: Uuid,
    ) -> Result<Uuid, sqlx::Error> {
        if let Some(id) = self.resolve_internal_location(conn, warehouse_id).await? {
            return Ok(id);
        }
        let id = Uuid::new_v4();
        sqlx::query(
            r#"INSERT INTO inventory.locations
                 (id, name, complete_name, usage, active, company_id, warehouse_id, parent_path)
               VALUES ($1, 'Stock', 'Stock', 'internal'::location_usage, TRUE, $2, $3, $1::text)"#,
        )
        .bind(id)
        .bind(company_id)
        .bind(warehouse_id)
        .execute(conn)
        .await?;
        Ok(id)
    }

    /// Bootstrap the company's inventory-loss location (usage `inventory` — the scrap /
    /// adjustment sink). Shared root trees would normally seed this; the module ships no
    /// location seed yet, so the door mints it on first use.
    pub async fn ensure_inventory_loss_location(
        &self,
        conn: &mut PgConnection,
        company_id: Uuid,
    ) -> Result<Uuid, sqlx::Error> {
        if let Some(id) = self.resolve_inventory_loss_location(conn, company_id).await? {
            return Ok(id);
        }
        let id = Uuid::new_v4();
        sqlx::query(
            r#"INSERT INTO inventory.locations
                 (id, name, complete_name, usage, active, company_id, parent_path)
               VALUES ($1, 'Inventory adjustment', 'Inventory adjustment',
                       'inventory'::location_usage, TRUE, $2, $1::text)"#,
        )
        .bind(id)
        .bind(company_id)
        .execute(conn)
        .await?;
        Ok(id)
    }

    /// Resolve the company's location of a partner usage (`supplier` / `customer`): the
    /// company-owned one first, the shared root (company NULL) as fallback. `None` when
    /// neither exists — the caller decides whether to bootstrap one.
    pub async fn resolve_partner_location(
        &self,
        conn: &mut PgConnection,
        company_id: Uuid,
        usage: &str,
    ) -> Result<Option<Uuid>, sqlx::Error> {
        let row = sqlx::query(
            r#"SELECT id FROM inventory.locations
               WHERE usage = $2::location_usage AND active
                 AND (metadata->>'deleted_at') IS NULL
                 AND (company_id = $1 OR company_id IS NULL)
               ORDER BY (company_id IS NOT NULL) DESC, name ASC LIMIT 1"#,
        )
        .bind(company_id)
        .bind(usage)
        .fetch_optional(conn)
        .await?;
        Ok(row.map(|r| r.get("id")))
    }

    /// Bootstrap the company's counterpart location for a partner usage (`supplier` — the
    /// far end of every goods receipt; `customer` — the far end of every delivery). Voucher
    /// doors are warehouse-grain, moves are location-grain: this is the resolve-or-mint that
    /// gives the door's moves their virtual counterpart endpoint.
    pub async fn ensure_partner_location(
        &self,
        conn: &mut PgConnection,
        company_id: Uuid,
        usage: &str,
    ) -> Result<Uuid, sqlx::Error> {
        if let Some(id) = self.resolve_partner_location(conn, company_id, usage).await? {
            return Ok(id);
        }
        let id = Uuid::new_v4();
        let name = if usage == "supplier" { "Suppliers" } else { "Customers" };
        sqlx::query(
            r#"INSERT INTO inventory.locations
                 (id, name, complete_name, usage, active, company_id, parent_path)
               VALUES ($1, $2, $2, $3::location_usage, TRUE, $4, $1::text)"#,
        )
        .bind(id)
        .bind(name)
        .bind(usage)
        .bind(company_id)
        .execute(conn)
        .await?;
        Ok(id)
    }

    /// The quant-surface backfill: guarantee a quant row exists at the bin grain
    /// (item, location, all tracking dims NULL), initialized from the Bin balance of the
    /// location's warehouse. Stock seeded through the legacy voucher paths wrote Bins without
    /// quants; converged moves draw from quants, so the first converged touch of a grain
    /// heals it. Idempotent by construction (NOT EXISTS guard + the same advisory-lock
    /// pattern `QuantRepository::lock_or_init` uses — the unique index does not dedup NULL
    /// dimensions, so creation must serialize). A second call finds the row and does nothing:
    /// this is a one-time catch-up, never an ongoing second writer of `quantity` (the move
    /// pipeline is the only ongoing writer).
    pub async fn ensure_quant_surface(
        &self,
        conn: &mut PgConnection,
        company_id: Uuid,
        item_id: Uuid,
        location_id: Uuid,
        warehouse_id: Option<Uuid>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(format!("quant:{company_id}:{item_id}:{location_id}:NULL:NULL:NULL"))
            .execute(&mut *conn)
            .await?;
        // Location with a warehouse: seed from that warehouse's Bin balance (the legacy
        // voucher paths wrote Bins only — the heal makes the quant surface catch up to the
        // balance of record exactly once).
        sqlx::query(
            r#"INSERT INTO inventory.stock_quants
                 (id, item_id, location_id, quantity, reserved_quantity, available_quantity,
                  in_date, company_id)
               SELECT $1, $2, $3,
                      COALESCE((SELECT b.actual_qty FROM inventory.bins b
                                WHERE b.company_id = $4 AND b.item_id = $2
                                  AND b.warehouse_id = $5::uuid
                                  AND (b.metadata->>'deleted_at') IS NULL), 0),
                      0,
                      COALESCE((SELECT b.actual_qty FROM inventory.bins b
                                WHERE b.company_id = $4 AND b.item_id = $2
                                  AND b.warehouse_id = $5::uuid
                                  AND (b.metadata->>'deleted_at') IS NULL), 0),
                      NOW(), $4
               WHERE $5::uuid IS NOT NULL
                 AND NOT EXISTS (
                   SELECT 1 FROM inventory.stock_quants q
                   WHERE q.company_id = $4 AND q.item_id = $2 AND q.location_id = $3
                     AND q.lot_id IS NULL AND q.package_id IS NULL AND q.owner_id IS NULL
                     AND (q.metadata->>'deleted_at') IS NULL
                 )"#,
        )
        .bind(Uuid::new_v4())
        .bind(item_id)
        .bind(location_id)
        .bind(company_id)
        .bind(warehouse_id)
        .execute(&mut *conn)
        .await?;
        // Location without a warehouse (shared / virtual): a zeroed quant row is enough —
        // there is no Bin behind it, and the moves that touch it flip it from zero.
        sqlx::query(
            r#"INSERT INTO inventory.stock_quants
                 (id, item_id, location_id, quantity, reserved_quantity, available_quantity,
                  in_date, company_id)
               SELECT $1, $2, $3, 0, 0, 0, NOW(), $4
               WHERE $5::uuid IS NULL
                 AND NOT EXISTS (
                   SELECT 1 FROM inventory.stock_quants q
                   WHERE q.company_id = $4 AND q.item_id = $2 AND q.location_id = $3
                     AND q.lot_id IS NULL AND q.package_id IS NULL AND q.owner_id IS NULL
                     AND (q.metadata->>'deleted_at') IS NULL
                 )"#,
        )
        .bind(Uuid::new_v4())
        .bind(item_id)
        .bind(location_id)
        .bind(company_id)
        .bind(warehouse_id)
        .execute(&mut *conn)
        .await?;
        Ok(())
    }

    /// The adjustment door's mint-once gate: an existing `is_inventory` move addressing this
    /// item between exactly these two locations (either direction — the count direction only
    /// flips the pair). Live moves first (resume candidates), then done ones (the
    /// already-applied evidence), newest first.
    pub async fn find_adjustment_moves(
        &self,
        conn: &mut PgConnection,
        company_id: Uuid,
        item_id: Uuid,
        location_a: Uuid,
        location_b: Uuid,
    ) -> Result<Vec<AdjustmentMoveRow>, sqlx::Error> {
        let rows = sqlx::query(
            r#"SELECT id, state::text AS state, date
               FROM inventory.stock_moves
               WHERE company_id = $1 AND item_id = $2 AND is_inventory
                 AND (metadata->>'deleted_at') IS NULL
                 AND ((location_id = $3 AND location_dest_id = $4)
                   OR (location_id = $4 AND location_dest_id = $3))
               ORDER BY (state NOT IN ('done'::move_state, 'cancel'::move_state)) DESC,
                        date DESC NULLS LAST, create_date DESC"#,
        )
        .bind(company_id)
        .bind(item_id)
        .bind(location_a)
        .bind(location_b)
        .fetch_all(conn)
        .await?;
        Ok(rows
            .iter()
            .map(|r| AdjustmentMoveRow { id: r.get("id"), state: r.get("state"), date: r.get("date") })
            .collect())
    }
}
