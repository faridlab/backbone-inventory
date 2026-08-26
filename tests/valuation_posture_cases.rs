//! Posting-posture probes for the valuation overlay: the per-company settings row
//! (`inventory.inventory_company_settings`: cost-method vocabulary, perpetual/periodic axis,
//! anglo-saxon delivery-debit posture) switching WHICH account the EXISTING door posts use —
//! never a second posting path.
//!
//! Coverage (P1–P8):
//! - P1/P6 the rollout no-op: an ABSENT settings row (and posture OFF) posts exactly today's
//!   shapes (receipt Dr Inventory · Cr GR/IR; delivery Dr COGS · Cr Inventory).
//! - P2 the anglo-saxon scope probe: posture ON swaps the delivery DEBIT to the
//!   interim-delivered account (Dr interim · Cr Inventory); the receipt side is unchanged.
//! - P3 fail-closed: posture ON with no interim account configured refuses the delivery
//!   (`anglo_posture_unconfigured`) — nothing minted, nothing posted.
//! - P4 posture symmetry: the SAME resolution across submit, repost, and the cancel
//!   compensation.
//! - P5 `periodic` suppresses real-time posts: doors retire to `not_applicable`, and the move
//!   engine's own GL legs stay `not_applicable` too.
//! - P7 the location valuation-account override beats the door-header account.
//! - P8 the explicit account-move gate: a voucher that carries neither value nor quantity
//!   builds no envelope and retires to `not_applicable`.
//!
//! Requires DATABASE_URL (:5433/backbone_inventory), inventory schema applied (the accounting
//! schema is NOT needed — envelopes are captured by a recording sink, not posted to a ledger).

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
    DeliveryLine, InventoryError, InventoryWriteService, NewDelivery, NewReceipt, NewWarehouse,
    ReceiptLine,
};

// --- the recording sink -------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct RecLine {
    posting_type: String,
    account_id: Uuid,
    debit: Decimal,
    credit: Decimal,
}

/// Captures every envelope line the service emits, so tests assert the posting SHAPE (which
/// account each leg uses) without depending on a real ledger.
#[derive(Default)]
struct Recorder {
    lines: std::sync::Mutex<Vec<RecLine>>,
}
#[async_trait::async_trait]
impl GlPostSink for Recorder {
    async fn post(&self, e: &AccountingPostEnvelope) -> Result<GlPostAck, GlPostRejected> {
        let mut seen = self.lines.lock().unwrap();
        for l in &e.lines {
            seen.push(RecLine {
                posting_type: e.posting_type.clone(),
                account_id: l.account_id,
                debit: l.debit,
                credit: l.credit,
            });
        }
        Ok(GlPostAck { post_id: Uuid::new_v4(), journal_id: Uuid::new_v4(), idempotent_reuse: false })
    }
}
impl Recorder {
    fn debits(&self, acct: Uuid) -> Vec<RecLine> {
        self.lines.lock().unwrap().iter()
            .filter(|l| l.account_id == acct && l.debit > Decimal::ZERO)
            .cloned().collect()
    }
    fn credits(&self, acct: Uuid) -> Vec<RecLine> {
        self.lines.lock().unwrap().iter()
            .filter(|l| l.account_id == acct && l.credit > Decimal::ZERO)
            .cloned().collect()
    }
    fn count(&self) -> usize {
        self.lines.lock().unwrap().len()
    }
}

// --- scaffolding ---------------------------------------------------------------------------------

fn d(s: &str) -> Decimal { Decimal::from_str_exact(s).unwrap() }
fn day() -> chrono::NaiveDate { chrono::NaiveDate::from_ymd_opt(2026, 8, 26).unwrap() }
fn uq(p: &str) -> String { format!("{p}-{}", &Uuid::new_v4().simple().to_string()[..8]) }
async fn pool() -> PgPool {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:postgres@localhost:5433/backbone_inventory".to_string());
    PgPool::connect(&url).await.expect("connect DB")
}

