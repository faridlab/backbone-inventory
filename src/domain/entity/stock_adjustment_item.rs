use chrono::{DateTime, Utc, NaiveDate};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use rust_decimal::Decimal;

use super::UnitOfMeasure;
use super::AuditMetadata;

/// Strongly-typed ID for StockAdjustmentItem
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StockAdjustmentItemId(pub Uuid);

impl StockAdjustmentItemId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for StockAdjustmentItemId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for StockAdjustmentItemId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for StockAdjustmentItemId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<StockAdjustmentItemId> for Uuid {
    fn from(id: StockAdjustmentItemId) -> Self { id.0 }
}

impl AsRef<Uuid> for StockAdjustmentItemId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for StockAdjustmentItemId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct StockAdjustmentItem {
    pub id: Uuid,
    pub adjustment_id: Uuid,
    pub provider_id: Uuid,
    pub line_number: i32,
    pub product_id: Uuid,
    pub stock_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub batch_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub batch_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiry_date: Option<NaiveDate>,
    pub uom: UnitOfMeasure,
    pub system_quantity: Decimal,
    pub counted_quantity: Decimal,
    pub variance_quantity: Decimal,
    pub currency: String,
    pub unit_cost: Decimal,
    pub system_value: Decimal,
    pub adjustment_value: Decimal,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variance_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bin_location: Option<String>,
    pub is_posted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub movement_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    pub data: serde_json::Value,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl StockAdjustmentItem {
    /// Create a builder for StockAdjustmentItem
    pub fn builder() -> StockAdjustmentItemBuilder {
        StockAdjustmentItemBuilder::default()
    }

    /// Create a new StockAdjustmentItem with required fields
    pub fn new(adjustment_id: Uuid, provider_id: Uuid, line_number: i32, product_id: Uuid, stock_id: Uuid, uom: UnitOfMeasure, system_quantity: Decimal, counted_quantity: Decimal, variance_quantity: Decimal, currency: String, unit_cost: Decimal, system_value: Decimal, adjustment_value: Decimal, is_posted: bool, data: serde_json::Value) -> Self {
        Self {
            id: Uuid::new_v4(),
            adjustment_id,
            provider_id,
            line_number,
            product_id,
            stock_id,
            batch_id: None,
            batch_number: None,
            expiry_date: None,
            uom,
            system_quantity,
            counted_quantity,
            variance_quantity,
            currency,
            unit_cost,
            system_value,
            adjustment_value,
            variance_reason: None,
            bin_location: None,
            is_posted,
            movement_id: None,
            notes: None,
            data,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> StockAdjustmentItemId {
        StockAdjustmentItemId(self.id)
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
    // Fluent Setters (with_* for optional fields)
    // ==========================================================

    /// Set the batch_id field (chainable)
    pub fn with_batch_id(mut self, value: Uuid) -> Self {
        self.batch_id = Some(value);
        self
    }

    /// Set the batch_number field (chainable)
    pub fn with_batch_number(mut self, value: String) -> Self {
        self.batch_number = Some(value);
        self
    }

    /// Set the expiry_date field (chainable)
    pub fn with_expiry_date(mut self, value: NaiveDate) -> Self {
        self.expiry_date = Some(value);
        self
    }

    /// Set the variance_reason field (chainable)
    pub fn with_variance_reason(mut self, value: String) -> Self {
        self.variance_reason = Some(value);
        self
    }

    /// Set the bin_location field (chainable)
    pub fn with_bin_location(mut self, value: String) -> Self {
        self.bin_location = Some(value);
        self
    }

    /// Set the movement_id field (chainable)
    pub fn with_movement_id(mut self, value: Uuid) -> Self {
        self.movement_id = Some(value);
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
                "adjustment_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.adjustment_id = v; }
                }
                "provider_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.provider_id = v; }
                }
                "line_number" => {
                    if let Ok(v) = serde_json::from_value(value) { self.line_number = v; }
                }
                "product_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.product_id = v; }
                }
                "stock_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.stock_id = v; }
                }
                "batch_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.batch_id = v; }
                }
                "batch_number" => {
                    if let Ok(v) = serde_json::from_value(value) { self.batch_number = v; }
                }
                "expiry_date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.expiry_date = v; }
                }
                "uom" => {
                    if let Ok(v) = serde_json::from_value(value) { self.uom = v; }
                }
                "system_quantity" => {
                    if let Ok(v) = serde_json::from_value(value) { self.system_quantity = v; }
                }
                "counted_quantity" => {
                    if let Ok(v) = serde_json::from_value(value) { self.counted_quantity = v; }
                }
                "variance_quantity" => {
                    if let Ok(v) = serde_json::from_value(value) { self.variance_quantity = v; }
                }
                "currency" => {
                    if let Ok(v) = serde_json::from_value(value) { self.currency = v; }
                }
                "unit_cost" => {
                    if let Ok(v) = serde_json::from_value(value) { self.unit_cost = v; }
                }
                "system_value" => {
                    if let Ok(v) = serde_json::from_value(value) { self.system_value = v; }
                }
                "adjustment_value" => {
                    if let Ok(v) = serde_json::from_value(value) { self.adjustment_value = v; }
                }
                "variance_reason" => {
                    if let Ok(v) = serde_json::from_value(value) { self.variance_reason = v; }
                }
                "bin_location" => {
                    if let Ok(v) = serde_json::from_value(value) { self.bin_location = v; }
                }
                "is_posted" => {
                    if let Ok(v) = serde_json::from_value(value) { self.is_posted = v; }
                }
                "movement_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.movement_id = v; }
                }
                "notes" => {
                    if let Ok(v) = serde_json::from_value(value) { self.notes = v; }
                }
                "data" => {
                    if let Ok(v) = serde_json::from_value(value) { self.data = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for StockAdjustmentItem {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "StockAdjustmentItem"
    }
}

impl backbone_core::PersistentEntity for StockAdjustmentItem {
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

impl backbone_orm::EntityRepoMeta for StockAdjustmentItem {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("adjustment_id".to_string(), "uuid".to_string());
        m.insert("provider_id".to_string(), "uuid".to_string());
        m.insert("product_id".to_string(), "uuid".to_string());
        m.insert("stock_id".to_string(), "uuid".to_string());
        m.insert("batch_id".to_string(), "uuid".to_string());
        m.insert("movement_id".to_string(), "uuid".to_string());
        m.insert("uom".to_string(), "unit_of_measure".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["currency"]
    }
}

/// Builder for StockAdjustmentItem entity
///
/// Provides a fluent API for constructing StockAdjustmentItem instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct StockAdjustmentItemBuilder {
    adjustment_id: Option<Uuid>,
    provider_id: Option<Uuid>,
    line_number: Option<i32>,
    product_id: Option<Uuid>,
    stock_id: Option<Uuid>,
    batch_id: Option<Uuid>,
    batch_number: Option<String>,
    expiry_date: Option<NaiveDate>,
    uom: Option<UnitOfMeasure>,
    system_quantity: Option<Decimal>,
    counted_quantity: Option<Decimal>,
    variance_quantity: Option<Decimal>,
    currency: Option<String>,
    unit_cost: Option<Decimal>,
    system_value: Option<Decimal>,
    adjustment_value: Option<Decimal>,
    variance_reason: Option<String>,
    bin_location: Option<String>,
    is_posted: Option<bool>,
    movement_id: Option<Uuid>,
    notes: Option<String>,
    data: Option<serde_json::Value>,
}

impl StockAdjustmentItemBuilder {
    /// Set the adjustment_id field (required)
    pub fn adjustment_id(mut self, value: Uuid) -> Self {
        self.adjustment_id = Some(value);
        self
    }

    /// Set the provider_id field (required)
    pub fn provider_id(mut self, value: Uuid) -> Self {
        self.provider_id = Some(value);
        self
    }

    /// Set the line_number field (required)
    pub fn line_number(mut self, value: i32) -> Self {
        self.line_number = Some(value);
        self
    }

    /// Set the product_id field (required)
    pub fn product_id(mut self, value: Uuid) -> Self {
        self.product_id = Some(value);
        self
    }

    /// Set the stock_id field (required)
    pub fn stock_id(mut self, value: Uuid) -> Self {
        self.stock_id = Some(value);
        self
    }

    /// Set the batch_id field (optional)
    pub fn batch_id(mut self, value: Uuid) -> Self {
        self.batch_id = Some(value);
        self
    }

    /// Set the batch_number field (optional)
    pub fn batch_number(mut self, value: String) -> Self {
        self.batch_number = Some(value);
        self
    }

    /// Set the expiry_date field (optional)
    pub fn expiry_date(mut self, value: NaiveDate) -> Self {
        self.expiry_date = Some(value);
        self
    }

    /// Set the uom field (required)
    pub fn uom(mut self, value: UnitOfMeasure) -> Self {
        self.uom = Some(value);
        self
    }

    /// Set the system_quantity field (required)
    pub fn system_quantity(mut self, value: Decimal) -> Self {
        self.system_quantity = Some(value);
        self
    }

    /// Set the counted_quantity field (required)
    pub fn counted_quantity(mut self, value: Decimal) -> Self {
        self.counted_quantity = Some(value);
        self
    }

    /// Set the variance_quantity field (required)
    pub fn variance_quantity(mut self, value: Decimal) -> Self {
        self.variance_quantity = Some(value);
        self
    }

    /// Set the currency field (default: `"IDR".to_string()`)
    pub fn currency(mut self, value: String) -> Self {
        self.currency = Some(value);
        self
    }

    /// Set the unit_cost field (required)
    pub fn unit_cost(mut self, value: Decimal) -> Self {
        self.unit_cost = Some(value);
        self
    }

    /// Set the system_value field (required)
    pub fn system_value(mut self, value: Decimal) -> Self {
        self.system_value = Some(value);
        self
    }

    /// Set the adjustment_value field (required)
    pub fn adjustment_value(mut self, value: Decimal) -> Self {
        self.adjustment_value = Some(value);
        self
    }

    /// Set the variance_reason field (optional)
    pub fn variance_reason(mut self, value: String) -> Self {
        self.variance_reason = Some(value);
        self
    }

    /// Set the bin_location field (optional)
    pub fn bin_location(mut self, value: String) -> Self {
        self.bin_location = Some(value);
        self
    }

    /// Set the is_posted field (default: `false`)
    pub fn is_posted(mut self, value: bool) -> Self {
        self.is_posted = Some(value);
        self
    }

    /// Set the movement_id field (optional)
    pub fn movement_id(mut self, value: Uuid) -> Self {
        self.movement_id = Some(value);
        self
    }

    /// Set the notes field (optional)
    pub fn notes(mut self, value: String) -> Self {
        self.notes = Some(value);
        self
    }

    /// Set the data field (default: `serde_json::json!({"sku":null,"product_name":null})`)
    pub fn data(mut self, value: serde_json::Value) -> Self {
        self.data = Some(value);
        self
    }

    /// Build the StockAdjustmentItem entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<StockAdjustmentItem, String> {
        let adjustment_id = self.adjustment_id.ok_or_else(|| "adjustment_id is required".to_string())?;
        let provider_id = self.provider_id.ok_or_else(|| "provider_id is required".to_string())?;
        let line_number = self.line_number.ok_or_else(|| "line_number is required".to_string())?;
        let product_id = self.product_id.ok_or_else(|| "product_id is required".to_string())?;
        let stock_id = self.stock_id.ok_or_else(|| "stock_id is required".to_string())?;
        let uom = self.uom.ok_or_else(|| "uom is required".to_string())?;
        let system_quantity = self.system_quantity.ok_or_else(|| "system_quantity is required".to_string())?;
        let counted_quantity = self.counted_quantity.ok_or_else(|| "counted_quantity is required".to_string())?;
        let variance_quantity = self.variance_quantity.ok_or_else(|| "variance_quantity is required".to_string())?;
        let unit_cost = self.unit_cost.ok_or_else(|| "unit_cost is required".to_string())?;
        let system_value = self.system_value.ok_or_else(|| "system_value is required".to_string())?;
        let adjustment_value = self.adjustment_value.ok_or_else(|| "adjustment_value is required".to_string())?;

        Ok(StockAdjustmentItem {
            id: Uuid::new_v4(),
            adjustment_id,
            provider_id,
            line_number,
            product_id,
            stock_id,
            batch_id: self.batch_id,
            batch_number: self.batch_number,
            expiry_date: self.expiry_date,
            uom,
            system_quantity,
            counted_quantity,
            variance_quantity,
            currency: self.currency.unwrap_or("IDR".to_string()),
            unit_cost,
            system_value,
            adjustment_value,
            variance_reason: self.variance_reason,
            bin_location: self.bin_location,
            is_posted: self.is_posted.unwrap_or(false),
            movement_id: self.movement_id,
            notes: self.notes,
            data: self.data.unwrap_or(serde_json::json!({"sku":null,"product_name":null})),
            metadata: AuditMetadata::default(),
        })
    }
}
