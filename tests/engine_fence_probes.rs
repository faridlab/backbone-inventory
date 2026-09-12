//! Stock-move engine probes on the module-native schema, plus the tenancy posture probe.
//!
//! The behavioral probes run against the scratch database (export `DATABASE_URL`; default
//! :5433/backbone_inventory) as the migration owner — the owner pool bypasses row-level
//! security, so what they exercise is the engine itself: the full lifecycle
//! `create_move → action_confirm → action_assign → action_done`, one `action_cancel` path, one
//! `recompute_orderpoint` pass, the receipt door's location valuation-override resolution, the
//! `unreserve_move` release arm, a `repost_move_gl` re-drive against the real backbone-accounting
//! ledger, and the procurement provisioning CRUD (`create_route` / `create_rule` /
//! `create_orderpoint`).
//!
//! The tenancy posture probe pins the module-side half of the fence (ADR-0029): the module
//! ships NO tenant column and NO row-level-security policy of its own. What it ships instead
//! is the half-fence the composing service's tenancy decorator completes: every inventory base
//! table carries ENABLE + FORCE ROW LEVEL SECURITY with zero policies. A plain non-superuser,
//! NOBYPASSRLS role is therefore default-DENIED — zero rows, writes refused — no matter what
//! legacy variable is set, while the owner pool still sees the rows it seeded (the denial is
//! the absent policy set, not an empty database).
//!
//! The GL re-drive probe needs the accounting schema's up-migrations on the same database (it
//! posts through the REAL backbone-accounting PostingService — the same ACL the composing
//! service ships).

use rust_decimal::Decimal;
use sqlx::{Connection, PgPool, Row};
use std::sync::Arc;
use uuid::Uuid;

use backbone_accounting::application::service::posting_service::{
    PostingLine, PostingRequest, PostingService,
};
use backbone_accounting::infrastructure::persistence::SqlxPostingRepository;
use backbone_inventory::application::service::inventory_gl::{
    AccountingPostEnvelope, GlPostAck, GlPostRejected, GlPostSink,
};
use backbone_inventory::application::service::inventory_move_engine::{
    BackorderPolicy, MoveGlDirective, NewStockMove,
};
use backbone_inventory::application::service::inventory_write_service::{
    InventoryWriteService, NewReceipt, NewWarehouse, ReceiptLine,
};
use backbone_inventory::application::service::procurement_service::{
    NewOrderpoint, NewRoute, NewRouteRule, ProcurementService,
};
use backbone_inventory::infrastructure::persistence::ProcurementRepository;

fn d(s: &str) -> Decimal {
    Decimal::from_str_exact(s).unwrap()
}

fn uq(p: &str) -> String {
    format!("{p}-{}", &Uuid::new_v4().simple().to_string()[..8])
}

async fn pool() -> PgPool {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:postgres@localhost:5433/backbone_inventory".to_string());
    PgPool::connect(&url).await.expect("connect DB")
}

/// A GL sink that acknowledges every post and records the envelopes it saw.
#[derive(Default)]
struct RecordingSink {
    posts: std::sync::Mutex<Vec<AccountingPostEnvelope>>,
}

#[async_trait::async_trait]
impl GlPostSink for RecordingSink {
    async fn post(&self, e: &AccountingPostEnvelope) -> Result<GlPostAck, GlPostRejected> {
        self.posts.lock().unwrap().push(e.clone());
        Ok(GlPostAck { post_id: Uuid::new_v4(), journal_id: Uuid::new_v4(), idempotent_reuse: false })
    }
}

/// A sink that only acknowledges (for paths that must not be observed posting).
struct AckSink;

#[async_trait::async_trait]
impl GlPostSink for AckSink {
    async fn post(&self, _e: &AccountingPostEnvelope) -> Result<GlPostAck, GlPostRejected> {
        Ok(GlPostAck { post_id: Uuid::new_v4(), journal_id: Uuid::new_v4(), idempotent_reuse: false })
    }
}

/// Map inventory's envelope into the REAL backbone-accounting PostingService — the same ACL
/// the composing service ships. A repost through this sink lands a real journal, so the
/// re-drive's idempotency (source-identity dedupe, no duplicate journal lines) is proven
/// against the actual ledger, not an acknowledging stub. The envelope's `company_id` is the
/// documented legacy twin (ADR-0029) — accounting's request shape still carries it, and no
/// statement keys on it.
struct RealLedgerSink {
    svc: PostingService,
}

