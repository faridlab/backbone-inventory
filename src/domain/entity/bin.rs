use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use rust_decimal::Decimal;
use super::AuditMetadata;

/// Strongly-typed ID for Bin
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BinId(pub Uuid);

impl BinId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for BinId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for BinId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for BinId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<BinId> for Uuid {
    fn from(id: BinId) -> Self { id.0 }
}

impl AsRef<Uuid> for BinId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for BinId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Bin {
    pub id: Uuid,
    pub company_id: Uuid,
    pub item_id: Uuid,
    pub warehouse_id: Uuid,
    pub actual_qty: Decimal,
    pub reserved_qty: Decimal,
    pub valuation_rate: Decimal,
    pub stock_value: Decimal,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl Bin {
    /// Create a builder for Bin
    pub fn builder() -> BinBuilder {
        BinBuilder::default()
    }

    /// Create a new Bin with required fields
    pub fn new(company_id: Uuid, item_id: Uuid, warehouse_id: Uuid, actual_qty: Decimal, reserved_qty: Decimal, valuation_rate: Decimal, stock_value: Decimal) -> Self {
        Self {
            id: Uuid::new_v4(),
            company_id,
            item_id,
            warehouse_id,
            actual_qty,
            reserved_qty,
            valuation_rate,
            stock_value,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> BinId {
        BinId(self.id)
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
                "actual_qty" => {
                    if let Ok(v) = serde_json::from_value(value) { self.actual_qty = v; }
                }
                "reserved_qty" => {
                    if let Ok(v) = serde_json::from_value(value) { self.reserved_qty = v; }
                }
                "valuation_rate" => {
                    if let Ok(v) = serde_json::from_value(value) { self.valuation_rate = v; }
                }
                "stock_value" => {
                    if let Ok(v) = serde_json::from_value(value) { self.stock_value = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for Bin {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "Bin"
    }
}

impl backbone_core::PersistentEntity for Bin {
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

impl backbone_orm::EntityRepoMeta for Bin {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("company_id".to_string(), "uuid".to_string());
        m.insert("item_id".to_string(), "uuid".to_string());
        m.insert("warehouse_id".to_string(), "uuid".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &[]
    }
}

/// Builder for Bin entity
///
/// Provides a fluent API for constructing Bin instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct BinBuilder {
    company_id: Option<Uuid>,
    item_id: Option<Uuid>,
    warehouse_id: Option<Uuid>,
    actual_qty: Option<Decimal>,
    reserved_qty: Option<Decimal>,
    valuation_rate: Option<Decimal>,
    stock_value: Option<Decimal>,
}

impl BinBuilder {
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

    /// Set the actual_qty field (default: `Decimal::from(0)`)
    pub fn actual_qty(mut self, value: Decimal) -> Self {
        self.actual_qty = Some(value);
        self
    }

    /// Set the reserved_qty field (default: `Decimal::from(0)`)
    pub fn reserved_qty(mut self, value: Decimal) -> Self {
        self.reserved_qty = Some(value);
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

    /// Build the Bin entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<Bin, String> {
        let company_id = self.company_id.ok_or_else(|| "company_id is required".to_string())?;
        let item_id = self.item_id.ok_or_else(|| "item_id is required".to_string())?;
        let warehouse_id = self.warehouse_id.ok_or_else(|| "warehouse_id is required".to_string())?;

        Ok(Bin {
            id: Uuid::new_v4(),
            company_id,
            item_id,
            warehouse_id,
            actual_qty: self.actual_qty.unwrap_or(Decimal::from(0)),
            reserved_qty: self.reserved_qty.unwrap_or(Decimal::from(0)),
            valuation_rate: self.valuation_rate.unwrap_or(Decimal::from(0)),
            stock_value: self.stock_value.unwrap_or(Decimal::from(0)),
            metadata: AuditMetadata::default(),
        })
    }
}
