//! The move GL-leg settlement surface: every path that posts a move's own AccountingPost
//! records the outcome on the move (`posting_state`), a rejected post is a durable `failed`
//! — never a silent hole — and the service-layer repost verb re-drives it idempotently
//! through the REAL backbone-accounting ledger.
//!
//! Covers the engine post path end to end (reject → failed → repost heals → posted), the
//! crash-window dedupe (re-drive cannot double post), and the not-applicable contract: moves
//! whose GL is owned by their voucher door (receipt/delivery legs and their reversals) and
//! value-neutral warehouse-to-warehouse shapes never claim a GL leg of their own.
//!
//! Requires DATABASE_URL (:5433/backbone_inventory), inventory + accounting schemas applied.

use std::collections::HashMap;
use std::sync::Arc;

use rust_decimal::Decimal;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use backbone_inventory::application::service::inventory_gl::{
    AccountingPostEnvelope, GlPostAck, GlPostRejected, GlPostSink,
};
use backbone_inventory::application::service::inventory_move_engine::{
    BackorderPolicy, MoveGlDirective, NewStockMove,
};
use backbone_inventory::application::service::inventory_write_service::{
    InventoryWriteService, NewReceipt, NewWarehouse, ReceiptLine,
};
use backbone_accounting::application::service::posting_service::{
    PostingLine, PostingRequest, PostingService,
};
use backbone_accounting::infrastructure::persistence::SqlxPostingRepository;

/// Map inventory's envelope into accounting's real PostingService — the same ACL the composing
/// service ships. A repost through this sink lands a real journal, proving the heal end to end.
struct AccountingAdapter { svc: PostingService }
#[async_trait::async_trait]
impl GlPostSink for AccountingAdapter {
    async fn post(&self, env: &AccountingPostEnvelope) -> Result<GlPostAck, GlPostRejected> {
        let mut req = PostingRequest::original(env.company_id, &env.source_type, env.source_id, env.posting_date);
        req.branch_id = env.branch_id;
        req.source_reference = env.source_reference.clone();
        req.currency = env.currency.clone();
        req.description = env.description.clone();
        req.lines = env.lines.iter().map(|l| PostingLine {
            account_id: l.account_id, debit: l.debit, credit: l.credit,
            party_type: l.party_type.clone(), party_id: l.party_id,
            cost_center_id: None, project_id: None, department_id: None, description: l.description.clone(),
        }).collect();
        match self.svc.post(req, None).await {
            Ok(r) => Ok(GlPostAck { post_id: r.post_id, journal_id: r.journal_id, idempotent_reuse: r.idempotent_reuse }),
            Err(e) => Err(GlPostRejected { code: e.code().to_string(), message: e.to_string() }),
        }
    }
}

/// A sink that always rejects — simulates a transient accounting outage.
struct FailingGl;
#[async_trait::async_trait]
impl GlPostSink for FailingGl {
    async fn post(&self, _e: &AccountingPostEnvelope) -> Result<GlPostAck, GlPostRejected> {
        Err(GlPostRejected { code: "period_closed".into(), message: "transient".into() })
    }
}

mod common;

fn d(s: &str) -> Decimal { Decimal::from_str_exact(s).unwrap() }
fn day() -> chrono::NaiveDate { chrono::NaiveDate::from_ymd_opt(2026, 8, 26).unwrap() }
fn uq(p: &str) -> String { format!("{p}-{}", &Uuid::new_v4().simple().to_string()[..8]) }
async fn pool() -> PgPool {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:postgres@localhost:5433/backbone_inventory".to_string());
    PgPool::connect(&url).await.expect("connect DB")
}