#[async_trait::async_trait]
impl GlPostSink for RealLedgerSink {
    async fn post(&self, e: &AccountingPostEnvelope) -> Result<GlPostAck, GlPostRejected> {
        let mut req = PostingRequest::original(
            e.company_id,
            &e.source_type,
            e.source_id,
            e.posting_date,
        );
        req.branch_id = e.branch_id;
        req.source_reference = e.source_reference.clone();
        req.currency = e.currency.clone();
        req.description = e.description.clone();
        req.lines = e
            .lines
            .iter()
            .map(|l| PostingLine {
                account_id: l.account_id,
                debit: l.debit,
                credit: l.credit,
                party_type: l.party_type.clone(),
                party_id: l.party_id,
                cost_center_id: None,
                project_id: None,
                department_id: None,
                description: l.description.clone(),
            })
            .collect();
        match self.svc.post(req, None).await {
            Ok(r) => Ok(GlPostAck {
                post_id: r.post_id,
                journal_id: r.journal_id,
                idempotent_reuse: r.idempotent_reuse,
            }),
            Err(e) => Err(GlPostRejected { code: e.code().to_string(), message: e.to_string() }),
        }
    }
}

/// Insert a location row. `override_acct` optionally stamps the location's valuation-account
/// override, the column the door's account-resolution chain reads. The `parent_path` chains
/// from the row's own id — the subtree queries walk `parent_path LIKE parent || '%'`, so a
/// flat "" path would make every location a root of every other and double-count moves.
async fn loc(
    pool: &PgPool,
    usage: &str,
    wh: Option<Uuid>,
    override_acct: Option<Uuid>,
) -> Uuid {
    let id = Uuid::new_v4();
    let name = uq("LOC");
    sqlx::query(
        r#"INSERT INTO inventory.locations
             (id, name, complete_name, usage, parent_path, warehouse_id,
              valuation_account_id)
           VALUES ($1,$2,$3,$4::location_usage,$5,$6,$7)"#,
    )
    .bind(id)
    .bind(&name)
    .bind(&name)
    .bind(usage)
    .bind(format!("{id}/"))
    .bind(wh)
    .bind(override_acct)
    .execute(pool)
    .await
    .expect("insert location");
    id
}

/// Seed the warehouse's valuation bin (the (item, warehouse) estate the done verb's OUT leg
/// values the draw from). The module keeps two estates — quants (physical, location grain)
/// and bins (valuation, warehouse grain) — a move draws from both, so a probe that seeds
/// only the quant estate fails the bin's availability guard.
async fn seed_bin(
    pool: &PgPool,
    item: Uuid,
    wh: Uuid,
    qty: &str,
    rate: &str,
) {
    sqlx::query(
        r#"INSERT INTO inventory.bins
             (id, item_id, warehouse_id, actual_qty, reserved_qty, valuation_rate, stock_value)
           VALUES ($1,$2,$3,$4,0,$5,$6)"#,
    )
    .bind(Uuid::new_v4())
    .bind(item)
    .bind(wh)
    .bind(d(qty))
    .bind(d(rate))
    .bind(d(qty) * d(rate))
    .execute(pool)
    .await
    .expect("seed bin");
}

/// Seed on-hand stock at a location (one untracked-dims quant row).
async fn seed_quant(
    pool: &PgPool,
    item: Uuid,
    location: Uuid,
    qty: &str,
) {
    sqlx::query(
        r#"INSERT INTO inventory.stock_quants
             (id, item_id, location_id, quantity, reserved_quantity, available_quantity)
           VALUES ($1,$2,$3,$4,0,$4)"#,
    )
    .bind(Uuid::new_v4())
    .bind(item)
    .bind(location)
    .bind(d(qty))
    .execute(pool)
    .await
    .expect("seed quant");
}

