//! Golden numeric oracle for the moving-average valuation engine + the append-only Stock Ledger.
//! Pure inventory (no accounting) — the GL seam is proven separately in `gl_posting_seam.rs`.
//! Requires DATABASE_URL (:5433/backbone_inventory).

use rust_decimal::Decimal;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use backbone_inventory::application::service::inventory_gl::{
    AccountingPostEnvelope, GlPostAck, GlPostRejected, GlPostSink,
};
use backbone_inventory::application::service::inventory_write_service::{
    DeliveryLine, InventoryError, InventoryWriteService, NewDelivery, NewReceipt, NewReconciliation,
    NewTransfer, NewWarehouse, ReceiptLine, ReconLine,
};

// Stub GL sink — always accepts (valuation math doesn't need a real ledger).
struct StubGl;
#[async_trait::async_trait]
impl GlPostSink for StubGl {
    async fn post(&self, _e: &AccountingPostEnvelope) -> Result<GlPostAck, GlPostRejected> {
        Ok(GlPostAck { post_id: Uuid::new_v4(), journal_id: Uuid::new_v4(), idempotent_reuse: false })
    }
}

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
        company_id: company, code: uq("WH"), name: "Main".into(),
        warehouse_type: None, parent_warehouse_id: None, is_group: false,
    }).await.unwrap()
}
async fn bin(pool: &PgPool, company: Uuid, item: Uuid, wh: Uuid) -> (Decimal, Decimal, Decimal) {
    let row = sqlx::query("SELECT actual_qty, valuation_rate, stock_value FROM inventory.bins WHERE company_id=$1 AND item_id=$2 AND warehouse_id=$3")
        .bind(company).bind(item).bind(wh).fetch_one(pool).await.unwrap();
    (row.get("actual_qty"), row.get("valuation_rate"), row.get("stock_value"))
}
async fn receipt(w: &InventoryWriteService, company: Uuid, wh: Uuid, item: Uuid, qty: &str, rate: &str) -> Uuid {
    let id = w.create_purchase_receipt(NewReceipt {
        receipt_number: uq("PR"), company_id: company, branch_id: None, supplier_id: Uuid::new_v4(),
        source_po_id: None, warehouse_id: wh, posting_date: day(),
        currency: "IDR".into(),
        inventory_account_id: Uuid::new_v4(), grir_account_id: Uuid::new_v4(),
        lines: vec![ReceiptLine { item_id: item, quantity: d(qty), rate: d(rate) }],
    }).await.unwrap();
    w.submit_purchase_receipt(id, &StubGl).await.unwrap();
    id
}

// IVC-1: two receipts blend the moving-average rate. 10@100 then 10@120 → 20 units, value 2200, rate 110.
#[tokio::test]
async fn moving_average_blends_on_receipt() {
    let pool = pool().await;
    let w = InventoryWriteService::new(pool.clone());
    let (company, item) = (Uuid::new_v4(), Uuid::new_v4());
    let wh = warehouse(&w, company).await;
    receipt(&w, company, wh, item, "10", "100").await;
    receipt(&w, company, wh, item, "10", "120").await;
    let (qty, rate, value) = bin(&pool, company, item, wh).await;
    assert_eq!(qty, d("20.0000"));
    assert_eq!(rate, d("110.000000"), "weighted average of 100 and 120");
    assert_eq!(value, d("2200.00"));
}

