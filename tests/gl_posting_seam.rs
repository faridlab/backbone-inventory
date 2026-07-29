//! The supply-chain GL seam: **inventory → accounting**. Proves the COGS and asset-receipt posts
//! land balanced in the REAL backbone-accounting ledger, via a serialized `AccountingPostEnvelope`
//! mapped by an in-test ACL adapter into accounting's `PostingService`. `backbone-accounting` is a
//! DEV-dependency only — the shipped inventory library has ZERO normal Cargo edge.
//! Both schemas (`inventory.*`, `accounting.*`) live in one DB. Requires DATABASE_URL (:5433/backbone_inventory).

use std::collections::HashMap;
use std::sync::Arc;

use rust_decimal::Decimal;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use backbone_inventory::application::service::inventory_gl::{
    AccountingPostEnvelope, GlPostAck, GlPostRejected, GlPostSink,
};
use backbone_inventory::application::service::inventory_write_service::{
    DeliveryLine, InventoryError, InventoryWriteService, NewDelivery, NewReceipt, NewReconciliation,
    NewWarehouse, ReceiptLine, ReconLine,
};
use backbone_accounting::application::service::posting_service::{
    PostingLine, PostingRequest, PostingService,
};
use backbone_accounting::infrastructure::persistence::SqlxPostingRepository;

struct AccountingAdapter { svc: PostingService }
#[async_trait::async_trait]
impl GlPostSink for AccountingAdapter {
    async fn post(&self, env: &AccountingPostEnvelope) -> Result<GlPostAck, GlPostRejected> {
        let mut req = PostingRequest::original(env.company_id, &env.source_type, env.source_id, env.posting_date);
        req.branch_id = env.branch_id;
        req.source_reference = env.source_reference.clone();
        req.currency = env.currency.clone();
        req.description = env.description.clone();
        if env.posting_type == "reversal" {
            // A reversal: hand accounting the original post id and NO lines — its PostingService reads
            // the original via reverses_post_id and mirrors the lines (posting_type='reversal' is a
            // distinct idempotency key from 'original', so it creates a new reversal journal).
            req.posting_type = "reversal".into();
            req.reverses_post_id = env.reverses_post_id;
            req.lines = Vec::new();
        } else {
            req.lines = env.lines.iter().map(|l| PostingLine {
                account_id: l.account_id, debit: l.debit, credit: l.credit,
                party_type: l.party_type.clone(), party_id: l.party_id,
                cost_center_id: None, project_id: None, department_id: None, description: l.description.clone(),
            }).collect();
        }
        match self.svc.post(req, None).await {
            Ok(r) => Ok(GlPostAck { post_id: r.post_id, journal_id: r.journal_id, idempotent_reuse: r.idempotent_reuse }),
            Err(e) => Err(GlPostRejected { code: e.code().to_string(), message: e.to_string() }),
        }
    }
}

// A sink that always rejects — simulates a transient accounting outage.
struct FailingGl;
#[async_trait::async_trait]
impl GlPostSink for FailingGl {
    async fn post(&self, _e: &AccountingPostEnvelope) -> Result<GlPostAck, GlPostRejected> {
        Err(GlPostRejected { code: "period_closed".into(), message: "transient".into() })
    }
}

