//! THE estate-divergence detector — the regression guard the legacy-door re-wire exists for.
//!
//! Defect class this file kills: a voucher door that writes ONE stock estate (Bins + Stock
//! Ledger Entries) without the other (stock quants), or vice versa — the two estates silently
//! diverge and nothing notices. Every voucher door (submit / cancel, goods-in / goods-out)
//! now mints its legs through the ONE move engine, so after each door fires:
//!
//! 1. the quant ON-HAND at the target location MUST have changed by exactly the door quantity
//!    (signed: +receive / −deliver / −cancel-receive / +cancel-deliver);
//! 2. the estates MUST agree: the warehouse Bin's qty == the quant on-hand at the stock
//!    location, and the append-only ledger MUST balance to both (Σ SLE actual_qty == Bin qty,
//!    Σ SLE value diff == Bin value);
//! 3. the GL leg the door emitted MUST agree with the value its moves actually carried;
//! 4. the legacy direct-write signature MUST be gone — no SLE row is written under the
//!    voucher's own type any more; every leg rides `voucher_type='stock_entry'` keyed by the
//!    move that minted it, and the moves exist under the voucher number as `origin`.
//!
//! If any door ever regresses to direct writes (or writes moves without flipping quants),
//! these assertions fail loudly. Pure inventory + a recording GL sink; requires DATABASE_URL
//! (defaults to :5433/backbone_inventory).

use std::sync::{Arc, Mutex};

use rust_decimal::Decimal;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use backbone_inventory::application::service::inventory_gl::{
    AccountingPostEnvelope, GlPostAck, GlPostRejected, GlPostSink,
};
use backbone_inventory::application::service::inventory_write_service::{
    DeliveryLine, InventoryWriteService, NewDelivery, NewReceipt, NewWarehouse, ReceiptLine,
};

// --- harness -------------------------------------------------------------------

fn d(s: &str) -> Decimal { Decimal::from_str_exact(s).unwrap() }
fn day() -> chrono::NaiveDate { chrono::NaiveDate::from_ymd_opt(2026, 7, 4).unwrap() }
fn uq(p: &str) -> String { format!("{p}-{}", &Uuid::new_v4().simple().to_string()[..8]) }

async fn pool() -> PgPool {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:postgres@localhost:5433/backbone_inventory".to_string());
    PgPool::connect(&url).await.expect("connect DB")
}

async fn warehouse(w: &InventoryWriteService, company: Uuid) -> Uuid {
    w.create_warehouse(NewWarehouse {
        org_unit_id: company, code: uq("WH"), name: uq("Main"),
        warehouse_type: None, parent_warehouse_id: None, is_group: false,
    }).await.unwrap()
}

/// A captured GL post: its posting type and total debit amount.
#[derive(Debug, Clone, PartialEq)]
struct CapturedPost { posting_type: String, total: Decimal }

/// The recording GL sink: accepts every post (like the module's other pure-inventory
/// harnesses) and records the envelope's shape so the door's GL leg can be checked against
/// the value its moves actually carried.
#[derive(Clone)]
struct RecordingSink(Arc<Mutex<Vec<CapturedPost>>>);
impl RecordingSink {
    fn new() -> (Self, Arc<Mutex<Vec<CapturedPost>>>) {
        let inner = Arc::new(Mutex::new(Vec::new()));
        (Self(inner.clone()), inner)
    }
}
#[async_trait::async_trait]
impl GlPostSink for RecordingSink {
    async fn post(&self, e: &AccountingPostEnvelope) -> Result<GlPostAck, GlPostRejected> {
        self.0.lock().unwrap().push(CapturedPost {
            posting_type: e.posting_type.clone(),
            total: e.lines.iter().map(|l| l.debit).sum(),
        });
        Ok(GlPostAck { post_id: Uuid::new_v4(), journal_id: Uuid::new_v4(), idempotent_reuse: false })
    }
}

