//! Procurement service (hand-authored, user-owned): `stock.route` / `stock.rule` /
//! `stock.warehouse.orderpoint` — the pull/push procurement configuration and the reorder
//! rung the daily scheduler drives (spec: docs/odoo/inventory/stock/stock-business-logic.md
//! §6 routes/rules, §7 orderpoints + scheduler, §11 R6/R11, §12 T11).
//!
//! What lives here and what deliberately does not:
//!
//! - **Rule selection** (`search_rule`) — highest-sequence active pull-capable rule for a
//!   demand location (§6). Feeds every move this service mints.
//! - **`run_pull` / `run_push`** (Odoo `_run_pull` / `_run_push`) — mint DRAFT moves only.
//!   Confirm/assign/done are the stock-move engine's verbs; the minted row enters the same
//!   lifecycle every other move does. `run_push` is the entry the move engine calls on
//!   `_action_confirm` to propagate a chain step (§1).
//! - **Orderpoint computes** (T11) — on hand / forecast / to-order / lead days, written back
//!   as stored computes on the orderpoint row. `qty_to_order = max(forecast, item_max_qty) -
//!   forecast`, floored at zero; a positive `qty_to_order_manual` overrides the computed figure
//!   for the order the scheduler mints.
//! - **Company consistency (R11)** — the SERVICE half of an `enforcement: both` pair: rule
//!   company == operation-type company == warehouse company whenever both sides are set. The
//!   DB half is the trigger in migrations/20260826090000_rule_company_consistency.up.sql; a
//!   raw-SQL writer hits the trigger, a service caller gets the typed error instead.
//! - **Orderpoint uniqueness (R6)** — the SERVICE half over the unique index
//!   `(item_id, location_id, company_id)`; the index is the DB backstop.
//!
//! This file holds no SQL — every statement lives in
//! `infrastructure/persistence/procurement_repository.rs` (the module's 4-layer rule).

use async_trait::async_trait;
use rust_decimal::Decimal;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use crate::domain::entity::MoveState;
use crate::infrastructure::persistence::procurement_repository::{
    ForecastComponents, NewMoveRow, OrderpointComputes, OrderpointRow, ProcurementRepository,
    RuleRow,
};

use super::inventory_events::{InventoryEvent, InventoryEventSink, OrderpointTriggered};

// --- errors -------------------------------------------------------------------

#[derive(Debug)]
pub enum ProcurementError {
    /// R11: the rule's company disagrees with its operation type's or warehouse's company.
    RuleCompanyMismatch { rule: Option<Uuid>, picking_type: Option<Uuid>, warehouse: Option<Uuid> },
    /// R13-rule: a rule must not push into a `view` location.
    RuleDestIsView { location_id: Uuid },
    /// R6: an orderpoint already exists for this (item, location, company).
    OrderpointExists { item_id: Uuid, location_id: Uuid },
    /// No active pull rule matches the demand (route selection found nothing).
    NoRuleForDemand { item_id: Uuid, location_id: Uuid },
    /// The referenced row does not exist.
    NotFound(Uuid),
    Db(sqlx::Error),
}

impl ProcurementError {
    pub fn code(&self) -> String {
        // Codes are the stable error_codes declared in schema/hooks/stock.hook.yaml.
        match self {
            ProcurementError::RuleCompanyMismatch { .. } => "rule_company_mismatch".into(),
            ProcurementError::RuleDestIsView { .. } => "rule_dest_is_view".into(),
            ProcurementError::OrderpointExists { .. } => "orderpoint_exists".into(),
            ProcurementError::NoRuleForDemand { .. } => "no_rule_for_demand".into(),
            ProcurementError::NotFound(_) => "not_found".into(),
            ProcurementError::Db(_) => "internal_error".into(),
        }
    }
    pub fn http_status(&self) -> u16 {
        match self {
            ProcurementError::NotFound(_) => 404,
            ProcurementError::Db(_) => 500,
            _ => 422,
        }
    }
}

