//! Guarded route composition — the RECOMMENDED way to mount the inventory module.
//!
//! Hand-authored (user-owned). Read all stock documents + **validated create** (warehouse,
//! stock-item, purchase-receipt draft, delivery-note draft); generic create/update/delete CRUD is
//! NOT mounted, so no caller can write an SLE/Bin directly or persist an inconsistent document.
//! Every validated write derives its tenant from a signed Bearer token (`CompanyContext`) rather than
//! from the request body — a client cannot name the company it writes into.
//!
//! Submitting a movement (which writes the SLE, updates the Bin, and emits the GL post) needs a
//! `GlPostSink` from the composing service, so it is service/job-driven — proven by the seam test,
//! not exposed as a bare HTTP route. `InventoryWriteService` is built from the pool (regen-safe).

use std::sync::Arc;

use axum::{
    extract::State, http::StatusCode, middleware::from_fn_with_state, response::IntoResponse,
    routing::post, Json, Router,
};
use backbone_auth::company::{company_auth, CompanyContext, CompanyVerifier};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::application::service::inventory_write_service::{
    InventoryError, InventoryWriteService, NewDelivery, NewReceipt, NewWarehouse, ReceiptLine,
    DeliveryLine,
};
use crate::application::service::inventory_read::InventoryReadService;
use crate::application::service::inventory_intake::{DeliveryIntake, DeliveryRequestLine, DeliveryRequested};
use crate::InventoryModule;

use axum::extract::Query;

use super::{
    create_bin_read_routes, create_delivery_note_read_routes, create_purchase_receipt_read_routes,
    create_stock_ledger_entry_read_routes, create_warehouse_read_routes,
};

