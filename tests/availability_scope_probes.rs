//! Availability-scope probes — the two-scope read at one warehouse pivot.
//!
//! Requires DATABASE_URL (:5433/backbone_inventory), inventory schema applied.
//!
//! Proves the four load-bearing postures of the availability surface:
//!   1. The sold-out verdict evaluates EVERY item in the set (the upstream
//!      first-variant-only template check is the bug class this refuses).
//!   2. Display and checkout are two scopes over the same fresh estate
//!      (checkout subtracts the caller's own holdings) and every read is
//!      computed per call — a stock write between reads moves the answer
//!      with no writer between them.
//!   3. There is NO all-warehouses fallback: stock in a sibling warehouse
//!      never leaks into the pivot's read; an unset pivot cannot even be
//!      asked (the pivot parameter is required), and a bogus pivot refuses
//!      typed instead of reading as all-sold-out.
//!   4. Nothing about readiness is persisted: no warning/readiness/sold-out
//!      column exists in the schema, and the reads leave no row state.

use rust_decimal::Decimal;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use backbone_inventory::application::service::{
    AvailabilityScopeError, AvailabilityScopePort, AvailabilityScopeRead, CheckoutDemand,
    InventoryWriteService, NewDelivery, NewReceipt, NewWarehouse, ReceiptLine, DeliveryLine,
};

struct StubGl;
#[async_trait::async_trait]
impl backbone_inventory::application::service::inventory_gl::GlPostSink for StubGl {
    async fn post(
        &self,
        _e: &backbone_inventory::application::service::inventory_gl::AccountingPostEnvelope,
    ) -> Result<
        backbone_inventory::application::service::inventory_gl::GlPostAck,
        backbone_inventory::application::service::inventory_gl::GlPostRejected,
    > {
        Ok(backbone_inventory::application::service::inventory_gl::GlPostAck {
            post_id: Uuid::new_v4(),
            journal_id: Uuid::new_v4(),
            idempotent_reuse: false,
        })
    }
}

fn d(s: &str) -> Decimal {
    Decimal::from_str_exact(s).unwrap()
}
fn day() -> chrono::NaiveDate {
    chrono::NaiveDate::from_ymd_opt(2026, 9, 4).unwrap()
}
fn uq(p: &str) -> String {
    format!("{p}-{}", &Uuid::new_v4().simple().to_string()[..8])
}
async fn pool() -> PgPool {
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgresql://postgres:postgres@localhost:5433/backbone_inventory".to_string()
    });
    PgPool::connect(&url).await.expect("connect DB")
}
async fn warehouse(w: &InventoryWriteService, is_group: bool) -> Uuid {
    w.create_warehouse(NewWarehouse {
        code: uq("WH"),
        name: uq("Main"),
        warehouse_type: None,
        parent_warehouse_id: None,
        is_group,
    })
    .await
    .unwrap()
}
async fn receive(w: &InventoryWriteService, wh: Uuid, item: Uuid, qty: Decimal) {
    let rid = w
        .create_purchase_receipt(NewReceipt {
            receipt_number: uq("PR"),
            branch_id: None,
            supplier_id: Uuid::new_v4(),
            source_po_id: None,
            warehouse_id: wh,
            posting_date: day(),
            currency: "IDR".into(),
            inventory_account_id: Uuid::new_v4(),
            grir_account_id: Uuid::new_v4(),
            lines: vec![ReceiptLine {
                item_id: item,
                quantity: qty,
                rate: d("100"),
                is_landed_costs_line: false,
            }],
        })
        .await
        .unwrap();
    w.submit_purchase_receipt(rid, &StubGl).await.unwrap();
}
async fn deliver(w: &InventoryWriteService, wh: Uuid, item: Uuid, qty: Decimal) {
    let did = w
        .create_delivery_note(NewDelivery {
            delivery_number: uq("DN"),
            branch_id: None,
            customer_id: Uuid::new_v4(),
            source_so_id: None,
            warehouse_id: wh,
            posting_date: day(),
            currency: "IDR".into(),
            cogs_account_id: Uuid::new_v4(),
            inventory_account_id: Uuid::new_v4(),
            lines: vec![DeliveryLine { item_id: item, quantity: qty }],
        })
        .await
        .unwrap();
    w.submit_delivery_note(did, &StubGl).await.unwrap();
}

