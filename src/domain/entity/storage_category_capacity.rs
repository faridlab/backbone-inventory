use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use rust_decimal::Decimal;
use super::AuditMetadata;

/// Strongly-typed ID for StorageCategoryCapacity
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StorageCategoryCapacityId(pub Uuid);

impl StorageCategoryCapacityId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for StorageCategoryCapacityId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for StorageCategoryCapacityId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for StorageCategoryCapacityId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<StorageCategoryCapacityId> for Uuid {
    fn from(id: StorageCategoryCapacityId) -> Self { id.0 }
}

impl AsRef<Uuid> for StorageCategoryCapacityId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for StorageCategoryCapacityId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct StorageCategoryCapacity {
    pub id: Uuid,
    pub storage_category_id: Uuid,
    pub item_id: Option<Uuid>,
    pub package_type_id: Option<Uuid>,
    pub quantity: Decimal,
    pub company_id: Option<Uuid>,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl StorageCategoryCapacity {
    /// Create a builder for StorageCategoryCapacity
    pub fn builder() -> StorageCategoryCapacityBuilder {
        <StorageCategoryCapacityBuilder as Default>::default()
    }

    /// Create a new StorageCategoryCapacity with required fields
    pub fn new(storage_category_id: Uuid, quantity: Decimal) -> Self {
        Self {
            id: Uuid::new_v4(),
            storage_category_id,
            item_id: None,
            package_type_id: None,
            quantity,
            company_id: None,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> StorageCategoryCapacityId {
        StorageCategoryCapacityId(self.id)
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

    /// Set the item_id field (chainable)
    pub fn with_item_id(mut self, value: Uuid) -> Self {
        self.item_id = Some(value);
        self
    }

    /// Set the package_type_id field (chainable)
    pub fn with_package_type_id(mut self, value: Uuid) -> Self {
        self.package_type_id = Some(value);
        self
    }

    /// Set the company_id field (chainable)
    pub fn with_company_id(mut self, value: Uuid) -> Self {
        self.company_id = Some(value);
        self
    }

    // ==========================================================
    // Partial Update
    // ==========================================================

    /// Apply partial updates from a map of field name to JSON value
    pub fn apply_patch(&mut self, fields: std::collections::HashMap<String, serde_json::Value>) {
        for (key, value) in fields {
            match key.as_str() {
                "storage_category_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.storage_category_id = v; }
                }
                "item_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.item_id = v; }
                }
                "package_type_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.package_type_id = v; }
                }
                "quantity" => {
                    if let Ok(v) = serde_json::from_value(value) { self.quantity = v; }
                }
                "company_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.company_id = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for StorageCategoryCapacity {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "StorageCategoryCapacity"
    }
}

impl backbone_core::PersistentEntity for StorageCategoryCapacity {
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

impl backbone_orm::EntityRepoMeta for StorageCategoryCapacity {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("storage_category_id".to_string(), "uuid".to_string());
        m.insert("item_id".to_string(), "uuid".to_string());
        m.insert("package_type_id".to_string(), "uuid".to_string());
        m.insert("company_id".to_string(), "uuid".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &[]
    }
    fn company_field() -> Option<&'static str> {
        Some("company_id")
    }
}

/// Builder for StorageCategoryCapacity entity
///
/// Provides a fluent API for constructing StorageCategoryCapacity instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct StorageCategoryCapacityBuilder {
    storage_category_id: Option<Uuid>,
    item_id: Option<Uuid>,
    package_type_id: Option<Uuid>,
    quantity: Option<Decimal>,
    company_id: Option<Uuid>,
}

impl StorageCategoryCapacityBuilder {
    /// Set the storage_category_id field (required)
    pub fn storage_category_id(mut self, value: Uuid) -> Self {
        self.storage_category_id = Some(value);
        self
    }

    /// Set the item_id field (optional)
    pub fn item_id(mut self, value: Uuid) -> Self {
        self.item_id = Some(value);
        self
    }

    /// Set the package_type_id field (optional)
    pub fn package_type_id(mut self, value: Uuid) -> Self {
        self.package_type_id = Some(value);
        self
    }

    /// Set the quantity field (default: `Decimal::from(1)`)
    pub fn quantity(mut self, value: Decimal) -> Self {
        self.quantity = Some(value);
        self
    }

    /// Set the company_id field (optional)
    pub fn company_id(mut self, value: Uuid) -> Self {
        self.company_id = Some(value);
        self
    }

    /// Build the StorageCategoryCapacity entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<StorageCategoryCapacity, String> {
        let storage_category_id = self.storage_category_id.ok_or_else(|| "storage_category_id is required".to_string())?;

        Ok(StorageCategoryCapacity {
            id: Uuid::new_v4(),
            storage_category_id,
            item_id: self.item_id,
            package_type_id: self.package_type_id,
            quantity: self.quantity.unwrap_or(Decimal::from(1)),
            company_id: self.company_id,
            metadata: AuditMetadata::default(),
        })
    }
}