impl std::fmt::Display for ProcurementError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProcurementError::RuleCompanyMismatch { .. } => write!(f, "rule_company_mismatch: rule, operation type and warehouse must belong to one company (R11)"),
            ProcurementError::RuleDestIsView { .. } => write!(f, "rule_dest_is_view: a rule destination cannot be a view location (R13)"),
            ProcurementError::OrderpointExists { item_id, location_id } => write!(f, "orderpoint_exists: a reordering rule already covers item {item_id} at location {location_id} (R6)"),
            ProcurementError::NoRuleForDemand { item_id, location_id } => write!(f, "no_rule_for_demand: no active pull rule covers item {item_id} demanded at {location_id}"),
            ProcurementError::NotFound(id) => write!(f, "not_found: {id}"),
            ProcurementError::Db(e) => write!(f, "internal_error: {e}"),
        }
    }
}
impl std::error::Error for ProcurementError {}
impl From<sqlx::Error> for ProcurementError {
    fn from(e: sqlx::Error) -> Self {
        ProcurementError::Db(e)
    }
}

/// A move-lifecycle verb failure crossing the [`MovePipeline`] boundary. Deliberately a flat
/// `{code, message}` — the implementing engine's own error taxonomy stays on its side of the
/// port; the scheduler only logs and continues with the next claimed move.
#[derive(Debug, Clone)]
pub struct MovePipelineError {
    pub code: String,
    pub message: String,
}

impl std::fmt::Display for MovePipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
impl std::error::Error for MovePipelineError {}

/// The stock-move engine's lifecycle verbs, seen from the scheduler (the port the engine
/// implements — the same fail-closed shape as the module's other deferred ports: there is no
/// silent no-op implementation, a job cannot run without a real engine behind it).
///
/// `confirm` is `_action_confirm` (draft → waiting/confirmed, which itself fires `run_push`
/// for transparent push rules); `assign` is `_action_assign` (confirmed/partially_available →
/// partially_available/assigned — the reservation apex write). Both receive the caller's
/// connection: the scheduler drives them inside its per-batch transaction, so a batch's
/// transitions commit or roll back together.
#[async_trait]
pub trait MovePipeline: Send + Sync {
    async fn confirm(
        &self,
        conn: &mut sqlx::PgConnection,
        company_id: Uuid,
        move_id: Uuid,
    ) -> Result<MoveState, MovePipelineError>;

    async fn assign(
        &self,
        conn: &mut sqlx::PgConnection,
        company_id: Uuid,
        move_id: Uuid,
    ) -> Result<MoveState, MovePipelineError>;
}

// --- inputs -------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct NewRoute {
    pub name: String,
    pub active: bool,
    pub sequence: i32,
    pub company_id: Option<Uuid>,
}

#[derive(Debug, Clone)]
pub struct NewRouteRule {
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

#[derive(Debug, Clone)]
pub struct NewOrderpoint {
    pub name: String,
    pub trigger: String,
    pub item_id: Uuid,
    pub location_id: Uuid,
    pub warehouse_id: Uuid,
    pub company_id: Uuid,
    pub item_min_qty: Decimal,
    pub item_max_qty: Decimal,
    pub route_id: Option<Uuid>,
}

/// A demand that needs supply pulled to a location — the entry a confirm engine (sale-stock)
/// or a buying bridge calls; implemented here, consumed by later passes.
#[derive(Debug, Clone)]
pub struct ProcurementRequest {
    pub item_id: Uuid,
    pub quantity: Decimal,
    /// Where the stock is needed (the rule whose destination covers this location supplies it).
    pub location_id: Uuid,
    pub warehouse_id: Option<Uuid>,
    pub company_id: Uuid,
    /// Explicit route candidates; None = every active route visible to the company.
    pub route_ids: Option<Vec<Uuid>>,
    pub origin: String,
    /// The orderpoint this order answers (present when the scheduler minted it).
    pub orderpoint_id: Option<Uuid>,
}

/// What one reorder rung produced — the durable record behind the `OrderpointTriggered` event.
#[derive(Debug, Clone)]
pub struct ReorderOutcome {
    pub orderpoint_id: Uuid,
    pub item_id: Uuid,
    pub company_id: Uuid,
    pub qty_to_order: Decimal,
    pub forecast_qty: Decimal,
    /// The minted replenishment move (draft until the pipeline confirms it).
    pub move_id: Uuid,
}

/// Counters the scheduler reports per run.
#[derive(Debug, Clone, Copy, Default)]
pub struct AssignSweepReport {
    pub claimed: usize,
    pub assigned: usize,
    pub partially_available: usize,
    pub failed: usize,
}

// --- service ------------------------------------------------------------------

#[derive(Clone)]
pub struct ProcurementService {
    db_pool: PgPool,
    sink: Arc<dyn InventoryEventSink>,
}

impl ProcurementService {
    pub fn new(db_pool: PgPool) -> Self {
        Self::with_sink(db_pool, Arc::new(super::inventory_events::LoggingSink))
    }

