use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use rust_decimal::Decimal;
use super::AuditMetadata;

/// Strongly-typed ID for Lot
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LotId(pub Uuid);

impl LotId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for LotId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for LotId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for LotId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<LotId> for Uuid {
    fn from(id: LotId) -> Self { id.0 }
}

impl AsRef<Uuid> for LotId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for LotId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Lot {
    pub id: Uuid,
    pub name: String,
    pub reference: Option<String>,
    pub item_id: Uuid,
    pub product_qty: Decimal,
    pub location_id: Option<Uuid>,
    pub note: Option<String>,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl Lot {
    /// Create a builder for Lot
    pub fn builder() -> LotBuilder {
        <LotBuilder as Default>::default()
    }

    /// Create a new Lot with required fields
    pub fn new(name: String, item_id: Uuid, product_qty: Decimal) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            reference: None,
            item_id,
            product_qty,
            location_id: None,
            note: None,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> LotId {
        LotId(self.id)
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

    /// Set the reference field (chainable)
    pub fn with_reference(mut self, value: String) -> Self {
        self.reference = Some(value);
        self
    }

    /// Set the location_id field (chainable)
    pub fn with_location_id(mut self, value: Uuid) -> Self {
        self.location_id = Some(value);
        self
    }

    /// Set the note field (chainable)
    pub fn with_note(mut self, value: String) -> Self {
        self.note = Some(value);
        self
    }

    // ==========================================================
    // Partial Update
    // ==========================================================

    /// Apply partial updates from a map of field name to JSON value
    pub fn apply_patch(&mut self, fields: std::collections::HashMap<String, serde_json::Value>) {
        for (key, value) in fields {
            match key.as_str() {
                "name" => {
                    if let Ok(v) = serde_json::from_value(value) { self.name = v; }
                }
                "reference" => {
                    if let Ok(v) = serde_json::from_value(value) { self.reference = v; }
                }
                "item_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.item_id = v; }
                }
                "product_qty" => {
                    if let Ok(v) = serde_json::from_value(value) { self.product_qty = v; }
                }
                "location_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.location_id = v; }
                }
                "note" => {
                    if let Ok(v) = serde_json::from_value(value) { self.note = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for Lot {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "Lot"
    }
}

impl backbone_core::PersistentEntity for Lot {
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

impl backbone_orm::EntityRepoMeta for Lot {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("item_id".to_string(), "uuid".to_string());
        m.insert("location_id".to_string(), "uuid".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["name"]
    }
}

/// Builder for Lot entity
///
/// Provides a fluent API for constructing Lot instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct LotBuilder {
    name: Option<String>,
    reference: Option<String>,
    item_id: Option<Uuid>,
    product_qty: Option<Decimal>,
    location_id: Option<Uuid>,
    note: Option<String>,
}

impl LotBuilder {
    /// Set the name field (required)
    pub fn name(mut self, value: String) -> Self {
        self.name = Some(value);
        self
    }

    /// Set the reference field (optional)
    pub fn reference(mut self, value: String) -> Self {
        self.reference = Some(value);
        self
    }

    /// Set the item_id field (required)
    pub fn item_id(mut self, value: Uuid) -> Self {
        self.item_id = Some(value);
        self
    }

    /// Set the product_qty field (default: `Decimal::from(0)`)
    pub fn product_qty(mut self, value: Decimal) -> Self {
        self.product_qty = Some(value);
        self
    }

    /// Set the location_id field (optional)
    pub fn location_id(mut self, value: Uuid) -> Self {
        self.location_id = Some(value);
        self
    }

    /// Set the note field (optional)
    pub fn note(mut self, value: String) -> Self {
        self.note = Some(value);
        self
    }

    /// Build the Lot entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<Lot, String> {
        let name = self.name.ok_or_else(|| "name is required".to_string())?;
        let item_id = self.item_id.ok_or_else(|| "item_id is required".to_string())?;

        Ok(Lot {
            id: Uuid::new_v4(),
            name,
            reference: self.reference,
            item_id,
            product_qty: self.product_qty.unwrap_or(Decimal::from(0)),
            location_id: self.location_id,
            note: self.note,
            metadata: AuditMetadata::default(),
        })
    }
}
