use chrono::{DateTime, Utc, NaiveDate};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use super::AuditMetadata;

/// Strongly-typed ID for Package
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PackageId(pub Uuid);

impl PackageId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for PackageId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for PackageId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for PackageId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<PackageId> for Uuid {
    fn from(id: PackageId) -> Self { id.0 }
}

impl AsRef<Uuid> for PackageId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for PackageId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Package {
    pub id: Uuid,
    pub name: String,
    pub complete_name: String,
    pub location_id: Option<Uuid>,
    pub company_id: Option<Uuid>,
    pub parent_package_id: Option<Uuid>,
    pub parent_path: String,
    pub pack_date: Option<NaiveDate>,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl Package {
    /// Create a builder for Package
    pub fn builder() -> PackageBuilder {
        <PackageBuilder as Default>::default()
    }

    /// Create a new Package with required fields
    pub fn new(name: String, complete_name: String, parent_path: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            complete_name,
            location_id: None,
            company_id: None,
            parent_package_id: None,
            parent_path,
            pack_date: None,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> PackageId {
        PackageId(self.id)
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

    /// Set the company_id field (chainable)
    pub fn with_company_id(mut self, value: Uuid) -> Self {
        self.company_id = Some(value);
        self
    }

    /// Set the parent_package_id field (chainable)
    pub fn with_parent_package_id(mut self, value: Uuid) -> Self {
        self.parent_package_id = Some(value);
        self
    }

    /// Set the pack_date field (chainable)
    pub fn with_pack_date(mut self, value: NaiveDate) -> Self {
        self.pack_date = Some(value);
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
                "location_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.location_id = v; }
                }
                "company_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.company_id = v; }
                }
                "parent_package_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.parent_package_id = v; }
                }
                "parent_path" => {
                    if let Ok(v) = serde_json::from_value(value) { self.parent_path = v; }
                }
                "pack_date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.pack_date = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for Package {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "Package"
    }
}

impl backbone_core::PersistentEntity for Package {
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

impl backbone_orm::EntityRepoMeta for Package {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("location_id".to_string(), "uuid".to_string());
        m.insert("company_id".to_string(), "uuid".to_string());
        m.insert("parent_package_id".to_string(), "uuid".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["name", "complete_name", "parent_path"]
    }
    fn company_field() -> Option<&'static str> {
        Some("company_id")
    }
}

/// Builder for Package entity
///
/// Provides a fluent API for constructing Package instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct PackageBuilder {
    name: Option<String>,
    complete_name: Option<String>,
    location_id: Option<Uuid>,
    company_id: Option<Uuid>,
    parent_package_id: Option<Uuid>,
    parent_path: Option<String>,
    pack_date: Option<NaiveDate>,
}

impl PackageBuilder {
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

    /// Set the location_id field (optional)
    pub fn location_id(mut self, value: Uuid) -> Self {
        self.location_id = Some(value);
        self
    }

    /// Set the company_id field (optional)
    pub fn company_id(mut self, value: Uuid) -> Self {
        self.company_id = Some(value);
        self
    }

    /// Set the parent_package_id field (optional)
    pub fn parent_package_id(mut self, value: Uuid) -> Self {
        self.parent_package_id = Some(value);
        self
    }

    /// Set the parent_path field (required)
    pub fn parent_path(mut self, value: String) -> Self {
        self.parent_path = Some(value);
        self
    }

    /// Set the pack_date field (optional)
    pub fn pack_date(mut self, value: NaiveDate) -> Self {
        self.pack_date = Some(value);
        self
    }

    /// Build the Package entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<Package, String> {
        let name = self.name.ok_or_else(|| "name is required".to_string())?;
        let complete_name = self.complete_name.ok_or_else(|| "complete_name is required".to_string())?;
        let parent_path = self.parent_path.ok_or_else(|| "parent_path is required".to_string())?;

        Ok(Package {
            id: Uuid::new_v4(),
            name,
            complete_name,
            location_id: self.location_id,
            company_id: self.company_id,
            parent_package_id: self.parent_package_id,
            parent_path,
            pack_date: self.pack_date,
            metadata: AuditMetadata::default(),
        })
    }
}