    pub fn with_sink(db_pool: PgPool, sink: Arc<dyn InventoryEventSink>) -> Self {
        Self { db_pool, sink }
    }

    // -- routes ---------------------------------------------------------------

    /// Create a route (an ordered rule collection). Routes are shared_blank master data: a
    /// NULL company is the shared/global posture (ADR-0014), e.g. the seeded MTO route.
    pub async fn create_route(&self, r: NewRoute) -> Result<Uuid, ProcurementError> {
        let id = Uuid::new_v4();
        ProcurementRepository::insert_route(&self.db_pool, id, &r.name, r.active, r.sequence, r.company_id).await?;
        Ok(id)
    }

    // -- rules ----------------------------------------------------------------

    /// Create a procurement rule after the R11 service pre-check (rule company == operation
    /// type company == warehouse company, whenever both sides are set) and the R13-rule
    /// destination check. The DB trigger is the backstop for raw writers; this check exists so
    /// a service caller gets the typed error instead of a constraint 500 (ADR-0015 `both`).
    pub async fn create_rule(&self, r: NewRouteRule) -> Result<Uuid, ProcurementError> {
        self.check_rule_consistency(
            r.picking_type_id,
            r.warehouse_id,
            r.company_id,
            r.location_dest_id,
        )
        .await?;

        let id = Uuid::new_v4();
        ProcurementRepository::insert_rule(
            &self.db_pool,
            id,
            &r.name,
            r.sequence,
            &r.action,
            &r.auto,
            &r.procure_method,
            r.delay,
            r.location_src_id,
            r.location_dest_id,
            r.picking_type_id,
            r.route_id,
            r.warehouse_id,
            r.company_id,
            r.propagate_cancel,
        )
        .await?;
        Ok(id)
    }

    /// The R11 + R13-rule service pre-check. Shared by every rule write path here.
    async fn check_rule_consistency(
        &self,
        picking_type_id: Uuid,
        warehouse_id: Option<Uuid>,
        rule_company: Option<Uuid>,
        location_dest_id: Uuid,
    ) -> Result<(), ProcurementError> {
        let (pt_company, wh_company) =
            ProcurementRepository::rule_company_refs(&self.db_pool, picking_type_id, warehouse_id)
                .await?;
        if pt_company.is_none() {
            return Err(ProcurementError::NotFound(picking_type_id));
        }
        // R11: every company that IS set must agree. A NULL rule company is the shared
        // posture and agrees with anything; a set one must match both set counterparts.
        let mismatch = |rc: Option<Uuid>| {
            rc.is_some_and(|c| (pt_company.is_some_and(|p| p != c)) || (wh_company.is_some_and(|w| w != c)))
        };
        if mismatch(rule_company) {
            return Err(ProcurementError::RuleCompanyMismatch {
                rule: rule_company,
                picking_type: pt_company,
                warehouse: wh_company,
            });
        }
        // When the rule itself carries no company, its two references must still agree with
        // each other (a rule cannot bridge two companies).
        if let (Some(p), Some(w)) = (pt_company, wh_company) {
            if p != w {
                return Err(ProcurementError::RuleCompanyMismatch {
                    rule: rule_company,
                    picking_type: pt_company,
                    warehouse: wh_company,
                });
            }
        }
        // R13-rule: never route INTO a view location.
        if let Some(usage) = ProcurementRepository::location_usage(&self.db_pool, location_dest_id).await? {
            if usage == "view" {
                return Err(ProcurementError::RuleDestIsView { location_id: location_dest_id });
            }
        }
        Ok(())
    }

    // -- rule selection + move minting (§6) ------------------------------------

    /// `_search_rule`: the highest-sequence active pull-capable rule covering the demand
    /// location. Public — the confirm engine and later bridges select rules through the same
    /// ordering the scheduler does.
    pub async fn search_rule(
        &self,
        company_id: Uuid,
        demand_location_id: Uuid,
        route_ids: Option<Vec<Uuid>>,
    ) -> Result<Option<RuleRow>, ProcurementError> {
        Ok(ProcurementRepository::search_rule(&self.db_pool, company_id, demand_location_id, route_ids).await?)
    }