fn new_move(item: Uuid, src: Uuid, dst: Uuid, qty: &str) -> NewStockMove {
    NewStockMove {
        name: uq("MV"),
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

// ── probe 1: the full lifecycle through the state verbs ─────────────────────────

/// `create_move → action_confirm → action_assign → action_done` through the real verbs. The
/// probe drives the mint-to-done lifecycle and asserts the physical estate the done verb
/// wrote (quant flipped at both endpoints, SLE row minted), so a silent no-op cannot pass
/// for a lifecycle.
#[tokio::test]
async fn lifecycle_confirm_assign_done_writes_real_rows() {
    let pool = pool().await;
    let w = InventoryWriteService::new(pool.clone());
    let item = Uuid::new_v4();

    let wh = w
        .create_warehouse(NewWarehouse {
            code: uq("WH"),
            name: uq("Main"),
            warehouse_type: None,
            parent_warehouse_id: None,
            is_group: false,
        })
        .await
        .expect("create_warehouse");
    let stock = loc(&pool, "internal", Some(wh), None).await;
    let customer = loc(&pool, "customer", None, None).await;
    seed_quant(&pool, item, stock, "10").await;
    seed_bin(&pool, item, wh, "10", "50").await;

    let mv = w
        .create_move(new_move(item, stock, customer, "6"))
        .await
        .expect("create_move");

    let confirmed = w.action_confirm(mv).await.expect("confirm");
    assert_eq!(confirmed, "confirmed");

    let assigned = w.action_assign(mv).await.expect("assign");
    assert_eq!(assigned.state, "assigned", "10 on hand covers demand 6");

    let done = w
        .action_done(mv, BackorderPolicy::Never, &MoveGlDirective::default(), &AckSink)
        .await
        .expect("done");
    assert_eq!(done.done_qty, d("6"));

    // Real rows, read back as the owner.
    let state: String = sqlx::query_scalar("SELECT state::text FROM inventory.stock_moves WHERE id=$1")
        .bind(mv)
        .fetch_one(&pool)
        .await
        .expect("move row visible");
    assert_eq!(state, "done");
    let src_qty: Decimal = sqlx::query_scalar(
        "SELECT quantity FROM inventory.stock_quants WHERE item_id=$1 AND location_id=$2",
    )
    .bind(item)
    .bind(stock)
    .fetch_one(&pool)
    .await
    .expect("src quant visible");
    assert_eq!(src_qty, d("4"), "6 drawn from the source quant");
    let dst_qty: Decimal = sqlx::query_scalar(
        "SELECT quantity FROM inventory.stock_quants WHERE item_id=$1 AND location_id=$2",
    )
    .bind(item)
    .bind(customer)
    .fetch_one(&pool)
    .await
    .expect("dest quant visible");
    assert_eq!(dst_qty, d("6"), "6 landed at the destination quant");
    let sle: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM inventory.stock_ledger_entries WHERE voucher_id=$1",
    )
    .bind(mv)
    .fetch_one(&pool)
    .await
    .expect("SLE rows visible");
    assert!(sle >= 1, "the done verb minted its ledger rows (found {sle})");
}

// ── probe 2: the cancel path ─────────────────────────────────────────────────────

/// `create → confirm → assign → cancel`: cancel releases the reservation and lands `cancel`.
#[tokio::test]
async fn cancel_releases_the_reservation() {
    let pool = pool().await;
    let w = InventoryWriteService::new(pool.clone());
    let item = Uuid::new_v4();

    let wh = w
        .create_warehouse(NewWarehouse {
            code: uq("WH"),
            name: uq("Main"),
            warehouse_type: None,
            parent_warehouse_id: None,
            is_group: false,
        })
        .await
        .unwrap();
    let stock = loc(&pool, "internal", Some(wh), None).await;
    let customer = loc(&pool, "customer", None, None).await;
    seed_quant(&pool, item, stock, "5").await;

    let mv = w.create_move(new_move(item, stock, customer, "5")).await.unwrap();
    w.action_confirm(mv).await.unwrap();
    let assigned = w.action_assign(mv).await.unwrap();
    assert_eq!(assigned.state, "assigned");

    let released = w.action_cancel(mv).await.expect("cancel");
    assert_eq!(released, d("5"), "the whole reservation came back");

    let state: String = sqlx::query_scalar("SELECT state::text FROM inventory.stock_moves WHERE id=$1")
        .bind(mv)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(state, "cancel");
    let reserved: Decimal = sqlx::query_scalar(
        "SELECT reserved_quantity FROM inventory.stock_quants WHERE item_id=$1 AND location_id=$2",
    )
    .bind(item)
    .bind(stock)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(reserved, d("0"), "the authoritative counter drained back to zero");
}

// ── probe 3: the orderpoint compute read model ───────────────────────────────────

/// `recompute_orderpoint`: the orderpoint fetch and the compute/stamp write ride one
/// transaction, the forecast counts an incoming confirmed move, and the stamped computes
/// are readable afterward.
#[tokio::test]
async fn recompute_orderpoint_stamps_the_computes() {
    let pool = pool().await;
    let svc = ProcurementService::new(pool.clone());
    let item = Uuid::new_v4();

    let wh = InventoryWriteService::new(pool.clone())
        .create_warehouse(NewWarehouse {
            code: uq("WH"),
            name: uq("Main"),
            warehouse_type: None,
            parent_warehouse_id: None,
            is_group: false,
        })
        .await
        .unwrap();
    let stock = loc(&pool, "internal", Some(wh), None).await;
    let supplier = loc(&pool, "supplier", None, None).await;
    seed_quant(&pool, item, stock, "4").await;

    // Provisioned through the SERVICE CRUD (the real provisioning path).
    let op = svc
        .create_orderpoint(NewOrderpoint {
            name: uq("OP"),
            trigger: "manual".into(),
            item_id: item,
            location_id: stock,
            warehouse_id: wh,
            item_min_qty: d("8"),
            item_max_qty: d("12"),
            route_id: None,
        })
        .await
        .expect("create orderpoint through the service");
    // An incoming confirmed move the forecast must count (5 landing from the supplier).
    sqlx::query(
        r#"INSERT INTO inventory.stock_moves
             (id, name, state, item_id, demand_qty, quantity, price_unit, procure_method,
              location_id, location_dest_id, move_orig_ids, move_dest_ids)
           VALUES ($1,$2,'confirmed',$3,5,0,0,'make_to_stock',$4,$5,'{}'::uuid[],'{}'::uuid[])"#,
    )
    .bind(Uuid::new_v4())
    .bind(uq("MV-IN"))
    .bind(item)
    .bind(supplier)
    .bind(stock)
    .execute(&pool)
    .await
    .expect("insert incoming move");

    let computes = svc
        .recompute_orderpoint(op)
        .await
        .expect("recompute");
    assert_eq!(computes.qty_on_hand, d("4"));
    assert_eq!(computes.qty_forecast, d("9"), "on hand 4 + incoming 5");

    let stamped: Decimal = sqlx::query_scalar(
        "SELECT qty_forecast FROM inventory.reordering_rules WHERE id=$1",
    )
    .bind(op)
    .fetch_one(&pool)
    .await
    .expect("stamped computes visible");
    assert_eq!(stamped, d("9"), "the compute stamped the rule");
}

// ── probe 4: the location valuation-override read on the receipt door ───────────

/// The receipt door's inventory-leg account resolves the STOCK location's
/// `valuation_account_id` override when set, else the receipt header's account. The probe
/// stamps the override on the warehouse's stock location, submits a receipt, and asserts the
/// posted envelope's inventory debit used the OVERRIDE account — not a silent fallback to
/// the header account.
#[tokio::test]
async fn receipt_picks_up_the_location_valuation_override() {
    let pool = pool().await;
    let w = InventoryWriteService::new(pool.clone());
    let item = Uuid::new_v4();

    let wh = w
        .create_warehouse(NewWarehouse {
            code: uq("WH"),
            name: uq("Main"),
            warehouse_type: None,
            parent_warehouse_id: None,
            is_group: false,
        })
        .await
        .unwrap();
    let header_acct = Uuid::new_v4();
    let grir_acct = Uuid::new_v4();
    let override_acct = Uuid::new_v4();
    // The warehouse's stock location carries the valuation-account override.
    let stock = loc(&pool, "internal", Some(wh), Some(override_acct)).await;

    let rid = w
        .create_purchase_receipt(NewReceipt {
            receipt_number: uq("PR"),
            branch_id: None,
            supplier_id: Uuid::new_v4(),
            source_po_id: None,
            warehouse_id: wh,
            posting_date: chrono::Utc::now().date_naive(),
            currency: "IDR".into(),
            inventory_account_id: header_acct,
            grir_account_id: grir_acct,
            lines: vec![ReceiptLine {
                item_id: item,
                quantity: d("3"),
                rate: d("100"),
                is_landed_costs_line: false,
            }],
        })
        .await
        .expect("create receipt");

    let sink = Arc::new(RecordingSink::default());
    let outcome = w
        .submit_purchase_receipt(rid, sink.as_ref())
        .await
        .expect("submit receipt");
    assert!(outcome.posted, "the receipt posted its envelope");

    let posts = sink.posts.lock().unwrap();
    let env = posts
        .iter()
        .find(|e| e.source_id == rid)
        .expect("the receipt's own envelope was posted");
    let debit = env
        .lines
        .iter()
        .find(|l| l.debit > Decimal::ZERO)
        .expect("the envelope carries a debit leg");
    assert_eq!(
        debit.account_id, override_acct,
        "the inventory debit resolved the location's valuation-account override, not the \
         header account — a read that cannot see the location would silently book {header_acct}"
    );
    assert_ne!(debit.account_id, header_acct);

    // The receipt's move also completed end-to-end through the state verbs.
    let move_state: String = sqlx::query_scalar(
        r#"SELECT m.state::text FROM inventory.stock_moves m
           WHERE m.origin=$1 LIMIT 1"#,
    )
    .bind(env.source_reference.clone().expect("envelope carries the receipt number"))
    .fetch_one(&pool)
    .await
    .expect("the receipt's line move is visible");
    assert_eq!(move_state, "done");
    // The stock location the override was stamped on is the move's destination.
    let dst: Uuid = sqlx::query_scalar(
        r#"SELECT location_dest_id FROM inventory.stock_moves
           WHERE origin=$1 LIMIT 1"#,
    )
    .bind(env.source_reference.clone().unwrap())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(dst, stock, "the door resolved the warehouse's stock location");
}

// ── probe 5: the unreserve release arm ──────────────────────────────────────────

/// `create → confirm → assign → unreserve_move`. The probe asserts the release completed AND
/// the reservation actually returned to available: the mirror lines zero out, the
/// authoritative `reserved_quantity` drains back to zero and `available_quantity` is whole
/// again. The move's state is deliberately untouched — the release arm frees the hold; the
/// state verbs own the transitions.
#[tokio::test]
async fn unreserve_returns_the_reservation_to_available() {
    let pool = pool().await;
    let w = InventoryWriteService::new(pool.clone());
    let item = Uuid::new_v4();

    let wh = w
        .create_warehouse(NewWarehouse {
            code: uq("WH"),
            name: uq("Main"),
            warehouse_type: None,
            parent_warehouse_id: None,
            is_group: false,
        })
        .await
        .unwrap();
    let stock = loc(&pool, "internal", Some(wh), None).await;
    let customer = loc(&pool, "customer", None, None).await;
    seed_quant(&pool, item, stock, "5").await;

    let mv = w.create_move(new_move(item, stock, customer, "5")).await.unwrap();
    w.action_confirm(mv).await.unwrap();
    let assigned = w.action_assign(mv).await.unwrap();
    assert_eq!(assigned.state, "assigned");

    let (reserved, available): (Decimal, Decimal) = sqlx::query_as(
        "SELECT reserved_quantity, available_quantity FROM inventory.stock_quants \
         WHERE item_id=$1 AND location_id=$2",
    )
    .bind(item)
    .bind(stock)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((reserved, available), (d("5"), d("0")), "the assign held the whole bin");

    let released = w.unreserve_move(mv).await.expect("unreserve");
    assert_eq!(released, d("5"), "the release reports the quantity the lines held");

    let (reserved, available): (Decimal, Decimal) = sqlx::query_as(
        "SELECT reserved_quantity, available_quantity FROM inventory.stock_quants \
         WHERE item_id=$1 AND location_id=$2",
    )
    .bind(item)
    .bind(stock)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        (reserved, available),
        (d("0"), d("5")),
        "the authoritative counters returned the reservation to available"
    );
    let line_qty: Decimal = sqlx::query_scalar(
        "SELECT COALESCE(SUM(quantity),0) FROM inventory.stock_move_lines WHERE move_id=$1",
    )
    .bind(mv)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(line_qty, d("0"), "the mirror lines zeroed out");
    let state: String = sqlx::query_scalar("SELECT state::text FROM inventory.stock_moves WHERE id=$1")
        .bind(mv)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(state, "assigned", "the release arm holds the state; verbs transition it");
}

