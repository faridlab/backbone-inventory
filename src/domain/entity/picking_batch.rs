use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use super::PickingBatchState;
use super::AuditMetadata;

/// Strongly-typed ID for PickingBatch
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PickingBatchId(pub Uuid);

impl PickingBatchId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for PickingBatchId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for PickingBatchId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for PickingBatchId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<PickingBatchId> for Uuid {
    fn from(id: PickingBatchId) -> Self { id.0 }
}

impl AsRef<Uuid> for PickingBatchId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for PickingBatchId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PickingBatch {
    pub id: Uuid,
    pub name: String,
    pub note: Option<String>,
    pub state: PickingBatchState,
    pub is_wave: bool,
    pub user_id: Option<Uuid>,
    pub scheduled_date: DateTime<Utc>,
    pub had_members: bool,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl PickingBatch {
    /// Create a builder for PickingBatch
    pub fn builder() -> PickingBatchBuilder {
        <PickingBatchBuilder as Default>::default()
    }

    /// Create a new PickingBatch with required fields
    pub fn new(name: String, state: PickingBatchState, is_wave: bool, scheduled_date: DateTime<Utc>, had_members: bool) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            note: None,
            state,
            is_wave,
            user_id: None,
            scheduled_date,
            had_members,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> PickingBatchId {
        PickingBatchId(self.id)
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

    /// Set the note field (chainable)
    pub fn with_note(mut self, value: String) -> Self {
        self.note = Some(value);
        self
    }

    /// Set the user_id field (chainable)
    pub fn with_user_id(mut self, value: Uuid) -> Self {
        self.user_id = Some(value);
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
                "note" => {
                    if let Ok(v) = serde_json::from_value(value) { self.note = v; }
                }
                "state" => {
                    if let Ok(v) = serde_json::from_value(value) { self.state = v; }
                }
                "is_wave" => {
                    if let Ok(v) = serde_json::from_value(value) { self.is_wave = v; }
                }
                "user_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.user_id = v; }
                }
                "scheduled_date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.scheduled_date = v; }
                }
                "had_members" => {
                    if let Ok(v) = serde_json::from_value(value) { self.had_members = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for PickingBatch {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "PickingBatch"
    }
}

impl backbone_core::PersistentEntity for PickingBatch {
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

impl backbone_orm::EntityRepoMeta for PickingBatch {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("user_id".to_string(), "uuid".to_string());
        m.insert("state".to_string(), "picking_batch_state".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["name"]
    }
}

/// Builder for PickingBatch entity
///
/// Provides a fluent API for constructing PickingBatch instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct PickingBatchBuilder {
    name: Option<String>,
    note: Option<String>,
    state: Option<PickingBatchState>,
    is_wave: Option<bool>,
    user_id: Option<Uuid>,
    scheduled_date: Option<DateTime<Utc>>,
    had_members: Option<bool>,
}

impl PickingBatchBuilder {
    /// Set the name field (default: `"/".to_string()`)
    pub fn name(mut self, value: String) -> Self {
        self.name = Some(value);
        self
    }

    /// Set the note field (optional)
    pub fn note(mut self, value: String) -> Self {
        self.note = Some(value);
        self
    }

    /// Set the state field (default: `PickingBatchState::default()`)
    pub fn state(mut self, value: PickingBatchState) -> Self {
        self.state = Some(value);
        self
    }

    /// Set the is_wave field (default: `false`)
    pub fn is_wave(mut self, value: bool) -> Self {
        self.is_wave = Some(value);
        self
    }

    /// Set the user_id field (optional)
    pub fn user_id(mut self, value: Uuid) -> Self {
        self.user_id = Some(value);
        self
    }

    /// Set the scheduled_date field (default: `Utc::now()`)
    pub fn scheduled_date(mut self, value: DateTime<Utc>) -> Self {
        self.scheduled_date = Some(value);
        self
    }

    /// Set the had_members field (default: `false`)
    pub fn had_members(mut self, value: bool) -> Self {
        self.had_members = Some(value);
        self
    }

    /// Build the PickingBatch entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<PickingBatch, String> {

        Ok(PickingBatch {
            id: Uuid::new_v4(),
            name: self.name.unwrap_or("/".to_string()),
            note: self.note,
            state: self.state.unwrap_or_default(),
            is_wave: self.is_wave.unwrap_or(false),
            user_id: self.user_id,
            scheduled_date: self.scheduled_date.unwrap_or(Utc::now()),
            had_members: self.had_members.unwrap_or(false),
            metadata: AuditMetadata::default(),
        })
    }
}