// --- estate probes -------------------------------------------------------------

/// The warehouse's internal stock location (the door bootstraps `Stock` on first use).
/// `None` before the warehouse has ever been touched by a door.
async fn stock_location(pool: &PgPool, company: Uuid, wh: Uuid) -> Option<Uuid> {
    sqlx::query_scalar::<_, Uuid>(
        r#"SELECT id FROM inventory.locations
           WHERE company_id=$1 AND warehouse_id=$2 AND usage='internal' AND active
             AND (metadata->>'deleted_at') IS NULL
           ORDER BY (metadata->>'created_at') NULLS LAST, id LIMIT 1"#,
    )
    .bind(company).bind(wh)
    .fetch_optional(pool).await.unwrap()
}

/// Quant on-hand at a location (0 when neither the location nor the quant exists yet —
/// a location the door never bootstrapped holds nothing by definition).
async fn on_hand_at(pool: &PgPool, company: Uuid, item: Uuid, loc: Option<Uuid>) -> Decimal {
    match loc {
        None => Decimal::ZERO,
        Some(l) => sqlx::query_scalar::<_, Decimal>(
            r#"SELECT COALESCE(SUM(quantity), 0) FROM inventory.stock_quants
               WHERE company_id=$1 AND item_id=$2 AND location_id=$3
                 AND (metadata->>'deleted_at') IS NULL"#,
        )
        .bind(company).bind(item).bind(l)
        .fetch_one(pool).await.unwrap(),
    }
}

/// The warehouse Bin's balance — the second estate (the pre-convergence record of truth).
async fn bin_balance(pool: &PgPool, company: Uuid, item: Uuid, wh: Uuid) -> (Decimal, Decimal) {
    let row = sqlx::query(
        "SELECT actual_qty, stock_value FROM inventory.bins WHERE company_id=$1 AND item_id=$2 AND warehouse_id=$3",
    )
    .bind(company).bind(item).bind(wh)
    .fetch_one(pool).await.unwrap();
    (row.get("actual_qty"), row.get("stock_value"))
}

/// The append-only ledger's sums at the warehouse grain: (Σ actual_qty, Σ value diff).
async fn ledger_sums(pool: &PgPool, company: Uuid, item: Uuid, wh: Uuid) -> (Decimal, Decimal) {
    sqlx::query_as(
        r#"SELECT COALESCE(SUM(actual_qty),0), COALESCE(SUM(stock_value_difference),0)
           FROM inventory.stock_ledger_entries
           WHERE company_id=$1 AND item_id=$2 AND warehouse_id=$3
             AND (metadata->>'deleted_at') IS NULL"#,
    )
    .bind(company).bind(item).bind(wh)
    .fetch_one(pool).await.unwrap()
}

/// The engine-minted moves a voucher door stamped under one origin (voucher number),
/// with their states — the door's physical legs must ALL live here.
async fn moves_of_origin(pool: &PgPool, company: Uuid, origin: &str) -> Vec<(String, String)> {
    sqlx::query(
        r#"SELECT name, state::text AS state FROM inventory.stock_moves
           WHERE company_id=$1 AND origin=$2 AND (metadata->>'deleted_at') IS NULL ORDER BY name"#,
    )
    .bind(company).bind(origin)
    .fetch_all(pool).await.unwrap()
    .iter()
    .map(|r| (r.get("name"), r.get("state")))
    .collect()
}

/// Count SLE rows the door wrote under its own voucher type (the legacy direct-write
/// signature — must be ZERO from now on; every leg rides the move that minted it).
async fn legacy_voucher_sle_rows(pool: &PgPool, company: Uuid, item: Uuid) -> i64 {
    sqlx::query_scalar::<_, i64>(
        r#"SELECT COUNT(*) FROM inventory.stock_ledger_entries
           WHERE company_id=$1 AND item_id=$2
             AND voucher_type IN ('purchase_receipt','delivery_note')
             AND (metadata->>'deleted_at') IS NULL"#,
    )
    .bind(company).bind(item)
    .fetch_one(pool).await.unwrap()
}

