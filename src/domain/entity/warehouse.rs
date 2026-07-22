use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use super::WarehouseType;
use super::AuditMetadata;

/// Strongly-typed ID for Warehouse
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WarehouseId(pub Uuid);

impl WarehouseId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for WarehouseId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for WarehouseId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for WarehouseId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<WarehouseId> for Uuid {
    fn from(id: WarehouseId) -> Self { id.0 }
}

impl AsRef<Uuid> for WarehouseId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for WarehouseId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Warehouse {
    pub id: Uuid,
    pub company_id: Uuid,
    pub code: String,
    pub name: String,
    pub warehouse_type: WarehouseType,
    pub parent_warehouse_id: Option<Uuid>,
    pub is_group: bool,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl Warehouse {
    /// Create a builder for Warehouse
    pub fn builder() -> WarehouseBuilder {
        WarehouseBuilder::default()
    }

    /// Create a new Warehouse with required fields
    pub fn new(company_id: Uuid, code: String, name: String, warehouse_type: WarehouseType, is_group: bool) -> Self {
        Self {
            id: Uuid::new_v4(),
            company_id,
            code,
            name,
            warehouse_type,
            parent_warehouse_id: None,
            is_group,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> WarehouseId {
        WarehouseId(self.id)
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

    /// Set the parent_warehouse_id field (chainable)
    pub fn with_parent_warehouse_id(mut self, value: Uuid) -> Self {
        self.parent_warehouse_id = Some(value);
        self
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
                "code" => {
                    if let Ok(v) = serde_json::from_value(value) { self.code = v; }
                }
                "name" => {
                    if let Ok(v) = serde_json::from_value(value) { self.name = v; }
                }
                "warehouse_type" => {
                    if let Ok(v) = serde_json::from_value(value) { self.warehouse_type = v; }
                }
                "parent_warehouse_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.parent_warehouse_id = v; }
                }
                "is_group" => {
                    if let Ok(v) = serde_json::from_value(value) { self.is_group = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for Warehouse {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "Warehouse"
    }
}

impl backbone_core::PersistentEntity for Warehouse {
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

impl backbone_orm::EntityRepoMeta for Warehouse {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("company_id".to_string(), "uuid".to_string());
        m.insert("parent_warehouse_id".to_string(), "uuid".to_string());
        m.insert("warehouse_type".to_string(), "warehouse_type".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["code", "name"]
    }
    fn company_field() -> Option<&'static str> {
        Some("company_id")
    }
}

/// Builder for Warehouse entity
///
/// Provides a fluent API for constructing Warehouse instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct WarehouseBuilder {
    company_id: Option<Uuid>,
    code: Option<String>,
    name: Option<String>,
    warehouse_type: Option<WarehouseType>,
    parent_warehouse_id: Option<Uuid>,
    is_group: Option<bool>,
}

impl WarehouseBuilder {
    /// Set the company_id field (required)
    pub fn company_id(mut self, value: Uuid) -> Self {
        self.company_id = Some(value);
        self
    }

    /// Set the code field (required)
    pub fn code(mut self, value: String) -> Self {
        self.code = Some(value);
        self
    }

    /// Set the name field (required)
    pub fn name(mut self, value: String) -> Self {
        self.name = Some(value);
        self
    }

    /// Set the warehouse_type field (default: `WarehouseType::default()`)
    pub fn warehouse_type(mut self, value: WarehouseType) -> Self {
        self.warehouse_type = Some(value);
        self
    }

    /// Set the parent_warehouse_id field (optional)
    pub fn parent_warehouse_id(mut self, value: Uuid) -> Self {
        self.parent_warehouse_id = Some(value);
        self
    }

    /// Set the is_group field (default: `false`)
    pub fn is_group(mut self, value: bool) -> Self {
        self.is_group = Some(value);
        self
    }

    /// Build the Warehouse entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<Warehouse, String> {
        let company_id = self.company_id.ok_or_else(|| "company_id is required".to_string())?;
        let code = self.code.ok_or_else(|| "code is required".to_string())?;
        let name = self.name.ok_or_else(|| "name is required".to_string())?;

        Ok(Warehouse {
            id: Uuid::new_v4(),
            company_id,
            code,
            name,
            warehouse_type: self.warehouse_type.unwrap_or(WarehouseType::default()),
            parent_warehouse_id: self.parent_warehouse_id,
            is_group: self.is_group.unwrap_or(false),
            metadata: AuditMetadata::default(),
        })
    }
}