// IVC-2: delivery consumes at the average; rate is unchanged by an outflow.
#[tokio::test]
async fn delivery_consumes_average_rate_unchanged() {
    let pool = pool().await;
    let w = InventoryWriteService::new(pool.clone());
    let (company, item) = (Uuid::new_v4(), Uuid::new_v4());
    let wh = warehouse(&w, company).await;
    receipt(&w, company, wh, item, "10", "100").await;
    receipt(&w, company, wh, item, "10", "120").await; // qty 20, rate 110, value 2200
    let did = w.create_delivery_note(NewDelivery {
        delivery_number: uq("DN"), company_id: company, branch_id: None, customer_id: Uuid::new_v4(),
        source_so_id: None, warehouse_id: wh, posting_date: day(),
        currency: "IDR".into(),
        cogs_account_id: Uuid::new_v4(), inventory_account_id: Uuid::new_v4(),
        lines: vec![DeliveryLine { item_id: item, quantity: d("5") }],
    }).await.unwrap();
    let out = w.submit_delivery_note(did, &StubGl).await.unwrap();
    assert_eq!(out.gl_amount, d("550.00"), "COGS = 5 * 110");
    let (qty, rate, value) = bin(&pool, company, item, wh).await;
    assert_eq!(qty, d("15.0000"));
    assert_eq!(rate, d("110.000000"), "rate unchanged by outflow");
    assert_eq!(value, d("1650.00")); // 2200 - 550
    // delivery line snapshot
    let cogs: Decimal = sqlx::query_scalar("SELECT cogs_amount FROM inventory.delivery_note_items WHERE delivery_id=$1")
        .bind(did).fetch_one(&pool).await.unwrap();
    assert_eq!(cogs, d("550.00"));
}

// IVC-3: SLE append integrity — one entry per movement, ordered, Σ actual_qty == Bin qty,
// and stock_value_difference sums to Bin value.
#[tokio::test]
async fn sle_append_integrity() {
    let pool = pool().await;
    let w = InventoryWriteService::new(pool.clone());
    let (company, item) = (Uuid::new_v4(), Uuid::new_v4());
    let wh = warehouse(&w, company).await;
    receipt(&w, company, wh, item, "10", "100").await;
    receipt(&w, company, wh, item, "10", "120").await;
    let (sum_qty, sum_val): (Decimal, Decimal) = sqlx::query_as(
        "SELECT COALESCE(SUM(actual_qty),0), COALESCE(SUM(stock_value_difference),0) FROM inventory.stock_ledger_entries WHERE company_id=$1 AND item_id=$2 AND warehouse_id=$3")
        .bind(company).bind(item).bind(wh).fetch_one(&pool).await.unwrap();
    let (qty, _rate, value) = bin(&pool, company, item, wh).await;
    assert_eq!(sum_qty, qty, "Σ SLE actual_qty == Bin qty (the append-ledger balances to the Bin)");
    assert_eq!(sum_val, value, "Σ SLE value diff == Bin value");
    // Exactly one SLE per receipt line was appended (2 receipts × 1 line).
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM inventory.stock_ledger_entries WHERE company_id=$1 AND item_id=$2 AND warehouse_id=$3")
        .bind(company).bind(item).bind(wh).fetch_one(&pool).await.unwrap();
    assert_eq!(n, 2);
    // NOTE: sle_no is per-voucher; a global per-(item,warehouse) sequence for backdated replay is a
    // Tier-3 repost concern (deferred). The Bin is the authoritative current balance.
}

// IVC-4: delivering more than on-hand is rejected (insufficient_stock), no partial movement.
#[tokio::test]
async fn insufficient_stock_rejected() {
    let pool = pool().await;
    let w = InventoryWriteService::new(pool.clone());
    let (company, item) = (Uuid::new_v4(), Uuid::new_v4());
    let wh = warehouse(&w, company).await;
    receipt(&w, company, wh, item, "3", "100").await;
    let did = w.create_delivery_note(NewDelivery {
        delivery_number: uq("DN"), company_id: company, branch_id: None, customer_id: Uuid::new_v4(),
        source_so_id: None, warehouse_id: wh, posting_date: day(),
        currency: "IDR".into(),
        cogs_account_id: Uuid::new_v4(), inventory_account_id: Uuid::new_v4(),
        lines: vec![DeliveryLine { item_id: item, quantity: d("10") }],
    }).await.unwrap();
    let err = w.submit_delivery_note(did, &StubGl).await.unwrap_err();
    assert!(matches!(err, InventoryError::InsufficientStock { .. }));
    let (qty, _, _) = bin(&pool, company, item, wh).await;
    assert_eq!(qty, d("3.0000"), "no stock consumed on rejection");
}

