//! Stock-move engine behavior tests: the 7-state lifecycle, quant-grain reservation (the
//! triangle: authoritative `reserved_quantity` + mirror lines + aggregate move state), the
//! two-step `_synchronize_quant` on done, the backorder split, chain propagation off the
//! `waiting` gate, and the V7 ordering proof (OUT valued before / IN valued after) through the
//! unchanged moving-average Bin/SLE core. Requires DATABASE_URL (:5433/backbone_inventory),
//! schema applied.

use rust_decimal::Decimal;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use backbone_inventory::application::service::inventory_gl::{
    AccountingPostEnvelope, GlPostAck, GlPostRejected, GlPostSink,
};
use backbone_inventory::application::service::inventory_move_engine::{
    BackorderPolicy, MoveGlDirective, NewStockMove,
};
use backbone_inventory::application::service::inventory_read::InventoryReadService;
use backbone_inventory::application::service::inventory_write_service::{
    InventoryError, InventoryWriteService, NewWarehouse,
};

struct StubGl;
#[async_trait::async_trait]
impl GlPostSink for StubGl {
    async fn post(&self, _e: &AccountingPostEnvelope) -> Result<GlPostAck, GlPostRejected> {
        Ok(GlPostAck { post_id: Uuid::new_v4(), journal_id: Uuid::new_v4(), idempotent_reuse: false })
    }
}

fn d(s: &str) -> Decimal { Decimal::from_str_exact(s).unwrap() }
fn uq(p: &str) -> String { format!("{p}-{}", &Uuid::new_v4().simple().to_string()[..8]) }
async fn pool() -> PgPool {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:postgres@localhost:5433/backbone_inventory".to_string());
    PgPool::connect(&url).await.expect("connect DB")
}
fn no_gl() -> MoveGlDirective { MoveGlDirective::default() }
fn out_gl() -> MoveGlDirective {
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
        org_unit_id: company, code: uq("WH"), name: uq("Main"),
        warehouse_type: None, parent_warehouse_id: None, is_group: false,
    }).await.unwrap()
}

/// Insert a location row (usage: supplier/view/internal/customer/...). `warehouse_id` binds the
/// valuation bin an internal location resolves to.
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

fn new_move(company: Uuid, item: Uuid, src: Uuid, dst: Uuid, qty: &str) -> NewStockMove {
    NewStockMove {
        name: uq("MV"),
        company_id: company,
        item_id: item,
        demand_qty: d(qty),
        price_unit: Decimal::ZERO,
        procure_method: "make_to_stock".into(),
        picking_id: None,
        origin: None,
        location_id: src,
        location_dest_id: dst,
        partner_id: None,
        warehouse_id: None,
        orderpoint_id: None,
        move_orig_ids: vec![],
        move_dest_ids: vec![],
        is_inventory: false,
        scrapped: false,
        forced_value: None,
    }
}

async fn move_state(pool: &PgPool, id: Uuid) -> String {
    let s: String = sqlx::query_scalar("SELECT state::text FROM inventory.stock_moves WHERE id=$1")
        .bind(id).fetch_one(pool).await.unwrap();
    s
}

async fn quant_at(pool: &PgPool, company: Uuid, item: Uuid, location: Uuid) -> (Decimal, Decimal) {
    let row = sqlx::query(
        r#"SELECT COALESCE(SUM(quantity),0) AS q, COALESCE(SUM(reserved_quantity),0) AS r
           FROM inventory.stock_quants
           WHERE company_id=$1 AND item_id=$2 AND location_id=$3 AND (metadata->>'deleted_at') IS NULL"#,
    )
    .bind(company).bind(item).bind(location)
    .fetch_one(pool).await.unwrap();
    (row.get::<Decimal,_>("q"), row.get::<Decimal,_>("r"))
}

async fn bin_at(pool: &PgPool, company: Uuid, item: Uuid, wh: Uuid) -> (Decimal, Decimal, Decimal) {
    let row = sqlx::query(
        "SELECT actual_qty, valuation_rate, stock_value FROM inventory.bins WHERE company_id=$1 AND item_id=$2 AND warehouse_id=$3",
    )
    .bind(company).bind(item).bind(wh).fetch_one(pool).await.unwrap();
    (row.get("actual_qty"), row.get("valuation_rate"), row.get("stock_value"))
}

