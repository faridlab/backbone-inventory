use chrono::{DateTime, Utc, NaiveDate};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use rust_decimal::Decimal;

use super::VoucherType;
use super::AuditMetadata;

/// Strongly-typed ID for StockLedgerEntry
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StockLedgerEntryId(pub Uuid);

impl StockLedgerEntryId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for StockLedgerEntryId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for StockLedgerEntryId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for StockLedgerEntryId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<StockLedgerEntryId> for Uuid {
    fn from(id: StockLedgerEntryId) -> Self { id.0 }
}

impl AsRef<Uuid> for StockLedgerEntryId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for StockLedgerEntryId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct StockLedgerEntry {
    pub id: Uuid,
    pub company_id: Uuid,
    pub item_id: Uuid,
    pub warehouse_id: Uuid,
    pub posting_date: NaiveDate,
    pub actual_qty: Decimal,
    pub qty_after_txn: Decimal,
    pub incoming_rate: Decimal,
    pub valuation_rate: Decimal,
    pub stock_value: Decimal,
    pub stock_value_difference: Decimal,
    pub voucher_type: VoucherType,
    pub voucher_id: Uuid,
    pub voucher_no: String,
    pub sle_no: i32,
    pub is_cancelled: bool,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl StockLedgerEntry {
    /// Create a builder for StockLedgerEntry
    pub fn builder() -> StockLedgerEntryBuilder {
        StockLedgerEntryBuilder::default()
    }

    /// Create a new StockLedgerEntry with required fields
    pub fn new(company_id: Uuid, item_id: Uuid, warehouse_id: Uuid, posting_date: NaiveDate, actual_qty: Decimal, qty_after_txn: Decimal, incoming_rate: Decimal, valuation_rate: Decimal, stock_value: Decimal, stock_value_difference: Decimal, voucher_type: VoucherType, voucher_id: Uuid, voucher_no: String, sle_no: i32, is_cancelled: bool) -> Self {
        Self {
            id: Uuid::new_v4(),
            company_id,
            item_id,
            warehouse_id,
            posting_date,
            actual_qty,
            qty_after_txn,
            incoming_rate,
            valuation_rate,
            stock_value,
            stock_value_difference,
            voucher_type,
            voucher_id,
            voucher_no,
            sle_no,
            is_cancelled,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> StockLedgerEntryId {
        StockLedgerEntryId(self.id)
    }

    /// Get when this entity was created
    pub fn created_at(&self) -> Option<&DateTime<Utc>> {
        self.metadata.created_at.as_ref()
    }

    /// Get when this entity was last updated
    pub fn updated_at(&self) -> Option<&DateTime<Utc>> {
        self.metadata.updated_at.as_ref()
    }

    /// Check if this entity is soft deleted
    pub fn is_deleted(&self) -> bool {
        self.metadata.deleted_at.is_some()
    }

    /// Check if this entity is active (not deleted)
    pub fn is_active(&self) -> bool {
        self.metadata.deleted_at.is_none()
    }

    /// Get when this entity was deleted
    pub fn deleted_at(&self) -> Option<&DateTime<Utc>> {
        self.metadata.deleted_at.as_ref()
    }

    /// Get who created this entity
    pub fn created_by(&self) -> Option<&Uuid> {
        self.metadata.created_by.as_ref()
    }

    /// Get who last updated this entity
    pub fn updated_by(&self) -> Option<&Uuid> {
        self.metadata.updated_by.as_ref()
    }

    /// Get who deleted this entity
    pub fn deleted_by(&self) -> Option<&Uuid> {
        self.metadata.deleted_by.as_ref()
    }


    // ==========================================================
    // Partial Update
    // ==========================================================

    /// Apply partial updates from a map of field name to JSON value
    pub fn apply_patch(&mut self, fields: std::collections::HashMap<String, serde_json::Value>) {
        for (key, value) in fields {
            match key.as_str() {
                "company_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.company_id = v; }
                }
                "item_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.item_id = v; }
                }
                "warehouse_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.warehouse_id = v; }
                }
                "posting_date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.posting_date = v; }
                }
                "actual_qty" => {
                    if let Ok(v) = serde_json::from_value(value) { self.actual_qty = v; }
                }
                "qty_after_txn" => {
                    if let Ok(v) = serde_json::from_value(value) { self.qty_after_txn = v; }
                }
                "incoming_rate" => {
                    if let Ok(v) = serde_json::from_value(value) { self.incoming_rate = v; }
                }
                "valuation_rate" => {
                    if let Ok(v) = serde_json::from_value(value) { self.valuation_rate = v; }
                }
                "stock_value" => {
                    if let Ok(v) = serde_json::from_value(value) { self.stock_value = v; }
                }
                "stock_value_difference" => {
                    if let Ok(v) = serde_json::from_value(value) { self.stock_value_difference = v; }
                }
                "voucher_type" => {
                    if let Ok(v) = serde_json::from_value(value) { self.voucher_type = v; }
                }
                "voucher_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.voucher_id = v; }
                }
                "voucher_no" => {
                    if let Ok(v) = serde_json::from_value(value) { self.voucher_no = v; }
                }
                "sle_no" => {
                    if let Ok(v) = serde_json::from_value(value) { self.sle_no = v; }
                }
                "is_cancelled" => {
                    if let Ok(v) = serde_json::from_value(value) { self.is_cancelled = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for StockLedgerEntry {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "StockLedgerEntry"
    }
}

impl backbone_core::PersistentEntity for StockLedgerEntry {
    fn entity_id(&self) -> String {
        self.id.to_string()
    }
    fn set_entity_id(&mut self, id: String) {
        if let Ok(uuid) = uuid::Uuid::parse_str(&id) {
            self.id = uuid;
        }
    }
    fn created_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.metadata.created_at
    }
    fn set_created_at(&mut self, ts: chrono::DateTime<chrono::Utc>) {
        self.metadata.created_at = Some(ts);
    }
    fn updated_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.metadata.updated_at
    }
    fn set_updated_at(&mut self, ts: chrono::DateTime<chrono::Utc>) {
        self.metadata.updated_at = Some(ts);
    }
    fn deleted_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.metadata.deleted_at
    }
    fn set_deleted_at(&mut self, ts: Option<chrono::DateTime<chrono::Utc>>) {
        self.metadata.deleted_at = ts;
    }
}

