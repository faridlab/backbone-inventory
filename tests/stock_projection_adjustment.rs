//! Picking-as-projection (T1) + the quant-driven adjustment door probes. Requires
//! DATABASE_URL (default :5433/backbone_inventory), schema applied.
//!
//! **T1 (spec stock-business-logic.md §2 / §12 T1, ADR-0016):** the transfer's state is a
//! stored compute re-derived from its member move states — every assertion here READS the
//! projection (`fetch_picking`) after driving MOVE changes; no test writes a transfer
//! state, and no service under test exposes a way to.
//!
//! **The ONE adjustment door (spec §5.2 — no stock.inventory model):** counts stage on the
//! quant (`inventory_quantity` + the T4 stored diff), applying mints the `is_inventory`
//! move through the engine pipeline, the staging gate makes re-apply a no-op, a move
//! landing between count and apply is a LOUD outdated conflict, and the reconciliation
//! voucher rides the door while posting its single value-diff GL envelope exactly once.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use rust_decimal::Decimal;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use backbone_inventory::application::service::inventory_gl::{
    AccountingPostEnvelope, GlPostAck, GlPostRejected, GlPostSink,
};
use backbone_inventory::application::service::inventory_move_engine::{
    BackorderPolicy, MoveGlDirective, NewStockMove,
};
use backbone_inventory::application::service::inventory_transfer::{NewPicking, PickingLine};
use backbone_inventory::application::service::inventory_write_service::{
    InventoryError, InventoryWriteService, NewReconciliation, NewWarehouse, ReconLine,
};
use backbone_inventory::infrastructure::persistence::QuantSelector;

/// A GL sink that records every envelope it is handed (count + the last one), so the
/// exactly-once assertions can count posts without a ledger.
struct CountingSink {
    posts: AtomicUsize,
}
#[async_trait::async_trait]
impl GlPostSink for CountingSink {
    async fn post(&self, _e: &AccountingPostEnvelope) -> Result<GlPostAck, GlPostRejected> {
        self.posts.fetch_add(1, Ordering::SeqCst);
        Ok(GlPostAck { post_id: Uuid::new_v4(), journal_id: Uuid::new_v4(), idempotent_reuse: false })
    }
}
fn counting_sink() -> Arc<CountingSink> { Arc::new(CountingSink { posts: AtomicUsize::new(0) }) }

fn d(s: &str) -> Decimal { Decimal::from_str_exact(s).unwrap() }
fn uq(p: &str) -> String { format!("{p}-{}", &Uuid::new_v4().simple().to_string()[..8]) }
async fn pool() -> PgPool {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:postgres@localhost:5433/backbone_inventory".to_string());
    PgPool::connect(&url).await.expect("connect DB")
}

fn adj_gl() -> MoveGlDirective {
    MoveGlDirective {
        cogs_account_id: Some(Uuid::new_v4()),
        inventory_account_id: Some(Uuid::new_v4()),
        grir_account_id: Some(Uuid::new_v4()),
        adjustment_account_id: Some(Uuid::new_v4()),
        currency: "IDR".into(),
    }
}

async fn warehouse(w: &InventoryWriteService, company: Uuid) -> Uuid {
    w.create_warehouse(NewWarehouse {
        company_id: company, code: uq("WH"), name: uq("Main"),
        warehouse_type: None, parent_warehouse_id: None, is_group: false,
    }).await.unwrap()
}

/// Insert a location row. `warehouse_id` binds the valuation bin an internal location
/// resolves to (and the warehouse whose stock location the reconciliation door resolves to).
async fn loc(pool: &PgPool, company: Uuid, usage: &str, wh: Option<Uuid>) -> Uuid {
    let id = Uuid::new_v4();
    let name = uq("LOC");
    sqlx::query(
        r#"INSERT INTO inventory.locations
             (id, name, complete_name, usage, parent_path, company_id, warehouse_id)
           VALUES ($1,$2,$3,$4::location_usage,$5,$6,$7)"#,
    )
    .bind(id).bind(&name).bind(&name).bind(usage).bind("").bind(company).bind(wh)
    .execute(pool).await.unwrap();
    id
}

async fn op_type(pool: &PgPool, company: Uuid, code: &str, reservation: &str, src: Uuid, dst: Uuid) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO inventory.operation_types
             (id, name, sequence_code, code, company_id,
              default_location_src_id, default_location_dest_id, reservation_method, create_backorder)
           VALUES ($1,$2,$3,$4::picking_code,$5,$6,$7,$8::reservation_method,'ask'::create_backorder)"#,
    )
    .bind(id).bind(uq("PT")).bind(uq("SEQ")).bind(code).bind(company)
    .bind(src).bind(dst).bind(reservation)
    .execute(pool).await.unwrap();
    id
}