/// MEC-1: the full lifecycle — draft → confirmed → assigned → done — with the reservation
/// triangle intact at every step and the OUT leg valued at the current average.
#[tokio::test]
async fn lifecycle_confirm_assign_done() {
    let pool = pool().await;
    let w = InventoryWriteService::new(pool.clone());
    let (company, item) = (Uuid::new_v4(), Uuid::new_v4());
    let wh = warehouse(&w, company).await;
    let stock = loc(&pool, company, "internal", Some(wh)).await;
    let customer = loc(&pool, company, "customer", None).await;
    seed_quant(&pool, company, item, stock, "10").await;
    seed_bin(&pool, company, item, wh, "10", "100").await;

    let mv = w.create_move(new_move(company, item, stock, customer, "6")).await.unwrap();
    assert_eq!(move_state(&pool, mv).await, "draft", "create lands draft — no state at insert (spec §1)");

    assert_eq!(w.action_confirm(company, mv).await.unwrap(), "confirmed");
    let a = w.action_assign(company, mv).await.unwrap();
    assert_eq!(a.state, "assigned", "10 on hand covers demand 6");
    assert_eq!(a.reserved_qty, d("6"));
    // Triangle: authoritative quant reserved=6 (mirror line minted), available = 10-6 = 4 (a READ).
    let (q, r) = quant_at(&pool, company, item, stock).await;
    assert_eq!((q, r), (d("10"), d("6")));
    let read = InventoryReadService::new(pool.clone());
    let avail = read.quant_availability(company, item, stock).await.unwrap();
    assert_eq!(avail.available_qty, d("4"), "T2: available = quantity - reserved as a read");
    assert_eq!(avail.on_hand_qty, d("10"));

    let out = w.action_done(company, mv, BackorderPolicy::Always, &out_gl(), &StubGl).await.unwrap();
    assert_eq!(out.done_qty, d("6"));
    assert!(out.backorder_move_id.is_none(), "full validate mints no backorder");
    assert_eq!(out.sle_count, 1, "OUT leg only (customer destination holds no bin)");
    assert!(out.gl_posted, "COGS shape posts through the AccountingPost seam");
    assert_eq!(out.gl_amount, d("600.00"), "6 * current average 100");
    assert_eq!(move_state(&pool, mv).await, "done");

    // Two-step sync: the reservation is gone (line flipped done → mirror self-heal), the
    // physical moved — src 4 on hand / 0 reserved, and the Bin consumed at the average.
    let (q, r) = quant_at(&pool, company, item, stock).await;
    assert_eq!((q, r), (d("4"), d("0")), "reserved step released, available step moved 6");
    let (bq, brate, bval) = bin_at(&pool, company, item, wh).await;
    assert_eq!(bq, d("4.0000"));
    assert_eq!(brate, d("100.000000"), "outflow never reblends the rate");
    assert_eq!(bval, d("400.00"));
}

/// MEC-2: competing reservations — two assigns against 10 on hand; the row lock serializes them,
/// the pair never over-reserves (R22), and the second lands `partially_available`.
#[tokio::test]
async fn competing_reservations_one_counter() {
    let pool = pool().await;
    let w = InventoryWriteService::new(pool.clone());
    let (company, item) = (Uuid::new_v4(), Uuid::new_v4());
    let wh = warehouse(&w, company).await;
    let stock = loc(&pool, company, "internal", Some(wh)).await;
    let customer = loc(&pool, company, "customer", None).await;
    seed_quant(&pool, company, item, stock, "10").await;
    seed_bin(&pool, company, item, wh, "10", "100").await;

    let a = w.create_move(new_move(company, item, stock, customer, "6")).await.unwrap();
    let b = w.create_move(new_move(company, item, stock, customer, "6")).await.unwrap();
    w.action_confirm(company, a).await.unwrap();
    w.action_confirm(company, b).await.unwrap();

    let oa = w.action_assign(company, a).await.unwrap();
    assert_eq!(oa.state, "assigned");
    let ob = w.action_assign(company, b).await.unwrap();
    assert_eq!(ob.state, "partially_available", "only 4 remain free — never an over-reserve");
    assert_eq!(ob.reserved_qty, d("4"));

    let (q, r) = quant_at(&pool, company, item, stock).await;
    assert_eq!(r, d("10"), "reserved == on-hand exactly (R22: reserved can never exceed it)");
    assert_eq!(q, d("10"));

    // Done on A: the mirror self-heal leaves B's 4 reserved on the quant.
    w.action_done(company, a, BackorderPolicy::Never, &no_gl(), &StubGl).await.unwrap();
    let (q, r) = quant_at(&pool, company, item, stock).await;
    assert_eq!((q, r), (d("4"), d("4")), "A consumed 6 physically; B's live line still holds 4");
}

