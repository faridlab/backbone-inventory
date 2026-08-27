//! Routes / rules / orderpoints + the daily scheduler (stock-business-logic §6, §7, §11 R6/R11,
//! §12 T11; ADR-0020 scheduler postures). Requires DATABASE_URL — defaults to the module's test
//! database inside the metaphora dev postgres container; every test seeds its own company so the
//! cases run concurrently against one database without colliding.
//!
//! Proven here, per flag:
//! - **R11 (enforcement: both)** — the service pre-check returns the typed `rule_company_mismatch`
//!   error AND a raw INSERT (a writer that skips the service) hits the DB trigger.
//! - **R13-rule** — a rule destination that is a `view` location is rejected (both halves).
//! - **R6** — duplicate (item, location, company) orderpoint coverage: typed service error +
//!   unique-index backstop.
//! - **SS6 rule selection** — `_search_rule` ordering (highest sequence), visibility (inactive
//!   route / wrong company excluded, NULL-company shared rule visible), explicit route filter.
//! - **T11 computes** — forecast = on hand + incoming − outgoing over the location subtree,
//!   to-order = max − forecast floored at zero, lead days from the matched rule's delay.
//! - **SS7 scheduler** — the three ordered tasks: reorder mints the draft replenishment move +
//!   publishes `OrderpointTriggered` after commit, the open-move predicate makes a replay
//!   idempotent, the assign sweep drives the MovePipeline on its claimed moves, and the quant
//!   vacuum removes only zero/unreserved/count-free rows.
//! - **Procurement hook surface** — `run_procurement` (rule selection → `_run_pull`) and
//!   `_run_push` chain minting, ready for the later sale-stock / purchase passes.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use rust_decimal::Decimal;
use sqlx::{PgConnection, PgPool, Row};
use uuid::Uuid;

use backbone_inventory::application::service::inventory_events::{
    InventoryEvent, InventoryEventSink,
};
use backbone_inventory::application::service::inventory_gl::{
    AccountingPostEnvelope, GlPostAck, GlPostRejected, GlPostSink,
};
use backbone_inventory::application::service::inventory_move_engine::MoveGlDirective;
use backbone_inventory::application::service::procurement_service::{
    MovePipeline, MovePipelineError, NewOrderpoint, NewRoute, NewRouteRule, ProcurementRequest,
    ProcurementService,
};
use backbone_inventory::application::service::inventory_write_service::InventoryWriteService;
use backbone_inventory::domain::entity::{GlPostingState, MoveState, Priority, ProcureMethod, StockMove};
use backbone_inventory::infrastructure::jobs::{run_scheduler_with, SchedulerBatching};
use backbone_inventory::infrastructure::persistence::procurement_repository::OrderpointRow;

fn d(s: &str) -> Decimal {
    Decimal::from_str_exact(s).unwrap()
}
fn uq(p: &str) -> String {
    format!("{p}-{}", &Uuid::new_v4().simple().to_string()[..8])
}
async fn pool() -> PgPool {
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgresql://sherpa:27bb6a7f7a46ad66a0fbee9277dcb286c33fbdc6f317ff7a@localhost:5432/backbone_inventory_test".to_string()
    });
    PgPool::connect(&url).await.expect("connect DB")
}

// --- fixtures -------------------------------------------------------------------

/// A location whose `parent_path` chains from its real id (the subtree queries walk
/// `parent_path LIKE parent || '%'`, so a child's path must literally extend its parent's).
async fn loc(
    conn: &mut PgConnection,
    name: &str,
    usage: &str,
    parent_path: Option<String>,
    company: Uuid,
) -> Uuid {
    let id = Uuid::new_v4();
    let path = match &parent_path {
        Some(p) => format!("{p}{id}/"),
        None => format!("{id}/"),
    };
    sqlx::query(
        r#"INSERT INTO inventory.locations (id, name, complete_name, usage, parent_path, company_id)
           VALUES ($1, $2, $2, $3::location_usage, $4, $5)"#,
    )
    .bind(id)
    .bind(name)
    .bind(usage)
    .bind(path)
    .bind(company)
    .execute(&mut *conn)
    .await
    .unwrap();
    id
}

async fn warehouse(conn: &mut PgConnection, company: Uuid) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO inventory.warehouses (id, company_id, code, name) VALUES ($1, $2, $3, $4)")
        .bind(id)
        .bind(company)
        .bind(uq("WH"))
        .bind(uq("Warehouse"))
        .execute(&mut *conn)
        .await
        .unwrap();
    id
}

async fn op_type(
    conn: &mut PgConnection,
    company: Uuid,
    src: Uuid,
    dest: Uuid,
) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO inventory.operation_types
               (id, name, sequence_code, code, company_id,
                default_location_src_id, default_location_dest_id)
           VALUES ($1, $2, $3, 'incoming'::picking_code, $4, $5, $6)"#,
    )
    .bind(id)
    .bind(uq("PT"))
    .bind(uq("SEQ"))
    .bind(company)
    .bind(src)
    .bind(dest)
    .execute(&mut *conn)
    .await
    .unwrap();
    id
}

/// A raw move row (bypasses every service path) — used to build forecast inputs and to seed the
/// assign sweep's intake.
#[allow(clippy::too_many_arguments)]
async fn raw_move(
    conn: &mut PgConnection,
    state: &str,
    item: Uuid,
    src: Uuid,
    dest: Uuid,
    demand: Decimal,
    quantity: Decimal,
    company: Uuid,
) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO inventory.stock_moves
               (id, name, state, item_id, demand_qty, quantity, procure_method,
                location_id, location_dest_id, company_id, move_orig_ids, move_dest_ids)
           VALUES ($1, $2, $3::move_state, $4, $5, $6, 'make_to_stock', $7, $8, $9,
                   '{}'::uuid[], '{}'::uuid[])"#,
    )
    .bind(id)
    .bind(uq("MV"))
    .bind(state)
    .bind(item)
    .bind(demand)
    .bind(quantity)
    .bind(src)
    .bind(dest)
    .bind(company)
    .execute(&mut *conn)
    .await
    .unwrap();
    id
}