// ── probe 6: the GL re-drive sweep path ─────────────────────────────────────────

/// `repost_move_gl` against the REAL accounting ledger. The probe drives a move to `done`
/// with its GL posted (one real journal), simulates the crash window (the post landed, the
/// status write was lost → `posting_state='failed'`), re-drives, and asserts accounting's
/// source-identity dedupe returned the ORIGINAL journal — no duplicate journal lines — and
/// that an already-posted move short-circuits.
#[tokio::test]
async fn repost_gl_redrives_idempotently() {
    let pool = pool().await;
    let w = InventoryWriteService::new(pool.clone());
    let item = Uuid::new_v4();

    // The GL legs' accounts (a real chart pair: COGS detail + Inventory detail).
    let (cogs_acct, inv_acct) = (Uuid::new_v4(), Uuid::new_v4());
    for (id, num, name, at, st) in [
        (cogs_acct, "5100", "HPP (COGS)", "cogs", "direct_cost"),
        (inv_acct, "1300", "Persediaan", "asset", "inventory"),
    ] {
        sqlx::query(
            r#"INSERT INTO accounting.accounts
                 (id, account_number, account_code, name, account_type,
                  account_subtype, normal_balance, is_header, is_detail, status)
               VALUES ($1,$2,$2,$3,$4::account_type,$5::account_subtype,'debit',FALSE,TRUE,
                       'active'::account_status)"#,
        )
        .bind(id)
        .bind(num)
        .bind(name)
        .bind(at)
        .bind(st)
        .execute(&pool)
        .await
        .expect("seed account");
    }
    let adapter = RealLedgerSink {
        svc: PostingService::new(Arc::new(SqlxPostingRepository::new(pool.clone()))),
    };

    let wh = w
        .create_warehouse(NewWarehouse {
            code: uq("WH"),
            name: uq("Main"),
            warehouse_type: None,
            parent_warehouse_id: None,
            is_group: false,
        })
        .await
        .unwrap();
    let stock = loc(&pool, "internal", Some(wh), None).await;
    let customer = loc(&pool, "customer", None, None).await;
    seed_quant(&pool, item, stock, "10").await;
    seed_bin(&pool, item, wh, "10", "100").await;

    let gl = MoveGlDirective {
        cogs_account_id: Some(cogs_acct),
        inventory_account_id: Some(inv_acct),
        grir_account_id: None,
        adjustment_account_id: None,
        currency: "IDR".into(),
    };
    let mv = w
        .create_move(new_move(item, stock, customer, "4"))
        .await
        .unwrap();
    w.action_confirm(mv).await.unwrap();
    w.action_assign(mv).await.unwrap();
    let done = w
        .action_done(mv, BackorderPolicy::Never, &gl, &adapter)
        .await
        .expect("done (with GL)");
    assert!(done.gl_posted, "the original post landed");
    let journals: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM accounting.journals WHERE source_type='inventory' AND source_id=$1",
    )
    .bind(mv)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(journals, 1, "exactly one journal before the re-drive");
    let orig_jid: Uuid = sqlx::query_scalar(
        "SELECT id FROM accounting.journals WHERE source_type='inventory' AND source_id=$1",
    )
    .bind(mv)
    .fetch_one(&pool)
    .await
    .unwrap();

    // The crash window: the post landed but the status write was lost.
    sqlx::query("UPDATE inventory.stock_moves SET posting_state='failed'::gl_posting_state WHERE id=$1")
        .bind(mv)
        .execute(&pool)
        .await
        .expect("simulate the lost status write");

    // The re-drive: the fetch sees the move, the envelope re-builds from the committed
    // ledger legs, and accounting's dedupe on the source identity returns the ORIGINAL
    // journal instead of a second one.
    let out = w
        .repost_move_gl(mv, &gl, &adapter)
        .await
        .expect("repost");
    assert!(out.posted, "the re-drive healed the leg");
    assert_eq!(out.journal_id, Some(orig_jid), "dedupe returned the original journal");
    let journals: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM accounting.journals WHERE source_type='inventory' AND source_id=$1",
    )
    .bind(mv)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(journals, 1, "no second journal — no duplicate journal lines");
    let lines: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM accounting.journal_lines WHERE journal_id=$1",
    )
    .bind(orig_jid)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(lines, 2, "still exactly the original Dr/Cr pair");

    // An already-posted move short-circuits without re-emitting.
    let noop = w
        .repost_move_gl(mv, &gl, &adapter)
        .await
        .expect("settled repost");
    assert!(noop.posted);
    assert!(noop.journal_id.is_none(), "settled short-circuit re-emits nothing");
}