/// Seed on-hand stock at a location (the quant grain: one row, untracked dims).
async fn seed_quant(pool: &PgPool, company: Uuid, item: Uuid, location: Uuid, qty: &str) {
    sqlx::query(
        r#"INSERT INTO inventory.stock_quants
             (id, item_id, location_id, quantity, reserved_quantity, available_quantity, company_id)
           VALUES ($1,$2,$3,$4,0,$4,$5)"#,
    )
    .bind(Uuid::new_v4()).bind(item).bind(location).bind(d(qty)).bind(company)
    .execute(pool).await.unwrap();
}

/// Seed a Bin running balance (item x warehouse) for the valuation core.
async fn seed_bin(pool: &PgPool, company: Uuid, item: Uuid, wh: Uuid, qty: &str, rate: &str) {
    sqlx::query(
        r#"INSERT INTO inventory.bins
             (id, company_id, item_id, warehouse_id, actual_qty, reserved_qty, valuation_rate, stock_value)
           VALUES ($1,$2,$3,$4,$5,0,$6,$7)"#,
    )
    .bind(Uuid::new_v4()).bind(company).bind(item).bind(wh)
    .bind(d(qty)).bind(d(rate)).bind(d(qty) * d(rate))
    .execute(pool).await.unwrap();
}

async fn on_hand(pool: &PgPool, company: Uuid, item: Uuid, location: Uuid) -> Decimal {
    sqlx::query_scalar(
        r#"SELECT COALESCE(SUM(quantity),0) FROM inventory.stock_quants
           WHERE company_id=$1 AND item_id=$2 AND location_id=$3 AND (metadata->>'deleted_at') IS NULL"#,
    )
    .bind(company).bind(item).bind(location)
    .fetch_one(pool).await.unwrap()
}

async fn count_inventory_moves(pool: &PgPool, company: Uuid, item: Uuid) -> i64 {
    sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM inventory.stock_moves
           WHERE company_id=$1 AND item_id=$2 AND is_inventory"#,
    )
    .bind(company).bind(item)
    .fetch_one(pool).await.unwrap()
}

fn picking(name: &str, company: Uuid, op: Uuid, src: Uuid, dst: Uuid, lines: Vec<PickingLine>) -> NewPicking {
    NewPicking {
        name: name.into(), company_id: company, picking_type_id: op,
        location_id: src, location_dest_id: dst, partner_id: None,
        move_type: "direct".into(), origin: None, lines,
    }
}

// ── T1: the transfer is a PROJECTION of its moves ────────────────────────────

/// Mint → confirm (+assign per the operation type) → the probe reads the PROJECTED state;
/// validate (`button_validate` = `_action_done` over the moves) → the probe reads `done`.
/// The receipt-shaped GL leg posts through the engine (W1 shape preserved).
#[tokio::test]
async fn picking_projects_confirm_assign_and_done() {
    let pool = pool().await;
    let svc = InventoryWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let wh = warehouse(&svc, company).await;
    let supplier = loc(&pool, company, "supplier", None).await;
    let stock = loc(&pool, company, "internal", Some(wh)).await;
    let op = op_type(&pool, company, "incoming", "at_confirm", supplier, stock).await;
    let item = Uuid::new_v4();

    let created = svc.create_picking(picking(&uq("PICK"), company, op, supplier, stock, vec![
        PickingLine { item_id: item, demand_qty: d("10"), price_unit: d("2") },
    ])).await.unwrap();
    assert_eq!(created.move_ids.len(), 1);

    // Inbound move: assign mints the execution line unconditionally → the member move (and
    // so the projection) reads `assigned`. A READ of the stored compute, never an assertion
    // this test could have written.
    let (header, moves) = svc.fetch_picking(company, created.transfer_id).await.unwrap();
    assert_eq!(header.state, "assigned");
    assert_eq!(moves[0].state, "assigned");

    // button_validate: _action_done over the transfer's moves — the only validate verb.
    let sink = counting_sink();
    let validated = svc.validate_picking(company, created.transfer_id, &adj_gl(), &*sink).await.unwrap();
    assert_eq!(validated.projected_state, "done");
    assert_eq!(validated.validated_moves.len(), 1);
    assert!(validated.validated_moves[0].gl_posted);
    assert_eq!(validated.validated_moves[0].gl_amount, d("20")); // 10 qty × 2 rate, receipt shape

    // The physical truth: the destination quant landed the demand; the projection was never
    // a second writer of it.
    assert_eq!(on_hand(&pool, company, item, stock).await, d("10"));
    assert_eq!(sink.posts.load(Ordering::SeqCst), 1);

    // Idempotent re-validate: every member move is done — nothing re-runs, no second GL.
    let again = svc.validate_picking(company, created.transfer_id, &adj_gl(), &*sink).await.unwrap();
    assert_eq!(again.validated_moves.len(), 0);
    assert_eq!(again.projected_state, "done");
    assert_eq!(sink.posts.load(Ordering::SeqCst), 1);
}