    /// `_run_pull`: mint the inbound DRAFT move a selected pull rule prescribes — FROM the
    /// rule's source (or the operation type's default source when the rule names none) TO the
    /// rule's destination, at the requested quantity. The move enters the lifecycle as a draft;
    /// the caller (scheduler or confirm engine) drives it through the pipeline.
    pub async fn run_pull(
        &self,
        conn: &mut sqlx::PgConnection,
        rule: &RuleRow,
        req: &ProcurementRequest,
    ) -> Result<Uuid, ProcurementError> {
        let src = match rule.location_src_id {
            Some(s) => s,
            None => {
                sqlx::query_scalar::<_, Uuid>(
                    r#"SELECT default_location_src_id FROM inventory.operation_types
                       WHERE id = $1 AND (metadata->>'deleted_at') IS NULL"#,
                )
                .bind(rule.picking_type_id)
                .fetch_one(&mut *conn)
                .await?
            }
        };
        let row = NewMoveRow {
            name: format!("PULL/{}/{}", rule.name, &Uuid::new_v4().simple().to_string()[..8]),
            item_id: req.item_id,
            demand_qty: req.quantity,
            procure_method: rule.procure_method.clone(),
            origin: req.origin.clone(),
            location_id: src,
            location_dest_id: rule.location_dest_id,
            company_id: req.company_id,
            rule_id: Some(rule.id),
            warehouse_id: rule.warehouse_id.or(req.warehouse_id),
            orderpoint_id: req.orderpoint_id,
            move_orig_ids: Vec::new(),
            move_dest_ids: Vec::new(),
            propagate_cancel: rule.propagate_cancel,
        };
        Ok(ProcurementRepository::insert_move(&mut *conn, &row).await?)
    }

    /// `_run_push`: propagate a confirmed move FROM the rule's source — mint the chained
    /// downstream move (rule source → the upstream's destination) and link it onto the
    /// upstream's `move_dest_ids`. Called by the move engine on `_action_confirm` for
    /// `transparent` push rules; `manual` rules wait for a human.
    pub async fn run_push(
        &self,
        conn: &mut sqlx::PgConnection,
        rule: &RuleRow,
        upstream_move: &crate::domain::entity::StockMove,
    ) -> Result<Uuid, ProcurementError> {
        let src = rule.location_src_id.unwrap_or(upstream_move.location_id);
        let row = NewMoveRow {
            name: format!("PUSH/{}/{}", rule.name, &Uuid::new_v4().simple().to_string()[..8]),
            item_id: upstream_move.item_id,
            demand_qty: upstream_move.demand_qty - upstream_move.quantity,
            procure_method: rule.procure_method.clone(),
            origin: upstream_move.name.clone(),
            location_id: src,
            location_dest_id: upstream_move.location_dest_id,
            company_id: upstream_move.company_id,
            rule_id: Some(rule.id),
            warehouse_id: rule.warehouse_id.or(upstream_move.warehouse_id),
            orderpoint_id: upstream_move.orderpoint_id,
            move_orig_ids: vec![upstream_move.id],
            move_dest_ids: Vec::new(),
            propagate_cancel: rule.propagate_cancel,
        };
        let downstream = ProcurementRepository::insert_move(&mut *conn, &row).await?;
        ProcurementRepository::link_move_chain(&mut *conn, upstream_move.id, downstream).await?;
        Ok(downstream)
    }

    /// The procurement-group entry point: resolve the rule for a demand and mint the pull
    /// move. This is the surface a sale-stock confirm engine calls per storable line and a
    /// purchase bridge (`_run_buy`) attaches to — implemented here, consumed by later passes.
    pub async fn run_procurement(
        &self,
        conn: &mut sqlx::PgConnection,
        req: &ProcurementRequest,
    ) -> Result<Uuid, ProcurementError> {
        let rule = self
            .search_rule(req.company_id, req.location_id, req.route_ids.clone())
            .await?
            .ok_or(ProcurementError::NoRuleForDemand {
                item_id: req.item_id,
                location_id: req.location_id,
            })?;
        self.run_pull(conn, &rule, req).await
    }

    // -- orderpoints (§7, T11) --------------------------------------------------