// ASP-1: the sold-out verdict evaluates EVERY item in the set. The set below is a
// multi-variant template's full variant list with the FIRST variant sold out and a later
// variant holding stock — upstream's first-variant-only check mislabels this sold out;
// the verdict here is false. Exhausting the holding variant flips it true.
#[tokio::test]
async fn sold_out_verdict_evaluates_every_variant() {
    let pool = pool().await;
    let w = InventoryWriteService::new(pool.clone());
    let read = AvailabilityScopeRead::new(pool.clone());
    let wh = warehouse(&w, false).await;
    // The template's variant set: v1 (never received), v2 (holding 5), v3 (never received).
    let (v1, v2, v3) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    receive(&w, wh, v2, d("5")).await;

    let verdict = read.sold_out(&[v1, v2, v3], wh).await.unwrap();
    // First-variant-only would answer true (v1 has nothing). Every-variant answers false.
    assert!(!verdict.sold_out, "v2 holds stock: the set is not sold out");
    // The verdict is complete over the input set, in input order.
    assert_eq!(verdict.per_item.len(), 3);
    assert_eq!(verdict.per_item[0].item_id, v1);
    assert_eq!(verdict.per_item[0].available_qty, d("0"));
    assert_eq!(verdict.per_item[1].item_id, v2);
    assert_eq!(verdict.per_item[1].available_qty, d("5"));
    assert_eq!(verdict.per_item[2].item_id, v3);
    assert_eq!(verdict.per_item[2].available_qty, d("0"));

    // Exhaust the holding variant: NOW every member reads zero → sold out.
    deliver(&w, wh, v2, d("5")).await;
    let exhausted = read.sold_out(&[v1, v2, v3], wh).await.unwrap();
    assert!(exhausted.sold_out, "no variant holds availability");
}

// ASP-2: display vs checkout are two scopes over the same estate, and both are computed
// FRESH per call — a stock write between reads moves the answer with no writer between
// them (no materialized scope state anywhere).
#[tokio::test]
async fn display_and_checkout_scopes_differ_and_stay_fresh() {
    let pool = pool().await;
    let w = InventoryWriteService::new(pool.clone());
    let read = AvailabilityScopeRead::new(pool.clone());
    let wh = warehouse(&w, false).await;
    let item = Uuid::new_v4();
    receive(&w, wh, item, d("10")).await;

    // Display scope: the raw fresh availability.
    let display = read
        .display_availability(&[item], wh)
        .await
        .unwrap();
    assert_eq!(display[0].on_hand_qty, d("10"));
    assert_eq!(display[0].available_qty, d("10"));

    // Checkout scope: the same freshness minus the caller's own holdings.
    let with_six_held = read
        .checkout_free_qty(
            &[CheckoutDemand { item_id: item, held_qty: d("6") }],
            wh,
        )
        .await
        .unwrap();
    assert_eq!(with_six_held[0].available_qty, d("10"));
    assert_eq!(with_six_held[0].free_qty, d("4"));

    // Held beyond availability floors at zero — never negative.
    let over_held = read
        .checkout_free_qty(
            &[CheckoutDemand { item_id: item, held_qty: d("12") }],
            wh,
        )
        .await
        .unwrap();
    assert_eq!(over_held[0].free_qty, d("0"));

    // Freshness: deliver 4 with NO write between the reads around it — both scopes move.
    deliver(&w, wh, item, d("4")).await;
    let display_after = read
        .display_availability(&[item], wh)
        .await
        .unwrap();
    assert_eq!(display_after[0].on_hand_qty, d("6"), "read tracks the estate per call");
    let checkout_after = read
        .checkout_free_qty(
            &[CheckoutDemand { item_id: item, held_qty: d("6") }],
            wh,
        )
        .await
        .unwrap();
    assert_eq!(checkout_after[0].available_qty, d("6"));
    assert_eq!(checkout_after[0].free_qty, d("0"));
}

// ASP-3: NO all-warehouses fallback. Stock in a sibling warehouse never leaks into the
// pivot's read — a shop pinned to W1 stays sold out for an item only W2 holds.
#[tokio::test]
async fn no_all_warehouses_fallback_at_the_pivot() {
    let pool = pool().await;
    let w = InventoryWriteService::new(pool.clone());
    let read = AvailabilityScopeRead::new(pool.clone());
    let w1 = warehouse(&w, false).await;
    let w2 = warehouse(&w, false).await;
    let item = Uuid::new_v4();
    // All 10 units sit in W2; W1 holds nothing.
    receive(&w, w2, item, d("10")).await;

    let at_shop = read.display_availability(&[item], w1).await.unwrap();
    assert_eq!(at_shop[0].available_qty, d("0"), "W2's stock must not leak into W1");
    let verdict = read.sold_out(&[item], w1).await.unwrap();
    assert!(verdict.sold_out, "sold out at the shop pivot despite stock in W2");

    let at_w2 = read.display_availability(&[item], w2).await.unwrap();
    assert_eq!(at_w2[0].available_qty, d("10"));
}

