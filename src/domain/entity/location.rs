use chrono::{DateTime, Utc, NaiveDate};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use super::LocationUsage;
use super::AuditMetadata;

/// Strongly-typed ID for Location
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LocationId(pub Uuid);

impl LocationId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for LocationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for LocationId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for LocationId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<LocationId> for Uuid {
    fn from(id: LocationId) -> Self { id.0 }
}

impl AsRef<Uuid> for LocationId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for LocationId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Location {
    pub id: Uuid,
    pub name: String,
    pub complete_name: String,
    pub active: bool,
    pub usage: LocationUsage,
    pub location_id: Option<Uuid>,
    pub parent_path: String,
    pub barcode: Option<String>,
    pub warehouse_id: Option<Uuid>,
    pub cyclic_inventory_frequency: i32,
    pub last_inventory_date: Option<NaiveDate>,
    pub next_inventory_date: Option<NaiveDate>,
    pub valuation_account_id: Option<Uuid>,
    pub storage_category_id: Option<Uuid>,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl Location {
    /// Create a builder for Location
    pub fn builder() -> LocationBuilder {
        <LocationBuilder as Default>::default()
    }

    /// Create a new Location with required fields
    pub fn new(name: String, complete_name: String, active: bool, usage: LocationUsage, parent_path: String, cyclic_inventory_frequency: i32) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            complete_name,
            active,
            usage,
            location_id: None,
            parent_path,
            barcode: None,
            warehouse_id: None,
            cyclic_inventory_frequency,
            last_inventory_date: None,
            next_inventory_date: None,
            valuation_account_id: None,
            storage_category_id: None,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> LocationId {
        LocationId(self.id)
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

    /// Set the location_id field (chainable)
    pub fn with_location_id(mut self, value: Uuid) -> Self {
        self.location_id = Some(value);
        self
    }

    /// Set the barcode field (chainable)
    pub fn with_barcode(mut self, value: String) -> Self {
        self.barcode = Some(value);
        self
    }

    /// Set the warehouse_id field (chainable)
    pub fn with_warehouse_id(mut self, value: Uuid) -> Self {
        self.warehouse_id = Some(value);
        self
    }

    /// Set the last_inventory_date field (chainable)
    pub fn with_last_inventory_date(mut self, value: NaiveDate) -> Self {
        self.last_inventory_date = Some(value);
        self
    }

    /// Set the next_inventory_date field (chainable)
    pub fn with_next_inventory_date(mut self, value: NaiveDate) -> Self {
        self.next_inventory_date = Some(value);
        self
    }

    /// Set the valuation_account_id field (chainable)
    pub fn with_valuation_account_id(mut self, value: Uuid) -> Self {
        self.valuation_account_id = Some(value);
        self
    }

    /// Set the storage_category_id field (chainable)
    pub fn with_storage_category_id(mut self, value: Uuid) -> Self {
        self.storage_category_id = Some(value);
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
                "complete_name" => {
                    if let Ok(v) = serde_json::from_value(value) { self.complete_name = v; }
                }
                "active" => {
                    if let Ok(v) = serde_json::from_value(value) { self.active = v; }
                }
                "usage" => {
                    if let Ok(v) = serde_json::from_value(value) { self.usage = v; }
                }
                "location_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.location_id = v; }
                }
                "parent_path" => {
                    if let Ok(v) = serde_json::from_value(value) { self.parent_path = v; }
                }
                "barcode" => {
                    if let Ok(v) = serde_json::from_value(value) { self.barcode = v; }
                }
                "warehouse_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.warehouse_id = v; }
                }
                "cyclic_inventory_frequency" => {
                    if let Ok(v) = serde_json::from_value(value) { self.cyclic_inventory_frequency = v; }
                }
                "last_inventory_date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.last_inventory_date = v; }
                }
                "next_inventory_date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.next_inventory_date = v; }
                }
                "valuation_account_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.valuation_account_id = v; }
                }
                "storage_category_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.storage_category_id = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for Location {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "Location"
    }
}

