//! Service-tracking column probes (the service-delivery policy on the product surface):
//! the four-rung ladder + its two project anchors on `inventory.stock_items`.
//!
//! Coverage:
//! - DEFAULT manual: an item registered through the module's own write path (which does
//!   not name the columns) reads back `manual` with NULL anchors — the safe posture that
//!   keeps existing service items minting nothing until a rung is deliberately configured.
//! - Round-trip: every rung and both anchor columns persist and read back through the
//!   `StockItem` entity mapping (proves the `service_tracking_type` sqlx enum binding,
//!   not just raw SQL text).
//! - DB-level validation: an unknown rung label is rejected by the enum type.
//!
//! Rows are keyed by `item_id` alone: org scoping is the composing service's decorator
//! posture, so the bare module has no company axis to fence on.
//!
//! Requires DATABASE_URL pointing at a migrated database (default :5433/backbone_inventory).

use sqlx::PgPool;
use uuid::Uuid;

use backbone_inventory::application::service::inventory_write_service::{
    InventoryWriteService, NewStockItem,
};
use backbone_inventory::domain::entity::{ServiceTrackingType, StockItem};

fn pool_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:postgres@localhost:5433/backbone_inventory".to_string())
}

async fn pool() -> PgPool {
    PgPool::connect(&pool_url()).await.expect("connect DB")
}

/// Register an item through the module's write path (the columns are not named by the
/// INSERT, so the DB defaults are what land). Returns the item_id.
async fn register_item(w: &InventoryWriteService) -> Uuid {
    let item = Uuid::new_v4();
    w.create_stock_item(NewStockItem {
        item_id: item,
        stock_uom: "unit".into(),
        valuation_method: None,
        reorder_level: rust_decimal::Decimal::ZERO,
    })
    .await
    .expect("register stock item");
    item
}

/// Read the row back through the entity mapping (exercises the FromRow impl incl. the
/// `service_tracking_type` enum binding).
async fn fetch_entity(pool: &PgPool, item: Uuid) -> StockItem {
    sqlx::query_as::<_, StockItem>(
        "SELECT * FROM inventory.stock_items WHERE item_id = $1",
    )
    .bind(item)
    .fetch_one(pool)
    .await
    .expect("fetch stock item")
}

#[tokio::test]
async fn service_tracking_defaults_to_manual() {
    let pool = pool().await;
    let w = InventoryWriteService::new(pool.clone());
    let item = register_item(&w).await;

    let row = fetch_entity(&pool, item).await;
    assert_eq!(row.service_tracking, ServiceTrackingType::Manual);
    assert_eq!(row.service_project_id, None);
    assert_eq!(row.service_project_template_id, None);

    // The default also holds at the raw column level, not just through the mapping.
    let raw: String = sqlx::query_scalar(
        "SELECT service_tracking::text FROM inventory.stock_items WHERE item_id = $1",
    )
    .bind(item)
    .fetch_one(&pool)
    .await
    .expect("raw read");
    assert_eq!(raw, "manual");
}

#[tokio::test]
async fn service_tracking_round_trips_every_rung_and_anchor() {
    let pool = pool().await;
    let w = InventoryWriteService::new(pool.clone());
    let item = register_item(&w).await;

    let project = Uuid::new_v4();
    let template = Uuid::new_v4();
    for rung in [
        ServiceTrackingType::TaskGlobalProject,
        ServiceTrackingType::TaskInProject,
        ServiceTrackingType::ProjectOnly,
        ServiceTrackingType::Manual,
    ] {
        sqlx::query(
            r#"UPDATE inventory.stock_items
               SET service_tracking = $2::service_tracking_type,
                   service_project_id = $3,
                   service_project_template_id = $4
               WHERE item_id = $1"#,
        )
        .bind(item)
        .bind(rung.to_string())
        .bind(project)
        .bind(template)
        .execute(&pool)
        .await
        .expect("update rung");

        let row = fetch_entity(&pool, item).await;
        assert_eq!(row.service_tracking, rung, "rung {rung} must round-trip");
        assert_eq!(row.service_project_id, Some(project));
        assert_eq!(row.service_project_template_id, Some(template));
    }

    // Clearing the anchors round-trips too (back to the unconfigured shape).
    sqlx::query(
        r#"UPDATE inventory.stock_items
           SET service_project_id = NULL, service_project_template_id = NULL
           WHERE item_id = $1"#,
    )
    .bind(item)
    .execute(&pool)
    .await
    .expect("clear anchors");
    let row = fetch_entity(&pool, item).await;
    assert_eq!(row.service_project_id, None);
    assert_eq!(row.service_project_template_id, None);
}

#[tokio::test]
async fn service_tracking_rejects_unknown_rung() {
    let pool = pool().await;
    let w = InventoryWriteService::new(pool.clone());
    let item = register_item(&w).await;

    let err = sqlx::query(
        r#"UPDATE inventory.stock_items
           SET service_tracking = $2::service_tracking_type
           WHERE item_id = $1"#,
    )
    .bind(item)
    .bind("perpetual_inventory")
    .execute(&pool)
    .await
    .expect_err("unknown rung must be rejected");

    let db_err = err
        .as_database_error()
        .expect("database error");
    assert_eq!(
        db_err.code().unwrap().as_ref(),
        "22P02",
        "invalid enum label must raise invalid_text_representation, got: {db_err}"
    );

    // The row keeps its default rung after the refused write.
    let row = fetch_entity(&pool, item).await;
    assert_eq!(row.service_tracking, ServiceTrackingType::Manual);
}