/// The full estate-agreement check — the detector's core. Asserts the two stock estates and
/// the ledger all tell the SAME story at the warehouse grain.
async fn assert_estates_agree(pool: &PgPool, company: Uuid, item: Uuid, wh: Uuid, what: &str) {
    let loc = stock_location(pool, company, wh).await;
    let on_hand = on_hand_at(pool, company, item, loc).await;
    let (bin_qty, bin_value) = bin_balance(pool, company, item, wh).await;
    let (led_qty, led_value) = ledger_sums(pool, company, item, wh).await;
    assert_eq!(on_hand, bin_qty,
        "{what}: quant on-hand at the stock location ({on_hand}) must equal the Bin qty ({bin_qty}) — the two stock estates diverged");
    assert_eq!(led_qty, bin_qty,
        "{what}: Σ SLE actual_qty ({led_qty}) must balance to the Bin qty ({bin_qty}) — the ledger diverged from the estate");
    assert_eq!(led_value, bin_value,
        "{what}: Σ SLE value diff ({led_value}) must balance to the Bin value ({bin_value}) — the ledger diverged from the estate");
}

// --- door fixtures --------------------------------------------------------------

async fn create_and_submit_receipt(
    w: &InventoryWriteService, company: Uuid, wh: Uuid, item: Uuid, qty: &str, rate: &str,
    sink: &dyn GlPostSink,
) -> Uuid {
    let id = w.create_purchase_receipt(NewReceipt {
        receipt_number: uq("PR"), company_id: company, branch_id: None,
        supplier_id: Uuid::new_v4(), source_po_id: None, warehouse_id: wh,
        posting_date: day(), currency: "IDR".into(),
        inventory_account_id: Uuid::new_v4(), grir_account_id: Uuid::new_v4(),
        lines: vec![ReceiptLine { item_id: item, quantity: d(qty), rate: d(rate) , is_landed_costs_line: false }],
    }).await.unwrap();
    w.submit_purchase_receipt(id, sink).await.unwrap();
    id
}

async fn create_and_submit_delivery(
    w: &InventoryWriteService, company: Uuid, wh: Uuid, item: Uuid, qty: &str,
    sink: &dyn GlPostSink,
) -> Uuid {
    let id = w.create_delivery_note(NewDelivery {
        delivery_number: uq("DN"), company_id: company, branch_id: None,
        customer_id: Uuid::new_v4(), source_so_id: None, warehouse_id: wh,
        posting_date: day(), currency: "IDR".into(),
        cogs_account_id: Uuid::new_v4(), inventory_account_id: Uuid::new_v4(),
        lines: vec![DeliveryLine { item_id: item, quantity: d(qty) }],
    }).await.unwrap();
    w.submit_delivery_note(id, sink).await.unwrap();
    id
}

// --- the detector tests ----------------------------------------------------------