// ── probe 7: procurement provisioning through the CRUD path ─────────────────────

/// `create_route → create_rule → create_orderpoint` over the service. Each provisioning
/// write rides its own transaction with its duplicate-check read; the probe provisions a
/// full replenishment configuration and asserts `orderpoint_exists` — the service's own
/// duplicate guard — sees the row, and that a duplicate is the typed error, not a 500.
#[tokio::test]
async fn procurement_provisions_through_the_crud_path() {
    let pool = pool().await;
    let svc = ProcurementService::new(pool.clone());
    let item = Uuid::new_v4();

    let wh = InventoryWriteService::new(pool.clone())
        .create_warehouse(NewWarehouse {
            code: uq("WH"),
            name: uq("Main"),
            warehouse_type: None,
            parent_warehouse_id: None,
            is_group: false,
        })
        .await
        .unwrap();
    let stock = loc(&pool, "internal", Some(wh), None).await;

    // An incoming operation type (the rule's R11 reference). Its default endpoints are the
    // shared virtual roots the reference seed carries.
    let pt = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO inventory.operation_types
             (id, name, sequence_code, code, default_location_src_id,
              default_location_dest_id)
           VALUES ($1,$2,$3,'incoming'::picking_code,
                   '2c72e32f-0000-0000-0000-000000000001'::uuid,
                   '2c72e32f-0000-0000-0000-000000000002'::uuid)"#,
    )
    .bind(pt)
    .bind(uq("PT"))
    .bind(uq("IN"))
    .execute(&pool)
    .await
    .expect("seed operation type");

    let route = svc
        .create_route(NewRoute {
            name: uq("RT"),
            active: true,
            sequence: 10,
        })
        .await
        .expect("create_route");
    let rule = svc
        .create_rule(NewRouteRule {
            name: uq("RULE"),
            sequence: 10,
            action: "pull".into(),
            auto: "manual".into(),
            procure_method: "make_to_order".into(),
            delay: 1,
            location_src_id: None,
            location_dest_id: stock,
            picking_type_id: pt,
            route_id: route,
            warehouse_id: Some(wh),
            propagate_cancel: false,
        })
        .await
        .expect("create_rule (R13 pre-check reads included)");
    let op = svc
        .create_orderpoint(NewOrderpoint {
            name: uq("OP"),
            trigger: "manual".into(),
            item_id: item,
            location_id: stock,
            warehouse_id: wh,
            item_min_qty: d("4"),
            item_max_qty: d("9"),
            route_id: Some(route),
        })
        .await
        .expect("create_orderpoint");

    // Every row is really there, read back as the owner.
    let routes: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM inventory.routes WHERE id=$1")
        .bind(route)
        .fetch_one(&pool)
        .await
        .unwrap();
    let rules: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM inventory.route_rules WHERE id=$1")
        .bind(rule)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!((routes, rules), (1, 1), "the route and the rule landed");

    // The service's own duplicate guard sees the row — the read that guards the mint.
    let existing = ProcurementRepository::orderpoint_exists(&pool, item, stock)
        .await
        .expect("orderpoint_exists");
    assert_eq!(existing, 1, "the duplicate guard sees the provisioned orderpoint");

    // A duplicate (item, location) is still the typed R6 error, not a 500.
    let err = svc
        .create_orderpoint(NewOrderpoint {
            name: uq("OP2"),
            trigger: "manual".into(),
            item_id: item,
            location_id: stock,
            warehouse_id: wh,
            item_min_qty: d("4"),
            item_max_qty: d("9"),
            route_id: None,
        })
        .await
        .expect_err("the duplicate must be refused");
    assert_eq!(err.code(), "orderpoint_exists", "the R6 typed error survives");
    let orderpoints: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM inventory.reordering_rules WHERE id=$1",
    )
    .bind(op)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(orderpoints, 1, "the refused duplicate minted no second row");
}