/// Seed a real chart of accounts: asset Inventory, liability GR/IR, COGS, adjustment, and a
/// non-postable asset HEADER (the rejection lever).
async fn seed_coa(pool: &PgPool) -> (Uuid, HashMap<&'static str, Uuid>) {
    let company = common::fresh_company(pool).await;
    let coa: &[(&str, &str, &str, &str, &str, bool, bool)] = &[
        ("1000", "Header Aset", "asset", "current_asset", "debit", true, false),
        ("1300", "Persediaan", "asset", "inventory", "debit", false, true),
        ("2150", "GR/IR Clearing", "liability", "current_liability", "credit", false, true),
        ("5100", "HPP (COGS)", "cogs", "direct_cost", "debit", false, true),
        ("5200", "Selisih Persediaan", "expense", "operating_expense", "debit", false, true),
    ];
    let mut m = HashMap::new();
    for (code, name, at, st, nb, is_header, is_detail) in coa {
        let id = Uuid::new_v4();
        sqlx::query(
            r#"INSERT INTO accounting.accounts
                (id, company_id, account_number, account_code, name, account_type, account_subtype,
                 normal_balance, is_header, is_detail, status)
               VALUES ($1,$2,$3,$4,$5,$6::account_type,$7::account_subtype,$8::normal_balance,$9,$10,'active'::account_status)"#,
        )
        .bind(id).bind(company).bind(code).bind(code).bind(name).bind(at).bind(st).bind(nb).bind(is_header).bind(is_detail)
        .execute(pool).await.expect("seed account");
        m.insert(*code, id);
    }
    (company, m)
}

async fn warehouse(w: &InventoryWriteService, company: Uuid) -> Uuid {
    w.create_warehouse(NewWarehouse {
        org_unit_id: company, code: uq("WH"), name: uq("Main"),
        warehouse_type: None, parent_warehouse_id: None, is_group: false,
    }).await.unwrap()
}

/// Insert a location row (usage: internal/customer/supplier...). `warehouse_id` binds the
/// valuation bin an internal location resolves to.
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

/// Seed on-hand stock at a location (the quant grain: one row, untracked dims).
async fn seed_quant(pool: &PgPool, company: Uuid, item: Uuid, location: Uuid, qty: &str) {
    sqlx::query(
        r#"INSERT INTO inventory.stock_quants
             (id, item_id, location_id, quantity, reserved_quantity, available_quantity, company_id)
           VALUES ($1,$2,$3,$4,0,$4,$5)"#,
    )
    .bind(Uuid::new_v4()).bind(item).bind(location).bind(d(qty)).bind(company)
    .execute(pool).await.unwrap();
}

/// Seed a Bin running balance (item x warehouse) for the valuation core.
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

fn new_move(company: Uuid, item: Uuid, src: Uuid, dst: Uuid, qty: &str) -> NewStockMove {
    NewStockMove {
        name: uq("MV"),
        company_id: company,
        item_id: item,
        demand_qty: d(qty),
        price_unit: Decimal::ZERO,
        procure_method: "make_to_stock".into(),
        picking_id: None,
        origin: None,
        location_id: src,
        location_dest_id: dst,
        partner_id: None,
        warehouse_id: None,
        orderpoint_id: None,
        move_orig_ids: vec![],
        move_dest_ids: vec![],
        is_inventory: false,
        scrapped: false,
        forced_value: None,
    }
}

/// Drive a move draft → confirmed → assigned → done with the given directive + sink.
async fn drive_to_done(w: &InventoryWriteService, mv: NewStockMove, gl: &MoveGlDirective, sink: &dyn GlPostSink) -> Result<backbone_inventory::application::service::inventory_move_engine::MoveDoneOutcome, backbone_inventory::application::service::inventory_write_service::InventoryError> {
    let company = mv.company_id;
    let id = w.create_move(mv).await.unwrap();
    w.action_confirm(company, id).await.unwrap();
    w.action_assign(company, id).await.unwrap();
    w.action_done(company, id, BackorderPolicy::Never, gl, sink).await
}

async fn posting_state_of(pool: &PgPool, id: Uuid) -> String {
    sqlx::query_scalar("SELECT posting_state::text FROM inventory.stock_moves WHERE id=$1")
        .bind(id).fetch_one(pool).await.unwrap()
}

async fn state_of(pool: &PgPool, id: Uuid) -> String {
    sqlx::query_scalar("SELECT state::text FROM inventory.stock_moves WHERE id=$1")
        .bind(id).fetch_one(pool).await.unwrap()
}

async fn journal_count(pool: &PgPool, company: Uuid) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM accounting.journals WHERE company_id=$1")
        .bind(company).fetch_one(pool).await.unwrap()
}

