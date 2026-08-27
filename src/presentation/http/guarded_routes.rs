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
    // Procurement configuration reads: routes, rules, and orderpoints. Orderpoints mount
    // READ-ONLY here by design — a `manual` trigger orderpoint surfaces in the replenishment
    // view for a human to order against; the computes recommend, writes go through the
    // service surface (later passes expose the ordering verbs).
    create_route_read_routes, create_route_rule_read_routes, create_reordering_rule_read_routes,
    // Satellite/master-data reads: the batch + scrap documents and the package/storage/
    // putaway vocabulary. Writes for the vocabulary stay on their validated surface; the
    // batch and scrap documents write only through the verbs above.
    create_picking_batch_read_routes, create_scrap_read_routes, create_scrap_reason_tag_read_routes,
    create_package_type_read_routes, create_storage_category_read_routes,
    create_storage_category_capacity_read_routes, create_putaway_rule_read_routes,
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
struct ReceiptLineBody {
    item_id: Uuid,
    quantity: Decimal,
    rate: Decimal,
    /// Landed-cost service line (the seam owned by inventory): a flagged line carries cost
    /// into a LandedCost document, NOT stock — the receipt door mints no move for it. The
    /// flag defaults to `false`, so an unannotated body behaves exactly as before.
    #[serde(default)]
    is_landed_costs_line: bool,
}
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
        lines: b.lines.into_iter().map(|l| ReceiptLine {
            item_id: l.item_id, quantity: l.quantity, rate: l.rate,
            is_landed_costs_line: l.is_landed_costs_line,
        }).collect(),
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

