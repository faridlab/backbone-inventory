use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use super::Priority;
use super::MoveType;
use super::TransferState;
use super::AuditMetadata;

/// Strongly-typed ID for Transfer
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TransferId(pub Uuid);

impl TransferId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for TransferId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for TransferId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for TransferId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<TransferId> for Uuid {
    fn from(id: TransferId) -> Self { id.0 }
}

impl AsRef<Uuid> for TransferId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for TransferId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Transfer {
    pub id: Uuid,
    pub name: String,
    pub origin: Option<String>,
    pub note: Option<String>,
    pub priority: Priority,
    pub picking_type_id: Uuid,
    pub location_id: Uuid,
    pub location_dest_id: Uuid,
    pub partner_id: Option<Uuid>,
    pub company_id: Uuid,
    pub move_type: MoveType,
    pub scheduled_date: DateTime<Utc>,
    pub date_done: Option<DateTime<Utc>>,
    pub state: TransferState,
    pub is_locked: bool,
    pub backorder_id: Option<Uuid>,
    pub return_id: Option<Uuid>,
    pub batch_id: Option<Uuid>,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl Transfer {
    /// Create a builder for Transfer
    pub fn builder() -> TransferBuilder {
        <TransferBuilder as Default>::default()
    }

    /// Create a new Transfer with required fields
    pub fn new(name: String, priority: Priority, picking_type_id: Uuid, location_id: Uuid, location_dest_id: Uuid, company_id: Uuid, move_type: MoveType, scheduled_date: DateTime<Utc>, state: TransferState, is_locked: bool) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            origin: None,
            note: None,
            priority,
            picking_type_id,
            location_id,
            location_dest_id,
            partner_id: None,
            company_id,
            move_type,
            scheduled_date,
            date_done: None,
            state,
            is_locked,
            backorder_id: None,
            return_id: None,
            batch_id: None,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> TransferId {
        TransferId(self.id)
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

    /// Set the note field (chainable)
    pub fn with_note(mut self, value: String) -> Self {
        self.note = Some(value);
        self
    }

    /// Set the partner_id field (chainable)
    pub fn with_partner_id(mut self, value: Uuid) -> Self {
        self.partner_id = Some(value);
        self
    }

    /// Set the date_done field (chainable)
    pub fn with_date_done(mut self, value: DateTime<Utc>) -> Self {
        self.date_done = Some(value);
        self
    }

    /// Set the backorder_id field (chainable)
    pub fn with_backorder_id(mut self, value: Uuid) -> Self {
        self.backorder_id = Some(value);
        self
    }

    /// Set the return_id field (chainable)
    pub fn with_return_id(mut self, value: Uuid) -> Self {
        self.return_id = Some(value);
        self
    }

    /// Set the batch_id field (chainable)
    pub fn with_batch_id(mut self, value: Uuid) -> Self {
        self.batch_id = Some(value);
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
                "origin" => {
                    if let Ok(v) = serde_json::from_value(value) { self.origin = v; }
                }
                "note" => {
                    if let Ok(v) = serde_json::from_value(value) { self.note = v; }
                }
                "priority" => {
                    if let Ok(v) = serde_json::from_value(value) { self.priority = v; }
                }
                "picking_type_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.picking_type_id = v; }
                }
                "location_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.location_id = v; }
                }
                "location_dest_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.location_dest_id = v; }
                }
                "partner_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.partner_id = v; }
                }
                "company_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.company_id = v; }
                }
                "move_type" => {
                    if let Ok(v) = serde_json::from_value(value) { self.move_type = v; }
                }
                "scheduled_date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.scheduled_date = v; }
                }
                "date_done" => {
                    if let Ok(v) = serde_json::from_value(value) { self.date_done = v; }
                }
                "state" => {
                    if let Ok(v) = serde_json::from_value(value) { self.state = v; }
                }
                "is_locked" => {
                    if let Ok(v) = serde_json::from_value(value) { self.is_locked = v; }
                }
                "backorder_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.backorder_id = v; }
                }
                "return_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.return_id = v; }
                }
                "batch_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.batch_id = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for Transfer {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "Transfer"
    }
}

impl backbone_core::PersistentEntity for Transfer {
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

impl backbone_orm::EntityRepoMeta for Transfer {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("picking_type_id".to_string(), "uuid".to_string());
        m.insert("location_id".to_string(), "uuid".to_string());
        m.insert("location_dest_id".to_string(), "uuid".to_string());
        m.insert("partner_id".to_string(), "uuid".to_string());
        m.insert("company_id".to_string(), "uuid".to_string());
        m.insert("backorder_id".to_string(), "uuid".to_string());
        m.insert("return_id".to_string(), "uuid".to_string());
        m.insert("batch_id".to_string(), "uuid".to_string());
        m.insert("priority".to_string(), "priority".to_string());
        m.insert("move_type".to_string(), "move_type".to_string());
        m.insert("state".to_string(), "transfer_state".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["name"]
    }
    fn company_field() -> Option<&'static str> {
        Some("company_id")
    }
}

