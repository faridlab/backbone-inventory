use chrono::{NaiveDate};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use rust_decimal::Decimal;

use super::TransferItemStatus;

/// Strongly-typed ID for StockTransferItem
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StockTransferItemId(pub Uuid);

impl StockTransferItemId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for StockTransferItemId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for StockTransferItemId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for StockTransferItemId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<StockTransferItemId> for Uuid {
    fn from(id: StockTransferItemId) -> Self { id.0 }
}

impl AsRef<Uuid> for StockTransferItemId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for StockTransferItemId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct StockTransferItem {
    pub id: Uuid,
    pub transfer_id: Uuid,
    pub product_id: Uuid,
    pub requested_quantity: Decimal,
    pub shipped_quantity: Decimal,
    pub received_quantity: Decimal,
    pub rejected_quantity: Decimal,
    pub variance_quantity: Decimal,
    pub uom: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub batch_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub batch_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiry_date: Option<NaiveDate>,
    pub unit_cost: Decimal,
    pub total_cost: Decimal,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_bin: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_bin: Option<String>,
    pub status: TransferItemStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rejection_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    pub data: serde_json::Value,
}

impl StockTransferItem {
    /// Create a builder for StockTransferItem
    pub fn builder() -> StockTransferItemBuilder {
        StockTransferItemBuilder::default()
    }

    /// Create a new StockTransferItem with required fields
    pub fn new(transfer_id: Uuid, product_id: Uuid, requested_quantity: Decimal, shipped_quantity: Decimal, received_quantity: Decimal, rejected_quantity: Decimal, variance_quantity: Decimal, uom: String, unit_cost: Decimal, total_cost: Decimal, status: TransferItemStatus, data: serde_json::Value) -> Self {
        Self {
            id: Uuid::new_v4(),
            transfer_id,
            product_id,
            requested_quantity,
            shipped_quantity,
            received_quantity,
            rejected_quantity,
            variance_quantity,
            uom,
            batch_id: None,
            batch_number: None,
            expiry_date: None,
            unit_cost,
            total_cost,
            from_bin: None,
            to_bin: None,
            status,
            rejection_reason: None,
            notes: None,
            data,
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> StockTransferItemId {
        StockTransferItemId(self.id)
    }

    /// Get the current status
    pub fn status(&self) -> &TransferItemStatus {
        &self.status
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

    /// Set the from_bin field (chainable)
    pub fn with_from_bin(mut self, value: String) -> Self {
        self.from_bin = Some(value);
        self
    }

    /// Set the to_bin field (chainable)
    pub fn with_to_bin(mut self, value: String) -> Self {
        self.to_bin = Some(value);
        self
    }

    /// Set the rejection_reason field (chainable)
    pub fn with_rejection_reason(mut self, value: String) -> Self {
        self.rejection_reason = Some(value);
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
                "transfer_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.transfer_id = v; }
                }
                "product_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.product_id = v; }
                }
                "requested_quantity" => {
                    if let Ok(v) = serde_json::from_value(value) { self.requested_quantity = v; }
                }
                "shipped_quantity" => {
                    if let Ok(v) = serde_json::from_value(value) { self.shipped_quantity = v; }
                }
                "received_quantity" => {
                    if let Ok(v) = serde_json::from_value(value) { self.received_quantity = v; }
                }
                "rejected_quantity" => {
                    if let Ok(v) = serde_json::from_value(value) { self.rejected_quantity = v; }
                }
                "variance_quantity" => {
                    if let Ok(v) = serde_json::from_value(value) { self.variance_quantity = v; }
                }
                "uom" => {
                    if let Ok(v) = serde_json::from_value(value) { self.uom = v; }
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
                "from_bin" => {
                    if let Ok(v) = serde_json::from_value(value) { self.from_bin = v; }
                }
                "to_bin" => {
                    if let Ok(v) = serde_json::from_value(value) { self.to_bin = v; }
                }
                "status" => {
                    if let Ok(v) = serde_json::from_value(value) { self.status = v; }
                }
                "rejection_reason" => {
                    if let Ok(v) = serde_json::from_value(value) { self.rejection_reason = v; }
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

impl super::Entity for StockTransferItem {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "StockTransferItem"
    }
}

impl backbone_core::PersistentEntity for StockTransferItem {
    fn entity_id(&self) -> String {
        self.id.to_string()
    }
    fn set_entity_id(&mut self, id: String) {
        if let Ok(uuid) = uuid::Uuid::parse_str(&id) {
            self.id = uuid;
        }
    }
    fn created_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        None
    }
    fn set_created_at(&mut self, ts: chrono::DateTime<chrono::Utc>) {
        let _ = ts;
    }
    fn updated_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        None
    }
    fn set_updated_at(&mut self, ts: chrono::DateTime<chrono::Utc>) {
        let _ = ts;
    }
    fn deleted_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        None
    }
    fn set_deleted_at(&mut self, ts: Option<chrono::DateTime<chrono::Utc>>) {
        let _ = ts;
    }
}

impl backbone_orm::EntityRepoMeta for StockTransferItem {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("transfer_id".to_string(), "uuid".to_string());
        m.insert("product_id".to_string(), "uuid".to_string());
        m.insert("batch_id".to_string(), "uuid".to_string());
        m.insert("status".to_string(), "transfer_item_status".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["uom"]
    }
}

/// Builder for StockTransferItem entity
///
/// Provides a fluent API for constructing StockTransferItem instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct StockTransferItemBuilder {
    transfer_id: Option<Uuid>,
    product_id: Option<Uuid>,
    requested_quantity: Option<Decimal>,
    shipped_quantity: Option<Decimal>,
    received_quantity: Option<Decimal>,
    rejected_quantity: Option<Decimal>,
    variance_quantity: Option<Decimal>,
    uom: Option<String>,
    batch_id: Option<Uuid>,
    batch_number: Option<String>,
    expiry_date: Option<NaiveDate>,
    unit_cost: Option<Decimal>,
    total_cost: Option<Decimal>,
    from_bin: Option<String>,
    to_bin: Option<String>,
    status: Option<TransferItemStatus>,
    rejection_reason: Option<String>,
    notes: Option<String>,
    data: Option<serde_json::Value>,
}

