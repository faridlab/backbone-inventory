//! Inventory read-models (hand-authored, user-owned) — the availability/stock-balance surface the
//! brief names as consumed by selling + buying (`docs/erp/modules/backbone-inventory.md:11,51-52`).
//!
//! These project the `Bin` running balance into the DTOs a consumer actually needs:
//!   - `AvailabilityView { item_id, warehouse_id, available_qty }` — `actual_qty − reserved_qty`,
//!     the quantity a Sales Order may commit against.
//!   - `StockBalance { item_id, warehouse_id, actual_qty, valuation_rate, stock_value }`.
//! Re-exported from `crate::exports` as the stable public read surface.

use std::sync::Arc;

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::infrastructure::persistence::BinRepository;

/// The available-to-commit view over a bin: `available_qty = actual_qty − reserved_qty`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AvailabilityView {
    pub item_id: Uuid,
    pub warehouse_id: Uuid,
    pub actual_qty: Decimal,
    pub reserved_qty: Decimal,
    pub available_qty: Decimal,
}

/// The valuation view over a bin.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StockBalance {
    pub item_id: Uuid,
    pub warehouse_id: Uuid,
    pub actual_qty: Decimal,
    pub valuation_rate: Decimal,
    pub stock_value: Decimal,
}

/// Read-only queries over inventory balances (the consumer-facing read port).
///
/// This service only projects — the SQL lives in [`BinRepository`], per the module's 4-layer rule.
#[derive(Clone)]
pub struct InventoryReadService {
    db_pool: PgPool,
    bins: Arc<BinRepository>,
}

impl InventoryReadService {
    pub fn new(db_pool: PgPool) -> Self {
        Self {
            bins: Arc::new(BinRepository::new(db_pool.clone())),
            db_pool,
        }
    }

    /// Availability for one item in one warehouse. Returns a zeroed view (available 0) when no bin
    /// exists yet — an un-received item is simply unavailable, not an error.
    pub async fn availability(&self, company_id: Uuid, item_id: Uuid, warehouse_id: Uuid) -> Result<AvailabilityView, sqlx::Error> {
        let row = self.bins.fetch_availability(&self.db_pool, company_id, item_id, warehouse_id).await?;
        let (actual, reserved) = match row {
            Some(r) => (r.actual_qty, r.reserved_qty),
            None => (Decimal::ZERO, Decimal::ZERO),
        };
        Ok(AvailabilityView { item_id, warehouse_id, actual_qty: actual, reserved_qty: reserved, available_qty: actual - reserved })
    }

    /// Availability for one item across every warehouse of the company that holds a bin for it.
    pub async fn availability_across_warehouses(&self, company_id: Uuid, item_id: Uuid) -> Result<Vec<AvailabilityView>, sqlx::Error> {
        let rows = self.bins.fetch_availability_across_warehouses(&self.db_pool, company_id, item_id).await?;
        Ok(rows.into_iter().map(|r| {
            let actual = r.actual_qty;
            let reserved = r.reserved_qty;
            AvailabilityView { item_id, warehouse_id: r.warehouse_id, actual_qty: actual, reserved_qty: reserved, available_qty: actual - reserved }
        }).collect())
    }

    /// Valuation balance for one item in one warehouse (None if no bin exists).
    pub async fn stock_balance(&self, company_id: Uuid, item_id: Uuid, warehouse_id: Uuid) -> Result<Option<StockBalance>, sqlx::Error> {
        let row = self.bins.fetch_balance(&self.db_pool, company_id, item_id, warehouse_id).await?;
        Ok(row.map(|r| StockBalance {
            item_id, warehouse_id,
            actual_qty: r.actual_qty, valuation_rate: r.valuation_rate, stock_value: r.stock_value,
        }))
    }
}
