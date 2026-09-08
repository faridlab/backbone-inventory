//! Procurement SQL (hand-authored, user-owned): everything behind the routes/rules/orderpoints
//! service (`application/service/procurement_service.rs`) and the daily scheduler
//! (`infrastructure/jobs/run_scheduler.rs`).
//!
//! Scope: rule search (`_search_rule`, stock-business-logic §6), the orderpoint T11 forecast
//! aggregation (§12 T11), replenishment-move minting, the scheduler's intake claims
//! (`FOR UPDATE SKIP LOCKED`, ADR-0020 §4), and the quant vacuum tail. The move lifecycle verbs
//! themselves (confirm / assign / done) live in the stock-move engine — this file only mints
//! draft rows and claims existing ones; it never mutates a move's state.
//!
//! Conventions carried from the module's proven hand-written SQL:
//! - enum values bind as text with an explicit cast (`$n::move_state`), never via `sqlx::Type`
//!   derives, and decode through `::text` aliases (the schema-normalization R1 lessons);
//! - every read filters `(metadata->>'deleted_at') IS NULL` (soft-delete grain);
//! - every claim is company-scoped by an explicit `company_id = $n` predicate (defense-in-depth
//!   under the RLS fence, ADR-0008 — the job binds `app.company_id` on the connection too).

use rust_decimal::Decimal;
use sqlx::{Executor, PgConnection, Postgres, Row};
use uuid::Uuid;

/// A `stock.rule` row as the procurement surface reads it (`inventory.route_rules`).
#[derive(Debug, Clone)]
pub struct RuleRow {
    pub id: Uuid,
    pub name: String,
    pub sequence: i32,
    pub action: String,
    pub auto: String,
    pub procure_method: String,
    pub delay: i32,
    pub location_src_id: Option<Uuid>,
    pub location_dest_id: Uuid,
    pub picking_type_id: Uuid,
    pub route_id: Uuid,
    pub warehouse_id: Option<Uuid>,
    pub company_id: Option<Uuid>,
    pub propagate_cancel: bool,
}

/// A `stock.warehouse.orderpoint` row as the scheduler claims it (`inventory.reordering_rules`).
#[derive(Debug, Clone)]
pub struct OrderpointRow {
    pub id: Uuid,
    pub name: String,
    pub trigger: String,
    pub item_id: Uuid,
    pub location_id: Uuid,
    pub warehouse_id: Uuid,
    pub company_id: Uuid,
    pub item_min_qty: Decimal,
    pub item_max_qty: Decimal,
    pub route_id: Option<Uuid>,
    pub qty_to_order_manual: Decimal,
}

/// A claimed confirmed/partially-available move (the scheduler's assign sweep intake).
#[derive(Debug, Clone)]
pub struct ClaimedMoveRow {
    pub id: Uuid,
    pub name: String,
    pub item_id: Uuid,
    pub state: String,
}

/// The T11 inputs gathered per orderpoint: available on hand plus the open incoming/outgoing
/// remainder around the monitored location subtree.
#[derive(Debug, Clone, Copy, Default)]
pub struct ForecastComponents {
    pub on_hand: Decimal,
    pub incoming: Decimal,
    pub outgoing: Decimal,
}

/// The stored computes written back onto the orderpoint row (§12 T11).
#[derive(Debug, Clone, Copy)]
pub struct OrderpointComputes {
    pub qty_on_hand: Decimal,
    pub qty_forecast: Decimal,
    pub qty_to_order: Decimal,
    pub lead_days: Decimal,
    pub deadline_date: Option<chrono::NaiveDate>,
}

/// The values a minted draft move carries into `inventory.stock_moves`.
#[derive(Debug, Clone)]
pub struct NewMoveRow {
    pub name: String,
    pub item_id: Uuid,
    pub demand_qty: Decimal,
    pub procure_method: String,
    pub origin: String,
    pub location_id: Uuid,
    pub location_dest_id: Uuid,
    pub company_id: Uuid,
    pub rule_id: Option<Uuid>,
    pub warehouse_id: Option<Uuid>,
    pub orderpoint_id: Option<Uuid>,
    pub move_orig_ids: Vec<Uuid>,
    pub move_dest_ids: Vec<Uuid>,
    pub propagate_cancel: bool,
}