impl backbone_core::PersistentEntity for Location {
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

impl backbone_orm::EntityRepoMeta for Location {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("location_id".to_string(), "uuid".to_string());
        m.insert("warehouse_id".to_string(), "uuid".to_string());
        m.insert("valuation_account_id".to_string(), "uuid".to_string());
        m.insert("storage_category_id".to_string(), "uuid".to_string());
        m.insert("usage".to_string(), "location_usage".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["name", "complete_name", "parent_path"]
    }
}

/// Builder for Location entity
///
/// Provides a fluent API for constructing Location instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct LocationBuilder {
    name: Option<String>,
    complete_name: Option<String>,
    active: Option<bool>,
    usage: Option<LocationUsage>,
    location_id: Option<Uuid>,
    parent_path: Option<String>,
    barcode: Option<String>,
    warehouse_id: Option<Uuid>,
    cyclic_inventory_frequency: Option<i32>,
    last_inventory_date: Option<NaiveDate>,
    next_inventory_date: Option<NaiveDate>,
    valuation_account_id: Option<Uuid>,
    storage_category_id: Option<Uuid>,
}

impl LocationBuilder {
    /// Set the name field (required)
    pub fn name(mut self, value: String) -> Self {
        self.name = Some(value);
        self
    }

    /// Set the complete_name field (required)
    pub fn complete_name(mut self, value: String) -> Self {
        self.complete_name = Some(value);
        self
    }

    /// Set the active field (default: `true`)
    pub fn active(mut self, value: bool) -> Self {
        self.active = Some(value);
        self
    }

    /// Set the usage field (default: `LocationUsage::default()`)
    pub fn usage(mut self, value: LocationUsage) -> Self {
        self.usage = Some(value);
        self
    }

    /// Set the location_id field (optional)
    pub fn location_id(mut self, value: Uuid) -> Self {
        self.location_id = Some(value);
        self
    }

    /// Set the parent_path field (required)
    pub fn parent_path(mut self, value: String) -> Self {
        self.parent_path = Some(value);
        self
    }

    /// Set the barcode field (optional)
    pub fn barcode(mut self, value: String) -> Self {
        self.barcode = Some(value);
        self
    }

    /// Set the warehouse_id field (optional)
    pub fn warehouse_id(mut self, value: Uuid) -> Self {
        self.warehouse_id = Some(value);
        self
    }

    /// Set the cyclic_inventory_frequency field (default: `0`)
    pub fn cyclic_inventory_frequency(mut self, value: i32) -> Self {
        self.cyclic_inventory_frequency = Some(value);
        self
    }

    /// Set the last_inventory_date field (optional)
    pub fn last_inventory_date(mut self, value: NaiveDate) -> Self {
        self.last_inventory_date = Some(value);
        self
    }

    /// Set the next_inventory_date field (optional)
    pub fn next_inventory_date(mut self, value: NaiveDate) -> Self {
        self.next_inventory_date = Some(value);
        self
    }

    /// Set the valuation_account_id field (optional)
    pub fn valuation_account_id(mut self, value: Uuid) -> Self {
        self.valuation_account_id = Some(value);
        self
    }

    /// Set the storage_category_id field (optional)
    pub fn storage_category_id(mut self, value: Uuid) -> Self {
        self.storage_category_id = Some(value);
        self
    }

    /// Build the Location entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<Location, String> {
        let name = self.name.ok_or_else(|| "name is required".to_string())?;
        let complete_name = self.complete_name.ok_or_else(|| "complete_name is required".to_string())?;
        let parent_path = self.parent_path.ok_or_else(|| "parent_path is required".to_string())?;

        Ok(Location {
            id: Uuid::new_v4(),
            name,
            complete_name,
            active: self.active.unwrap_or(true),
            usage: self.usage.unwrap_or_default(),
            location_id: self.location_id,
            parent_path,
            barcode: self.barcode,
            warehouse_id: self.warehouse_id,
            cyclic_inventory_frequency: self.cyclic_inventory_frequency.unwrap_or(0),
            last_inventory_date: self.last_inventory_date,
            next_inventory_date: self.next_inventory_date,
            valuation_account_id: self.valuation_account_id,
            storage_category_id: self.storage_category_id,
            metadata: AuditMetadata::default(),
        })
    }
}
