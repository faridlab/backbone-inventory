//! Picking-batch (SB-1) cases. Requires DATABASE_URL (default :5433/backbone_inventory),
//! schema applied.
//!
//! **The batch state is a PROJECTION (spec misc-features.md SB / ADR-0016):** every
//! assertion here READS the projection (`fetch_batch`) after driving MEMBER changes; no
//! test writes a batch state, and the service exposes no way to. Membership moves the
//! pointer; the engine's verbs change the member pickings; the stored compute derives the
//! batch in both cases (the membership verbs reproject in-transaction, and the picking
//! projection cascades the recompute on every member-state change).

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use rust_decimal::Decimal;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use backbone_inventory::application::service::inventory_gl::{
    AccountingPostEnvelope, GlPostAck, GlPostRejected, GlPostSink,
};
use backbone_inventory::application::service::inventory_move_engine::MoveGlDirective;
use backbone_inventory::application::service::inventory_transfer::PickingLine;
use backbone_inventory::application::service::inventory_write_service::{
    InventoryError, InventoryWriteService, NewWarehouse,
};

mod common;

struct CountingSink { posts: AtomicUsize }
#[async_trait::async_trait]
impl GlPostSink for CountingSink {
    async fn post(&self, _e: &AccountingPostEnvelope) -> Result<GlPostAck, GlPostRejected> {
        self.posts.fetch_add(1, Ordering::SeqCst);
        Ok(GlPostAck { post_id: Uuid::new_v4(), journal_id: Uuid::new_v4(), idempotent_reuse: false })
    }
}
fn counting_sink() -> Arc<CountingSink> { Arc::new(CountingSink { posts: AtomicUsize::new(0) }) }

fn d(s: &str) -> Decimal { Decimal::from_str_exact(s).unwrap() }
fn uq(p: &str) -> String { format!("{p}-{}", &Uuid::new_v4().simple().to_string()[..8]) }
async fn pool() -> PgPool {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:postgres@localhost:5433/backbone_inventory".to_string());
    PgPool::connect(&url).await.expect("connect DB")
}

fn gl() -> MoveGlDirective {
    MoveGlDirective {
        cogs_account_id: Some(Uuid::new_v4()),
        inventory_account_id: Some(Uuid::new_v4()),
        grir_account_id: Some(Uuid::new_v4()),
        adjustment_account_id: Some(Uuid::new_v4()),
        currency: "IDR".into(),
    }
}