/// R3: the picking name is unique per company — the typed duplicate error.
#[tokio::test]
async fn r3_picking_name_unique_per_company() {
    let pool = pool().await;
    let svc = InventoryWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let wh = warehouse(&svc, company).await;
    let supplier = loc(&pool, company, "supplier", None).await;
    let stock = loc(&pool, company, "internal", Some(wh)).await;
    let op = op_type(&pool, company, "incoming", "manual", supplier, stock).await;
    let name = uq("DUP");

    let line = || vec![PickingLine { item_id: Uuid::new_v4(), demand_qty: d("1"), price_unit: d("1") }];
    svc.create_picking(picking(&name, company, op, supplier, stock, line())).await.unwrap();
    let err = svc.create_picking(picking(&name, company, op, supplier, stock, line())).await.unwrap_err();
    assert!(matches!(err, InventoryError::DuplicateNumber(_)), "got {err:?}");

    // A different company may reuse the name (uniqueness is per company).
    let other = Uuid::new_v4();
    let wh2 = warehouse(&svc, other).await;
    let stock2 = loc(&pool, other, "internal", Some(wh2)).await;
    let supplier2 = loc(&pool, other, "supplier", None).await;
    let op2 = op_type(&pool, other, "incoming", "manual", supplier2, stock2).await;
    svc.create_picking(picking(&name, other, op2, supplier2, stock2, line())).await.unwrap();
}

/// Mixed member-move states aggregate to the LEAST-advanced live state (the projection
/// never says `done` while a member move is open), and a member cancel reprojects (the
/// cancelled move drops out of the aggregation; all-cancelled shows `cancel`).
#[tokio::test]
async fn projection_aggregates_and_reprojects_on_cancel() {
    let pool = pool().await;
    let svc = InventoryWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let wh = warehouse(&svc, company).await;
    let supplier = loc(&pool, company, "supplier", None).await;
    let stock = loc(&pool, company, "internal", Some(wh)).await;
    // manual reservation: the moves stay `confirmed` after mint (the aggregate under test).
    let op = op_type(&pool, company, "incoming", "manual", supplier, stock).await;
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();

    let created = svc.create_picking(picking(&uq("MIX"), company, op, supplier, stock, vec![
        PickingLine { item_id: a, demand_qty: d("2"), price_unit: d("1") },
        PickingLine { item_id: b, demand_qty: d("3"), price_unit: d("1") },
    ])).await.unwrap();
    let (header, moves) = svc.fetch_picking(company, created.transfer_id).await.unwrap();
    assert_eq!(header.state, "confirmed");
    assert!(moves.iter().all(|m| m.state == "confirmed"));

    // Cancel ONE member move through the move engine — the transfer reprojects (min-rank
    // over live moves: one cancelled, one confirmed → still `confirmed`, never `cancel`).
    svc.action_cancel(moves[0].id).await.unwrap();
    let (header, moves) = svc.fetch_picking(company, created.transfer_id).await.unwrap();
    assert_eq!(header.state, "confirmed");
    assert_eq!(moves.iter().filter(|m| m.state == "cancel").count(), 1);

    // Cancel the remaining member: all-cancelled → the projection reads `cancel`.
    svc.action_cancel(moves[1].id).await.unwrap();
    let (header, _) = svc.fetch_picking(company, created.transfer_id).await.unwrap();
    assert_eq!(header.state, "cancel");
}

