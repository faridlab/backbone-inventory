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