/// One company's posting posture. `None` interim + `anglo=true` is the fail-closed case.
async fn set_posture(
    pool: &PgPool,
    company: Uuid,
    policy: &str,
    anglo: bool,
    interim: Option<Uuid>,
) {
    sqlx::query(
        r#"INSERT INTO inventory.inventory_company_settings
             (id, company_id, cost_method, valuation_policy, anglo_saxon_accounting,
              stock_interim_delivered_account_id)
           VALUES ($1,$2,'average',$3::valuation_policy,$4,$5)"#,
    )
    .bind(Uuid::new_v4()).bind(company).bind(policy).bind(anglo).bind(interim)
    .execute(pool).await.expect("seed posture");
}

async fn warehouse(w: &InventoryWriteService, company: Uuid) -> Uuid {
    w.create_warehouse(NewWarehouse {
        company_id: company, code: uq("WH"), name: uq("Main"),
        warehouse_type: None, parent_warehouse_id: None, is_group: false,
    }).await.unwrap()
}

/// Five placeholder account ids — the recorder asserts WHICH id each leg used, never a ledger.
#[derive(Clone, Copy)]
struct Accts {
    inv: Uuid,
    grir: Uuid,
    cogs: Uuid,
    interim: Uuid,
    override_acct: Uuid,
}
fn accts() -> Accts {
    Accts {
        inv: Uuid::new_v4(),
        grir: Uuid::new_v4(),
        cogs: Uuid::new_v4(),
        interim: Uuid::new_v4(),
        override_acct: Uuid::new_v4(),
    }
}

async fn receive(w: &InventoryWriteService, company: Uuid, wh: Uuid, a: &Accts, item: Uuid, qty: &str, rate: &str, sink: &dyn GlPostSink) -> Uuid {
    let rid = w.create_purchase_receipt(NewReceipt {
        receipt_number: uq("PR"), company_id: company, branch_id: None, supplier_id: Uuid::new_v4(),
        source_po_id: None, warehouse_id: wh, posting_date: day(),
        currency: "IDR".into(),
        inventory_account_id: a.inv, grir_account_id: a.grir,
        lines: vec![ReceiptLine { item_id: item, quantity: d(qty), rate: d(rate) }],
    }).await.unwrap();
    w.submit_purchase_receipt(rid, sink).await.unwrap();
    rid
}

async fn deliver(w: &InventoryWriteService, company: Uuid, wh: Uuid, a: &Accts, item: Uuid, qty: &str, sink: &dyn GlPostSink) -> Result<backbone_inventory::application::service::inventory_write_service::SubmitOutcome, InventoryError> {
    let did = w.create_delivery_note(NewDelivery {
        delivery_number: uq("DN"), company_id: company, branch_id: None, customer_id: Uuid::new_v4(),
        source_so_id: None, warehouse_id: wh, posting_date: day(),
        currency: "IDR".into(),
        cogs_account_id: a.cogs, inventory_account_id: a.inv,
        lines: vec![DeliveryLine { item_id: item, quantity: d(qty) }],
    }).await.unwrap();
    w.submit_delivery_note(did, sink).await
}

async fn voucher_state(pool: &PgPool, table: &str, id: Uuid) -> (String, String) {
    let sql = format!("SELECT status::text AS st, posting_state::text AS ps FROM inventory.{table} WHERE id=$1");
    let r = sqlx::query(&sql).bind(id).fetch_one(pool).await.unwrap();
    (r.get("st"), r.get("ps"))
}

// --- P1 + P6: posture OFF / absent row = exactly today's shapes (the rollout no-op) --------------