/// A partial validate (less done than demanded) mints the backorder move onto the SAME
/// picking — the projection must NOT read `done` while that backorder is open.
#[tokio::test]
async fn partial_validate_keeps_projection_open_via_backorder() {
    let pool = pool().await;
    let svc = InventoryWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let wh = warehouse(&svc, company).await;
    let stock = loc(&pool, company, "internal", Some(wh)).await;
    let customer = loc(&pool, company, "customer", None).await;
    let op = op_type(&pool, company, "outgoing", "manual", stock, customer).await;
    let item = Uuid::new_v4();
    seed_quant(&pool, company, item, stock, "4").await;
    seed_bin(&pool, company, item, wh, "4", "2").await;

    let created = svc.create_picking(picking(&uq("PART"), company, op, stock, customer, vec![
        PickingLine { item_id: item, demand_qty: d("10"), price_unit: d("0") },
    ])).await.unwrap();
    // manual posture → confirm only; the move reads `confirmed` (nothing reserved yet).
    let (header, _) = svc.fetch_picking(company, created.transfer_id).await.unwrap();
    assert_eq!(header.state, "confirmed");

    let sink = counting_sink();
    let validated = svc.validate_picking(company, created.transfer_id, &adj_gl(), &*sink).await.unwrap();
    // Only 4 were on hand: the prepare pass reserved them; _action_done drew exactly that.
    assert_eq!(validated.validated_moves[0].done_qty, d("4"));
    let (header, moves) = svc.fetch_picking(company, created.transfer_id).await.unwrap();
    // One done member + the minted backorder (draft): the projection stays OPEN — the
    // least-advanced live move keeps the transfer below `done` (and below `assigned`).
    assert_ne!(header.state, "done");
    assert_ne!(header.state, "assigned");
    assert!(moves.iter().any(|m| m.state == "done"));
    assert!(moves.iter().any(|m| m.state == "draft"));
    // Stock truth: the source quant holds none of the drawn 4; the customer location got 4.
    assert_eq!(on_hand(&pool, company, item, stock).await, d("0"));
    assert_eq!(on_hand(&pool, company, item, customer).await, d("4"));
}

// ── the ONE adjustment door (spec §5.2) ──────────────────────────────────────

/// Stage → the T4 stored compute sits on the quant (counted, diff, gate); apply → the
/// `is_inventory` move is minted THROUGH the engine, the on-hand becomes the count, the
/// gate drops, and the adjustment GL leg posts once.
#[tokio::test]
async fn stage_and_apply_count_up_mints_is_inventory_move() {
    let pool = pool().await;
    let svc = InventoryWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let wh = warehouse(&svc, company).await;
    let stock = loc(&pool, company, "internal", Some(wh)).await;
    let item = Uuid::new_v4();
    seed_quant(&pool, company, item, stock, "10").await;
    seed_bin(&pool, company, item, wh, "10", "2").await;

    let staged = svc.stage_quant_count(company, item, stock, d("15")).await.unwrap();
    assert_eq!(staged.on_hand_qty, d("10"));
    assert_eq!(staged.diff_qty, d("5"));

    // The staging is ON THE QUANT (T4): counted level, stored diff, gate up.
    let row = sqlx::query(
        r#"SELECT inventory_quantity, inventory_diff_quantity, inventory_quantity_set
           FROM inventory.stock_quants WHERE id=$1"#,
    )
    .bind(staged.quant_id).fetch_one(&pool).await.unwrap();
    assert_eq!(row.get::<Decimal, _>("inventory_quantity"), d("15"));
    assert_eq!(row.get::<Decimal, _>("inventory_diff_quantity"), d("5"));
    assert!(row.get::<bool, _>("inventory_quantity_set"));

    let sink = counting_sink();
    let applied = svc.apply_inventory(
        company, QuantSelector::ItemLocation { item_id: item, location_id: stock },
        &adj_gl(), &*sink,
    ).await.unwrap();
    assert!(applied.applied);
    assert!(applied.move_id.is_some());
    assert_eq!(applied.diff_qty, d("5"));
    assert_eq!(on_hand(&pool, company, item, stock).await, d("15"));

    // The move is the ONE writer: a done is_inventory move exists; the staging is consumed.
    assert_eq!(count_inventory_moves(&pool, company, item).await, 1);
    let mv_state: String = sqlx::query_scalar(
        "SELECT state::text FROM inventory.stock_moves WHERE id=$1",
    )
    .bind(applied.move_id.unwrap()).fetch_one(&pool).await.unwrap();
    assert_eq!(mv_state, "done");
    let gate: bool = sqlx::query_scalar(
        "SELECT inventory_quantity_set FROM inventory.stock_quants WHERE id=$1",
    )
    .bind(staged.quant_id).fetch_one(&pool).await.unwrap();
    assert!(!gate);

    // Value diff at the current moving average (5 × 2) posted once through the engine's
    // is_inventory shape.
    assert_eq!(sink.posts.load(Ordering::SeqCst), 1);
}