fn out_gl(cogs: Uuid, inv: Uuid) -> MoveGlDirective {
    MoveGlDirective {
        cogs_account_id: Some(cogs),
        inventory_account_id: Some(inv),
        grir_account_id: None,
        adjustment_account_id: None,
        currency: "IDR".into(),
    }
}

// IGLR-1 (the probe): a rejected move post is a durable failed state, never a silent hole —
// the physical movement stands, and the repost verb heals the GL leg through the real ledger.
#[tokio::test]
async fn rejected_post_marks_failed_and_repost_heals() {
    let pool = pool().await;
    let (company, coa) = seed_coa(&pool).await;
    let w = InventoryWriteService::new(pool.clone());
    let adapter = AccountingAdapter { svc: PostingService::new(Arc::new(SqlxPostingRepository::new(pool.clone()))) };
    let wh = warehouse(&w, company).await;
    let item = Uuid::new_v4();
    let stock = loc(&pool, company, "internal", Some(wh)).await;
    let customer = loc(&pool, company, "customer", None).await;
    seed_quant(&pool, company, item, stock, "10").await;
    seed_bin(&pool, company, item, wh, "10", "100").await;

    // Transient outage: the done path commits the physical movement, then the post rejects.
    let err = drive_to_done(&w, new_move(company, item, stock, customer, "4"), &out_gl(coa["5100"], coa["1300"]), &FailingGl)
        .await.unwrap_err();
    assert_eq!(err.code(), "period_closed");
    let mv_id: Uuid = sqlx::query_scalar("SELECT id FROM inventory.stock_moves WHERE company_id=$1 ORDER BY create_date DESC LIMIT 1")
        .bind(company).fetch_one(&pool).await.unwrap();

    // The physical movement stands: move done, stock really moved, NO journal anywhere.
    assert_eq!(state_of(&pool, mv_id).await, "done");
    assert_eq!(posting_state_of(&pool, mv_id).await, "failed", "the GL hole is recorded, not silent");
    let (at_cust,): (Decimal,) = sqlx::query_as(
        "SELECT COALESCE(SUM(quantity),0) FROM inventory.stock_quants WHERE company_id=$1 AND item_id=$2 AND location_id=$3",
    ).bind(company).bind(item).bind(customer).fetch_one(&pool).await.unwrap();
    assert_eq!(at_cust, d("4"), "the physical leg committed before the post was attempted");
    assert_eq!(journal_count(&pool, company).await, 0, "the GL leg is genuinely missing");

    // Repost with a healthy sink → posted, and the real journal carries the engine's valuation.
    let out = w.repost_move_gl(company, mv_id, &out_gl(coa["5100"], coa["1300"]), &adapter).await.unwrap();
    assert!(out.posted);
    assert_eq!(posting_state_of(&pool, mv_id).await, "posted");
    assert_eq!(journal_count(&pool, company).await, 1);
    let jid = out.journal_id.unwrap();
    let r = sqlx::query("SELECT total_debit, total_credit FROM accounting.journals WHERE id=$1")
        .bind(jid).fetch_one(&pool).await.unwrap();
    assert_eq!(r.get::<Decimal, _>("total_debit"), d("400.00"), "4 @ moving-average 100");
    assert_eq!(r.get::<Decimal, _>("total_credit"), d("400.00"));
    let cogs: Decimal = sqlx::query_scalar(
        "SELECT debit_amount FROM accounting.journal_lines WHERE journal_id=$1 AND account_id=$2",
    ).bind(jid).bind(coa["5100"]).fetch_one(&pool).await.unwrap();
    assert_eq!(cogs, d("400.00"), "Dr COGS");
}