// A sink that captures the envelope inventory emits — so a test can assert the reversal contract
// (posting_type, reverses_post_id, balanced, swapped Dr/Cr) without depending on accounting's
// reversal mapping (that ACL lives in backbone-accounting, not here).
#[derive(Clone)]
struct CapturedEnv { posting_type: String, reverses_post_id: Option<Uuid>, dr: Decimal, cr: Decimal }
#[derive(Default)]
struct CapturingGl { seen: std::sync::Mutex<Option<CapturedEnv>> }
#[async_trait::async_trait]
impl GlPostSink for CapturingGl {
    async fn post(&self, e: &AccountingPostEnvelope) -> Result<GlPostAck, GlPostRejected> {
        let dr: Decimal = e.lines.iter().map(|l| l.debit).sum();
        let cr: Decimal = e.lines.iter().map(|l| l.credit).sum();
        *self.seen.lock().unwrap() = Some(CapturedEnv {
            posting_type: e.posting_type.clone(), reverses_post_id: e.reverses_post_id, dr, cr,
        });
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

/// Seed the COA (asset Inventory, liability GR/IR clearing, COGS, expense adjustment + a header).
async fn seed_coa(pool: &PgPool) -> (Uuid, HashMap<&'static str, Uuid>) {
    let company = Uuid::new_v4();
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
    w.create_warehouse(NewWarehouse { company_id: company, code: uq("WH"), name: "Main".into(), warehouse_type: None, parent_warehouse_id: None, is_group: false }).await.unwrap()
}
async fn jrow(pool: &PgPool, jid: Uuid) -> (Decimal, Decimal) {
    let r = sqlx::query("SELECT total_debit, total_credit FROM accounting.journals WHERE id=$1").bind(jid).fetch_one(pool).await.unwrap();
    (r.get("total_debit"), r.get("total_credit"))
}
async fn line_amt(pool: &PgPool, jid: Uuid, acct: Uuid) -> (Decimal, Decimal) {
    let r = sqlx::query("SELECT debit_amount, credit_amount FROM accounting.journal_lines WHERE journal_id=$1 AND account_id=$2").bind(jid).bind(acct).fetch_one(pool).await.unwrap();
    (r.get("debit_amount"), r.get("credit_amount"))
}
async fn journal_count(pool: &PgPool, company: Uuid) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM accounting.journals WHERE company_id=$1").bind(company).fetch_one(pool).await.unwrap()
}

// ISEAM-1: goods receipt (10 @ 100 = 1,000) → Dr Inventory 1,000 · Cr GR/IR 1,000 in the real GL.
#[tokio::test]
async fn receipt_posts_asset_to_real_gl() {
    let pool = pool().await;
    let (company, coa) = seed_coa(&pool).await;
    let w = InventoryWriteService::new(pool.clone());
    let adapter = AccountingAdapter { svc: PostingService::new(Arc::new(SqlxPostingRepository::new(pool.clone()))) };
    let wh = warehouse(&w, company).await;
    let item = Uuid::new_v4();
    let rid = w.create_purchase_receipt(NewReceipt {
        receipt_number: uq("PR"), company_id: company, branch_id: None, supplier_id: Uuid::new_v4(),
        source_po_id: None, warehouse_id: wh, posting_date: day(),
        currency: "IDR".into(),
        inventory_account_id: coa["1300"], grir_account_id: coa["2150"],
        lines: vec![ReceiptLine { item_id: item, quantity: d("10"), rate: d("100") }],
    }).await.unwrap();
    let out = w.submit_purchase_receipt(rid, &adapter).await.unwrap();
    assert!(out.posted);
    let jid = out.journal_id.unwrap();
    assert_eq!(jrow(&pool, jid).await, (d("1000"), d("1000")));
    assert_eq!(line_amt(&pool, jid, coa["1300"]).await.0, d("1000"), "Dr Inventory");
    assert_eq!(line_amt(&pool, jid, coa["2150"]).await.1, d("1000"), "Cr GR/IR");
    let st: String = sqlx::query_scalar("SELECT posting_state::text FROM inventory.purchase_receipts WHERE id=$1").bind(rid).fetch_one(&pool).await.unwrap();
    assert_eq!(st, "posted");
}

// ISEAM-2: delivery (5 @ moving-avg 110 → COGS 550) → Dr COGS 550 · Cr Inventory 550.
#[tokio::test]
async fn delivery_posts_cogs_to_real_gl() {
    let pool = pool().await;
    let (company, coa) = seed_coa(&pool).await;
    let w = InventoryWriteService::new(pool.clone());
    let adapter = AccountingAdapter { svc: PostingService::new(Arc::new(SqlxPostingRepository::new(pool.clone()))) };
    let wh = warehouse(&w, company).await;
    let item = Uuid::new_v4();
    for (q, r) in [("10", "100"), ("10", "120")] {
        let rid = w.create_purchase_receipt(NewReceipt {
            receipt_number: uq("PR"), company_id: company, branch_id: None, supplier_id: Uuid::new_v4(),
            source_po_id: None, warehouse_id: wh, posting_date: day(),
            currency: "IDR".into(),
            inventory_account_id: coa["1300"], grir_account_id: coa["2150"],
            lines: vec![ReceiptLine { item_id: item, quantity: d(q), rate: d(r) }],
        }).await.unwrap();
        w.submit_purchase_receipt(rid, &adapter).await.unwrap();
    }
    let did = w.create_delivery_note(NewDelivery {
        delivery_number: uq("DN"), company_id: company, branch_id: None, customer_id: Uuid::new_v4(),
        source_so_id: None, warehouse_id: wh, posting_date: day(),
        currency: "IDR".into(),
        cogs_account_id: coa["5100"], inventory_account_id: coa["1300"],
        lines: vec![DeliveryLine { item_id: item, quantity: d("5") }],
    }).await.unwrap();
    let out = w.submit_delivery_note(did, &adapter).await.unwrap();
    let jid = out.journal_id.unwrap();
    assert_eq!(jrow(&pool, jid).await, (d("550"), d("550")));
    assert_eq!(line_amt(&pool, jid, coa["5100"]).await.0, d("550"), "Dr COGS");
    assert_eq!(line_amt(&pool, jid, coa["1300"]).await.1, d("550"), "Cr Inventory");
}

// ISEAM-3: reconciliation shrinkage (−200) → Dr Adjustment 200 · Cr Inventory 200.
#[tokio::test]
async fn reconciliation_posts_adjustment_to_real_gl() {
    let pool = pool().await;
    let (company, coa) = seed_coa(&pool).await;
    let w = InventoryWriteService::new(pool.clone());
    let adapter = AccountingAdapter { svc: PostingService::new(Arc::new(SqlxPostingRepository::new(pool.clone()))) };
    let wh = warehouse(&w, company).await;
    let item = Uuid::new_v4();
    let rid = w.create_purchase_receipt(NewReceipt {
        receipt_number: uq("PR"), company_id: company, branch_id: None, supplier_id: Uuid::new_v4(),
        source_po_id: None, warehouse_id: wh, posting_date: day(),
        currency: "IDR".into(),
        inventory_account_id: coa["1300"], grir_account_id: coa["2150"],
        lines: vec![ReceiptLine { item_id: item, quantity: d("10"), rate: d("100") }],
    }).await.unwrap();
    w.submit_purchase_receipt(rid, &adapter).await.unwrap();
    let sr = w.submit_reconciliation(NewReconciliation {
        recon_number: uq("SR"), company_id: company, warehouse_id: wh, posting_date: day(),
        currency: "IDR".into(),
        inventory_account_id: coa["1300"], adjustment_account_id: coa["5200"],
        lines: vec![ReconLine { item_id: item, counted_qty: d("8"), counted_rate: Decimal::ZERO }],
    }, &adapter).await.unwrap();
    let jid: Uuid = sqlx::query_scalar("SELECT journal_id FROM inventory.stock_reconciliations WHERE id=$1").bind(sr).fetch_one(&pool).await.unwrap();
    assert_eq!(jrow(&pool, jid).await, (d("200"), d("200")));
    assert_eq!(line_amt(&pool, jid, coa["5200"]).await.0, d("200"), "Dr Adjustment (shrinkage)");
    assert_eq!(line_amt(&pool, jid, coa["1300"]).await.1, d("200"), "Cr Inventory");
}

// ISEAM-4: GL rejection (Inventory pointed at a non-postable header) → posting_state=failed, but the
// physical movement (SLE + Bin) stands — eventually-consistent, retryable.
#[tokio::test]
async fn gl_rejection_leaves_movement_but_marks_failed() {
    let pool = pool().await;
    let (company, coa) = seed_coa(&pool).await;
    let w = InventoryWriteService::new(pool.clone());
    let adapter = AccountingAdapter { svc: PostingService::new(Arc::new(SqlxPostingRepository::new(pool.clone()))) };
    let wh = warehouse(&w, company).await;
    let item = Uuid::new_v4();
    let rid = w.create_purchase_receipt(NewReceipt {
        receipt_number: uq("PR"), company_id: company, branch_id: None, supplier_id: Uuid::new_v4(),
        source_po_id: None, warehouse_id: wh, posting_date: day(),
        currency: "IDR".into(),
        inventory_account_id: coa["1000"], grir_account_id: coa["2150"], // 1000 is a header → rejected
        lines: vec![ReceiptLine { item_id: item, quantity: d("10"), rate: d("100") }],
    }).await.unwrap();
    let err = w.submit_purchase_receipt(rid, &adapter).await.unwrap_err();
    assert_eq!(err.code(), "non_postable_account");
    let st: String = sqlx::query_scalar("SELECT posting_state::text FROM inventory.purchase_receipts WHERE id=$1").bind(rid).fetch_one(&pool).await.unwrap();
    assert_eq!(st, "failed");
    // The stock movement is real despite the GL rejection (retryable).
    let (qty,): (Decimal,) = sqlx::query_as("SELECT actual_qty FROM inventory.bins WHERE company_id=$1 AND item_id=$2 AND warehouse_id=$3").bind(company).bind(item).bind(wh).fetch_one(&pool).await.unwrap();
    assert_eq!(qty, d("10.0000"), "SLE+Bin committed; only the GL post failed");
}

// ISEAM-5: re-submitting a submitted receipt is refused (not_draft) — the movement posts once.
#[tokio::test]
async fn resubmit_is_refused() {
    let pool = pool().await;
    let (company, coa) = seed_coa(&pool).await;
    let w = InventoryWriteService::new(pool.clone());
    let adapter = AccountingAdapter { svc: PostingService::new(Arc::new(SqlxPostingRepository::new(pool.clone()))) };
    let wh = warehouse(&w, company).await;
    let rid = w.create_purchase_receipt(NewReceipt {
        receipt_number: uq("PR"), company_id: company, branch_id: None, supplier_id: Uuid::new_v4(),
        source_po_id: None, warehouse_id: wh, posting_date: day(),
        currency: "IDR".into(),
        inventory_account_id: coa["1300"], grir_account_id: coa["2150"],
        lines: vec![ReceiptLine { item_id: Uuid::new_v4(), quantity: d("1"), rate: d("100") }],
    }).await.unwrap();
    w.submit_purchase_receipt(rid, &adapter).await.unwrap();
    let err = w.submit_purchase_receipt(rid, &adapter).await.unwrap_err();
    assert!(matches!(err, InventoryError::NotDraft(_)));
}

// ISEAM-6: a failed post (transient outage) is not terminal — repost re-drives it to posted, and
// the physical movement was already committed. Closes the stuck-failed-voucher blocker (council).
#[tokio::test]
async fn repost_recovers_a_failed_post() {
    let pool = pool().await;
    let (company, coa) = seed_coa(&pool).await;
    let w = InventoryWriteService::new(pool.clone());
    let good = AccountingAdapter { svc: PostingService::new(Arc::new(SqlxPostingRepository::new(pool.clone()))) };
    let wh = warehouse(&w, company).await;
    let item = Uuid::new_v4();
    let rid = w.create_purchase_receipt(NewReceipt {
        receipt_number: uq("PR"), company_id: company, branch_id: None, supplier_id: Uuid::new_v4(),
        source_po_id: None, warehouse_id: wh, posting_date: day(),
        currency: "IDR".into(),
        inventory_account_id: coa["1300"], grir_account_id: coa["2150"],
        lines: vec![ReceiptLine { item_id: item, quantity: d("10"), rate: d("100") }],
    }).await.unwrap();
    // Transient failure → movement committed, post failed.
    let err = w.submit_purchase_receipt(rid, &FailingGl).await.unwrap_err();
    assert_eq!(err.code(), "period_closed");
    let st: String = sqlx::query_scalar("SELECT posting_state::text FROM inventory.purchase_receipts WHERE id=$1").bind(rid).fetch_one(&pool).await.unwrap();
    assert_eq!(st, "failed");
    let (qty,): (Decimal,) = sqlx::query_as("SELECT actual_qty FROM inventory.bins WHERE company_id=$1 AND item_id=$2 AND warehouse_id=$3").bind(company).bind(item).bind(wh).fetch_one(&pool).await.unwrap();
    assert_eq!(qty, d("10.0000"), "movement committed despite failed post");
    // Repost → posted, exactly one journal.
    let out = w.repost_purchase_receipt(rid, &good).await.unwrap();
    assert!(out.posted);
    assert_eq!(jrow(&pool, out.journal_id.unwrap()).await, (d("1000"), d("1000")));
    let st2: String = sqlx::query_scalar("SELECT posting_state::text FROM inventory.purchase_receipts WHERE id=$1").bind(rid).fetch_one(&pool).await.unwrap();
    assert_eq!(st2, "posted");
}

// ISEAM-7: repost is idempotent against the crash window (physically posted, status not updated).
// Force posting_state back to 'failed' on an already-posted voucher, then repost → accounting
// dedupes on source_id, returns the SAME journal — no second journal.
#[tokio::test]
async fn repost_does_not_double_post() {
    let pool = pool().await;
    let (company, coa) = seed_coa(&pool).await;
    let w = InventoryWriteService::new(pool.clone());
    let adapter = AccountingAdapter { svc: PostingService::new(Arc::new(SqlxPostingRepository::new(pool.clone()))) };
    let wh = warehouse(&w, company).await;
    let rid = w.create_purchase_receipt(NewReceipt {
        receipt_number: uq("PR"), company_id: company, branch_id: None, supplier_id: Uuid::new_v4(),
        source_po_id: None, warehouse_id: wh, posting_date: day(),
        currency: "IDR".into(),
        inventory_account_id: coa["1300"], grir_account_id: coa["2150"],
        lines: vec![ReceiptLine { item_id: Uuid::new_v4(), quantity: d("2"), rate: d("100") }],
    }).await.unwrap();
    let first = w.submit_purchase_receipt(rid, &adapter).await.unwrap();
    // Simulate the crash window: accounting posted, but selling-side status update was lost.
    sqlx::query("UPDATE inventory.purchase_receipts SET posting_state='failed'::gl_posting_state WHERE id=$1").bind(rid).execute(&pool).await.unwrap();
    // Repost → dedup returns the same journal; still exactly one journal for the company.
    let again = w.repost_purchase_receipt(rid, &adapter).await.unwrap();
    assert_eq!(again.journal_id, first.journal_id, "same journal (dedup on source_id)");
    assert_eq!(journal_count(&pool, company).await, 1, "no double post");
    // And an already-posted voucher short-circuits (no re-emit).
    let noop = w.repost_purchase_receipt(rid, &adapter).await.unwrap();
    assert_eq!(noop.journal_id, first.journal_id);
    assert_eq!(journal_count(&pool, company).await, 1);
}

// ISEAM-8 (council 2026-07-29): cancelling a posted receipt emits a balanced REVERSAL envelope —
// posting_type='reversal', reverses_post_id = the original post, Dr/Cr swapped vs the original
// (Dr Inventory·Cr GR/IR → Dr GR/IR·Cr Inventory) — and records the reversal post id on the header.
#[tokio::test]
async fn cancel_emits_balanced_reversal_envelope() {
    let pool = pool().await;
    let (company, coa) = seed_coa(&pool).await;
    let w = InventoryWriteService::new(pool.clone());
    let adapter = AccountingAdapter { svc: PostingService::new(Arc::new(SqlxPostingRepository::new(pool.clone()))) };
    let wh = warehouse(&w, company).await;
    let item = Uuid::new_v4();
    let rid = w.create_purchase_receipt(NewReceipt {
        receipt_number: uq("PR"), company_id: company, branch_id: None, supplier_id: Uuid::new_v4(),
        source_po_id: None, warehouse_id: wh, posting_date: day(),
        currency: "IDR".into(),
        inventory_account_id: coa["1300"], grir_account_id: coa["2150"],
        lines: vec![ReceiptLine { item_id: item, quantity: d("10"), rate: d("100") }],
    }).await.unwrap();
    let orig = w.submit_purchase_receipt(rid, &adapter).await.unwrap();
    let orig_post = orig.post_id.unwrap();
    // Cancel through a capturing sink so we can assert the reversal envelope inventory emits.
    let cap = CapturingGl::default();
    let out = w.cancel_purchase_receipt(rid, &cap).await.unwrap();
    assert!(out.posted);
    let env = cap.seen.lock().unwrap().clone().expect("reversal envelope emitted");
    assert_eq!(env.posting_type, "reversal");
    assert_eq!(env.reverses_post_id, Some(orig_post), "reversal references the original post");
    assert_eq!(env.dr, d("1000"));
    assert_eq!(env.cr, d("1000"), "balanced");
    let st: String = sqlx::query_scalar("SELECT status::text FROM inventory.purchase_receipts WHERE id=$1")
        .bind(rid).fetch_one(&pool).await.unwrap();
    assert_eq!(st, "cancelled");
    let rev: Option<Uuid> = sqlx::query_scalar("SELECT reversal_accounting_post_id FROM inventory.purchase_receipts WHERE id=$1")
        .bind(rid).fetch_one(&pool).await.unwrap();
    assert!(rev.is_some(), "reversal post id recorded on the header");
}

// ISEAM-9 (council 2026-07-29): end-to-end reversal through the REAL accounting ledger. Cancelling a
// posted receipt lands a SECOND, balanced journal that MIRRORS the original (Dr GR/IR · Cr Inventory),
// proving the inventory→accounting reversal seam works, not just inventory's envelope shape.
#[tokio::test]
async fn cancel_posts_a_real_reversal_journal() {
    let pool = pool().await;
    let (company, coa) = seed_coa(&pool).await;
    let w = InventoryWriteService::new(pool.clone());
    let adapter = AccountingAdapter { svc: PostingService::new(Arc::new(SqlxPostingRepository::new(pool.clone()))) };
    let wh = warehouse(&w, company).await;
    let item = Uuid::new_v4();
    let rid = w.create_purchase_receipt(NewReceipt {
        receipt_number: uq("PR"), company_id: company, branch_id: None, supplier_id: Uuid::new_v4(),
        source_po_id: None, warehouse_id: wh, posting_date: day(),
        currency: "IDR".into(),
        inventory_account_id: coa["1300"], grir_account_id: coa["2150"],
        lines: vec![ReceiptLine { item_id: item, quantity: d("10"), rate: d("100") }],
    }).await.unwrap();
    let orig = w.submit_purchase_receipt(rid, &adapter).await.unwrap();
    let orig_jid = orig.journal_id.unwrap();
    assert_eq!(journal_count(&pool, company).await, 1);

    // Cancel through the REAL adapter → accounting posts a distinct reversal journal.
    let rev = w.cancel_purchase_receipt(rid, &adapter).await.unwrap();
    assert!(rev.posted);
    let rev_jid = rev.journal_id.unwrap();
    assert_ne!(rev_jid, orig_jid, "reversal is a distinct journal");
    assert_eq!(journal_count(&pool, company).await, 2, "original + reversal");

    // The reversal mirrors the original (Dr Inventory·Cr GR/IR → Dr GR/IR · Cr Inventory).
    assert_eq!(jrow(&pool, rev_jid).await, (d("1000"), d("1000")), "balanced reversal");
    assert_eq!(line_amt(&pool, rev_jid, coa["2150"]).await.0, d("1000"), "Dr GR/IR (swapped)");
    assert_eq!(line_amt(&pool, rev_jid, coa["1300"]).await.1, d("1000"), "Cr Inventory (swapped)");

    // And the original journal is marked reversed by accounting.
    let orig_reversed: bool = sqlx::query_scalar("SELECT is_reversed FROM accounting.journals WHERE id=$1")
        .bind(orig_jid).fetch_one(&pool).await.unwrap();
    assert!(orig_reversed, "accounting marked the original journal reversed");
}