async fn quant(
    conn: &mut PgConnection,
    item: Uuid,
    location: Uuid,
    quantity: Decimal,
    reserved: Decimal,
    company: Uuid,
) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO inventory.stock_quants (id, item_id, location_id, quantity, reserved_quantity, company_id)
           VALUES ($1, $2, $3, $4, $5, $6)"#,
    )
    .bind(id)
    .bind(item)
    .bind(location)
    .bind(quantity)
    .bind(reserved)
    .bind(company)
    .execute(&mut *conn)
    .await
    .unwrap();
    id
}

fn rule(name: &str, seq: i32, dest: Uuid, pt: Uuid, route: Uuid, company: Option<Uuid>, delay: i32) -> NewRouteRule {
    NewRouteRule {
        name: name.into(),
        sequence: seq,
        action: "pull".into(),
        auto: "manual".into(),
        procure_method: "make_to_stock".into(),
        delay,
        location_src_id: None,
        location_dest_id: dest,
        picking_type_id: pt,
        route_id: route,
        warehouse_id: None,
        company_id: company,
        propagate_cancel: false,
    }
}

// --- doubles --------------------------------------------------------------------

/// Captures `OrderpointTriggered` publications so the scheduler test can assert the event rides
/// WITH (never ahead of) the committed replenishment move.
#[derive(Default)]
struct CapturingSink {
    events: Mutex<Vec<(Uuid, Decimal, Decimal)>>,
}
impl InventoryEventSink for CapturingSink {
    fn publish(&self, event: InventoryEvent) {
        if let InventoryEvent::OrderpointTriggered(e) = event {
            self.events.lock().unwrap().push((e.orderpoint_id, e.qty_to_order, e.forecast_qty));
        }
    }
}

/// Test double for the move engine's lifecycle verbs: the assign sweep hands it a claimed move on
/// the batch connection and it performs a real (minimal) state write, proving the sweep's claims
/// reach the pipeline inside the caller's transaction.
struct StubPipeline;
#[async_trait]
impl MovePipeline for StubPipeline {
    async fn confirm(
        &self,
        conn: &mut PgConnection,
        company_id: Uuid,
        move_id: Uuid,
    ) -> Result<MoveState, MovePipelineError> {
        self.drive(conn, company_id, move_id).await
    }
    async fn assign(
        &self,
        conn: &mut PgConnection,
        company_id: Uuid,
        move_id: Uuid,
    ) -> Result<MoveState, MovePipelineError> {
        self.drive(conn, company_id, move_id).await
    }
}
impl StubPipeline {
    async fn drive(
        &self,
        conn: &mut PgConnection,
        company_id: Uuid,
        move_id: Uuid,
    ) -> Result<MoveState, MovePipelineError> {
        sqlx::query("UPDATE inventory.stock_moves SET state = 'assigned' WHERE id = $1 AND company_id = $2")
            .bind(move_id)
            .bind(company_id)
            .execute(&mut *conn)
            .await
            .map_err(|e| MovePipelineError { code: "stub_db".into(), message: e.to_string() })?;
        Ok(MoveState::Assigned)
    }
}

// --- R11 / R13-rule: enforcement both --------------------------------------------

// The service half returns the typed error; the DB half (trigger) catches a raw writer. Both are
// exercised against the same fixture so the pair cannot drift.
#[tokio::test]
async fn r11_rule_company_consistency_service_and_trigger() {
    let pool = pool().await;
    let mut conn = pool.acquire().await.unwrap();
    let (co_a, co_b) = (Uuid::new_v4(), Uuid::new_v4());
    let src = loc(&mut conn, "SUP", "supplier", None, co_a).await;
    let dst = loc(&mut conn, "STK", "internal", None, co_a).await;
    let _wh_a = warehouse(&mut conn, co_a).await;
    let pt = op_type(&mut conn, co_a, src, dst).await;
    drop(conn);

    let svc = ProcurementService::new(pool.clone());
    let route = svc
        .create_route(NewRoute { name: uq("RT"), active: true, sequence: 10, company_id: Some(co_a) })
        .await
        .unwrap();

    // Service half: a rule whose company differs from the operation type's → typed error.
    let mut bad = rule("r11-bad", 10, dst, pt, route, Some(co_b), 0);
    let err = svc.create_rule(bad.clone()).await.unwrap_err();
    assert_eq!(err.code(), "rule_company_mismatch", "service pre-check (R11)");
    // The rejected rule must not exist.
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM inventory.route_rules WHERE name = 'r11-bad'")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(count, 0);

    // DB half: the same row inserted raw hits the trigger.
    let raw = sqlx::query(
        r#"INSERT INTO inventory.route_rules
               (name, sequence, action, auto, procure_method, delay,
                location_dest_id, picking_type_id, route_id, company_id)
           VALUES ('r11-raw', 10, 'pull'::rule_action, 'manual'::rule_auto,
                   'make_to_stock'::procure_method, 0, $1, $2, $3, $4)"#,
    )
    .bind(dst)
    .bind(pt)
    .bind(route)
    .bind(co_b)
    .execute(&pool)
    .await;
    match raw {
        Err(e) => assert!(
            e.to_string().contains("rule_company_mismatch"),
            "DB trigger must raise rule_company_mismatch, got: {e}"
        ),
        Ok(_) => panic!("raw insert of a company-mismatched rule must hit the R11 trigger"),
    }

    // Positive: rule company == operation type company == warehouse company passes.
    bad.company_id = Some(co_a);
    bad.name = "r11-good".into();
    svc.create_rule(bad).await.expect("company-consistent rule is accepted");

    // A company-less (shared) rule may not bridge two companies: operation type of company B
    // against a warehouse of company A is still a mismatch.
    let wh_b_of_a = Uuid::new_v4();
    sqlx::query("INSERT INTO inventory.warehouses (id, company_id, code, name) VALUES ($1, $2, $3, $4)")
        .bind(wh_b_of_a)
        .bind(co_a) // warehouse belongs to A
        .bind(uq("WHX"))
        .bind(uq("Warehouse X"))
        .execute(&pool)
        .await
        .unwrap();
    let pt_b = {
        let mut c = pool.acquire().await.unwrap();
        op_type(&mut c, co_b, src, dst).await
    };
    let bridging = NewRouteRule {
        name: "r11-bridge".into(),
        company_id: None, // shared posture — but its references disagree with each other
        warehouse_id: Some(wh_b_of_a),
        picking_type_id: pt_b,
        ..rule("r11-bridge", 10, dst, pt_b, route, None, 0)
    };
    let err = svc.create_rule(bridging).await.unwrap_err();
    assert_eq!(err.code(), "rule_company_mismatch", "a shared rule cannot bridge two companies");

    // R13-rule: destination must not be a view location (service half).
    let mut conn = pool.acquire().await.unwrap();
    let view = loc(&mut conn, "VIE", "view", None, co_a).await;
    drop(conn);
    let err = svc.create_rule(rule("r13-view", 10, view, pt, route, Some(co_a), 0)).await.unwrap_err();
    assert_eq!(err.code(), "rule_dest_is_view", "service pre-check (R13-rule)");
    // R13-rule: DB half.
    let raw = sqlx::query(
        r#"INSERT INTO inventory.route_rules
               (name, sequence, action, auto, procure_method, delay,
                location_dest_id, picking_type_id, route_id, company_id)
           VALUES ('r13-raw', 10, 'pull'::rule_action, 'manual'::rule_auto,
                   'make_to_stock'::procure_method, 0, $1, $2, $3, $4)"#,
    )
    .bind(view)
    .bind(pt)
    .bind(route)
    .bind(co_a)
    .execute(&pool)
    .await;
    match raw {
        Err(e) => assert!(
            e.to_string().contains("rule_dest_is_view"),
            "DB trigger must raise rule_dest_is_view, got: {e}"
        ),
        Ok(_) => panic!("raw insert of a view-destination rule must hit the R13-rule trigger"),
    }
}

