use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use rust_decimal::Decimal;

use super::StorageAllowNewProduct;
use super::AuditMetadata;

/// Strongly-typed ID for StorageCategory
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StorageCategoryId(pub Uuid);

impl StorageCategoryId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for StorageCategoryId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for StorageCategoryId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for StorageCategoryId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<StorageCategoryId> for Uuid {
    fn from(id: StorageCategoryId) -> Self { id.0 }
}

impl AsRef<Uuid> for StorageCategoryId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for StorageCategoryId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct StorageCategory {
    pub id: Uuid,
    pub name: String,
    pub max_weight: Option<Decimal>,
    pub allow_new_product: StorageAllowNewProduct,
    pub active: bool,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl StorageCategory {
    /// Create a builder for StorageCategory
    pub fn builder() -> StorageCategoryBuilder {
        <StorageCategoryBuilder as Default>::default()
    }

    /// Create a new StorageCategory with required fields
    pub fn new(name: String, allow_new_product: StorageAllowNewProduct, active: bool) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            max_weight: None,
            allow_new_product,
            active,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> StorageCategoryId {
        StorageCategoryId(self.id)
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

    /// Set the max_weight field (chainable)
    pub fn with_max_weight(mut self, value: Decimal) -> Self {
        self.max_weight = Some(value);
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
                "max_weight" => {
                    if let Ok(v) = serde_json::from_value(value) { self.max_weight = v; }
                }
                "allow_new_product" => {
                    if let Ok(v) = serde_json::from_value(value) { self.allow_new_product = v; }
                }
                "active" => {
                    if let Ok(v) = serde_json::from_value(value) { self.active = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for StorageCategory {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "StorageCategory"
    }
}

impl backbone_core::PersistentEntity for StorageCategory {
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

impl backbone_orm::EntityRepoMeta for StorageCategory {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("allow_new_product".to_string(), "storage_allow_new_product".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["name"]
    }
}

/// Builder for StorageCategory entity
///
/// Provides a fluent API for constructing StorageCategory instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct StorageCategoryBuilder {
    name: Option<String>,
    max_weight: Option<Decimal>,
    allow_new_product: Option<StorageAllowNewProduct>,
    active: Option<bool>,
}

impl StorageCategoryBuilder {
    /// Set the name field (required)
    pub fn name(mut self, value: String) -> Self {
        self.name = Some(value);
        self
    }

    /// Set the max_weight field (optional)
    pub fn max_weight(mut self, value: Decimal) -> Self {
        self.max_weight = Some(value);
        self
    }

    /// Set the allow_new_product field (default: `StorageAllowNewProduct::default()`)
    pub fn allow_new_product(mut self, value: StorageAllowNewProduct) -> Self {
        self.allow_new_product = Some(value);
        self
    }

    /// Set the active field (default: `true`)
    pub fn active(mut self, value: bool) -> Self {
        self.active = Some(value);
        self
    }

    /// Build the StorageCategory entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<StorageCategory, String> {
        let name = self.name.ok_or_else(|| "name is required".to_string())?;

        Ok(StorageCategory {
            id: Uuid::new_v4(),
            name,
            max_weight: self.max_weight,
            allow_new_product: self.allow_new_product.unwrap_or_default(),
            active: self.active.unwrap_or(true),
            metadata: AuditMetadata::default(),
        })
    }
}