#[tokio::test]
async fn absent_settings_row_posts_todays_shapes() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    // NOTE: no inventory_company_settings row — the runtime defaults must equal today.
    let w = InventoryWriteService::new(pool.clone());
    let rec = Recorder::default();
    let wh = warehouse(&w, company).await;
    let a = accts();
    let item = Uuid::new_v4();
    receive(&w, company, wh, &a, item, "10", "100", &rec).await;
    assert_eq!(rec.debits(a.inv).len(), 1, "P6: Dr Inventory (header account, no override)");
    assert_eq!(rec.debits(a.inv)[0].debit, d("1000"));
    assert_eq!(rec.credits(a.grir).len(), 1, "P6: Cr GR/IR clearing");
    let out = deliver(&w, company, wh, &a, item, "4", &rec).await.unwrap();
    assert!(out.posted, "P1: posture OFF still posts");
    assert_eq!(rec.debits(a.cogs).len(), 1, "P1: Dr COGS unchanged");
    assert_eq!(rec.debits(a.cogs)[0].debit, d("400"), "4 @ moving-average 100");
    assert_eq!(rec.credits(a.inv).len(), 1, "P1: Cr Inventory unchanged");
    assert_eq!(rec.credits(a.interim).len(), 0, "no interim leg anywhere");
}

// --- P2: the anglo-saxon scope probe — the delivery DEBIT swaps to the interim-delivered account --

#[tokio::test]
async fn anglo_posture_swaps_delivery_debit_to_interim() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let w = InventoryWriteService::new(pool.clone());
    let rec = Recorder::default();
    let wh = warehouse(&w, company).await;
    let a = accts();
    let item = Uuid::new_v4();
    set_posture(&pool, company, "perpetual", true, Some(a.interim)).await;
    receive(&w, company, wh, &a, item, "10", "100", &rec).await;
    // A1: the receipt side is unchanged in BOTH postures — grir IS the interim-received leg.
    assert_eq!(rec.debits(a.inv).len(), 1, "receipt still Dr Inventory");
    assert_eq!(rec.credits(a.grir).len(), 1, "receipt still Cr GR/IR");
    let out = deliver(&w, company, wh, &a, item, "4", &rec).await.unwrap();
    assert!(out.posted);
    assert_eq!(rec.debits(a.interim).len(), 1, "P2: Dr interim-delivered (the swap)");
    assert_eq!(rec.debits(a.interim)[0].debit, d("400"));
    assert_eq!(rec.debits(a.cogs).len(), 0, "P2: COGS recognition deferred — no COGS debit");
    assert_eq!(rec.credits(a.inv).len(), 1, "P2: Cr Inventory unchanged");
    // The envelope is one balanced post, not a second path.
    let all = rec.lines.lock().unwrap().clone();
    assert_eq!(all.iter().filter(|l| l.posting_type == "original").count(), 4,
        "exactly two envelopes' legs (receipt + delivery), nothing more");
}

// --- P3: fail-closed — posture ON with no interim account configured -----------------------------

#[tokio::test]
async fn anglo_posture_without_interim_account_fails_closed() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let w = InventoryWriteService::new(pool.clone());
    let rec = Recorder::default();
    let wh = warehouse(&w, company).await;
    let a = accts();
    let item = Uuid::new_v4();
    set_posture(&pool, company, "perpetual", true, None).await;
    receive(&w, company, wh, &a, item, "10", "100", &rec).await; // receipt side needs no interim account
    let did = w.create_delivery_note(NewDelivery {
        delivery_number: uq("DN"), company_id: company, branch_id: None, customer_id: Uuid::new_v4(),
        source_so_id: None, warehouse_id: wh, posting_date: day(),
        currency: "IDR".into(),
        cogs_account_id: a.cogs, inventory_account_id: a.inv,
        lines: vec![DeliveryLine { item_id: item, quantity: d("4") }],
    }).await.unwrap();
    let err = w.submit_delivery_note(did, &rec).await.unwrap_err();
    assert_eq!(err.code(), "anglo_posture_unconfigured", "P3: loud fail-closed");
    assert!(matches!(err, InventoryError::AngloPostureUnconfigured { .. }));
    assert_eq!(rec.debits(a.cogs).len(), 0, "P3: no silent COGS fallback");
    assert_eq!(rec.count(), 2, "P3: only the receipt's legs — the delivery posted nothing");
    let (status, _) = voucher_state(&pool, "delivery_notes", did).await;
    assert_eq!(status, "draft", "P3: the voucher never left draft");
    let moves: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM inventory.stock_moves WHERE company_id=$1 AND origin LIKE 'DN-%'")
        .bind(company).fetch_one(&pool).await.unwrap();
    assert_eq!(moves, 0, "P3: nothing minted — refused before any movement");
}

