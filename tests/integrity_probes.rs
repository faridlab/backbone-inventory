//! Route-level probes: the guarded surface validates creates and does NOT expose generic mutation
//! (create/update/delete/bulk) or direct SLE/Bin writes. Requires DATABASE_URL
//! (:5433/backbone_inventory).
//!
//! IIP-1..IIP-5  the CRUD-bypass and validated-write invariants.
//! IIT-1..IIT-4  the identity invariants that remain provable at module level. The
//!               cross-tenant data-isolation legs this suite once carried (the token-claim
//!               fence, the persisted-tenant proof) retired with the company strip (ADR-0029):
//!               the module is tenant-agnostic, ships BARE of authentication, and an undecorated
//!               deployment is unfenced by design — row isolation is proven by the composing
//!               service's decorator probes plus the undecorated half-fence pin in
//!               tests/engine_fence_probes.rs.
//!
//! The module mounts no auth middleware of its own: each request runs in-process via
//! `tower::ServiceExt::oneshot` with the caller identity inserted as a request extension the
//! way the composing service's org-auth stack does in production. Handlers that open a
//! document extract `OrgContext` — the branch a document opens at is the signed acting node,
//! never a body field — and the extractor rejects a request that arrives without one 401.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::middleware::{self, Next};
use axum::Router;
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

use backbone_auth::org::OrgContext;
use backbone_inventory::presentation::http::create_guarded_inventory_routes;
use backbone_inventory::InventoryModule;

/// The caller identity a request carries in production (inserted by the composing service's
/// org-auth layer). The module's handlers require its PRESENCE and read the acting node off
/// it; the database scope itself is the ambient request scope the host bound.
fn caller() -> OrgContext {
    OrgContext {
        acting_unit_id: Uuid::new_v4(),
        entitled_units: vec![],
        legacy_company_id: None,
        user_id: "integrity-probe".to_string(),
    }
}

/// Wrap the router with the extension the host auth stack provides in production.
fn with_caller(router: Router, org: OrgContext) -> Router {
    router.layer(middleware::from_fn(
        move |mut req: axum::extract::Request, next: Next| {
            let org = org.clone();
            async move {
                req.extensions_mut().insert(org);
                next.run(req).await
            }
        },
    ))
}

async fn pool() -> PgPool {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:postgres@localhost:5433/backbone_inventory".to_string());
    PgPool::connect(&url).await.unwrap()
}
async fn module(pool: &PgPool) -> InventoryModule {
    InventoryModule::builder().with_database(pool.clone()).build().unwrap()
}
fn app(pool: &PgPool, m: &InventoryModule) -> axum::Router {
    create_guarded_inventory_routes(m, pool.clone())
}

/// Send a request, optionally carrying a caller identity (None models a request the host's
/// org-auth stack never let in).
async fn req_with(
    app: axum::Router,
    caller: Option<OrgContext>,
    method: &str,
    uri: &str,
    body: Option<String>,
) -> (StatusCode, String) {
    let app = match caller {
        Some(c) => with_caller(app, c),
        None => app,
    };
    let b = body.map(Body::from).unwrap_or(Body::empty());
    let req = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(b)
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024).await.unwrap();
    (status, String::from_utf8_lossy(&bytes).to_string())
}

/// Authenticated request: a caller identity rides the extension, as in production.
async fn req(app: axum::Router, method: &str, uri: &str, body: Option<String>) -> (StatusCode, String) {
    req_with(app, Some(caller()), method, uri, body).await
}

fn uq(p: &str) -> String { format!("{p}-{}", &uuid::Uuid::new_v4().simple().to_string()[..8]) }

// IIP-1: the guarded surface exposes the SLE and Bin as READ-ONLY — no write path exists. The
// invariant is "no write SUCCEEDS" (no 2xx): a POST to the read-only collection paths must not
// create anything. Routing is decided before any handler (and before identity matters), so the
// bare router's status here says exactly what is and isn't mounted.
#[tokio::test]
async fn guarded_surface_has_no_direct_sle_or_bin_writes() {
    let pool = pool().await;
    let m = module(&pool).await;
    let (s, _) = req(app(&pool, &m), "POST", "/stock-ledger-entries", Some("{}".into())).await;
    assert!(!s.is_success(), "no direct SLE write; got {s}");
    let (s2, _) = req(app(&pool, &m), "POST", "/bins/bulk", Some("[]".into())).await;
    assert!(!s2.is_success(), "no direct Bin write; got {s2}");
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
    let body = format!(r#"{{"code":"{}","name":"Main"}}"#, uq("WH"));
    let (s, _) = req(app(&pool, &m), "POST", "/warehouses", Some(body)).await;
    assert_eq!(s, StatusCode::CREATED);
}

// IIP-4: validated receipt create rejects an empty document (422 empty_document).
#[tokio::test]
async fn guarded_create_receipt_rejects_empty() {
    let pool = pool().await;
    let m = module(&pool).await;
    let body = format!(
        r#"{{"receiptNumber":"{}","supplierId":"{}","warehouseId":"{}","postingDate":"2026-07-04","inventoryAccountId":"{}","grirAccountId":"{}","lines":[]}}"#,
        uq("PR"), uuid::Uuid::new_v4(), uuid::Uuid::new_v4(), uuid::Uuid::new_v4(), uuid::Uuid::new_v4());
    let (s, b) = req(app(&pool, &m), "POST", "/purchase-receipts", Some(body)).await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(b.contains("empty_document"), "got: {b}");
}

// IIT-1: a write that opens a document is rejected without a caller identity. The module mounts
// no auth middleware; the `OrgContext` extractor refuses a request that arrives unauthenticated
// (401) — a door that needs an acting node can never be driven by an anonymous caller.
#[tokio::test]
async fn guarded_write_rejects_unauthenticated() {
    let pool = pool().await;
    let m = module(&pool).await;
    let body = format!(
        r#"{{"receiptNumber":"{}","supplierId":"{}","warehouseId":"{}","postingDate":"2026-07-04","inventoryAccountId":"{}","grirAccountId":"{}","lines":[]}}"#,
        uq("PR"), uuid::Uuid::new_v4(), uuid::Uuid::new_v4(), uuid::Uuid::new_v4(), uuid::Uuid::new_v4());
    let (s, _) = req_with(app(&pool, &m), None, "POST", "/purchase-receipts", Some(body)).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED, "an unauthenticated write must not reach the service");
}

