use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use rust_decimal::Decimal;
use super::AuditMetadata;

/// Strongly-typed ID for StockReconciliationItem
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StockReconciliationItemId(pub Uuid);

impl StockReconciliationItemId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for StockReconciliationItemId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for StockReconciliationItemId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for StockReconciliationItemId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<StockReconciliationItemId> for Uuid {
    fn from(id: StockReconciliationItemId) -> Self { id.0 }
}

impl AsRef<Uuid> for StockReconciliationItemId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for StockReconciliationItemId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct StockReconciliationItem {
    pub id: Uuid,
    pub reconciliation_id: Uuid,
    pub company_id: Uuid,
    pub item_id: Uuid,
    pub counted_qty: Decimal,
    pub counted_rate: Decimal,
    pub qty_difference: Decimal,
    pub value_difference: Decimal,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl StockReconciliationItem {
    /// Create a builder for StockReconciliationItem
    pub fn builder() -> StockReconciliationItemBuilder {
        StockReconciliationItemBuilder::default()
    }

    /// Create a new StockReconciliationItem with required fields
    pub fn new(reconciliation_id: Uuid, company_id: Uuid, item_id: Uuid, counted_qty: Decimal, counted_rate: Decimal, qty_difference: Decimal, value_difference: Decimal) -> Self {
        Self {
            id: Uuid::new_v4(),
            reconciliation_id,
            company_id,
            item_id,
            counted_qty,
            counted_rate,
            qty_difference,
            value_difference,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> StockReconciliationItemId {
        StockReconciliationItemId(self.id)
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
                "reconciliation_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.reconciliation_id = v; }
                }
                "company_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.company_id = v; }
                }
                "item_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.item_id = v; }
                }
                "counted_qty" => {
                    if let Ok(v) = serde_json::from_value(value) { self.counted_qty = v; }
                }
                "counted_rate" => {
                    if let Ok(v) = serde_json::from_value(value) { self.counted_rate = v; }
                }
                "qty_difference" => {
                    if let Ok(v) = serde_json::from_value(value) { self.qty_difference = v; }
                }
                "value_difference" => {
                    if let Ok(v) = serde_json::from_value(value) { self.value_difference = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for StockReconciliationItem {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "StockReconciliationItem"
    }
}

impl backbone_core::PersistentEntity for StockReconciliationItem {
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

impl backbone_orm::EntityRepoMeta for StockReconciliationItem {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("reconciliation_id".to_string(), "uuid".to_string());
        m.insert("company_id".to_string(), "uuid".to_string());
        m.insert("item_id".to_string(), "uuid".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &[]
    }
    fn company_field() -> Option<&'static str> {
        Some("company_id")
    }
    fn relations() -> &'static [(&'static str, &'static str, &'static str)] {
        &[("reconciliation", "stock_reconciliations", "reconciliationId")]
    }
}

/// Builder for StockReconciliationItem entity
///
/// Provides a fluent API for constructing StockReconciliationItem instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct StockReconciliationItemBuilder {
    reconciliation_id: Option<Uuid>,
    company_id: Option<Uuid>,
    item_id: Option<Uuid>,
    counted_qty: Option<Decimal>,
    counted_rate: Option<Decimal>,
    qty_difference: Option<Decimal>,
    value_difference: Option<Decimal>,
}

impl StockReconciliationItemBuilder {
    /// Set the reconciliation_id field (required)
    pub fn reconciliation_id(mut self, value: Uuid) -> Self {
        self.reconciliation_id = Some(value);
        self
    }

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

    /// Set the counted_qty field (required)
    pub fn counted_qty(mut self, value: Decimal) -> Self {
        self.counted_qty = Some(value);
        self
    }

    /// Set the counted_rate field (default: `Decimal::from(0)`)
    pub fn counted_rate(mut self, value: Decimal) -> Self {
        self.counted_rate = Some(value);
        self
    }

    /// Set the qty_difference field (default: `Decimal::from(0)`)
    pub fn qty_difference(mut self, value: Decimal) -> Self {
        self.qty_difference = Some(value);
        self
    }

    /// Set the value_difference field (default: `Decimal::from(0)`)
    pub fn value_difference(mut self, value: Decimal) -> Self {
        self.value_difference = Some(value);
        self
    }

    /// Build the StockReconciliationItem entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<StockReconciliationItem, String> {
        let reconciliation_id = self.reconciliation_id.ok_or_else(|| "reconciliation_id is required".to_string())?;
        let company_id = self.company_id.ok_or_else(|| "company_id is required".to_string())?;
        let item_id = self.item_id.ok_or_else(|| "item_id is required".to_string())?;
        let counted_qty = self.counted_qty.ok_or_else(|| "counted_qty is required".to_string())?;

        Ok(StockReconciliationItem {
            id: Uuid::new_v4(),
            reconciliation_id,
            company_id,
            item_id,
            counted_qty,
            counted_rate: self.counted_rate.unwrap_or(Decimal::from(0)),
            qty_difference: self.qty_difference.unwrap_or(Decimal::from(0)),
            value_difference: self.value_difference.unwrap_or(Decimal::from(0)),
            metadata: AuditMetadata::default(),
        })
    }
}
