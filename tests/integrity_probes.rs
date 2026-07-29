//! Route-level probes: the guarded surface validates creates and does NOT expose generic mutation
//! (create/update/delete/bulk) or direct SLE/Bin writes — and every validated write derives its
//! tenant from a signed token rather than the request body. Requires DATABASE_URL
//! (:5433/backbone_inventory).
//!
//! IIP-1..IIP-4  the CRUD-bypass and validated-write invariants.
//! IIT-1..IIT-4  the tenancy invariants (mirrors the IGT-* cases backbone-selling proved).

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use backbone_auth::company::CompanyVerifier;
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use serde::Serialize;
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

use backbone_inventory::presentation::http::create_guarded_inventory_routes;
use backbone_inventory::InventoryModule;

const SECRET: &[u8] = b"inventory-integrity-probe-secret";

#[derive(Serialize)]
struct TestClaims {
    sub: String,
    exp: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    company_id: Option<Uuid>,
}

/// Mint an HS256 access token. `company_id = None` models a token that authenticates a user but
/// carries no tenant — it must not be allowed to write.
fn token(company_id: Option<Uuid>) -> String {
    let claims = TestClaims { sub: "probe-user".into(), exp: 9_999_999_999, company_id };
    encode(&Header::new(Algorithm::HS256), &claims, &EncodingKey::from_secret(SECRET)).unwrap()
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
    create_guarded_inventory_routes(m, pool.clone(), CompanyVerifier::hs256(SECRET))
}

/// Send a request with an optional bearer token.
async fn req_with(
    app: axum::Router,
    method: &str,
    uri: &str,
    body: Option<String>,
    bearer: Option<String>,
) -> (StatusCode, String) {
    let b = body.map(Body::from).unwrap_or(Body::empty());
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json");
    if let Some(t) = bearer {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {t}"));
    }
    let resp = app.oneshot(builder.body(b).unwrap()).await.unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024).await.unwrap();
    (status, String::from_utf8_lossy(&bytes).to_string())
}

/// Unauthenticated request.
async fn req(app: axum::Router, method: &str, uri: &str, body: Option<String>) -> (StatusCode, String) {
    req_with(app, method, uri, body, None).await
}

/// Request authenticated as a principal of `company`.
async fn req_as(
    app: axum::Router,
    company: Uuid,
    method: &str,
    uri: &str,
    body: Option<String>,
) -> (StatusCode, String) {
    req_with(app, method, uri, body, Some(token(Some(company)))).await
}

fn uq(p: &str) -> String { format!("{p}-{}", &uuid::Uuid::new_v4().simple().to_string()[..8]) }

