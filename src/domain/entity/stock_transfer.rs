use chrono::{DateTime, Utc, NaiveDate};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use rust_decimal::Decimal;

use super::TransferType;
use super::TransferStatus;
use super::AuditMetadata;

/// Strongly-typed ID for StockTransfer
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StockTransferId(pub Uuid);

impl StockTransferId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for StockTransferId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for StockTransferId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for StockTransferId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<StockTransferId> for Uuid {
    fn from(id: StockTransferId) -> Self { id.0 }
}

impl AsRef<Uuid> for StockTransferId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for StockTransferId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct StockTransfer {
    pub id: Uuid,
    pub provider_id: Uuid,
    pub transfer_number: String,
    pub transfer_type: TransferType,
    pub from_warehouse_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_location: Option<String>,
    pub to_warehouse_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_provider_id: Option<Uuid>,
    pub transfer_date: NaiveDate,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_arrival_date: Option<NaiveDate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shipped_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub received_at: Option<DateTime<Utc>>,
    pub total_items: i32,
    pub total_quantity: Decimal,
    pub total_value: Decimal,
    pub currency: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shipping_method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub carrier: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tracking_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shipping_cost: Option<Decimal>,
    pub status: TransferStatus,
    pub requires_approval: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approved_by: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approved_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requested_by: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shipped_by: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub received_by: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receiving_notes: Option<String>,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl StockTransfer {
    /// Create a builder for StockTransfer
    pub fn builder() -> StockTransferBuilder {
        StockTransferBuilder::default()
    }

    /// Create a new StockTransfer with required fields
    pub fn new(provider_id: Uuid, transfer_number: String, transfer_type: TransferType, from_warehouse_id: Uuid, to_warehouse_id: Uuid, transfer_date: NaiveDate, total_items: i32, total_quantity: Decimal, total_value: Decimal, currency: String, status: TransferStatus, requires_approval: bool) -> Self {
        Self {
            id: Uuid::new_v4(),
            provider_id,
            transfer_number,
            transfer_type,
            from_warehouse_id,
            from_location: None,
            to_warehouse_id,
            to_location: None,
            to_provider_id: None,
            transfer_date,
            expected_arrival_date: None,
            shipped_at: None,
            received_at: None,
            total_items,
            total_quantity,
            total_value,
            currency,
            shipping_method: None,
            carrier: None,
            tracking_number: None,
            shipping_cost: None,
            status,
            requires_approval,
            approved_by: None,
            approved_at: None,
            requested_by: None,
            shipped_by: None,
            received_by: None,
            reason: None,
            notes: None,
            receiving_notes: None,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> StockTransferId {
        StockTransferId(self.id)
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
    pub fn status(&self) -> &TransferStatus {
        &self.status
    }


    // ==========================================================
    // Fluent Setters (with_* for optional fields)
    // ==========================================================

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

    /// Set the to_provider_id field (chainable)
    pub fn with_to_provider_id(mut self, value: Uuid) -> Self {
        self.to_provider_id = Some(value);
        self
    }

    /// Set the expected_arrival_date field (chainable)
    pub fn with_expected_arrival_date(mut self, value: NaiveDate) -> Self {
        self.expected_arrival_date = Some(value);
        self
    }

    /// Set the shipped_at field (chainable)
    pub fn with_shipped_at(mut self, value: DateTime<Utc>) -> Self {
        self.shipped_at = Some(value);
        self
    }

    /// Set the received_at field (chainable)
    pub fn with_received_at(mut self, value: DateTime<Utc>) -> Self {
        self.received_at = Some(value);
        self
    }

    /// Set the shipping_method field (chainable)
    pub fn with_shipping_method(mut self, value: String) -> Self {
        self.shipping_method = Some(value);
        self
    }

    /// Set the carrier field (chainable)
    pub fn with_carrier(mut self, value: String) -> Self {
        self.carrier = Some(value);
        self
    }

    /// Set the tracking_number field (chainable)
    pub fn with_tracking_number(mut self, value: String) -> Self {
        self.tracking_number = Some(value);
        self
    }

    /// Set the shipping_cost field (chainable)
    pub fn with_shipping_cost(mut self, value: Decimal) -> Self {
        self.shipping_cost = Some(value);
        self
    }

    /// Set the approved_by field (chainable)
    pub fn with_approved_by(mut self, value: Uuid) -> Self {
        self.approved_by = Some(value);
        self
    }

    /// Set the approved_at field (chainable)
    pub fn with_approved_at(mut self, value: DateTime<Utc>) -> Self {
        self.approved_at = Some(value);
        self
    }

    /// Set the requested_by field (chainable)
    pub fn with_requested_by(mut self, value: Uuid) -> Self {
        self.requested_by = Some(value);
        self
    }

    /// Set the shipped_by field (chainable)
    pub fn with_shipped_by(mut self, value: Uuid) -> Self {
        self.shipped_by = Some(value);
        self
    }

    /// Set the received_by field (chainable)
    pub fn with_received_by(mut self, value: Uuid) -> Self {
        self.received_by = Some(value);
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

    /// Set the receiving_notes field (chainable)
    pub fn with_receiving_notes(mut self, value: String) -> Self {
        self.receiving_notes = Some(value);
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
                "transfer_number" => {
                    if let Ok(v) = serde_json::from_value(value) { self.transfer_number = v; }
                }
                "transfer_type" => {
                    if let Ok(v) = serde_json::from_value(value) { self.transfer_type = v; }
                }
                "from_warehouse_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.from_warehouse_id = v; }
                }
                "from_location" => {
                    if let Ok(v) = serde_json::from_value(value) { self.from_location = v; }
                }
                "to_warehouse_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.to_warehouse_id = v; }
                }
                "to_location" => {
                    if let Ok(v) = serde_json::from_value(value) { self.to_location = v; }
                }
                "to_provider_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.to_provider_id = v; }
                }
                "transfer_date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.transfer_date = v; }
                }
                "expected_arrival_date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.expected_arrival_date = v; }
                }
                "shipped_at" => {
                    if let Ok(v) = serde_json::from_value(value) { self.shipped_at = v; }
                }
                "received_at" => {
                    if let Ok(v) = serde_json::from_value(value) { self.received_at = v; }
                }
                "total_items" => {
                    if let Ok(v) = serde_json::from_value(value) { self.total_items = v; }
                }
                "total_quantity" => {
                    if let Ok(v) = serde_json::from_value(value) { self.total_quantity = v; }
                }
                "total_value" => {
                    if let Ok(v) = serde_json::from_value(value) { self.total_value = v; }
                }
                "currency" => {
                    if let Ok(v) = serde_json::from_value(value) { self.currency = v; }
                }
                "shipping_method" => {
                    if let Ok(v) = serde_json::from_value(value) { self.shipping_method = v; }
                }
                "carrier" => {
                    if let Ok(v) = serde_json::from_value(value) { self.carrier = v; }
                }
                "tracking_number" => {
                    if let Ok(v) = serde_json::from_value(value) { self.tracking_number = v; }
                }
                "shipping_cost" => {
                    if let Ok(v) = serde_json::from_value(value) { self.shipping_cost = v; }
                }
                "status" => {
                    if let Ok(v) = serde_json::from_value(value) { self.status = v; }
                }
                "requires_approval" => {
                    if let Ok(v) = serde_json::from_value(value) { self.requires_approval = v; }
                }
                "approved_by" => {
                    if let Ok(v) = serde_json::from_value(value) { self.approved_by = v; }
                }
                "approved_at" => {
                    if let Ok(v) = serde_json::from_value(value) { self.approved_at = v; }
                }
                "requested_by" => {
                    if let Ok(v) = serde_json::from_value(value) { self.requested_by = v; }
                }
                "shipped_by" => {
                    if let Ok(v) = serde_json::from_value(value) { self.shipped_by = v; }
                }
                "received_by" => {
                    if let Ok(v) = serde_json::from_value(value) { self.received_by = v; }
                }
                "reason" => {
                    if let Ok(v) = serde_json::from_value(value) { self.reason = v; }
                }
                "notes" => {
                    if let Ok(v) = serde_json::from_value(value) { self.notes = v; }
                }
                "receiving_notes" => {
                    if let Ok(v) = serde_json::from_value(value) { self.receiving_notes = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for StockTransfer {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "StockTransfer"
    }
}

impl backbone_core::PersistentEntity for StockTransfer {
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

impl backbone_orm::EntityRepoMeta for StockTransfer {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("provider_id".to_string(), "uuid".to_string());
        m.insert("from_warehouse_id".to_string(), "uuid".to_string());
        m.insert("to_warehouse_id".to_string(), "uuid".to_string());
        m.insert("to_provider_id".to_string(), "uuid".to_string());
        m.insert("transfer_type".to_string(), "transfer_type".to_string());
        m.insert("status".to_string(), "transfer_status".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["transfer_number", "currency"]
    }
}

/// Builder for StockTransfer entity
///
/// Provides a fluent API for constructing StockTransfer instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct StockTransferBuilder {
    provider_id: Option<Uuid>,
    transfer_number: Option<String>,
    transfer_type: Option<TransferType>,
    from_warehouse_id: Option<Uuid>,
    from_location: Option<String>,
    to_warehouse_id: Option<Uuid>,
    to_location: Option<String>,
    to_provider_id: Option<Uuid>,
    transfer_date: Option<NaiveDate>,
    expected_arrival_date: Option<NaiveDate>,
    shipped_at: Option<DateTime<Utc>>,
    received_at: Option<DateTime<Utc>>,
    total_items: Option<i32>,
    total_quantity: Option<Decimal>,
    total_value: Option<Decimal>,
    currency: Option<String>,
    shipping_method: Option<String>,
    carrier: Option<String>,
    tracking_number: Option<String>,
    shipping_cost: Option<Decimal>,
    status: Option<TransferStatus>,
    requires_approval: Option<bool>,
    approved_by: Option<Uuid>,
    approved_at: Option<DateTime<Utc>>,
    requested_by: Option<Uuid>,
    shipped_by: Option<Uuid>,
    received_by: Option<Uuid>,
    reason: Option<String>,
    notes: Option<String>,
    receiving_notes: Option<String>,
}

impl StockTransferBuilder {
    /// Set the provider_id field (required)
    pub fn provider_id(mut self, value: Uuid) -> Self {
        self.provider_id = Some(value);
        self
    }

    /// Set the transfer_number field (required)
    pub fn transfer_number(mut self, value: String) -> Self {
        self.transfer_number = Some(value);
        self
    }

    /// Set the transfer_type field (default: `TransferType::default()`)
    pub fn transfer_type(mut self, value: TransferType) -> Self {
        self.transfer_type = Some(value);
        self
    }

    /// Set the from_warehouse_id field (required)
    pub fn from_warehouse_id(mut self, value: Uuid) -> Self {
        self.from_warehouse_id = Some(value);
        self
    }

    /// Set the from_location field (optional)
    pub fn from_location(mut self, value: String) -> Self {
        self.from_location = Some(value);
        self
    }

    /// Set the to_warehouse_id field (required)
    pub fn to_warehouse_id(mut self, value: Uuid) -> Self {
        self.to_warehouse_id = Some(value);
        self
    }

    /// Set the to_location field (optional)
    pub fn to_location(mut self, value: String) -> Self {
        self.to_location = Some(value);
        self
    }

    /// Set the to_provider_id field (optional)
    pub fn to_provider_id(mut self, value: Uuid) -> Self {
        self.to_provider_id = Some(value);
        self
    }

    /// Set the transfer_date field (required)
    pub fn transfer_date(mut self, value: NaiveDate) -> Self {
        self.transfer_date = Some(value);
        self
    }

    /// Set the expected_arrival_date field (optional)
    pub fn expected_arrival_date(mut self, value: NaiveDate) -> Self {
        self.expected_arrival_date = Some(value);
        self
    }

    /// Set the shipped_at field (optional)
    pub fn shipped_at(mut self, value: DateTime<Utc>) -> Self {
        self.shipped_at = Some(value);
        self
    }

    /// Set the received_at field (optional)
    pub fn received_at(mut self, value: DateTime<Utc>) -> Self {
        self.received_at = Some(value);
        self
    }

    /// Set the total_items field (default: `0`)
    pub fn total_items(mut self, value: i32) -> Self {
        self.total_items = Some(value);
        self
    }

    /// Set the total_quantity field (default: `Decimal::from(0)`)
    pub fn total_quantity(mut self, value: Decimal) -> Self {
        self.total_quantity = Some(value);
        self
    }

    /// Set the total_value field (default: `Decimal::from(0)`)
    pub fn total_value(mut self, value: Decimal) -> Self {
        self.total_value = Some(value);
        self
    }

    /// Set the currency field (default: `"IDR".to_string()`)
    pub fn currency(mut self, value: String) -> Self {
        self.currency = Some(value);
        self
    }

    /// Set the shipping_method field (optional)
    pub fn shipping_method(mut self, value: String) -> Self {
        self.shipping_method = Some(value);
        self
    }

    /// Set the carrier field (optional)
    pub fn carrier(mut self, value: String) -> Self {
        self.carrier = Some(value);
        self
    }

    /// Set the tracking_number field (optional)
    pub fn tracking_number(mut self, value: String) -> Self {
        self.tracking_number = Some(value);
        self
    }

    /// Set the shipping_cost field (optional)
    pub fn shipping_cost(mut self, value: Decimal) -> Self {
        self.shipping_cost = Some(value);
        self
    }

    /// Set the status field (default: `TransferStatus::default()`)
    pub fn status(mut self, value: TransferStatus) -> Self {
        self.status = Some(value);
        self
    }

    /// Set the requires_approval field (default: `false`)
    pub fn requires_approval(mut self, value: bool) -> Self {
        self.requires_approval = Some(value);
        self
    }

    /// Set the approved_by field (optional)
    pub fn approved_by(mut self, value: Uuid) -> Self {
        self.approved_by = Some(value);
        self
    }

    /// Set the approved_at field (optional)
    pub fn approved_at(mut self, value: DateTime<Utc>) -> Self {
        self.approved_at = Some(value);
        self
    }

    /// Set the requested_by field (optional)
    pub fn requested_by(mut self, value: Uuid) -> Self {
        self.requested_by = Some(value);
        self
    }

    /// Set the shipped_by field (optional)
    pub fn shipped_by(mut self, value: Uuid) -> Self {
        self.shipped_by = Some(value);
        self
    }

    /// Set the received_by field (optional)
    pub fn received_by(mut self, value: Uuid) -> Self {
        self.received_by = Some(value);
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

    /// Set the receiving_notes field (optional)
    pub fn receiving_notes(mut self, value: String) -> Self {
        self.receiving_notes = Some(value);
        self
    }

    /// Build the StockTransfer entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<StockTransfer, String> {
        let provider_id = self.provider_id.ok_or_else(|| "provider_id is required".to_string())?;
        let transfer_number = self.transfer_number.ok_or_else(|| "transfer_number is required".to_string())?;
        let from_warehouse_id = self.from_warehouse_id.ok_or_else(|| "from_warehouse_id is required".to_string())?;
        let to_warehouse_id = self.to_warehouse_id.ok_or_else(|| "to_warehouse_id is required".to_string())?;
        let transfer_date = self.transfer_date.ok_or_else(|| "transfer_date is required".to_string())?;

        Ok(StockTransfer {
            id: Uuid::new_v4(),
            provider_id,
            transfer_number,
            transfer_type: self.transfer_type.unwrap_or(TransferType::default()),
            from_warehouse_id,
            from_location: self.from_location,
            to_warehouse_id,
            to_location: self.to_location,
            to_provider_id: self.to_provider_id,
            transfer_date,
            expected_arrival_date: self.expected_arrival_date,
            shipped_at: self.shipped_at,
            received_at: self.received_at,
            total_items: self.total_items.unwrap_or(0),
            total_quantity: self.total_quantity.unwrap_or(Decimal::from(0)),
            total_value: self.total_value.unwrap_or(Decimal::from(0)),
            currency: self.currency.unwrap_or("IDR".to_string()),
            shipping_method: self.shipping_method,
            carrier: self.carrier,
            tracking_number: self.tracking_number,
            shipping_cost: self.shipping_cost,
            status: self.status.unwrap_or(TransferStatus::default()),
            requires_approval: self.requires_approval.unwrap_or(false),
            approved_by: self.approved_by,
            approved_at: self.approved_at,
            requested_by: self.requested_by,
            shipped_by: self.shipped_by,
            received_by: self.received_by,
            reason: self.reason,
            notes: self.notes,
            receiving_notes: self.receiving_notes,
            metadata: AuditMetadata::default(),
        })
    }
}