/// GOODS-IN: submitting a purchase receipt must flip the quant on-hand at the stock location
/// by EXACTLY the received qty, keep both estates + the ledger in agreement, and emit a GL
/// leg that matches the value the move actually carried.
#[tokio::test]
async fn submit_receipt_moves_quants_and_ledger_together() {
    let pool = pool().await;
    let w = InventoryWriteService::new(pool.clone());
    let (company, item) = (Uuid::new_v4(), Uuid::new_v4());
    let wh = warehouse(&w, company).await;

    let (sink, posts) = RecordingSink::new();
    let before = on_hand_at(&pool, company, item, stock_location(&pool, company, wh).await).await;
    assert_eq!(before, Decimal::ZERO, "a fresh warehouse holds nothing");

    let receipt_number = {
        // create + submit through the door (10 units @ 100 → value 1000)
        let id = w.create_purchase_receipt(NewReceipt {
            receipt_number: uq("PR"), company_id: company, branch_id: None,
            supplier_id: Uuid::new_v4(), source_po_id: None, warehouse_id: wh,
            posting_date: day(), currency: "IDR".into(),
            inventory_account_id: Uuid::new_v4(), grir_account_id: Uuid::new_v4(),
            lines: vec![ReceiptLine { item_id: item, quantity: d("10"), rate: d("100") , is_landed_costs_line: false }],
        }).await.unwrap();
        w.submit_purchase_receipt(id, &sink).await.unwrap();
        sqlx::query_scalar::<_, String>(
            "SELECT receipt_number FROM inventory.purchase_receipts WHERE id=$1",
        ).bind(id).fetch_one(&pool).await.unwrap()
    };

    // (1) on-hand flipped by the door qty
    let loc = stock_location(&pool, company, wh).await.expect("door bootstrapped the stock location");
    let after = on_hand_at(&pool, company, item, Some(loc)).await;
    assert_eq!(after, before + d("10"), "quant on-hand at the stock location must move by the door qty");

    // (2) both estates + the ledger agree
    assert_estates_agree(&pool, company, item, wh, "after submit_receipt").await;
    let (_, bin_value) = bin_balance(&pool, company, item, wh).await;
    assert_eq!(bin_value, d("1000.00"));

    // (3) the GL leg matches what the moves carried (10 @ 100)
    let posts = posts.lock().unwrap().clone();
    assert_eq!(posts, vec![CapturedPost { posting_type: "original".into(), total: d("1000.00") }],
        "ONE door-owned envelope for the received value");

    // (4) the door's legs are engine-minted: one DONE move under the voucher origin,
    //     and ZERO ledger rows under the legacy voucher type.
    let moves = moves_of_origin(&pool, company, &receipt_number).await;
    assert_eq!(moves, vec![({format!("{receipt_number}/1")}.to_string(), "done".to_string())]);
    assert_eq!(legacy_voucher_sle_rows(&pool, company, item).await, 0,
        "the receipt door must not write SLE rows under its own voucher type any more");
}

/// GOODS-OUT: submitting a delivery note must flip the quant on-hand at the stock location
/// by EXACTLY the delivered qty (down), keep both estates + the ledger in agreement, and
/// emit a COGS leg that matches the value the move carried off.
#[tokio::test]
async fn submit_delivery_moves_quants_and_ledger_together() {
    let pool = pool().await;
    let w = InventoryWriteService::new(pool.clone());
    let (company, item) = (Uuid::new_v4(), Uuid::new_v4());
    let wh = warehouse(&w, company).await;

    let (rsink, _) = RecordingSink::new();
    create_and_submit_receipt(&w, company, wh, item, "10", "100", &rsink).await;

    let (sink, posts) = RecordingSink::new();
    let delivery_number = {
        let id = w.create_delivery_note(NewDelivery {
            delivery_number: uq("DN"), company_id: company, branch_id: None,
            customer_id: Uuid::new_v4(), source_so_id: None, warehouse_id: wh,
            posting_date: day(), currency: "IDR".into(),
            cogs_account_id: Uuid::new_v4(), inventory_account_id: Uuid::new_v4(),
            lines: vec![DeliveryLine { item_id: item, quantity: d("4") }],
        }).await.unwrap();
        w.submit_delivery_note(id, &sink).await.unwrap();
        sqlx::query_scalar::<_, String>(
            "SELECT delivery_number FROM inventory.delivery_notes WHERE id=$1",
        ).bind(id).fetch_one(&pool).await.unwrap()
    };

    // (1) on-hand flipped DOWN by the door qty
    let loc = stock_location(&pool, company, wh).await.expect("stock location");
    assert_eq!(on_hand_at(&pool, company, item, Some(loc)).await, d("6"),
        "quant on-hand must drop by exactly the delivered qty");

    // (2) both estates + the ledger agree
    assert_estates_agree(&pool, company, item, wh, "after submit_delivery").await;

    // (3) the COGS leg matches the value the move carried off (4 @ avg 100)
    let posts = posts.lock().unwrap().clone();
    assert_eq!(posts, vec![CapturedPost { posting_type: "original".into(), total: d("400.00") }]);
    let (_, bin_value) = bin_balance(&pool, company, item, wh).await;
    assert_eq!(bin_value, d("600.00"), "1000 received − 400 COGS");

    // (4) engine-minted legs; legacy signature gone
    let moves = moves_of_origin(&pool, company, &delivery_number).await;
    assert_eq!(moves, vec![({format!("{delivery_number}/1")}.to_string(), "done".to_string())]);
    assert_eq!(legacy_voucher_sle_rows(&pool, company, item).await, 0);
}