// --- R6: orderpoint uniqueness, both halves ---------------------------------------

#[tokio::test]
async fn r6_orderpoint_unique_per_item_location_company() {
    let pool = pool().await;
    let mut conn = pool.acquire().await.unwrap();
    let (co_a, co_b) = (Uuid::new_v4(), Uuid::new_v4());
    let stock = loc(&mut conn, "STK", "internal", None, co_a).await;
    let wh_a = warehouse(&mut conn, co_a).await;
    let wh_b = warehouse(&mut conn, co_b).await;
    drop(conn);
    let (item, item2) = (Uuid::new_v4(), Uuid::new_v4());

    let svc = ProcurementService::new(pool.clone());
    let new_op = |item: Uuid, location: Uuid, company: Uuid, wh: Uuid| NewOrderpoint {
        name: uq("OP"),
        trigger: "auto".into(),
        item_id: item,
        location_id: location,
        warehouse_id: wh,
        company_id: company,
        item_min_qty: d("5"),
        item_max_qty: d("12"),
        route_id: None,
    };
    svc.create_orderpoint(new_op(item, stock, co_a, wh_a)).await.unwrap();

    // Service half: same (item, location, company) → typed error.
    let err = svc.create_orderpoint(new_op(item, stock, co_a, wh_a)).await.unwrap_err();
    assert_eq!(err.code(), "orderpoint_exists", "service pre-check (R6)");

    // The unique key is the triple: a different company or item may open its own orderpoint.
    svc.create_orderpoint(new_op(item, stock, co_b, wh_b)).await.expect("another company may cover the same item/location");
    svc.create_orderpoint(new_op(item2, stock, co_a, wh_a)).await.expect("another item may share the location");

    // DB half: a raw duplicate insert hits the partial unique index.
    let raw = sqlx::query(
        r#"INSERT INTO inventory.reordering_rules
               (name, trigger, item_id, location_id, warehouse_id, company_id,
                item_min_qty, item_max_qty)
           VALUES ('r6-raw', 'auto'::orderpoint_trigger, $1, $2, $3, $4, 1, 2)"#,
    )
    .bind(item)
    .bind(stock)
    .bind(wh_a)
    .bind(co_a)
    .execute(&pool)
    .await;
    match raw {
        Err(e) => assert!(
            e.as_database_error().map(|db| db.is_unique_violation()).unwrap_or(false),
            "raw duplicate must hit the R6 unique index, got: {e}"
        ),
        Ok(_) => panic!("raw duplicate orderpoint must hit the R6 unique index"),
    }
}

// --- SS6: _search_rule ordering + visibility ---------------------------------------

