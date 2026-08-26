//! Landed-cost allocation probes (the valuation overlay's landed-cost increment): the three
//! split bases over a target receipt's DONE moves, the LOUD zero-denominator rejection (no
//! silent equal-split fallback — the decided deviation), HALF-UP rounding with the
//! deterministic last-line-eats-the-diff recipient, the destructive worksheet rebuild, the
//! retroactive-revaluation asymmetry (only the still-on-hand share revalues — no COGS
//! true-up leg), the negative-document reversal pattern, the state-machine guards, and the
//! single-writer guarantee (the landed-cost door never touches the SLE/bin writers).
//!
//! Coverage (L1–L15 + the receipt-seam probe):
//! - L1/L2/L3 split by quantity / value / weight — golden allocations.
//! - L4 the loud zero-denominator rejection on the reachable bases (weight with no per-unit
//!   weights, value with zero-valued target lines); the quantity basis is structurally
//!   guarded (a non-positive-qty move never enters the target set, so an empty target set
//!   rejects loudly first).
//! - L5 HALF-UP at the minor unit + the last target line (by move_line_id) eats the diff.
//! - L6 the worksheet is a destructive rebuild: pre-seeded junk rows are wiped and the
//!   recomputed cumulative `additional_landed_cost` is identical.
//! - L7 the asymmetry: 10 received, 4 delivered → the bin revalues ONLY the 6-unit share;
//!   the SLE row carries the remaining-share delta; ZERO COGS-correction legs.
//! - L8 a negative landed cost is the reversal pattern: swapped legs verbatim, bin rate falls.
//! - L9 a `done` document can never cancel; cancel works from `draft`.
//! - L10 the dual check-sum: worksheet Σ allocations == Σ cost amounts AND journal Σ == Σ δ.
//! - L11 a cost line without a credit account is rejected loudly.
//! - L12 a `standard`-costing company is refused loudly.
//! - L13 static single-writer probe: `insert_sle`/`update_balance` call sites stay
//!   engine-only; the landed-cost door holds no direct SLE/bin writes; the asymmetry is
//!   documented at both code sites.
//! - L14 the strict company fence: a cross-company read sees 0 rows.
//! - L15 the GL leg rides posting_state: deferred validate arms `pending` in-tx, repost
//!   heals, and a re-repost short-circuits (no double post).
//! - SEAM a flagged `is_landed_costs_line` receipt line mints no move, no bin, no envelope
//!   leg — it carries cost into a landed-cost document, not stock.
//!
//! Requires DATABASE_URL (:5433/backbone_inventory), inventory schema applied (the
//! accounting schema is NOT needed — envelopes are captured by a recording sink).

use rust_decimal::Decimal;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use backbone_inventory::application::service::inventory_gl::{
    AccountingPostEnvelope, GlPostAck, GlPostRejected, GlPostSink,
};
use backbone_inventory::application::service::inventory_write_service::{
    InventoryError, InventoryWriteService, LcCostLine, NewDelivery, NewLandedCost, NewReceipt,
    NewWarehouse, ReceiptLine, DeliveryLine,
};

// --- the recording sink ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct RecLine {
    account_id: Uuid,
    debit: Decimal,
    credit: Decimal,
}

/// Captures every envelope line the service emits, so tests assert the posting SHAPE without
/// depending on a real ledger.
#[derive(Default)]
struct Recorder {
    lines: std::sync::Mutex<Vec<RecLine>>,
}
#[async_trait::async_trait]
impl GlPostSink for Recorder {
    async fn post(&self, e: &AccountingPostEnvelope) -> Result<GlPostAck, GlPostRejected> {
        let mut seen = self.lines.lock().unwrap();
        for l in &e.lines {
            seen.push(RecLine { account_id: l.account_id, debit: l.debit, credit: l.credit });
        }
        Ok(GlPostAck { post_id: Uuid::new_v4(), journal_id: Uuid::new_v4(), idempotent_reuse: false })
    }
}
impl Recorder {
    fn count(&self) -> usize { self.lines.lock().unwrap().len() }
    fn debit_on(&self, acct: Uuid) -> Decimal {
        self.lines.lock().unwrap().iter().filter(|l| l.account_id == acct).map(|l| l.debit).sum()
    }
    fn credit_on(&self, acct: Uuid) -> Decimal {
        self.lines.lock().unwrap().iter().filter(|l| l.account_id == acct).map(|l| l.credit).sum()
    }
}

// --- scaffolding -----------------------------------------------------------------------------------

fn d(s: &str) -> Decimal { Decimal::from_str_exact(s).unwrap() }
fn day() -> chrono::NaiveDate { chrono::NaiveDate::from_ymd_opt(2026, 8, 26).unwrap() }
fn uq(p: &str) -> String { format!("{p}-{}", &Uuid::new_v4().simple().to_string()[..8]) }
async fn pool() -> PgPool {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:postgres@localhost:5433/backbone_inventory".to_string());
    PgPool::connect(&url).await.expect("connect DB")
}

/// Two placeholder GL accounts: the inventory side and the landed-cost credit side.
#[derive(Clone, Copy)]
struct Accts { inv: Uuid, cost: Uuid, grir: Uuid, cogs: Uuid }
fn accts() -> Accts {
    Accts { inv: Uuid::new_v4(), cost: Uuid::new_v4(), grir: Uuid::new_v4(), cogs: Uuid::new_v4() }
}