/// CANCEL GOODS-IN: cancelling a submitted receipt must mint REVERSE moves through the
/// engine — on-hand returns by exactly the received qty, the Bin reblends to its
/// pre-receipt state, the ledger compensates to zero, and the reversal GL leg matches.
#[tokio::test]
async fn cancel_receipt_reverses_through_the_engine_and_estates_agree() {
    let pool = pool().await;
    let w = InventoryWriteService::new(pool.clone());
    let (company, item) = (Uuid::new_v4(), Uuid::new_v4());
    let wh = warehouse(&w, company).await;

    let (rsink, _) = RecordingSink::new();
    let rid = create_and_submit_receipt(&w, company, wh, item, "10", "100", &rsink).await;
    let receipt_number = sqlx::query_scalar::<_, String>(
        "SELECT receipt_number FROM inventory.purchase_receipts WHERE id=$1",
    ).bind(rid).fetch_one(&pool).await.unwrap();
    let loc = stock_location(&pool, company, wh).await.expect("stock location");
    assert_eq!(on_hand_at(&pool, company, item, Some(loc)).await, d("10"));

    let (sink, posts) = RecordingSink::new();
    w.cancel_purchase_receipt(rid, &sink).await.unwrap();

    // (1) on-hand returned by exactly the door qty
    assert_eq!(on_hand_at(&pool, company, item, Some(loc)).await, Decimal::ZERO,
        "cancel must draw the received qty back out of the stock location");

    // (2) both estates + the ledger agree — the compensating entries balance to zero
    assert_estates_agree(&pool, company, item, wh, "after cancel_receipt").await;
    let (bin_qty, bin_value) = bin_balance(&pool, company, item, wh).await;
    assert_eq!((bin_qty, bin_value), (Decimal::ZERO, Decimal::ZERO),
        "the Bin returns to its pre-receipt state");

    // (3) the reversal GL leg matches the original value
    let posts = posts.lock().unwrap().clone();
    assert_eq!(posts, vec![CapturedPost { posting_type: "reversal".into(), total: d("1000.00") }]);

    // (4) the reverse leg is a DONE engine move under the SAME origin, named REV
    let mut moves = moves_of_origin(&pool, company, &receipt_number).await;
    moves.sort();
    assert_eq!(moves, vec![
        ({format!("{receipt_number}/1")}.to_string(), "done".to_string()),
        ({format!("{receipt_number}/REV/1")}.to_string(), "done".to_string()),
    ], "forward + reverse legs, both engine-minted and done");
    assert_eq!(legacy_voucher_sle_rows(&pool, company, item).await, 0);
}

