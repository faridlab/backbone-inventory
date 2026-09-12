use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use rust_decimal::Decimal;

use super::PackageUse;
use super::AuditMetadata;

/// Strongly-typed ID for PackageType
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PackageTypeId(pub Uuid);

impl PackageTypeId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for PackageTypeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for PackageTypeId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for PackageTypeId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<PackageTypeId> for Uuid {
    fn from(id: PackageTypeId) -> Self { id.0 }
}

impl AsRef<Uuid> for PackageTypeId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for PackageTypeId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PackageType {
    pub id: Uuid,
    pub name: String,
    pub barcode: Option<String>,
    pub package_use: PackageUse,
    pub sequence: i32,
    pub length: Option<Decimal>,
    pub width: Option<Decimal>,
    pub height: Option<Decimal>,
    pub max_weight: Option<Decimal>,
    pub active: bool,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl PackageType {
    /// Create a builder for PackageType
    pub fn builder() -> PackageTypeBuilder {
        <PackageTypeBuilder as Default>::default()
    }

    /// Create a new PackageType with required fields
    pub fn new(name: String, package_use: PackageUse, sequence: i32, active: bool) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            barcode: None,
            package_use,
            sequence,
            length: None,
            width: None,
            height: None,
            max_weight: None,
            active,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> PackageTypeId {
        PackageTypeId(self.id)
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

    /// Set the barcode field (chainable)
    pub fn with_barcode(mut self, value: String) -> Self {
        self.barcode = Some(value);
        self
    }

    /// Set the length field (chainable)
    pub fn with_length(mut self, value: Decimal) -> Self {
        self.length = Some(value);
        self
    }

    /// Set the width field (chainable)
    pub fn with_width(mut self, value: Decimal) -> Self {
        self.width = Some(value);
        self
    }

    /// Set the height field (chainable)
    pub fn with_height(mut self, value: Decimal) -> Self {
        self.height = Some(value);
        self
    }

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
                "barcode" => {
                    if let Ok(v) = serde_json::from_value(value) { self.barcode = v; }
                }
                "package_use" => {
                    if let Ok(v) = serde_json::from_value(value) { self.package_use = v; }
                }
                "sequence" => {
                    if let Ok(v) = serde_json::from_value(value) { self.sequence = v; }
                }
                "length" => {
                    if let Ok(v) = serde_json::from_value(value) { self.length = v; }
                }
                "width" => {
                    if let Ok(v) = serde_json::from_value(value) { self.width = v; }
                }
                "height" => {
                    if let Ok(v) = serde_json::from_value(value) { self.height = v; }
                }
                "max_weight" => {
                    if let Ok(v) = serde_json::from_value(value) { self.max_weight = v; }
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

impl super::Entity for PackageType {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "PackageType"
    }
}

impl backbone_core::PersistentEntity for PackageType {
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

impl backbone_orm::EntityRepoMeta for PackageType {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("package_use".to_string(), "package_use".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["name"]
    }
}

/// Builder for PackageType entity
///
/// Provides a fluent API for constructing PackageType instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct PackageTypeBuilder {
    name: Option<String>,
    barcode: Option<String>,
    package_use: Option<PackageUse>,
    sequence: Option<i32>,
    length: Option<Decimal>,
    width: Option<Decimal>,
    height: Option<Decimal>,
    max_weight: Option<Decimal>,
    active: Option<bool>,
}

impl PackageTypeBuilder {
    /// Set the name field (required)
    pub fn name(mut self, value: String) -> Self {
        self.name = Some(value);
        self
    }

    /// Set the barcode field (optional)
    pub fn barcode(mut self, value: String) -> Self {
        self.barcode = Some(value);
        self
    }

    /// Set the package_use field (default: `PackageUse::default()`)
    pub fn package_use(mut self, value: PackageUse) -> Self {
        self.package_use = Some(value);
        self
    }

    /// Set the sequence field (default: `0`)
    pub fn sequence(mut self, value: i32) -> Self {
        self.sequence = Some(value);
        self
    }

    /// Set the length field (optional)
    pub fn length(mut self, value: Decimal) -> Self {
        self.length = Some(value);
        self
    }

    /// Set the width field (optional)
    pub fn width(mut self, value: Decimal) -> Self {
        self.width = Some(value);
        self
    }

    /// Set the height field (optional)
    pub fn height(mut self, value: Decimal) -> Self {
        self.height = Some(value);
        self
    }

    /// Set the max_weight field (optional)
    pub fn max_weight(mut self, value: Decimal) -> Self {
        self.max_weight = Some(value);
        self
    }

    /// Set the active field (default: `true`)
    pub fn active(mut self, value: bool) -> Self {
        self.active = Some(value);
        self
    }

    /// Build the PackageType entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<PackageType, String> {
        let name = self.name.ok_or_else(|| "name is required".to_string())?;

        Ok(PackageType {
            id: Uuid::new_v4(),
            name,
            barcode: self.barcode,
            package_use: self.package_use.unwrap_or_default(),
            sequence: self.sequence.unwrap_or(0),
            length: self.length,
            width: self.width,
            height: self.height,
            max_weight: self.max_weight,
            active: self.active.unwrap_or(true),
            metadata: AuditMetadata::default(),
        })
    }
}