    /// Create an orderpoint (min/max replenishment rule) — the service half of R6: a duplicate
    /// (item, location, company) coverage is the typed `orderpoint_exists` error; the partial
    /// unique index is the DB backstop a raw writer hits instead.
    pub async fn create_orderpoint(&self, o: NewOrderpoint) -> Result<Uuid, ProcurementError> {
        let existing = ProcurementRepository::orderpoint_exists(&self.db_pool, o.item_id, o.location_id, o.company_id).await?;
        if existing > 0 {
            return Err(ProcurementError::OrderpointExists { item_id: o.item_id, location_id: o.location_id });
        }

        let id = Uuid::new_v4();
        let inserted = ProcurementRepository::insert_orderpoint(
            &self.db_pool,
            id,
            &o.name,
            &o.trigger,
            o.item_id,
            o.location_id,
            o.warehouse_id,
            o.company_id,
            o.item_min_qty,
            o.item_max_qty,
            o.route_id,
        )
        .await;
        if let Err(e) = inserted {
            if e.as_database_error().map(|d| d.is_unique_violation()).unwrap_or(false) {
                return Err(ProcurementError::OrderpointExists {
                    item_id: o.item_id,
                    location_id: o.location_id,
                });
            }
            return Err(e.into());
        }
        Ok(id)
    }

    /// The T11 computes for one orderpoint, from the forecast aggregation:
    /// `qty_forecast = on_hand + incoming − outgoing` and
    /// `qty_to_order = max(qty_forecast, item_max_qty) − qty_forecast` (floored at zero).
    /// Lead days are the matched rule's delay; the deadline is `today + lead_days`.
    pub async fn compute_orderpoint(
        &self,
        conn: &mut sqlx::PgConnection,
        op: &OrderpointRow,
    ) -> Result<OrderpointComputes, ProcurementError> {
        let f: ForecastComponents =
            ProcurementRepository::forecast_components(&mut *conn, op.item_id, op.company_id, op.location_id)
                .await?;
        let forecast = f.on_hand + f.incoming - f.outgoing;
        let to_order = (op.item_max_qty - forecast).max(Decimal::ZERO);

        let rule = ProcurementRepository::search_rule(
            &mut *conn,
            op.company_id,
            op.location_id,
            op.route_id.map(|r| vec![r]),
        )
        .await?;
        let delay = match &rule {
            Some(r) => r.delay,
            None => 0,
        };
        let lead_days = Decimal::from(delay);
        let deadline = chrono::Utc::now().date_naive()
            + chrono::Duration::days(i64::from(delay));
        Ok(OrderpointComputes {
            qty_on_hand: f.on_hand,
            qty_forecast: forecast,
            qty_to_order: to_order,
            lead_days,
            deadline_date: Some(deadline),
        })
    }

