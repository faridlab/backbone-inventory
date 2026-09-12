use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use super::AuditMetadata;

/// Strongly-typed ID for Route
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RouteId(pub Uuid);

impl RouteId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for RouteId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for RouteId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for RouteId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<RouteId> for Uuid {
    fn from(id: RouteId) -> Self { id.0 }
}

impl AsRef<Uuid> for RouteId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for RouteId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Route {
    pub id: Uuid,
    pub name: String,
    pub active: bool,
    pub sequence: i32,
    pub product_selectable: bool,
    pub product_categ_selectable: bool,
    pub warehouse_selectable: bool,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl Route {
    /// Create a builder for Route
    pub fn builder() -> RouteBuilder {
        <RouteBuilder as Default>::default()
    }

    /// Create a new Route with required fields
    pub fn new(name: String, active: bool, sequence: i32, product_selectable: bool, product_categ_selectable: bool, warehouse_selectable: bool) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            active,
            sequence,
            product_selectable,
            product_categ_selectable,
            warehouse_selectable,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> RouteId {
        RouteId(self.id)
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
                "name" => {
                    if let Ok(v) = serde_json::from_value(value) { self.name = v; }
                }
                "active" => {
                    if let Ok(v) = serde_json::from_value(value) { self.active = v; }
                }
                "sequence" => {
                    if let Ok(v) = serde_json::from_value(value) { self.sequence = v; }
                }
                "product_selectable" => {
                    if let Ok(v) = serde_json::from_value(value) { self.product_selectable = v; }
                }
                "product_categ_selectable" => {
                    if let Ok(v) = serde_json::from_value(value) { self.product_categ_selectable = v; }
                }
                "warehouse_selectable" => {
                    if let Ok(v) = serde_json::from_value(value) { self.warehouse_selectable = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for Route {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "Route"
    }
}

impl backbone_core::PersistentEntity for Route {
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

impl backbone_orm::EntityRepoMeta for Route {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["name"]
    }
}

/// Builder for Route entity
///
/// Provides a fluent API for constructing Route instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct RouteBuilder {
    name: Option<String>,
    active: Option<bool>,
    sequence: Option<i32>,
    product_selectable: Option<bool>,
    product_categ_selectable: Option<bool>,
    warehouse_selectable: Option<bool>,
}

impl RouteBuilder {
    /// Set the name field (required)
    pub fn name(mut self, value: String) -> Self {
        self.name = Some(value);
        self
    }

    /// Set the active field (default: `false`)
    pub fn active(mut self, value: bool) -> Self {
        self.active = Some(value);
        self
    }

    /// Set the sequence field (default: `0`)
    pub fn sequence(mut self, value: i32) -> Self {
        self.sequence = Some(value);
        self
    }

    /// Set the product_selectable field (default: `true`)
    pub fn product_selectable(mut self, value: bool) -> Self {
        self.product_selectable = Some(value);
        self
    }

    /// Set the product_categ_selectable field (default: `false`)
    pub fn product_categ_selectable(mut self, value: bool) -> Self {
        self.product_categ_selectable = Some(value);
        self
    }

    /// Set the warehouse_selectable field (default: `false`)
    pub fn warehouse_selectable(mut self, value: bool) -> Self {
        self.warehouse_selectable = Some(value);
        self
    }

    /// Build the Route entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<Route, String> {
        let name = self.name.ok_or_else(|| "name is required".to_string())?;

        Ok(Route {
            id: Uuid::new_v4(),
            name,
            active: self.active.unwrap_or(false),
            sequence: self.sequence.unwrap_or(0),
            product_selectable: self.product_selectable.unwrap_or(true),
            product_categ_selectable: self.product_categ_selectable.unwrap_or(false),
            warehouse_selectable: self.warehouse_selectable.unwrap_or(false),
            metadata: AuditMetadata::default(),
        })
    }
}
