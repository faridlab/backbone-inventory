//! Route-level probes: the guarded surface validates creates and does NOT expose generic mutation
//! (create/update/delete/bulk) or direct SLE/Bin writes. Requires DATABASE_URL (:5433/backbone_inventory).

use axum::body::Body;
use axum::http::{Request, StatusCode};
use sqlx::PgPool;
use tower::ServiceExt;

use backbone_inventory::presentation::http::create_guarded_inventory_routes;
use backbone_inventory::InventoryModule;

async fn pool() -> PgPool {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:postgres@localhost:5433/backbone_inventory".to_string());
    PgPool::connect(&url).await.unwrap()
}
async fn module(pool: &PgPool) -> InventoryModule {
    InventoryModule::builder().with_database(pool.clone()).build().unwrap()
}
fn app(pool: &PgPool, m: &InventoryModule) -> axum::Router { create_guarded_inventory_routes(m, pool.clone()) }
async fn req(app: axum::Router, method: &str, uri: &str, body: Option<String>) -> (StatusCode, String) {
    let b = body.map(Body::from).unwrap_or(Body::empty());
    let resp = app.oneshot(Request::builder().method(method).uri(uri).header("content-type", "application/json").body(b).unwrap()).await.unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024).await.unwrap();
    (status, String::from_utf8_lossy(&bytes).to_string())
}
fn uq(p: &str) -> String { format!("{p}-{}", &uuid::Uuid::new_v4().simple().to_string()[..8]) }

// IIP-1: generic bulk create on the SLE (the append-ledger) is NOT exposed — nothing can write an
// SLE directly bypassing the valuation engine.
#[tokio::test]
async fn guarded_locks_direct_sle_write() {
    let pool = pool().await;
    let m = module(&pool).await;
    let (s, _) = req(app(&pool, &m), "POST", "/stock-ledger-entries", Some("{}".into())).await;
    assert!(s == StatusCode::METHOD_NOT_ALLOWED || s == StatusCode::NOT_FOUND, "no direct SLE write; got {s}");
    let (s2, _) = req(app(&pool, &m), "POST", "/bins/bulk", Some("[]".into())).await;
    assert!(s2 == StatusCode::METHOD_NOT_ALLOWED || s2 == StatusCode::NOT_FOUND, "no direct Bin write; got {s2}");
}

// IIP-2: generic delete on a receipt is NOT exposed.
#[tokio::test]
async fn guarded_locks_generic_receipt_delete() {
    let pool = pool().await;
    let m = module(&pool).await;
    let id = uuid::Uuid::new_v4();
    let (s, _) = req(app(&pool, &m), "DELETE", &format!("/purchase-receipts/{id}"), None).await;
    assert!(s == StatusCode::METHOD_NOT_ALLOWED || s == StatusCode::NOT_FOUND, "no generic delete; got {s}");
}

// IIP-3: validated warehouse create works (201).
#[tokio::test]
async fn guarded_create_warehouse_ok() {
    let pool = pool().await;
    let m = module(&pool).await;
    let body = format!(r#"{{"companyId":"{}","code":"{}","name":"Main"}}"#, uuid::Uuid::new_v4(), uq("WH"));
    let (s, _) = req(app(&pool, &m), "POST", "/warehouses", Some(body)).await;
    assert_eq!(s, StatusCode::CREATED);
}

// IIP-4: validated receipt create rejects an empty document (422 empty_document).
#[tokio::test]
async fn guarded_create_receipt_rejects_empty() {
    let pool = pool().await;
    let m = module(&pool).await;
    let body = format!(
        r#"{{"receiptNumber":"{}","companyId":"{}","supplierId":"{}","warehouseId":"{}","postingDate":"2026-07-04","inventoryAccountId":"{}","grirAccountId":"{}","lines":[]}}"#,
        uq("PR"), uuid::Uuid::new_v4(), uuid::Uuid::new_v4(), uuid::Uuid::new_v4(), uuid::Uuid::new_v4(), uuid::Uuid::new_v4());
    let (s, b) = req(app(&pool, &m), "POST", "/purchase-receipts", Some(body)).await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(b.contains("empty_document"), "got: {b}");
}