async fn warehouse(w: &InventoryWriteService, company: Uuid) -> Uuid {
    w.create_warehouse(NewWarehouse {
        company_id: company, code: uq("WH"), name: uq("Main"),
        warehouse_type: None, parent_warehouse_id: None, is_group: false,
    }).await.unwrap()
}

/// Register an item on the item master with an optional per-unit weight (the weight basis).
async fn item(w: &InventoryWriteService, pool: &PgPool, company: Uuid, weight: Option<&str>) -> Uuid {
    let item_id = Uuid::new_v4();
    w.create_stock_item(backbone_inventory::application::service::inventory_write_service::NewStockItem {
        item_id, company_id: company, stock_uom: "unit".into(),
        valuation_method: None, reorder_level: Decimal::ZERO,
    }).await.unwrap();
    if let Some(wt) = weight {
        sqlx::query("UPDATE inventory.stock_items SET weight_per_unit=$3 WHERE company_id=$1 AND item_id=$2")
            .bind(company).bind(item_id).bind(d(wt)).execute(pool).await.unwrap();
    }
    item_id
}

/// Submit a two-line receipt: (item, qty, rate) pairs. Returns the receipt id.
async fn receive(
    w: &InventoryWriteService, company: Uuid, wh: Uuid, a: &Accts,
    lines: &[(Uuid, &str, &str)], rec: &Recorder,
) -> Uuid {
    let rid = w.create_purchase_receipt(NewReceipt {
        receipt_number: uq("PR"), company_id: company, branch_id: None, supplier_id: Uuid::new_v4(),
        source_po_id: None, warehouse_id: wh, posting_date: day(),
        currency: "IDR".into(),
        inventory_account_id: a.inv, grir_account_id: a.grir,
        lines: lines.iter().map(|(i, q, r)| ReceiptLine {
            item_id: *i, quantity: d(q), rate: d(r), is_landed_costs_line: false,
        }).collect(),
    }).await.unwrap();
    w.submit_purchase_receipt(rid, rec).await.unwrap();
    rid
}

/// Open a DRAFT landed cost over `receipt`: (name, split_method, amount) cost lines, all on
/// the shared credit account. Returns the landed-cost id.
async fn draft_lc(
    w: &InventoryWriteService, company: Uuid, receipt: Uuid,
    lines: &[(&str, &str, &str)], credit_acct: Uuid,
) -> Uuid {
    w.create_landed_cost(NewLandedCost {
        lc_number: uq("LC"), company_id: company, branch_id: None,
        target_receipt_id: receipt, posting_date: day(), currency: "IDR".into(), notes: None,
        lines: lines.iter().map(|(n, m, amt)| LcCostLine {
            name: (*n).into(), account_id: credit_acct, split_method: (*m).into(), amount: d(amt),
        }).collect(),
    }).await.unwrap()
}

/// One worksheet row joined back to its item (the human grain the tests assert on).
#[derive(Debug)]
struct WsRow { item_id: Uuid, share: Decimal, cumulative: Decimal, remaining_qty: Decimal }

async fn worksheet(pool: &PgPool, lc_id: Uuid) -> Vec<WsRow> {
    let rows = sqlx::query(
        r#"SELECT m.item_id, w.share, w.additional_landed_cost, w.remaining_qty
           FROM inventory.landed_cost_adjustment_lines w
           JOIN inventory.stock_move_lines ml ON ml.id = w.move_line_id
           JOIN inventory.stock_moves m ON m.id = ml.move_id
           WHERE w.lc_id=$1
           ORDER BY w.id"#,
    )
    .bind(lc_id).fetch_all(pool).await.unwrap();
    rows.iter().map(|r| WsRow {
        item_id: r.get("item_id"), share: r.get("share"),
        cumulative: r.get("additional_landed_cost"), remaining_qty: r.get("remaining_qty"),
    }).collect()
}

async fn bin(pool: &PgPool, company: Uuid, item: Uuid, wh: Uuid) -> (Decimal, Decimal, Decimal) {
    sqlx::query_as(
        "SELECT actual_qty, valuation_rate, stock_value FROM inventory.bins \
         WHERE company_id=$1 AND item_id=$2 AND warehouse_id=$3",
    ).bind(company).bind(item).bind(wh).fetch_one(pool).await.unwrap()
}

async fn lc_state(pool: &PgPool, lc_id: Uuid) -> (String, String) {
    let r = sqlx::query("SELECT state::text AS st, posting_state::text AS ps FROM inventory.landed_costs WHERE id=$1")
        .bind(lc_id).fetch_one(pool).await.unwrap();
    (r.get("st"), r.get("ps"))
}

/// One company's posting posture (cost-method axis only matters here).
async fn set_cost_method(pool: &PgPool, company: Uuid, cost_method: &str) {
    sqlx::query(
        r#"INSERT INTO inventory.inventory_company_settings
             (id, company_id, cost_method, valuation_policy, anglo_saxon_accounting)
           VALUES ($1,$2,$3::inventory_cost_method,'perpetual',false)"#,
    )
    .bind(Uuid::new_v4()).bind(company).bind(cost_method).execute(pool).await.unwrap();
}

/// The standard two-item target receipt: A = 10 units @ 100 (carried 1000),
/// B = 2 units @ 250 (carried 500). Returns (receipt_id, item_a, item_b).
async fn two_line_receipt(
    w: &InventoryWriteService, pool: &PgPool, company: Uuid, a: &Accts, rec: &Recorder,
) -> (Uuid, Uuid, Uuid) {
    let wh = warehouse(w, company).await;
    let ia = item(w, pool, company, None).await;
    let ib = item(w, pool, company, None).await;
    let rid = receive(w, company, wh, a, &[(ia, "10", "100"), (ib, "2", "250")], rec).await;
    (rid, ia, ib)
}