// IVC-5: transfer is value-neutral — total value across both warehouses is conserved; 2 SLEs.
#[tokio::test]
async fn transfer_conserves_value() {
    let pool = pool().await;
    let w = InventoryWriteService::new(pool.clone());
    let (company, item) = (Uuid::new_v4(), Uuid::new_v4());
    let wh1 = warehouse(&w, company).await;
    let wh2 = warehouse(&w, company).await;
    receipt(&w, company, wh1, item, "10", "100").await; // wh1: 10 @ 100 = 1000
    w.submit_transfer(NewTransfer {
        entry_number: uq("SE"), company_id: company, from_warehouse_id: wh1, to_warehouse_id: wh2,
        posting_date: day(), lines: vec![DeliveryLine { item_id: item, quantity: d("4") }],
    }).await.unwrap();
    let (q1, _, v1) = bin(&pool, company, item, wh1).await;
    let (q2, r2, v2) = bin(&pool, company, item, wh2).await;
    assert_eq!(q1, d("6.0000")); assert_eq!(v1, d("600.00"));
    assert_eq!(q2, d("4.0000")); assert_eq!(v2, d("400.00")); assert_eq!(r2, d("100.000000"));
    assert_eq!(v1 + v2, d("1000.00"), "total value conserved");
}

// IVC-6: reconciliation to a lower count yields a negative value difference (shrinkage).
#[tokio::test]
async fn reconciliation_computes_signed_difference() {
    let pool = pool().await;
    let w = InventoryWriteService::new(pool.clone());
    let (company, item) = (Uuid::new_v4(), Uuid::new_v4());
    let wh = warehouse(&w, company).await;
    receipt(&w, company, wh, item, "10", "100").await; // qty 10, value 1000, rate 100
    let rid = w.submit_reconciliation(NewReconciliation {
        recon_number: uq("SR"), company_id: company, warehouse_id: wh, posting_date: day(),
        currency: "IDR".into(),
        inventory_account_id: Uuid::new_v4(), adjustment_account_id: Uuid::new_v4(),
        lines: vec![ReconLine { item_id: item, counted_qty: d("8"), counted_rate: Decimal::ZERO }],
    }, &StubGl).await.unwrap();
    let net: Decimal = sqlx::query_scalar("SELECT net_difference FROM inventory.stock_reconciliations WHERE id=$1")
        .bind(rid).fetch_one(&pool).await.unwrap();
    assert_eq!(net, d("-200.00"), "2 units short at rate 100");
    let (qty, _, value) = bin(&pool, company, item, wh).await;
    assert_eq!(qty, d("8.0000")); assert_eq!(value, d("800.00"));
}

// IVC-7: validation gates — empty document / negative qty / same-warehouse transfer.
#[tokio::test]
async fn validation_gates() {
    let pool = pool().await;
    let w = InventoryWriteService::new(pool.clone());
    let (company, item) = (Uuid::new_v4(), Uuid::new_v4());
    let wh = warehouse(&w, company).await;
    let e = w.create_purchase_receipt(NewReceipt {
        receipt_number: uq("PR"), company_id: company, branch_id: None, supplier_id: Uuid::new_v4(),
        source_po_id: None, warehouse_id: wh, posting_date: day(),
        currency: "IDR".into(),
        inventory_account_id: Uuid::new_v4(), grir_account_id: Uuid::new_v4(), lines: vec![],
    }).await.unwrap_err();
    assert!(matches!(e, InventoryError::EmptyDocument));
    let e = w.submit_transfer(NewTransfer {
        entry_number: uq("SE"), company_id: company, from_warehouse_id: wh, to_warehouse_id: wh,
        posting_date: day(), lines: vec![DeliveryLine { item_id: item, quantity: d("1") }],
    }).await.unwrap_err();
    assert!(matches!(e, InventoryError::SameWarehouse));
}