fn rule_row(m: sqlx::postgres::PgRow) -> RuleRow {
    RuleRow {
        id: m.get("id"),
        name: m.get("name"),
        sequence: m.get("sequence"),
        action: m.get("action"),
        auto: m.get("auto"),
        procure_method: m.get("procure_method"),
        delay: m.get("delay"),
        location_src_id: m.get("location_src_id"),
        location_dest_id: m.get("location_dest_id"),
        picking_type_id: m.get("picking_type_id"),
        route_id: m.get("route_id"),
        warehouse_id: m.get("warehouse_id"),
        company_id: m.get("company_id"),
        propagate_cancel: m.get("propagate_cancel"),
    }
}

/// SQL fragment shared by every subtree read: the set of location ids at or below the demand
/// location's path (a CTE named `demand` exposing `parent_path`). A location's `parent_path`
/// prefixes every descendant's path, so `LIKE path || '%'` is the whole subtree including the
/// location itself.
const SUBTREE_SQL: &str =
    "SELECT l.id FROM inventory.locations l, demand d WHERE l.parent_path LIKE d.parent_path || '%'";

/// Unit struct over static SQL — every method is stateless and takes its executor from the
/// caller (the scheduler passes its batch transaction; service entry points pass the pool).
#[derive(Debug, Clone, Copy, Default)]
pub struct ProcurementRepository;

impl ProcurementRepository {
    pub fn new() -> Self {
        Self
    }