impl StockTransferItemBuilder {
    /// Set the transfer_id field (required)
    pub fn transfer_id(mut self, value: Uuid) -> Self {
        self.transfer_id = Some(value);
        self
    }

    /// Set the product_id field (required)
    pub fn product_id(mut self, value: Uuid) -> Self {
        self.product_id = Some(value);
        self
    }

    /// Set the requested_quantity field (required)
    pub fn requested_quantity(mut self, value: Decimal) -> Self {
        self.requested_quantity = Some(value);
        self
    }

    /// Set the shipped_quantity field (default: `Decimal::from(0)`)
    pub fn shipped_quantity(mut self, value: Decimal) -> Self {
        self.shipped_quantity = Some(value);
        self
    }

    /// Set the received_quantity field (default: `Decimal::from(0)`)
    pub fn received_quantity(mut self, value: Decimal) -> Self {
        self.received_quantity = Some(value);
        self
    }

    /// Set the rejected_quantity field (default: `Decimal::from(0)`)
    pub fn rejected_quantity(mut self, value: Decimal) -> Self {
        self.rejected_quantity = Some(value);
        self
    }

    /// Set the variance_quantity field (required)
    pub fn variance_quantity(mut self, value: Decimal) -> Self {
        self.variance_quantity = Some(value);
        self
    }

    /// Set the uom field (required)
    pub fn uom(mut self, value: String) -> Self {
        self.uom = Some(value);
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

    /// Set the from_bin field (optional)
    pub fn from_bin(mut self, value: String) -> Self {
        self.from_bin = Some(value);
        self
    }

    /// Set the to_bin field (optional)
    pub fn to_bin(mut self, value: String) -> Self {
        self.to_bin = Some(value);
        self
    }

    /// Set the status field (default: `TransferItemStatus::default()`)
    pub fn status(mut self, value: TransferItemStatus) -> Self {
        self.status = Some(value);
        self
    }

    /// Set the rejection_reason field (optional)
    pub fn rejection_reason(mut self, value: String) -> Self {
        self.rejection_reason = Some(value);
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

    /// Build the StockTransferItem entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<StockTransferItem, String> {
        let transfer_id = self.transfer_id.ok_or_else(|| "transfer_id is required".to_string())?;
        let product_id = self.product_id.ok_or_else(|| "product_id is required".to_string())?;
        let requested_quantity = self.requested_quantity.ok_or_else(|| "requested_quantity is required".to_string())?;
        let variance_quantity = self.variance_quantity.ok_or_else(|| "variance_quantity is required".to_string())?;
        let uom = self.uom.ok_or_else(|| "uom is required".to_string())?;
        let unit_cost = self.unit_cost.ok_or_else(|| "unit_cost is required".to_string())?;
        let total_cost = self.total_cost.ok_or_else(|| "total_cost is required".to_string())?;

        Ok(StockTransferItem {
            id: Uuid::new_v4(),
            transfer_id,
            product_id,
            requested_quantity,
            shipped_quantity: self.shipped_quantity.unwrap_or(Decimal::from(0)),
            received_quantity: self.received_quantity.unwrap_or(Decimal::from(0)),
            rejected_quantity: self.rejected_quantity.unwrap_or(Decimal::from(0)),
            variance_quantity,
            uom,
            batch_id: self.batch_id,
            batch_number: self.batch_number,
            expiry_date: self.expiry_date,
            unit_cost,
            total_cost,
            from_bin: self.from_bin,
            to_bin: self.to_bin,
            status: self.status.unwrap_or(TransferItemStatus::default()),
            rejection_reason: self.rejection_reason,
            notes: self.notes,
            data: self.data.unwrap_or(serde_json::json!({})),
        })
    }
}