/// The worksheet share of one item (single-cost-line documents).
fn share_of(rows: &[WsRow], item: Uuid) -> Decimal {
    rows.iter().find(|r| r.item_id == item).map(|r| r.share).expect("row for item")
}

// --- L1: split by quantity -------------------------------------------------------------------------

#[tokio::test]
async fn l1_split_by_quantity() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let w = InventoryWriteService::new(pool.clone());
    let rec = Recorder::default();
    let a = accts();
    let (rid, ia, ib) = two_line_receipt(&w, &pool, company, &a, &rec).await;
    let lc = draft_lc(&w, company, rid, &[("freight", "quantity", "300")], a.cost).await;
    let lc_rec = Recorder::default();
    w.validate_landed_cost(lc, &lc_rec).await.unwrap();
    let rows = worksheet(&pool, lc).await;
    // 300 × 10/12 and 300 × 2/12 — both fully on hand, so share == delta.
    assert_eq!(share_of(&rows, ia), d("250.00"), "L1: 10 of 12 units");
    assert_eq!(share_of(&rows, ib), d("50.00"), "L1: 2 of 12 units");
    let wh = sqlx::query_scalar::<_, Uuid>(
        "SELECT warehouse_id FROM inventory.purchase_receipts WHERE id=$1").bind(rid)
        .fetch_one(&pool).await.unwrap();
    assert_eq!(bin(&pool, company, ia, wh).await, (d("10"), d("125.000000"), d("1250.00")),
        "L1: bin A revalued 1000 + 250");
    assert_eq!(bin(&pool, company, ib, wh).await, (d("2"), d("275.000000"), d("550.00")),
        "L1: bin B revalued 500 + 50");
    // The door-owned envelope: Dr inventory 300 / Cr cost line 300.
    assert_eq!(lc_rec.debit_on(a.inv), d("300"), "L1: Dr inventory the full amount (all on hand)");
    assert_eq!(lc_rec.credit_on(a.cost), d("300"), "L1: Cr the cost line's account");
}

// --- L2: split by value ----------------------------------------------------------------------------

#[tokio::test]
async fn l2_split_by_value() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let w = InventoryWriteService::new(pool.clone());
    let rec = Recorder::default();
    let a = accts();
    let (rid, ia, ib) = two_line_receipt(&w, &pool, company, &a, &rec).await;
    let lc = draft_lc(&w, company, rid, &[("insurance", "value", "300")], a.cost).await;
    w.validate_landed_cost(lc, &rec).await.unwrap();
    let rows = worksheet(&pool, lc).await;
    // Carried values 1000 and 500 → 300 × 2/3 and 300 × 1/3.
    assert_eq!(share_of(&rows, ia), d("200.00"), "L2: 1000 of 1500 carried");
    assert_eq!(share_of(&rows, ib), d("100.00"), "L2: 500 of 1500 carried");
}

// --- L3: split by weight ---------------------------------------------------------------------------

#[tokio::test]
async fn l3_split_by_weight() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let w = InventoryWriteService::new(pool.clone());
    let rec = Recorder::default();
    let a = accts();
    let wh = warehouse(&w, company).await;
    let ia = item(&w, &pool, company, Some("3")).await;
    let ib = item(&w, &pool, company, Some("0.5")).await;
    let rid = receive(&w, company, wh, &a, &[(ia, "10", "100"), (ib, "2", "250")], &rec).await;
    let lc = draft_lc(&w, company, rid, &[("freight", "weight", "310")], a.cost).await;
    w.validate_landed_cost(lc, &rec).await.unwrap();
    let rows = worksheet(&pool, lc).await;
    // Weight basis: A = 10 × 3 = 30, B = 2 × 0.5 = 1 → 310 × 30/31 = 300 and 310 × 1/31 = 10.
    assert_eq!(share_of(&rows, ia), d("300.00"), "L3: 30 of 31 weight-units");
    assert_eq!(share_of(&rows, ib), d("10.00"), "L3: 1 of 31 weight-units");
}

// --- L4: the loud zero-denominator rejection --------------------------------------------------------

