use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use super::PickingCode;
use super::ReservationMethod;
use super::MoveType;
use super::CreateBackorder;
use super::AuditMetadata;

/// Strongly-typed ID for OperationType
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OperationTypeId(pub Uuid);

impl OperationTypeId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for OperationTypeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for OperationTypeId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for OperationTypeId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<OperationTypeId> for Uuid {
    fn from(id: OperationTypeId) -> Self { id.0 }
}

impl AsRef<Uuid> for OperationTypeId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for OperationTypeId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct OperationType {
    pub id: Uuid,
    pub name: String,
    pub sequence_code: String,
    pub code: PickingCode,
    pub active: bool,
    pub sequence: i32,
    pub warehouse_id: Option<Uuid>,
    pub default_location_src_id: Uuid,
    pub default_location_dest_id: Uuid,
    pub reservation_method: ReservationMethod,
    pub reservation_days_before: Option<i32>,
    pub move_type: MoveType,
    pub create_backorder: CreateBackorder,
    pub use_create_lots: bool,
    pub use_existing_lots: bool,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl OperationType {
    /// Create a builder for OperationType
    pub fn builder() -> OperationTypeBuilder {
        <OperationTypeBuilder as Default>::default()
    }

    /// Create a new OperationType with required fields
    pub fn new(name: String, sequence_code: String, code: PickingCode, active: bool, sequence: i32, default_location_src_id: Uuid, default_location_dest_id: Uuid, reservation_method: ReservationMethod, move_type: MoveType, create_backorder: CreateBackorder, use_create_lots: bool, use_existing_lots: bool) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            sequence_code,
            code,
            active,
            sequence,
            warehouse_id: None,
            default_location_src_id,
            default_location_dest_id,
            reservation_method,
            reservation_days_before: None,
            move_type,
            create_backorder,
            use_create_lots,
            use_existing_lots,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> OperationTypeId {
        OperationTypeId(self.id)
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

    /// Set the warehouse_id field (chainable)
    pub fn with_warehouse_id(mut self, value: Uuid) -> Self {
        self.warehouse_id = Some(value);
        self
    }

    /// Set the reservation_days_before field (chainable)
    pub fn with_reservation_days_before(mut self, value: i32) -> Self {
        self.reservation_days_before = Some(value);
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
                "sequence_code" => {
                    if let Ok(v) = serde_json::from_value(value) { self.sequence_code = v; }
                }
                "code" => {
                    if let Ok(v) = serde_json::from_value(value) { self.code = v; }
                }
                "active" => {
                    if let Ok(v) = serde_json::from_value(value) { self.active = v; }
                }
                "sequence" => {
                    if let Ok(v) = serde_json::from_value(value) { self.sequence = v; }
                }
                "warehouse_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.warehouse_id = v; }
                }
                "default_location_src_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.default_location_src_id = v; }
                }
                "default_location_dest_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.default_location_dest_id = v; }
                }
                "reservation_method" => {
                    if let Ok(v) = serde_json::from_value(value) { self.reservation_method = v; }
                }
                "reservation_days_before" => {
                    if let Ok(v) = serde_json::from_value(value) { self.reservation_days_before = v; }
                }
                "move_type" => {
                    if let Ok(v) = serde_json::from_value(value) { self.move_type = v; }
                }
                "create_backorder" => {
                    if let Ok(v) = serde_json::from_value(value) { self.create_backorder = v; }
                }
                "use_create_lots" => {
                    if let Ok(v) = serde_json::from_value(value) { self.use_create_lots = v; }
                }
                "use_existing_lots" => {
                    if let Ok(v) = serde_json::from_value(value) { self.use_existing_lots = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for OperationType {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "OperationType"
    }
}

impl backbone_core::PersistentEntity for OperationType {
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

impl backbone_orm::EntityRepoMeta for OperationType {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("warehouse_id".to_string(), "uuid".to_string());
        m.insert("default_location_src_id".to_string(), "uuid".to_string());
        m.insert("default_location_dest_id".to_string(), "uuid".to_string());
        m.insert("code".to_string(), "picking_code".to_string());
        m.insert("reservation_method".to_string(), "reservation_method".to_string());
        m.insert("move_type".to_string(), "move_type".to_string());
        m.insert("create_backorder".to_string(), "create_backorder".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["name", "sequence_code"]
    }
}

/// Builder for OperationType entity
///
/// Provides a fluent API for constructing OperationType instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct OperationTypeBuilder {
    name: Option<String>,
    sequence_code: Option<String>,
    code: Option<PickingCode>,
    active: Option<bool>,
    sequence: Option<i32>,
    warehouse_id: Option<Uuid>,
    default_location_src_id: Option<Uuid>,
    default_location_dest_id: Option<Uuid>,
    reservation_method: Option<ReservationMethod>,
    reservation_days_before: Option<i32>,
    move_type: Option<MoveType>,
    create_backorder: Option<CreateBackorder>,
    use_create_lots: Option<bool>,
    use_existing_lots: Option<bool>,
}

impl OperationTypeBuilder {
    /// Set the name field (required)
    pub fn name(mut self, value: String) -> Self {
        self.name = Some(value);
        self
    }

    /// Set the sequence_code field (required)
    pub fn sequence_code(mut self, value: String) -> Self {
        self.sequence_code = Some(value);
        self
    }

    /// Set the code field (default: `PickingCode::default()`)
    pub fn code(mut self, value: PickingCode) -> Self {
        self.code = Some(value);
        self
    }

    /// Set the active field (default: `true`)
    pub fn active(mut self, value: bool) -> Self {
        self.active = Some(value);
        self
    }

    /// Set the sequence field (default: `10`)
    pub fn sequence(mut self, value: i32) -> Self {
        self.sequence = Some(value);
        self
    }

    /// Set the warehouse_id field (optional)
    pub fn warehouse_id(mut self, value: Uuid) -> Self {
        self.warehouse_id = Some(value);
        self
    }

    /// Set the default_location_src_id field (required)
    pub fn default_location_src_id(mut self, value: Uuid) -> Self {
        self.default_location_src_id = Some(value);
        self
    }

    /// Set the default_location_dest_id field (required)
    pub fn default_location_dest_id(mut self, value: Uuid) -> Self {
        self.default_location_dest_id = Some(value);
        self
    }

    /// Set the reservation_method field (default: `ReservationMethod::default()`)
    pub fn reservation_method(mut self, value: ReservationMethod) -> Self {
        self.reservation_method = Some(value);
        self
    }

    /// Set the reservation_days_before field (optional)
    pub fn reservation_days_before(mut self, value: i32) -> Self {
        self.reservation_days_before = Some(value);
        self
    }

    /// Set the move_type field (default: `MoveType::default()`)
    pub fn move_type(mut self, value: MoveType) -> Self {
        self.move_type = Some(value);
        self
    }

    /// Set the create_backorder field (default: `CreateBackorder::default()`)
    pub fn create_backorder(mut self, value: CreateBackorder) -> Self {
        self.create_backorder = Some(value);
        self
    }

    /// Set the use_create_lots field (default: `true`)
    pub fn use_create_lots(mut self, value: bool) -> Self {
        self.use_create_lots = Some(value);
        self
    }

    /// Set the use_existing_lots field (default: `true`)
    pub fn use_existing_lots(mut self, value: bool) -> Self {
        self.use_existing_lots = Some(value);
        self
    }

    /// Build the OperationType entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<OperationType, String> {
        let name = self.name.ok_or_else(|| "name is required".to_string())?;
        let sequence_code = self.sequence_code.ok_or_else(|| "sequence_code is required".to_string())?;
        let default_location_src_id = self.default_location_src_id.ok_or_else(|| "default_location_src_id is required".to_string())?;
        let default_location_dest_id = self.default_location_dest_id.ok_or_else(|| "default_location_dest_id is required".to_string())?;

        Ok(OperationType {
            id: Uuid::new_v4(),
            name,
            sequence_code,
            code: self.code.unwrap_or_default(),
            active: self.active.unwrap_or(true),
            sequence: self.sequence.unwrap_or(10),
            warehouse_id: self.warehouse_id,
            default_location_src_id,
            default_location_dest_id,
            reservation_method: self.reservation_method.unwrap_or_default(),
            reservation_days_before: self.reservation_days_before,
            move_type: self.move_type.unwrap_or_default(),
            create_backorder: self.create_backorder.unwrap_or_default(),
            use_create_lots: self.use_create_lots.unwrap_or(true),
            use_existing_lots: self.use_existing_lots.unwrap_or(true),
            metadata: AuditMetadata::default(),
        })
    }
}