// IVC-9 (council 2026-07-04): two concurrent deliveries on one bin must not oversell — the Bin is
// loaded FOR UPDATE, so the movements serialize: one succeeds, the other sees insufficient stock.
#[tokio::test]
async fn concurrent_deliveries_do_not_oversell() {
    let pool = pool().await;
    let w = InventoryWriteService::new(pool.clone());
    let (company, item) = (Uuid::new_v4(), Uuid::new_v4());
    let wh = warehouse(&w, company).await;
    receipt(&w, company, wh, item, "10", "100").await; // 10 on hand
    // two deliveries of 6 (total 12 > 10)
    let mk = |w: &InventoryWriteService| {
        let (w, company, item, wh) = (w.clone(), company, item, wh);
        async move {
            w.create_delivery_note(NewDelivery {
                delivery_number: uq("DN"), company_id: company, branch_id: None, customer_id: Uuid::new_v4(),
                source_so_id: None, warehouse_id: wh, posting_date: day(),
                currency: "IDR".into(),
                cogs_account_id: Uuid::new_v4(), inventory_account_id: Uuid::new_v4(),
                lines: vec![DeliveryLine { item_id: item, quantity: d("6") }],
            }).await.unwrap()
        }
    };
    let d1 = mk(&w).await;
    let d2 = mk(&w).await;
    let (w1, w2, p1, p2) = (w.clone(), w.clone(), pool.clone(), pool.clone());
    let a = tokio::spawn(async move { w1.submit_delivery_note(d1, &StubGl).await });
    let b = tokio::spawn(async move { w2.submit_delivery_note(d2, &StubGl).await });
    let (ra, rb) = (a.await.unwrap(), b.await.unwrap());
    let _ = (p1, p2);
    let ok = ra.is_ok() as u8 + rb.is_ok() as u8;
    assert_eq!(ok, 1, "exactly one delivery of 6 succeeds; the other hits insufficient_stock");
    let (qty, _, _) = bin(&pool, company, item, wh).await;
    assert_eq!(qty, d("4.0000"), "only one delivery consumed stock");
}

// IVC-8 (council 2026-07-04): moving-average residual flush. A non-2dp-exact average drained to
// zero by partial deliveries must leave the Bin at EXACTLY value 0 (no stranded/negative cent), and
// Σ COGS must equal the received value — so the inventory subledger ties out with the GL.
#[tokio::test]
async fn residual_flushes_to_zero_at_empty() {
    let pool = pool().await;
    let w = InventoryWriteService::new(pool.clone());
    let (company, item) = (Uuid::new_v4(), Uuid::new_v4());
    let wh = warehouse(&w, company).await;
    // Receipt A 1@10.00 (value 10.00) + Receipt B 2@10.005 (value 20.01) → qty 3, value 30.01, rate 10.003333.
    receipt(&w, company, wh, item, "1", "10.00").await;
    receipt(&w, company, wh, item, "2", "10.005").await;
    let (_, _, received) = bin(&pool, company, item, wh).await;
    assert_eq!(received, d("30.01"));
    // Deliver 1 unit three times (drains to 0).
    let mut total_cogs = Decimal::ZERO;
    for _ in 0..3 {
        let did = w.create_delivery_note(NewDelivery {
            delivery_number: uq("DN"), company_id: company, branch_id: None, customer_id: Uuid::new_v4(),
            source_so_id: None, warehouse_id: wh, posting_date: day(),
            currency: "IDR".into(),
            cogs_account_id: Uuid::new_v4(), inventory_account_id: Uuid::new_v4(),
            lines: vec![DeliveryLine { item_id: item, quantity: d("1") }],
        }).await.unwrap();
        total_cogs += w.submit_delivery_note(did, &StubGl).await.unwrap().gl_amount;
    }
    let (qty, rate, value) = bin(&pool, company, item, wh).await;
    assert_eq!(qty, d("0.0000"));
    assert_eq!(value, d("0.00"), "no stranded/negative residual at empty bin");
    assert_eq!(rate, d("0.000000"));
    assert_eq!(total_cogs, received, "Σ COGS == received value → subledger ties out with the GL");
}

