//! Warehouse + stock-item masters (hand-authored, user-owned).
//!
//! An `impl InventoryWriteService` chunk over the vocabulary in [`super::inventory_write_service`]:
//! the two master-data creators. Both are simple inserts under the ambient org scope —
//! no SLE, no Bin, no GL.
//!
//! Tenancy (ADR-0029): the module is tenant-agnostic. The inserts ride the ambient org scope
//! the composing service set per request (the repositories run `org_scope::execute_scoped`);
//! the composing decorator owns isolation.
//!
//! Per the module's 4-layer rule this file holds no SQL — the statements live on
//! `WarehouseRepository` / `StockItemRepository`.

use uuid::Uuid;

use crate::infrastructure::persistence::{NewStockItemRow, NewWarehouseRow};

use super::inventory_write_service::{is_dup, InventoryError, InventoryWriteService, NewStockItem, NewWarehouse};

impl InventoryWriteService {
    // ---- masters ------------------------------------------------------------

    pub async fn create_warehouse(&self, w: NewWarehouse) -> Result<Uuid, InventoryError> {
        let id = Uuid::new_v4();
        let wt = w.warehouse_type.unwrap_or_else(|| "stock".into());
        let r = self.warehouses.insert_warehouse(&self.db_pool, &NewWarehouseRow {
            id,
            code: &w.code,
            name: &w.name,
            warehouse_type: &wt,
            parent_warehouse_id: w.parent_warehouse_id,
            is_group: w.is_group,
        }).await;
        match r {
            Ok(_) => Ok(id),
            Err(e) if is_dup(&e) => Err(InventoryError::DuplicateNumber(w.code)),
            Err(e) => Err(e.into()),
        }
    }

    pub async fn create_stock_item(&self, s: NewStockItem) -> Result<Uuid, InventoryError> {
        let id = Uuid::new_v4();
        let vm = s.valuation_method.unwrap_or_else(|| "moving_average".into());
        let r = self.stock_items.insert_stock_item(&self.db_pool, &NewStockItemRow {
            id,
            item_id: s.item_id,
            stock_uom: &s.stock_uom,
            valuation_method: &vm,
            reorder_level: s.reorder_level,
        }).await;
        match r {
            Ok(_) => Ok(id),
            Err(e) if is_dup(&e) => Err(InventoryError::DuplicateNumber(s.item_id.to_string())),
            Err(e) => Err(e.into()),
        }
    }
}