// ── probe 8: the tenancy posture (ADR-0029) ─────────────────────────────────────

const ROLE: &str = "inventory_tenancy_probe";
const PWD: &str = "probe";

/// Shed the probe role's grants, then drop it. Leftover grants (from a run whose teardown
/// never reached the drop) make plain DROP ROLE fail with 2BP01 — DROP OWNED BY first keeps
/// both bootstrap and teardown idempotent across runs.
async fn drop_role(admin: &PgPool) {
    let _ = sqlx::query(&format!("DROP OWNED BY {ROLE}")).execute(admin).await;
    let _ = sqlx::query(&format!("DROP ROLE IF EXISTS {ROLE}")).execute(admin).await;
}

async fn bootstrap_role(admin: &PgPool, tables: &[&str]) {
    drop_role(admin).await;
    for stmt in [
        format!("CREATE ROLE {ROLE} LOGIN PASSWORD '{PWD}' NOSUPERUSER NOBYPASSRLS"),
        format!("GRANT USAGE ON SCHEMA inventory TO {ROLE}"),
    ]
    .into_iter()
    .chain(tables.iter().map(|t| {
        format!("GRANT SELECT, INSERT, UPDATE ON TABLE inventory.{t} TO {ROLE}")
    })) {
        sqlx::query(&stmt).execute(admin).await.unwrap();
    }
}