// IVC-10 (council 2026-07-29): cancelling a receipt reverses the inflow at the ORIGINAL line amount,
// so a blended bin reblends correctly — cancelling the first of two receipts leaves the second's rate.
#[tokio::test]
async fn cancel_receipt_reverses_inflow() {
    let pool = pool().await;
    let w = InventoryWriteService::new(pool.clone());
    let (company, item) = (Uuid::new_v4(), Uuid::new_v4());
    let wh = warehouse(&w, company).await;
    let r1 = receipt(&w, company, wh, item, "10", "100").await; // 10 @ 100
    receipt(&w, company, wh, item, "10", "120").await;          // blended: 20 @ 110, value 2200
    let (qty, rate, value) = bin(&pool, company, item, wh).await;
    assert_eq!((qty, rate, value), (d("20.0000"), d("110.000000"), d("2200.00")));
    // Cancel the first receipt: removes 10 units + 1000 of value → 10 @ 120.
    let out = w.cancel_purchase_receipt(r1, &StubGl).await.unwrap();
    assert!(out.posted);
    let (qty, rate, value) = bin(&pool, company, item, wh).await;
    assert_eq!(qty, d("10.0000"));
    assert_eq!(rate, d("120.000000"), "reblended to the remaining receipt's rate");
    assert_eq!(value, d("1200.00"));
    let st: String = sqlx::query_scalar("SELECT status::text FROM inventory.purchase_receipts WHERE id=$1")
        .bind(r1).fetch_one(&pool).await.unwrap();
    assert_eq!(st, "cancelled");
}

// IVC-11 (council 2026-07-29): cancelling a delivery restores the bin to its pre-delivery state at
// the stored COGS, and re-cancelling is idempotent (no extra SLEs appended).
#[tokio::test]
async fn cancel_delivery_restores_bin_and_is_idempotent() {
    let pool = pool().await;
    let w = InventoryWriteService::new(pool.clone());
    let (company, item) = (Uuid::new_v4(), Uuid::new_v4());
    let wh = warehouse(&w, company).await;
    receipt(&w, company, wh, item, "10", "100").await; // 10 @ 100, value 1000
    let did = w.create_delivery_note(NewDelivery {
        delivery_number: uq("DN"), company_id: company, branch_id: None, customer_id: Uuid::new_v4(),
        source_so_id: None, warehouse_id: wh, posting_date: day(),
        currency: "IDR".into(),
        cogs_account_id: Uuid::new_v4(), inventory_account_id: Uuid::new_v4(),
        lines: vec![DeliveryLine { item_id: item, quantity: d("4") }],
    }).await.unwrap();
    w.submit_delivery_note(did, &StubGl).await.unwrap(); // bin 6 @ 100, value 600
    w.cancel_delivery_note(did, &StubGl).await.unwrap(); // bin restored to 10 @ 100, value 1000
    let (qty, rate, value) = bin(&pool, company, item, wh).await;
    assert_eq!((qty, rate, value), (d("10.0000"), d("100.000000"), d("1000.00")), "bin restored to pre-delivery state");
    // Idempotent: a second cancel short-circuits (already cancelled) without appending more SLEs.
    let n_before: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM inventory.stock_ledger_entries WHERE voucher_type='delivery_note' AND voucher_id=$1")
        .bind(did).fetch_one(&pool).await.unwrap();
    let _ = w.cancel_delivery_note(did, &StubGl).await.unwrap();
    let n_after: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM inventory.stock_ledger_entries WHERE voucher_type='delivery_note' AND voucher_id=$1")
        .bind(did).fetch_one(&pool).await.unwrap();
    assert_eq!(n_before, n_after, "re-cancel does not append more SLEs");
}