/// MEC-3: partial validate splits the backorder (demand 10, done 6 → child demand 4, chained),
/// and the shrunk line's residual reservation self-heals away.
#[tokio::test]
async fn backorder_split_on_partial_done() {
    let pool = pool().await;
    let w = InventoryWriteService::new(pool.clone());
    let (company, item) = (Uuid::new_v4(), Uuid::new_v4());
    let wh = warehouse(&w, company).await;
    let stock = loc(&pool, company, "internal", Some(wh)).await;
    let customer = loc(&pool, company, "customer", None).await;
    seed_quant(&pool, company, item, stock, "10").await;
    seed_bin(&pool, company, item, wh, "10", "100").await;

    let mv = w.create_move(new_move(company, item, stock, customer, "10")).await.unwrap();
    w.action_confirm(company, mv).await.unwrap();
    w.action_assign(company, mv).await.unwrap();
    // The operator validates only 6 of the reserved 10 (the picking's done quantity).
    sqlx::query("UPDATE inventory.stock_move_lines SET quantity=6 WHERE move_id=$1")
        .bind(mv).execute(&pool).await.unwrap();

    let out = w.action_done(company, mv, BackorderPolicy::Always, &no_gl(), &StubGl).await.unwrap();
    assert_eq!(out.done_qty, d("6"));
    let child = out.backorder_move_id.expect("backorder minted");
    let (demand, state, origs): (Decimal, String, Vec<Uuid>) = sqlx::query_as(
        "SELECT demand_qty, state::text, move_orig_ids FROM inventory.stock_moves WHERE id=$1",
    )
    .bind(child).fetch_one(&pool).await.unwrap();
    assert_eq!(demand, d("4"));
    // The backorder is minted CONFIRMED (a confirmed move is what the scheduler's assign sweep
    // and a re-validate can drive; a draft one is invisible to both), and the `Always` policy
    // reserves it right away — the 4 units the partial done released are exactly its demand.
    assert_eq!(state, "assigned", "Always mints the backorder confirmed + reserved (reserve on mint)");
    assert_eq!(origs, vec![mv], "chained to its parent (done-qty propagation walks this)");

    // The residual reservation (10 reserved, line shrunk to 6) must NOT survive the done — the
    // mirror self-heal releases the stranded 4, which the `Always` backorder then re-reserves
    // (reserve on mint): the parent's leftover demand holds the units its own partial done freed.
    let (q, r) = quant_at(&pool, company, item, stock).await;
    assert_eq!((q, r), (d("4"), d("4")), "self-heal released the stranded 4; the backorder re-reserved it");

    // Policy Never leaves the remainder unbackordered. (Release the backorder's hold first —
    // under the reserve-on-mint contract it owns every free unit at the location, and the Never
    // probe below needs reservable stock.)
    w.unreserve_move(company, child).await.unwrap();
    let mv2 = w.create_move(new_move(company, item, stock, customer, "2")).await.unwrap();
    w.action_confirm(company, mv2).await.unwrap();
    w.action_assign(company, mv2).await.unwrap();
    sqlx::query("UPDATE inventory.stock_move_lines SET quantity=1 WHERE move_id=$1")
        .bind(mv2).execute(&pool).await.unwrap();
    let out2 = w.action_done(company, mv2, BackorderPolicy::Never, &no_gl(), &StubGl).await.unwrap();
    assert!(out2.backorder_move_id.is_none());
}