// (IIT-2 — "a token with no tenant claim is rejected" — retired with the company strip
// (ADR-0029): the module no longer validates tokens or inspects claims at all. Authentication
// is the composing service's org-auth stack; a caller identity reaches these handlers only as
// the extension `req_with` inserts, and nothing module-side keys on a token's tenant.)

// IIT-3: the DeliveryRequested intake is a document-persisting write, so it demands a caller
// identity too — an unauthenticated caller cannot drive the selling↔inventory seam.
#[tokio::test]
async fn guarded_intake_rejects_unauthenticated() {
    let pool = pool().await;
    let m = module(&pool).await;
    let body = format!(
        r#"{{"deliveryNumber":"{}","customerId":"{}","warehouseId":"{}","postingDate":"2026-07-04","cogsAccountId":"{}","inventoryAccountId":"{}","lines":[]}}"#,
        uq("DN"), uuid::Uuid::new_v4(), uuid::Uuid::new_v4(), uuid::Uuid::new_v4(), uuid::Uuid::new_v4());
    let (s, _) = req_with(app(&pool, &m), None, "POST", "/delivery-requests", Some(body)).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED, "an unauthenticated intake must not reach the service");
}

// IIT-4: tenant/branch keys smuggled in the body are ignored — the branch a document opens at
// is the SIGNED acting node, never a body field. Post-strip (ADR-0029) the tables carry no
// tenant column, so the old persisted-tenant-is-the-token's proof lives in the composing
// service's decorator probes; what this leg still proves at module level: the smuggled fields
// are TOLERATED (the create succeeds) and the persisted branch is the caller's acting node.
#[tokio::test]
async fn body_company_id_cannot_override_the_token_tenant() {
    let pool = pool().await;
    let m = module(&pool).await;
    let acting_unit = Uuid::new_v4();
    let smuggled = Uuid::new_v4();
    let body = format!(
        r#"{{"companyId":"{}","branchId":"{}","receiptNumber":"{}","supplierId":"{}","warehouseId":"{}","postingDate":"2026-07-04","inventoryAccountId":"{}","grirAccountId":"{}","lines":[{{"itemId":"{}","quantity":"1","rate":"100"}}]}}"#,
        smuggled, smuggled, uq("PR"), uuid::Uuid::new_v4(), uuid::Uuid::new_v4(),
        uuid::Uuid::new_v4(), uuid::Uuid::new_v4(), uuid::Uuid::new_v4());
    let (s, created) = req_with(
        app(&pool, &m),
        Some(OrgContext {
            acting_unit_id: acting_unit,
            entitled_units: vec![],
            legacy_company_id: None,
            user_id: "integrity-probe".to_string(),
        }),
        "POST", "/purchase-receipts", Some(body),
    ).await;
    assert_eq!(s, StatusCode::CREATED, "{created}");
    let id: Uuid = serde_json::from_str::<serde_json::Value>(&created).unwrap()["id"].as_str().unwrap().parse().unwrap();

    let branch: Option<Uuid> =
        sqlx::query_scalar("SELECT branch_id FROM inventory.purchase_receipts WHERE id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .expect("receipt row");
    assert_eq!(branch, Some(acting_unit),
        "the branch a document opens at must be the signed acting node, not a body field");
}

// IIP-5 (council 2026-07-29): the Bin running balance must tie to the append-only SLE for EVERY
// (item, warehouse) after a mixed workload (receipt + delivery + transfer + reconciliation + cancel).
// bin.stock_value == Σ sle.stock_value_difference and bin.actual_qty == Σ sle.actual_qty. A non-zero
// drift would mean a Bin was touched outside the engine — the leak Phase-1 closed. Defense-in-depth.
#[tokio::test]
async fn bin_ties_to_sle_after_mixed_workload() {
    use backbone_inventory::application::service::inventory_gl::{
        AccountingPostEnvelope, GlPostAck, GlPostRejected, GlPostSink,
    };
    use backbone_inventory::application::service::inventory_write_service::{
        DeliveryLine, InventoryWriteService, NewDelivery, NewReceipt, NewReconciliation, NewTransfer,
        NewWarehouse, ReceiptLine, ReconLine,
    };
    use rust_decimal::Decimal;

    struct StubGl;
    #[async_trait::async_trait]
    impl GlPostSink for StubGl {
        async fn post(&self, _e: &AccountingPostEnvelope) -> Result<GlPostAck, GlPostRejected> {
            Ok(GlPostAck { post_id: Uuid::new_v4(), journal_id: Uuid::new_v4(), idempotent_reuse: false })
        }
    }
    fn d(s: &str) -> Decimal { Decimal::from_str_exact(s).unwrap() }
    fn day() -> chrono::NaiveDate { chrono::NaiveDate::from_ymd_opt(2026, 7, 29).unwrap() }

    let pool = pool().await;
    let w = InventoryWriteService::new(pool.clone());
    let item = Uuid::new_v4();
    let wh1 = w.create_warehouse(NewWarehouse {
        code: uq("WH"), name: "A".into(),
        warehouse_type: None, parent_warehouse_id: None, is_group: false,
    }).await.unwrap();
    let wh2 = w.create_warehouse(NewWarehouse {
        code: uq("WH"), name: "B".into(),
        warehouse_type: None, parent_warehouse_id: None, is_group: false,
    }).await.unwrap();

    // A workload spanning every movement kind + a cancellation.
    let r1 = w.create_purchase_receipt(NewReceipt {
        receipt_number: uq("PR"), branch_id: None, supplier_id: Uuid::new_v4(),
        source_po_id: None, warehouse_id: wh1, posting_date: day(), currency: "IDR".into(),
        inventory_account_id: Uuid::new_v4(), grir_account_id: Uuid::new_v4(),
        lines: vec![ReceiptLine { item_id: item, quantity: d("10"), rate: d("100") , is_landed_costs_line: false }],
    }).await.unwrap();
    w.submit_purchase_receipt(r1, &StubGl).await.unwrap();
    let did = w.create_delivery_note(NewDelivery {
        delivery_number: uq("DN"), branch_id: None, customer_id: Uuid::new_v4(),
        source_so_id: None, warehouse_id: wh1, posting_date: day(), currency: "IDR".into(),
        cogs_account_id: Uuid::new_v4(), inventory_account_id: Uuid::new_v4(),
        lines: vec![DeliveryLine { item_id: item, quantity: d("3") }],
    }).await.unwrap();
    w.submit_delivery_note(did, &StubGl).await.unwrap();
    w.submit_transfer(NewTransfer {
        entry_number: uq("SE"), from_warehouse_id: wh1, to_warehouse_id: wh2,
        posting_date: day(), lines: vec![DeliveryLine { item_id: item, quantity: d("2") }],
    }).await.unwrap();
    w.submit_reconciliation(NewReconciliation {
        recon_number: uq("SR"), warehouse_id: wh2, posting_date: day(),
        currency: "IDR".into(), inventory_account_id: Uuid::new_v4(), adjustment_account_id: Uuid::new_v4(),
        lines: vec![ReconLine { item_id: item, counted_qty: d("2"), counted_rate: Decimal::ZERO }],
    }, &StubGl).await.unwrap();
    w.cancel_delivery_note(did, &StubGl).await.unwrap();

    // Drift check across every (item, warehouse) bin this workload touched: Bin == Σ SLE.
    let drift: Vec<(Uuid, Uuid, Decimal, Decimal)> = sqlx::query_as(
        r#"WITH sle AS (
              SELECT item_id, warehouse_id,
                     SUM(actual_qty) AS sum_qty, SUM(stock_value_difference) AS sum_val
              FROM inventory.stock_ledger_entries
              WHERE item_id=$1 AND (metadata->>'deleted_at') IS NULL
              GROUP BY item_id, warehouse_id)
           SELECT b.item_id, b.warehouse_id,
                  b.actual_qty - COALESCE(sle.sum_qty, 0),
                  b.stock_value - COALESCE(sle.sum_val, 0)
           FROM inventory.bins b
           LEFT JOIN sle USING (item_id, warehouse_id)
           WHERE b.item_id=$1 AND (b.metadata->>'deleted_at') IS NULL"#,
    )
    .bind(item)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert!(!drift.is_empty(), "the workload should have produced at least one bin");
    for (item_id, wh_id, qty_drift, val_drift) in &drift {
        assert_eq!(*qty_drift, Decimal::ZERO, "qty drift on item {item_id} wh {wh_id}");
        assert_eq!(*val_drift, Decimal::ZERO, "value drift on item {item_id} wh {wh_id}");
    }
}
