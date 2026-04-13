use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use super::UnitOfMeasure;
use super::CostingMethod;
use super::AuditMetadata;

/// Strongly-typed ID for ProductCategory
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProductCategoryId(pub Uuid);

impl ProductCategoryId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for ProductCategoryId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for ProductCategoryId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for ProductCategoryId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<ProductCategoryId> for Uuid {
    fn from(id: ProductCategoryId) -> Self { id.0 }
}

impl AsRef<Uuid> for ProductCategoryId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for ProductCategoryId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ProductCategory {
    pub id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<Uuid>,
    pub code: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<Uuid>,
    pub level: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub sort_order: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_uom: Option<UnitOfMeasure>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_costing_method: Option<CostingMethod>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_inventory_account_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_cogs_account_id: Option<Uuid>,
    pub is_active: bool,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl ProductCategory {
    /// Create a builder for ProductCategory
    pub fn builder() -> ProductCategoryBuilder {
        ProductCategoryBuilder::default()
    }

    /// Create a new ProductCategory with required fields
    pub fn new(code: String, name: String, level: i32, sort_order: i32, is_active: bool) -> Self {
        Self {
            id: Uuid::new_v4(),
            provider_id: None,
            code,
            name,
            description: None,
            parent_id: None,
            level,
            path: None,
            sort_order,
            default_uom: None,
            default_costing_method: None,
            default_inventory_account_id: None,
            default_cogs_account_id: None,
            is_active,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> ProductCategoryId {
        ProductCategoryId(self.id)
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

    /// Set the provider_id field (chainable)
    pub fn with_provider_id(mut self, value: Uuid) -> Self {
        self.provider_id = Some(value);
        self
    }

    /// Set the description field (chainable)
    pub fn with_description(mut self, value: String) -> Self {
        self.description = Some(value);
        self
    }

    /// Set the parent_id field (chainable)
    pub fn with_parent_id(mut self, value: Uuid) -> Self {
        self.parent_id = Some(value);
        self
    }

    /// Set the path field (chainable)
    pub fn with_path(mut self, value: String) -> Self {
        self.path = Some(value);
        self
    }

    /// Set the default_uom field (chainable)
    pub fn with_default_uom(mut self, value: UnitOfMeasure) -> Self {
        self.default_uom = Some(value);
        self
    }

    /// Set the default_costing_method field (chainable)
    pub fn with_default_costing_method(mut self, value: CostingMethod) -> Self {
        self.default_costing_method = Some(value);
        self
    }

    /// Set the default_inventory_account_id field (chainable)
    pub fn with_default_inventory_account_id(mut self, value: Uuid) -> Self {
        self.default_inventory_account_id = Some(value);
        self
    }

    /// Set the default_cogs_account_id field (chainable)
    pub fn with_default_cogs_account_id(mut self, value: Uuid) -> Self {
        self.default_cogs_account_id = Some(value);
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
                "code" => {
                    if let Ok(v) = serde_json::from_value(value) { self.code = v; }
                }
                "name" => {
                    if let Ok(v) = serde_json::from_value(value) { self.name = v; }
                }
                "description" => {
                    if let Ok(v) = serde_json::from_value(value) { self.description = v; }
                }
                "parent_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.parent_id = v; }
                }
                "level" => {
                    if let Ok(v) = serde_json::from_value(value) { self.level = v; }
                }
                "path" => {
                    if let Ok(v) = serde_json::from_value(value) { self.path = v; }
                }
                "sort_order" => {
                    if let Ok(v) = serde_json::from_value(value) { self.sort_order = v; }
                }
                "default_uom" => {
                    if let Ok(v) = serde_json::from_value(value) { self.default_uom = v; }
                }
                "default_costing_method" => {
                    if let Ok(v) = serde_json::from_value(value) { self.default_costing_method = v; }
                }
                "default_inventory_account_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.default_inventory_account_id = v; }
                }
                "default_cogs_account_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.default_cogs_account_id = v; }
                }
                "is_active" => {
                    if let Ok(v) = serde_json::from_value(value) { self.is_active = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for ProductCategory {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "ProductCategory"
    }
}

impl backbone_core::PersistentEntity for ProductCategory {
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

impl backbone_orm::EntityRepoMeta for ProductCategory {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("provider_id".to_string(), "uuid".to_string());
        m.insert("parent_id".to_string(), "uuid".to_string());
        m.insert("default_inventory_account_id".to_string(), "uuid".to_string());
        m.insert("default_cogs_account_id".to_string(), "uuid".to_string());
        m.insert("default_uom".to_string(), "unit_of_measure".to_string());
        m.insert("default_costing_method".to_string(), "costing_method".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["code", "name"]
    }
}

/// Builder for ProductCategory entity
///
/// Provides a fluent API for constructing ProductCategory instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct ProductCategoryBuilder {
    provider_id: Option<Uuid>,
    code: Option<String>,
    name: Option<String>,
    description: Option<String>,
    parent_id: Option<Uuid>,
    level: Option<i32>,
    path: Option<String>,
    sort_order: Option<i32>,
    default_uom: Option<UnitOfMeasure>,
    default_costing_method: Option<CostingMethod>,
    default_inventory_account_id: Option<Uuid>,
    default_cogs_account_id: Option<Uuid>,
    is_active: Option<bool>,
}

impl ProductCategoryBuilder {
    /// Set the provider_id field (optional)
    pub fn provider_id(mut self, value: Uuid) -> Self {
        self.provider_id = Some(value);
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

    /// Set the description field (optional)
    pub fn description(mut self, value: String) -> Self {
        self.description = Some(value);
        self
    }

    /// Set the parent_id field (optional)
    pub fn parent_id(mut self, value: Uuid) -> Self {
        self.parent_id = Some(value);
        self
    }

    /// Set the level field (default: `0`)
    pub fn level(mut self, value: i32) -> Self {
        self.level = Some(value);
        self
    }

    /// Set the path field (optional)
    pub fn path(mut self, value: String) -> Self {
        self.path = Some(value);
        self
    }

    /// Set the sort_order field (default: `0`)
    pub fn sort_order(mut self, value: i32) -> Self {
        self.sort_order = Some(value);
        self
    }

    /// Set the default_uom field (optional)
    pub fn default_uom(mut self, value: UnitOfMeasure) -> Self {
        self.default_uom = Some(value);
        self
    }

    /// Set the default_costing_method field (optional)
    pub fn default_costing_method(mut self, value: CostingMethod) -> Self {
        self.default_costing_method = Some(value);
        self
    }

    /// Set the default_inventory_account_id field (optional)
    pub fn default_inventory_account_id(mut self, value: Uuid) -> Self {
        self.default_inventory_account_id = Some(value);
        self
    }

    /// Set the default_cogs_account_id field (optional)
    pub fn default_cogs_account_id(mut self, value: Uuid) -> Self {
        self.default_cogs_account_id = Some(value);
        self
    }

    /// Set the is_active field (default: `true`)
    pub fn is_active(mut self, value: bool) -> Self {
        self.is_active = Some(value);
        self
    }

    /// Build the ProductCategory entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<ProductCategory, String> {
        let code = self.code.ok_or_else(|| "code is required".to_string())?;
        let name = self.name.ok_or_else(|| "name is required".to_string())?;

        Ok(ProductCategory {
            id: Uuid::new_v4(),
            provider_id: self.provider_id,
            code,
            name,
            description: self.description,
            parent_id: self.parent_id,
            level: self.level.unwrap_or(0),
            path: self.path,
            sort_order: self.sort_order.unwrap_or(0),
            default_uom: self.default_uom,
            default_costing_method: self.default_costing_method,
            default_inventory_account_id: self.default_inventory_account_id,
            default_cogs_account_id: self.default_cogs_account_id,
            is_active: self.is_active.unwrap_or(true),
            metadata: AuditMetadata::default(),
        })
    }
}