// IGLR-2: a rejection from the REAL accounting service (a non-postable account in the
// directive) also parks the move in failed; reposting with a corrected directive posts.
#[tokio::test]
async fn real_rejection_parks_failed_and_corrected_directive_reposts() {
    let pool = pool().await;
    let (company, coa) = seed_coa(&pool).await;
    let w = InventoryWriteService::new(pool.clone());
    let adapter = AccountingAdapter { svc: PostingService::new(Arc::new(SqlxPostingRepository::new(pool.clone()))) };
    let wh = warehouse(&w, company).await;
    let item = Uuid::new_v4();
    let stock = loc(&pool, company, "internal", Some(wh)).await;
    let customer = loc(&pool, company, "customer", None).await;
    seed_quant(&pool, company, item, stock, "10").await;
    seed_bin(&pool, company, item, wh, "10", "100").await;

    // COGS pointed at a header account → the real posting service rejects it.
    let err = drive_to_done(&w, new_move(company, item, stock, customer, "2"), &out_gl(coa["1000"], coa["1300"]), &adapter)
        .await.unwrap_err();
    assert_eq!(err.code(), "non_postable_account");
    let mv_id: Uuid = sqlx::query_scalar("SELECT id FROM inventory.stock_moves WHERE company_id=$1 ORDER BY create_date DESC LIMIT 1")
        .bind(company).fetch_one(&pool).await.unwrap();
    assert_eq!(state_of(&pool, mv_id).await, "done");
    assert_eq!(posting_state_of(&pool, mv_id).await, "failed");

    // The directive is caller-supplied config — reposting with the corrected account posts.
    let out = w.repost_move_gl(company, mv_id, &out_gl(coa["5100"], coa["1300"]), &adapter).await.unwrap();
    assert!(out.posted);
    assert_eq!(posting_state_of(&pool, mv_id).await, "posted");
    assert_eq!(journal_count(&pool, company).await, 1);
}

// IGLR-3: repost is idempotent across the crash window — a re-drive of a leg that actually
// landed (status write lost) returns the ORIGINAL journal via accounting's dedupe, and an
// already-posted move short-circuits without re-emitting.
#[tokio::test]
async fn repost_does_not_double_post() {
    let pool = pool().await;
    let (company, coa) = seed_coa(&pool).await;
    let w = InventoryWriteService::new(pool.clone());
    let adapter = AccountingAdapter { svc: PostingService::new(Arc::new(SqlxPostingRepository::new(pool.clone()))) };
    let wh = warehouse(&w, company).await;
    let item = Uuid::new_v4();
    let stock = loc(&pool, company, "internal", Some(wh)).await;
    let customer = loc(&pool, company, "customer", None).await;
    seed_quant(&pool, company, item, stock, "10").await;
    seed_bin(&pool, company, item, wh, "10", "100").await;
    let gl = out_gl(coa["5100"], coa["1300"]);
    let id = w.create_move(new_move(company, item, stock, customer, "5")).await.unwrap();
    w.action_confirm(company, id).await.unwrap();
    w.action_assign(company, id).await.unwrap();
    let first = w.action_done(company, id, BackorderPolicy::Never, &gl, &adapter).await.unwrap();
    assert!(first.gl_posted);
    assert_eq!(posting_state_of(&pool, id).await, "posted");
    let orig_jid: Uuid = sqlx::query_scalar("SELECT id FROM accounting.journals WHERE company_id=$1")
        .bind(company).fetch_one(&pool).await.unwrap();

    // Simulate the crash window: the post landed but the status update was lost.
    sqlx::query("UPDATE inventory.stock_moves SET posting_state='failed'::gl_posting_state WHERE id=$1")
        .bind(id).execute(&pool).await.unwrap();
    let again = w.repost_move_gl(company, id, &gl, &adapter).await.unwrap();
    assert!(again.posted);
    assert_eq!(again.journal_id, Some(orig_jid), "dedupe returns the original journal");
    assert_eq!(journal_count(&pool, company).await, 1, "no second journal");

    // An already-posted move short-circuits.
    let noop = w.repost_move_gl(company, id, &gl, &adapter).await.unwrap();
    assert!(noop.posted);
    assert!(noop.journal_id.is_none(), "settled short-circuit re-emits nothing");
    assert_eq!(journal_count(&pool, company).await, 1);
}

