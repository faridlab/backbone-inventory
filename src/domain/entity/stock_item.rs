use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use rust_decimal::Decimal;

use super::ValuationMethod;
use super::AuditMetadata;

/// Strongly-typed ID for StockItem
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StockItemId(pub Uuid);

impl StockItemId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for StockItemId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for StockItemId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for StockItemId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<StockItemId> for Uuid {
    fn from(id: StockItemId) -> Self { id.0 }
}

impl AsRef<Uuid> for StockItemId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for StockItemId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct StockItem {
    pub id: Uuid,
    pub item_id: Uuid,
    pub company_id: Uuid,
    pub stock_uom: String,
    pub is_stock_item: bool,
    pub has_batch: bool,
    pub valuation_method: ValuationMethod,
    pub reorder_level: Decimal,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl StockItem {
    /// Create a builder for StockItem
    pub fn builder() -> StockItemBuilder {
        StockItemBuilder::default()
    }

    /// Create a new StockItem with required fields
    pub fn new(item_id: Uuid, company_id: Uuid, stock_uom: String, is_stock_item: bool, has_batch: bool, valuation_method: ValuationMethod, reorder_level: Decimal) -> Self {
        Self {
            id: Uuid::new_v4(),
            item_id,
            company_id,
            stock_uom,
            is_stock_item,
            has_batch,
            valuation_method,
            reorder_level,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> StockItemId {
        StockItemId(self.id)
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
                "item_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.item_id = v; }
                }
                "company_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.company_id = v; }
                }
                "stock_uom" => {
                    if let Ok(v) = serde_json::from_value(value) { self.stock_uom = v; }
                }
                "is_stock_item" => {
                    if let Ok(v) = serde_json::from_value(value) { self.is_stock_item = v; }
                }
                "has_batch" => {
                    if let Ok(v) = serde_json::from_value(value) { self.has_batch = v; }
                }
                "valuation_method" => {
                    if let Ok(v) = serde_json::from_value(value) { self.valuation_method = v; }
                }
                "reorder_level" => {
                    if let Ok(v) = serde_json::from_value(value) { self.reorder_level = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for StockItem {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "StockItem"
    }
}

impl backbone_core::PersistentEntity for StockItem {
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

impl backbone_orm::EntityRepoMeta for StockItem {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("item_id".to_string(), "uuid".to_string());
        m.insert("company_id".to_string(), "uuid".to_string());
        m.insert("valuation_method".to_string(), "valuation_method".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["stock_uom"]
    }
    fn company_field() -> Option<&'static str> {
        Some("company_id")
    }
}

/// Builder for StockItem entity
///
/// Provides a fluent API for constructing StockItem instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct StockItemBuilder {
    item_id: Option<Uuid>,
    company_id: Option<Uuid>,
    stock_uom: Option<String>,
    is_stock_item: Option<bool>,
    has_batch: Option<bool>,
    valuation_method: Option<ValuationMethod>,
    reorder_level: Option<Decimal>,
}

impl StockItemBuilder {
    /// Set the item_id field (required)
    pub fn item_id(mut self, value: Uuid) -> Self {
        self.item_id = Some(value);
        self
    }

    /// Set the company_id field (required)
    pub fn company_id(mut self, value: Uuid) -> Self {
        self.company_id = Some(value);
        self
    }

    /// Set the stock_uom field (required)
    pub fn stock_uom(mut self, value: String) -> Self {
        self.stock_uom = Some(value);
        self
    }

    /// Set the is_stock_item field (default: `true`)
    pub fn is_stock_item(mut self, value: bool) -> Self {
        self.is_stock_item = Some(value);
        self
    }

    /// Set the has_batch field (default: `false`)
    pub fn has_batch(mut self, value: bool) -> Self {
        self.has_batch = Some(value);
        self
    }

    /// Set the valuation_method field (default: `ValuationMethod::default()`)
    pub fn valuation_method(mut self, value: ValuationMethod) -> Self {
        self.valuation_method = Some(value);
        self
    }

    /// Set the reorder_level field (default: `Decimal::from(0)`)
    pub fn reorder_level(mut self, value: Decimal) -> Self {
        self.reorder_level = Some(value);
        self
    }

    /// Build the StockItem entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<StockItem, String> {
        let item_id = self.item_id.ok_or_else(|| "item_id is required".to_string())?;
        let company_id = self.company_id.ok_or_else(|| "company_id is required".to_string())?;
        let stock_uom = self.stock_uom.ok_or_else(|| "stock_uom is required".to_string())?;

        Ok(StockItem {
            id: Uuid::new_v4(),
            item_id,
            company_id,
            stock_uom,
            is_stock_item: self.is_stock_item.unwrap_or(true),
            has_batch: self.has_batch.unwrap_or(false),
            valuation_method: self.valuation_method.unwrap_or(ValuationMethod::default()),
            reorder_level: self.reorder_level.unwrap_or(Decimal::from(0)),
            metadata: AuditMetadata::default(),
        })
    }
}
