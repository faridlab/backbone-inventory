//! Router-level route-match probes for the guarded HTTP surface.
//!
//! The behavior suites drive the services directly, so they cannot see route registration:
//! a path parameter written in a dialect the pinned axum does not treat as a parameter
//! (this crate pins axum 0.7, whose router reads `:id` and treats an `{id}` segment as a
//! literal) registers a route that NEVER matches — an empty 404 in every environment,
//! including standalone. These probes build the REAL router (`create_guarded_inventory_routes`,
//! the composer a service mounts) and drive it in-process with `tower::ServiceExt::oneshot`,
//! asserting each parameterized route family matches a real-UUID path with its real status
//! on a real entity. If the crate ever moves to an axum whose parameter dialect differs,
//! these probes fail loudly and the route table must be rewritten with them.
//!
//! One probe per swept route family: the picking probe GET, the batch-member add/remove
//! and batch GET probe, the scrap GET (served by the generated scrap read surface — the
//! door's own GET was a never-matching duplicate of it) and process verb, and the
//! landed-cost validate + cancel verbs. Plus a source-level dialect guard so the class
//! cannot be reintroduced silently on a route the probes do not drive.
//!
//! Requires DATABASE_URL (default :5433/backbone_inventory), schema applied.

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use serde::Serialize;
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

use backbone_auth::company::CompanyVerifier;
use backbone_inventory::application::service::inventory_gl::{
    AccountingPostEnvelope, GlPostAck, GlPostRejected, GlPostSink,
};
use backbone_inventory::application::service::inventory_scrap::NewScrap;
use backbone_inventory::application::service::inventory_transfer::{NewPicking, PickingLine};
use backbone_inventory::application::service::inventory_write_service::{
    InventoryWriteService, NewReceipt, NewWarehouse, ReceiptLine,
};
use backbone_inventory::presentation::http::create_guarded_inventory_routes;
use backbone_inventory::InventoryModule;

mod common;

const SECRET: &[u8] = b"inventory-route-match-probes-secret";

/// The guarded surface the probes drive, and the source its dialect guard reads.
const GUARDED_ROUTES_RS: &str = include_str!("../src/presentation/http/guarded_routes.rs");

#[derive(Serialize)]
struct TestClaims {
    sub: String,
    exp: usize,
    company_id: Option<Uuid>,
}

/// Mint an HS256 token carrying the tenant the request acts for (the claim the
/// `company_auth` guard requires).
fn token(company_id: Uuid) -> String {
    let claims = TestClaims { sub: "probe-1".into(), exp: 9_999_999_999, company_id: Some(company_id) };
    encode(&Header::new(Algorithm::HS256), &claims, &EncodingKey::from_secret(SECRET)).unwrap()
}

async fn pool() -> PgPool {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:postgres@localhost:5433/backbone_inventory".to_string());
    PgPool::connect(&url).await.expect("connect DB")
}

/// The REAL router a composing service mounts — the same builder, verifier, and route
/// table production uses.
async fn app() -> axum::Router {
    let pool = pool().await;
    let module = InventoryModule::builder().with_database(pool.clone()).build().unwrap();
    create_guarded_inventory_routes(&module, pool, CompanyVerifier::hs256(SECRET))
}

fn req(method: &str, uri: &str, body: Option<Value>, bearer: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {bearer}"))
        .body(Body::from(body.map(|v| v.to_string()).unwrap_or_default()))
        .unwrap()
}

async fn send(router: axum::Router, r: Request<Body>) -> (StatusCode, Value) {
    let resp = router.oneshot(r).await.unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, body)
}

// ── seeding (the family's established shapes) ─────────────────────────────────

/// GL sink that acknowledges every post — the receipt submit only needs a live sink.
struct AckSink;
#[async_trait::async_trait]
impl GlPostSink for AckSink {
    async fn post(&self, _e: &AccountingPostEnvelope) -> Result<GlPostAck, GlPostRejected> {
        Ok(GlPostAck { post_id: Uuid::new_v4(), journal_id: Uuid::new_v4(), idempotent_reuse: false })
    }
}