// IGLR-4: moves whose GL is owned by their voucher door never claim a GL leg of their own —
// the receipt's line moves (and its cancellation's reverse moves) stay not_applicable while
// the VOUCHER carries the posting_state, and a blind repost on such a move is a no-op.
#[tokio::test]
async fn voucher_door_moves_stay_not_applicable() {
    let pool = pool().await;
    let (company, coa) = seed_coa(&pool).await;
    let w = InventoryWriteService::new(pool.clone());
    let adapter = AccountingAdapter { svc: PostingService::new(Arc::new(SqlxPostingRepository::new(pool.clone()))) };
    let wh = warehouse(&w, company).await;
    let item = Uuid::new_v4();
    let number = uq("PR");
    let rid = w.create_purchase_receipt(NewReceipt {
        receipt_number: number.clone(), company_id: company, branch_id: None,
        supplier_id: Uuid::new_v4(), source_po_id: None, warehouse_id: wh, posting_date: day(),
        currency: "IDR".into(),
        inventory_account_id: coa["1300"], grir_account_id: coa["2150"],
        lines: vec![ReceiptLine { item_id: item, quantity: d("10"), rate: d("100") , is_landed_costs_line: false }],
    }).await.unwrap();
    let out = w.submit_purchase_receipt(rid, &adapter).await.unwrap();
    assert!(out.posted, "the voucher owns the GL envelope");

    let states: Vec<(Uuid, String, String)> = sqlx::query_as(
        "SELECT id, state::text, posting_state::text FROM inventory.stock_moves WHERE company_id=$1 AND origin=$2",
    ).bind(company).bind(&number).fetch_all(&pool).await.unwrap();
    assert_eq!(states.len(), 1, "one line move");
    assert_eq!(states[0].1, "done");
    assert_eq!(states[0].2, "not_applicable", "the door's move posts no GL of its own");

    // A blind repost on a door-owned move is a no-op (sweep-safe).
    let noop = w.repost_move_gl(company, states[0].0, &out_gl(coa["5100"], coa["1300"]), &adapter).await.unwrap();
    assert!(!noop.posted);
    assert_eq!(journal_count(&pool, company).await, 1, "still exactly the voucher's journal");

    // Cancel: the reverse moves are engine legs too — also not_applicable; the voucher's
    // reversal post is the door's.
    w.cancel_purchase_receipt(rid, &adapter).await.unwrap();
    let states: Vec<(Uuid, String, String)> = sqlx::query_as(
        "SELECT id, state::text, posting_state::text FROM inventory.stock_moves WHERE company_id=$1 AND origin=$2",
    ).bind(company).bind(&number).fetch_all(&pool).await.unwrap();
    assert_eq!(states.len(), 2, "forward + reverse move");
    for (_, state, posting) in &states {
        assert_eq!(state, "done");
        assert_eq!(posting, "not_applicable", "reverse legs post through the door's reversal envelope");
    }
}

// IGLR-5: a value-neutral warehouse-to-warehouse move builds no envelope and stays
// not_applicable — there is no GL leg to settle, and repost agrees.
#[tokio::test]
async fn value_neutral_move_stays_not_applicable() {
    let pool = pool().await;
    let (company, coa) = seed_coa(&pool).await;
    let w = InventoryWriteService::new(pool.clone());
    let adapter = AccountingAdapter { svc: PostingService::new(Arc::new(SqlxPostingRepository::new(pool.clone()))) };
    let wh1 = warehouse(&w, company).await;
    let wh2 = warehouse(&w, company).await;
    let item = Uuid::new_v4();
    let a = loc(&pool, company, "internal", Some(wh1)).await;
    let b = loc(&pool, company, "internal", Some(wh2)).await;
    seed_quant(&pool, company, item, a, "10").await;
    seed_bin(&pool, company, item, wh1, "10", "100").await;
    seed_bin(&pool, company, item, wh2, "0", "0").await;

    // Directive carries real accounts — the shape still posts nothing (value-neutral contract).
    let out = drive_to_done(&w, new_move(company, item, a, b, "3"), &out_gl(coa["5100"], coa["1300"]), &adapter)
        .await.unwrap();
    assert!(!out.gl_posted);
    let mv_id: Uuid = sqlx::query_scalar("SELECT id FROM inventory.stock_moves WHERE company_id=$1 ORDER BY create_date DESC LIMIT 1")
        .bind(company).fetch_one(&pool).await.unwrap();
    assert_eq!(posting_state_of(&pool, mv_id).await, "not_applicable");
    let noop = w.repost_move_gl(company, mv_id, &out_gl(coa["5100"], coa["1300"]), &adapter).await.unwrap();
    assert!(!noop.posted);
    assert_eq!(journal_count(&pool, company).await, 0);
}