/// MEC-4: V7 ordering proof, cross-warehouse transfer — the OUT leg is valued at the SOURCE's
/// pre-move average BEFORE the destination reblends; value is conserved across the pair; the
/// move mints no GL (value-neutral internal move, the transfer-path contract).
#[tokio::test]
async fn v7_out_valued_before_in_after() {
    let pool = pool().await;
    let w = InventoryWriteService::new(pool.clone());
    let (company, item) = (Uuid::new_v4(), Uuid::new_v4());
    let wh1 = warehouse(&w, company).await;
    let wh2 = warehouse(&w, company).await;
    let src = loc(&pool, company, "internal", Some(wh1)).await;
    let dst = loc(&pool, company, "internal", Some(wh2)).await;
    // WH1 holds 10@100 (value 1000); WH2 holds 5@60 (value 300).
    seed_quant(&pool, company, item, src, "10").await;
    seed_bin(&pool, company, item, wh1, "10", "100").await;
    seed_quant(&pool, company, item, dst, "5").await;
    seed_bin(&pool, company, item, wh2, "5", "60").await;

    let mv = w.create_move(new_move(company, item, src, dst, "4")).await.unwrap();
    w.action_confirm(company, mv).await.unwrap();
    w.action_assign(company, mv).await.unwrap();
    let out = w.action_done(company, mv, BackorderPolicy::Never, &out_gl(), &StubGl).await.unwrap();
    assert_eq!(out.sle_count, 2, "paired OUT + IN legs");
    assert!(!out.gl_posted, "internal cross-warehouse move posts no GL (value-neutral)");

    // OUT leg: WH1 10@100 → 6@100, value 600. The OUT consumed the PRE-move average — had the
    // IN reblended first, WH2's rate would have contaminated it (V7 is exactly this ordering).
    let (q1, r1, v1) = bin_at(&pool, company, item, wh1).await;
    assert_eq!(q1, d("6.0000"));
    assert_eq!(r1, d("100.000000"));
    assert_eq!(v1, d("600.00"));
    // IN leg: WH2 5@60 + carried 400 → 9 units, value 700, blended rate 700/9.
    let (q2, r2, v2) = bin_at(&pool, company, item, wh2).await;
    assert_eq!(q2, d("9.0000"));
    assert_eq!(v2, d("700.00"), "carried value 4*100 blends into 300");
    assert_eq!(r2, (d("700.00") / d("9")).round_dp(6), "moving average reblend");
    // Conservation: 600 + 700 == 1000 + 300.
    assert_eq!(v1 + v2, d("1300.00"));
    // Quants flipped both sides.
    assert_eq!(quant_at(&pool, company, item, src).await, (d("6"), d("0")));
    assert_eq!(quant_at(&pool, company, item, dst).await, (d("9"), d("0")));
}

/// MEC-5: inbound move — supply from a non-internal source is unconditionally available: assign
/// mints the execution line without touching any source quant; done lands the dest quant, blends
/// the bin at price_unit, and posts the receipt-shape GL leg.
#[tokio::test]
async fn inbound_move_receipt_shape() {
    let pool = pool().await;
    let w = InventoryWriteService::new(pool.clone());
    let (company, item) = (Uuid::new_v4(), Uuid::new_v4());
    let wh = warehouse(&w, company).await;
    let supplier = loc(&pool, company, "supplier", None).await;
    let stock = loc(&pool, company, "internal", Some(wh)).await;

    let mut input = new_move(company, item, supplier, stock, "8");
    input.price_unit = d("25");
    let mv = w.create_move(input).await.unwrap();
    w.action_confirm(company, mv).await.unwrap();
    let a = w.action_assign(company, mv).await.unwrap();
    assert_eq!(a.state, "assigned", "incoming supply needs no reservation");
    assert_eq!(a.reserved_qty, d("8"));

    let out = w.action_done(company, mv, BackorderPolicy::Never, &out_gl(), &StubGl).await.unwrap();
    assert_eq!(out.done_qty, d("8"));
    assert_eq!(out.sle_count, 1, "IN leg only");
    assert!(out.gl_posted, "receipt shape: Dr Inventory / Cr GR/IR");
    assert_eq!(out.gl_amount, d("200.00"), "8 * 25");
    let (q, r) = quant_at(&pool, company, item, stock).await;
    assert_eq!((q, r), (d("8"), d("0")));
    let (bq, brate, bval) = bin_at(&pool, company, item, wh).await;
    assert_eq!((bq, brate, bval), (d("8.0000"), d("25.000000"), d("200.00")));
}