// --- P4: posture symmetry across submit / repost / cancel compensation ---------------------------

#[tokio::test]
async fn posture_is_identical_across_submit_repost_and_cancel() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let w = InventoryWriteService::new(pool.clone());
    let rec = Recorder::default();
    let wh = warehouse(&w, company).await;
    let a = accts();
    let item = Uuid::new_v4();
    set_posture(&pool, company, "perpetual", true, Some(a.interim)).await;
    receive(&w, company, wh, &a, item, "10", "100", &rec).await;
    let did = w.create_delivery_note(NewDelivery {
        delivery_number: uq("DN"), company_id: company, branch_id: None, customer_id: Uuid::new_v4(),
        source_so_id: None, warehouse_id: wh, posting_date: day(),
        currency: "IDR".into(),
        cogs_account_id: a.cogs, inventory_account_id: a.inv,
        lines: vec![DeliveryLine { item_id: item, quantity: d("4") }],
    }).await.unwrap();
    let orig = w.submit_delivery_note(did, &rec).await.unwrap();
    assert!(orig.posted);
    assert_eq!(rec.debits(a.interim)[0].posting_type, "original", "submit debited the interim leg");

    // Repost: simulate the crash window (posted physically, status lost) — the rebuilt
    // envelope must carry the SAME interim debit.
    sqlx::query("UPDATE inventory.delivery_notes SET posting_state='failed'::gl_posting_state WHERE id=$1")
        .bind(did).execute(&pool).await.unwrap();
    let rec2 = Recorder::default();
    let again = w.repost_delivery_note(did, &rec2).await.unwrap();
    assert!(again.posted);
    assert_eq!(rec2.debits(a.interim).len(), 1, "P4: repost debits the SAME interim account");
    assert_eq!(rec2.debits(a.cogs).len(), 0);
    let _ = orig; // the original outcome settled; the repost carries its own ack

    // Cancel compensation: the credit leg mirrors the original debit (interim), the debit
    // restores inventory.
    let rec3 = Recorder::default();
    let rev = w.cancel_delivery_note(did, &rec3).await.unwrap();
    assert!(rev.posted);
    let reversal: Vec<RecLine> = rec3.lines.lock().unwrap().iter()
        .filter(|l| l.posting_type == "reversal").cloned().collect();
    assert_eq!(reversal.len(), 2, "P4: one balanced reversal envelope");
    assert_eq!(reversal.iter().filter(|l| l.credit > Decimal::ZERO && l.account_id == a.interim).count(), 1,
        "P4: the compensation CREDITS the interim leg (mirrors the original debit)");
    assert_eq!(reversal.iter().filter(|l| l.credit > Decimal::ZERO && l.account_id == a.cogs).count(), 0,
        "P4: no COGS credit — the original never debited COGS");
    assert_eq!(reversal.iter().filter(|l| l.debit > Decimal::ZERO && l.account_id == a.inv).count(), 1,
        "P4: Dr Inventory restores the estate");
}

// --- P5: periodic suppresses real-time posts (doors AND engine legs) -----------------------------

