use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use rust_decimal::Decimal;
use super::AuditMetadata;

/// Strongly-typed ID for Stock
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StockId(pub Uuid);

impl StockId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for StockId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for StockId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for StockId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<StockId> for Uuid {
    fn from(id: StockId) -> Self { id.0 }
}

impl AsRef<Uuid> for StockId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for StockId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Stock {
    pub id: Uuid,
    pub provider_id: Uuid,
    pub product_id: Uuid,
    pub warehouse_id: Uuid,
    pub quantity_on_hand: Decimal,
    pub quantity_reserved: Decimal,
    pub quantity_on_order: Decimal,
    pub quantity_in_transit: Decimal,
    pub quantity_quarantine: Decimal,
    pub quantity_damaged: Decimal,
    pub currency: String,
    pub unit_cost: Decimal,
    pub total_value: Decimal,
    pub data: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bin_location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl Stock {
    /// Create a builder for Stock
    pub fn builder() -> StockBuilder {
        StockBuilder::default()
    }

    /// Create a new Stock with required fields
    pub fn new(provider_id: Uuid, product_id: Uuid, warehouse_id: Uuid, quantity_on_hand: Decimal, quantity_reserved: Decimal, quantity_on_order: Decimal, quantity_in_transit: Decimal, quantity_quarantine: Decimal, quantity_damaged: Decimal, currency: String, unit_cost: Decimal, total_value: Decimal, data: serde_json::Value) -> Self {
        Self {
            id: Uuid::new_v4(),
            provider_id,
            product_id,
            warehouse_id,
            quantity_on_hand,
            quantity_reserved,
            quantity_on_order,
            quantity_in_transit,
            quantity_quarantine,
            quantity_damaged,
            currency,
            unit_cost,
            total_value,
            data,
            bin_location: None,
            notes: None,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> StockId {
        StockId(self.id)
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
                "quantity_on_hand" => {
                    if let Ok(v) = serde_json::from_value(value) { self.quantity_on_hand = v; }
                }
                "quantity_reserved" => {
                    if let Ok(v) = serde_json::from_value(value) { self.quantity_reserved = v; }
                }
                "quantity_on_order" => {
                    if let Ok(v) = serde_json::from_value(value) { self.quantity_on_order = v; }
                }
                "quantity_in_transit" => {
                    if let Ok(v) = serde_json::from_value(value) { self.quantity_in_transit = v; }
                }
                "quantity_quarantine" => {
                    if let Ok(v) = serde_json::from_value(value) { self.quantity_quarantine = v; }
                }
                "quantity_damaged" => {
                    if let Ok(v) = serde_json::from_value(value) { self.quantity_damaged = v; }
                }
                "currency" => {
                    if let Ok(v) = serde_json::from_value(value) { self.currency = v; }
                }
                "unit_cost" => {
                    if let Ok(v) = serde_json::from_value(value) { self.unit_cost = v; }
                }
                "total_value" => {
                    if let Ok(v) = serde_json::from_value(value) { self.total_value = v; }
                }
                "data" => {
                    if let Ok(v) = serde_json::from_value(value) { self.data = v; }
                }
                "bin_location" => {
                    if let Ok(v) = serde_json::from_value(value) { self.bin_location = v; }
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

impl super::Entity for Stock {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "Stock"
    }
}

impl backbone_core::PersistentEntity for Stock {
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

impl backbone_orm::EntityRepoMeta for Stock {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("provider_id".to_string(), "uuid".to_string());
        m.insert("product_id".to_string(), "uuid".to_string());
        m.insert("warehouse_id".to_string(), "uuid".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["currency"]
    }
}

/// Builder for Stock entity
///
/// Provides a fluent API for constructing Stock instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct StockBuilder {
    provider_id: Option<Uuid>,
    product_id: Option<Uuid>,
    warehouse_id: Option<Uuid>,
    quantity_on_hand: Option<Decimal>,
    quantity_reserved: Option<Decimal>,
    quantity_on_order: Option<Decimal>,
    quantity_in_transit: Option<Decimal>,
    quantity_quarantine: Option<Decimal>,
    quantity_damaged: Option<Decimal>,
    currency: Option<String>,
    unit_cost: Option<Decimal>,
    total_value: Option<Decimal>,
    data: Option<serde_json::Value>,
    bin_location: Option<String>,
    notes: Option<String>,
}

impl StockBuilder {
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

    /// Set the warehouse_id field (required)
    pub fn warehouse_id(mut self, value: Uuid) -> Self {
        self.warehouse_id = Some(value);
        self
    }

    /// Set the quantity_on_hand field (default: `Decimal::from(0)`)
    pub fn quantity_on_hand(mut self, value: Decimal) -> Self {
        self.quantity_on_hand = Some(value);
        self
    }

    /// Set the quantity_reserved field (default: `Decimal::from(0)`)
    pub fn quantity_reserved(mut self, value: Decimal) -> Self {
        self.quantity_reserved = Some(value);
        self
    }

    /// Set the quantity_on_order field (default: `Decimal::from(0)`)
    pub fn quantity_on_order(mut self, value: Decimal) -> Self {
        self.quantity_on_order = Some(value);
        self
    }

    /// Set the quantity_in_transit field (default: `Decimal::from(0)`)
    pub fn quantity_in_transit(mut self, value: Decimal) -> Self {
        self.quantity_in_transit = Some(value);
        self
    }

    /// Set the quantity_quarantine field (default: `Decimal::from(0)`)
    pub fn quantity_quarantine(mut self, value: Decimal) -> Self {
        self.quantity_quarantine = Some(value);
        self
    }

    /// Set the quantity_damaged field (default: `Decimal::from(0)`)
    pub fn quantity_damaged(mut self, value: Decimal) -> Self {
        self.quantity_damaged = Some(value);
        self
    }

    /// Set the currency field (default: `"IDR".to_string()`)
    pub fn currency(mut self, value: String) -> Self {
        self.currency = Some(value);
        self
    }

    /// Set the unit_cost field (default: `Decimal::from(0)`)
    pub fn unit_cost(mut self, value: Decimal) -> Self {
        self.unit_cost = Some(value);
        self
    }

    /// Set the total_value field (default: `Decimal::from(0)`)
    pub fn total_value(mut self, value: Decimal) -> Self {
        self.total_value = Some(value);
        self
    }

    /// Set the data field (default: `serde_json::json!({"quantity_available":0,"is_below_min":false,"is_above_max":false,"reorder_needed":false,"reorder_quantity":null,"has_expiring_stock":false,"earliest_expiry_date":null,"quantity_expiring_soon":null,"batch_count":0,"last_receipt_date":null,"last_receipt_quantity":null,"last_issue_date":null,"last_issue_quantity":null,"last_count_date":null,"last_count_quantity":null,"avg_daily_usage":null,"days_of_stock":null})`)
    pub fn data(mut self, value: serde_json::Value) -> Self {
        self.data = Some(value);
        self
    }

    /// Set the bin_location field (optional)
    pub fn bin_location(mut self, value: String) -> Self {
        self.bin_location = Some(value);
        self
    }

    /// Set the notes field (optional)
    pub fn notes(mut self, value: String) -> Self {
        self.notes = Some(value);
        self
    }

    /// Build the Stock entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<Stock, String> {
        let provider_id = self.provider_id.ok_or_else(|| "provider_id is required".to_string())?;
        let product_id = self.product_id.ok_or_else(|| "product_id is required".to_string())?;
        let warehouse_id = self.warehouse_id.ok_or_else(|| "warehouse_id is required".to_string())?;

        Ok(Stock {
            id: Uuid::new_v4(),
            provider_id,
            product_id,
            warehouse_id,
            quantity_on_hand: self.quantity_on_hand.unwrap_or(Decimal::from(0)),
            quantity_reserved: self.quantity_reserved.unwrap_or(Decimal::from(0)),
            quantity_on_order: self.quantity_on_order.unwrap_or(Decimal::from(0)),
            quantity_in_transit: self.quantity_in_transit.unwrap_or(Decimal::from(0)),
            quantity_quarantine: self.quantity_quarantine.unwrap_or(Decimal::from(0)),
            quantity_damaged: self.quantity_damaged.unwrap_or(Decimal::from(0)),
            currency: self.currency.unwrap_or("IDR".to_string()),
            unit_cost: self.unit_cost.unwrap_or(Decimal::from(0)),
            total_value: self.total_value.unwrap_or(Decimal::from(0)),
            data: self.data.unwrap_or(serde_json::json!({"quantity_available":0,"is_below_min":false,"is_above_max":false,"reorder_needed":false,"reorder_quantity":null,"has_expiring_stock":false,"earliest_expiry_date":null,"quantity_expiring_soon":null,"batch_count":0,"last_receipt_date":null,"last_receipt_quantity":null,"last_issue_date":null,"last_issue_quantity":null,"last_count_date":null,"last_count_quantity":null,"avg_daily_usage":null,"days_of_stock":null})),
            bin_location: self.bin_location,
            notes: self.notes,
            metadata: AuditMetadata::default(),
        })
    }
}
