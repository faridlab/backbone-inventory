//! Scrap-door + satellite master-data guard cases. Requires DATABASE_URL (default
//! :5433/backbone_inventory), schema applied.
//!
//! **The scrap door rides the ONE move engine (spec stock-business-logic.md §8):** the
//! document state is genuinely hand-set (draft → done), but the physical/valuation work
//! is a single `scrapped` move the engine drives — the quants flip, the SLEs mint, and
//! the GL leg posts exactly once through the engine's `is_inventory` shape. There is no
//! second estate anywhere in these cases.
//!
//! **The master-data guards (the register's deferred constraint family):** R7 scrap-reason
//! uniqueness, R8/R9 storage-capacity uniques, R10 package-type barcode, R15/16/17
//! positivity CHECKs, the capacity target XOR, and the T12 putaway derivation trigger —
//! each proven at the DB level (a raw-SQL writer cannot skip them), which is the
//! `enforcement: both` backstop half.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use rust_decimal::Decimal;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use backbone_inventory::application::service::inventory_gl::{
    AccountingPostEnvelope, GlPostAck, GlPostRejected, GlPostSink,
};
use backbone_inventory::application::service::inventory_move_engine::MoveGlDirective;
use backbone_inventory::application::service::inventory_scrap::NewScrap;
use backbone_inventory::application::service::inventory_write_service::{
    InventoryError, InventoryWriteService, NewWarehouse,
};

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

async fn warehouse(w: &InventoryWriteService) -> Uuid {
    w.create_warehouse(NewWarehouse {
        code: uq("WH"), name: uq("Main"),
        warehouse_type: None, parent_warehouse_id: None, is_group: false,
    }).await.unwrap()
}

async fn loc(pool: &PgPool, usage: &str, wh: Option<Uuid>) -> Uuid {
    let id = Uuid::new_v4();
    let name = uq("LOC");
    sqlx::query(
        r#"INSERT INTO inventory.locations
             (id, name, complete_name, usage, parent_path, warehouse_id)
           VALUES ($1,$2,$3,$4::location_usage,$5,$6)"#,
    )
    .bind(id).bind(&name).bind(&name).bind(usage).bind("").bind(wh)
    .execute(pool).await.unwrap();
    id
}

async fn seed_quant(pool: &PgPool, item: Uuid, location: Uuid, qty: &str) {
    sqlx::query(
        r#"INSERT INTO inventory.stock_quants
             (id, item_id, location_id, quantity, reserved_quantity, available_quantity)
           VALUES ($1,$2,$3,$4,0,$4)"#,
    )
    .bind(Uuid::new_v4()).bind(item).bind(location).bind(d(qty))
    .execute(pool).await.unwrap();
}

async fn seed_bin(pool: &PgPool, item: Uuid, wh: Uuid, qty: &str, rate: &str) {
    sqlx::query(
        r#"INSERT INTO inventory.bins
             (id, item_id, warehouse_id, actual_qty, reserved_qty, valuation_rate, stock_value)
           VALUES ($1,$2,$3,$4,0,$5,$6)"#,
    )
    .bind(Uuid::new_v4()).bind(item).bind(wh)
    .bind(d(qty)).bind(d(rate)).bind(d(qty) * d(rate))
    .execute(pool).await.unwrap();
}

async fn on_hand(pool: &PgPool, item: Uuid, location: Uuid) -> Decimal {
    sqlx::query_scalar(
        r#"SELECT COALESCE(SUM(quantity),0) FROM inventory.stock_quants
           WHERE item_id=$1 AND location_id=$2 AND (metadata->>'deleted_at') IS NULL"#,
    )
    .bind(item).bind(location)
    .fetch_one(pool).await.unwrap()
}

// ── the scrap door rides the move engine ─────────────────────────────────────

