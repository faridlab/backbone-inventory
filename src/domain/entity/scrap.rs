use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use rust_decimal::Decimal;

use super::ScrapState;
use super::AuditMetadata;

/// Strongly-typed ID for Scrap
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ScrapId(pub Uuid);

impl ScrapId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for ScrapId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for ScrapId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for ScrapId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<ScrapId> for Uuid {
    fn from(id: ScrapId) -> Self { id.0 }
}

impl AsRef<Uuid> for ScrapId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for ScrapId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Scrap {
    pub id: Uuid,
    pub name: String,
    pub state: ScrapState,
    pub origin: Option<String>,
    pub date_expected: DateTime<Utc>,
    pub item_id: Uuid,
    pub scrap_qty: Decimal,
    pub location_id: Uuid,
    pub scrap_location_id: Uuid,
    pub lot_id: Option<Uuid>,
    pub package_id: Option<Uuid>,
    pub owner_id: Option<Uuid>,
    pub picking_id: Option<Uuid>,
    pub move_id: Option<Uuid>,
    pub scrap_reason_tag_ids: Vec<Uuid>,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl Scrap {
    /// Create a builder for Scrap
    pub fn builder() -> ScrapBuilder {
        <ScrapBuilder as Default>::default()
    }

    /// Create a new Scrap with required fields
    pub fn new(name: String, state: ScrapState, date_expected: DateTime<Utc>, item_id: Uuid, scrap_qty: Decimal, location_id: Uuid, scrap_location_id: Uuid, scrap_reason_tag_ids: Vec<Uuid>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            state,
            origin: None,
            date_expected,
            item_id,
            scrap_qty,
            location_id,
            scrap_location_id,
            lot_id: None,
            package_id: None,
            owner_id: None,
            picking_id: None,
            move_id: None,
            scrap_reason_tag_ids,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> ScrapId {
        ScrapId(self.id)
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

    /// Set the origin field (chainable)
    pub fn with_origin(mut self, value: String) -> Self {
        self.origin = Some(value);
        self
    }

    /// Set the lot_id field (chainable)
    pub fn with_lot_id(mut self, value: Uuid) -> Self {
        self.lot_id = Some(value);
        self
    }

    /// Set the package_id field (chainable)
    pub fn with_package_id(mut self, value: Uuid) -> Self {
        self.package_id = Some(value);
        self
    }

    /// Set the owner_id field (chainable)
    pub fn with_owner_id(mut self, value: Uuid) -> Self {
        self.owner_id = Some(value);
        self
    }

    /// Set the picking_id field (chainable)
    pub fn with_picking_id(mut self, value: Uuid) -> Self {
        self.picking_id = Some(value);
        self
    }

    /// Set the move_id field (chainable)
    pub fn with_move_id(mut self, value: Uuid) -> Self {
        self.move_id = Some(value);
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
                "state" => {
                    if let Ok(v) = serde_json::from_value(value) { self.state = v; }
                }
                "origin" => {
                    if let Ok(v) = serde_json::from_value(value) { self.origin = v; }
                }
                "date_expected" => {
                    if let Ok(v) = serde_json::from_value(value) { self.date_expected = v; }
                }
                "item_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.item_id = v; }
                }
                "scrap_qty" => {
                    if let Ok(v) = serde_json::from_value(value) { self.scrap_qty = v; }
                }
                "location_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.location_id = v; }
                }
                "scrap_location_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.scrap_location_id = v; }
                }
                "lot_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.lot_id = v; }
                }
                "package_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.package_id = v; }
                }
                "owner_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.owner_id = v; }
                }
                "picking_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.picking_id = v; }
                }
                "move_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.move_id = v; }
                }
                "scrap_reason_tag_ids" => {
                    if let Ok(v) = serde_json::from_value(value) { self.scrap_reason_tag_ids = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for Scrap {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "Scrap"
    }
}

impl backbone_core::PersistentEntity for Scrap {
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

impl backbone_orm::EntityRepoMeta for Scrap {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("item_id".to_string(), "uuid".to_string());
        m.insert("location_id".to_string(), "uuid".to_string());
        m.insert("scrap_location_id".to_string(), "uuid".to_string());
        m.insert("lot_id".to_string(), "uuid".to_string());
        m.insert("package_id".to_string(), "uuid".to_string());
        m.insert("owner_id".to_string(), "uuid".to_string());
        m.insert("picking_id".to_string(), "uuid".to_string());
        m.insert("move_id".to_string(), "uuid".to_string());
        m.insert("state".to_string(), "scrap_state".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["name"]
    }
}

/// Builder for Scrap entity
///
/// Provides a fluent API for constructing Scrap instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct ScrapBuilder {
    name: Option<String>,
    state: Option<ScrapState>,
    origin: Option<String>,
    date_expected: Option<DateTime<Utc>>,
    item_id: Option<Uuid>,
    scrap_qty: Option<Decimal>,
    location_id: Option<Uuid>,
    scrap_location_id: Option<Uuid>,
    lot_id: Option<Uuid>,
    package_id: Option<Uuid>,
    owner_id: Option<Uuid>,
    picking_id: Option<Uuid>,
    move_id: Option<Uuid>,
    scrap_reason_tag_ids: Option<Vec<Uuid>>,
}

impl ScrapBuilder {
    /// Set the name field (default: `"/".to_string()`)
    pub fn name(mut self, value: String) -> Self {
        self.name = Some(value);
        self
    }

    /// Set the state field (default: `ScrapState::default()`)
    pub fn state(mut self, value: ScrapState) -> Self {
        self.state = Some(value);
        self
    }

    /// Set the origin field (optional)
    pub fn origin(mut self, value: String) -> Self {
        self.origin = Some(value);
        self
    }

    /// Set the date_expected field (default: `Utc::now()`)
    pub fn date_expected(mut self, value: DateTime<Utc>) -> Self {
        self.date_expected = Some(value);
        self
    }

    /// Set the item_id field (required)
    pub fn item_id(mut self, value: Uuid) -> Self {
        self.item_id = Some(value);
        self
    }

    /// Set the scrap_qty field (default: `Decimal::from(1)`)
    pub fn scrap_qty(mut self, value: Decimal) -> Self {
        self.scrap_qty = Some(value);
        self
    }

    /// Set the location_id field (required)
    pub fn location_id(mut self, value: Uuid) -> Self {
        self.location_id = Some(value);
        self
    }

    /// Set the scrap_location_id field (required)
    pub fn scrap_location_id(mut self, value: Uuid) -> Self {
        self.scrap_location_id = Some(value);
        self
    }

    /// Set the lot_id field (optional)
    pub fn lot_id(mut self, value: Uuid) -> Self {
        self.lot_id = Some(value);
        self
    }

    /// Set the package_id field (optional)
    pub fn package_id(mut self, value: Uuid) -> Self {
        self.package_id = Some(value);
        self
    }

    /// Set the owner_id field (optional)
    pub fn owner_id(mut self, value: Uuid) -> Self {
        self.owner_id = Some(value);
        self
    }

    /// Set the picking_id field (optional)
    pub fn picking_id(mut self, value: Uuid) -> Self {
        self.picking_id = Some(value);
        self
    }

    /// Set the move_id field (optional)
    pub fn move_id(mut self, value: Uuid) -> Self {
        self.move_id = Some(value);
        self
    }

    /// Set the scrap_reason_tag_ids field (required)
    pub fn scrap_reason_tag_ids(mut self, value: Vec<Uuid>) -> Self {
        self.scrap_reason_tag_ids = Some(value);
        self
    }

    /// Build the Scrap entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<Scrap, String> {
        let item_id = self.item_id.ok_or_else(|| "item_id is required".to_string())?;
        let location_id = self.location_id.ok_or_else(|| "location_id is required".to_string())?;
        let scrap_location_id = self.scrap_location_id.ok_or_else(|| "scrap_location_id is required".to_string())?;
        let scrap_reason_tag_ids = self.scrap_reason_tag_ids.ok_or_else(|| "scrap_reason_tag_ids is required".to_string())?;

        Ok(Scrap {
            id: Uuid::new_v4(),
            name: self.name.unwrap_or("/".to_string()),
            state: self.state.unwrap_or_default(),
            origin: self.origin,
            date_expected: self.date_expected.unwrap_or(Utc::now()),
            item_id,
            scrap_qty: self.scrap_qty.unwrap_or(Decimal::from(1)),
            location_id,
            scrap_location_id,
            lot_id: self.lot_id,
            package_id: self.package_id,
            owner_id: self.owner_id,
            picking_id: self.picking_id,
            move_id: self.move_id,
            scrap_reason_tag_ids,
            metadata: AuditMetadata::default(),
        })
    }
}