#[tokio::test]
async fn l4_zero_denominator_is_loud_with_no_fallback() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let w = InventoryWriteService::new(pool.clone());
    let rec = Recorder::default();
    let a = accts();

    // (a) WEIGHT basis, no item carries a per-unit weight → Σ basis = 0.
    let wh = warehouse(&w, company).await;
    let ia = item(&w, &pool, company, None).await; // weight 0 — the unweighted default
    let rid = receive(&w, company, wh, &a, &[(ia, "10", "100")], &rec).await;
    let lc = draft_lc(&w, company, rid, &[("freight", "weight", "100")], a.cost).await;
    let err = w.validate_landed_cost(lc, &rec).await.unwrap_err();
    assert!(matches!(&err, InventoryError::LandedCostZeroSplitBasis { basis, .. } if basis == "weight"),
        "L4a: loud zero-basis on weight, got {}", err.code());
    let rows = worksheet(&pool, lc).await;
    assert!(rows.is_empty(), "L4a: no partial worksheet rows — the rejection wrote nothing");
    assert_eq!(lc_state(&pool, lc).await.0, "draft", "L4a: the document never left draft");

    // (b) VALUE basis, zero-valued target lines (rate 0) → Σ carried value = 0.
    let ib = item(&w, &pool, company, None).await;
    let rid0 = receive(&w, company, wh, &a, &[(ib, "5", "0")], &rec).await;
    let lc0 = draft_lc(&w, company, rid0, &[("fee", "value", "100")], a.cost).await;
    let err0 = w.validate_landed_cost(lc0, &rec).await.unwrap_err();
    assert!(matches!(&err0, InventoryError::LandedCostZeroSplitBasis { basis, .. } if basis == "value"),
        "L4b: loud zero-basis on value, got {}", err0.code());
    assert!(worksheet(&pool, lc0).await.is_empty(), "L4b: no partial rows");

    // (c) QUANTITY basis is structurally guarded: a non-positive-qty move never enters the
    // target set, so the loud rejection for an all-empty receipt is NoValuedTargets.
    let ic = item(&w, &pool, company, None).await;
    let ridz = w.create_purchase_receipt(NewReceipt {
        receipt_number: uq("PR"), company_id: company, branch_id: None, supplier_id: Uuid::new_v4(),
        source_po_id: None, warehouse_id: wh, posting_date: day(), currency: "IDR".into(),
        inventory_account_id: a.inv, grir_account_id: a.grir,
        lines: vec![ReceiptLine { item_id: ic, quantity: Decimal::ZERO, rate: d("100"), is_landed_costs_line: false }],
    }).await.unwrap();
    w.submit_purchase_receipt(ridz, &rec).await.unwrap();
    let lcz = draft_lc(&w, company, ridz, &[("freight", "quantity", "100")], a.cost).await;
    let errz = w.validate_landed_cost(lcz, &rec).await.unwrap_err();
    assert_eq!(errz.code(), "landed_cost_no_valued_targets",
        "L4c: an empty target set rejects loudly (the quantity basis cannot sum to zero)");
}

// --- L5: HALF-UP rounding + the deterministic last-line-eats-the-diff recipient ----------------------

#[tokio::test]
async fn l5_half_up_and_last_line_eats_the_diff() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let w = InventoryWriteService::new(pool.clone());
    let rec = Recorder::default();
    let a = accts();
    let wh = warehouse(&w, company).await;
    let items = [item(&w, &pool, company, None).await, item(&w, &pool, company, None).await];

    // (a) 1.00 over three equal-qty lines: 0.33, 0.33, and the LAST (by move_line_id) eats 0.34.
    let r3 = receive(&w, company, wh, &a,
        &[(items[0], "1", "10"), (items[1], "1", "10"), (item(&w, &pool, company, None).await, "1", "10")], &rec).await;
    let lc3 = draft_lc(&w, company, r3, &[("fee", "quantity", "1.00")], a.cost).await;
    w.validate_landed_cost(lc3, &rec).await.unwrap();
    let rows = worksheet(&pool, lc3).await;
    let sum: Decimal = rows.iter().map(|r| r.share).sum();
    assert_eq!(sum, d("1.00"), "L5a: Σ shares == the cost amount exactly");
    assert_eq!(rows.iter().filter(|r| r.share == d("0.33")).count(), 2, "L5a: two rounded 0.33 legs");
    assert_eq!(rows.iter().filter(|r| r.share == d("0.34")).count(), 1, "L5a: the last line eats the diff");

    // (b) A true HALF-UP midpoint: 0.05 over two equal lines → money(0.025) = 0.03 (away from
    // zero), the last line carries 0.02. Banker's rounding would give 0.02/0.03.
    let r2 = receive(&w, company, wh, &a, &[(items[0], "1", "10"), (items[1], "1", "10")], &rec).await;
    let lc2 = draft_lc(&w, company, r2, &[("fee", "quantity", "0.05")], a.cost).await;
    w.validate_landed_cost(lc2, &rec).await.unwrap();
    let rows2 = worksheet(&pool, lc2).await;
    assert_eq!(rows2.iter().filter(|r| r.share == d("0.03")).count(), 1,
        "L5b: HALF-UP rounds the 0.025 midpoint AWAY from zero");
    assert_eq!(rows2.iter().filter(|r| r.share == d("0.02")).count(), 1,
        "L5b: the last line carries the remainder");
}

// --- L6: the worksheet is a destructive rebuild ------------------------------------------------------

#[tokio::test]
async fn l6_worksheet_rebuild_is_destructive_and_deterministic() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let w = InventoryWriteService::new(pool.clone());
    let rec = Recorder::default();
    let a = accts();
    let (rid, ia, _ib) = two_line_receipt(&w, &pool, company, &a, &rec).await;
    let lc = draft_lc(&w, company, rid,
        &[("freight", "quantity", "120"), ("insurance", "quantity", "60")], a.cost).await;
    // Pre-seed junk worksheet rows: a validation MUST wipe them (delete + recreate), never
    // append to them — the worksheet is a transient read-back surface, not a ledger.
    for (mlid, bogus) in [("00000000-0000-0000-0000-000000000001".parse::<Uuid>().unwrap(), d("999")),
                          ("00000000-0000-0000-0000-000000000002".parse::<Uuid>().unwrap(), d("888"))] {
        sqlx::query(
            r#"INSERT INTO inventory.landed_cost_adjustment_lines
                 (id, lc_id, company_id, move_line_id, cost_line_id, share, additional_landed_cost, remaining_qty)
               VALUES ($1,$2,$3,$4,$5,$6,$6,0)"#,
        )
        .bind(Uuid::new_v4()).bind(lc).bind(company).bind(mlid)
        .bind(Uuid::new_v4()).bind(bogus).execute(&pool).await.unwrap();
    }
    w.validate_landed_cost(lc, &rec).await.unwrap();
    let rows = worksheet(&pool, lc).await;
    // Freight 120 → 100/20; insurance 60 → 50/10; cumulative per move line = 150/30.
    // No 999/888 junk survives the rebuild.
    assert_eq!(rows.len(), 4, "L6: exactly one row per cost line × target line");
    assert!(rows.iter().all(|r| r.share != d("999") && r.share != d("888")),
        "L6: the junk rows were wiped by the destructive rebuild");
    for r in &rows {
        let expect = if r.item_id == ia { d("150.00") } else { d("30.00") };
        assert_eq!(r.cumulative, expect,
            "L6: cumulative additional_landed_cost across BOTH cost lines (120+60 split 10:2)");
    }
    // The two cost lines' shares per item are exactly the per-line splits.
    let ia_shares: Vec<Decimal> = rows.iter().filter(|r| r.item_id == ia).map(|r| r.share).collect();
    let mut sorted = ia_shares.clone();
    sorted.sort();
    assert_eq!(sorted, vec![d("50.00"), d("100.00")], "L6: freight 100 + insurance 50 on item A");
}