#[tokio::test]
async fn ss6_search_rule_highest_sequence_and_visibility() {
    let pool = pool().await;
    let mut conn = pool.acquire().await.unwrap();
    let (co_a, co_b) = (Uuid::new_v4(), Uuid::new_v4());
    let parent = loc(&mut conn, "PAR", "internal", None, co_a).await;
    let demand = loc(&mut conn, "DEM", "internal", Some(format!("{parent}/")), co_a).await;
    let other = loc(&mut conn, "OTH", "internal", None, co_a).await;
    let pt = op_type(&mut conn, co_a, other, parent).await;
    let pt_b = op_type(&mut conn, co_b, other, parent).await;
    drop(conn);

    let svc = ProcurementService::new(pool.clone());
    let r_route = |active: bool, company: Option<Uuid>| NewRoute {
        name: uq("RT"), active, sequence: 10, company_id: company,
    };
    let route1 = svc.create_route(r_route(true, Some(co_a))).await.unwrap();
    let route2 = svc.create_route(r_route(true, Some(co_a))).await.unwrap();
    let dead_route = svc.create_route(r_route(false, Some(co_a))).await.unwrap();
    let shared_route = svc.create_route(r_route(true, None)).await.unwrap();
    let foreign_route = svc.create_route(r_route(true, Some(co_b))).await.unwrap();

    // seq 10 → demand location itself; seq 20 → an ANCESTOR of the demand (still covers it).
    let r10 = svc.create_rule(rule("seq10", 10, demand, pt, route1, Some(co_a), 0)).await.unwrap();
    let r20 = svc.create_rule(rule("seq20", 20, parent, pt, route2, Some(co_a), 0)).await.unwrap();
    // Highest sequence of all, but its route is inactive → never selected.
    svc.create_rule(rule("seq99", 99, parent, pt, dead_route, Some(co_a), 0)).await.unwrap();
    // A NULL-company rule on a shared route → visible to every company.
    let r05 = svc.create_rule(rule("seq05", 5, parent, pt, shared_route, None, 0)).await.unwrap();
    // A rule of another company → invisible to A (its own company sees it).
    let r30 = svc.create_rule(rule("seq30", 30, parent, pt_b, foreign_route, Some(co_b), 0)).await.unwrap();
    // A rule whose destination does not cover the demand location → never selected.
    svc.create_rule(rule("seq40-unrelated", 40, other, pt, route1, Some(co_a), 0)).await.unwrap();

    // Company A: seq 20 wins over seq 10 and the shared seq 5 (99 is on an inactive route, 30
    // and 40 are invisible/irrelevant).
    let found = svc.search_rule(co_a, demand, None).await.unwrap().expect("a rule covers the demand");
    assert_eq!(found.id, r20, "highest-sequence covering rule wins");
    assert_eq!(found.sequence, 20);

    // Explicit route filter restricts the candidate set.
    let found = svc.search_rule(co_a, demand, Some(vec![route1])).await.unwrap().expect("route1 has a covering rule");
    assert_eq!(found.id, r10, "route filter pins the selection to that route's rules");

    // Company B: its own seq 30 beats the shared seq 5; company A's rules are invisible.
    let found = svc.search_rule(co_b, demand, None).await.unwrap().expect("shared + own rules cover the demand");
    assert_eq!(found.id, r30, "own-company rule outranks the shared one");

    // A company with only company-A rules around still sees the shared NULL-company rule.
    let co_c = Uuid::new_v4();
    let found = svc.search_rule(co_c, demand, None).await.unwrap().expect("shared rule is visible to any company");
    assert_eq!(found.id, r05, "the NULL-company shared rule is the shared fallback");
}

// --- T11: forecast / to-order computes ---------------------------------------------

// on_hand 5 (a quant at a CHILD location — the subtree must count it) + incoming 8 − outgoing 3
// = forecast 10; to_order = max(20, 10) − 10 = 10; lead days 3 from the matched rule's delay.
#[tokio::test]
async fn t11_forecast_and_to_order_computes() {
    let pool = pool().await;
    let mut conn = pool.acquire().await.unwrap();
    let co = Uuid::new_v4();
    let stock = loc(&mut conn, "STK", "internal", None, co).await;
    let child = loc(&mut conn, "SUB", "internal", Some(format!("{stock}/")), co).await;
    let sup = loc(&mut conn, "SUP", "supplier", None, co).await;
    let cust = loc(&mut conn, "CUS", "customer", None, co).await;
    let wh = warehouse(&mut conn, co).await;
    let pt = op_type(&mut conn, co, sup, stock).await;
    let item = Uuid::new_v4();

    // On hand 5: quantity 7, reserved 2 — parked at the CHILD so the subtree aggregation is what
    // sees it from the orderpoint's monitored location.
    quant(&mut conn, item, child, d("7"), d("2"), co).await;
    // Incoming 8: confirmed move supplying stock (demand 10, done-so-far 2).
    raw_move(&mut conn, "confirmed", item, sup, stock, d("10"), d("2"), co).await;
    // Outgoing 3: confirmed move shipping out of stock (demand 4, done-so-far 1).
    raw_move(&mut conn, "confirmed", item, stock, cust, d("4"), d("1"), co).await;
    // A draft outgoing of 100 must NOT enter the forecast.
    raw_move(&mut conn, "draft", item, stock, cust, d("100"), d("0"), co).await;
    drop(conn);

    let svc = ProcurementService::new(pool.clone());
    let route = svc
        .create_route(NewRoute { name: uq("RT"), active: true, sequence: 10, company_id: Some(co) })
        .await
        .unwrap();
    svc.create_rule(rule("t11", 10, stock, pt, route, Some(co), 3)).await.unwrap(); // delay 3

    let op = svc
        .create_orderpoint(NewOrderpoint {
            name: uq("OP"),
            trigger: "auto".into(),
            item_id: item,
            location_id: stock,
            warehouse_id: wh,
            company_id: co,
            item_min_qty: d("12"), // forecast 10 < 12 → the reorder rung would fire
            item_max_qty: d("20"),
            route_id: Some(route),
        })
        .await
        .unwrap();

    let computes = svc.recompute_orderpoint(op).await.unwrap();
    assert_eq!(computes.qty_on_hand, d("5"), "on hand nets out reservations, over the subtree");
    assert_eq!(computes.qty_forecast, d("10"), "on hand 5 + incoming 8 - outgoing 3");
    assert_eq!(computes.qty_to_order, d("10"), "to order = max(20, forecast 10) - 10, floored at zero");
    assert_eq!(computes.lead_days, d("3"), "lead days come from the matched rule's delay");
    let expected_deadline = chrono::Utc::now().date_naive() + chrono::Duration::days(3);
    assert_eq!(computes.deadline_date, Some(expected_deadline), "deadline = today + lead days");

    // The computes are persisted as the orderpoint's stored read model.
    let row = sqlx::query(
        "SELECT qty_on_hand, qty_forecast, qty_to_order, lead_days FROM inventory.reordering_rules WHERE id = $1",
    )
    .bind(op)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.get::<Decimal, _>("qty_forecast"), d("10"));
    assert_eq!(row.get::<Decimal, _>("qty_to_order"), d("10"));
    assert_eq!(row.get::<Decimal, _>("lead_days"), d("3"));
}

