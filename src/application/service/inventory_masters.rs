//! Warehouse + stock-item masters (hand-authored, user-owned).
//!
//! An `impl InventoryWriteService` chunk over the vocabulary in [`super::inventory_write_service`]:
//! the two master-data creators. Both are simple tenant-scoped inserts — no SLE, no Bin, no GL.
//!
//! Per the module's 4-layer rule this file holds no SQL — the statements live on
//! `WarehouseRepository` / `StockItemRepository`.

use backbone_orm::company_scope;
use uuid::Uuid;

use crate::infrastructure::persistence::{NewStockItemRow, NewWarehouseRow};

use super::inventory_write_service::{is_dup, InventoryError, InventoryWriteService, NewStockItem, NewWarehouse};

impl InventoryWriteService {
    // ---- masters ------------------------------------------------------------

    pub async fn create_warehouse(&self, w: NewWarehouse) -> Result<Uuid, InventoryError> {
        let id = Uuid::new_v4();
        let wt = w.warehouse_type.unwrap_or_else(|| "stock".into());
        // RLS scope (ADR-0008): company on the DTO.
        let r = company_scope::with_company_scope(
            Some(w.company_id),
            self.warehouses.insert_warehouse(&self.db_pool, &NewWarehouseRow {
                id,
                company_id: w.company_id,
                code: &w.code,
                name: &w.name,
                warehouse_type: &wt,
                parent_warehouse_id: w.parent_warehouse_id,
                is_group: w.is_group,
            }),
        ).await;
        match r {
            Ok(_) => Ok(id),
            Err(e) if is_dup(&e) => Err(InventoryError::DuplicateNumber(w.code)),
            Err(e) => Err(e.into()),
        }
    }

    pub async fn create_stock_item(&self, s: NewStockItem) -> Result<Uuid, InventoryError> {
        let id = Uuid::new_v4();
        let vm = s.valuation_method.unwrap_or_else(|| "moving_average".into());
        // RLS scope (ADR-0008): company on the DTO.
        let r = company_scope::with_company_scope(
            Some(s.company_id),
            self.stock_items.insert_stock_item(&self.db_pool, &NewStockItemRow {
                id,
                item_id: s.item_id,
                company_id: s.company_id,
                stock_uom: &s.stock_uom,
                valuation_method: &vm,
                reorder_level: s.reorder_level,
            }),
        ).await;
        match r {
            Ok(_) => Ok(id),
            Err(e) if is_dup(&e) => Err(InventoryError::DuplicateNumber(s.item_id.to_string())),
            Err(e) => Err(e.into()),
        }
    }
}