/// Mint (draft) → process: ONE scrapped move lands done, the stock leaves the source, the
/// GL posts exactly once through the engine's adjustment shape, and the header closes
/// `done` bound to that move. A second process is the typed not-draft refusal.
#[tokio::test]
async fn scrap_processes_through_the_engine_once() {
    let pool = pool().await;
    let svc = InventoryWriteService::new(pool.clone());
    let wh = warehouse(&svc).await;
    let stock = loc(&pool, "internal", Some(wh)).await;
    let item = Uuid::new_v4();
    seed_quant(&pool, item, stock, "7").await;
    seed_bin(&pool, item, wh, "7", "3").await;

    let created = svc.create_scrap(NewScrap {
        item_id: item, scrap_qty: d("3"),
        location_id: stock, scrap_location_id: None,
        lot_id: None, package_id: None, owner_id: None, picking_id: None,
        origin: None, scrap_reason_tag_ids: vec![],
    }).await.unwrap();
    assert_eq!(created.state, "draft");
    assert!(created.move_id.is_none());
    // The default scrap sink: the bootstrapped inventory-loss location.
    let loss_usage: String = sqlx::query_scalar(
        r#"SELECT usage::text FROM inventory.locations WHERE id = $1"#,
    )
    .bind(created.scrap_location_id)
    .fetch_one(&pool).await.unwrap();
    assert_eq!(loss_usage, "inventory");

    let sink = counting_sink();
    let processed = svc.process_scrap(created.id, &gl(), &*sink).await.unwrap();
    assert_eq!(processed.scrap_id, created.id);

    // The header closed done, bound to the engine move — and the move carries the scrap
    // markers (scrapped + is_inventory: the adjustment shape, never a second estate).
    let header = svc.fetch_scrap(created.id).await.unwrap().unwrap();
    assert_eq!(header.state, "done");
    assert_eq!(header.move_id, Some(processed.move_id));
    let mv: (bool, bool, String) = sqlx::query_as(
        r#"SELECT scrapped, is_inventory, state::text FROM inventory.stock_moves WHERE id = $1"#,
    )
    .bind(processed.move_id)
    .fetch_one(&pool).await.unwrap();
    assert_eq!(mv, (true, true, "done".into()));

    // Physical truth: the scrapped 3 left the source quant.
    assert_eq!(on_hand(&pool, item, stock).await, d("4"));
    // GL posted exactly once (3 × 3 at the current average, adjustment shape).
    assert_eq!(sink.posts.load(Ordering::SeqCst), 1);

    // Second process: the typed not-draft refusal.
    let err = svc.process_scrap(created.id, &gl(), &*sink).await.unwrap_err();
    assert!(matches!(err, InventoryError::ScrapNotDraft { .. }), "got {err:?}");
    assert_eq!(sink.posts.load(Ordering::SeqCst), 1);
}

/// The deferred form (the HTTP shape): the physical movement lands whole with NO GL post;
/// the move's posting stays for a service-driven repost.
#[tokio::test]
async fn scrap_deferred_lands_without_gl() {
    let pool = pool().await;
    let svc = InventoryWriteService::new(pool.clone());
    let wh = warehouse(&svc).await;
    let stock = loc(&pool, "internal", Some(wh)).await;
    let item = Uuid::new_v4();
    seed_quant(&pool, item, stock, "5").await;
    seed_bin(&pool, item, wh, "5", "2").await;

    let created = svc.create_scrap(NewScrap {
        item_id: item, scrap_qty: d("2"),
        location_id: stock, scrap_location_id: None,
        lot_id: None, package_id: None, owner_id: None, picking_id: None,
        origin: None, scrap_reason_tag_ids: vec![],
    }).await.unwrap();
    let processed = svc.process_scrap_deferred(created.id).await.unwrap();
    assert_eq!(on_hand(&pool, item, stock).await, d("3"));
    let posting: String = sqlx::query_scalar(
        r#"SELECT posting_state::text FROM inventory.stock_moves WHERE id = $1"#,
    )
    .bind(processed.move_id)
    .fetch_one(&pool).await.unwrap();
    assert_eq!(posting, "not_applicable", "no accounts on the directive → the engine built no envelope");
    let header = svc.fetch_scrap(created.id).await.unwrap().unwrap();
    assert_eq!(header.state, "done");
}

