use chrono::{DateTime, Utc, NaiveDate};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use rust_decimal::Decimal;

use super::AdjustmentType;
use super::AdjustmentStatus;
use super::AuditMetadata;

/// Strongly-typed ID for StockAdjustment
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StockAdjustmentId(pub Uuid);

impl StockAdjustmentId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for StockAdjustmentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for StockAdjustmentId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for StockAdjustmentId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<StockAdjustmentId> for Uuid {
    fn from(id: StockAdjustmentId) -> Self { id.0 }
}

impl AsRef<Uuid> for StockAdjustmentId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for StockAdjustmentId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct StockAdjustment {
    pub id: Uuid,
    pub provider_id: Uuid,
    pub adjustment_number: String,
    pub warehouse_id: Uuid,
    pub adjustment_type: AdjustmentType,
    pub adjustment_date: NaiveDate,
    pub is_full_count: bool,
    pub product_filter: serde_json::Value,
    pub total_items: i32,
    pub items_with_variance: i32,
    pub total_system_quantity: Decimal,
    pub total_counted_quantity: Decimal,
    pub total_variance_quantity: Decimal,
    pub currency: String,
    pub total_system_value: Decimal,
    pub total_adjustment_value: Decimal,
    pub status: AdjustmentStatus,
    pub requires_approval: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approved_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approved_by: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rejection_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub posted_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub posted_by: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    pub data: serde_json::Value,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl StockAdjustment {
    /// Create a builder for StockAdjustment
    pub fn builder() -> StockAdjustmentBuilder {
        StockAdjustmentBuilder::default()
    }

    /// Create a new StockAdjustment with required fields
    pub fn new(provider_id: Uuid, adjustment_number: String, warehouse_id: Uuid, adjustment_type: AdjustmentType, adjustment_date: NaiveDate, is_full_count: bool, product_filter: serde_json::Value, total_items: i32, items_with_variance: i32, total_system_quantity: Decimal, total_counted_quantity: Decimal, total_variance_quantity: Decimal, currency: String, total_system_value: Decimal, total_adjustment_value: Decimal, status: AdjustmentStatus, requires_approval: bool, data: serde_json::Value) -> Self {
        Self {
            id: Uuid::new_v4(),
            provider_id,
            adjustment_number,
            warehouse_id,
            adjustment_type,
            adjustment_date,
            is_full_count,
            product_filter,
            total_items,
            items_with_variance,
            total_system_quantity,
            total_counted_quantity,
            total_variance_quantity,
            currency,
            total_system_value,
            total_adjustment_value,
            status,
            requires_approval,
            approved_at: None,
            approved_by: None,
            rejection_reason: None,
            posted_at: None,
            posted_by: None,
            reason: None,
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
    pub fn typed_id(&self) -> StockAdjustmentId {
        StockAdjustmentId(self.id)
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
    pub fn status(&self) -> &AdjustmentStatus {
        &self.status
    }


    // ==========================================================
    // Fluent Setters (with_* for optional fields)
    // ==========================================================

    /// Set the approved_at field (chainable)
    pub fn with_approved_at(mut self, value: DateTime<Utc>) -> Self {
        self.approved_at = Some(value);
        self
    }

    /// Set the approved_by field (chainable)
    pub fn with_approved_by(mut self, value: Uuid) -> Self {
        self.approved_by = Some(value);
        self
    }

    /// Set the rejection_reason field (chainable)
    pub fn with_rejection_reason(mut self, value: String) -> Self {
        self.rejection_reason = Some(value);
        self
    }

    /// Set the posted_at field (chainable)
    pub fn with_posted_at(mut self, value: DateTime<Utc>) -> Self {
        self.posted_at = Some(value);
        self
    }

    /// Set the posted_by field (chainable)
    pub fn with_posted_by(mut self, value: Uuid) -> Self {
        self.posted_by = Some(value);
        self
    }

    /// Set the reason field (chainable)
    pub fn with_reason(mut self, value: String) -> Self {
        self.reason = Some(value);
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
                "adjustment_number" => {
                    if let Ok(v) = serde_json::from_value(value) { self.adjustment_number = v; }
                }
                "warehouse_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.warehouse_id = v; }
                }
                "adjustment_type" => {
                    if let Ok(v) = serde_json::from_value(value) { self.adjustment_type = v; }
                }
                "adjustment_date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.adjustment_date = v; }
                }
                "is_full_count" => {
                    if let Ok(v) = serde_json::from_value(value) { self.is_full_count = v; }
                }
                "product_filter" => {
                    if let Ok(v) = serde_json::from_value(value) { self.product_filter = v; }
                }
                "total_items" => {
                    if let Ok(v) = serde_json::from_value(value) { self.total_items = v; }
                }
                "items_with_variance" => {
                    if let Ok(v) = serde_json::from_value(value) { self.items_with_variance = v; }
                }
                "total_system_quantity" => {
                    if let Ok(v) = serde_json::from_value(value) { self.total_system_quantity = v; }
                }
                "total_counted_quantity" => {
                    if let Ok(v) = serde_json::from_value(value) { self.total_counted_quantity = v; }
                }
                "total_variance_quantity" => {
                    if let Ok(v) = serde_json::from_value(value) { self.total_variance_quantity = v; }
                }
                "currency" => {
                    if let Ok(v) = serde_json::from_value(value) { self.currency = v; }
                }
                "total_system_value" => {
                    if let Ok(v) = serde_json::from_value(value) { self.total_system_value = v; }
                }
                "total_adjustment_value" => {
                    if let Ok(v) = serde_json::from_value(value) { self.total_adjustment_value = v; }
                }
                "status" => {
                    if let Ok(v) = serde_json::from_value(value) { self.status = v; }
                }
                "requires_approval" => {
                    if let Ok(v) = serde_json::from_value(value) { self.requires_approval = v; }
                }
                "approved_at" => {
                    if let Ok(v) = serde_json::from_value(value) { self.approved_at = v; }
                }
                "approved_by" => {
                    if let Ok(v) = serde_json::from_value(value) { self.approved_by = v; }
                }
                "rejection_reason" => {
                    if let Ok(v) = serde_json::from_value(value) { self.rejection_reason = v; }
                }
                "posted_at" => {
                    if let Ok(v) = serde_json::from_value(value) { self.posted_at = v; }
                }
                "posted_by" => {
                    if let Ok(v) = serde_json::from_value(value) { self.posted_by = v; }
                }
                "reason" => {
                    if let Ok(v) = serde_json::from_value(value) { self.reason = v; }
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

impl super::Entity for StockAdjustment {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "StockAdjustment"
    }
}

impl backbone_core::PersistentEntity for StockAdjustment {
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

impl backbone_orm::EntityRepoMeta for StockAdjustment {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("provider_id".to_string(), "uuid".to_string());
        m.insert("warehouse_id".to_string(), "uuid".to_string());
        m.insert("adjustment_type".to_string(), "adjustment_type".to_string());
        m.insert("status".to_string(), "adjustment_status".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["adjustment_number", "currency"]
    }
}

/// Builder for StockAdjustment entity
///
/// Provides a fluent API for constructing StockAdjustment instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct StockAdjustmentBuilder {
    provider_id: Option<Uuid>,
    adjustment_number: Option<String>,
    warehouse_id: Option<Uuid>,
    adjustment_type: Option<AdjustmentType>,
    adjustment_date: Option<NaiveDate>,
    is_full_count: Option<bool>,
    product_filter: Option<serde_json::Value>,
    total_items: Option<i32>,
    items_with_variance: Option<i32>,
    total_system_quantity: Option<Decimal>,
    total_counted_quantity: Option<Decimal>,
    total_variance_quantity: Option<Decimal>,
    currency: Option<String>,
    total_system_value: Option<Decimal>,
    total_adjustment_value: Option<Decimal>,
    status: Option<AdjustmentStatus>,
    requires_approval: Option<bool>,
    approved_at: Option<DateTime<Utc>>,
    approved_by: Option<Uuid>,
    rejection_reason: Option<String>,
    posted_at: Option<DateTime<Utc>>,
    posted_by: Option<Uuid>,
    reason: Option<String>,
    notes: Option<String>,
    data: Option<serde_json::Value>,
}

impl StockAdjustmentBuilder {
    /// Set the provider_id field (required)
    pub fn provider_id(mut self, value: Uuid) -> Self {
        self.provider_id = Some(value);
        self
    }

    /// Set the adjustment_number field (required)
    pub fn adjustment_number(mut self, value: String) -> Self {
        self.adjustment_number = Some(value);
        self
    }

    /// Set the warehouse_id field (required)
    pub fn warehouse_id(mut self, value: Uuid) -> Self {
        self.warehouse_id = Some(value);
        self
    }

    /// Set the adjustment_type field (default: `AdjustmentType::default()`)
    pub fn adjustment_type(mut self, value: AdjustmentType) -> Self {
        self.adjustment_type = Some(value);
        self
    }

    /// Set the adjustment_date field (required)
    pub fn adjustment_date(mut self, value: NaiveDate) -> Self {
        self.adjustment_date = Some(value);
        self
    }

    /// Set the is_full_count field (default: `false`)
    pub fn is_full_count(mut self, value: bool) -> Self {
        self.is_full_count = Some(value);
        self
    }

    /// Set the product_filter field (default: `serde_json::json!([])`)
    pub fn product_filter(mut self, value: serde_json::Value) -> Self {
        self.product_filter = Some(value);
        self
    }

    /// Set the total_items field (default: `0`)
    pub fn total_items(mut self, value: i32) -> Self {
        self.total_items = Some(value);
        self
    }

    /// Set the items_with_variance field (default: `0`)
    pub fn items_with_variance(mut self, value: i32) -> Self {
        self.items_with_variance = Some(value);
        self
    }

    /// Set the total_system_quantity field (default: `Decimal::from(0)`)
    pub fn total_system_quantity(mut self, value: Decimal) -> Self {
        self.total_system_quantity = Some(value);
        self
    }

    /// Set the total_counted_quantity field (default: `Decimal::from(0)`)
    pub fn total_counted_quantity(mut self, value: Decimal) -> Self {
        self.total_counted_quantity = Some(value);
        self
    }

    /// Set the total_variance_quantity field (default: `Decimal::from(0)`)
    pub fn total_variance_quantity(mut self, value: Decimal) -> Self {
        self.total_variance_quantity = Some(value);
        self
    }

    /// Set the currency field (default: `"IDR".to_string()`)
    pub fn currency(mut self, value: String) -> Self {
        self.currency = Some(value);
        self
    }

    /// Set the total_system_value field (default: `Decimal::from(0)`)
    pub fn total_system_value(mut self, value: Decimal) -> Self {
        self.total_system_value = Some(value);
        self
    }

    /// Set the total_adjustment_value field (default: `Decimal::from(0)`)
    pub fn total_adjustment_value(mut self, value: Decimal) -> Self {
        self.total_adjustment_value = Some(value);
        self
    }

    /// Set the status field (default: `AdjustmentStatus::default()`)
    pub fn status(mut self, value: AdjustmentStatus) -> Self {
        self.status = Some(value);
        self
    }

    /// Set the requires_approval field (default: `true`)
    pub fn requires_approval(mut self, value: bool) -> Self {
        self.requires_approval = Some(value);
        self
    }

    /// Set the approved_at field (optional)
    pub fn approved_at(mut self, value: DateTime<Utc>) -> Self {
        self.approved_at = Some(value);
        self
    }

    /// Set the approved_by field (optional)
    pub fn approved_by(mut self, value: Uuid) -> Self {
        self.approved_by = Some(value);
        self
    }

    /// Set the rejection_reason field (optional)
    pub fn rejection_reason(mut self, value: String) -> Self {
        self.rejection_reason = Some(value);
        self
    }

    /// Set the posted_at field (optional)
    pub fn posted_at(mut self, value: DateTime<Utc>) -> Self {
        self.posted_at = Some(value);
        self
    }

    /// Set the posted_by field (optional)
    pub fn posted_by(mut self, value: Uuid) -> Self {
        self.posted_by = Some(value);
        self
    }

    /// Set the reason field (optional)
    pub fn reason(mut self, value: String) -> Self {
        self.reason = Some(value);
        self
    }

    /// Set the notes field (optional)
    pub fn notes(mut self, value: String) -> Self {
        self.notes = Some(value);
        self
    }

    /// Set the data field (default: `serde_json::json!({})`)
    pub fn data(mut self, value: serde_json::Value) -> Self {
        self.data = Some(value);
        self
    }

    /// Build the StockAdjustment entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<StockAdjustment, String> {
        let provider_id = self.provider_id.ok_or_else(|| "provider_id is required".to_string())?;
        let adjustment_number = self.adjustment_number.ok_or_else(|| "adjustment_number is required".to_string())?;
        let warehouse_id = self.warehouse_id.ok_or_else(|| "warehouse_id is required".to_string())?;
        let adjustment_date = self.adjustment_date.ok_or_else(|| "adjustment_date is required".to_string())?;

        Ok(StockAdjustment {
            id: Uuid::new_v4(),
            provider_id,
            adjustment_number,
            warehouse_id,
            adjustment_type: self.adjustment_type.unwrap_or(AdjustmentType::default()),
            adjustment_date,
            is_full_count: self.is_full_count.unwrap_or(false),
            product_filter: self.product_filter.unwrap_or(serde_json::json!([])),
            total_items: self.total_items.unwrap_or(0),
            items_with_variance: self.items_with_variance.unwrap_or(0),
            total_system_quantity: self.total_system_quantity.unwrap_or(Decimal::from(0)),
            total_counted_quantity: self.total_counted_quantity.unwrap_or(Decimal::from(0)),
            total_variance_quantity: self.total_variance_quantity.unwrap_or(Decimal::from(0)),
            currency: self.currency.unwrap_or("IDR".to_string()),
            total_system_value: self.total_system_value.unwrap_or(Decimal::from(0)),
            total_adjustment_value: self.total_adjustment_value.unwrap_or(Decimal::from(0)),
            status: self.status.unwrap_or(AdjustmentStatus::default()),
            requires_approval: self.requires_approval.unwrap_or(true),
            approved_at: self.approved_at,
            approved_by: self.approved_by,
            rejection_reason: self.rejection_reason,
            posted_at: self.posted_at,
            posted_by: self.posted_by,
            reason: self.reason,
            notes: self.notes,
            data: self.data.unwrap_or(serde_json::json!({})),
            metadata: AuditMetadata::default(),
        })
    }
}