// --- SS7: the one daily scheduler ---------------------------------------------------

// Reorder mints the draft replenishment move + publishes the event; the open-move predicate makes
// a replay idempotent; the assign sweep drives the pipeline on claimed moves; the quant vacuum
// removes only zero/unreserved/count-free rows.
#[tokio::test]
async fn ss7_scheduler_reorder_assign_housekeep() {
    let pool = pool().await;
    let mut conn = pool.acquire().await.unwrap();
    let co = Uuid::new_v4();
    let stock = loc(&mut conn, "STK", "internal", None, co).await;
    let sup = loc(&mut conn, "SUP", "supplier", None, co).await;
    let wh = warehouse(&mut conn, co).await;
    let pt = op_type(&mut conn, co, sup, stock).await;
    drop(conn);

    let svc = ProcurementService::new(pool.clone());
    let route = svc
        .create_route(NewRoute { name: uq("RT"), active: true, sequence: 10, company_id: Some(co) })
        .await
        .unwrap();
    svc.create_rule(NewRouteRule {
        location_src_id: Some(sup),
        delay: 2,
        ..rule("sched", 10, stock, pt, route, Some(co), 0)
    })
    .await
    .unwrap();

    let new_op = |item: Uuid| NewOrderpoint {
        name: uq("OP"),
        trigger: "auto".into(),
        item_id: item,
        location_id: stock,
        warehouse_id: wh,
        company_id: co,
        item_min_qty: d("5"),
        item_max_qty: d("12"),
        route_id: Some(route),
    };
    let item1 = Uuid::new_v4(); // fires: empty stock, computed to-order
    let item2 = Uuid::new_v4(); // fires: manual override qty
    let item3 = Uuid::new_v4(); // no fire: forecast above min
    let op1 = svc.create_orderpoint(new_op(item1)).await.unwrap();
    let op2 = svc.create_orderpoint(new_op(item2)).await.unwrap();
    svc.create_orderpoint(new_op(item3)).await.unwrap();
    sqlx::query("UPDATE inventory.reordering_rules SET qty_to_order_manual = 4 WHERE id = $1")
        .bind(op2)
        .execute(&pool)
        .await
        .unwrap();

    let mut conn = pool.acquire().await.unwrap();
    // item3 sits above its minimum (6 on hand >= min 5).
    let keep3 = quant(&mut conn, item3, stock, d("6"), d("0"), co).await;
    // Assign sweep intake: a confirmed move nothing else will touch.
    let confirmed_move = raw_move(&mut conn, "confirmed", Uuid::new_v4(), sup, stock, d("3"), d("0"), co).await;
    // Housekeep candidates: exactly one removable (zero, unreserved, no staged count).
    let removable = quant(&mut conn, Uuid::new_v4(), stock, d("0"), d("0"), co).await;
    let staged = quant(&mut conn, Uuid::new_v4(), stock, d("0"), d("0"), co).await; // pinned by a count
    sqlx::query("UPDATE inventory.stock_quants SET inventory_quantity_set = TRUE WHERE id = $1")
        .bind(staged)
        .execute(&mut *conn)
        .await
        .unwrap();
    let reserved_zero = quant(&mut conn, Uuid::new_v4(), stock, d("0"), d("1"), co).await; // reserved pins it
    drop(conn);

    let sink = Arc::new(CapturingSink::default());
    let svc = ProcurementService::with_sink(pool.clone(), sink.clone());
    let report = run_scheduler_with(
        &pool,
        &svc,
        Arc::new(StubPipeline),
        co,
        SchedulerBatching { batch_size: 50, max_batches: 10 },
    )
    .await
    .unwrap();

    // Task 1 — reorder.
    assert_eq!(report.orderpoints_claimed, 3, "all three auto orderpoints claimed");
    assert_eq!(report.orderpoints_triggered, 2, "only the below-minimum orderpoints fire");
    assert_eq!(report.orderpoints_skipped, 1, "the above-minimum orderpoint skips");
    assert_eq!(report.replenishment_moves_minted, 2);

    let row = sqlx::query(
        "SELECT state::text AS state, demand_qty, orderpoint_id FROM inventory.stock_moves WHERE orderpoint_id = $1",
    )
    .bind(op1)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.get::<String, _>("state"), "draft", "reorder mints a DRAFT move — the pipeline drives its lifecycle");
    assert_eq!(row.get::<Decimal, _>("demand_qty"), d("12"), "computed to order = max(12, 0) - 0");
    assert_eq!(row.get::<Option<Uuid>, _>("orderpoint_id"), Some(op1));

    let demand2: Decimal = sqlx::query_scalar(
        "SELECT demand_qty FROM inventory.stock_moves WHERE orderpoint_id = $1",
    )
    .bind(op2)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(demand2, d("4"), "a positive qty_to_order_manual overrides the computed figure");

    // The event rode with the commit — one per fired orderpoint, carrying its figures.
    let events = sink.events.lock().unwrap().clone();
    assert_eq!(events.len(), 2, "one OrderpointTriggered per fired orderpoint");
    assert!(events.contains(&(op1, d("12"), d("0"))), "event carries op1's to-order and forecast");
    assert!(events.contains(&(op2, d("4"), d("0"))), "event carries op2's manual to-order");

    // Task 2 — assign sweep drove the pipeline on its claim.
    assert!(report.moves_assign_attempted >= 1);
    assert!(report.moves_assigned >= 1);
    let state: String = sqlx::query_scalar("SELECT state::text FROM inventory.stock_moves WHERE id = $1")
        .bind(confirmed_move)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(state, "assigned", "the sweep's claimed move reached the pipeline's assign verb");

    // Task 3 — quant vacuum removed exactly the removable row.
    assert_eq!(report.quants_vacuumed, 1, "only the zero/unreserved/count-free quant goes");
    for (id, label) in
        [(staged, "pinned by a staged count"), (reserved_zero, "pinned by a reservation"), (keep3, "nonzero on hand")]
    {
        let exists: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM inventory.stock_quants WHERE id = $1)")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert!(exists, "quant must survive the vacuum ({label} wrongly removed)");
    }
    let removed: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM inventory.stock_quants WHERE id = $1)")
        .bind(removable)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(!removed, "the zero/unreserved/count-free quant is vacuumed");

    // Replay idempotence: op1/op2 now have open replenishment moves → not claimed again. op3 has
    // no open move, is claimed, and skips again.
    let sink2 = Arc::new(CapturingSink::default());
    let svc2 = ProcurementService::with_sink(pool.clone(), sink2.clone());
    let report2 = run_scheduler_with(
        &pool,
        &svc2,
        Arc::new(StubPipeline),
        co,
        SchedulerBatching { batch_size: 50, max_batches: 10 },
    )
    .await
    .unwrap();
    assert_eq!(report2.orderpoints_claimed, 1, "an open replenishment move excludes its orderpoint");
    assert_eq!(report2.orderpoints_triggered, 0, "a replay mints nothing new");
    assert!(sink2.events.lock().unwrap().is_empty(), "no events without durable records");
}