#[derive(Debug, Serialize)]
struct ErrorBody { error: String, message: String }
#[derive(Debug, Serialize)]
struct IdResponse { id: Uuid }
/// Serde default for the optional `currency` field on write bodies. Omitted → "IDR" (the module's
/// historical single-currency behavior); a composing service sets it for a non-IDR ledger.
fn default_currency() -> String { "IDR".into() }
fn err(e: InventoryError) -> axum::response::Response {
    let s = StatusCode::from_u16(e.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    (s, Json(ErrorBody { error: e.code(), message: e.to_string() })).into_response()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateWarehouseBody {
    // No `company_id`: the tenant is derived from the signed token via `CompanyContext`, never from
    // the request body — a client must not be able to name the tenant it writes into.
    code: String,
    name: String,
    #[serde(default)] warehouse_type: Option<String>,
    #[serde(default)] parent_warehouse_id: Option<Uuid>,
    #[serde(default)] is_group: bool,
}
async fn create_warehouse(State(svc): State<Arc<InventoryWriteService>>, tenant: CompanyContext, Json(b): Json<CreateWarehouseBody>) -> axum::response::Response {
    match svc.create_warehouse(NewWarehouse {
        company_id: tenant.company_id, code: b.code, name: b.name, warehouse_type: b.warehouse_type,
        parent_warehouse_id: b.parent_warehouse_id, is_group: b.is_group,
    }).await {
        Ok(id) => (StatusCode::CREATED, Json(IdResponse { id })).into_response(),
        Err(e) => err(e),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReceiptLineBody { item_id: Uuid, quantity: Decimal, rate: Decimal }
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateReceiptBody {
    receipt_number: String,
    // Tenant (company/branch) comes from the signed token (`CompanyContext`), not the body.
    supplier_id: Uuid,
    #[serde(default)] source_po_id: Option<Uuid>,
    warehouse_id: Uuid,
    posting_date: chrono::NaiveDate,
    #[serde(default = "default_currency")] currency: String,
    inventory_account_id: Uuid,
    grir_account_id: Uuid,
    lines: Vec<ReceiptLineBody>,
}
async fn create_receipt(State(svc): State<Arc<InventoryWriteService>>, tenant: CompanyContext, Json(b): Json<CreateReceiptBody>) -> axum::response::Response {
    let r = NewReceipt {
        receipt_number: b.receipt_number, company_id: tenant.company_id, branch_id: tenant.branch_id,
        supplier_id: b.supplier_id, source_po_id: b.source_po_id, warehouse_id: b.warehouse_id,
        posting_date: b.posting_date, currency: b.currency, inventory_account_id: b.inventory_account_id, grir_account_id: b.grir_account_id,
        lines: b.lines.into_iter().map(|l| ReceiptLine { item_id: l.item_id, quantity: l.quantity, rate: l.rate }).collect(),
    };
    match svc.create_purchase_receipt(r).await {
        Ok(id) => (StatusCode::CREATED, Json(IdResponse { id })).into_response(),
        Err(e) => err(e),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeliveryLineBody { item_id: Uuid, quantity: Decimal }
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateDeliveryBody {
    delivery_number: String,
    // Tenant (company/branch) comes from the signed token (`CompanyContext`), not the body.
    customer_id: Uuid,
    #[serde(default)] source_so_id: Option<Uuid>,
    warehouse_id: Uuid,
    posting_date: chrono::NaiveDate,
    #[serde(default = "default_currency")] currency: String,
    cogs_account_id: Uuid,
    inventory_account_id: Uuid,
    lines: Vec<DeliveryLineBody>,
}
async fn create_delivery(State(svc): State<Arc<InventoryWriteService>>, tenant: CompanyContext, Json(b): Json<CreateDeliveryBody>) -> axum::response::Response {
    let dn = NewDelivery {
        delivery_number: b.delivery_number, company_id: tenant.company_id, branch_id: tenant.branch_id,
        customer_id: b.customer_id, source_so_id: b.source_so_id, warehouse_id: b.warehouse_id,
        posting_date: b.posting_date, currency: b.currency, cogs_account_id: b.cogs_account_id, inventory_account_id: b.inventory_account_id,
        lines: b.lines.into_iter().map(|l| DeliveryLine { item_id: l.item_id, quantity: l.quantity }).collect(),
    };
    match svc.create_delivery_note(dn).await {
        Ok(id) => (StatusCode::CREATED, Json(IdResponse { id })).into_response(),
        Err(e) => err(e),
    }
}

fn write_routes(svc: Arc<InventoryWriteService>, verifier: CompanyVerifier) -> Router {
    Router::new()
        .route("/warehouses", post(create_warehouse))
        .route("/purchase-receipts", post(create_receipt))
        .route("/delivery-notes", post(create_delivery))
        // Every write above is tenant-scoped: `company_auth` rejects a request whose token is absent,
        // invalid, or carries no `company_id`, so a handler only ever runs with a proven tenant.
        //
        // `route_layer`, not `layer`: `layer` would also wrap this router's fallback, so once merged
        // every *unmatched* path (e.g. the generic CRUD paths this surface deliberately does not
        // mount) would answer 401 instead of 404 — leaking "auth required" for routes that do not
        // exist, and masking the CRUD-bypass probes.
        .route_layer(from_fn_with_state(verifier, company_auth))
        .with_state(svc)
}

// ── availability read-model (the surface selling consumes to check stock) ────
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AvailabilityQuery { company_id: Uuid, item_id: Uuid, warehouse_id: Uuid }
async fn get_availability(State(svc): State<Arc<InventoryReadService>>, Query(q): Query<AvailabilityQuery>) -> axum::response::Response {
    match svc.availability(q.company_id, q.item_id, q.warehouse_id).await {
        Ok(view) => (StatusCode::OK, Json(view)).into_response(),
        Err(e) => {
            // The read model returns a raw `sqlx::Error` (no domain taxonomy); log the typed error
            // so a 500 here is diagnosable instead of a bare "availability query failed".
            tracing::error!(target: "inventory.read", error = ?e, company_id = %q.company_id, item_id = %q.item_id, warehouse_id = %q.warehouse_id, "availability query failed");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorBody { error: "internal_error".into(), message: "availability query failed".into() })).into_response()
        }
    }
}
fn read_routes(svc: Arc<InventoryReadService>) -> Router {
    Router::new().route("/availability", axum::routing::get(get_availability)).with_state(svc)
}

// ── DeliveryRequested intake (the trigger of the selling↔inventory delivery seam) ──
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeliveryRequestLineBody { item_id: Uuid, quantity: Decimal }
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeliveryRequestedBody {
    delivery_number: String,
    // Tenant (company/branch) comes from the signed token (`CompanyContext`), not the body: this
    // intake persists a Delivery Note, so a body-supplied tenant would be a cross-tenant write.
    customer_id: Uuid,
    #[serde(default)] source_so_id: Option<Uuid>,
    warehouse_id: Uuid,
    posting_date: chrono::NaiveDate,
    #[serde(default = "default_currency")] currency: String,
    cogs_account_id: Uuid,
    inventory_account_id: Uuid,
    lines: Vec<DeliveryRequestLineBody>,
}
async fn post_delivery_requested(State(intake): State<Arc<DeliveryIntake>>, tenant: CompanyContext, Json(b): Json<DeliveryRequestedBody>) -> axum::response::Response {
    let req = DeliveryRequested {
        delivery_number: b.delivery_number, company_id: tenant.company_id, branch_id: tenant.branch_id,
        customer_id: b.customer_id, source_so_id: b.source_so_id, warehouse_id: b.warehouse_id,
        posting_date: b.posting_date, currency: b.currency, cogs_account_id: b.cogs_account_id, inventory_account_id: b.inventory_account_id,
        lines: b.lines.into_iter().map(|l| DeliveryRequestLine { item_id: l.item_id, quantity: l.quantity }).collect(),
    };
    match intake.on_delivery_requested(req).await {
        Ok(id) => (StatusCode::CREATED, Json(IdResponse { id })).into_response(),
        Err(e) => err(e),
    }
}
fn intake_routes(intake: Arc<DeliveryIntake>, verifier: CompanyVerifier) -> Router {
    Router::new()
        .route("/delivery-requests", post(post_delivery_requested))
        // Same guard as the write surface (and `route_layer` for the same fallback reason): the HTTP
        // face of the seam persists a document, so it must prove its tenant like any other write.
        .route_layer(from_fn_with_state(verifier, company_auth))
        .with_state(intake)
}

/// Mount the inventory module: read stock documents + validated, tenant-scoped creates. Generic
/// mutation and direct SLE/Bin writes are not exposed. **Prefer this over
/// `InventoryModule::all_crud_routes()`.**
///
/// The composing service builds one [`CompanyVerifier`] from its JWT secret and passes it here; the
/// write surface derives `company_id` from the token, so no tenant crosses the wire in a body.
pub fn create_guarded_inventory_routes(
    m: &InventoryModule,
    pool: PgPool,
    verifier: CompanyVerifier,
) -> Router {
    let write = Arc::new(InventoryWriteService::new(pool.clone()));
    let read = Arc::new(InventoryReadService::new(pool.clone()));
    let intake = Arc::new(DeliveryIntake::new(pool));
    // The generic entity read routes are tenant-scoped by the same `company_auth` layer as the writes:
    // it establishes the request scope (app.company_id bound on a dedicated connection), and the generic
    // list/get path runs through `company_scope::fetch_*_scoped`, which rides that connection so RLS
    // returns only the caller's rows. All five entities are company-fenced. Unauthenticated reads → 401.
    let entity_reads = Router::new()
        .merge(create_warehouse_read_routes(m.warehouse_service.clone()))
        .merge(create_bin_read_routes(m.bin_service.clone()))
        .merge(create_stock_ledger_entry_read_routes(m.stock_ledger_entry_service.clone()))
        .merge(create_purchase_receipt_read_routes(m.purchase_receipt_service.clone()))
        .merge(create_delivery_note_read_routes(m.delivery_note_service.clone()))
        .route_layer(from_fn_with_state(verifier.clone(), company_auth));
    Router::new()
        .merge(entity_reads)
        .merge(write_routes(write, verifier.clone()))
        .merge(read_routes(read))
        .merge(intake_routes(intake, verifier))
}