/// MEC-6: the waiting gate + done-qty propagation — a chained child sits `waiting` until ALL its
/// parents are done, then releases to `confirmed`.
#[tokio::test]
async fn waiting_gate_releases_when_parents_done() {
    let pool = pool().await;
    let w = InventoryWriteService::new(pool.clone());
    let (company, item) = (Uuid::new_v4(), Uuid::new_v4());
    let wh1 = warehouse(&w, company).await;
    let wh2 = warehouse(&w, company).await;
    let src = loc(&pool, company, "internal", Some(wh1)).await;
    let mid = loc(&pool, company, "internal", Some(wh2)).await;
    let customer = loc(&pool, company, "customer", None).await;
    seed_quant(&pool, company, item, src, "5").await;
    seed_bin(&pool, company, item, wh1, "5", "50").await;

    let parent = w.create_move(new_move(company, item, src, mid, "5")).await.unwrap();
    let mut child_input = new_move(company, item, mid, customer, "5");
    child_input.move_orig_ids = vec![parent];
    let child = w.create_move(child_input).await.unwrap();

    assert_eq!(w.action_confirm(company, parent).await.unwrap(), "confirmed");
    assert_eq!(w.action_confirm(company, child).await.unwrap(), "waiting", "parent not done — child waits");
    w.action_assign(company, parent).await.unwrap();
    w.action_done(company, parent, BackorderPolicy::Never, &no_gl(), &StubGl).await.unwrap();
    assert_eq!(move_state(&pool, child).await, "confirmed", "all parents done — child released");
}

/// MEC-7: cancel releases the reservation and never propagates into a done move.
#[tokio::test]
async fn cancel_releases_reservation() {
    let pool = pool().await;
    let w = InventoryWriteService::new(pool.clone());
    let (company, item) = (Uuid::new_v4(), Uuid::new_v4());
    let wh = warehouse(&w, company).await;
    let stock = loc(&pool, company, "internal", Some(wh)).await;
    let customer = loc(&pool, company, "customer", None).await;
    seed_quant(&pool, company, item, stock, "10").await;

    let mv = w.create_move(new_move(company, item, stock, customer, "6")).await.unwrap();
    w.action_confirm(company, mv).await.unwrap();
    w.action_assign(company, mv).await.unwrap();
    let released = w.action_cancel(company, mv).await.unwrap();
    assert_eq!(released, d("6"));
    assert_eq!(move_state(&pool, mv).await, "cancel");
    let (q, r) = quant_at(&pool, company, item, stock).await;
    assert_eq!((q, r), (d("10"), d("0")), "stock untouched, reservation freed");
}

/// MEC-8: the guards — R9 same-location, R23 negative demand, R24 done-needs-lines, and the
/// state machine's refusal to confirm a non-draft move.
#[tokio::test]
async fn guards_reject_bad_transitions() {
    let pool = pool().await;
    let w = InventoryWriteService::new(pool.clone());
    let (company, item) = (Uuid::new_v4(), Uuid::new_v4());
    let wh = warehouse(&w, company).await;
    let stock = loc(&pool, company, "internal", Some(wh)).await;
    let customer = loc(&pool, company, "customer", None).await;

    // R9: src == dest.
    let err = w.create_move(new_move(company, item, stock, stock, "1")).await.unwrap_err();
    assert!(matches!(err, InventoryError::SameLocation { .. }));
    // R23: negative demand.
    let err = w.create_move(new_move(company, item, stock, customer, "-1")).await.unwrap_err();
    assert!(matches!(err, InventoryError::NegativeQuantity));
    // R13: a view location can hold no stock on either side.
    let view = loc(&pool, company, "view", None).await;
    let err = w.create_move(new_move(company, item, view, customer, "1")).await.unwrap_err();
    assert!(matches!(err, InventoryError::ViewLocationHoldsNoStock { .. }));
    // R26: an internal location belongs to another company — the move may not draw from it.
    let foreign = loc(&pool, Uuid::new_v4(), "internal", Some(wh)).await;
    let err = w.create_move(new_move(company, item, foreign, customer, "1")).await.unwrap_err();
    assert!(matches!(err, InventoryError::QuantCompanyMismatch { .. }));

    // Guarded machine: confirm a non-draft move; done with no lines.
    let mv = w.create_move(new_move(company, item, stock, customer, "1")).await.unwrap();
    let err = w.action_assign(company, mv).await.unwrap_err();
    assert!(matches!(err, InventoryError::WrongMoveState { .. }), "assign on draft is refused");
    w.action_confirm(company, mv).await.unwrap();
    let err = w.action_confirm(company, mv).await.unwrap_err();
    assert!(matches!(err, InventoryError::WrongMoveState { .. }), "confirm twice is refused");
    let err = w.action_done(company, mv, BackorderPolicy::Never, &no_gl(), &StubGl).await.unwrap_err();
    assert!(matches!(err, InventoryError::MoveLinesRequired { .. }), "R24: done needs lines");
    let err = w.action_done(company, mv, BackorderPolicy::Never, &no_gl(), &StubGl).await.unwrap_err();
    assert!(matches!(err, InventoryError::MoveLinesRequired { .. }));
}