// --- procurement hook surface (§6 run_pull / run_push / run_procurement) ------------

// run_procurement resolves the covering rule and mints the pull move; a demand no rule covers is
// the typed no_rule_for_demand error.
#[tokio::test]
async fn run_procurement_selects_rule_and_mints_pull() {
    let pool = pool().await;
    let mut conn = pool.acquire().await.unwrap();
    let co = Uuid::new_v4();
    let stock = loc(&mut conn, "STK", "internal", None, co).await;
    let sup = loc(&mut conn, "SUP", "supplier", None, co).await;
    let nowhere = loc(&mut conn, "NOW", "internal", None, co).await;
    let pt = op_type(&mut conn, co, sup, stock).await;
    drop(conn);

    let svc = ProcurementService::new(pool.clone());
    let route = svc
        .create_route(NewRoute { name: uq("RT"), active: true, sequence: 10, company_id: Some(co) })
        .await
        .unwrap();
    let rule_id = svc
        .create_rule(NewRouteRule { location_src_id: Some(sup), ..rule("hook", 10, stock, pt, route, Some(co), 0) })
        .await
        .unwrap();
    let item = Uuid::new_v4();

    let mut conn = pool.acquire().await.unwrap();
    let req = ProcurementRequest {
        item_id: item,
        quantity: d("6"),
        location_id: stock,
        warehouse_id: None,
        company_id: co,
        route_ids: None,
        origin: "sale-stock confirm".into(),
        orderpoint_id: None,
    };
    let move_id = svc.run_procurement(&mut conn, &req).await.unwrap();
    drop(conn);

    let row = sqlx::query(
        "SELECT state::text AS state, demand_qty, location_id, location_dest_id, rule_id, origin FROM inventory.stock_moves WHERE id = $1",
    )
    .bind(move_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.get::<String, _>("state"), "draft");
    assert_eq!(row.get::<Decimal, _>("demand_qty"), d("6"));
    assert_eq!(row.get::<Uuid, _>("location_id"), sup, "_run_pull sources from the rule's source location");
    assert_eq!(row.get::<Uuid, _>("location_dest_id"), stock);
    assert_eq!(row.get::<Option<Uuid>, _>("rule_id"), Some(rule_id));
    assert_eq!(row.get::<String, _>("origin"), "sale-stock confirm");

    // No rule covers a location outside every rule's destination subtree.
    let mut conn = pool.acquire().await.unwrap();
    let err = svc
        .run_procurement(&mut conn, &ProcurementRequest {
            item_id: item,
            quantity: d("1"),
            location_id: nowhere,
            warehouse_id: None,
            company_id: co,
            route_ids: None,
            origin: "test".into(),
            orderpoint_id: None,
        })
        .await
        .unwrap_err();
    assert_eq!(err.code(), "no_rule_for_demand");
}