/// A count DOWN draws from the location into the inventory-loss location — same door,
/// reversed GL shape.
#[tokio::test]
async fn apply_count_down_reverses_leg() {
    let pool = pool().await;
    let svc = InventoryWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let wh = warehouse(&svc, company).await;
    let stock = loc(&pool, company, "internal", Some(wh)).await;
    let item = Uuid::new_v4();
    seed_quant(&pool, company, item, stock, "9").await;
    seed_bin(&pool, company, item, wh, "9", "3").await;

    svc.stage_quant_count(company, item, stock, d("6")).await.unwrap();
    let sink = counting_sink();
    let applied = svc.apply_inventory(
        company, QuantSelector::ItemLocation { item_id: item, location_id: stock },
        &adj_gl(), &*sink,
    ).await.unwrap();
    assert_eq!(applied.diff_qty, d("-3"));
    assert_eq!(on_hand(&pool, company, item, stock).await, d("6"));
    assert_eq!(sink.posts.load(Ordering::SeqCst), 1);
}

/// Idempotent apply: the gate is down after an apply — a second apply is a NO-OP (no new
/// move, no new GL). Minting twice is impossible.
#[tokio::test]
async fn reapply_is_a_noop() {
    let pool = pool().await;
    let svc = InventoryWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let wh = warehouse(&svc, company).await;
    let stock = loc(&pool, company, "internal", Some(wh)).await;
    let item = Uuid::new_v4();
    seed_quant(&pool, company, item, stock, "8").await;
    seed_bin(&pool, company, item, wh, "8", "2").await;

    svc.stage_quant_count(company, item, stock, d("12")).await.unwrap();
    let sink = counting_sink();
    let selector = QuantSelector::ItemLocation { item_id: item, location_id: stock };
    let first = svc.apply_inventory(company, selector, &adj_gl(), &*sink).await.unwrap();
    assert!(first.applied);
    let second = svc.apply_inventory(company, selector, &adj_gl(), &*sink).await.unwrap();
    assert!(!second.applied);
    assert!(second.move_id.is_none());
    assert_eq!(count_inventory_moves(&pool, company, item).await, 1);
    assert_eq!(sink.posts.load(Ordering::SeqCst), 1);
    assert_eq!(on_hand(&pool, company, item, stock).await, d("12"));
}

/// Outdated count (spec `is_outdated`): a move landing between the count and the apply is
/// a LOUD conflict — the apply refuses, the staging survives for a re-count.
#[tokio::test]
async fn move_between_count_and_apply_is_a_loud_conflict() {
    let pool = pool().await;
    let svc = InventoryWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let wh = warehouse(&svc, company).await;
    let stock = loc(&pool, company, "internal", Some(wh)).await;
    let item = Uuid::new_v4();
    seed_quant(&pool, company, item, stock, "10").await;
    seed_bin(&pool, company, item, wh, "10", "2").await;

    svc.stage_quant_count(company, item, stock, d("15")).await.unwrap();
    // A move lands between the count and the apply (simulated at the quant surface — the
    // observable invariant is the on-hand moved after the count staged its diff).
    sqlx::query("UPDATE inventory.stock_quants SET quantity = quantity + 3 WHERE company_id=$1 AND item_id=$2 AND location_id=$3")
        .bind(company).bind(item).bind(stock)
        .execute(&pool).await.unwrap();

    let sink = counting_sink();
    let err = svc.apply_inventory(
        company, QuantSelector::ItemLocation { item_id: item, location_id: stock },
        &adj_gl(), &*sink,
    ).await.unwrap_err();
    assert!(matches!(err, InventoryError::OutdatedCount { .. }), "got {err:?}");
    assert_eq!(sink.posts.load(Ordering::SeqCst), 0);
    assert_eq!(count_inventory_moves(&pool, company, item).await, 0);

    // Re-staging against the new on-hand (the count is a level) applies cleanly.
    svc.stage_quant_count(company, item, stock, d("15")).await.unwrap();
    svc.apply_inventory(
        company, QuantSelector::ItemLocation { item_id: item, location_id: stock },
        &adj_gl(), &*sink,
    ).await.unwrap();
    assert_eq!(on_hand(&pool, company, item, stock).await, d("15"));
}

