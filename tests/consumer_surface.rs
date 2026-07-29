//! The consumer-facing surface the selling↔inventory delivery seam binds to (council 2026-07-04
//! completeness): the availability read-model, the DeliveryRequested intake, and the resolved
//! InventoryEvent name-collision. Requires DATABASE_URL (:5433/backbone_inventory).

use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

// The stable public surface a consumer binds to is `backbone_inventory::application::service::*`
// (the CUSTOM-protected, regen-safe re-exports). The generated `exports/` tree is unwired
// scaffolding (`pub mod exports` is absent from lib.rs in every module), so the semantic types here
// are the ONLY reachable ones — no collision with the generated CRUD `InventoryEvent`.
use backbone_inventory::application::service::{
    AvailabilityView, DeliveryIntake, DeliveryRequestLine, DeliveryRequested, InventoryEvent,
    InventoryReadService, InventoryWriteService, NewReceipt, NewWarehouse, ReceiptLine, StockBalance,
    StockDelivered,
};
use backbone_inventory::application::service::inventory_gl::{
    AccountingPostEnvelope, GlPostAck, GlPostRejected, GlPostSink,
};

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

// CS-1: the public `InventoryEvent` (application::service) is the SEMANTIC enum — constructing
// `::StockDelivered` FAILS TO COMPILE if it were the generated CRUD enum (no such variant). So
// compiling is the proof the consumer binds to the right type (collision moot: exports is unwired).
#[test]
fn public_inventory_event_is_the_semantic_enum() {
    let e: InventoryEvent = InventoryEvent::StockDelivered(StockDelivered {
        delivery_id: Uuid::new_v4(), company_id: Uuid::new_v4(), warehouse_id: Uuid::new_v4(),
        source_so_id: Some(Uuid::new_v4()), total_cogs: d("100.00"),
    });
    assert!(matches!(e, InventoryEvent::StockDelivered(_)));
}

// CS-2: availability read-model — after receiving 10, available_qty is 10 (actual − reserved).
#[tokio::test]
async fn availability_reflects_received_stock() {
    let pool = pool().await;
    let w = InventoryWriteService::new(pool.clone());
    let read = InventoryReadService::new(pool.clone());
    let (company, item) = (Uuid::new_v4(), Uuid::new_v4());
    let wh = w.create_warehouse(NewWarehouse { company_id: company, code: uq("WH"), name: "Main".into(), warehouse_type: None, parent_warehouse_id: None, is_group: false }).await.unwrap();

    // Before any receipt: an un-stocked item is available 0 (not an error).
    let a0: AvailabilityView = read.availability(company, item, wh).await.unwrap();
    assert_eq!(a0.available_qty, d("0"));

    let rid = w.create_purchase_receipt(NewReceipt {
        receipt_number: uq("PR"), company_id: company, branch_id: None, supplier_id: Uuid::new_v4(),
        source_po_id: None, warehouse_id: wh, posting_date: day(),
        currency: "IDR".into(),
        inventory_account_id: Uuid::new_v4(), grir_account_id: Uuid::new_v4(),
        lines: vec![ReceiptLine { item_id: item, quantity: d("10"), rate: d("100") }],
    }).await.unwrap();
    w.submit_purchase_receipt(rid, &StubGl).await.unwrap();

    let a: AvailabilityView = read.availability(company, item, wh).await.unwrap();
    assert_eq!(a.actual_qty, d("10.0000"));
    assert_eq!(a.reserved_qty, d("0.0000"));
    assert_eq!(a.available_qty, d("10.0000"));
    let sb: StockBalance = read.stock_balance(company, item, wh).await.unwrap().expect("bin exists");
    assert_eq!(sb.valuation_rate, d("100.000000"));
    assert_eq!(sb.stock_value, d("1000.00"));
}

// CS-3: DeliveryRequested intake — turns a selling request into a DRAFT delivery note linked to the
// source sales order (the inventory half of the delivery seam).
#[tokio::test]
async fn delivery_requested_creates_draft_linked_to_order() {
    let pool = pool().await;
    let w = InventoryWriteService::new(pool.clone());
    let intake = DeliveryIntake::new(pool.clone());
    let (company, item, so) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let wh = w.create_warehouse(NewWarehouse { company_id: company, code: uq("WH"), name: "Main".into(), warehouse_type: None, parent_warehouse_id: None, is_group: false }).await.unwrap();

    let did = intake.on_delivery_requested(DeliveryRequested {
        delivery_number: uq("DN"), company_id: company, branch_id: None, customer_id: Uuid::new_v4(),
        source_so_id: Some(so), warehouse_id: wh, posting_date: day(),
        currency: "IDR".into(),
        cogs_account_id: Uuid::new_v4(), inventory_account_id: Uuid::new_v4(),
        lines: vec![DeliveryRequestLine { item_id: item, quantity: d("3") }],
    }).await.unwrap();

    let (status, linked): (String, Option<Uuid>) = sqlx::query_as(
        "SELECT status::text, source_so_id FROM inventory.delivery_notes WHERE id=$1")
        .bind(did).fetch_one(&pool).await.unwrap();
    assert_eq!(status, "draft", "intake creates a draft; submit is a separate step");
    assert_eq!(linked, Some(so), "delivery note carries the source sales order");
}
