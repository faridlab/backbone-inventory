use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use super::PutawaySublocation;
use super::AuditMetadata;

/// Strongly-typed ID for PutawayRule
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PutawayRuleId(pub Uuid);

impl PutawayRuleId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for PutawayRuleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for PutawayRuleId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for PutawayRuleId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<PutawayRuleId> for Uuid {
    fn from(id: PutawayRuleId) -> Self { id.0 }
}

impl AsRef<Uuid> for PutawayRuleId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for PutawayRuleId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PutawayRule {
    pub id: Uuid,
    pub sequence: i32,
    pub location_in_id: Uuid,
    pub location_out_id: Uuid,
    pub item_id: Option<Uuid>,
    pub category_id: Option<Uuid>,
    pub package_type_id: Option<Uuid>,
    pub storage_category_id: Option<Uuid>,
    pub sublocation: PutawaySublocation,
    pub active: bool,
    pub company_id: Option<Uuid>,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl PutawayRule {
    /// Create a builder for PutawayRule
    pub fn builder() -> PutawayRuleBuilder {
        <PutawayRuleBuilder as Default>::default()
    }

    /// Create a new PutawayRule with required fields
    pub fn new(sequence: i32, location_in_id: Uuid, location_out_id: Uuid, sublocation: PutawaySublocation, active: bool) -> Self {
        Self {
            id: Uuid::new_v4(),
            sequence,
            location_in_id,
            location_out_id,
            item_id: None,
            category_id: None,
            package_type_id: None,
            storage_category_id: None,
            sublocation,
            active,
            company_id: None,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> PutawayRuleId {
        PutawayRuleId(self.id)
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

    /// Set the category_id field (chainable)
    pub fn with_category_id(mut self, value: Uuid) -> Self {
        self.category_id = Some(value);
        self
    }

    /// Set the package_type_id field (chainable)
    pub fn with_package_type_id(mut self, value: Uuid) -> Self {
        self.package_type_id = Some(value);
        self
    }

    /// Set the storage_category_id field (chainable)
    pub fn with_storage_category_id(mut self, value: Uuid) -> Self {
        self.storage_category_id = Some(value);
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
                "sequence" => {
                    if let Ok(v) = serde_json::from_value(value) { self.sequence = v; }
                }
                "location_in_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.location_in_id = v; }
                }
                "location_out_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.location_out_id = v; }
                }
                "item_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.item_id = v; }
                }
                "category_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.category_id = v; }
                }
                "package_type_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.package_type_id = v; }
                }
                "storage_category_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.storage_category_id = v; }
                }
                "sublocation" => {
                    if let Ok(v) = serde_json::from_value(value) { self.sublocation = v; }
                }
                "active" => {
                    if let Ok(v) = serde_json::from_value(value) { self.active = v; }
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

impl super::Entity for PutawayRule {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "PutawayRule"
    }
}

impl backbone_core::PersistentEntity for PutawayRule {
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

impl backbone_orm::EntityRepoMeta for PutawayRule {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("location_in_id".to_string(), "uuid".to_string());
        m.insert("location_out_id".to_string(), "uuid".to_string());
        m.insert("item_id".to_string(), "uuid".to_string());
        m.insert("category_id".to_string(), "uuid".to_string());
        m.insert("package_type_id".to_string(), "uuid".to_string());
        m.insert("storage_category_id".to_string(), "uuid".to_string());
        m.insert("company_id".to_string(), "uuid".to_string());
        m.insert("sublocation".to_string(), "putaway_sublocation".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &[]
    }
    fn company_field() -> Option<&'static str> {
        Some("company_id")
    }
}

/// Builder for PutawayRule entity
///
/// Provides a fluent API for constructing PutawayRule instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct PutawayRuleBuilder {
    sequence: Option<i32>,
    location_in_id: Option<Uuid>,
    location_out_id: Option<Uuid>,
    item_id: Option<Uuid>,
    category_id: Option<Uuid>,
    package_type_id: Option<Uuid>,
    storage_category_id: Option<Uuid>,
    sublocation: Option<PutawaySublocation>,
    active: Option<bool>,
    company_id: Option<Uuid>,
}

impl PutawayRuleBuilder {
    /// Set the sequence field (default: `0`)
    pub fn sequence(mut self, value: i32) -> Self {
        self.sequence = Some(value);
        self
    }

    /// Set the location_in_id field (required)
    pub fn location_in_id(mut self, value: Uuid) -> Self {
        self.location_in_id = Some(value);
        self
    }

    /// Set the location_out_id field (required)
    pub fn location_out_id(mut self, value: Uuid) -> Self {
        self.location_out_id = Some(value);
        self
    }

    /// Set the item_id field (optional)
    pub fn item_id(mut self, value: Uuid) -> Self {
        self.item_id = Some(value);
        self
    }

    /// Set the category_id field (optional)
    pub fn category_id(mut self, value: Uuid) -> Self {
        self.category_id = Some(value);
        self
    }

    /// Set the package_type_id field (optional)
    pub fn package_type_id(mut self, value: Uuid) -> Self {
        self.package_type_id = Some(value);
        self
    }

    /// Set the storage_category_id field (optional)
    pub fn storage_category_id(mut self, value: Uuid) -> Self {
        self.storage_category_id = Some(value);
        self
    }

    /// Set the sublocation field (default: `PutawaySublocation::default()`)
    pub fn sublocation(mut self, value: PutawaySublocation) -> Self {
        self.sublocation = Some(value);
        self
    }

    /// Set the active field (default: `true`)
    pub fn active(mut self, value: bool) -> Self {
        self.active = Some(value);
        self
    }

    /// Set the company_id field (optional)
    pub fn company_id(mut self, value: Uuid) -> Self {
        self.company_id = Some(value);
        self
    }

    /// Build the PutawayRule entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<PutawayRule, String> {
        let location_in_id = self.location_in_id.ok_or_else(|| "location_in_id is required".to_string())?;
        let location_out_id = self.location_out_id.ok_or_else(|| "location_out_id is required".to_string())?;

        Ok(PutawayRule {
            id: Uuid::new_v4(),
            sequence: self.sequence.unwrap_or(0),
            location_in_id,
            location_out_id,
            item_id: self.item_id,
            category_id: self.category_id,
            package_type_id: self.package_type_id,
            storage_category_id: self.storage_category_id,
            sublocation: self.sublocation.unwrap_or_default(),
            active: self.active.unwrap_or(true),
            company_id: self.company_id,
            metadata: AuditMetadata::default(),
        })
    }
}