// --- L7: the retroactive-revaluation asymmetry --------------------------------------------------------

#[tokio::test]
async fn l7_only_the_remaining_share_revalues_no_cogs_true_up() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let w = InventoryWriteService::new(pool.clone());
    let rec = Recorder::default();
    let a = accts();
    let wh = warehouse(&w, company).await;
    let i = item(&w, &pool, company, None).await;
    // Receive 10 @ 100, then deliver 4 BEFORE the landed cost lands.
    let rid = receive(&w, company, wh, &a, &[(i, "10", "100")], &rec).await;
    let did = w.create_delivery_note(NewDelivery {
        delivery_number: uq("DN"), company_id: company, branch_id: None, customer_id: Uuid::new_v4(),
        source_so_id: None, warehouse_id: wh, posting_date: day(), currency: "IDR".into(),
        cogs_account_id: a.cogs, inventory_account_id: a.inv,
        lines: vec![DeliveryLine { item_id: i, quantity: d("4") }],
    }).await.unwrap();
    w.submit_delivery_note(did, &rec).await.unwrap();
    assert_eq!(bin(&pool, company, i, wh).await, (d("6"), d("100.000000"), d("600.00")),
        "L7: 6 on hand at the old average before the landed cost");

    let lc = draft_lc(&w, company, rid, &[("freight", "quantity", "100")], a.cost).await;
    let lc_rec = Recorder::default();
    w.validate_landed_cost(lc, &lc_rec).await.unwrap();
    let ws = worksheet(&pool, lc).await;
    assert_eq!(ws[0].remaining_qty, d("6.0000"),
        "L7: the worksheet snapshots the FIFO-attributed remaining share");

    // THE ASYMMETRY: δ = 100 × 6/10 = 60 — the consumed 4 units' 40 produces NOTHING.
    assert_eq!(bin(&pool, company, i, wh).await, (d("6"), d("110.000000"), d("660.00")),
        "L7: only the 6-unit share revalues (600 + 60), rate reblends to 110");
    let sle: Option<(Decimal, Decimal)> = sqlx::query_as(
        r#"SELECT actual_qty, stock_value_difference FROM inventory.stock_ledger_entries
           WHERE company_id=$1 AND voucher_type='landed_cost' AND voucher_id=$2"#,
    ).bind(company).bind(lc).fetch_optional(&pool).await.unwrap();
    let (qty, diff) = sle.expect("L7: exactly one landed-cost SLE row");
    assert_eq!(qty, Decimal::ZERO, "L7: the revaluation moves no quantity");
    assert_eq!(diff, d("60.00"), "L7: the SLE carries the REMAINING-share delta only");

    // ZERO COGS-correction legs: the landed cost's own envelope is the only post — a balanced
    // Dr inventory 60 / Cr cost-line 60. Nothing debits COGS for the consumed share.
    assert_eq!(lc_rec.count(), 2, "L7: exactly one two-legged envelope");
    assert_eq!(lc_rec.debit_on(a.inv), d("60"), "L7: Dr inventory the remaining-share value");
    assert_eq!(lc_rec.credit_on(a.cost), d("60"), "L7: Cr the cost line's remaining-share value");
    assert_eq!(lc_rec.debit_on(a.cogs), Decimal::ZERO, "L7: NO COGS true-up leg");
    assert_eq!(lc_rec.credit_on(a.cogs), Decimal::ZERO, "L7: NO COGS credit either");
}

// --- L8: a negative landed cost is the reversal pattern ----------------------------------------------