fn uq(p: &str) -> String { format!("{p}-{}", &Uuid::new_v4().simple().to_string()[..8]) }

async fn warehouse(svc: &InventoryWriteService, company: Uuid) -> Uuid {
    svc.create_warehouse(NewWarehouse {
        org_unit_id: company, code: uq("WH"), name: uq("Main"),
        warehouse_type: None, parent_warehouse_id: None, is_group: false,
    }).await.unwrap()
}

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

async fn op_type(pool: &PgPool, company: Uuid, src: Uuid, dst: Uuid) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO inventory.operation_types
             (id, name, sequence_code, code, company_id,
              default_location_src_id, default_location_dest_id, reservation_method, create_backorder)
           VALUES ($1,$2,$3,$4::picking_code,$5,$6,$7,'manual'::reservation_method,'ask'::create_backorder)"#,
    )
    .bind(id).bind(uq("PT")).bind(uq("SEQ")).bind("incoming").bind(company)
    .bind(src).bind(dst)
    .execute(pool).await.unwrap();
    id
}

async fn seed_quant(pool: &PgPool, company: Uuid, item: Uuid, location: Uuid, qty: &str) {
    let q = rust_decimal::Decimal::from_str_exact(qty).unwrap();
    sqlx::query(
        r#"INSERT INTO inventory.stock_quants
             (id, item_id, location_id, quantity, reserved_quantity, available_quantity, company_id)
           VALUES ($1,$2,$3,$4,0,$4,$5)"#,
    )
    .bind(Uuid::new_v4()).bind(item).bind(location).bind(q).bind(company)
    .execute(pool).await.unwrap();
}

async fn seed_bin(pool: &PgPool, company: Uuid, item: Uuid, wh: Uuid, qty: &str, rate: &str) {
    let (q, r) = (rust_decimal::Decimal::from_str_exact(qty).unwrap(),
                  rust_decimal::Decimal::from_str_exact(rate).unwrap());
    sqlx::query(
        r#"INSERT INTO inventory.bins
             (id, company_id, item_id, warehouse_id, actual_qty, reserved_qty, valuation_rate, stock_value)
           VALUES ($1,$2,$3,$4,$5,0,$6,$7)"#,
    )
    .bind(Uuid::new_v4()).bind(company).bind(item).bind(wh).bind(q).bind(r).bind(q * r)
    .execute(pool).await.unwrap();
}

/// A member picking in `confirmed` (manual reservation): returns its transfer id.
async fn picking(svc: &InventoryWriteService, pool: &PgPool, company: Uuid, wh: Uuid) -> Uuid {
    let supplier = loc(pool, company, "supplier", None).await;
    let stock = loc(pool, company, "internal", Some(wh)).await;
    let op = op_type(pool, company, supplier, stock).await;
    let created = svc.create_picking(NewPicking {
        name: uq("PICK"), company_id: company, picking_type_id: op,
        location_id: supplier, location_dest_id: stock, partner_id: None,
        move_type: "direct".into(), origin: None,
        lines: vec![PickingLine { item_id: Uuid::new_v4(), demand_qty: "5".parse().unwrap(), price_unit: "1".parse().unwrap() }],
    }).await.unwrap();
    created.transfer_id
}

// ── /pickings/:id — the projection probe GET ──────────────────────────────────

/// The picking probe GET must MATCH a real transfer id and answer 200 with the
/// projection body — an unmatched (wrong-dialect) route answers an empty 404 instead.
#[tokio::test]
async fn picking_probe_get_matches() {
    let router = app().await;
    let pool = pool().await;
    let svc = InventoryWriteService::new(pool.clone());
    let company = common::fresh_company(&pool).await;
    let wh = warehouse(&svc, company).await;
    let transfer = picking(&svc, &pool, company, wh).await;
    let bearer = token(company);

    let (status, body) = send(router, req("GET", &format!("/pickings/{transfer}"), None, &bearer)).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["id"].as_str(), Some(transfer.to_string().as_str()), "body: {body}");
    assert!(body["moves"].as_array().map(|m| !m.is_empty()).unwrap_or(false), "body: {body}");
    assert!(body["state"].is_string(), "body: {body}");
}