/// CANCEL GOODS-OUT: cancelling a submitted delivery must mint REVERSE moves through the
/// engine — on-hand returns by exactly the delivered qty, the Bin reblends to its
/// pre-delivery value (the forced-value proof: the EXACT COGS comes back), and the
/// reversal GL leg matches.
#[tokio::test]
async fn cancel_delivery_reverses_through_the_engine_and_estates_agree() {
    let pool = pool().await;
    let w = InventoryWriteService::new(pool.clone());
    let (company, item) = (Uuid::new_v4(), Uuid::new_v4());
    let wh = warehouse(&w, company).await;

    let (rsink, _) = RecordingSink::new();
    create_and_submit_receipt(&w, company, wh, item, "10", "100", &rsink).await;
    let (dsink, _) = RecordingSink::new();
    let did = create_and_submit_delivery(&w, company, wh, item, "4", &dsink).await;
    let delivery_number = sqlx::query_scalar::<_, String>(
        "SELECT delivery_number FROM inventory.delivery_notes WHERE id=$1",
    ).bind(did).fetch_one(&pool).await.unwrap();
    let loc = stock_location(&pool, company, wh).await.expect("stock location");
    assert_eq!(on_hand_at(&pool, company, item, Some(loc)).await, d("6"));

    let (sink, posts) = RecordingSink::new();
    w.cancel_delivery_note(did, &sink).await.unwrap();

    // (1) on-hand returned by exactly the delivered qty
    assert_eq!(on_hand_at(&pool, company, item, Some(loc)).await, d("10"),
        "cancel must put the delivered qty back into the stock location");

    // (2) both estates + the ledger agree — and the Bin is restored to its pre-delivery value
    assert_estates_agree(&pool, company, item, wh, "after cancel_delivery").await;
    let (bin_qty, bin_value) = bin_balance(&pool, company, item, wh).await;
    assert_eq!((bin_qty, bin_value), (d("10"), d("1000.00")),
        "qty AND value return to the pre-delivery state (the exact COGS comes home)");

    // (3) the reversal GL leg matches the COGS the delivery posted
    let posts = posts.lock().unwrap().clone();
    assert_eq!(posts, vec![CapturedPost { posting_type: "reversal".into(), total: d("400.00") }]);

    // (4) the reverse leg is a DONE engine move under the SAME origin, named REV
    let mut moves = moves_of_origin(&pool, company, &delivery_number).await;
    moves.sort();
    assert_eq!(moves, vec![
        ({format!("{delivery_number}/1")}.to_string(), "done".to_string()),
        ({format!("{delivery_number}/REV/1")}.to_string(), "done".to_string()),
    ]);
    assert_eq!(legacy_voucher_sle_rows(&pool, company, item).await, 0);
}

/// A receipt whose goods were already issued cannot be cancelled — the refusal must leave
/// BOTH estates untouched (still in agreement, still holding the issued stock's record).
#[tokio::test]
async fn refused_receipt_cancel_leaves_both_estates_untouched() {
    let pool = pool().await;
    let w = InventoryWriteService::new(pool.clone());
    let (company, item) = (Uuid::new_v4(), Uuid::new_v4());
    let wh = warehouse(&w, company).await;

    // Receive 10 @ 100, deliver 8 — only 2 remain, so cancelling the receipt (which needs
    // all 10 back) must refuse.
    let (rsink, _) = RecordingSink::new();
    let rid = create_and_submit_receipt(&w, company, wh, item, "10", "100", &rsink).await;
    let (dsink, _) = RecordingSink::new();
    create_and_submit_delivery(&w, company, wh, item, "8", &dsink).await;

    let (sink, posts) = RecordingSink::new();
    let err = w.cancel_purchase_receipt(rid, &sink).await.unwrap_err();
    assert!(matches!(err, backbone_inventory::application::service::inventory_write_service::InventoryError::InsufficientStockToReverse { .. }),
        "got {err:?}");

    // The refusal changed nothing: on-hand still 2, estates still agree, no reversal posted.
    let loc = stock_location(&pool, company, wh).await.expect("stock location");
    assert_eq!(on_hand_at(&pool, company, item, Some(loc)).await, d("2"));
    assert_estates_agree(&pool, company, item, wh, "after a refused cancel").await;
    assert!(posts.lock().unwrap().is_empty(), "a refused cancel posts no GL");
}