#[tokio::test]
async fn l8_negative_landed_cost_swaps_legs_and_lowers_the_rate() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let w = InventoryWriteService::new(pool.clone());
    let rec = Recorder::default();
    let a = accts();
    let wh = warehouse(&w, company).await;
    let i = item(&w, &pool, company, None).await;
    let rid = receive(&w, company, wh, &a, &[(i, "10", "100")], &rec).await;
    // First a POSITIVE landed cost (+100), then its negative correction (-100).
    let pos = draft_lc(&w, company, rid, &[("freight", "quantity", "100")], a.cost).await;
    w.validate_landed_cost(pos, &rec).await.unwrap();
    assert_eq!(bin(&pool, company, i, wh).await, (d("10"), d("110.000000"), d("1100.00")),
        "L8: +100 revalues the full on-hand estate");

    let neg = draft_lc(&w, company, rid, &[("freight correction", "quantity", "-100")], a.cost).await;
    let neg_rec = Recorder::default();
    w.validate_landed_cost(neg, &neg_rec).await.unwrap();
    assert_eq!(bin(&pool, company, i, wh).await, (d("10"), d("100.000000"), d("1000.00")),
        "L8: the negative document lowers the bin value back (rate 110 → 100)");
    // Swapped legs verbatim: Dr the cost-line account / Cr inventory.
    assert_eq!(neg_rec.debit_on(a.cost), d("100"), "L8: the reversal DEBITS the cost-line account");
    assert_eq!(neg_rec.credit_on(a.inv), d("100"), "L8: the reversal CREDITS inventory");
    assert_eq!(neg_rec.count(), 2, "L8: one balanced swapped envelope");
}

// --- L9: the state machine — done can never cancel --------------------------------------------------

#[tokio::test]
async fn l9_done_cannot_cancel_and_cancel_only_from_draft() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let w = InventoryWriteService::new(pool.clone());
    let rec = Recorder::default();
    let a = accts();
    let (rid, _, _) = two_line_receipt(&w, &pool, company, &a, &rec).await;

    // A draft cancels cleanly.
    let draft = draft_lc(&w, company, rid, &[("fee", "quantity", "10")], a.cost).await;
    w.cancel_landed_cost(draft).await.unwrap();
    assert_eq!(lc_state(&pool, draft).await.0, "cancel", "L9: draft → cancel");

    // A DONE document can never cancel — the correction is a negative landed cost.
    let done = draft_lc(&w, company, rid, &[("fee", "quantity", "10")], a.cost).await;
    w.validate_landed_cost(done, &rec).await.unwrap();
    let err = w.cancel_landed_cost(done).await.unwrap_err();
    assert_eq!(err.code(), "landed_cost_not_draft", "L9: done refuses to cancel loudly");

    // A cancelled document cannot cancel again either.
    let err2 = w.cancel_landed_cost(draft).await.unwrap_err();
    assert_eq!(err2.code(), "landed_cost_not_draft", "L9: cancel only from draft");
}

// --- L10: the dual check-sum -------------------------------------------------------------------------

#[tokio::test]
async fn l10_worksheet_and_journal_both_sum_to_the_document() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let w = InventoryWriteService::new(pool.clone());
    let rec = Recorder::default();
    let a = accts();
    let (rid, ia, ib) = two_line_receipt(&w, &pool, company, &a, &rec).await;
    // Two cost lines on DIFFERENT accounts with awkward 3-way splits.
    let wh = sqlx::query_scalar::<_, Uuid>(
        "SELECT warehouse_id FROM inventory.purchase_receipts WHERE id=$1").bind(rid)
        .fetch_one(&pool).await.unwrap();
    let ic = item(&w, &pool, company, None).await;
    let rid3 = receive(&w, company, wh, &a,
        &[(ia, "10", "100"), (ib, "2", "250"), (ic, "3", "50")], &rec).await;
    let other = Uuid::new_v4();
    let lc = w.create_landed_cost(NewLandedCost {
        lc_number: uq("LC"), company_id: company, branch_id: None,
        target_receipt_id: rid3, posting_date: day(), currency: "IDR".into(), notes: None,
        lines: vec![
            LcCostLine { name: "freight".into(), account_id: a.cost, split_method: "quantity".into(), amount: d("100.00") },
            LcCostLine { name: "insurance".into(), account_id: other, split_method: "value".into(), amount: d("50.00") },
        ],
    }).await.unwrap();
    let lc_rec = Recorder::default();
    w.validate_landed_cost(lc, &lc_rec).await.unwrap();

    // (a) The worksheet: Σ per-cost-line shares == the declared cost amounts.
    let lines: Vec<(Uuid, Decimal)> = sqlx::query_as(
        "SELECT id, amount FROM inventory.landed_cost_lines WHERE lc_id=$1",
    ).bind(lc).fetch_all(&pool).await.unwrap();
    for (line_id, amount) in &lines {
        let s: Decimal = sqlx::query_scalar(
            "SELECT COALESCE(SUM(share),0) FROM inventory.landed_cost_adjustment_lines WHERE lc_id=$1 AND cost_line_id=$2",
        ).bind(lc).bind(line_id).fetch_one(&pool).await.unwrap();
        assert_eq!(s, *amount, "L10a: worksheet Σ == cost amount {amount}");
    }
    // (b) The journal: Dr Σ == Cr Σ == Σ remaining-share deltas == the value that hit the bins.
    let revalued: Decimal = sqlx::query_scalar(
        "SELECT COALESCE(SUM(stock_value_difference),0) FROM inventory.stock_ledger_entries \
         WHERE voucher_type='landed_cost' AND voucher_id=$1",
    ).bind(lc).fetch_one(&pool).await.unwrap();
    let dr: Decimal = lc_rec.lines.lock().unwrap().iter().map(|l| l.debit).sum();
    let cr: Decimal = lc_rec.lines.lock().unwrap().iter().map(|l| l.credit).sum();
    // Everything is on hand → Σ δ == Σ amounts == 150.
    assert_eq!((dr, cr, revalued), (d("150.00"), d("150.00"), d("150.00")),
        "L10b: journal Dr Σ == Cr Σ == Σ δ == the revalued estate");
}

// --- L11: a cost line without a credit account is rejected -------------------------------------------

