use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use rust_decimal::Decimal;
use super::AuditMetadata;

/// Strongly-typed ID for DeliveryNoteItem
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DeliveryNoteItemId(pub Uuid);

impl DeliveryNoteItemId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for DeliveryNoteItemId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for DeliveryNoteItemId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for DeliveryNoteItemId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<DeliveryNoteItemId> for Uuid {
    fn from(id: DeliveryNoteItemId) -> Self { id.0 }
}

impl AsRef<Uuid> for DeliveryNoteItemId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for DeliveryNoteItemId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DeliveryNoteItem {
    pub id: Uuid,
    pub delivery_id: Uuid,
    pub company_id: Uuid,
    pub item_id: Uuid,
    pub quantity: Decimal,
    pub valuation_rate: Decimal,
    pub cogs_amount: Decimal,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl DeliveryNoteItem {
    /// Create a builder for DeliveryNoteItem
    pub fn builder() -> DeliveryNoteItemBuilder {
        DeliveryNoteItemBuilder::default()
    }

    /// Create a new DeliveryNoteItem with required fields
    pub fn new(delivery_id: Uuid, company_id: Uuid, item_id: Uuid, quantity: Decimal, valuation_rate: Decimal, cogs_amount: Decimal) -> Self {
        Self {
            id: Uuid::new_v4(),
            delivery_id,
            company_id,
            item_id,
            quantity,
            valuation_rate,
            cogs_amount,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> DeliveryNoteItemId {
        DeliveryNoteItemId(self.id)
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
                "delivery_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.delivery_id = v; }
                }
                "company_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.company_id = v; }
                }
                "item_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.item_id = v; }
                }
                "quantity" => {
                    if let Ok(v) = serde_json::from_value(value) { self.quantity = v; }
                }
                "valuation_rate" => {
                    if let Ok(v) = serde_json::from_value(value) { self.valuation_rate = v; }
                }
                "cogs_amount" => {
                    if let Ok(v) = serde_json::from_value(value) { self.cogs_amount = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for DeliveryNoteItem {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "DeliveryNoteItem"
    }
}

impl backbone_core::PersistentEntity for DeliveryNoteItem {
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

impl backbone_orm::EntityRepoMeta for DeliveryNoteItem {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("delivery_id".to_string(), "uuid".to_string());
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
        &[("delivery", "delivery_notes", "deliveryId")]
    }
}

/// Builder for DeliveryNoteItem entity
///
/// Provides a fluent API for constructing DeliveryNoteItem instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct DeliveryNoteItemBuilder {
    delivery_id: Option<Uuid>,
    company_id: Option<Uuid>,
    item_id: Option<Uuid>,
    quantity: Option<Decimal>,
    valuation_rate: Option<Decimal>,
    cogs_amount: Option<Decimal>,
}

impl DeliveryNoteItemBuilder {
    /// Set the delivery_id field (required)
    pub fn delivery_id(mut self, value: Uuid) -> Self {
        self.delivery_id = Some(value);
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

    /// Set the quantity field (required)
    pub fn quantity(mut self, value: Decimal) -> Self {
        self.quantity = Some(value);
        self
    }

    /// Set the valuation_rate field (default: `Decimal::from(0)`)
    pub fn valuation_rate(mut self, value: Decimal) -> Self {
        self.valuation_rate = Some(value);
        self
    }

    /// Set the cogs_amount field (default: `Decimal::from(0)`)
    pub fn cogs_amount(mut self, value: Decimal) -> Self {
        self.cogs_amount = Some(value);
        self
    }

    /// Build the DeliveryNoteItem entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<DeliveryNoteItem, String> {
        let delivery_id = self.delivery_id.ok_or_else(|| "delivery_id is required".to_string())?;
        let company_id = self.company_id.ok_or_else(|| "company_id is required".to_string())?;
        let item_id = self.item_id.ok_or_else(|| "item_id is required".to_string())?;
        let quantity = self.quantity.ok_or_else(|| "quantity is required".to_string())?;

        Ok(DeliveryNoteItem {
            id: Uuid::new_v4(),
            delivery_id,
            company_id,
            item_id,
            quantity,
            valuation_rate: self.valuation_rate.unwrap_or(Decimal::from(0)),
            cogs_amount: self.cogs_amount.unwrap_or(Decimal::from(0)),
            metadata: AuditMetadata::default(),
        })
    }
}