// ── /picking-batches/:id/pickings (+ /:picking_id, + the GET probe) ───────────

/// The batch membership verbs must MATCH: POST adds a member (200, projection body with
/// `hadMembers`), DELETE removes one (200), and the GET probe reads the batch (200).
#[tokio::test]
async fn batch_membership_routes_match() {
    let router = app().await;
    let pool = pool().await;
    let svc = InventoryWriteService::new(pool.clone());
    let company = common::fresh_company(&pool).await;
    let wh = warehouse(&svc, company).await;
    let batch = svc.create_batch(company, uq("BATCH"), false, None).await.unwrap().id;
    let a = picking(&svc, &pool, company, wh).await;
    let b = picking(&svc, &pool, company, wh).await;
    let bearer = token(company);

    // Add: the route matches and the reprojected header comes back.
    let (status, body) = send(
        router.clone(),
        req("POST", &format!("/picking-batches/{batch}/pickings"), Some(json!({ "pickingId": a })), &bearer),
    ).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["id"].as_str(), Some(batch.to_string().as_str()), "body: {body}");
    assert_eq!(body["hadMembers"], Value::Bool(true), "body: {body}");

    // A second member joins (the projection stays over both).
    let (status, _) = send(
        router.clone(),
        req("POST", &format!("/picking-batches/{batch}/pickings"), Some(json!({ "pickingId": b })), &bearer),
    ).await;
    assert_eq!(status, StatusCode::OK);

    // The batch GET probe (the hyphenated path — this door's alone; the generated
    // picking-batch reads mount at the underscore form) matches and reports both members.
    let (status, body) = send(router.clone(), req("GET", &format!("/picking-batches/{batch}"), None, &bearer)).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["batch"]["id"].as_str(), Some(batch.to_string().as_str()), "body: {body}");
    assert_eq!(body["members"].as_array().map(Vec::len), Some(2), "body: {body}");

    // Remove (TWO parameters on one path): the route matches and the header comes back.
    let (status, body) = send(
        router.clone(),
        req("DELETE", &format!("/picking-batches/{batch}/pickings/{a}"), None, &bearer),
    ).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["id"].as_str(), Some(batch.to_string().as_str()), "body: {body}");
}

// ── /scraps/:id (+ /process) ──────────────────────────────────────────────────

/// The scrap verbs must MATCH: the GET probe reads the draft (200), the process verb
/// rides the move engine (200 with the scrap + move ids), and the GET probe reads the
/// closed header (200, state `done`).
#[tokio::test]
async fn scrap_process_route_matches() {
    let router = app().await;
    let pool = pool().await;
    let svc = InventoryWriteService::new(pool.clone());
    let company = common::fresh_company(&pool).await;
    let wh = warehouse(&svc, company).await;
    let stock = loc(&pool, company, "internal", Some(wh)).await;
    let item = Uuid::new_v4();
    seed_quant(&pool, company, item, stock, "7").await;
    seed_bin(&pool, company, item, wh, "7", "3").await;
    let scrap_row = svc.create_scrap(NewScrap {
        company_id: company, item_id: item, scrap_qty: "3".parse().unwrap(),
        location_id: stock, scrap_location_id: None,
        lot_id: None, package_id: None, owner_id: None, picking_id: None,
        origin: None, scrap_reason_tag_ids: vec![],
    }).await.unwrap();
    let scrap = scrap_row.id;
    let bearer = token(company);

    // The generated scrap read surface owns GET /scraps/:id (the generic envelope).
    let (status, body) = send(router.clone(), req("GET", &format!("/scraps/{scrap}"), None, &bearer)).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["success"], Value::Bool(true), "body: {body}");
    assert_eq!(body["data"]["id"].as_str(), Some(scrap.to_string().as_str()), "body: {body}");
    assert_eq!(body["data"]["state"], json!("draft"), "body: {body}");

    let (status, body) = send(router.clone(), req("POST", &format!("/scraps/{scrap}/process"), None, &bearer)).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["scrapId"].as_str(), Some(scrap.to_string().as_str()), "body: {body}");
    assert!(body["moveId"].is_string(), "body: {body}");

    let (status, body) = send(router, req("GET", &format!("/scraps/{scrap}"), None, &bearer)).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["data"]["state"], json!("done"), "body: {body}");
}