// IIP-1: the guarded surface exposes the SLE and Bin as READ-ONLY — no write path exists, even for
// an authenticated principal. The invariant is "no write SUCCEEDS" (no 2xx): an authenticated POST
// to the read-only collection paths must not create anything. (Asserting the authed status matters:
// an unauthenticated POST hits the `company_auth` route-layer first and returns 401 regardless of
// whether a write route exists, so the unauth status says nothing about exposure.)
#[tokio::test]
async fn guarded_surface_has_no_direct_sle_or_bin_writes() {
    let pool = pool().await;
    let m = module(&pool).await;
    let company = uuid::Uuid::new_v4();
    let (s, _) = req_as(app(&pool, &m), company, "POST", "/stock-ledger-entries", Some("{}".into())).await;
    assert!(!s.is_success(), "no direct SLE write; got {s}");
    let (s2, _) = req_as(app(&pool, &m), company, "POST", "/bins/bulk", Some("[]".into())).await;
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

// IIP-3: validated warehouse create works (201). No `companyId` in the body — the tenant rides on
// the token.
#[tokio::test]
async fn guarded_create_warehouse_ok() {
    let pool = pool().await;
    let m = module(&pool).await;
    let body = format!(r#"{{"code":"{}","name":"Main"}}"#, uq("WH"));
    let (s, _) = req_as(app(&pool, &m), uuid::Uuid::new_v4(), "POST", "/warehouses", Some(body)).await;
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
    let (s, b) = req_as(app(&pool, &m), uuid::Uuid::new_v4(), "POST", "/purchase-receipts", Some(body)).await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(b.contains("empty_document"), "got: {b}");
}

// IIT-1: an unauthenticated write is rejected. Before the tenant guard this create succeeded and
// stamped whatever `companyId` the caller put in the body.
#[tokio::test]
async fn guarded_write_rejects_unauthenticated() {
    let pool = pool().await;
    let m = module(&pool).await;
    let body = format!(r#"{{"code":"{}","name":"Main"}}"#, uq("WH"));
    let (s, _) = req(app(&pool, &m), "POST", "/warehouses", Some(body)).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED, "an unauthenticated write must not reach the service");
}

// IIT-2: a token that authenticates a user but carries no `company_id` claim is rejected — a writer
// that cannot name its tenant must never run.
#[tokio::test]
async fn guarded_write_rejects_token_without_company_id() {
    let pool = pool().await;
    let m = module(&pool).await;
    let body = format!(r#"{{"code":"{}","name":"Main"}}"#, uq("WH"));
    let (s, _) = req_with(app(&pool, &m), "POST", "/warehouses", Some(body), Some(token(None))).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED, "a token with no tenant must not write");
}

// IIT-3: the DeliveryRequested intake is a document-persisting write, so it is guarded too — an
// unauthenticated caller cannot drive the selling↔inventory seam into an arbitrary tenant.
#[tokio::test]
async fn guarded_intake_rejects_unauthenticated() {
    let pool = pool().await;
    let m = module(&pool).await;
    let body = format!(
        r#"{{"deliveryNumber":"{}","customerId":"{}","warehouseId":"{}","postingDate":"2026-07-04","cogsAccountId":"{}","inventoryAccountId":"{}","lines":[]}}"#,
        uq("DN"), uuid::Uuid::new_v4(), uuid::Uuid::new_v4(), uuid::Uuid::new_v4(), uuid::Uuid::new_v4());
    let (s, _) = req(app(&pool, &m), "POST", "/delivery-requests", Some(body)).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED, "an unauthenticated intake must not reach the service");
}

// IIT-4: a `companyId` smuggled in the body is ignored — the persisted tenant is the token's. This is
// the regression that motivated the change: the body must not be able to name the tenant.
#[tokio::test]
async fn body_company_id_cannot_override_the_token_tenant() {
    let pool = pool().await;
    let m = module(&pool).await;
    let token_company = uuid::Uuid::new_v4();
    let attacker_company = uuid::Uuid::new_v4();
    let code = uq("WH");
    let body = format!(
        r#"{{"companyId":"{}","code":"{}","name":"Main"}}"#, attacker_company, code);
    let (s, _) = req_as(app(&pool, &m), token_company, "POST", "/warehouses", Some(body)).await;
    assert_eq!(s, StatusCode::CREATED);

    let persisted: Uuid =
        sqlx::query_scalar("SELECT company_id FROM inventory.warehouses WHERE code = $1")
            .bind(&code)
            .fetch_one(&pool)
            .await
            .expect("warehouse row");
    assert_eq!(persisted, token_company, "tenant must come from the token, not the body");
    assert_ne!(persisted, attacker_company, "the body's companyId must be ignored");
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
    let (company, item) = (Uuid::new_v4(), Uuid::new_v4());
    let wh1 = w.create_warehouse(NewWarehouse {
        company_id: company, code: uq("WH"), name: "A".into(),
        warehouse_type: None, parent_warehouse_id: None, is_group: false,
    }).await.unwrap();
    let wh2 = w.create_warehouse(NewWarehouse {
        company_id: company, code: uq("WH"), name: "B".into(),
        warehouse_type: None, parent_warehouse_id: None, is_group: false,
    }).await.unwrap();

    // A workload spanning every movement kind + a cancellation.
    let r1 = w.create_purchase_receipt(NewReceipt {
        receipt_number: uq("PR"), company_id: company, branch_id: None, supplier_id: Uuid::new_v4(),
        source_po_id: None, warehouse_id: wh1, posting_date: day(), currency: "IDR".into(),
        inventory_account_id: Uuid::new_v4(), grir_account_id: Uuid::new_v4(),
        lines: vec![ReceiptLine { item_id: item, quantity: d("10"), rate: d("100") }],
    }).await.unwrap();
    w.submit_purchase_receipt(r1, &StubGl).await.unwrap();
    let did = w.create_delivery_note(NewDelivery {
        delivery_number: uq("DN"), company_id: company, branch_id: None, customer_id: Uuid::new_v4(),
        source_so_id: None, warehouse_id: wh1, posting_date: day(), currency: "IDR".into(),
        cogs_account_id: Uuid::new_v4(), inventory_account_id: Uuid::new_v4(),
        lines: vec![DeliveryLine { item_id: item, quantity: d("3") }],
    }).await.unwrap();
    w.submit_delivery_note(did, &StubGl).await.unwrap();
    w.submit_transfer(NewTransfer {
        entry_number: uq("SE"), company_id: company, from_warehouse_id: wh1, to_warehouse_id: wh2,
        posting_date: day(), lines: vec![DeliveryLine { item_id: item, quantity: d("2") }],
    }).await.unwrap();
    w.submit_reconciliation(NewReconciliation {
        recon_number: uq("SR"), company_id: company, warehouse_id: wh2, posting_date: day(),
        currency: "IDR".into(), inventory_account_id: Uuid::new_v4(), adjustment_account_id: Uuid::new_v4(),
        lines: vec![ReconLine { item_id: item, counted_qty: d("2"), counted_rate: Decimal::ZERO }],
    }, &StubGl).await.unwrap();
    w.cancel_delivery_note(did, &StubGl).await.unwrap();

    // Drift check across every (item, warehouse) bin this company owns: Bin == Σ SLE.
    let drift: Vec<(Uuid, Uuid, Decimal, Decimal)> = sqlx::query_as(
        r#"WITH sle AS (
              SELECT item_id, warehouse_id,
                     SUM(actual_qty) AS sum_qty, SUM(stock_value_difference) AS sum_val
              FROM inventory.stock_ledger_entries
              WHERE company_id=$1 AND (metadata->>'deleted_at') IS NULL
              GROUP BY item_id, warehouse_id)
           SELECT b.item_id, b.warehouse_id,
                  b.actual_qty - COALESCE(sle.sum_qty, 0),
                  b.stock_value - COALESCE(sle.sum_val, 0)
           FROM inventory.bins b
           LEFT JOIN sle USING (item_id, warehouse_id)
           WHERE b.company_id=$1 AND (b.metadata->>'deleted_at') IS NULL"#,
    )
    .bind(company)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert!(!drift.is_empty(), "the workload should have produced at least one bin");
    for (item_id, wh_id, qty_drift, val_drift) in &drift {
        assert_eq!(*qty_drift, Decimal::ZERO, "qty drift on item {item_id} wh {wh_id}");
        assert_eq!(*val_drift, Decimal::ZERO, "value drift on item {item_id} wh {wh_id}");
    }
}
