//! Armed-fence lifecycle probes for the stock-move engine and its read paths.
//!
//! These probes MUST run against a fenced database: a non-superuser role connecting to a
//! database where the inventory tables carry their row-level-security fence (ENABLE + FORCE,
//! policy `company_id = current_setting('app.company_id') ...`) — exactly the posture the
//! module's own migrations create and the composing service runs under. The behavior suites
//! run as the migration owner (superuser), which BYPASSES row-level security, so they are
//! structurally blind to the unbound-read defect class these probes exist to hold shut: any
//! read issued on a pooled connection or before the transaction's `app.company_id` bind sees
//! zero rows and the verb 404s (or a silent fallback fires) for a perfectly legitimate move.
//!
//! Probes drive the REAL services (the same `InventoryWriteService` / `ProcurementService`
//! constructors a composing service builds) connected AS the restricted role, and exercise:
//! the full lifecycle `create_move → action_confirm → action_assign → action_done`, one
//! `action_cancel` path, one `recompute_orderpoint` pass, the receipt door's location
//! valuation-override resolution, the `unreserve_move` release arm, a `repost_move_gl`
//! re-drive against the real accounting ledger, and the procurement provisioning CRUD
//! (`create_route` / `create_rule` / `create_orderpoint`) — every one of these 404'd,
//! no-op'd, fell back to the header account, or failed the RLS WITH CHECK under an armed
//! fence before the bind-before-fetch / bind-before-write fixes they pin.
//!
//! Gated on `INVENTORY_FENCE_DSN`: a DSN for the restricted, non-superuser, non-owner role
//! (e.g. `postgresql://inv_fence_app:<pw>@127.0.0.1:5433/<db>`). The database must already
//! carry the module's migrations (they arm the fence), the reference-data seed, and — for
//! the GL re-drive probe, which posts through the REAL backbone-accounting PostingService —
//! the accounting schema's up-migrations. The role needs USAGE on the inventory schema plus
//! SELECT/INSERT/UPDATE/DELETE on its tables and USAGE on its sequences, and the same grants
//! on the accounting schema for the GL leg. Skips with a printed reason when the DSN is
//! absent, so an unfenced dev database does not fail the run.

use rust_decimal::Decimal;
use sqlx::PgPool;
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

fn fence_dsn() -> Option<String> {
    std::env::var("INVENTORY_FENCE_DSN").ok()
}