// ── landed-cost documents (draft → validate → done; cancel from draft only) ──
//
// The validated surface over the landed-cost family: a draft is opened with its cost lines,
// validation allocates each line over the target receipt's DONE moves and revalues the
// remaining stock through the move engine's adjustment verb, and a draft may cancel (a
// validated document corrects via a NEGATIVE landed cost — swapped legs — never a cancel).
//
// Validate here is the DEFERRED shape: the GL post needs the composing service's `GlPostSink`,
// so the HTTP verb performs the full physical revaluation and leaves the GL leg armed
// `pending` for a service/job-driven repost — the module's standing posture for GL-posting
// verbs (voucher submit works the same way).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LcLineBody {
    name: String,
    account_id: Uuid,
    // quantity | value | weight — an invalid value fails the DB's enum cast loudly.
    split_method: String,
    amount: Decimal,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateLandedCostBody {
    lc_number: String,
    // Tenant (company/branch) comes from the signed token (`CompanyContext`), not the body.
    target_receipt_id: Uuid,
    posting_date: chrono::NaiveDate,
    #[serde(default = "default_currency")] currency: String,
    #[serde(default)] notes: Option<String>,
    lines: Vec<LcLineBody>,
}
async fn create_landed_cost(
    State(svc): State<Arc<InventoryWriteService>>,
    tenant: CompanyContext,
    Json(b): Json<CreateLandedCostBody>,
) -> axum::response::Response {
    let lc = crate::application::service::inventory_write_service::NewLandedCost {
        lc_number: b.lc_number,
        company_id: tenant.company_id,
        branch_id: tenant.branch_id,
        target_receipt_id: b.target_receipt_id,
        posting_date: b.posting_date,
        currency: b.currency,
        notes: b.notes,
        lines: b.lines.into_iter().map(|l| crate::application::service::inventory_write_service::LcCostLine {
            name: l.name, account_id: l.account_id, split_method: l.split_method, amount: l.amount,
        }).collect(),
    };
    match svc.create_landed_cost(lc).await {
        Ok(id) => (StatusCode::CREATED, Json(IdResponse { id })).into_response(),
        Err(e) => err(e),
    }
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LcOutcomeBody { id: Uuid, posted: bool, gl_amount: Decimal }
async fn validate_landed_cost(
    State(svc): State<Arc<InventoryWriteService>>,
    _tenant: CompanyContext,
    axum::extract::Path(lc_id): axum::extract::Path<Uuid>,
) -> axum::response::Response {
    match svc.validate_landed_cost_deferred(lc_id).await {
        Ok(out) => (StatusCode::OK, Json(LcOutcomeBody { id: out.voucher_id, posted: out.posted, gl_amount: out.gl_amount })).into_response(),
        Err(e) => err(e),
    }
}
async fn cancel_landed_cost(
    State(svc): State<Arc<InventoryWriteService>>,
    _tenant: CompanyContext,
    axum::extract::Path(lc_id): axum::extract::Path<Uuid>,
) -> axum::response::Response {
    // `tenant` proves the caller's company; the service re-reads the document and binds ITS
    // company before any write (a document of another company is simply NotFound to the fence).
    match svc.cancel_landed_cost(lc_id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => err(e),
    }
}

// ── picking-batch surface (SB-1 — the batch state is a PROJECTION of its members) ──
//
// No verb here writes a batch state: `create` mints a draft header, the membership verbs
// move the `batch_id` pointer (the recompute that follows derives the state), and the probe
// READS it. Same posture as the picking surface above — the only batch-state writer in the
// module is the projection recompute.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BatchHeaderBody {
    id: Uuid,
    name: String,
    /// The PROJECTED state (a read — never an assertion).
    state: String,
    is_wave: bool,
    had_members: bool,
    scheduled_date: chrono::DateTime<chrono::Utc>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BatchMemberBody { id: Uuid, name: String, state: String }
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BatchProbeBody { batch: BatchHeaderBody, members: Vec<BatchMemberBody> }
fn batch_header_body(h: crate::infrastructure::persistence::BatchHeaderRow) -> BatchHeaderBody {
    BatchHeaderBody {
        id: h.id, name: h.name, state: h.state, is_wave: h.is_wave,
        had_members: h.had_members, scheduled_date: h.scheduled_date,
    }
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateBatchBody {
    name: String,
    #[serde(default)] is_wave: bool,
}
async fn create_batch(
    State(svc): State<Arc<InventoryWriteService>>,
    tenant: CompanyContext,
    Json(b): Json<CreateBatchBody>,
) -> axum::response::Response {
    match svc.create_batch(tenant.company_id, b.name, b.is_wave, None).await {
        Ok(h) => (StatusCode::CREATED, Json(batch_header_body(h))).into_response(),
        Err(e) => err(e),
    }
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BatchMemberBodyIn { picking_id: Uuid }
async fn add_batch_member(
    State(svc): State<Arc<InventoryWriteService>>,
    tenant: CompanyContext,
    axum::extract::Path(batch_id): axum::extract::Path<Uuid>,
    Json(b): Json<BatchMemberBodyIn>,
) -> axum::response::Response {
    match svc.add_picking_to_batch(tenant.company_id, batch_id, b.picking_id).await {
        Ok(h) => (StatusCode::OK, Json(batch_header_body(h))).into_response(),
        Err(e) => err(e),
    }
}
async fn remove_batch_member(
    State(svc): State<Arc<InventoryWriteService>>,
    tenant: CompanyContext,
    axum::extract::Path((batch_id, picking_id)): axum::extract::Path<(Uuid, Uuid)>,
) -> axum::response::Response {
    match svc.remove_picking_from_batch(tenant.company_id, batch_id, picking_id).await {
        Ok(h) => (StatusCode::OK, Json(batch_header_body(h))).into_response(),
        Err(e) => err(e),
    }
}
async fn get_batch(
    State(svc): State<Arc<InventoryWriteService>>,
    tenant: CompanyContext,
    axum::extract::Path(batch_id): axum::extract::Path<Uuid>,
) -> axum::response::Response {
    match svc.fetch_batch(tenant.company_id, batch_id).await {
        Ok((h, members)) => (StatusCode::OK, Json(BatchProbeBody {
            batch: batch_header_body(h),
            members: members.into_iter().map(|m| BatchMemberBody {
                id: m.id, name: m.name, state: m.state,
            }).collect(),
        })).into_response(),
        Err(e) => err(e),
    }
}

// ── scrap surface (the door rides the ONE move engine; deferred GL on HTTP) ──
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ScrapBody {
    id: Uuid,
    name: String,
    state: String,
    origin: Option<String>,
    item_id: Uuid,
    scrap_qty: Decimal,
    location_id: Uuid,
    scrap_location_id: Uuid,
    move_id: Option<Uuid>,
}
fn scrap_body(r: crate::infrastructure::persistence::ScrapRow) -> ScrapBody {
    ScrapBody {
        id: r.id, name: r.name, state: r.state, origin: r.origin, item_id: r.item_id,
        scrap_qty: r.scrap_qty, location_id: r.location_id,
        scrap_location_id: r.scrap_location_id, move_id: r.move_id,
    }
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateScrapBody {
    item_id: Uuid,
    scrap_qty: Decimal,
    location_id: Uuid,
    #[serde(default)] scrap_location_id: Option<Uuid>,
    #[serde(default)] lot_id: Option<Uuid>,
    #[serde(default)] package_id: Option<Uuid>,
    #[serde(default)] owner_id: Option<Uuid>,
    #[serde(default)] picking_id: Option<Uuid>,
    #[serde(default)] origin: Option<String>,
    #[serde(default)] scrap_reason_tag_ids: Vec<Uuid>,
}
async fn create_scrap(
    State(svc): State<Arc<InventoryWriteService>>,
    tenant: CompanyContext,
    Json(b): Json<CreateScrapBody>,
) -> axum::response::Response {
    let s = crate::application::service::inventory_scrap::NewScrap {
        company_id: tenant.company_id,
        item_id: b.item_id,
        scrap_qty: b.scrap_qty,
        location_id: b.location_id,
        scrap_location_id: b.scrap_location_id,
        lot_id: b.lot_id,
        package_id: b.package_id,
        owner_id: b.owner_id,
        picking_id: b.picking_id,
        origin: b.origin,
        scrap_reason_tag_ids: b.scrap_reason_tag_ids,
    };
    match svc.create_scrap(s).await {
        Ok(r) => (StatusCode::CREATED, Json(scrap_body(r))).into_response(),
        Err(e) => err(e),
    }
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ScrapProcessedBody { scrap_id: Uuid, move_id: Uuid }
async fn process_scrap(
    State(svc): State<Arc<InventoryWriteService>>,
    tenant: CompanyContext,
    axum::extract::Path(scrap_id): axum::extract::Path<Uuid>,
) -> axum::response::Response {
    // The deferred shape: the physical movement lands whole; the GL leg stays for a
    // service-driven repost (the composing service's sink is not available here).
    match svc.process_scrap_deferred(tenant.company_id, scrap_id).await {
        Ok(out) => (StatusCode::OK, Json(ScrapProcessedBody {
            scrap_id: out.scrap_id, move_id: out.move_id,
        })).into_response(),
        Err(e) => err(e),
    }
}
// NOTE: there is deliberately no hand-written `GET /scraps/:id` here. The generated scrap
// read surface (mounted above via `create_scrap_read_routes`) already owns that path, and
// axum refuses two handlers on one method+path. Its DTO carries everything the door view
// would (state, move_id, both locations), so a second route would only re-state it; the
// scrap door's own surface is the mint + process verbs below.
fn write_routes(svc: Arc<InventoryWriteService>, verifier: CompanyVerifier) -> Router {
    Router::new()
        .route("/warehouses", post(create_warehouse))
        .route("/purchase-receipts", post(create_receipt))
        .route("/delivery-notes", post(create_delivery))
        // Landed-cost lifecycle verbs. Validate is the deferred shape (the physical
        // revaluation + GL leg armed `pending`); the GL post itself needs the composing
        // service's `GlPostSink` and stays service/job-driven like voucher submit.
        .route("/landed-costs", post(create_landed_cost))
        .route("/landed-costs/:id/validate", post(validate_landed_cost))
        .route("/landed-costs/:id/cancel", post(cancel_landed_cost))
        // Picking mint + count staging (the projection/adjustment write surfaces that post no
        // GL). `validate_picking` / `apply_inventory` need the composing service's `GlPostSink`
        // (the GL-posting contract), so they stay service/job-driven like voucher submit —
        // proven by the seam tests, never exposed as bare HTTP.
        .route("/pickings", post(create_picking))
        .route("/counts", post(stage_count))
        // Picking-batch membership (SB-1): mints a draft grouping point and moves the
        // membership pointer — the batch state derives from the members, never written here.
        .route("/picking-batches", post(create_batch))
        .route("/picking-batches/:id/pickings", post(add_batch_member))
        .route("/picking-batches/:id/pickings/:picking_id", axum::routing::delete(remove_batch_member))
        // Scrap door: mint stays draft; process is the deferred shape (physical movement
        // lands whole, GL leg stays for the service-driven repost).
        .route("/scraps", post(create_scrap))
        .route("/scraps/:id/process", post(process_scrap))
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

// ── picking-as-projection surface (spec stock §2 T1 — the transfer is a PROJECTION) ──
//
// The picking document has NO hand-set state anywhere on this surface: `create` mints the
// header + member moves through the move engine (which reprojects the transfer on every
// move change), and the probe READS the projected state. Validation (`button_validate`) is
// service-driven (GL sink), so the only transfer-state writer in the module stays the
// engine's recompute.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PickingLineBody { item_id: Uuid, demand_qty: Decimal, #[serde(default)] price_unit: Decimal }
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreatePickingBody {
    name: String,
    // Tenant comes from the signed token (`CompanyContext`), not the body.
    picking_type_id: Uuid,
    location_id: Uuid,
    location_dest_id: Uuid,
    #[serde(default)] partner_id: Option<Uuid>,
    #[serde(default = "default_move_type")] move_type: String,
    #[serde(default)] origin: Option<String>,
    lines: Vec<PickingLineBody>,
}
/// Serde default for the optional `moveType` field: `direct` (ship as available) — the
/// generated operation-type default.
fn default_move_type() -> String { "direct".into() }
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PickingCreatedBody { transfer_id: Uuid, move_ids: Vec<Uuid>, projected_state: String }
async fn create_picking(
    State(svc): State<Arc<InventoryWriteService>>,
    tenant: CompanyContext,
    Json(b): Json<CreatePickingBody>,
) -> axum::response::Response {
    let p = crate::application::service::inventory_transfer::NewPicking {
        name: b.name,
        company_id: tenant.company_id,
        picking_type_id: b.picking_type_id,
        location_id: b.location_id,
        location_dest_id: b.location_dest_id,
        partner_id: b.partner_id,
        move_type: b.move_type,
        origin: b.origin,
        lines: b.lines.into_iter().map(|l| crate::application::service::inventory_transfer::PickingLine {
            item_id: l.item_id, demand_qty: l.demand_qty, price_unit: l.price_unit,
        }).collect(),
    };
    match svc.create_picking(p).await {
        Ok(created) => (StatusCode::CREATED, Json(PickingCreatedBody {
            transfer_id: created.transfer_id, move_ids: created.move_ids,
            projected_state: created.projected_state,
        })).into_response(),
        Err(e) => err(e),
    }
}

/// The projection probe: the transfer header (with its PROJECTED state — a stored compute
/// the move engine re-derived, never an assertion) plus the member move states that
/// aggregate into it.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MoveStateBody { id: Uuid, item_id: Uuid, state: String, demand_qty: Decimal, quantity: Decimal, is_inventory: bool }
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PickingProbeBody {
    id: Uuid, name: String, origin: Option<String>, picking_type_id: Uuid,
    location_id: Uuid, location_dest_id: Uuid, state: String,
    date_done: Option<chrono::DateTime<chrono::Utc>>,
    moves: Vec<MoveStateBody>,
}
async fn get_picking(
    State(svc): State<Arc<InventoryWriteService>>,
    tenant: CompanyContext,
    axum::extract::Path(transfer_id): axum::extract::Path<Uuid>,
) -> axum::response::Response {
    match svc.fetch_picking(tenant.company_id, transfer_id).await {
        Ok((h, moves)) => (StatusCode::OK, Json(PickingProbeBody {
            id: h.id, name: h.name, origin: h.origin, picking_type_id: h.picking_type_id,
            location_id: h.location_id, location_dest_id: h.location_dest_id,
            state: h.state, date_done: h.date_done,
            moves: moves.into_iter().map(|m| MoveStateBody {
                id: m.id, item_id: m.item_id, state: m.state, demand_qty: m.demand_qty,
                quantity: m.quantity, is_inventory: m.is_inventory,
            }).collect(),
        })).into_response(),
        Err(e) => err(e),
    }
}

// ── quant-driven adjustment door (spec stock §5.2 — no stock.inventory model) ──
//
// Staging a count is a pure quant-surface write (no GL, no move): the counted LEVEL lands
// on the quant with its stored diff compute, gated for Apply. Applying mints the
// `is_inventory` move and may post the adjustment GL leg — a service/job surface (GL sink),
// not a bare route.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StageCountBody { item_id: Uuid, location_id: Uuid, counted_qty: Decimal }
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct StagedCountBody {
    quant_id: Uuid, location_id: Uuid,
    on_hand_qty: Decimal, counted_qty: Decimal, diff_qty: Decimal,
}
async fn stage_count(
    State(svc): State<Arc<InventoryWriteService>>,
    tenant: CompanyContext,
    Json(b): Json<StageCountBody>,
) -> axum::response::Response {
    match svc.stage_quant_count(tenant.company_id, b.item_id, b.location_id, b.counted_qty).await {
        Ok(s) => (StatusCode::CREATED, Json(StagedCountBody {
            quant_id: s.quant_id, location_id: s.location_id,
            on_hand_qty: s.on_hand_qty, counted_qty: s.counted_qty, diff_qty: s.diff_qty,
        })).into_response(),
        Err(e) => err(e),
    }
}

/// The pending-count worklist at a location (the staged-but-unapplied counts — the partial
/// index `WHERE inventory_quantity_set = true` serves this read).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StagedCountsQuery { location_id: Uuid }
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct StagedCountRowBody {
    quant_id: Uuid, item_id: Uuid,
    on_hand_qty: Decimal, counted_qty: Option<Decimal>, diff_qty: Option<Decimal>,
}
async fn get_staged_counts(
    State(svc): State<Arc<InventoryWriteService>>,
    tenant: CompanyContext,
    Query(q): Query<StagedCountsQuery>,
) -> axum::response::Response {
    match svc.staged_counts(tenant.company_id, q.location_id).await {
        Ok(rows) => (StatusCode::OK, Json(rows.into_iter().map(|r| StagedCountRowBody {
            quant_id: r.quant_id, item_id: r.item_id,
            on_hand_qty: r.on_hand_qty, counted_qty: r.counted_qty, diff_qty: r.diff_qty,
        }).collect::<Vec<_>>())).into_response(),
        Err(e) => err(e),
    }
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

/// The projection/adjustment PROBES (tenant-fenced reads): the picking's projected state,
/// the batch projection (header + member states), and the pending-count worklist. Same
/// `company_auth` + `route_layer` posture as the writes — these reads take the tenant from
/// the token, so a caller cannot probe another company's transfers or staged counts.
fn probe_routes(svc: Arc<InventoryWriteService>, verifier: CompanyVerifier) -> Router {
    Router::new()
        .route("/pickings/:id", axum::routing::get(get_picking))
        .route("/counts", axum::routing::get(get_staged_counts))
        // The hyphenated batch path is this door's alone (the generated picking-batch read
        // surface mounts at the underscore form `/picking_batches`), so the projection probe
        // owns it: the batch header WITH its member pickings' states.
        .route("/picking-batches/:id", axum::routing::get(get_batch))
        .route_layer(from_fn_with_state(verifier, company_auth))
        .with_state(svc)
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
        // Procurement configuration reads (routes, rules, orderpoints) ride the same tenant
        // fence. Orderpoints are read-only here by design: the `manual` trigger surfaces in the
        // replenishment view for a human to order against; the computes recommend, and the
        // ordering verbs stay on the service/job surface (`ProcurementService`).
        .merge(create_route_read_routes(m.route_service.clone()))
        .merge(create_route_rule_read_routes(m.route_rule_service.clone()))
        .merge(create_reordering_rule_read_routes(m.reordering_rule_service.clone()))
        // Satellite + master-data reads (batch/scrap documents; package/storage/putaway
        // vocabulary): same tenant fence. The DB-level guards (uniques, CHECKs, the T12
        // trigger) backstop every writer on these tables.
        .merge(create_picking_batch_read_routes(m.picking_batch_service.clone()))
        .merge(create_scrap_read_routes(m.scrap_service.clone()))
        .merge(create_scrap_reason_tag_read_routes(m.scrap_reason_tag_service.clone()))
        .merge(create_package_type_read_routes(m.package_type_service.clone()))
        .merge(create_storage_category_read_routes(m.storage_category_service.clone()))
        .merge(create_storage_category_capacity_read_routes(m.storage_category_capacity_service.clone()))
        .merge(create_putaway_rule_read_routes(m.putaway_rule_service.clone()))
        .route_layer(from_fn_with_state(verifier.clone(), company_auth));
    Router::new()
        .merge(entity_reads)
        .merge(write_routes(write.clone(), verifier.clone()))
        .merge(read_routes(read))
        .merge(intake_routes(intake, verifier.clone()))
        .merge(probe_routes(write, verifier))
}