// ── /landed-costs/:id/validate + /cancel ──────────────────────────────────────

/// Open a DRAFT landed cost over `receipt` (one quantity-basis cost line). Returns its id.
async fn draft_lc(svc: &InventoryWriteService, company: Uuid, receipt: Uuid) -> Uuid {
    svc.create_landed_cost(backbone_inventory::application::service::inventory_write_service::NewLandedCost {
        lc_number: uq("LC"), company_id: company, branch_id: None,
        target_receipt_id: receipt,
        posting_date: chrono::NaiveDate::from_ymd_opt(2026, 8, 27).unwrap(),
        currency: "IDR".into(), notes: None,
        lines: vec![backbone_inventory::application::service::inventory_write_service::LcCostLine {
            name: uq("freight").into(), account_id: Uuid::new_v4(),
            split_method: "quantity".into(), amount: "100".parse().unwrap(),
        }],
    }).await.unwrap()
}

/// The landed-cost verbs must MATCH: validate answers 200 with the deferred outcome
/// (posted `false` — the GL leg stays armed for the service-driven repost), and cancel
/// answers 204 on a draft.
#[tokio::test]
async fn landed_cost_verb_routes_match() {
    let router = app().await;
    let pool = pool().await;
    let svc = InventoryWriteService::new(pool.clone());
    let company = common::fresh_company(&pool).await;
    let wh = warehouse(&svc, company).await;
    let item = Uuid::new_v4();
    let bearer = token(company);

    // A submitted receipt gives the allocation its DONE moves.
    let receipt = svc.create_purchase_receipt(NewReceipt {
        receipt_number: uq("PR"), company_id: company, branch_id: None, supplier_id: Uuid::new_v4(),
        source_po_id: None, warehouse_id: wh,
        posting_date: chrono::NaiveDate::from_ymd_opt(2026, 8, 27).unwrap(),
        currency: "IDR".into(),
        inventory_account_id: Uuid::new_v4(), grir_account_id: Uuid::new_v4(),
        lines: vec![ReceiptLine { item_id: item, quantity: "10".parse().unwrap(), rate: "100".parse().unwrap(), is_landed_costs_line: false }],
    }).await.unwrap();
    svc.submit_purchase_receipt(receipt, &AckSink).await.unwrap();

    let to_validate = draft_lc(&svc, company, receipt).await;
    let to_cancel = draft_lc(&svc, company, receipt).await;

    let (status, body) = send(router.clone(), req("POST", &format!("/landed-costs/{to_validate}/validate"), None, &bearer)).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["id"].as_str(), Some(to_validate.to_string().as_str()), "body: {body}");
    assert_eq!(body["posted"], Value::Bool(false), "the deferred HTTP shape arms the GL leg; it does not post: {body}");

    let (status, body) = send(router, req("POST", &format!("/landed-costs/{to_cancel}/cancel"), None, &bearer)).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "body: {body}");
}

// ── the source-level dialect guard ────────────────────────────────────────────

/// No route path in the guarded surface may carry a brace-style segment: axum 0.7 (the
/// pinned router) treats `{id}` as a literal, so such a route never matches — the empty
/// 404 the probes above exist to catch. This reads the source so a future hand-written
/// route fails here even before a probe drives it.
#[test]
fn guarded_routes_declare_no_brace_style_path_segments() {
    for line in GUARDED_ROUTES_RS.lines() {
        let trimmed = line.trim();
        let Some((_, rest)) = trimmed.split_once(".route(\"") else { continue };
        let Some(path) = rest.split('"').next() else { continue };
        assert!(
            !path.contains('{'),
            "a guarded route uses a brace-style segment axum 0.7 never matches: {path:?} — \
             path parameters must be written `:param` in this crate",
        );
    }
}