macro_rules! fence_or_skip {
    ($dsn:ident) => {
        let Some($dsn) = fence_dsn() else {
            eprintln!(
                "skipping: armed-fence probes need a fenced database — set INVENTORY_FENCE_DSN \
                 to a restricted-role DSN on a database with the inventory fence armed \
                 (see this file's header for the required grants)"
            );
            return;
        };
    };
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
/// against the actual ledger, not an acknowledging stub.
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

/// A connection from the RESTRICTED pool with `app.company_id` set at session level: every
/// raw seeding/assertion statement below runs as the fenced role inside the company's scope,
/// mirroring what the application role sees. The setting rides this one connection only.
async fn scoped_conn(
    pool: &PgPool,
    company: Uuid,
) -> sqlx::pool::PoolConnection<sqlx::Postgres> {
    let mut conn = pool.acquire().await.expect("acquire restricted connection");
    sqlx::query("SELECT set_config('app.company_id', $1, false)")
        .bind(company.to_string())
        .execute(&mut *conn)
        .await
        .expect("bind app.company_id on probe connection");
    conn
}

/// Insert a location row as the fenced role (the company scope must be set — see
/// [`scoped_conn`]). `override_acct` optionally stamps the location's valuation-account
/// override, the column the door's account-resolution chain reads. The `parent_path` chains
/// from the row's own id — the subtree queries walk `parent_path LIKE parent || '%'`, so a
/// flat "" path would make every location a root of every other and double-count moves.
async fn loc(
    conn: &mut sqlx::PgConnection,
    company: Uuid,
    usage: &str,
    wh: Option<Uuid>,
    override_acct: Option<Uuid>,
) -> Uuid {
    let id = Uuid::new_v4();
    let name = uq("LOC");
    sqlx::query(
        r#"INSERT INTO inventory.locations
             (id, name, complete_name, usage, parent_path, company_id, warehouse_id,
              valuation_account_id)
           VALUES ($1,$2,$3,$4::location_usage,$5,$6,$7,$8)"#,
    )
    .bind(id)
    .bind(&name)
    .bind(&name)
    .bind(usage)
    .bind(format!("{id}/"))
    .bind(company)
    .bind(wh)
    .bind(override_acct)
    .execute(conn)
    .await
    .expect("insert location as fenced role");
    id
}

/// Seed the warehouse's valuation bin (the (item, warehouse) estate the done verb's OUT leg
/// values the draw from). The module keeps two estates — quants (physical, location grain)
/// and bins (valuation, warehouse grain) — a move draws from both, so a probe that seeds
/// only the quant estate fails the bin's availability guard.
async fn seed_bin(
    conn: &mut sqlx::PgConnection,
    company: Uuid,
    item: Uuid,
    wh: Uuid,
    qty: &str,
    rate: &str,
) {
    sqlx::query(
        r#"INSERT INTO inventory.bins
             (id, company_id, item_id, warehouse_id, actual_qty, reserved_qty, valuation_rate, stock_value)
           VALUES ($1,$2,$3,$4,$5,0,$6,$7)"#,
    )
    .bind(Uuid::new_v4())
    .bind(company)
    .bind(item)
    .bind(wh)
    .bind(d(qty))
    .bind(d(rate))
    .bind(d(qty) * d(rate))
    .execute(conn)
    .await
    .expect("seed bin as fenced role");
}

/// Seed on-hand stock at a location (one untracked-dims quant row).
async fn seed_quant(
    conn: &mut sqlx::PgConnection,
    company: Uuid,
    item: Uuid,
    location: Uuid,
    qty: &str,
) {
    sqlx::query(
        r#"INSERT INTO inventory.stock_quants
             (id, item_id, location_id, quantity, reserved_quantity, available_quantity, company_id)
           VALUES ($1,$2,$3,$4,0,$4,$5)"#,
    )
    .bind(Uuid::new_v4())
    .bind(item)
    .bind(location)
    .bind(d(qty))
    .bind(company)
    .execute(conn)
    .await
    .expect("seed quant as fenced role");
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

// ── probe 1: the full lifecycle through the fixed verbs ─────────────────────────

/// `create_move → action_confirm → action_assign → action_done` as the restricted role under
/// the armed fence. Before the state verbs bound the company scope before their move fetch,
/// every one of the four verbs after the mint 404'd here (`NotFound`) — the unbound read saw
/// zero rows. The probe also asserts the physical estate the done verb wrote (quant flipped
/// at both endpoints, SLE row minted), so a silent no-op cannot pass for a lifecycle.
#[tokio::test]
async fn fenced_lifecycle_confirm_assign_done_writes_real_rows() {
    fence_or_skip!(dsn);
    let pool = PgPool::connect(&dsn).await.expect("connect as restricted role");
    let w = InventoryWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let item = Uuid::new_v4();
    let mut conn = scoped_conn(&pool, company).await;

    let wh = w
        .create_warehouse(NewWarehouse {
            company_id: company,
            code: uq("WH"),
            name: uq("Main"),
            warehouse_type: None,
            parent_warehouse_id: None,
            is_group: false,
        })
        .await
        .expect("create_warehouse under fence");
    let stock = loc(&mut conn, company, "internal", Some(wh), None).await;
    let customer = loc(&mut conn, company, "customer", None, None).await;
    seed_quant(&mut conn, company, item, stock, "10").await;
    seed_bin(&mut conn, company, item, wh, "10", "50").await;

    let mv = w
        .create_move(new_move(company, item, stock, customer, "6"))
        .await
        .expect("create_move under fence");

    let confirmed = w.action_confirm(company, mv).await.expect("confirm under fence");
    assert_eq!(confirmed, "confirmed");

    let assigned = w.action_assign(company, mv).await.expect("assign under fence");
    assert_eq!(assigned.state, "assigned", "10 on hand covers demand 6 under the fence");

    let done = w
        .action_done(company, mv, BackorderPolicy::Never, &MoveGlDirective::default(), &AckSink)
        .await
        .expect("done under fence");
    assert_eq!(done.done_qty, d("6"));

    // Real rows, read back through the fence as the restricted role.
    let state: String = sqlx::query_scalar("SELECT state::text FROM inventory.stock_moves WHERE id=$1")
        .bind(mv)
        .fetch_one(&mut *conn)
        .await
        .expect("move row visible in scope");
    assert_eq!(state, "done");
    let src_qty: Decimal = sqlx::query_scalar(
        "SELECT quantity FROM inventory.stock_quants WHERE company_id=$1 AND item_id=$2 AND location_id=$3",
    )
    .bind(company)
    .bind(item)
    .bind(stock)
    .fetch_one(&mut *conn)
    .await
    .expect("src quant visible in scope");
    assert_eq!(src_qty, d("4"), "6 drawn from the source quant");
    let dst_qty: Decimal = sqlx::query_scalar(
        "SELECT quantity FROM inventory.stock_quants WHERE company_id=$1 AND item_id=$2 AND location_id=$3",
    )
    .bind(company)
    .bind(item)
    .bind(customer)
    .fetch_one(&mut *conn)
    .await
    .expect("dest quant visible in scope");
    assert_eq!(dst_qty, d("6"), "6 landed at the destination quant");
    let sle: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM inventory.stock_ledger_entries WHERE company_id=$1 AND voucher_id=$2",
    )
    .bind(company)
    .bind(mv)
    .fetch_one(&mut *conn)
    .await
    .expect("SLE rows visible in scope");
    assert!(sle >= 1, "the done verb minted its ledger rows (found {sle})");
}

// ── probe 2: the cancel path ─────────────────────────────────────────────────────

/// `create → confirm → assign → cancel` as the restricted role: cancel's fetch must see the
/// reserved move (it 404'd before the bind), release the reservation, and land `cancel`.
#[tokio::test]
async fn fenced_cancel_releases_the_reservation() {
    fence_or_skip!(dsn);
    let pool = PgPool::connect(&dsn).await.expect("connect as restricted role");
    let w = InventoryWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let item = Uuid::new_v4();
    let mut conn = scoped_conn(&pool, company).await;

    let wh = w
        .create_warehouse(NewWarehouse {
            company_id: company,
            code: uq("WH"),
            name: uq("Main"),
            warehouse_type: None,
            parent_warehouse_id: None,
            is_group: false,
        })
        .await
        .unwrap();
    let stock = loc(&mut conn, company, "internal", Some(wh), None).await;
    let customer = loc(&mut conn, company, "customer", None, None).await;
    seed_quant(&mut conn, company, item, stock, "5").await;

    let mv = w.create_move(new_move(company, item, stock, customer, "5")).await.unwrap();
    w.action_confirm(company, mv).await.unwrap();
    let assigned = w.action_assign(company, mv).await.unwrap();
    assert_eq!(assigned.state, "assigned");

    let released = w.action_cancel(company, mv).await.expect("cancel under fence");
    assert_eq!(released, d("5"), "the whole reservation came back");

    let state: String = sqlx::query_scalar("SELECT state::text FROM inventory.stock_moves WHERE id=$1")
        .bind(mv)
        .fetch_one(&mut *conn)
        .await
        .unwrap();
    assert_eq!(state, "cancel");
    let reserved: Decimal = sqlx::query_scalar(
        "SELECT reserved_quantity FROM inventory.stock_quants WHERE company_id=$1 AND item_id=$2 AND location_id=$3",
    )
    .bind(company)
    .bind(item)
    .bind(stock)
    .fetch_one(&mut *conn)
    .await
    .unwrap();
    assert_eq!(reserved, d("0"), "the authoritative counter drained back to zero");
}

// ── probe 3: the orderpoint compute read model ───────────────────────────────────

/// `recompute_orderpoint` as the restricted role: the orderpoint fetch and the compute/stamp
/// writes all ride one company-bound transaction. Before the fix the unbound fetch 404'd (or,
/// unfenced, stamped computes across the fence boundary).
#[tokio::test]
async fn fenced_recompute_orderpoint_stamps_the_computes() {
    fence_or_skip!(dsn);
    let pool = PgPool::connect(&dsn).await.expect("connect as restricted role");
    let svc = ProcurementService::new(pool.clone());
    let company = Uuid::new_v4();
    let item = Uuid::new_v4();
    let mut conn = scoped_conn(&pool, company).await;

    let wh = InventoryWriteService::new(pool.clone())
        .create_warehouse(NewWarehouse {
            company_id: company,
            code: uq("WH"),
            name: uq("Main"),
            warehouse_type: None,
            parent_warehouse_id: None,
            is_group: false,
        })
        .await
        .unwrap();
    let stock = loc(&mut conn, company, "internal", Some(wh), None).await;
    let supplier = loc(&mut conn, company, "supplier", None, None).await;
    seed_quant(&mut conn, company, item, stock, "4").await;

    // Provisioned through the SERVICE CRUD (the real provisioning path) — the insert rides a
    // transaction whose `app.company_id` is bound before the duplicate-check read and the
    // write, so the fence's WITH CHECK admits it.
    let op = svc
        .create_orderpoint(NewOrderpoint {
            name: uq("OP"),
            trigger: "manual".into(),
            item_id: item,
            location_id: stock,
            warehouse_id: wh,
            company_id: company,
            item_min_qty: d("8"),
            item_max_qty: d("12"),
            route_id: None,
        })
        .await
        .expect("create orderpoint through the service under the fence");
    // An incoming confirmed move the forecast must count (5 landing from the supplier).
    sqlx::query(
        r#"INSERT INTO inventory.stock_moves
             (id, name, state, item_id, demand_qty, quantity, price_unit, procure_method,
              location_id, location_dest_id, company_id, move_orig_ids, move_dest_ids)
           VALUES ($1,$2,'confirmed',$3,5,0,0,'make_to_stock',$4,$5,$6,'{}'::uuid[],'{}'::uuid[])"#,
    )
    .bind(Uuid::new_v4())
    .bind(uq("MV-IN"))
    .bind(item)
    .bind(supplier)
    .bind(stock)
    .bind(company)
    .execute(&mut *conn)
    .await
    .expect("insert incoming move as fenced role");

    let computes = svc
        .recompute_orderpoint(company, op)
        .await
        .expect("recompute under fence");
    assert_eq!(computes.qty_on_hand, d("4"));
    assert_eq!(computes.qty_forecast, d("9"), "on hand 4 + incoming 5");

    let stamped: Decimal = sqlx::query_scalar(
        "SELECT qty_forecast FROM inventory.reordering_rules WHERE id=$1 AND company_id=$2",
    )
    .bind(op)
    .bind(company)
    .fetch_one(&mut *conn)
    .await
    .expect("stamped computes visible in scope");
    assert_eq!(stamped, d("9"), "the compute wrote through the fence's WITH CHECK");

    // A cross-company id must read as absent (fail-closed), not operable.
    let other = Uuid::new_v4();
    let err = svc.recompute_orderpoint(other, op).await;
    assert!(err.is_err(), "an orderpoint of another company cannot be recomputed");
}

// ── probe 4: the location valuation-override read on the receipt door ───────────

/// The receipt door's inventory-leg account resolves the STOCK location's
/// `valuation_account_id` override when set, else the receipt header's account. The override
/// read runs on a pooled connection: under an armed fence an unscoped read cannot see a
/// company-owned location at all and the chain silently falls back to the header account —
/// the wrong ledger. The probe stamps the override on the company's stock location, submits
/// a receipt as the restricted role, and asserts the posted envelope's inventory debit used
/// the OVERRIDE account.
#[tokio::test]
async fn fenced_receipt_picks_up_the_location_valuation_override() {
    fence_or_skip!(dsn);
    let pool = PgPool::connect(&dsn).await.expect("connect as restricted role");
    let w = InventoryWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let item = Uuid::new_v4();
    let mut conn = scoped_conn(&pool, company).await;

    let wh = w
        .create_warehouse(NewWarehouse {
            company_id: company,
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
    let stock = loc(&mut conn, company, "internal", Some(wh), Some(override_acct)).await;

    let rid = w
        .create_purchase_receipt(NewReceipt {
            receipt_number: uq("PR"),
            company_id: company,
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
        .expect("create receipt under fence");

    let sink = Arc::new(RecordingSink::default());
    let outcome = backbone_orm::company_scope::with_company_scope(
        Some(company),
        w.submit_purchase_receipt(rid, sink.as_ref()),
    )
    .await
    .expect("submit receipt under fence");
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
         header account — an unscoped read cannot see a company-owned location under the \
         fence and would silently book {header_acct}"
    );
    assert_ne!(debit.account_id, header_acct);

    // The receipt's move also completed end-to-end through the fixed verbs under the fence.
    let move_state: String = sqlx::query_scalar(
        r#"SELECT m.state::text FROM inventory.stock_moves m
           WHERE m.company_id=$1 AND m.origin=$2 LIMIT 1"#,
    )
    .bind(company)
    .bind(env.source_reference.clone().expect("envelope carries the receipt number"))
    .fetch_one(&mut *conn)
    .await
    .expect("the receipt's line move is visible in scope");
    assert_eq!(move_state, "done");
    // The stock location the override was stamped on is the move's destination.
    let dst: Uuid = sqlx::query_scalar(
        r#"SELECT location_dest_id FROM inventory.stock_moves
           WHERE company_id=$1 AND origin=$2 LIMIT 1"#,
    )
    .bind(company)
    .bind(env.source_reference.clone().unwrap())
    .fetch_one(&mut *conn)
    .await
    .unwrap();
    assert_eq!(dst, stock, "the door resolved the warehouse's stock location");
}

// ── probe 5: the unreserve release arm ──────────────────────────────────────────

/// `create → confirm → assign → unreserve_move` as the restricted role. Before the release
/// verb bound the company scope before its move fetch, the unbound read saw zero rows and a
/// legitimate reserved move 404'd — so the voucher doors' partial-assign rollback (release
/// whatever a partial reservation took) could never fire under the fence. The probe asserts
/// the release completed AND the reservation actually returned to available: the mirror
/// lines zero out, the authoritative `reserved_quantity` drains back to zero and
/// `available_quantity` is whole again. The move's state is deliberately untouched — the
/// release arm frees the hold; the state verbs own the transitions.
#[tokio::test]
async fn fenced_unreserve_returns_the_reservation_to_available() {
    fence_or_skip!(dsn);
    let pool = PgPool::connect(&dsn).await.expect("connect as restricted role");
    let w = InventoryWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let item = Uuid::new_v4();
    let mut conn = scoped_conn(&pool, company).await;

    let wh = w
        .create_warehouse(NewWarehouse {
            company_id: company,
            code: uq("WH"),
            name: uq("Main"),
            warehouse_type: None,
            parent_warehouse_id: None,
            is_group: false,
        })
        .await
        .unwrap();
    let stock = loc(&mut conn, company, "internal", Some(wh), None).await;
    let customer = loc(&mut conn, company, "customer", None, None).await;
    seed_quant(&mut conn, company, item, stock, "5").await;

    let mv = w.create_move(new_move(company, item, stock, customer, "5")).await.unwrap();
    w.action_confirm(company, mv).await.unwrap();
    let assigned = w.action_assign(company, mv).await.unwrap();
    assert_eq!(assigned.state, "assigned");

    let (reserved, available): (Decimal, Decimal) = sqlx::query_as(
        "SELECT reserved_quantity, available_quantity FROM inventory.stock_quants \
         WHERE company_id=$1 AND item_id=$2 AND location_id=$3",
    )
    .bind(company)
    .bind(item)
    .bind(stock)
    .fetch_one(&mut *conn)
    .await
    .unwrap();
    assert_eq!((reserved, available), (d("5"), d("0")), "the assign held the whole bin");

    let released = w.unreserve_move(company, mv).await.expect("unreserve under fence");
    assert_eq!(released, d("5"), "the release reports the quantity the lines held");

    let (reserved, available): (Decimal, Decimal) = sqlx::query_as(
        "SELECT reserved_quantity, available_quantity FROM inventory.stock_quants \
         WHERE company_id=$1 AND item_id=$2 AND location_id=$3",
    )
    .bind(company)
    .bind(item)
    .bind(stock)
    .fetch_one(&mut *conn)
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
    .fetch_one(&mut *conn)
    .await
    .unwrap();
    assert_eq!(line_qty, d("0"), "the mirror lines zeroed out");
    let state: String = sqlx::query_scalar("SELECT state::text FROM inventory.stock_moves WHERE id=$1")
        .bind(mv)
        .fetch_one(&mut *conn)
        .await
        .unwrap();
    assert_eq!(state, "assigned", "the release arm holds the state; verbs transition it");
}

// ── probe 6: the GL re-drive sweep path ─────────────────────────────────────────

/// `repost_move_gl` as the restricted role, against the REAL accounting ledger. Before the
/// re-drive verb bound the company scope before its move fetch, the unbound read saw zero
/// rows and a GL redrive sweep 404'd every stuck leg under the fence. The probe drives a
/// move to `done` with its GL posted (one real journal), simulates the crash window (the post
/// landed, the status write was lost → `posting_state='failed'`), re-drives through the
/// fence, and asserts accounting's source-identity dedupe returned the ORIGINAL journal —
/// no duplicate journal lines — and that an already-posted move short-circuits.
#[tokio::test]
async fn fenced_repost_gl_redrives_idempotently() {
    fence_or_skip!(dsn);
    let pool = PgPool::connect(&dsn).await.expect("connect as restricted role");
    let w = InventoryWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let item = Uuid::new_v4();
    let mut conn = scoped_conn(&pool, company).await;

    // The GL legs' accounts (a real chart pair: COGS detail + Inventory detail).
    let (cogs_acct, inv_acct) = (Uuid::new_v4(), Uuid::new_v4());
    for (id, num, name, at, st) in [
        (cogs_acct, "5100", "HPP (COGS)", "cogs", "direct_cost"),
        (inv_acct, "1300", "Persediaan", "asset", "inventory"),
    ] {
        sqlx::query(
            r#"INSERT INTO accounting.accounts
                 (id, company_id, account_number, account_code, name, account_type,
                  account_subtype, normal_balance, is_header, is_detail, status)
               VALUES ($1,$2,$3,$3,$4,$5::account_type,$6::account_subtype,'debit',FALSE,TRUE,
                       'active'::account_status)"#,
        )
        .bind(id)
        .bind(company)
        .bind(num)
        .bind(name)
        .bind(at)
        .bind(st)
        .execute(&mut *conn)
        .await
        .expect("seed account as fenced role");
    }
    let adapter = RealLedgerSink {
        svc: PostingService::new(Arc::new(SqlxPostingRepository::new(pool.clone()))),
    };

    let wh = w
        .create_warehouse(NewWarehouse {
            company_id: company,
            code: uq("WH"),
            name: uq("Main"),
            warehouse_type: None,
            parent_warehouse_id: None,
            is_group: false,
        })
        .await
        .unwrap();
    let stock = loc(&mut conn, company, "internal", Some(wh), None).await;
    let customer = loc(&mut conn, company, "customer", None, None).await;
    seed_quant(&mut conn, company, item, stock, "10").await;
    seed_bin(&mut conn, company, item, wh, "10", "100").await;

    let gl = MoveGlDirective {
        cogs_account_id: Some(cogs_acct),
        inventory_account_id: Some(inv_acct),
        grir_account_id: None,
        adjustment_account_id: None,
        currency: "IDR".into(),
    };
    // The ambient scope mirrors the composing service's request posture: pooled reads inside
    // the re-drive (the move's ledger-leg values, the posting repository) ride it.
    let mv = backbone_orm::company_scope::with_company_scope(
        Some(company),
        w.create_move(new_move(company, item, stock, customer, "4")),
    )
    .await
    .unwrap();
    backbone_orm::company_scope::with_company_scope(Some(company), w.action_confirm(company, mv))
        .await
        .unwrap();
    backbone_orm::company_scope::with_company_scope(Some(company), w.action_assign(company, mv))
        .await
        .unwrap();
    let done = backbone_orm::company_scope::with_company_scope(
        Some(company),
        w.action_done(company, mv, BackorderPolicy::Never, &gl, &adapter),
    )
    .await
    .expect("done (with GL) under fence");
    assert!(done.gl_posted, "the original post landed");
    let journals: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM accounting.journals WHERE company_id=$1",
    )
    .bind(company)
    .fetch_one(&mut *conn)
    .await
    .unwrap();
    assert_eq!(journals, 1, "exactly one journal before the re-drive");
    let orig_jid: Uuid = sqlx::query_scalar(
        "SELECT id FROM accounting.journals WHERE company_id=$1",
    )
    .bind(company)
    .fetch_one(&mut *conn)
    .await
    .unwrap();

    // The crash window: the post landed but the status write was lost.
    sqlx::query("UPDATE inventory.stock_moves SET posting_state='failed'::gl_posting_state WHERE id=$1")
        .bind(mv)
        .execute(&mut *conn)
        .await
        .expect("simulate the lost status write");

    // The re-drive through the fence: the fetch must see the move, the envelope re-builds
    // from the committed ledger legs, and accounting's dedupe on the source identity
    // returns the ORIGINAL journal instead of a second one.
    let out = backbone_orm::company_scope::with_company_scope(
        Some(company),
        w.repost_move_gl(company, mv, &gl, &adapter),
    )
    .await
    .expect("repost under fence");
    assert!(out.posted, "the re-drive healed the leg");
    assert_eq!(out.journal_id, Some(orig_jid), "dedupe returned the original journal");
    let journals: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM accounting.journals WHERE company_id=$1",
    )
    .bind(company)
    .fetch_one(&mut *conn)
    .await
    .unwrap();
    assert_eq!(journals, 1, "no second journal — no duplicate journal lines");
    let lines: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM accounting.journal_lines WHERE journal_id=$1",
    )
    .bind(orig_jid)
    .fetch_one(&mut *conn)
    .await
    .unwrap();
    assert_eq!(lines, 2, "still exactly the original Dr/Cr pair");

    // An already-posted move short-circuits without re-emitting.
    let noop = backbone_orm::company_scope::with_company_scope(
        Some(company),
        w.repost_move_gl(company, mv, &gl, &adapter),
    )
    .await
    .expect("settled repost under fence");
    assert!(noop.posted);
    assert!(noop.journal_id.is_none(), "settled short-circuit re-emits nothing");
}

// ── probe 7: procurement provisioning through the CRUD path ─────────────────────

/// `create_route → create_rule → create_orderpoint` over the service as the restricted
/// role. Before each provisioning write rode its own scope-bound transaction, the INSERTs
/// failed the fence's WITH CHECK (an unbound write cannot provision a company-owned
/// route/rule/orderpoint) and the duplicate-check read saw nothing. The probe provisions a
/// full replenishment configuration and asserts `orderpoint_exists` — the service's own
/// duplicate guard — sees the row as the fenced role.
#[tokio::test]
async fn fenced_procurement_provisions_through_the_crud_path() {
    fence_or_skip!(dsn);
    let pool = PgPool::connect(&dsn).await.expect("connect as restricted role");
    let svc = ProcurementService::new(pool.clone());
    let company = Uuid::new_v4();
    let item = Uuid::new_v4();
    let mut conn = scoped_conn(&pool, company).await;

    let wh = InventoryWriteService::new(pool.clone())
        .create_warehouse(NewWarehouse {
            company_id: company,
            code: uq("WH"),
            name: uq("Main"),
            warehouse_type: None,
            parent_warehouse_id: None,
            is_group: false,
        })
        .await
        .unwrap();
    let stock = loc(&mut conn, company, "internal", Some(wh), None).await;

    // A company-owned incoming operation type (the rule's R11 reference). Its default
    // endpoints are the shared virtual roots the reference seed carries.
    let pt = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO inventory.operation_types
             (id, name, sequence_code, code, company_id, default_location_src_id,
              default_location_dest_id)
           VALUES ($1,$2,$3,'incoming'::picking_code,$4,
                   '2c72e32f-0000-0000-0000-000000000001'::uuid,
                   '2c72e32f-0000-0000-0000-000000000002'::uuid)"#,
    )
    .bind(pt)
    .bind(uq("PT"))
    .bind(uq("IN"))
    .bind(company)
    .execute(&mut *conn)
    .await
    .expect("seed operation type as fenced role");

    let route = svc
        .create_route(NewRoute {
            name: uq("RT"),
            active: true,
            sequence: 10,
            company_id: Some(company),
        })
        .await
        .expect("create_route under fence");
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
            company_id: Some(company),
            propagate_cancel: false,
        })
        .await
        .expect("create_rule under fence (R11 pre-check reads included)");
    let op = svc
        .create_orderpoint(NewOrderpoint {
            name: uq("OP"),
            trigger: "manual".into(),
            item_id: item,
            location_id: stock,
            warehouse_id: wh,
            company_id: company,
            item_min_qty: d("4"),
            item_max_qty: d("9"),
            route_id: Some(route),
        })
        .await
        .expect("create_orderpoint under fence");

    // Every row is really there, read back through the fence as the restricted role.
    let routes: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM inventory.routes WHERE id=$1")
        .bind(route)
        .fetch_one(&mut *conn)
        .await
        .unwrap();
    let rules: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM inventory.route_rules WHERE id=$1 AND company_id=$2")
        .bind(rule)
        .bind(company)
        .fetch_one(&mut *conn)
        .await
        .unwrap();
    assert_eq!((routes, rules), (1, 1), "the route and the rule landed");

    // The service's own duplicate guard sees the row as the fenced role — the read that
    // was blind (always zero) before it rode the scope-bound transaction.
    let existing = ProcurementRepository::orderpoint_exists(&mut *conn, item, stock, company)
        .await
        .expect("orderpoint_exists under fence");
    assert_eq!(existing, 1, "the duplicate guard sees the provisioned orderpoint");

    // A duplicate (item, location, company) is still the typed R6 error, not a 500.
    let err = svc
        .create_orderpoint(NewOrderpoint {
            name: uq("OP2"),
            trigger: "manual".into(),
            item_id: item,
            location_id: stock,
            warehouse_id: wh,
            company_id: company,
            item_min_qty: d("4"),
            item_max_qty: d("9"),
            route_id: None,
        })
        .await
        .expect_err("the duplicate must be refused");
    assert_eq!(err.code(), "orderpoint_exists", "the R6 typed error survives the fence");
    let orderpoints: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM inventory.reordering_rules WHERE id=$1 AND company_id=$2",
    )
    .bind(op)
    .bind(company)
    .fetch_one(&mut *conn)
    .await
    .unwrap();
    assert_eq!(orderpoints, 1, "the refused duplicate minted no second row");
}