// _run_push mints the chained downstream move (rule source → upstream destination) and links the
// pair through move_orig_ids / move_dest_ids.
#[tokio::test]
async fn run_push_mints_and_links_the_chain() {
    let pool = pool().await;
    let mut conn = pool.acquire().await.unwrap();
    let co = Uuid::new_v4();
    let stock = loc(&mut conn, "STK", "internal", None, co).await;
    let shelf = loc(&mut conn, "SHF", "internal", Some(format!("{stock}/")), co).await;
    let sup = loc(&mut conn, "SUP", "supplier", None, co).await;
    let pt = op_type(&mut conn, co, sup, stock).await;
    drop(conn);

    let svc = ProcurementService::new(pool.clone());
    let route = svc
        .create_route(NewRoute { name: uq("RT"), active: true, sequence: 10, company_id: Some(co) })
        .await
        .unwrap();
    svc.create_rule(NewRouteRule {
        action: "pull_push".into(),
        location_src_id: Some(sup),
        ..rule("push", 20, stock, pt, route, Some(co), 0)
    })
    .await
    .unwrap();
    let item = Uuid::new_v4();

    // Mint the upstream (a confirmed receipt-into-stock move, 3 of 10 done → 7 to push onward).
    let mut conn = pool.acquire().await.unwrap();
    let req = ProcurementRequest {
        item_id: item,
        quantity: d("10"),
        location_id: stock,
        warehouse_id: None,
        company_id: co,
        route_ids: None,
        origin: "chain test".into(),
        orderpoint_id: None,
    };
    let upstream_id = svc.run_procurement(&mut conn, &req).await.unwrap();
    sqlx::query("UPDATE inventory.stock_moves SET state='confirmed', quantity=3 WHERE id=$1")
        .bind(upstream_id)
        .execute(&mut *conn)
        .await
        .unwrap();

    // The engine's confirm verb calls run_push with the upstream's domain view.
    let upstream = StockMove::new(
        uq("MV"), MoveState::Confirmed, GlPostingState::NotApplicable, Priority::Normal,
        chrono::Utc::now(), chrono::Utc::now(),
        item, d("10"), d("3"), d("0"), ProcureMethod::MakeToStock,
        sup, shelf, co, vec![], vec![], false, false, true,
    );
    let upstream = StockMove { id: upstream_id, ..upstream };
    let rule_row = backbone_inventory::infrastructure::persistence::ProcurementRepository::search_rule(
        &mut *conn, co, stock, Some(vec![route]),
    )
    .await
    .unwrap()
    .expect("covering rule");
    let downstream_id = svc.run_push(&mut conn, &rule_row, &upstream).await.unwrap();
    drop(conn);

    let row = sqlx::query(
        "SELECT state::text AS state, demand_qty, location_id, location_dest_id, move_orig_ids FROM inventory.stock_moves WHERE id = $1",
    )
    .bind(downstream_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.get::<String, _>("state"), "draft");
    assert_eq!(row.get::<Decimal, _>("demand_qty"), d("7"), "push forwards the remaining demand");
    assert_eq!(row.get::<Uuid, _>("location_id"), sup, "push sources from the rule's source");
    assert_eq!(row.get::<Uuid, _>("location_dest_id"), shelf, "push lands at the upstream's destination");
    assert_eq!(row.get::<Vec<Uuid>, _>("move_orig_ids"), vec![upstream_id]);

    let linked: Vec<Uuid> = sqlx::query_scalar(
        "SELECT move_dest_ids FROM inventory.stock_moves WHERE id = $1",
    )
    .bind(upstream_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(linked, vec![downstream_id], "the upstream's move_dest_ids names its continuation");
}

// --- scheduler intake claims: posture pull + pickup locks -----------------------------

// The claim predicates themselves: manual-trigger orderpoints are never claimed by the job (they
// are the read-only replenishment view for humans), and snoozed orderpoints wait.
#[tokio::test]
async fn scheduler_claim_excludes_manual_and_snoozed_orderpoints() {
    let pool = pool().await;
    let mut conn = pool.acquire().await.unwrap();
    let co = Uuid::new_v4();
    let stock = loc(&mut conn, "STK", "internal", None, co).await;
    let wh = warehouse(&mut conn, co).await;
    drop(conn);

    let svc = ProcurementService::new(pool.clone());
    let base = |item: Uuid, trigger: &str| NewOrderpoint {
        name: uq("OP"),
        trigger: trigger.into(),
        item_id: item,
        location_id: stock,
        warehouse_id: wh,
        company_id: co,
        item_min_qty: d("5"),
        item_max_qty: d("12"),
        route_id: None,
    };
    let auto = svc.create_orderpoint(base(Uuid::new_v4(), "auto")).await.unwrap();
    let _manual = svc.create_orderpoint(base(Uuid::new_v4(), "manual")).await.unwrap();
    let snoozed = svc.create_orderpoint(base(Uuid::new_v4(), "auto")).await.unwrap();
    sqlx::query("UPDATE inventory.reordering_rules SET snoozed_until = CURRENT_DATE + 7 WHERE id = $1")
        .bind(snoozed)
        .execute(&pool)
        .await
        .unwrap();

    let mut conn = pool.acquire().await.unwrap();
    let claimed = backbone_inventory::infrastructure::persistence::ProcurementRepository::claim_orderpoints(
        &mut *conn, co, 50,
    )
    .await
    .unwrap();
    let ids: Vec<Uuid> = claimed.iter().map(|o: &OrderpointRow| o.id).collect();
    assert_eq!(ids, vec![auto], "only the active auto-trigger orderpoint is claimed");
}

// --- picking assignment: rule-launched moves join the transfer surface (§2 T1) -------------

// A no-op GL sink: these cases validate with an empty directive (no accounts → no envelope),
// so nothing posts; the double exists because `validate_picking` requires a sink.
struct NullGlSink;
#[async_trait]
impl GlPostSink for NullGlSink {
    async fn post(&self, _e: &AccountingPostEnvelope) -> Result<GlPostAck, GlPostRejected> {
        Err(GlPostRejected { code: "no_gl_expected".into(), message: "no envelope expected".into() })
    }
}

/// The sale-shape fixture: an internal stock location, the Customers-root-style demand
/// location, an operation type over the pair, and one active pull rule sourcing from stock
/// into the demand location (the make-to-stock delivery leg). Returns
/// `(company, stock location, demand location, operation type, rule, procurement service)`.
async fn sale_shape(pool: &PgPool) -> (Uuid, Uuid, Uuid, Uuid, Uuid, ProcurementService) {
    let mut conn = pool.acquire().await.unwrap();
    let co = Uuid::new_v4();
    let stock = loc(&mut conn, "STK", "internal", None, co).await;
    let cust = loc(&mut conn, "CUST", "customer", None, co).await;
    let pt = op_type(&mut conn, co, stock, cust).await;
    drop(conn);

    let svc = ProcurementService::new(pool.clone());
    let route = svc
        .create_route(NewRoute { name: uq("RT"), active: true, sequence: 10, company_id: Some(co) })
        .await
        .unwrap();
    let rule_id = svc
        .create_rule(NewRouteRule {
            location_src_id: Some(stock),
            ..rule("deliver", 10, cust, pt, route, Some(co), 0)
        })
        .await
        .unwrap();
    (co, stock, cust, pt, rule_id, svc)
}

// Moves launched for ONE source document (the origin is the procurement-group stand-in) join
// the SAME open transfer; a different document mints its own. This is the hop that carries
// rule-launched demand onto the picking surface an operator validates.
#[tokio::test]
async fn assign_picking_groups_rule_launched_moves_by_origin() {
    let pool = pool().await;
    let (co, _stock, cust, pt, _rule, svc) = sale_shape(&pool).await;
    let w = InventoryWriteService::new(pool.clone());

    let order_a = uq("SO");
    let order_b = uq("SO");
    let req = |item: Uuid, origin: String| ProcurementRequest {
        item_id: item,
        quantity: d("4"),
        location_id: cust,
        warehouse_id: None,
        company_id: co,
        route_ids: None,
        origin,
        orderpoint_id: None,
    };
    let mut c = pool.acquire().await.unwrap();
    let a1 = svc.run_procurement(&mut c, &req(Uuid::new_v4(), order_a.clone())).await.unwrap();
    let a2 = svc.run_procurement(&mut c, &req(Uuid::new_v4(), order_a.clone())).await.unwrap();
    let b1 = svc.run_procurement(&mut c, &req(Uuid::new_v4(), order_b.clone())).await.unwrap();
    drop(c);

    let first = w.assign_picking(co, a1).await.unwrap();
    assert!(first.minted, "the first move of a document mints the transfer");
    let second = w.assign_picking(co, a2).await.unwrap();
    assert_eq!(second.transfer_id, first.transfer_id);
    assert!(!second.minted, "a sibling line of the same document JOINS the open transfer");
    let other = w.assign_picking(co, b1).await.unwrap();
    assert!(other.minted, "a different document gets its own transfer");
    assert_ne!(other.transfer_id, first.transfer_id);

    let (header, moves) = w.fetch_picking(co, first.transfer_id).await.unwrap();
    assert_eq!(header.origin.as_deref(), Some(order_a.as_str()));
    assert_eq!(header.picking_type_id, pt);
    assert_eq!(moves.len(), 2, "both lines of the document sit on its transfer");
    assert_eq!(header.state, "draft", "the projection derives from the draft member moves");

    let (other_header, other_moves) = w.fetch_picking(co, other.transfer_id).await.unwrap();
    assert_eq!(other_header.origin.as_deref(), Some(order_b.as_str()));
    assert_eq!(other_moves.len(), 1);
}

// Assignment is idempotent (a move already on a transfer returns it), fail-closed on a move
// with no procurement rule (voucher-door moves keep their voucher identity), and refuses
// terminal states.
#[tokio::test]
async fn assign_picking_is_idempotent_and_fails_closed() {
    let pool = pool().await;
    let (co, stock, cust, _pt, _rule, svc) = sale_shape(&pool).await;
    let item = Uuid::new_v4();
    let mut c = pool.acquire().await.unwrap();
    let mv = svc
        .run_procurement(&mut c, &ProcurementRequest {
            item_id: item,
            quantity: d("2"),
            location_id: cust,
            warehouse_id: None,
            company_id: co,
            route_ids: None,
            origin: uq("SO"),
            orderpoint_id: None,
        })
        .await
        .unwrap();
    drop(c);
    // A hand-minted move with NO rule and a DONE move, for the fail-closed arms.
    let mut conn = pool.acquire().await.unwrap();
    let ruleless = raw_move(&mut conn, "confirmed", item, stock, cust, d("1"), d("0"), co).await;
    let finished = raw_move(&mut conn, "done", item, stock, cust, d("1"), d("1"), co).await;
    drop(conn);

    let w = InventoryWriteService::new(pool.clone());
    let first = w.assign_picking(co, mv).await.unwrap();
    let again = w.assign_picking(co, mv).await.unwrap();
    assert_eq!(again.transfer_id, first.transfer_id);
    assert!(!again.minted, "the second call joins the transfer the first minted");

    let err = w.assign_picking(co, ruleless).await.unwrap_err();
    assert_eq!(err.code(), "move_has_no_rule");
    let err = w.assign_picking(co, finished).await.unwrap_err();
    assert_eq!(err.code(), "wrong_move_state");
}

// The full rule → move → picking → validate chain: what a confirmed order's demand goes
// through, proving the warehouse's button_validate surface now reaches rule-launched moves
// and that the moves-by-origin read reconstructs the delivered quantity afterwards.
#[tokio::test]
async fn validate_picking_drives_rule_launched_demand() {
    let pool = pool().await;
    let (co, stock, cust, _pt, _rule, svc) = sale_shape(&pool).await;
    let item = Uuid::new_v4();
    let mut conn = pool.acquire().await.unwrap();
    quant(&mut conn, item, stock, d("10"), d("0"), co).await;
    drop(conn);

    let origin = uq("SO");
    let mut c = pool.acquire().await.unwrap();
    let mv = svc
        .run_procurement(&mut c, &ProcurementRequest {
            item_id: item,
            quantity: d("6"),
            location_id: cust,
            warehouse_id: None,
            company_id: co,
            route_ids: None,
            origin: origin.clone(),
            orderpoint_id: None,
        })
        .await
        .unwrap();
    drop(c);

    let w = InventoryWriteService::new(pool.clone());
    let assignment = w.assign_picking(co, mv).await.unwrap();
    assert!(assignment.minted);
    let state = w.action_confirm(mv).await.unwrap();
    assert_eq!(state, "confirmed");

    let validated = w
        .validate_picking(co, assignment.transfer_id, &MoveGlDirective::default(), &NullGlSink)
        .await
        .unwrap();
    assert_eq!(validated.projected_state, "done", "the projection derives from the done move");

    // The delivered-quantity reconstruction read: moves by origin, done state, done qty.
    let moves_repo = backbone_inventory::infrastructure::persistence::StockMoveRepository::new(pool.clone());
    let mut c = pool.acquire().await.unwrap();
    let done = moves_repo.fetch_moves_by_origin(&mut c, co, &origin).await.unwrap();
    drop(c);
    assert_eq!(done.len(), 1);
    assert_eq!(done[0].state, "done");
    assert_eq!(done[0].quantity, d("6"));

    let on_hand: Decimal = sqlx::query_scalar(
        "SELECT quantity FROM inventory.stock_quants WHERE company_id = $1 AND item_id = $2 AND location_id = $3",
    )
    .bind(co)
    .bind(item)
    .bind(stock)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(on_hand, d("4"), "the validated picking drew the physical stock");
}