    /// Recompute and persist one orderpoint's stored computes (the read-model refresh a
    /// replenishment view drives for `manual` trigger rules — humans order, the computes
    /// recommend).
    pub async fn recompute_orderpoint(&self, orderpoint_id: Uuid) -> Result<OrderpointComputes, ProcurementError> {
        let mut tx = self.db_pool.begin().await?;
        let op = sqlx::query_as::<_, OrderpointRowDb>(
            r#"SELECT id, name, trigger::text AS trigger, item_id, location_id, warehouse_id,
                      company_id, item_min_qty, item_max_qty, route_id, qty_to_order_manual
               FROM inventory.reordering_rules
               WHERE id = $1 AND (metadata->>'deleted_at') IS NULL"#,
        )
        .bind(orderpoint_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(ProcurementError::NotFound(orderpoint_id))?;
        let op = op.into_row();
        let computes = self.compute_orderpoint(&mut tx, &op).await?;
        ProcurementRepository::stamp_orderpoint(&mut *tx, orderpoint_id, &computes).await?;
        tx.commit().await?;
        Ok(computes)
    }

    /// The reorder rung (`_run_orderpoints`, §7 task 1) for ONE claimed orderpoint: refresh
    /// the T11 computes, and when `qty_forecast < item_min_qty`, mint the replenishment move of
    /// `qty_to_order` (a positive `qty_to_order_manual` overrides the computed figure) and
    /// publish `OrderpointTriggered`. Returns the outcome when the rule fired.
    ///
    /// Runs inside the caller's transaction — the scheduler commits per batch and emits the
    /// event after that commit (an event never precedes its durable record).
    pub async fn reorder_one(
        &self,
        conn: &mut sqlx::PgConnection,
        op: &OrderpointRow,
    ) -> Result<Option<ReorderOutcome>, ProcurementError> {
        let computes = self.compute_orderpoint(&mut *conn, op).await?;
        ProcurementRepository::stamp_orderpoint(&mut *conn, op.id, &computes).await?;
        if computes.qty_forecast >= op.item_min_qty {
            return Ok(None);
        }
        let rule = ProcurementRepository::search_rule(
            &mut *conn,
            op.company_id,
            op.location_id,
            op.route_id.map(|r| vec![r]),
        )
        .await?
        .ok_or(ProcurementError::NoRuleForDemand {
            item_id: op.item_id,
            location_id: op.location_id,
        })?;
        let qty = if op.qty_to_order_manual > Decimal::ZERO {
            op.qty_to_order_manual
        } else {
            computes.qty_to_order
        };
        let req = ProcurementRequest {
            item_id: op.item_id,
            quantity: qty,
            location_id: op.location_id,
            warehouse_id: Some(op.warehouse_id),
            company_id: op.company_id,
            route_ids: op.route_id.map(|r| vec![r]),
            origin: format!("orderpoint:{}", op.name),
            orderpoint_id: Some(op.id),
        };
        let move_id = self.run_pull(&mut *conn, &rule, &req).await?;
        Ok(Some(ReorderOutcome {
            orderpoint_id: op.id,
            item_id: op.item_id,
            company_id: op.company_id,
            qty_to_order: qty,
            forecast_qty: computes.qty_forecast,
            move_id,
        }))
    }

    /// Publish the reorder event (after the batch commit). Kept separate from
    /// [`Self::reorder_one`] so the transaction boundary stays explicit.
    pub fn emit_orderpoint_triggered(&self, o: &ReorderOutcome) {
        self.sink.publish(InventoryEvent::OrderpointTriggered(OrderpointTriggered {
            orderpoint_id: o.orderpoint_id,
            company_id: o.company_id,
            item_id: o.item_id,
            qty_to_order: o.qty_to_order,
            forecast_qty: o.forecast_qty,
        }));
    }

    // -- assign sweep (§7 task 2) ----------------------------------------------

    /// `_run_scheduler_assign` for ONE claimed move: hand it to the move engine's assign verb
    /// on the caller's connection. The sweep (claiming, ordering, accounting) lives in the
    /// scheduler job; this is the per-move step so the engine contract is exercisable in
    /// isolation.
    pub async fn assign_one(
        &self,
        conn: &mut sqlx::PgConnection,
        pipeline: &dyn MovePipeline,
        company_id: Uuid,
        move_id: Uuid,
    ) -> Result<MoveState, MovePipelineError> {
        pipeline.assign(conn, company_id, move_id).await
    }

    /// The quant vacuum tail (§7 task 3) — see the repository for the R21-shaped predicate.
    /// Returns the raw driver error: this is a pure delegation with no domain decision, and
    /// the scheduler (its only caller) reports infrastructure failures as-is.
    pub async fn housekeep_quants(
        &self,
        conn: &mut sqlx::PgConnection,
        company_id: Uuid,
        limit: i64,
    ) -> Result<u64, sqlx::Error> {
        ProcurementRepository::housekeep_quants(conn, company_id, limit).await
    }

    pub fn sink(&self) -> Arc<dyn InventoryEventSink> {
        self.sink.clone()
    }
}

/// Query-shaped orderpoint row (private) — maps straight onto [`OrderpointRow`].
#[derive(sqlx::FromRow)]
struct OrderpointRowDb {
    id: Uuid,
    name: String,
    r#trigger: String,
    item_id: Uuid,
    location_id: Uuid,
    warehouse_id: Uuid,
    company_id: Uuid,
    item_min_qty: Decimal,
    item_max_qty: Decimal,
    route_id: Option<Uuid>,
    qty_to_order_manual: Decimal,
}

impl OrderpointRowDb {
    fn into_row(self) -> OrderpointRow {
        OrderpointRow {
            id: self.id,
            name: self.name,
            trigger: self.r#trigger,
            item_id: self.item_id,
            location_id: self.location_id,
            warehouse_id: self.warehouse_id,
            company_id: self.company_id,
            item_min_qty: self.item_min_qty,
            item_max_qty: self.item_max_qty,
            route_id: self.route_id,
            qty_to_order_manual: self.qty_to_order_manual,
        }
    }
}