#[tokio::test]
async fn l11_cost_line_without_account_is_rejected() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let w = InventoryWriteService::new(pool.clone());
    let rec = Recorder::default();
    let a = accts();
    let (rid, _, _) = two_line_receipt(&w, &pool, company, &a, &rec).await;
    let err = w.create_landed_cost(NewLandedCost {
        lc_number: uq("LC"), company_id: company, branch_id: None,
        target_receipt_id: rid, posting_date: day(), currency: "IDR".into(), notes: None,
        lines: vec![LcCostLine {
            name: "no account".into(), account_id: Uuid::nil(),
            split_method: "quantity".into(), amount: d("100"),
        }],
    }).await.unwrap_err();
    assert_eq!(err.code(), "landed_cost_line_needs_account",
        "L11: the credit side is required — its split would be undefined");
}

// --- L12: a standard-costing company is refused loudly ----------------------------------------------

#[tokio::test]
async fn l12_standard_cost_method_refuses_loudly() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let w = InventoryWriteService::new(pool.clone());
    let rec = Recorder::default();
    let a = accts();
    set_cost_method(&pool, company, "standard").await;
    let (rid, _, _) = two_line_receipt(&w, &pool, company, &a, &rec).await;
    let lc = draft_lc(&w, company, rid, &[("fee", "quantity", "100")], a.cost).await;
    let err = w.validate_landed_cost(lc, &rec).await.unwrap_err();
    assert_eq!(err.code(), "landed_cost_requires_cost_method",
        "L12: standard costing refuses loudly (a variance the recompute engine must absorb)");
    assert_eq!(lc_state(&pool, lc).await.0, "draft", "L12: nothing validated");
    assert!(worksheet(&pool, lc).await.is_empty(), "L12: no partial rows");
}

// --- L13: the single-writer static probe -------------------------------------------------------------

#[tokio::test]
async fn l13_sle_and_bin_writers_stay_engine_owned() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut offenders: Vec<String> = Vec::new();
    let mut stack = vec![root.clone()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("walk src") {
            let entry = entry.expect("walk entry");
            let path = entry.path();
            if path.is_dir() { stack.push(path); continue }
            if path.extension().and_then(|e| e.to_str()) != Some("rs") { continue }
            let rel = path.strip_prefix(&root).unwrap().to_string_lossy().to_string();
            // The engine owns the writers' call sites; the persistence layer owns their SQL.
            if rel == "application/service/inventory_move_engine.rs" { continue }
            if rel.starts_with("infrastructure/persistence/") { continue }
            let src = std::fs::read_to_string(&path).expect("read source");
            for token in [".insert_sle(", ".update_balance(", "INSERT INTO inventory.stock_ledger_entries"] {
                if src.contains(token) {
                    offenders.push(format!("{rel}: {token}"));
                }
            }
        }
    }
    assert!(offenders.is_empty(),
        "L13: the one-writer invariant — SLE/bin writes are engine-only:\n{}", offenders.join("\n"));

    // The landed-cost door mints exclusively through the engine verb.
    let door = std::fs::read_to_string(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src/application/service/landed_cost_service_custom.rs"),
    ).unwrap();
    assert!(door.contains("adjust_move_value"), "L13: the door calls the engine verb");
    assert!(!door.contains(".insert_sle(") && !door.contains(".update_balance("),
        "L13: the landed-cost door holds no direct SLE/bin writes");

    // The retroactive-revaluation asymmetry is documented AT the code sites (deliberate,
    // preserved — the valuation-overlay plan's P2 row).
    let engine = std::fs::read_to_string(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src/application/service/inventory_move_engine.rs"),
    ).unwrap();
    assert!(engine.contains("RETROACTIVE-REVALUATION ASYMMETRY"),
        "L13: the engine verb documents the asymmetry at the revalue site");
    assert!(door.contains("RETROACTIVE-REVALUATION ASYMMETRY"),
        "L13: the split site documents the asymmetry where δ is computed");
}

// --- L14: the strict company fence -------------------------------------------------------------------

#[tokio::test]
async fn l14_strict_company_fence_hides_cross_company_rows() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let other = Uuid::new_v4();
    let w = InventoryWriteService::new(pool.clone());
    let rec = Recorder::default();
    let a = accts();
    let (rid, _, _) = two_line_receipt(&w, &pool, company, &a, &rec).await;
    let lc = draft_lc(&w, company, rid, &[("fee", "quantity", "100")], a.cost).await;
    w.validate_landed_cost(lc, &rec).await.unwrap();

    // The test pool connects as a superuser, and BYPASSRLS beats even FORCED RLS — so the
    // fence is probed through a non-superuser role (the shared test DB's established
    // `*_probe_rls` pattern): SET LOCAL ROLE + the OTHER company's scope.
    sqlx::query(
        "DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname='inventory_lc_fence_probe') \
         THEN CREATE ROLE inventory_lc_fence_probe NOLOGIN; END IF; END $$")
        .execute(&pool).await.unwrap();
    sqlx::query("GRANT USAGE ON SCHEMA inventory TO inventory_lc_fence_probe").execute(&pool).await.unwrap();
    sqlx::query("GRANT SELECT ON ALL TABLES IN SCHEMA inventory TO inventory_lc_fence_probe")
        .execute(&pool).await.unwrap();

    let mut tx = pool.begin().await.unwrap();
    sqlx::query("SET LOCAL ROLE inventory_lc_fence_probe").execute(&mut *tx).await.unwrap();
    // `SET LOCAL` takes no bind parameter; a UUID string is injection-safe to interpolate.
    sqlx::query(&format!("SET LOCAL app.company_id = '{}'", other)).execute(&mut *tx).await.unwrap();
    for table in ["landed_costs", "landed_cost_lines", "landed_cost_adjustment_lines"] {
        let n: i64 = sqlx::query_scalar(&format!(
            "SELECT COUNT(*) FROM inventory.{table} WHERE company_id = $1"))
            .bind(company).fetch_one(&mut *tx).await.unwrap();
        assert_eq!(n, 0, "L14: {table} is invisible across the strict fence");
    }
    // Positive control: the SAME probe under the OWNING company's scope DOES see the row —
    // the zero above is the fence, not missing grants.
    sqlx::query(&format!("SET LOCAL app.company_id = '{}'", company)).execute(&mut *tx).await.unwrap();
    let own: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM inventory.landed_costs WHERE company_id = $1")
        .bind(company).fetch_one(&mut *tx).await.unwrap();
    assert_eq!(own, 1, "L14: the owning company's scope reads its own document");
    tx.rollback().await.unwrap();
}