async fn warehouse(w: &InventoryWriteService, company: Uuid) -> Uuid {
    w.create_warehouse(NewWarehouse {
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

async fn op_type(pool: &PgPool, company: Uuid, code: &str, reservation: &str, src: Uuid, dst: Uuid) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO inventory.operation_types
             (id, name, sequence_code, code, company_id,
              default_location_src_id, default_location_dest_id, reservation_method, create_backorder)
           VALUES ($1,$2,$3,$4::picking_code,$5,$6,$7,$8::reservation_method,'ask'::create_backorder)"#,
    )
    .bind(id).bind(uq("PT")).bind(uq("SEQ")).bind(code).bind(company)
    .bind(src).bind(dst).bind(reservation)
    .execute(pool).await.unwrap();
    id
}

async fn seed_bin(pool: &PgPool, company: Uuid, item: Uuid, wh: Uuid, qty: &str, rate: &str) {
    sqlx::query(
        r#"INSERT INTO inventory.bins
             (id, company_id, item_id, warehouse_id, actual_qty, reserved_qty, valuation_rate, stock_value)
           VALUES ($1,$2,$3,$4,$5,0,$6,$7)"#,
    )
    .bind(Uuid::new_v4()).bind(company).bind(item).bind(wh)
    .bind(d(qty)).bind(d(rate)).bind(d(qty) * d(rate))
    .execute(pool).await.unwrap();
}

async fn seed_quant(pool: &PgPool, company: Uuid, item: Uuid, location: Uuid, qty: &str) {
    sqlx::query(
        r#"INSERT INTO inventory.stock_quants
             (id, item_id, location_id, quantity, reserved_quantity, available_quantity, company_id)
           VALUES ($1,$2,$3,$4,0,$4,$5)"#,
    )
    .bind(Uuid::new_v4()).bind(item).bind(location).bind(d(qty)).bind(company)
    .execute(pool).await.unwrap();
}

/// Stage one member picking in the given projected state and return its transfer id.
/// `confirmed` / `assigned` mint through the real service verbs; the special bands are
/// staged the way the engine itself would leave them, as documented per call site.
async fn member_picking(
    svc: &InventoryWriteService,
    pool: &PgPool,
    company: Uuid,
    wh: Uuid,
    reservation: &str,
) -> Uuid {
    let supplier = loc(pool, company, "supplier", None).await;
    let stock = loc(pool, company, "internal", Some(wh)).await;
    let op = op_type(pool, company, "incoming", reservation, supplier, stock).await;
    let item = Uuid::new_v4();
    let created = svc.create_picking(backbone_inventory::application::service::inventory_transfer::NewPicking {
        name: uq("BP"), company_id: company, picking_type_id: op,
        location_id: supplier, location_dest_id: stock, partner_id: None,
        move_type: "direct".into(), origin: None,
        lines: vec![PickingLine { item_id: item, demand_qty: d("5"), price_unit: d("1") }],
    }).await.unwrap();
    created.transfer_id
}

// ── SB-1: the batch state derives from its members ───────────────────────────

/// Mint → draft. Members join and leave; the projection follows — never a hand-set state.
/// Mixed member states aggregate to the LEAST-advanced live member (a confirmed member
/// holds the batch at `draft`; all assigned → `ready`; all done → `done`).
#[tokio::test]
async fn batch_state_derives_from_members() {
    let pool = pool().await;
    let svc = InventoryWriteService::new(pool.clone());
    let company = common::fresh_company(&pool).await;
    let wh = warehouse(&svc, company).await;

    // A fresh batch has never grouped a picking: it reads `draft`.
    let header = svc.create_batch(company, uq("BATCH"), false, None).await.unwrap();
    assert_eq!(header.state, "draft");
    assert!(!header.had_members);
    let batch_id = header.id;

    // Member A: manual reservation → its moves stay `confirmed` (the least-advanced band).
    let a = member_picking(&svc, &pool, company, wh, "manual").await;
    // Member B: at_confirm → inbound assign mints the execution line → `assigned`.
    let b = member_picking(&svc, &pool, company, wh, "at_confirm").await;
    for (picking, want) in [(a, "confirmed"), (b, "assigned")] {
        let (h, _) = svc.fetch_picking(company, picking).await.unwrap();
        assert_eq!(h.state, want, "member staging sanity for {picking}");
    }

    // Empty → member A (confirmed) → member B joins (assigned): the batch reads the
    // LEAST-advanced live member — `draft` band while A is open below waiting.
    svc.add_picking_to_batch(company, batch_id, a).await.unwrap();
    let (h, members) = svc.fetch_batch(company, batch_id).await.unwrap();
    assert_eq!(h.state, "draft");
    assert_eq!(members.len(), 1);

    svc.add_picking_to_batch(company, batch_id, b).await.unwrap();
    let (h, members) = svc.fetch_batch(company, batch_id).await.unwrap();
    assert_eq!(h.state, "draft", "least-advanced live member wins over the assigned one");
    assert_eq!(members.len(), 2);
    assert!(h.had_members);

    // Detach the confirmed member: only the assigned one remains → the batch re-projects
    // to `ready` (adding/removing a member re-derives the stored compute).
    svc.remove_picking_from_batch(company, batch_id, a).await.unwrap();
    let (h, members) = svc.fetch_batch(company, batch_id).await.unwrap();
    assert_eq!(h.state, "ready");
    assert_eq!(members.len(), 1);

    // Re-attach the confirmed member → back to `draft` (a re-add re-derives too).
    svc.add_picking_to_batch(company, batch_id, a).await.unwrap();
    let (h, _) = svc.fetch_batch(company, batch_id).await.unwrap();
    assert_eq!(h.state, "draft");
}

/// The engine cascade: driving a MEMBER to done through the picking verbs (no batch verb
/// involved) re-derives the batch in the same transaction — the batch never lags.
#[tokio::test]
async fn engine_cascade_reprojects_the_batch() {
    let pool = pool().await;
    let svc = InventoryWriteService::new(pool.clone());
    let company = common::fresh_company(&pool).await;
    let wh = warehouse(&svc, company).await;
    let batch_id = svc.create_batch(company, uq("BATCH"), true, None).await.unwrap().id;

    let a = member_picking(&svc, &pool, company, wh, "manual").await;
    let b = member_picking(&svc, &pool, company, wh, "manual").await;
    svc.add_picking_to_batch(company, batch_id, a).await.unwrap();
    svc.add_picking_to_batch(company, batch_id, b).await.unwrap();
    let (h, _) = svc.fetch_batch(company, batch_id).await.unwrap();
    assert_eq!(h.state, "draft");

    // Validate ONE member (the engine's `_action_done` over its moves — the picking
    // re-projects, and the cascade rolls the batch up with it).
    let sink = counting_sink();
    svc.validate_picking(company, a, &gl(), &*sink).await.unwrap();
    let (h, _) = svc.fetch_picking(company, a).await.unwrap();
    assert_eq!(h.state, "done");
    let (h, _) = svc.fetch_batch(company, batch_id).await.unwrap();
    assert_eq!(h.state, "draft", "one done + one confirmed member: least-advanced wins");

    svc.validate_picking(company, b, &gl(), &*sink).await.unwrap();
    let (h, _) = svc.fetch_batch(company, batch_id).await.unwrap();
    assert_eq!(h.state, "done", "every live member done → the batch reads done");
}

/// Cancelling every member leaves a live-less batch → the projection reads `cancel`
/// (cancelled members drop out of the aggregation entirely).
#[tokio::test]
async fn all_members_cancelled_cancels_the_batch() {
    let pool = pool().await;
    let svc = InventoryWriteService::new(pool.clone());
    let company = common::fresh_company(&pool).await;
    let wh = warehouse(&svc, company).await;
    let batch_id = svc.create_batch(company, uq("BATCH"), false, None).await.unwrap().id;

    let a = member_picking(&svc, &pool, company, wh, "manual").await;
    svc.add_picking_to_batch(company, batch_id, a).await.unwrap();

    // Cancel the member's moves through the engine; the picking re-projects to cancel and
    // the cascade carries the batch.
    let (_, moves) = svc.fetch_picking(company, a).await.unwrap();
    for m in moves {
        svc.action_cancel(company, m.id).await.unwrap();
    }
    let (h, _) = svc.fetch_picking(company, a).await.unwrap();
    assert_eq!(h.state, "cancel");
    let (h, members) = svc.fetch_batch(company, batch_id).await.unwrap();
    assert_eq!(h.state, "cancel");
    assert_eq!(members.len(), 1, "the cancelled member still groups — it only drops out of the aggregation");
}

/// SB-1 auto-cancel on empty: detaching the LAST member cancels the batch (an emptied
/// batch must not linger as a live work item), while a batch that never grouped one
/// stays `draft`.
#[tokio::test]
async fn emptying_the_batch_auto_cancels() {
    let pool = pool().await;
    let svc = InventoryWriteService::new(pool.clone());
    let company = common::fresh_company(&pool).await;
    let wh = warehouse(&svc, company).await;
    let batch_id = svc.create_batch(company, uq("BATCH"), false, None).await.unwrap().id;

    // Never grouped a picking: still draft after a probe.
    let (h, _) = svc.fetch_batch(company, batch_id).await.unwrap();
    assert_eq!(h.state, "draft");

    let a = member_picking(&svc, &pool, company, wh, "manual").await;
    svc.add_picking_to_batch(company, batch_id, a).await.unwrap();
    // Detaching the only member empties a batch that HAS grouped work → cancel.
    svc.remove_picking_from_batch(company, batch_id, a).await.unwrap();
    let (h, members) = svc.fetch_batch(company, batch_id).await.unwrap();
    assert_eq!(h.state, "cancel");
    assert_eq!(members.len(), 0);
}

/// The `waiting` band: a member picking projecting `waiting` holds the batch at
/// `waiting` (rank between draft and assigned). The member is staged the way the engine
/// leaves chained pickings — its moves wait on a not-yet-done parent — by writing the
/// transfer's projected state directly (the projection column the engine owns; no
/// product code path writes it but the recompute).
#[tokio::test]
async fn waiting_member_holds_batch_at_waiting() {
    let pool = pool().await;
    let svc = InventoryWriteService::new(pool.clone());
    let company = common::fresh_company(&pool).await;
    let wh = warehouse(&svc, company).await;
    let batch_id = svc.create_batch(company, uq("BATCH"), false, None).await.unwrap().id;

    let a = member_picking(&svc, &pool, company, wh, "manual").await;
    let b = member_picking(&svc, &pool, company, wh, "at_confirm").await;
    // Stage one member's projection at `waiting` (the state the recompute itself would
    // store for a picking whose every move waits on an undone parent).
    sqlx::query("UPDATE inventory.transfers SET state = 'waiting'::transfer_state WHERE id = $1")
        .bind(a).execute(&pool).await.unwrap();

    svc.add_picking_to_batch(company, batch_id, a).await.unwrap();
    let (h, _) = svc.fetch_batch(company, batch_id).await.unwrap();
    assert_eq!(h.state, "waiting", "a single waiting member holds the batch at waiting");

    // An assigned member joins: the waiting member is still the least-advanced live one —
    // the batch stays at `waiting`.
    svc.add_picking_to_batch(company, batch_id, b).await.unwrap();
    let (h, _) = svc.fetch_batch(company, batch_id).await.unwrap();
    assert_eq!(h.state, "waiting");
    // Detach the WAITING member: only the assigned one remains → the band jumps to ready —
    // removal re-derives too.
    svc.remove_picking_from_batch(company, batch_id, a).await.unwrap();
    let (h, _) = svc.fetch_batch(company, batch_id).await.unwrap();
    assert_eq!(h.state, "ready");
    // Re-attach the waiting member: back down to waiting — a re-add re-derives as well.
    svc.add_picking_to_batch(company, batch_id, a).await.unwrap();
    let (h, _) = svc.fetch_batch(company, batch_id).await.unwrap();
    assert_eq!(h.state, "waiting");
}

// ── guards ────────────────────────────────────────────────────────────────────

/// R3-shaped: the batch name is unique per company — the typed duplicate error — and a
/// different company may reuse it.
#[tokio::test]
async fn batch_name_unique_per_company() {
    let pool = pool().await;
    let svc = InventoryWriteService::new(pool.clone());
    let company = common::fresh_company(&pool).await;
    let name = uq("BATCH");
    svc.create_batch(company, name.clone(), false, None).await.unwrap();
    let err = svc.create_batch(company, name.clone(), false, None).await.unwrap_err();
    assert!(matches!(err, InventoryError::DuplicateNumber(_)), "got {err:?}");
    svc.create_batch(Uuid::new_v4(), name, false, None).await.unwrap();
}

/// Cross-company: a wrong-company batch reads as NotFound (the fence — never a leak),
/// and a picking of ANOTHER company cannot join this company's batch (NotFound again).
#[tokio::test]
async fn cross_company_refusals() {
    let pool = pool().await;
    let svc = InventoryWriteService::new(pool.clone());
    let company = common::fresh_company(&pool).await;
    let other = common::fresh_company(&pool).await;
    let wh = warehouse(&svc, company).await;
    let wh_other = warehouse(&svc, other).await;
    let batch_id = svc.create_batch(company, uq("BATCH"), false, None).await.unwrap().id;

    // The other company's own batch is invisible to this company: NotFound, not a row.
    let other_batch = svc.create_batch(other, uq("BATCH"), false, None).await.unwrap().id;
    let err = svc.fetch_batch(company, other_batch).await.unwrap_err();
    assert!(matches!(err, InventoryError::NotFound(_)), "got {err:?}");

    // A picking of the other company cannot join this company's batch.
    let foreign = member_picking(&svc, &pool, other, wh_other, "manual").await;
    let err = svc.add_picking_to_batch(company, batch_id, foreign).await.unwrap_err();
    assert!(matches!(err, InventoryError::NotFound(_)), "got {err:?}");
    // And the reverse: this company's picking into the other company's batch.
    let own = member_picking(&svc, &pool, company, wh, "manual").await;
    let err = svc.add_picking_to_batch(other, other_batch, own).await.unwrap_err();
    assert!(matches!(err, InventoryError::NotFound(_)), "got {err:?}");
}

/// Terminal refusals: a picking may only JOIN while non-terminal, a terminal batch
/// refuses membership changes, a picking belongs to at most one batch, and removing a
/// non-member is the typed not-a-member error.
#[tokio::test]
async fn membership_guards() {
    let pool = pool().await;
    let svc = InventoryWriteService::new(pool.clone());
    let company = common::fresh_company(&pool).await;
    let wh = warehouse(&svc, company).await;
    let batch_id = svc.create_batch(company, uq("BATCH"), false, None).await.unwrap().id;
    let second_batch = svc.create_batch(company, uq("BATCH"), false, None).await.unwrap().id;

    // A DONE picking cannot join (validate a member fully first).
    let done = member_picking(&svc, &pool, company, wh, "manual").await;
    let sink = counting_sink();
    svc.validate_picking(company, done, &gl(), &*sink).await.unwrap();
    let err = svc.add_picking_to_batch(company, batch_id, done).await.unwrap_err();
    assert!(matches!(err, InventoryError::PickingTerminalForBatch { .. }), "got {err:?}");

    // A CANCELLED picking cannot join either.
    let cancelled = member_picking(&svc, &pool, company, wh, "manual").await;
    let (_, moves) = svc.fetch_picking(company, cancelled).await.unwrap();
    for m in moves {
        svc.action_cancel(company, m.id).await.unwrap();
    }
    let err = svc.add_picking_to_batch(company, batch_id, cancelled).await.unwrap_err();
    assert!(matches!(err, InventoryError::PickingTerminalForBatch { .. }), "got {err:?}");

    // One batch per picking: a member of the first batch refuses the second.
    let member = member_picking(&svc, &pool, company, wh, "manual").await;
    svc.add_picking_to_batch(company, batch_id, member).await.unwrap();
    let err = svc.add_picking_to_batch(company, second_batch, member).await.unwrap_err();
    assert!(matches!(err, InventoryError::PickingAlreadyBatched { .. }), "got {err:?}");

    // A batch the engine drove to done refuses NEW members (its work is finished).
    let a = member_picking(&svc, &pool, company, wh, "manual").await;
    let b = member_picking(&svc, &pool, company, wh, "manual").await;
    svc.add_picking_to_batch(company, second_batch, a).await.unwrap();
    svc.add_picking_to_batch(company, second_batch, b).await.unwrap();
    svc.validate_picking(company, a, &gl(), &*sink).await.unwrap();
    svc.validate_picking(company, b, &gl(), &*sink).await.unwrap();
    let (h, _) = svc.fetch_batch(company, second_batch).await.unwrap();
    assert_eq!(h.state, "done");
    let c = member_picking(&svc, &pool, company, wh, "manual").await;
    let err = svc.add_picking_to_batch(company, second_batch, c).await.unwrap_err();
    assert!(matches!(err, InventoryError::BatchTerminal { .. }), "got {err:?}");
    // Removal stays allowed on a terminal batch (that is how a finished list is cleaned).
    svc.remove_picking_from_batch(company, second_batch, a).await.unwrap();

    // Removing a picking that is not a member: the typed not-a-member error.
    let err = svc.remove_picking_from_batch(company, batch_id, c).await.unwrap_err();
    assert!(matches!(err, InventoryError::NotABatchMember { .. }), "got {err:?}");
}