/// DoD probe: two moves competing for the SAME quant with only enough free stock for one —
/// exactly ONE winner. The winner assigns in full; the loser's assign sees insufficient
/// available (zero free under the winner's committed reservation): it reserves nothing
/// (outcome `confirmed`, reserved 0 — never a partial phantom, never an over-reserve) and
/// the availability READ on the quant reports zero. A concurrent variant drives both
/// assigns at once: the quant's `FOR UPDATE` serializes them, so the aggregate invariants
/// hold regardless of who wins the race.
#[tokio::test]
async fn competing_reservations_exactly_one_winner() {
    let pool = pool().await;
    let w = InventoryWriteService::new(pool.clone());
    let (company, item) = (Uuid::new_v4(), Uuid::new_v4());
    let wh = warehouse(&w, company).await;
    let stock = loc(&pool, company, "internal", Some(wh)).await;
    let customer = loc(&pool, company, "customer", None).await;
    seed_quant(&pool, company, item, stock, "6").await;
    seed_bin(&pool, company, item, wh, "6", "10").await;

    // Sequential: A claims the full 6 first.
    let a = w.create_move(new_move(company, item, stock, customer, "6")).await.unwrap();
    let b = w.create_move(new_move(company, item, stock, customer, "6")).await.unwrap();
    w.action_confirm(company, a).await.unwrap();
    w.action_confirm(company, b).await.unwrap();

    let oa = w.action_assign(company, a).await.unwrap();
    assert_eq!((oa.state.as_str(), oa.reserved_qty), ("assigned", d("6")), "the winner takes all 6");

    let ob = w.action_assign(company, b).await.unwrap();
    assert_eq!((ob.state.as_str(), ob.reserved_qty), ("confirmed", d("0")),
        "the loser sees insufficient available: nothing free to reserve, state stays confirmed");

    // The authoritative counter and the availability READ (quantity - reserved) agree.
    let (q, r) = quant_at(&pool, company, item, stock).await;
    assert_eq!((q, r), (d("6"), d("6")));
    let avail: Decimal = sqlx::query_scalar(
        "SELECT available_quantity FROM inventory.stock_quants WHERE company_id=$1 AND item_id=$2 AND location_id=$3",
    ).bind(company).bind(item).bind(stock).fetch_one(&pool).await.unwrap();
    assert_eq!(avail, d("0"), "available = quantity - reserved, a read not a second writer");

    // The winner holds the only live mirror line; the loser holds none.
    let live: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM inventory.stock_move_lines l JOIN inventory.stock_moves m ON m.id=l.move_id \
         WHERE m.id = ANY($1::uuid[]) AND l.quantity > 0",
    ).bind(vec![a, b]).fetch_one(&pool).await.unwrap();
    assert_eq!(live, 1, "exactly one mirror line lives — the loser minted none");

    // Concurrent: two fresh competitors race for the SAME 6 free units (total demand 12 > 6
    // free) — the row lock serializes; whichever commits first wins all 6, the other reserves
    // 0. Aggregate invariants hold either way (never reserved > on-hand, exactly one assigned).
    sqlx::query("UPDATE inventory.stock_quants SET quantity=6, available_quantity=6, reserved_quantity=0 \
                 WHERE company_id=$1 AND item_id=$2 AND location_id=$3")
        .bind(company).bind(item).bind(stock).execute(&pool).await.unwrap();
    let c = w.create_move(new_move(company, item, stock, customer, "6")).await.unwrap();
    let e = w.create_move(new_move(company, item, stock, customer, "6")).await.unwrap();
    w.action_confirm(company, c).await.unwrap();
    w.action_confirm(company, e).await.unwrap();
    let (oc, oe) = tokio::join!(w.action_assign(company, c), w.action_assign(company, e));
    let (oc, oe) = (oc.unwrap(), oe.unwrap());
    let assigned: Vec<&str> = [&oc, &oe].iter().map(|o| o.state.as_str()).filter(|s| *s == "assigned").collect();
    assert_eq!(assigned.len(), 1, "exactly one winner even under the race: {:?}", (&oc.state, &oe.state));
    assert_eq!(oc.reserved_qty + oe.reserved_qty, d("6"), "no over-reserve: the pair reserved exactly the free 6");
    let (_, r2) = quant_at(&pool, company, item, stock).await;
    assert_eq!(r2, d("6"), "the authoritative counter equals the free stock taken");
}
