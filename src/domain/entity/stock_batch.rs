use chrono::{DateTime, Utc, NaiveDate};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use rust_decimal::Decimal;

use super::BatchStatus;
use super::AuditMetadata;

/// Strongly-typed ID for StockBatch
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StockBatchId(pub Uuid);

impl StockBatchId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for StockBatchId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for StockBatchId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for StockBatchId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<StockBatchId> for Uuid {
    fn from(id: StockBatchId) -> Self { id.0 }
}

impl AsRef<Uuid> for StockBatchId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for StockBatchId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct StockBatch {
    pub id: Uuid,
    pub provider_id: Uuid,
    pub product_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warehouse_id: Option<Uuid>,
    pub batch_number: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supplier_batch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manufacture_date: Option<NaiveDate>,
    pub received_date: NaiveDate,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiry_date: Option<NaiveDate>,
    pub initial_quantity: Decimal,
    pub current_quantity: Decimal,
    pub reserved_quantity: Decimal,
    pub quarantine_quantity: Decimal,
    pub uom: String,
    pub unit_cost: Decimal,
    pub total_value: Decimal,
    pub currency: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_document: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub goods_receipt_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supplier_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bin_location: Option<String>,
    pub storage_conditions: serde_json::Value,
    pub status: BatchStatus,
    pub is_active: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl StockBatch {
    /// Create a builder for StockBatch
    pub fn builder() -> StockBatchBuilder {
        StockBatchBuilder::default()
    }

    /// Create a new StockBatch with required fields
    pub fn new(provider_id: Uuid, product_id: Uuid, batch_number: String, received_date: NaiveDate, initial_quantity: Decimal, current_quantity: Decimal, reserved_quantity: Decimal, quarantine_quantity: Decimal, uom: String, unit_cost: Decimal, total_value: Decimal, currency: String, storage_conditions: serde_json::Value, status: BatchStatus, is_active: bool) -> Self {
        Self {
            id: Uuid::new_v4(),
            provider_id,
            product_id,
            warehouse_id: None,
            batch_number,
            supplier_batch: None,
            manufacture_date: None,
            received_date,
            expiry_date: None,
            initial_quantity,
            current_quantity,
            reserved_quantity,
            quarantine_quantity,
            uom,
            unit_cost,
            total_value,
            currency,
            source_type: None,
            source_document: None,
            goods_receipt_id: None,
            supplier_id: None,
            bin_location: None,
            storage_conditions,
            status,
            is_active,
            notes: None,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> StockBatchId {
        StockBatchId(self.id)
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

    /// Get the current status
    pub fn status(&self) -> &BatchStatus {
        &self.status
    }


    // ==========================================================
    // Fluent Setters (with_* for optional fields)
    // ==========================================================

    /// Set the warehouse_id field (chainable)
    pub fn with_warehouse_id(mut self, value: Uuid) -> Self {
        self.warehouse_id = Some(value);
        self
    }

    /// Set the supplier_batch field (chainable)
    pub fn with_supplier_batch(mut self, value: String) -> Self {
        self.supplier_batch = Some(value);
        self
    }

    /// Set the manufacture_date field (chainable)
    pub fn with_manufacture_date(mut self, value: NaiveDate) -> Self {
        self.manufacture_date = Some(value);
        self
    }

    /// Set the expiry_date field (chainable)
    pub fn with_expiry_date(mut self, value: NaiveDate) -> Self {
        self.expiry_date = Some(value);
        self
    }

    /// Set the source_type field (chainable)
    pub fn with_source_type(mut self, value: String) -> Self {
        self.source_type = Some(value);
        self
    }

    /// Set the source_document field (chainable)
    pub fn with_source_document(mut self, value: String) -> Self {
        self.source_document = Some(value);
        self
    }

    /// Set the goods_receipt_id field (chainable)
    pub fn with_goods_receipt_id(mut self, value: Uuid) -> Self {
        self.goods_receipt_id = Some(value);
        self
    }

    /// Set the supplier_id field (chainable)
    pub fn with_supplier_id(mut self, value: Uuid) -> Self {
        self.supplier_id = Some(value);
        self
    }

    /// Set the bin_location field (chainable)
    pub fn with_bin_location(mut self, value: String) -> Self {
        self.bin_location = Some(value);
        self
    }

    /// Set the notes field (chainable)
    pub fn with_notes(mut self, value: String) -> Self {
        self.notes = Some(value);
        self
    }

    // ==========================================================
    // Partial Update
    // ==========================================================

    /// Apply partial updates from a map of field name to JSON value
    pub fn apply_patch(&mut self, fields: std::collections::HashMap<String, serde_json::Value>) {
        for (key, value) in fields {
            match key.as_str() {
                "provider_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.provider_id = v; }
                }
                "product_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.product_id = v; }
                }
                "warehouse_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.warehouse_id = v; }
                }
                "batch_number" => {
                    if let Ok(v) = serde_json::from_value(value) { self.batch_number = v; }
                }
                "supplier_batch" => {
                    if let Ok(v) = serde_json::from_value(value) { self.supplier_batch = v; }
                }
                "manufacture_date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.manufacture_date = v; }
                }
                "received_date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.received_date = v; }
                }
                "expiry_date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.expiry_date = v; }
                }
                "initial_quantity" => {
                    if let Ok(v) = serde_json::from_value(value) { self.initial_quantity = v; }
                }
                "current_quantity" => {
                    if let Ok(v) = serde_json::from_value(value) { self.current_quantity = v; }
                }
                "reserved_quantity" => {
                    if let Ok(v) = serde_json::from_value(value) { self.reserved_quantity = v; }
                }
                "quarantine_quantity" => {
                    if let Ok(v) = serde_json::from_value(value) { self.quarantine_quantity = v; }
                }
                "uom" => {
                    if let Ok(v) = serde_json::from_value(value) { self.uom = v; }
                }
                "unit_cost" => {
                    if let Ok(v) = serde_json::from_value(value) { self.unit_cost = v; }
                }
                "total_value" => {
                    if let Ok(v) = serde_json::from_value(value) { self.total_value = v; }
                }
                "currency" => {
                    if let Ok(v) = serde_json::from_value(value) { self.currency = v; }
                }
                "source_type" => {
                    if let Ok(v) = serde_json::from_value(value) { self.source_type = v; }
                }
                "source_document" => {
                    if let Ok(v) = serde_json::from_value(value) { self.source_document = v; }
                }
                "goods_receipt_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.goods_receipt_id = v; }
                }
                "supplier_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.supplier_id = v; }
                }
                "bin_location" => {
                    if let Ok(v) = serde_json::from_value(value) { self.bin_location = v; }
                }
                "storage_conditions" => {
                    if let Ok(v) = serde_json::from_value(value) { self.storage_conditions = v; }
                }
                "status" => {
                    if let Ok(v) = serde_json::from_value(value) { self.status = v; }
                }
                "is_active" => {
                    if let Ok(v) = serde_json::from_value(value) { self.is_active = v; }
                }
                "notes" => {
                    if let Ok(v) = serde_json::from_value(value) { self.notes = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for StockBatch {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "StockBatch"
    }
}

impl backbone_core::PersistentEntity for StockBatch {
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

impl backbone_orm::EntityRepoMeta for StockBatch {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("provider_id".to_string(), "uuid".to_string());
        m.insert("product_id".to_string(), "uuid".to_string());
        m.insert("warehouse_id".to_string(), "uuid".to_string());
        m.insert("goods_receipt_id".to_string(), "uuid".to_string());
        m.insert("supplier_id".to_string(), "uuid".to_string());
        m.insert("status".to_string(), "batch_status".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["batch_number", "uom", "currency"]
    }
}

/// Builder for StockBatch entity
///
/// Provides a fluent API for constructing StockBatch instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct StockBatchBuilder {
    provider_id: Option<Uuid>,
    product_id: Option<Uuid>,
    warehouse_id: Option<Uuid>,
    batch_number: Option<String>,
    supplier_batch: Option<String>,
    manufacture_date: Option<NaiveDate>,
    received_date: Option<NaiveDate>,
    expiry_date: Option<NaiveDate>,
    initial_quantity: Option<Decimal>,
    current_quantity: Option<Decimal>,
    reserved_quantity: Option<Decimal>,
    quarantine_quantity: Option<Decimal>,
    uom: Option<String>,
    unit_cost: Option<Decimal>,
    total_value: Option<Decimal>,
    currency: Option<String>,
    source_type: Option<String>,
    source_document: Option<String>,
    goods_receipt_id: Option<Uuid>,
    supplier_id: Option<Uuid>,
    bin_location: Option<String>,
    storage_conditions: Option<serde_json::Value>,
    status: Option<BatchStatus>,
    is_active: Option<bool>,
    notes: Option<String>,
}

impl StockBatchBuilder {
    /// Set the provider_id field (required)
    pub fn provider_id(mut self, value: Uuid) -> Self {
        self.provider_id = Some(value);
        self
    }

    /// Set the product_id field (required)
    pub fn product_id(mut self, value: Uuid) -> Self {
        self.product_id = Some(value);
        self
    }

    /// Set the warehouse_id field (optional)
    pub fn warehouse_id(mut self, value: Uuid) -> Self {
        self.warehouse_id = Some(value);
        self
    }

    /// Set the batch_number field (required)
    pub fn batch_number(mut self, value: String) -> Self {
        self.batch_number = Some(value);
        self
    }

    /// Set the supplier_batch field (optional)
    pub fn supplier_batch(mut self, value: String) -> Self {
        self.supplier_batch = Some(value);
        self
    }

    /// Set the manufacture_date field (optional)
    pub fn manufacture_date(mut self, value: NaiveDate) -> Self {
        self.manufacture_date = Some(value);
        self
    }

    /// Set the received_date field (required)
    pub fn received_date(mut self, value: NaiveDate) -> Self {
        self.received_date = Some(value);
        self
    }

    /// Set the expiry_date field (optional)
    pub fn expiry_date(mut self, value: NaiveDate) -> Self {
        self.expiry_date = Some(value);
        self
    }

    /// Set the initial_quantity field (required)
    pub fn initial_quantity(mut self, value: Decimal) -> Self {
        self.initial_quantity = Some(value);
        self
    }

    /// Set the current_quantity field (required)
    pub fn current_quantity(mut self, value: Decimal) -> Self {
        self.current_quantity = Some(value);
        self
    }

    /// Set the reserved_quantity field (default: `Decimal::from(0)`)
    pub fn reserved_quantity(mut self, value: Decimal) -> Self {
        self.reserved_quantity = Some(value);
        self
    }

    /// Set the quarantine_quantity field (default: `Decimal::from(0)`)
    pub fn quarantine_quantity(mut self, value: Decimal) -> Self {
        self.quarantine_quantity = Some(value);
        self
    }

    /// Set the uom field (required)
    pub fn uom(mut self, value: String) -> Self {
        self.uom = Some(value);
        self
    }

    /// Set the unit_cost field (required)
    pub fn unit_cost(mut self, value: Decimal) -> Self {
        self.unit_cost = Some(value);
        self
    }

    /// Set the total_value field (required)
    pub fn total_value(mut self, value: Decimal) -> Self {
        self.total_value = Some(value);
        self
    }

    /// Set the currency field (default: `"IDR".to_string()`)
    pub fn currency(mut self, value: String) -> Self {
        self.currency = Some(value);
        self
    }

    /// Set the source_type field (optional)
    pub fn source_type(mut self, value: String) -> Self {
        self.source_type = Some(value);
        self
    }

    /// Set the source_document field (optional)
    pub fn source_document(mut self, value: String) -> Self {
        self.source_document = Some(value);
        self
    }

    /// Set the goods_receipt_id field (optional)
    pub fn goods_receipt_id(mut self, value: Uuid) -> Self {
        self.goods_receipt_id = Some(value);
        self
    }

    /// Set the supplier_id field (optional)
    pub fn supplier_id(mut self, value: Uuid) -> Self {
        self.supplier_id = Some(value);
        self
    }

    /// Set the bin_location field (optional)
    pub fn bin_location(mut self, value: String) -> Self {
        self.bin_location = Some(value);
        self
    }

    /// Set the storage_conditions field (default: `serde_json::json!({})`)
    pub fn storage_conditions(mut self, value: serde_json::Value) -> Self {
        self.storage_conditions = Some(value);
        self
    }

    /// Set the status field (default: `BatchStatus::default()`)
    pub fn status(mut self, value: BatchStatus) -> Self {
        self.status = Some(value);
        self
    }

    /// Set the is_active field (default: `true`)
    pub fn is_active(mut self, value: bool) -> Self {
        self.is_active = Some(value);
        self
    }

    /// Set the notes field (optional)
    pub fn notes(mut self, value: String) -> Self {
        self.notes = Some(value);
        self
    }

    /// Build the StockBatch entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<StockBatch, String> {
        let provider_id = self.provider_id.ok_or_else(|| "provider_id is required".to_string())?;
        let product_id = self.product_id.ok_or_else(|| "product_id is required".to_string())?;
        let batch_number = self.batch_number.ok_or_else(|| "batch_number is required".to_string())?;
        let received_date = self.received_date.ok_or_else(|| "received_date is required".to_string())?;
        let initial_quantity = self.initial_quantity.ok_or_else(|| "initial_quantity is required".to_string())?;
        let current_quantity = self.current_quantity.ok_or_else(|| "current_quantity is required".to_string())?;
        let uom = self.uom.ok_or_else(|| "uom is required".to_string())?;
        let unit_cost = self.unit_cost.ok_or_else(|| "unit_cost is required".to_string())?;
        let total_value = self.total_value.ok_or_else(|| "total_value is required".to_string())?;

        Ok(StockBatch {
            id: Uuid::new_v4(),
            provider_id,
            product_id,
            warehouse_id: self.warehouse_id,
            batch_number,
            supplier_batch: self.supplier_batch,
            manufacture_date: self.manufacture_date,
            received_date,
            expiry_date: self.expiry_date,
            initial_quantity,
            current_quantity,
            reserved_quantity: self.reserved_quantity.unwrap_or(Decimal::from(0)),
            quarantine_quantity: self.quarantine_quantity.unwrap_or(Decimal::from(0)),
            uom,
            unit_cost,
            total_value,
            currency: self.currency.unwrap_or("IDR".to_string()),
            source_type: self.source_type,
            source_document: self.source_document,
            goods_receipt_id: self.goods_receipt_id,
            supplier_id: self.supplier_id,
            bin_location: self.bin_location,
            storage_conditions: self.storage_conditions.unwrap_or(serde_json::json!({})),
            status: self.status.unwrap_or(BatchStatus::default()),
            is_active: self.is_active.unwrap_or(true),
            notes: self.notes,
            metadata: AuditMetadata::default(),
        })
    }
}