impl backbone_orm::EntityRepoMeta for StockLedgerEntry {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("company_id".to_string(), "uuid".to_string());
        m.insert("item_id".to_string(), "uuid".to_string());
        m.insert("warehouse_id".to_string(), "uuid".to_string());
        m.insert("voucher_id".to_string(), "uuid".to_string());
        m.insert("voucher_type".to_string(), "voucher_type".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["voucher_no"]
    }
    fn company_field() -> Option<&'static str> {
        Some("company_id")
    }
}

/// Builder for StockLedgerEntry entity
///
/// Provides a fluent API for constructing StockLedgerEntry instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct StockLedgerEntryBuilder {
    company_id: Option<Uuid>,
    item_id: Option<Uuid>,
    warehouse_id: Option<Uuid>,
    posting_date: Option<NaiveDate>,
    actual_qty: Option<Decimal>,
    qty_after_txn: Option<Decimal>,
    incoming_rate: Option<Decimal>,
    valuation_rate: Option<Decimal>,
    stock_value: Option<Decimal>,
    stock_value_difference: Option<Decimal>,
    voucher_type: Option<VoucherType>,
    voucher_id: Option<Uuid>,
    voucher_no: Option<String>,
    sle_no: Option<i32>,
    is_cancelled: Option<bool>,
}

impl StockLedgerEntryBuilder {
    /// Set the company_id field (required)
    pub fn company_id(mut self, value: Uuid) -> Self {
        self.company_id = Some(value);
        self
    }

    /// Set the item_id field (required)
    pub fn item_id(mut self, value: Uuid) -> Self {
        self.item_id = Some(value);
        self
    }

    /// Set the warehouse_id field (required)
    pub fn warehouse_id(mut self, value: Uuid) -> Self {
        self.warehouse_id = Some(value);
        self
    }

