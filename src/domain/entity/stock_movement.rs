use chrono::{DateTime, Utc, NaiveDate};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use rust_decimal::Decimal;

use super::MovementType;
use super::MovementReason;
use super::MovementStatus;
use super::AuditMetadata;

/// Strongly-typed ID for StockMovement
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StockMovementId(pub Uuid);

impl StockMovementId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for StockMovementId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for StockMovementId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for StockMovementId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<StockMovementId> for Uuid {
    fn from(id: StockMovementId) -> Self { id.0 }
}

impl AsRef<Uuid> for StockMovementId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for StockMovementId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct StockMovement {
    pub id: Uuid,
    pub provider_id: Uuid,
    pub movement_number: String,
    pub movement_type: MovementType,
    pub movement_reason: MovementReason,
    pub product_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warehouse_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_location: Option<String>,
    pub quantity: Decimal,
    pub uom: String,
    pub balance_before: Decimal,
    pub balance_after: Decimal,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub batch_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub batch_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiry_date: Option<NaiveDate>,
    pub unit_cost: Decimal,
    pub total_cost: Decimal,
    pub currency: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_number: Option<String>,
    pub status: MovementStatus,
    pub movement_date: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub posted_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub performed_by: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason_detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl StockMovement {
    /// Create a builder for StockMovement
    pub fn builder() -> StockMovementBuilder {
        StockMovementBuilder::default()
    }

    /// Create a new StockMovement with required fields
    pub fn new(provider_id: Uuid, movement_number: String, movement_type: MovementType, movement_reason: MovementReason, product_id: Uuid, quantity: Decimal, uom: String, balance_before: Decimal, balance_after: Decimal, unit_cost: Decimal, total_cost: Decimal, currency: String, status: MovementStatus, movement_date: DateTime<Utc>) -> Self {
        Self {
            id: Uuid::new_v4(),
            provider_id,
            movement_number,
            movement_type,
            movement_reason,
            product_id,
            warehouse_id: None,
            from_location: None,
            to_location: None,
            quantity,
            uom,
            balance_before,
            balance_after,
            batch_id: None,
            batch_number: None,
            expiry_date: None,
            unit_cost,
            total_cost,
            currency,
            source_type: None,
            source_id: None,
            source_number: None,
            status,
            movement_date,
            posted_at: None,
            performed_by: None,
            reason_detail: None,
            notes: None,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> StockMovementId {
        StockMovementId(self.id)
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
    pub fn status(&self) -> &MovementStatus {
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

    /// Set the from_location field (chainable)
    pub fn with_from_location(mut self, value: String) -> Self {
        self.from_location = Some(value);
        self
    }

    /// Set the to_location field (chainable)
    pub fn with_to_location(mut self, value: String) -> Self {
        self.to_location = Some(value);
        self
    }

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

    /// Set the source_type field (chainable)
    pub fn with_source_type(mut self, value: String) -> Self {
        self.source_type = Some(value);
        self
    }

    /// Set the source_id field (chainable)
    pub fn with_source_id(mut self, value: Uuid) -> Self {
        self.source_id = Some(value);
        self
    }

    /// Set the source_number field (chainable)
    pub fn with_source_number(mut self, value: String) -> Self {
        self.source_number = Some(value);
        self
    }

    /// Set the posted_at field (chainable)
    pub fn with_posted_at(mut self, value: DateTime<Utc>) -> Self {
        self.posted_at = Some(value);
        self
    }

    /// Set the performed_by field (chainable)
    pub fn with_performed_by(mut self, value: Uuid) -> Self {
        self.performed_by = Some(value);
        self
    }

    /// Set the reason_detail field (chainable)
    pub fn with_reason_detail(mut self, value: String) -> Self {
        self.reason_detail = Some(value);
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
                "movement_number" => {
                    if let Ok(v) = serde_json::from_value(value) { self.movement_number = v; }
                }
                "movement_type" => {
                    if let Ok(v) = serde_json::from_value(value) { self.movement_type = v; }
                }
                "movement_reason" => {
                    if let Ok(v) = serde_json::from_value(value) { self.movement_reason = v; }
                }
                "product_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.product_id = v; }
                }
                "warehouse_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.warehouse_id = v; }
                }
                "from_location" => {
                    if let Ok(v) = serde_json::from_value(value) { self.from_location = v; }
                }
                "to_location" => {
                    if let Ok(v) = serde_json::from_value(value) { self.to_location = v; }
                }
                "quantity" => {
                    if let Ok(v) = serde_json::from_value(value) { self.quantity = v; }
                }
                "uom" => {
                    if let Ok(v) = serde_json::from_value(value) { self.uom = v; }
                }
                "balance_before" => {
                    if let Ok(v) = serde_json::from_value(value) { self.balance_before = v; }
                }
                "balance_after" => {
                    if let Ok(v) = serde_json::from_value(value) { self.balance_after = v; }
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
                "unit_cost" => {
                    if let Ok(v) = serde_json::from_value(value) { self.unit_cost = v; }
                }
                "total_cost" => {
                    if let Ok(v) = serde_json::from_value(value) { self.total_cost = v; }
                }
                "currency" => {
                    if let Ok(v) = serde_json::from_value(value) { self.currency = v; }
                }
                "source_type" => {
                    if let Ok(v) = serde_json::from_value(value) { self.source_type = v; }
                }
                "source_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.source_id = v; }
                }
                "source_number" => {
                    if let Ok(v) = serde_json::from_value(value) { self.source_number = v; }
                }
                "status" => {
                    if let Ok(v) = serde_json::from_value(value) { self.status = v; }
                }
                "movement_date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.movement_date = v; }
                }
                "posted_at" => {
                    if let Ok(v) = serde_json::from_value(value) { self.posted_at = v; }
                }
                "performed_by" => {
                    if let Ok(v) = serde_json::from_value(value) { self.performed_by = v; }
                }
                "reason_detail" => {
                    if let Ok(v) = serde_json::from_value(value) { self.reason_detail = v; }
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

impl super::Entity for StockMovement {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "StockMovement"
    }
}

impl backbone_core::PersistentEntity for StockMovement {
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

impl backbone_orm::EntityRepoMeta for StockMovement {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("provider_id".to_string(), "uuid".to_string());
        m.insert("product_id".to_string(), "uuid".to_string());
        m.insert("warehouse_id".to_string(), "uuid".to_string());
        m.insert("batch_id".to_string(), "uuid".to_string());
        m.insert("source_id".to_string(), "uuid".to_string());
        m.insert("movement_type".to_string(), "movement_type".to_string());
        m.insert("movement_reason".to_string(), "movement_reason".to_string());
        m.insert("status".to_string(), "movement_status".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["movement_number", "uom", "currency"]
    }
}

/// Builder for StockMovement entity
///
/// Provides a fluent API for constructing StockMovement instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct StockMovementBuilder {
    provider_id: Option<Uuid>,
    movement_number: Option<String>,
    movement_type: Option<MovementType>,
    movement_reason: Option<MovementReason>,
    product_id: Option<Uuid>,
    warehouse_id: Option<Uuid>,
    from_location: Option<String>,
    to_location: Option<String>,
    quantity: Option<Decimal>,
    uom: Option<String>,
    balance_before: Option<Decimal>,
    balance_after: Option<Decimal>,
    batch_id: Option<Uuid>,
    batch_number: Option<String>,
    expiry_date: Option<NaiveDate>,
    unit_cost: Option<Decimal>,
    total_cost: Option<Decimal>,
    currency: Option<String>,
    source_type: Option<String>,
    source_id: Option<Uuid>,
    source_number: Option<String>,
    status: Option<MovementStatus>,
    movement_date: Option<DateTime<Utc>>,
    posted_at: Option<DateTime<Utc>>,
    performed_by: Option<Uuid>,
    reason_detail: Option<String>,
    notes: Option<String>,
}

impl StockMovementBuilder {
    /// Set the provider_id field (required)
    pub fn provider_id(mut self, value: Uuid) -> Self {
        self.provider_id = Some(value);
        self
    }

    /// Set the movement_number field (required)
    pub fn movement_number(mut self, value: String) -> Self {
        self.movement_number = Some(value);
        self
    }

    /// Set the movement_type field (default: `MovementType::default()`)
    pub fn movement_type(mut self, value: MovementType) -> Self {
        self.movement_type = Some(value);
        self
    }

    /// Set the movement_reason field (default: `MovementReason::default()`)
    pub fn movement_reason(mut self, value: MovementReason) -> Self {
        self.movement_reason = Some(value);
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

    /// Set the from_location field (optional)
    pub fn from_location(mut self, value: String) -> Self {
        self.from_location = Some(value);
        self
    }

    /// Set the to_location field (optional)
    pub fn to_location(mut self, value: String) -> Self {
        self.to_location = Some(value);
        self
    }

    /// Set the quantity field (required)
    pub fn quantity(mut self, value: Decimal) -> Self {
        self.quantity = Some(value);
        self
    }

    /// Set the uom field (required)
    pub fn uom(mut self, value: String) -> Self {
        self.uom = Some(value);
        self
    }

    /// Set the balance_before field (required)
    pub fn balance_before(mut self, value: Decimal) -> Self {
        self.balance_before = Some(value);
        self
    }

    /// Set the balance_after field (required)
    pub fn balance_after(mut self, value: Decimal) -> Self {
        self.balance_after = Some(value);
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

    /// Set the unit_cost field (required)
    pub fn unit_cost(mut self, value: Decimal) -> Self {
        self.unit_cost = Some(value);
        self
    }

    /// Set the total_cost field (required)
    pub fn total_cost(mut self, value: Decimal) -> Self {
        self.total_cost = Some(value);
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

    /// Set the source_id field (optional)
    pub fn source_id(mut self, value: Uuid) -> Self {
        self.source_id = Some(value);
        self
    }

    /// Set the source_number field (optional)
    pub fn source_number(mut self, value: String) -> Self {
        self.source_number = Some(value);
        self
    }

    /// Set the status field (default: `MovementStatus::default()`)
    pub fn status(mut self, value: MovementStatus) -> Self {
        self.status = Some(value);
        self
    }

    /// Set the movement_date field (required)
    pub fn movement_date(mut self, value: DateTime<Utc>) -> Self {
        self.movement_date = Some(value);
        self
    }

    /// Set the posted_at field (optional)
    pub fn posted_at(mut self, value: DateTime<Utc>) -> Self {
        self.posted_at = Some(value);
        self
    }

    /// Set the performed_by field (optional)
    pub fn performed_by(mut self, value: Uuid) -> Self {
        self.performed_by = Some(value);
        self
    }

    /// Set the reason_detail field (optional)
    pub fn reason_detail(mut self, value: String) -> Self {
        self.reason_detail = Some(value);
        self
    }

    /// Set the notes field (optional)
    pub fn notes(mut self, value: String) -> Self {
        self.notes = Some(value);
        self
    }

    /// Build the StockMovement entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<StockMovement, String> {
        let provider_id = self.provider_id.ok_or_else(|| "provider_id is required".to_string())?;
        let movement_number = self.movement_number.ok_or_else(|| "movement_number is required".to_string())?;
        let product_id = self.product_id.ok_or_else(|| "product_id is required".to_string())?;
        let quantity = self.quantity.ok_or_else(|| "quantity is required".to_string())?;
        let uom = self.uom.ok_or_else(|| "uom is required".to_string())?;
        let balance_before = self.balance_before.ok_or_else(|| "balance_before is required".to_string())?;
        let balance_after = self.balance_after.ok_or_else(|| "balance_after is required".to_string())?;
        let unit_cost = self.unit_cost.ok_or_else(|| "unit_cost is required".to_string())?;
        let total_cost = self.total_cost.ok_or_else(|| "total_cost is required".to_string())?;
        let movement_date = self.movement_date.ok_or_else(|| "movement_date is required".to_string())?;

        Ok(StockMovement {
            id: Uuid::new_v4(),
            provider_id,
            movement_number,
            movement_type: self.movement_type.unwrap_or(MovementType::default()),
            movement_reason: self.movement_reason.unwrap_or(MovementReason::default()),
            product_id,
            warehouse_id: self.warehouse_id,
            from_location: self.from_location,
            to_location: self.to_location,
            quantity,
            uom,
            balance_before,
            balance_after,
            batch_id: self.batch_id,
            batch_number: self.batch_number,
            expiry_date: self.expiry_date,
            unit_cost,
            total_cost,
            currency: self.currency.unwrap_or("IDR".to_string()),
            source_type: self.source_type,
            source_id: self.source_id,
            source_number: self.source_number,
            status: self.status.unwrap_or(MovementStatus::default()),
            movement_date,
            posted_at: self.posted_at,
            performed_by: self.performed_by,
            reason_detail: self.reason_detail,
            notes: self.notes,
            metadata: AuditMetadata::default(),
        })
    }
}