    /// `_search_rule` (§6): the highest-sequence ACTIVE pull-capable rule whose destination
    /// location COVERS the demand — the demand sits at or below the destination on the location
    /// tree (`demand.parent_path LIKE dest.parent_path || '%'`), so a rule pulling into an
    /// ancestor (WH/Stock) serves a demand in a child bin (WH/Stock/Shelf-1). The join enforces
    /// the coverage; the route must be active and visible to the company (a NULL-company rule is
    /// shared master data, ADR-0014 shared_blank).
    ///
    /// `route_ids: None` means "search every active route visible to the company" (the
    /// warehouse/orderpoint-resolved default); `Some(ids)` restricts to explicit candidates
    /// (e.g. the orderpoint's `route_id` override).
    pub async fn search_rule<'e, E>(
        executor: E,
        company_id: Uuid,
        demand_location_id: Uuid,
        route_ids: Option<Vec<Uuid>>,
    ) -> Result<Option<RuleRow>, sqlx::Error>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let row = sqlx::query(
            r#"SELECT r.id, r.name, r.sequence, r.action::text AS action, r.auto::text AS auto,
                      r.procure_method::text AS procure_method, r.delay, r.location_src_id,
                      r.location_dest_id, r.picking_type_id, r.route_id, r.warehouse_id,
                      r.company_id, r.propagate_cancel
               FROM inventory.route_rules r
               JOIN inventory.routes rt ON rt.id = r.route_id
                  AND rt.active
                  AND (rt.company_id IS NULL OR rt.company_id = $1)
                  AND (rt.metadata->>'deleted_at') IS NULL
               JOIN inventory.locations dest ON dest.id = r.location_dest_id
               JOIN inventory.locations demand ON demand.id = $2
                  AND (demand.metadata->>'deleted_at') IS NULL
                  AND demand.parent_path LIKE dest.parent_path || '%'
               WHERE r.active
                 AND r.action IN ('pull', 'pull_push')
                 AND (r.company_id IS NULL OR r.company_id = $1)
                 AND (r.metadata->>'deleted_at') IS NULL
                 AND ($3::uuid[] IS NULL OR r.route_id = ANY($3))
               ORDER BY r.sequence DESC, r.name
               LIMIT 1"#,
        )
        .bind(company_id)
        .bind(demand_location_id)
        .bind(route_ids)
        .fetch_optional(executor)
        .await?;
        Ok(row.map(rule_row))
    }

    /// The R11 reference companies: `(operation-type company, warehouse company)` for a rule.
    /// Both nullable — shared operation types / no warehouse on the rule. The warehouse's
    /// company is the company owning its org-tree node: warehouses key on `org_unit_id`
    /// (a company or branch node), so the reference walks up to the nearest company-kind
    /// node — the id every legacy `company_id` reference already points at, since the org
    /// spine backfilled companies as nodes preserving their ids.
    pub async fn rule_company_refs<'e, E>(
        executor: E,
        picking_type_id: Uuid,
        warehouse_id: Option<Uuid>,
    ) -> Result<(Option<Uuid>, Option<Uuid>), sqlx::Error>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let row = sqlx::query(
            r#"SELECT (SELECT company_id FROM inventory.operation_types
                       WHERE id = $1 AND (metadata->>'deleted_at') IS NULL) AS pt_company,
                      (SELECT cw.id
                         FROM inventory.warehouses wh
                         JOIN LATERAL (
                             WITH RECURSIVE up AS (
                                 SELECT u.id, u.parent_id, u.kind
                                 FROM organization.org_units u
                                 WHERE u.id = wh.org_unit_id
                                 UNION ALL
                                 SELECT u.id, u.parent_id, u.kind
                                 FROM organization.org_units u
                                 JOIN up ON u.id = up.parent_id
                             )
                             SELECT id FROM up WHERE kind = 'company' LIMIT 1
                         ) cw ON true
                        WHERE wh.id = $2 AND (wh.metadata->>'deleted_at') IS NULL) AS wh_company"#,
        )
        .bind(picking_type_id)
        .bind(warehouse_id)
        .fetch_one(executor)
        .await?;
        Ok((row.get("pt_company"), row.get("wh_company")))
    }

    /// The usage of a location (`location_usage` as text) — the R13-rule pre-check reads it
    /// (a rule never pushes into a view location).
    pub async fn location_usage<'e, E>(
        executor: E,
        location_id: Uuid,
    ) -> Result<Option<String>, sqlx::Error>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let row = sqlx::query(
            r#"SELECT usage::text AS usage FROM inventory.locations
               WHERE id = $1 AND (metadata->>'deleted_at') IS NULL"#,
        )
        .bind(location_id)
        .fetch_optional(executor)
        .await?;
        Ok(row.map(|r| r.get("usage")))
    }

    /// Mint a DRAFT move (`_run_pull` / `_run_push` output). State transitions are the move
    /// engine's alone — this only inserts.
    pub async fn insert_move<'e, E>(executor: E, m: &NewMoveRow) -> Result<Uuid, sqlx::Error>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let row = sqlx::query(
            r#"INSERT INTO inventory.stock_moves
                   (name, state, priority, item_id, demand_qty, quantity, price_unit,
                    procure_method, origin, location_id, location_dest_id, company_id,
                    rule_id, warehouse_id, orderpoint_id, move_orig_ids, move_dest_ids,
                    is_inventory, scrapped, propagate_cancel)
               VALUES ($1, 'draft', 'normal', $2, $3, 0, 0,
                       $4::procure_method, $5, $6, $7, $8,
                       $9, $10, $11, $12, $13, FALSE, FALSE, $14)
               RETURNING id"#,
        )
        .bind(&m.name)
        .bind(m.item_id)
        .bind(m.demand_qty)
        .bind(&m.procure_method)
        .bind(&m.origin)
        .bind(m.location_id)
        .bind(m.location_dest_id)
        .bind(m.company_id)
        .bind(m.rule_id)
        .bind(m.warehouse_id)
        .bind(m.orderpoint_id)
        .bind(&m.move_orig_ids)
        .bind(&m.move_dest_ids)
        .bind(m.propagate_cancel)
        .fetch_one(executor)
        .await?;
        Ok(row.get("id"))
    }

    /// Link a push-minted downstream move onto its upstream's `move_dest_ids` (the chain the
    /// waiting/done propagation walks).
    pub async fn link_move_chain<'e, E>(
        executor: E,
        upstream_move_id: Uuid,
        downstream_move_id: Uuid,
    ) -> Result<(), sqlx::Error>
    where
        E: Executor<'e, Database = Postgres>,
    {
        sqlx::query(
            r#"UPDATE inventory.stock_moves
               SET move_dest_ids = array_append(move_dest_ids, $2)
               WHERE id = $1"#,
        )
        .bind(upstream_move_id)
        .bind(downstream_move_id)
        .execute(executor)
        .await?;
        Ok(())
    }

    /// Claim auto-trigger orderpoints for the reorder task (ADR-0020 §4): `FOR UPDATE SKIP
    /// LOCKED`, and — critically for the `commit_per_batch` replay window — an orderpoint that
    /// already has an open replenishment move is excluded, so a crash mid-run re-orders at most
    /// the batch that never committed, never a second move for the same orderpoint.
    pub async fn claim_orderpoints<'e, E>(
        executor: E,
        company_id: Uuid,
        limit: i64,
    ) -> Result<Vec<OrderpointRow>, sqlx::Error>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let rows = sqlx::query(
            r#"SELECT id, name, trigger::text AS trigger, item_id, location_id, warehouse_id,
                      company_id, item_min_qty, item_max_qty, route_id, qty_to_order_manual
               FROM inventory.reordering_rules op
               WHERE op.company_id = $1
                 AND op.active AND op.trigger = 'auto'
                 AND (op.snoozed_until IS NULL OR op.snoozed_until <= CURRENT_DATE)
                 AND (op.metadata->>'deleted_at') IS NULL
                 AND NOT EXISTS (
                       SELECT 1 FROM inventory.stock_moves m
                       WHERE m.orderpoint_id = op.id
                         AND m.state::text = ANY($2)
                         AND (m.metadata->>'deleted_at') IS NULL)
               ORDER BY op.metadata->>'created_at', op.id
               LIMIT $3
               FOR UPDATE OF op SKIP LOCKED"#,
        )
        .bind(company_id)
        .bind(&["draft", "waiting", "confirmed", "partially_available", "assigned"][..])
        .bind(limit)
        .fetch_all(executor)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| OrderpointRow {
                id: r.get("id"),
                name: r.get("name"),
                trigger: r.get("trigger"),
                item_id: r.get("item_id"),
                location_id: r.get("location_id"),
                warehouse_id: r.get("warehouse_id"),
                company_id: r.get("company_id"),
                item_min_qty: r.get("item_min_qty"),
                item_max_qty: r.get("item_max_qty"),
                route_id: r.get("route_id"),
                qty_to_order_manual: r.get("qty_to_order_manual"),
            })
            .collect())
    }

    /// Claim the assign sweep's intake: confirmed and partially-available moves, `FOR UPDATE
    /// SKIP LOCKED` (two concurrent scheduler replicas take disjoint sets).
    pub async fn claim_assignable_moves<'e, E>(
        executor: E,
        company_id: Uuid,
        limit: i64,
    ) -> Result<Vec<ClaimedMoveRow>, sqlx::Error>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let rows = sqlx::query(
            r#"SELECT id, name, item_id, state::text AS state
               FROM inventory.stock_moves
               WHERE company_id = $1
                 AND state::text = ANY($2)
                 AND (metadata->>'deleted_at') IS NULL
               ORDER BY date, id
               LIMIT $3
               FOR UPDATE SKIP LOCKED"#,
        )
        .bind(company_id)
        .bind(&["confirmed", "partially_available"][..])
        .bind(limit)
        .fetch_all(executor)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| ClaimedMoveRow {
                id: r.get("id"),
                name: r.get("name"),
                item_id: r.get("item_id"),
                state: r.get("state"),
            })
            .collect())
    }

    /// The T11 aggregation inputs around the orderpoint's location subtree:
    /// available on hand (quants), open incoming remainder, open outgoing remainder.
    pub async fn forecast_components<'e, E>(
        executor: E,
        item_id: Uuid,
        company_id: Uuid,
        location_id: Uuid,
    ) -> Result<ForecastComponents, sqlx::Error>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let row = sqlx::query(&format!(
            r#"WITH demand AS (
                   SELECT parent_path FROM inventory.locations
                   WHERE id = $3 AND (metadata->>'deleted_at') IS NULL
               )
               SELECT
                 (SELECT COALESCE(SUM(quantity - reserved_quantity), 0)
                    FROM inventory.stock_quants q
                   WHERE q.item_id = $1 AND q.company_id = $2
                     AND (q.metadata->>'deleted_at') IS NULL
                     AND q.location_id IN ({subtree})) AS on_hand,
                 (SELECT COALESCE(SUM(demand_qty - quantity), 0)
                    FROM inventory.stock_moves m
                   WHERE m.item_id = $1 AND m.company_id = $2
                     AND m.state::text = ANY($4)
                     AND (m.metadata->>'deleted_at') IS NULL
                     AND m.location_dest_id IN ({subtree})) AS incoming,
                 (SELECT COALESCE(SUM(demand_qty - quantity), 0)
                    FROM inventory.stock_moves m
                   WHERE m.item_id = $1 AND m.company_id = $2
                     AND m.state::text = ANY($4)
                     AND (m.metadata->>'deleted_at') IS NULL
                     AND m.location_id IN ({subtree})) AS outgoing"#,
            subtree = SUBTREE_SQL
        ))
        .bind(item_id)
        .bind(company_id)
        .bind(location_id)
        .bind(&["waiting", "confirmed", "partially_available", "assigned"][..])
        .fetch_one(executor)
        .await?;
        Ok(ForecastComponents {
            on_hand: row.get("on_hand"),
            incoming: row.get("incoming"),
            outgoing: row.get("outgoing"),
        })
    }

    /// Write the T11 computes back onto the orderpoint row.
    pub async fn stamp_orderpoint<'e, E>(
        executor: E,
        orderpoint_id: Uuid,
        c: &OrderpointComputes,
    ) -> Result<(), sqlx::Error>
    where
        E: Executor<'e, Database = Postgres>,
    {
        sqlx::query(
            r#"UPDATE inventory.reordering_rules
               SET qty_on_hand = $2, qty_forecast = $3, qty_to_order = $4,
                   lead_days = $5, deadline_date = $6
               WHERE id = $1"#,
        )
        .bind(orderpoint_id)
        .bind(c.qty_on_hand)
        .bind(c.qty_forecast)
        .bind(c.qty_to_order)
        .bind(c.lead_days)
        .bind(c.deadline_date)
        .execute(executor)
        .await?;
        Ok(())
    }

    /// The quant vacuum tail (§7 task 3): remove zero-quantity, fully-unreserved quants with no
    /// staged count. R21 (a quant unlinks only at zero) is the predicate itself; a staged count
    /// (`inventory_quantity_set`) pins its row until the count applies.
    pub async fn housekeep_quants<'e, E>(
        executor: E,
        company_id: Uuid,
        limit: i64,
    ) -> Result<u64, sqlx::Error>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let res = sqlx::query(
            r#"DELETE FROM inventory.stock_quants
               WHERE ctid IN (
                   SELECT ctid FROM inventory.stock_quants
                   WHERE company_id = $1
                     AND quantity = 0 AND reserved_quantity = 0
                     AND inventory_quantity_set = FALSE
                     AND (metadata->>'deleted_at') IS NULL
                   LIMIT $2)"#,
        )
        .bind(company_id)
        .bind(limit)
        .execute(executor)
        .await?;
        Ok(res.rows_affected())
    }

    /// The lead-days half of T11: the matched rule's delay in days (the supplier-info leg is a
    /// later increment; when no rule matches, lead days are zero).
    pub async fn rule_delay_days<'e, E>(
        executor: E,
        rule_id: Uuid,
    ) -> Result<i32, sqlx::Error>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let row = sqlx::query(
            r#"SELECT delay FROM inventory.route_rules
               WHERE id = $1 AND (metadata->>'deleted_at') IS NULL"#,
        )
        .bind(rule_id)
        .fetch_optional(executor)
        .await?;
        Ok(row.map(|r| r.get::<i32, _>("delay")).unwrap_or(0))
    }

    /// The operation type a rule fulfills through (its `picking_type_id`) — the vocabulary the
    /// picking-assignment step derives a rule-launched move's grouping transfer from. `None`
    /// when the rule does not exist or is soft-deleted.
    pub async fn rule_picking_type<'e, E>(
        executor: E,
        rule_id: Uuid,
    ) -> Result<Option<Uuid>, sqlx::Error>
    where
        E: Executor<'e, Database = Postgres>,
    {
        let row = sqlx::query_scalar::<_, Uuid>(
            r#"SELECT picking_type_id FROM inventory.route_rules
               WHERE id = $1 AND (metadata->>'deleted_at') IS NULL"#,
        )
        .bind(rule_id)
        .fetch_optional(executor)
        .await?;
        Ok(row)
    }

    /// Bind `app.company_id` on a connection from the ambient scope (set by the scheduler's
    /// `with_company_scope` wrapper — the write-verb pattern: the RLS fence is what scopes a
    /// non-bypassing role; the explicit predicates above are defense-in-depth).
    pub async fn bind_company(conn: &mut PgConnection) -> Result<(), sqlx::Error> {
        backbone_orm::company_scope::bind_current_company(conn).await
    }

    /// Insert a new route (procurement configuration).
    pub async fn insert_route<'e, E>(executor: E, id: Uuid, name: &str, active: bool, sequence: i32, company_id: Option<Uuid>) -> Result<(), sqlx::Error>
    where
        E: Executor<'e, Database = Postgres>,
    {
        sqlx::query(
            r#"INSERT INTO inventory.routes (id, name, active, sequence, company_id)
               VALUES ($1, $2, $3, $4, $5)"#,
        )
        .bind(id)
        .bind(name)
        .bind(active)
        .bind(sequence)
        .bind(company_id)
        .execute(executor)
        .await?;
        Ok(())
    }

    /// Insert a new route rule after service-layer pre-checks.
    pub async fn insert_rule<'e, E>(
        executor: E,
        id: Uuid,
        name: &str,
        sequence: i32,
        action: &str,
        auto: &str,
        procure_method: &str,
        delay: i32,
        location_src_id: Option<Uuid>,
        location_dest_id: Uuid,
        picking_type_id: Uuid,
        route_id: Uuid,
        warehouse_id: Option<Uuid>,
        company_id: Option<Uuid>,
        propagate_cancel: bool,
    ) -> Result<(), sqlx::Error>
    where
        E: Executor<'e, Database = Postgres>,
    {
        sqlx::query(
            r#"INSERT INTO inventory.route_rules
                   (id, name, active, sequence, action, auto, procure_method, delay,
                    location_src_id, location_dest_id, picking_type_id, route_id,
                    warehouse_id, company_id, propagate_cancel)
               VALUES ($1, $2, TRUE, $3, $4::rule_action, $5::rule_auto,
                       $6::procure_method, $7, $8, $9, $10, $11, $12, $13, $14)"#,
        )
        .bind(id)
        .bind(name)
        .bind(sequence)
        .bind(action)
        .bind(auto)
        .bind(procure_method)
        .bind(delay)
        .bind(location_src_id)
        .bind(location_dest_id)
        .bind(picking_type_id)
        .bind(route_id)
        .bind(warehouse_id)
        .bind(company_id)
        .bind(propagate_cancel)
        .execute(executor)
        .await?;
        Ok(())
    }

    /// Insert a new orderpoint (reordering rule) after service-layer duplicate check.
    pub async fn insert_orderpoint<'e, E>(
        executor: E,
        id: Uuid,
        name: &str,
        trigger: &str,
        item_id: Uuid,
        location_id: Uuid,
        warehouse_id: Uuid,
        company_id: Uuid,
        item_min_qty: Decimal,
        item_max_qty: Decimal,
        route_id: Option<Uuid>,
    ) -> Result<(), sqlx::Error>
    where
        E: Executor<'e, Database = Postgres>,
    {
        sqlx::query(
            r#"INSERT INTO inventory.reordering_rules
                   (id, name, trigger, active, item_id, location_id, warehouse_id, company_id,
                    item_min_qty, item_max_qty, route_id)
               VALUES ($1, $2, $3::orderpoint_trigger, TRUE, $4, $5, $6, $7, $8, $9, $10)"#,
        )
        .bind(id)
        .bind(name)
        .bind(trigger)
        .bind(item_id)
        .bind(location_id)
        .bind(warehouse_id)
        .bind(company_id)
        .bind(item_min_qty)
        .bind(item_max_qty)
        .bind(route_id)
        .execute(executor)
        .await?;
        Ok(())
    }

    /// Check for existing orderpoint covering (item, location, company).
    pub async fn orderpoint_exists<'e, E>(
        executor: E,
        item_id: Uuid,
        location_id: Uuid,
        company_id: Uuid,
    ) -> Result<i64, sqlx::Error>
    where
        E: Executor<'e, Database = Postgres>,
    {
        sqlx::query_scalar::<_, i64>(
            r#"SELECT COUNT(*) FROM inventory.reordering_rules
               WHERE item_id = $1 AND location_id = $2 AND company_id = $3
                 AND (metadata->>'deleted_at') IS NULL"#,
        )
        .bind(item_id)
        .bind(location_id)
        .bind(company_id)
        .fetch_one(executor)
        .await
    }
}