#[tokio::test]
async fn periodic_policy_suppresses_realtime_posts() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let w = InventoryWriteService::new(pool.clone());
    let rec = Recorder::default();
    let wh = warehouse(&w, company).await;
    let a = accts();
    let item = Uuid::new_v4();
    set_posture(&pool, company, "periodic", false, None).await;
    receive(&w, company, wh, &a, item, "10", "100", &rec).await; // submit must succeed (physical movement)
    assert_eq!(rec.count(), 0, "P5: periodic posts nothing at receipt time");
    let (rstatus, rps) = voucher_state(&pool, "purchase_receipts",
        sqlx::query_scalar::<_, Uuid>("SELECT id FROM inventory.purchase_receipts WHERE company_id=$1")
            .bind(company).fetch_one(&pool).await.unwrap()).await;
    assert_eq!(rstatus, "submitted", "P5: the physical movement still happened");
    assert_eq!(rps, "not_applicable", "P5: the voucher retired to not_applicable");

    let did = w.create_delivery_note(NewDelivery {
        delivery_number: uq("DN"), company_id: company, branch_id: None, customer_id: Uuid::new_v4(),
        source_so_id: None, warehouse_id: wh, posting_date: day(),
        currency: "IDR".into(),
        cogs_account_id: a.cogs, inventory_account_id: a.inv,
        lines: vec![DeliveryLine { item_id: item, quantity: d("4") }],
    }).await.unwrap();
    let out = w.submit_delivery_note(did, &rec).await.unwrap();
    assert!(!out.posted, "P5: periodic delivery posts nothing");
    let (_, dps) = voucher_state(&pool, "delivery_notes", did).await;
    assert_eq!(dps, "not_applicable");
    // The stock really moved — only the GL legs are suppressed.
    let (qty,): (Decimal,) = sqlx::query_as(
        "SELECT actual_qty FROM inventory.bins WHERE company_id=$1 AND item_id=$2 AND warehouse_id=$3",
    ).bind(company).bind(item).bind(wh).fetch_one(&pool).await.unwrap();
    assert_eq!(qty, d("6"), "P5: the estate moved 10 → 6 despite no GL post");

    // Engine legs too: a directive-carrying move under a periodic company stays not_applicable.
    let stock: Uuid = sqlx::query_scalar("SELECT id FROM inventory.locations WHERE company_id=$1 AND warehouse_id=$2 AND usage='internal'")
        .bind(company).bind(wh).fetch_one(&pool).await.unwrap();
    let customer: Uuid = sqlx::query_scalar("SELECT id FROM inventory.locations WHERE company_id=$1 AND usage='customer'")
        .bind(company).fetch_one(&pool).await.unwrap();
    let gl = MoveGlDirective {
        cogs_account_id: Some(a.cogs), inventory_account_id: Some(a.inv),
        grir_account_id: None, adjustment_account_id: None, currency: "IDR".into(),
    };
    let mid = w.create_move(NewStockMove {
        name: uq("MV"), company_id: company, item_id: item, demand_qty: d("2"),
        price_unit: Decimal::ZERO, procure_method: "make_to_stock".into(), picking_id: None,
        origin: None, location_id: stock, location_dest_id: customer, partner_id: None,
        warehouse_id: Some(wh), orderpoint_id: None, move_orig_ids: vec![], move_dest_ids: vec![],
        is_inventory: false, scrapped: false, forced_value: None,
    }).await.unwrap();
    w.action_confirm(mid).await.unwrap();
    w.action_assign(mid).await.unwrap();
    w.action_done(mid, BackorderPolicy::Never, &gl, &rec).await.unwrap();
    let mps: String = sqlx::query_scalar("SELECT posting_state::text FROM inventory.stock_moves WHERE id=$1")
        .bind(mid).fetch_one(&pool).await.unwrap();
    assert_eq!(mps, "not_applicable", "P5: the engine leg never armed under periodic");
    assert_eq!(rec.count(), 0, "P5: no envelope anywhere");
}