/// R24 (`no_count_while_reserved`): a quant holding reservations cannot be staged or
/// applied — release the reservations first.
#[tokio::test]
async fn reserved_quant_refuses_counts() {
    let pool = pool().await;
    let svc = InventoryWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let wh = warehouse(&svc, company).await;
    let stock = loc(&pool, company, "internal", Some(wh)).await;
    let item = Uuid::new_v4();
    let quant = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO inventory.stock_quants
             (id, item_id, location_id, quantity, reserved_quantity, available_quantity, company_id)
           VALUES ($1,$2,$3,10,2,8,$4)"#,
    )
    .bind(quant).bind(item).bind(stock).bind(company)
    .execute(&pool).await.unwrap();

    let err = svc.stage_quant_count(company, item, stock, d("10")).await.unwrap_err();
    assert!(matches!(err, InventoryError::CountReserved { .. }), "got {err:?}");

    // An already-staged count on a reserved quant is refused at the apply door too.
    sqlx::query(
        r#"UPDATE inventory.stock_quants SET inventory_quantity=10,
             inventory_diff_quantity=8, inventory_quantity_set=TRUE, inventory_date=CURRENT_DATE
           WHERE id=$1"#,
    )
    .bind(quant)
    .execute(&pool).await.unwrap();
    let sink = counting_sink();
    let err = svc.apply_inventory(company, QuantSelector::ById(quant), &adj_gl(), &*sink).await.unwrap_err();
    assert!(matches!(err, InventoryError::CountReserved { .. }), "got {err:?}");
}

/// The reconciliation voucher rides the door: per line the count stages on the warehouse
/// stock location's quant and applies (is_inventory moves minted by the engine), and the
/// voucher posts its SINGLE value-diff envelope exactly once (net of the up/down lines).
#[tokio::test]
async fn reconciliation_converges_onto_the_door() {
    let pool = pool().await;
    let svc = InventoryWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let wh = warehouse(&svc, company).await;
    // The warehouse's stock location (what the voucher door resolves to) carries the stock.
    let stock = loc(&pool, company, "internal", Some(wh)).await;
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    seed_quant(&pool, company, a, stock, "10").await;
    seed_bin(&pool, company, a, wh, "10", "2").await;   // 20 value
    seed_quant(&pool, company, b, stock, "6").await;
    seed_bin(&pool, company, b, wh, "6", "3").await;    // 18 value

    let sink = counting_sink();
    let id = svc.submit_reconciliation(NewReconciliation {
        recon_number: uq("REC"),
        company_id: company,
        warehouse_id: wh,
        posting_date: chrono::Utc::now().date_naive(),
        currency: "IDR".into(),
        inventory_account_id: Uuid::new_v4(),
        adjustment_account_id: Uuid::new_v4(),
        lines: vec![
            ReconLine { item_id: a, counted_qty: d("12"), counted_rate: d("0") }, // +2 → +4.00
            ReconLine { item_id: b, counted_qty: d("5"), counted_rate: d("0") },  // −1 → −3.00
        ],
    }, &*sink).await.unwrap();

    // Physical: both quants hold their counts; one is_inventory move per non-zero diff.
    assert_eq!(on_hand(&pool, company, a, stock).await, d("12"));
    assert_eq!(on_hand(&pool, company, b, stock).await, d("5"));
    assert_eq!(count_inventory_moves(&pool, company, a).await, 1);
    assert_eq!(count_inventory_moves(&pool, company, b).await, 1);

    // Voucher: recorded with the net difference (4.00 − 3.00) and its items' diffs.
    let row = sqlx::query(
        r#"SELECT net_difference, posting_state::text AS ps FROM inventory.stock_reconciliations WHERE id=$1"#,
    )
    .bind(id).fetch_one(&pool).await.unwrap();
    assert_eq!(row.get::<Decimal, _>("net_difference"), d("1.00"));
    assert_eq!(row.get::<String, _>("ps"), "posted");

    // GL exactly once: the voucher's single net envelope (the door was driven with no
    // accounts — the moves posted nothing, so no second envelope exists).
    assert_eq!(sink.posts.load(Ordering::SeqCst), 1);

    // A second apply of the same counts mints nothing (the gate is consumed per line).
    svc.stage_quant_count(company, a, stock, d("12")).await.unwrap();
    svc.apply_inventory(
        company, QuantSelector::ItemLocation { item_id: a, location_id: stock },
        &MoveGlDirective::default(), &*counting_sink(),
    ).await.unwrap();
    assert_eq!(count_inventory_moves(&pool, company, a).await, 1);
}