// --- L15: the GL leg rides posting_state ---------------------------------------------------------------

#[tokio::test]
async fn l15_gl_rides_posting_state_pending_then_repost_heals() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let w = InventoryWriteService::new(pool.clone());
    let rec = Recorder::default();
    let a = accts();
    let (rid, _, _) = two_line_receipt(&w, &pool, company, &a, &rec).await;
    let lc = draft_lc(&w, company, rid, &[("fee", "quantity", "150")], a.cost).await;

    // The deferred (HTTP-shaped) validate: physical revaluation commits, GL armS pending.
    let out = w.validate_landed_cost_deferred(lc).await.unwrap();
    assert!(!out.posted, "L15: deferred validate posts nothing itself");
    let (state, ps) = lc_state(&pool, lc).await;
    assert_eq!((state.as_str(), ps.as_str()), ("done", "pending"),
        "L15: armed pending in the same transaction as the physical revaluation");
    let revalued: Decimal = sqlx::query_scalar(
        "SELECT COALESCE(SUM(stock_value_difference),0) FROM inventory.stock_ledger_entries \
         WHERE voucher_type='landed_cost' AND voucher_id=$1")
        .bind(lc).fetch_one(&pool).await.unwrap();
    assert_eq!(revalued, d("150.00"), "L15: the bins revalued even though GL is pending");

    // The repost heals the leg with ONE envelope.
    let heal = Recorder::default();
    let healed = w.repost_landed_cost(lc, &heal).await.unwrap();
    assert!(healed.posted, "L15: the repost posts the armed leg");
    assert_eq!(heal.count(), 2, "L15: exactly one balanced envelope");
    assert_eq!(lc_state(&pool, lc).await.1, "posted", "L15: posting_state reconciled");

    // A re-repost short-circuits — no double post.
    let again = Recorder::default();
    let rerun = w.repost_landed_cost(lc, &again).await.unwrap();
    assert!(rerun.posted, "L15: the second repost reports the recorded settlement");
    assert_eq!(again.count(), 0, "L15: no second envelope — the dedupe/short-circuit holds");
}

// --- the receipt seam: a flagged line carries cost, not stock ------------------------------------------

#[tokio::test]
async fn is_landed_costs_line_mints_no_stock_and_adds_no_value() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let w = InventoryWriteService::new(pool.clone());
    let rec = Recorder::default();
    let a = accts();
    let wh = warehouse(&w, company).await;
    let stock_item = item(&w, &pool, company, None).await;
    let svc_item = item(&w, &pool, company, None).await;
    let rid = w.create_purchase_receipt(NewReceipt {
        receipt_number: uq("PR"), company_id: company, branch_id: None, supplier_id: Uuid::new_v4(),
        source_po_id: None, warehouse_id: wh, posting_date: day(), currency: "IDR".into(),
        inventory_account_id: a.inv, grir_account_id: a.grir,
        lines: vec![
            ReceiptLine { item_id: stock_item, quantity: d("2"), rate: d("50"), is_landed_costs_line: false },
            // The seam: a freight service line flagged on the receipt — cost, not stock.
            ReceiptLine { item_id: svc_item, quantity: d("1"), rate: d("80"), is_landed_costs_line: true },
        ],
    }).await.unwrap();
    w.submit_purchase_receipt(rid, &rec).await.unwrap();

    // Only the stock line minted a move.
    let moves: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM inventory.stock_moves WHERE company_id=$1 AND origin LIKE 'PR-%'")
        .bind(company).fetch_one(&pool).await.unwrap();
    assert_eq!(moves, 1, "SEAM: the flagged line minted no move");
    // The flagged line's bin does not exist; the stock line's carries only its own value.
    assert_eq!(bin(&pool, company, stock_item, wh).await, (d("2"), d("50.000000"), d("100.00")),
        "SEAM: the stock line valued normally");
    let svc_bin: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM inventory.bins WHERE company_id=$1 AND item_id=$2")
        .bind(company).bind(svc_item).fetch_one(&pool).await.unwrap();
    assert_eq!(svc_bin, 0, "SEAM: no bin behind a service line");
    // The door envelope excluded the service line's 80: Dr Inventory 100 only.
    assert_eq!(rec.debit_on(a.inv), d("100"), "SEAM: the envelope carries only the stock value");
    assert_eq!(rec.credit_on(a.grir), d("100"), "SEAM: the credit side matches");
}