/// Mint guards: a non-positive quantity is refused loudly (the door's half of the
/// positivity CHECK) and a view location holds no stock to scrap.
#[tokio::test]
async fn scrap_mint_guards() {
    let pool = pool().await;
    let svc = InventoryWriteService::new(pool.clone());
    let wh = warehouse(&svc).await;
    let stock = loc(&pool, "internal", Some(wh)).await;
    let view = loc(&pool, "view", None).await;

    let err = svc.create_scrap(NewScrap {
        item_id: Uuid::new_v4(), scrap_qty: d("0"),
        location_id: stock, scrap_location_id: None,
        lot_id: None, package_id: None, owner_id: None, picking_id: None,
        origin: None, scrap_reason_tag_ids: vec![],
    }).await.unwrap_err();
    assert!(matches!(err, InventoryError::NegativeQuantity), "got {err:?}");

    let err = svc.create_scrap(NewScrap {
        item_id: Uuid::new_v4(), scrap_qty: d("1"),
        location_id: view, scrap_location_id: None,
        lot_id: None, package_id: None, owner_id: None, picking_id: None,
        origin: None, scrap_reason_tag_ids: vec![],
    }).await.unwrap_err();
    assert!(matches!(err, InventoryError::ViewLocationHoldsNoStock { .. }), "got {err:?}");
}

// ── master-data guards at the DB level (a raw-SQL writer cannot skip them) ────

/// R7 posture (ADR-0029): the tag-label unique's guarantee moved to the composing
/// service's decorator (the org-leading (org unit, name) re-declaration) — the module
/// ships no name unique of its own, so an undecorated module database cannot refuse a
/// duplicate. Pin that posture: duplicates are admitted undecorated and the table
/// carries no non-primary-key unique.
#[tokio::test]
async fn r7_scrap_reason_tag_names_unique() {
    let pool = pool().await;
    let name = uq("DMG");
    let ins = |name: String| {
        let pool = pool.clone();
        async move {
            sqlx::query(
                r#"INSERT INTO inventory.scrap_reason_tags (id, name)
                   VALUES ($1, $2)"#,
            )
            .bind(Uuid::new_v4()).bind(name)
            .execute(&pool).await
        }
    };
    ins(name.clone()).await.unwrap();
    ins(name).await.unwrap();
    ins(uq("DMG")).await.unwrap();
    let uniques: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pg_indexes WHERE schemaname = 'inventory' \
          AND tablename = 'scrap_reason_tags' AND indexdef ILIKE 'CREATE UNIQUE%' \
          AND indexname NOT LIKE '%\\_pkey'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        uniques, 0,
        "the module ships no scrap_reason_tags unique — the decorator owns the (org unit, name) slot"
    );
}