    /// Set the posting_date field (required)
    pub fn posting_date(mut self, value: NaiveDate) -> Self {
        self.posting_date = Some(value);
        self
    }

    /// Set the actual_qty field (required)
    pub fn actual_qty(mut self, value: Decimal) -> Self {
        self.actual_qty = Some(value);
        self
    }

    /// Set the qty_after_txn field (required)
    pub fn qty_after_txn(mut self, value: Decimal) -> Self {
        self.qty_after_txn = Some(value);
        self
    }

    /// Set the incoming_rate field (default: `Decimal::from(0)`)
    pub fn incoming_rate(mut self, value: Decimal) -> Self {
        self.incoming_rate = Some(value);
        self
    }

    /// Set the valuation_rate field (default: `Decimal::from(0)`)
    pub fn valuation_rate(mut self, value: Decimal) -> Self {
        self.valuation_rate = Some(value);
        self
    }

    /// Set the stock_value field (default: `Decimal::from(0)`)
    pub fn stock_value(mut self, value: Decimal) -> Self {
        self.stock_value = Some(value);
        self
    }

    /// Set the stock_value_difference field (required)
    pub fn stock_value_difference(mut self, value: Decimal) -> Self {
        self.stock_value_difference = Some(value);
        self
    }

    /// Set the voucher_type field (required)
    pub fn voucher_type(mut self, value: VoucherType) -> Self {
        self.voucher_type = Some(value);
        self
    }

    /// Set the voucher_id field (required)
    pub fn voucher_id(mut self, value: Uuid) -> Self {
        self.voucher_id = Some(value);
        self
    }

    /// Set the voucher_no field (required)
    pub fn voucher_no(mut self, value: String) -> Self {
        self.voucher_no = Some(value);
        self
    }

    /// Set the sle_no field (required)
    pub fn sle_no(mut self, value: i32) -> Self {
        self.sle_no = Some(value);
        self
    }

    /// Set the is_cancelled field (default: `false`)
    pub fn is_cancelled(mut self, value: bool) -> Self {
        self.is_cancelled = Some(value);
        self
    }

    /// Build the StockLedgerEntry entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<StockLedgerEntry, String> {
        let company_id = self.company_id.ok_or_else(|| "company_id is required".to_string())?;
        let item_id = self.item_id.ok_or_else(|| "item_id is required".to_string())?;
        let warehouse_id = self.warehouse_id.ok_or_else(|| "warehouse_id is required".to_string())?;
        let posting_date = self.posting_date.ok_or_else(|| "posting_date is required".to_string())?;
        let actual_qty = self.actual_qty.ok_or_else(|| "actual_qty is required".to_string())?;
        let qty_after_txn = self.qty_after_txn.ok_or_else(|| "qty_after_txn is required".to_string())?;
        let stock_value_difference = self.stock_value_difference.ok_or_else(|| "stock_value_difference is required".to_string())?;
        let voucher_type = self.voucher_type.ok_or_else(|| "voucher_type is required".to_string())?;
        let voucher_id = self.voucher_id.ok_or_else(|| "voucher_id is required".to_string())?;
        let voucher_no = self.voucher_no.ok_or_else(|| "voucher_no is required".to_string())?;
        let sle_no = self.sle_no.ok_or_else(|| "sle_no is required".to_string())?;

        Ok(StockLedgerEntry {
            id: Uuid::new_v4(),
            company_id,
            item_id,
            warehouse_id,
            posting_date,
            actual_qty,
            qty_after_txn,
            incoming_rate: self.incoming_rate.unwrap_or(Decimal::from(0)),
            valuation_rate: self.valuation_rate.unwrap_or(Decimal::from(0)),
            stock_value: self.stock_value.unwrap_or(Decimal::from(0)),
            stock_value_difference,
            voucher_type,
            voucher_id,
            voucher_no,
            sle_no,
            is_cancelled: self.is_cancelled.unwrap_or(false),
            metadata: AuditMetadata::default(),
        })
    }
}