// ASP-4: the pivot refusals are typed and fail-loud — a bogus pivot never reads as a
// silently-empty (all-sold-out) storefront.
#[tokio::test]
async fn pivot_refusals_are_typed() {
    let pool = pool().await;
    let w = InventoryWriteService::new(pool.clone());
    let read = AvailabilityScopeRead::new(pool.clone());
    let wh = warehouse(&w, false).await;
    let item = Uuid::new_v4();

    // A pivot warehouse that does not exist.
    let bogus = Uuid::new_v4();
    assert!(matches!(
        read.display_availability(&[item], bogus).await,
        Err(AvailabilityScopeError::UnknownWarehouse(w)) if w == bogus
    ));

    // A warehouse GROUP as the pivot — a grouping node is not a stock warehouse.
    let group = warehouse(&w, true).await;
    assert!(matches!(
        read.display_availability(&[item], group).await,
        Err(AvailabilityScopeError::GroupPivot(g)) if g == group
    ));

    // Empty and duplicate item sets refuse — a vacuous read is a failure, never a green tick.
    assert!(matches!(
        read.display_availability(&[], wh).await,
        Err(AvailabilityScopeError::EmptyItemSet)
    ));
    assert!(matches!(
        read.sold_out(&[], wh).await,
        Err(AvailabilityScopeError::EmptyItemSet)
    ));
    assert!(matches!(
        read.checkout_free_qty(&[], wh).await,
        Err(AvailabilityScopeError::EmptyItemSet)
    ));
    assert!(matches!(
        read.display_availability(&[item, item], wh).await,
        Err(AvailabilityScopeError::DuplicateItem(i)) if i == item
    ));
    assert!(matches!(
        read.checkout_free_qty(
            &[
                CheckoutDemand { item_id: item, held_qty: d("1") },
                CheckoutDemand { item_id: item, held_qty: d("2") },
            ],
            wh
        )
        .await,
        Err(AvailabilityScopeError::DuplicateItem(i)) if i == item
    ));

    // A negative held quantity refuses.
    assert!(matches!(
        read.checkout_free_qty(
            &[CheckoutDemand { item_id: item, held_qty: d("-1") }],
            wh
        )
        .await,
        Err(AvailabilityScopeError::NegativeHeld(i)) if i == item
    ));
}

// ASP-5: nothing about a readiness check is persisted. No warning/readiness/sold-out
// column exists anywhere in the schema (the upstream payment-readiness gate that
// persisted shop_warning writes during a validation probe is the anti-shape), and the
// reads themselves leave no row state — the quant rows' audit stamps do not move across
// a batch of availability reads.
#[tokio::test]
async fn no_materialized_readiness_state_and_reads_write_nothing() {
    let pool = pool().await;
    let w = InventoryWriteService::new(pool.clone());
    let read = AvailabilityScopeRead::new(pool.clone());
    let wh = warehouse(&w, false).await;
    let item = Uuid::new_v4();
    receive(&w, wh, item, d("3")).await;

    // No persisted readiness vocabulary exists in the inventory schema.
    let persisted: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM information_schema.columns
           WHERE table_schema = 'inventory'
             AND (column_name ILIKE '%shop_warning%'
                  OR column_name ILIKE '%readiness%'
                  OR column_name ILIKE '%sold_out%')"#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(persisted, 0, "no readiness/warning/sold-out column may exist");

    // A barrage of reads across every scope moves no row state: the quant audit
    // fingerprint (count + max updated_at) is identical before and after. Scoped
    // to this test's item — sibling tests write their own items concurrently.
    let before: (i64, Option<String>) = sqlx::query(
        r#"SELECT COUNT(*), MAX(metadata->>'updated_at')::text FROM inventory.stock_quants
           WHERE item_id = $1"#,
    )
    .bind(item)
    .fetch_one(&pool)
    .await
    .map(|r| (r.get(0), r.get(1)))
    .unwrap();

    for _ in 0..3 {
        let _ = read.display_availability(&[item], wh).await.unwrap();
        let _ = read
            .checkout_free_qty(
                &[CheckoutDemand { item_id: item, held_qty: d("1") }],
                wh,
            )
            .await
            .unwrap();
        let _ = read.sold_out(&[item], wh).await.unwrap();
    }

    let after: (i64, Option<String>) = sqlx::query(
        r#"SELECT COUNT(*), MAX(metadata->>'updated_at')::text FROM inventory.stock_quants
           WHERE item_id = $1"#,
    )
    .bind(item)
    .fetch_one(&pool)
    .await
    .map(|r| (r.get(0), r.get(1)))
    .unwrap();
    assert_eq!(before, after, "availability reads must not write anything");
}