/// Builder for Transfer entity
///
/// Provides a fluent API for constructing Transfer instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct TransferBuilder {
    name: Option<String>,
    origin: Option<String>,
    note: Option<String>,
    priority: Option<Priority>,
    picking_type_id: Option<Uuid>,
    location_id: Option<Uuid>,
    location_dest_id: Option<Uuid>,
    partner_id: Option<Uuid>,
    company_id: Option<Uuid>,
    move_type: Option<MoveType>,
    scheduled_date: Option<DateTime<Utc>>,
    date_done: Option<DateTime<Utc>>,
    state: Option<TransferState>,
    is_locked: Option<bool>,
    backorder_id: Option<Uuid>,
    return_id: Option<Uuid>,
    batch_id: Option<Uuid>,
}

impl TransferBuilder {
    /// Set the name field (default: `"/".to_string()`)
    pub fn name(mut self, value: String) -> Self {
        self.name = Some(value);
        self
    }

    /// Set the origin field (optional)
    pub fn origin(mut self, value: String) -> Self {
        self.origin = Some(value);
        self
    }

    /// Set the note field (optional)
    pub fn note(mut self, value: String) -> Self {
        self.note = Some(value);
        self
    }

    /// Set the priority field (default: `Priority::default()`)
    pub fn priority(mut self, value: Priority) -> Self {
        self.priority = Some(value);
        self
    }

    /// Set the picking_type_id field (required)
    pub fn picking_type_id(mut self, value: Uuid) -> Self {
        self.picking_type_id = Some(value);
        self
    }

    /// Set the location_id field (required)
    pub fn location_id(mut self, value: Uuid) -> Self {
        self.location_id = Some(value);
        self
    }

    /// Set the location_dest_id field (required)
    pub fn location_dest_id(mut self, value: Uuid) -> Self {
        self.location_dest_id = Some(value);
        self
    }

    /// Set the partner_id field (optional)
    pub fn partner_id(mut self, value: Uuid) -> Self {
        self.partner_id = Some(value);
        self
    }

    /// Set the company_id field (required)
    pub fn company_id(mut self, value: Uuid) -> Self {
        self.company_id = Some(value);
        self
    }

    /// Set the move_type field (default: `MoveType::default()`)
    pub fn move_type(mut self, value: MoveType) -> Self {
        self.move_type = Some(value);
        self
    }

    /// Set the scheduled_date field (default: `Utc::now()`)
    pub fn scheduled_date(mut self, value: DateTime<Utc>) -> Self {
        self.scheduled_date = Some(value);
        self
    }

    /// Set the date_done field (optional)
    pub fn date_done(mut self, value: DateTime<Utc>) -> Self {
        self.date_done = Some(value);
        self
    }

    /// Set the state field (default: `TransferState::default()`)
    pub fn state(mut self, value: TransferState) -> Self {
        self.state = Some(value);
        self
    }

    /// Set the is_locked field (default: `true`)
    pub fn is_locked(mut self, value: bool) -> Self {
        self.is_locked = Some(value);
        self
    }

    /// Set the backorder_id field (optional)
    pub fn backorder_id(mut self, value: Uuid) -> Self {
        self.backorder_id = Some(value);
        self
    }

    /// Set the return_id field (optional)
    pub fn return_id(mut self, value: Uuid) -> Self {
        self.return_id = Some(value);
        self
    }

    /// Set the batch_id field (optional)
    pub fn batch_id(mut self, value: Uuid) -> Self {
        self.batch_id = Some(value);
        self
    }

    /// Build the Transfer entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<Transfer, String> {
        let picking_type_id = self.picking_type_id.ok_or_else(|| "picking_type_id is required".to_string())?;
        let location_id = self.location_id.ok_or_else(|| "location_id is required".to_string())?;
        let location_dest_id = self.location_dest_id.ok_or_else(|| "location_dest_id is required".to_string())?;
        let company_id = self.company_id.ok_or_else(|| "company_id is required".to_string())?;

        Ok(Transfer {
            id: Uuid::new_v4(),
            name: self.name.unwrap_or("/".to_string()),
            origin: self.origin,
            note: self.note,
            priority: self.priority.unwrap_or_default(),
            picking_type_id,
            location_id,
            location_dest_id,
            partner_id: self.partner_id,
            company_id,
            move_type: self.move_type.unwrap_or_default(),
            scheduled_date: self.scheduled_date.unwrap_or(Utc::now()),
            date_done: self.date_done,
            state: self.state.unwrap_or_default(),
            is_locked: self.is_locked.unwrap_or(true),
            backorder_id: self.backorder_id,
            return_id: self.return_id,
            batch_id: self.batch_id,
            metadata: AuditMetadata::default(),
        })
    }
}