/// The module's base tables — the set whose company-fence artifacts the strip migration
/// removed (policies, company-leading indexes, the company_id column).
const BASE_TABLES: &[&str] = &[
    "warehouses",
    "stock_items",
    "locations",
    "lots",
    "packages",
    "operation_types",
    "transfers",
    "routes",
    "route_rules",
    "reordering_rules",
    "stock_moves",
    "stock_move_lines",
    "stock_quants",
    "stock_ledger_entries",
    "bins",
    "stock_entries",
    "stock_entry_items",
    "purchase_receipts",
    "purchase_receipt_items",
    "delivery_notes",
    "delivery_note_items",
    "stock_reconciliations",
    "stock_reconciliation_items",
    "inventory_company_settings",
    "landed_costs",
    "landed_cost_lines",
    "landed_cost_adjustment_lines",
    "picking_batches",
    "scraps",
    "scrap_reason_tags",
    "package_types",
    "storage_categories",
    "storage_category_capacities",
    "putaway_rules",
];

/// The tenancy posture, pinned from below (ADR-0029): every inventory base table carries
/// ENABLE + FORCE ROW LEVEL SECURITY (the decorator completes the fence with its org-scoped
/// policies) and the module ships ZERO policies of its own; no company_id column survives
/// anywhere in the schema; a plain NOBYPASSRLS role is default-denied — reads return zero
/// rows even with the legacy company variable set, and its writes are refused — while the
/// owner pool still sees the rows it seeded.
#[tokio::test]
async fn tenancy_posture_flags_without_policies() {
    let pool = pool().await;

    // Sanity: the half-fence is armed on every base table (ENABLE + FORCE — the owner is
    // fenced too once policies exist).
    let armed: Vec<String> = sqlx::query(
        "SELECT c.relname FROM pg_class c \
         JOIN pg_namespace n ON n.oid = c.relnamespace \
         WHERE n.nspname = 'inventory' AND c.relkind = 'r' \
           AND c.relrowsecurity AND c.relforcerowsecurity \
         ORDER BY c.relname",
    )
    .fetch_all(&pool)
    .await
    .expect("pg_class")
    .iter()
    .map(|r| r.get::<String, _>("relname"))
    .collect();
    for table in BASE_TABLES {
        assert!(
            armed.iter().any(|t| t == table),
            "{table} must carry ENABLE + FORCE ROW LEVEL SECURITY"
        );
    }

    // The module ships NO tenancy policy: under ADR-0029 isolation belongs to the composing
    // service's decorator, never to the module.
    let policies: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pg_policies WHERE schemaname = 'inventory'",
    )
    .fetch_one(&pool)
    .await
    .expect("pg_policies");
    assert_eq!(policies, 0, "the module declares no tenancy policy");

    // The company columns are gone from every inventory table.
    let company_cols: i64 = sqlx::query_scalar(
        r#"SELECT count(*) FROM information_schema.columns
            WHERE table_schema = 'inventory' AND column_name = 'company_id'"#,
    )
    .fetch_one(&pool)
    .await
    .expect("information_schema");
    assert_eq!(company_cols, 0, "no company_id column survives the strip");

    // A non-owner NOBYPASSRLS session: with no policy there is nothing to admit it, so it is
    // default-denied regardless of any legacy company variable — while the owner still sees
    // the seeded row (the denial is the fence, not an empty database).
    bootstrap_role(&pool, &["locations"]).await;
    let restricted = PgPool::connect(&format!(
        "postgresql://{ROLE}:{PWD}@localhost:5433/backbone_inventory"
    ))
    .await
    .expect("probe-role connect");

    let loc_id = Uuid::new_v4();
    let name = uq("LOC");
    sqlx::query(
        r#"INSERT INTO inventory.locations
             (id, name, complete_name, usage, parent_path)
           VALUES ($1,$2,$2,'internal',$3)"#,
    )
    .bind(loc_id)
    .bind(&name)
    .bind(format!("{loc_id}/"))
    .execute(&pool)
    .await
    .expect("seed location as owner");

    let mut app = sqlx::PgConnection::connect(&format!(
        "postgresql://{ROLE}:{PWD}@localhost:5433/backbone_inventory"
    ))
    .await
    .expect("app-role connect");
    sqlx::query("SELECT set_config('app.company_id', $1, false)")
        .bind(Uuid::new_v4().to_string())
        .execute(&mut app)
        .await
        .expect("set legacy variable");
    let seen: i64 = sqlx::query_scalar("SELECT count(*) FROM inventory.locations WHERE id=$1")
        .bind(loc_id)
        .fetch_one(&mut app)
        .await
        .expect("location count as the app role");
    assert_eq!(seen, 0, "no policy admits the app role, even with the legacy variable set");
    let owner_seen: i64 = sqlx::query_scalar("SELECT count(*) FROM inventory.locations WHERE id=$1")
        .bind(loc_id)
        .fetch_one(&pool)
        .await
        .expect("location count as owner");
    assert_eq!(owner_seen, 1, "the owner sees the seeded row");
    let insert = sqlx::query(
        r#"INSERT INTO inventory.locations
             (id, name, complete_name, usage, parent_path)
           VALUES ($1,$2,$2,'internal',$3)"#,
    )
    .bind(Uuid::new_v4())
    .bind(uq("LOC"))
    .bind("probe/".to_string())
    .execute(&mut app)
    .await;
    match insert {
        Err(e) => assert!(
            e.to_string().contains("row-level security") || e.to_string().contains("policy"),
            "wrong error: {e}"
        ),
        Ok(_) => panic!("a default-denied role must not insert"),
    }

    drop_role(&pool).await;
}