/// A count that matches the on-hand is a zero-diff recon: no GL envelope, `not_applicable`.
#[tokio::test]
async fn zero_diff_recon_posts_no_gl() {
    let pool = pool().await;
    let svc = InventoryWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let wh = warehouse(&svc, company).await;
    let stock = loc(&pool, company, "internal", Some(wh)).await;
    let item = Uuid::new_v4();
    seed_quant(&pool, company, item, stock, "7").await;
    seed_bin(&pool, company, item, wh, "7", "2").await;

    let sink = counting_sink();
    let id = svc.submit_reconciliation(NewReconciliation {
        recon_number: uq("REC0"),
        company_id: company,
        warehouse_id: wh,
        posting_date: chrono::Utc::now().date_naive(),
        currency: "IDR".into(),
        inventory_account_id: Uuid::new_v4(),
        adjustment_account_id: Uuid::new_v4(),
        lines: vec![ReconLine { item_id: item, counted_qty: d("7"), counted_rate: d("0") }],
    }, &*sink).await.unwrap();

    assert_eq!(on_hand(&pool, company, item, stock).await, d("7"));
    assert_eq!(count_inventory_moves(&pool, company, item).await, 0); // zero diff mints nothing
    assert_eq!(sink.posts.load(Ordering::SeqCst), 0);
    let ps: String = sqlx::query_scalar(
        "SELECT posting_state::text FROM inventory.stock_reconciliations WHERE id=$1",
    )
    .bind(id).fetch_one(&pool).await.unwrap();
    assert_eq!(ps, "not_applicable");
}

/// The door values the diff at the CURRENT moving average — an explicit counted rate is a
/// valuation-overlay concern and is refused loudly (never a silent revaluation).
#[tokio::test]
async fn counted_rate_is_refused() {
    let pool = pool().await;
    let svc = InventoryWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let wh = warehouse(&svc, company).await;
    let item = Uuid::new_v4();

    let err = svc.submit_reconciliation(NewReconciliation {
        recon_number: uq("RECR"),
        company_id: company,
        warehouse_id: wh,
        posting_date: chrono::Utc::now().date_naive(),
        currency: "IDR".into(),
        inventory_account_id: Uuid::new_v4(),
        adjustment_account_id: Uuid::new_v4(),
        lines: vec![ReconLine { item_id: item, counted_qty: d("5"), counted_rate: d("4") }],
    }, &*counting_sink()).await.unwrap_err();
    assert!(matches!(err, InventoryError::CountedRateUnsupported), "got {err:?}");
}