// --- P7: the location valuation-account override beats the door-header account --------------------

#[tokio::test]
async fn location_valuation_override_wins_over_header_account() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let w = InventoryWriteService::new(pool.clone());
    let rec = Recorder::default();
    let wh = warehouse(&w, company).await;
    let a = accts();
    let item = Uuid::new_v4();
    // First receipt bootstraps the warehouse's Stock location; then the override lands on it.
    receive(&w, company, wh, &a, item, "10", "100", &rec).await;
    let stock: Uuid = sqlx::query_scalar(
        "SELECT id FROM inventory.locations WHERE company_id=$1 AND warehouse_id=$2 AND usage='internal'",
    ).bind(company).bind(wh).fetch_one(&pool).await.unwrap();
    sqlx::query("UPDATE inventory.locations SET valuation_account_id=$2 WHERE id=$1")
        .bind(stock).bind(a.override_acct).execute(&pool).await.unwrap();

    // Receipt: the Inventory DEBIT leg resolves the override (chain: location → header).
    receive(&w, company, wh, &a, item, "10", "120", &rec).await;
    assert_eq!(rec.debits(a.override_acct).len(), 1, "P7: receipt Dr the location's valuation account");
    assert_eq!(rec.debits(a.override_acct)[0].debit, d("1200"));
    assert_eq!(rec.credits(a.grir).len(), 2, "GR/IR legs unchanged (one per receipt)");
    // Delivery: the Inventory CREDIT leg resolves the same override.
    deliver(&w, company, wh, &a, item, "4", &rec).await.unwrap();
    assert_eq!(rec.credits(a.override_acct).len(), 1, "P7: delivery Cr the location's valuation account");
    assert_eq!(rec.credits(a.override_acct)[0].credit, d("440"), "4 @ blended 110");
    assert_eq!(rec.credits(a.inv).len(), 0, "P7: the header account lost to the override");
}

// --- P8: the explicit account-move gate — zero value AND zero qty posts nothing -------------------

#[tokio::test]
async fn zero_value_zero_qty_voucher_posts_nothing() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let w = InventoryWriteService::new(pool.clone());
    let rec = Recorder::default();
    let wh = warehouse(&w, company).await;
    let a = accts();
    let item = Uuid::new_v4();
    let rid = w.create_purchase_receipt(NewReceipt {
        receipt_number: uq("PR"), company_id: company, branch_id: None, supplier_id: Uuid::new_v4(),
        source_po_id: None, warehouse_id: wh, posting_date: day(),
        currency: "IDR".into(),
        inventory_account_id: a.inv, grir_account_id: a.grir,
        lines: vec![ReceiptLine { item_id: item, quantity: Decimal::ZERO, rate: d("100") }],
    }).await.unwrap();
    let out = w.submit_purchase_receipt(rid, &rec).await.unwrap();
    assert!(!out.posted, "P8: an all-zero receipt carries nothing to post");
    let (_, ps) = voucher_state(&pool, "purchase_receipts", rid).await;
    assert_eq!(ps, "not_applicable", "P8: retired, not failed");

    let did = w.create_delivery_note(NewDelivery {
        delivery_number: uq("DN"), company_id: company, branch_id: None, customer_id: Uuid::new_v4(),
        source_so_id: None, warehouse_id: wh, posting_date: day(),
        currency: "IDR".into(),
        cogs_account_id: a.cogs, inventory_account_id: a.inv,
        lines: vec![DeliveryLine { item_id: item, quantity: Decimal::ZERO }],
    }).await.unwrap();
    let dout = w.submit_delivery_note(did, &rec).await.unwrap();
    assert!(!dout.posted, "P8: an all-zero delivery posts nothing");
    let (_, dps) = voucher_state(&pool, "delivery_notes", did).await;
    assert_eq!(dps, "not_applicable");
    assert_eq!(rec.count(), 0, "P8: no envelope was ever built");
}