/// R8/R9: a storage-category capacity row is unique per target — one item row and one
/// package-type row per category (the two partial uniques), while an item row and a
/// package row for the SAME category coexist (they are disjoint families).
#[tokio::test]
async fn r8_r9_storage_capacity_uniques() {
    let pool = pool().await;
    let cat = Uuid::new_v4();
    let ins_cat = sqlx::query(
        r#"INSERT INTO inventory.storage_categories (id, name) VALUES ($1, $2)"#)
        .bind(cat).bind(uq("CAT"));
    ins_cat.execute(&pool).await.unwrap();

    let ins = |item: Option<Uuid>, pt: Option<Uuid>, qty: &str| {
        let pool = pool.clone();
        let qty = qty.to_string();
        async move {
            sqlx::query(
                r#"INSERT INTO inventory.storage_category_capacities
                     (id, storage_category_id, item_id, package_type_id, quantity)
                   VALUES ($1, $2, $3, $4, $5)"#,
            )
            .bind(Uuid::new_v4()).bind(cat).bind(item).bind(pt).bind(d(&qty))
            .execute(&pool).await
        }
    };
    let item = Uuid::new_v4();
    let pt = Uuid::new_v4();
    // The capacity's package-type target is a real FK — the package type must exist.
    sqlx::query(r#"INSERT INTO inventory.package_types (id, name) VALUES ($1, $2)"#)
        .bind(pt).bind(uq("PT")).execute(&pool).await.unwrap();
    ins(Some(item), None, "5").await.unwrap();
    assert!(ins(Some(item), None, "6").await.is_err(), "R8: duplicate item capacity per category");
    ins(None, Some(pt), "2").await.unwrap();
    assert!(ins(None, Some(pt), "3").await.is_err(), "R9: duplicate package-type capacity per category");
    // XOR (see the xor case) blocks the both-set row; a neither-set row is blocked too.
    assert!(ins(None, None, "1").await.is_err(), "a capacity row must name exactly one target");
}

/// R10: a package type's barcode is unique among live rows.
#[tokio::test]
async fn r10_package_type_barcode_unique() {
    let pool = pool().await;
    let barcode = uq("BC");
    let ins = |barcode: String| {
        let pool = pool.clone();
        async move {
            sqlx::query(
                r#"INSERT INTO inventory.package_types (id, name, barcode) VALUES ($1, $2, $3)"#,
            )
            .bind(Uuid::new_v4()).bind(uq("PT")).bind(barcode)
            .execute(&pool).await
        }
    };
    ins(barcode.clone()).await.unwrap();
    assert!(ins(barcode).await.is_err(), "duplicate barcode among live package types");
    ins(uq("BC")).await.unwrap();
}

/// R15/R16/R17: positivity and non-negativity CHECKs — quantity > 0 on capacities,
/// non-negative dimensions/weight on package types and storage categories.
#[tokio::test]
async fn r15_r16_r17_positivity_checks() {
    let pool = pool().await;
    let cat = Uuid::new_v4();
    sqlx::query(r#"INSERT INTO inventory.storage_categories (id, name) VALUES ($1, $2)"#)
        .bind(cat).bind(uq("CAT")).execute(&pool).await.unwrap();

    // R15: a capacity quantity is strictly positive.
    for bad in ["0", "-1"] {
        let res = sqlx::query(
            r#"INSERT INTO inventory.storage_category_capacities
                 (id, storage_category_id, item_id, quantity) VALUES ($1, $2, $3, $4)"#,
        )
        .bind(Uuid::new_v4()).bind(cat).bind(Uuid::new_v4()).bind(d(bad))
        .execute(&pool).await;
        assert!(res.is_err(), "R15: quantity {bad} must be refused");
    }

    // R16: package-type dimensions and max weight are non-negative when present.
    for (col, bad) in [("length", "-1"), ("width", "-0.5"), ("height", "-2"), ("max_weight", "-3")] {
        let sql = format!(
            r#"INSERT INTO inventory.package_types (id, name, {col}) VALUES ($1, $2, $3)"#);
        let res = sqlx::query(&sql)
            .bind(Uuid::new_v4()).bind(uq("PT")).bind(d(bad))
            .execute(&pool).await;
        assert!(res.is_err(), "R16: {col} {bad} must be refused");
    }

    // R17: storage-category max weight is non-negative when present.
    let res = sqlx::query(
        r#"INSERT INTO inventory.storage_categories (id, name, max_weight) VALUES ($1, $2, $3)"#,
    )
    .bind(Uuid::new_v4()).bind(uq("CAT")).bind(d("-1"))
    .execute(&pool).await;
    assert!(res.is_err(), "R17: negative max_weight must be refused");
}

/// The capacity target XOR: a row names an item OR a package type, never both, never
/// neither (proven from the both-set side; the neither-set side rides R8/R9's partial
/// uniques above).
#[tokio::test]
async fn storage_capacity_target_xor() {
    let pool = pool().await;
    let cat = Uuid::new_v4();
    sqlx::query(r#"INSERT INTO inventory.storage_categories (id, name) VALUES ($1, $2)"#)
        .bind(cat).bind(uq("CAT")).execute(&pool).await.unwrap();
    let res = sqlx::query(
        r#"INSERT INTO inventory.storage_category_capacities
             (id, storage_category_id, item_id, package_type_id, quantity)
           VALUES ($1, $2, $3, $4, $5)"#,
    )
    .bind(Uuid::new_v4()).bind(cat).bind(Uuid::new_v4()).bind(Uuid::new_v4()).bind(d("1"))
    .execute(&pool).await;
    assert!(res.is_err(), "a capacity row naming BOTH an item and a package type must be refused");
}

/// Scrap positivity at the DB level (the CHECK half — the door refuses loudly first).
#[tokio::test]
async fn scrap_positive_qty_check() {
    let pool = pool().await;
    let svc = InventoryWriteService::new(pool.clone());
    let wh = warehouse(&svc).await;
    let stock = loc(&pool, "internal", Some(wh)).await;
    let res = sqlx::query(
        r#"INSERT INTO inventory.scraps
             (id, name, item_id, scrap_qty, location_id, scrap_location_id)
           VALUES ($1, $2, $3, $4, $5, $5)"#,
    )
    .bind(Uuid::new_v4()).bind(uq("SCRAP")).bind(Uuid::new_v4()).bind(d("0")).bind(stock)
    .execute(&pool).await;
    assert!(res.is_err(), "scrap_qty must be strictly positive at the DB level");
}

/// T12: `putaway_rules.storage_category_id` is a STORED derived copy of the destination
/// location's category — written by the trigger on insert and on every destination change,
/// and re-derived even when a raw writer claims a different value (never a free column).
#[tokio::test]
async fn t12_putaway_storage_category_derivation() {
    let pool = pool().await;
    let svc = InventoryWriteService::new(pool.clone());
    let wh = warehouse(&svc).await;
    let in_loc = loc(&pool, "internal", Some(wh)).await;
    let out_a = loc(&pool, "internal", Some(wh)).await;
    let out_b = loc(&pool, "internal", Some(wh)).await;
    let cat_a = Uuid::new_v4();
    let cat_b = Uuid::new_v4();
    for (cat, name) in [(cat_a, uq("CATA")), (cat_b, uq("CATB"))] {
        sqlx::query(r#"INSERT INTO inventory.storage_categories (id, name) VALUES ($1, $2)"#)
            .bind(cat).bind(name).execute(&pool).await.unwrap();
    }
    // The two destination locations carry different categories; the input carries none.
    sqlx::query(r#"UPDATE inventory.locations SET storage_category_id = $1 WHERE id = $2"#)
        .bind(cat_a).bind(out_a).execute(&pool).await.unwrap();
    sqlx::query(r#"UPDATE inventory.locations SET storage_category_id = $1 WHERE id = $2"#)
        .bind(cat_b).bind(out_b).execute(&pool).await.unwrap();

    // Insert WITHOUT naming a category: the trigger derives it from the destination.
    let rule = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO inventory.putaway_rules (id, location_in_id, location_out_id)
           VALUES ($1, $2, $3)"#,
    )
    .bind(rule).bind(in_loc).bind(out_a)
    .execute(&pool).await.unwrap();
    let derived: Uuid = sqlx::query_scalar(
        r#"SELECT storage_category_id FROM inventory.putaway_rules WHERE id = $1"#)
        .bind(rule).fetch_one(&pool).await.unwrap();
    assert_eq!(derived, cat_a, "the trigger derives the destination's category on insert");

    // A raw writer CLAIMS a different category: the trigger re-derives on destination
    // change — the claimed value never survives.
    sqlx::query(
        r#"UPDATE inventory.putaway_rules
           SET storage_category_id = $2, location_out_id = $3 WHERE id = $1"#,
    )
    .bind(rule).bind(cat_a).bind(out_b)
    .execute(&pool).await.unwrap();
    let rederived: Uuid = sqlx::query_scalar(
        r#"SELECT storage_category_id FROM inventory.putaway_rules WHERE id = $1"#)
        .bind(rule).fetch_one(&pool).await.unwrap();
    assert_eq!(rederived, cat_b, "the trigger re-derives on destination change, ignoring the claim");
}