/// The full-lifecycle projection walk: drive ONE transfer's member moves through every
/// lifecycle transition and READ the projected transfer state after EACH transition — the
/// projection must re-derive (including DOWNWARD: an open backorder or a newly minted draft
/// member drops the derived state, a cancelled member drops out of the aggregation). No
/// assertion here writes a transfer state; each probe is a fresh `fetch_picking` read.
///
/// Walk: partial validate (done + draft backorder) → confirm → assign (backorder) → mint a
/// chained draft → confirm to `waiting` → parent done releases the waiting child → assign
/// → done (all members) → mint + cancel a stray draft.
#[tokio::test]
async fn projection_rederives_after_every_move_transition() {
    let pool = pool().await;
    let svc = InventoryWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let wh = warehouse(&svc, company).await;
    let stock = loc(&pool, company, "internal", Some(wh)).await;
    let customer = loc(&pool, company, "customer", None).await;
    let supplier = loc(&pool, company, "supplier", None).await;
    let op = op_type(&pool, company, "outgoing", "manual", stock, customer).await;
    let item = Uuid::new_v4();
    seed_quant(&pool, company, item, stock, "4").await;
    seed_bin(&pool, company, item, wh, "4", "2").await;
    let sink = counting_sink();
    let probe = {
        let svc = &svc;
        move |tid: Uuid| async move { svc.fetch_picking(company, tid).await.unwrap() }
    };

    // Mint + auto-confirm: the projection derives `confirmed` from the member move.
    let created = svc.create_picking(picking(&uq("WALK"), company, op, stock, customer, vec![
        PickingLine { item_id: item, demand_qty: d("6"), price_unit: d("0") },
    ])).await.unwrap();
    let tid = created.transfer_id;
    let (h, _) = probe(tid).await;
    assert_eq!(h.state, "confirmed", "probe after mint+confirm");

    // Partial validate (4 of 6 on hand): the member goes `done` and the backorder member is
    // minted `draft` — the projection re-derives DOWNWARD to the least-advanced live move.
    let validated = svc.validate_picking(company, tid, &adj_gl(), &*sink).await.unwrap();
    assert_eq!(validated.validated_moves[0].done_qty, d("4"));
    let (h, moves) = probe(tid).await;
    assert_eq!(h.state, "draft", "open backorder re-derives the projection down");
    let backorder = moves.iter().find(|m| m.state == "draft").expect("backorder member").id;
    assert_eq!(moves.iter().find(|m| m.state == "done").expect("done member").demand_qty, d("6"));

    // Land the remaining supply, then confirm + assign the backorder member.
    sqlx::query(
        "UPDATE inventory.stock_quants SET quantity = quantity + 2, available_quantity = available_quantity + 2 \
         WHERE company_id=$1 AND item_id=$2 AND location_id=$3",
    ).bind(company).bind(item).bind(stock).execute(&pool).await.unwrap();
    sqlx::query(
        "UPDATE inventory.bins SET actual_qty = actual_qty + 2, stock_value = stock_value + 4 \
         WHERE company_id=$1 AND item_id=$2 AND warehouse_id=$3",
    ).bind(company).bind(item).bind(wh).execute(&pool).await.unwrap();

    svc.action_confirm(backorder).await.unwrap();
    let (h, _) = probe(tid).await;
    assert_eq!(h.state, "confirmed", "probe after backorder confirm");

    let a = svc.action_assign(backorder).await.unwrap();
    assert_eq!(a.state, "assigned");
    let (h, _) = probe(tid).await;
    assert_eq!(h.state, "assigned", "probe after backorder assign");

    // A raw draft member chained onto the backorder (pick/pack shape): the mint itself
    // re-derives the projection back DOWN to `draft`.
    let mut chained = NewStockMove {
        name: uq("CHAIN"), company_id: company, item_id: item, demand_qty: d("1"),
        price_unit: Decimal::ZERO, procure_method: "make_to_stock".into(), picking_id: Some(tid),
        origin: None, location_id: supplier, location_dest_id: customer, partner_id: None,
        warehouse_id: None, orderpoint_id: None, move_orig_ids: vec![backorder],
        move_dest_ids: vec![], is_inventory: false, scrapped: false,
    };
    let child = svc.create_move(chained.clone()).await.unwrap();
    let (h, _) = probe(tid).await;
    assert_eq!(h.state, "draft", "a minted draft member re-derives the projection down");

    // Confirm the chained member: its parent is not done, so it parks at `waiting` and the
    // projection follows it there.
    let to = svc.action_confirm(child).await.unwrap();
    assert_eq!(to, "waiting");
    let (h, _) = probe(tid).await;
    assert_eq!(h.state, "waiting", "probe after the chained confirm parks at waiting");

    // Parent done: the waiting child is RELEASED to confirmed by the chain propagation and
    // the projection re-derives off the released child (the only live non-done member).
    svc.action_done(backorder, BackorderPolicy::Never, &adj_gl(), &*sink).await.unwrap();
    let (h, moves) = probe(tid).await;
    assert_eq!(h.state, "confirmed", "waiting-gate release re-derives the projection");
    assert!(moves.iter().any(|m| m.id == child && m.state == "confirmed"));

    // Assign the child (virtual supplier source — supply is unconditionally available),
    // then validate it: every member is done, the projection reads `done` and date_done lands.
    svc.action_assign(child).await.unwrap();
    let (h, _) = probe(tid).await;
    assert_eq!(h.state, "assigned", "probe after the child assign");
    svc.action_done(child, BackorderPolicy::Never, &adj_gl(), &*sink).await.unwrap();
    let (h, _) = probe(tid).await;
    assert_eq!(h.state, "done", "probe after the last member lands");
    assert!(h.date_done.is_some(), "date_done is stamped by the projection");

    // A stray minted draft drags the projection down again; cancelling it drops it out of
    // the aggregation entirely — the projection returns to `done` with zero physical writes.
    chained.move_orig_ids = vec![];
    chained.name = uq("STRAY");
    let stray = svc.create_move(chained).await.unwrap();
    let (h, _) = probe(tid).await;
    assert_eq!(h.state, "draft", "the stray draft re-derives the projection down");
    svc.action_cancel(stray).await.unwrap();
    let (h, moves) = probe(tid).await;
    assert_eq!(h.state, "done", "the cancelled member drops out of the aggregation");
    assert!(moves.iter().all(|m| m.state == "done" || m.state == "cancel"));
}
