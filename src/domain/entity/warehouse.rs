use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use rust_decimal::Decimal;

use super::WarehouseType;
use super::WarehouseStatus;
use super::AuditMetadata;

/// Strongly-typed ID for Warehouse
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WarehouseId(pub Uuid);

impl WarehouseId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for WarehouseId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for WarehouseId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for WarehouseId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<WarehouseId> for Uuid {
    fn from(id: WarehouseId) -> Self { id.0 }
}

impl AsRef<Uuid> for WarehouseId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for WarehouseId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Warehouse {
    pub id: Uuid,
    pub provider_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outlet_id: Option<Uuid>,
    pub code: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub warehouse_type: WarehouseType,
    pub is_main: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contact_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contact_phone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contact_email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_capacity: Option<Decimal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capacity_unit: Option<String>,
    pub allow_receipt: bool,
    pub allow_issue: bool,
    pub allow_transfer_in: bool,
    pub allow_transfer_out: bool,
    pub allow_adjustment: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_inventory_account_id: Option<Uuid>,
    pub status: WarehouseStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl Warehouse {
    /// Create a builder for Warehouse
    pub fn builder() -> WarehouseBuilder {
        WarehouseBuilder::default()
    }

    /// Create a new Warehouse with required fields
    pub fn new(provider_id: Uuid, code: String, name: String, warehouse_type: WarehouseType, is_main: bool, allow_receipt: bool, allow_issue: bool, allow_transfer_in: bool, allow_transfer_out: bool, allow_adjustment: bool, status: WarehouseStatus) -> Self {
        Self {
            id: Uuid::new_v4(),
            provider_id,
            outlet_id: None,
            code,
            name,
            description: None,
            warehouse_type,
            is_main,
            address_id: None,
            contact_name: None,
            contact_phone: None,
            contact_email: None,
            total_capacity: None,
            capacity_unit: None,
            allow_receipt,
            allow_issue,
            allow_transfer_in,
            allow_transfer_out,
            allow_adjustment,
            default_inventory_account_id: None,
            status,
            notes: None,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> WarehouseId {
        WarehouseId(self.id)
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

    /// Get the current status
    pub fn status(&self) -> &WarehouseStatus {
        &self.status
    }


    // ==========================================================
    // Fluent Setters (with_* for optional fields)
    // ==========================================================

    /// Set the outlet_id field (chainable)
    pub fn with_outlet_id(mut self, value: Uuid) -> Self {
        self.outlet_id = Some(value);
        self
    }

    /// Set the description field (chainable)
    pub fn with_description(mut self, value: String) -> Self {
        self.description = Some(value);
        self
    }

    /// Set the address_id field (chainable)
    pub fn with_address_id(mut self, value: Uuid) -> Self {
        self.address_id = Some(value);
        self
    }

    /// Set the contact_name field (chainable)
    pub fn with_contact_name(mut self, value: String) -> Self {
        self.contact_name = Some(value);
        self
    }

    /// Set the contact_phone field (chainable)
    pub fn with_contact_phone(mut self, value: String) -> Self {
        self.contact_phone = Some(value);
        self
    }

    /// Set the contact_email field (chainable)
    pub fn with_contact_email(mut self, value: String) -> Self {
        self.contact_email = Some(value);
        self
    }

    /// Set the total_capacity field (chainable)
    pub fn with_total_capacity(mut self, value: Decimal) -> Self {
        self.total_capacity = Some(value);
        self
    }

    /// Set the capacity_unit field (chainable)
    pub fn with_capacity_unit(mut self, value: String) -> Self {
        self.capacity_unit = Some(value);
        self
    }

    /// Set the default_inventory_account_id field (chainable)
    pub fn with_default_inventory_account_id(mut self, value: Uuid) -> Self {
        self.default_inventory_account_id = Some(value);
        self
    }

    /// Set the notes field (chainable)
    pub fn with_notes(mut self, value: String) -> Self {
        self.notes = Some(value);
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
                "outlet_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.outlet_id = v; }
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
                "warehouse_type" => {
                    if let Ok(v) = serde_json::from_value(value) { self.warehouse_type = v; }
                }
                "is_main" => {
                    if let Ok(v) = serde_json::from_value(value) { self.is_main = v; }
                }
                "address_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.address_id = v; }
                }
                "contact_name" => {
                    if let Ok(v) = serde_json::from_value(value) { self.contact_name = v; }
                }
                "contact_phone" => {
                    if let Ok(v) = serde_json::from_value(value) { self.contact_phone = v; }
                }
                "contact_email" => {
                    if let Ok(v) = serde_json::from_value(value) { self.contact_email = v; }
                }
                "total_capacity" => {
                    if let Ok(v) = serde_json::from_value(value) { self.total_capacity = v; }
                }
                "capacity_unit" => {
                    if let Ok(v) = serde_json::from_value(value) { self.capacity_unit = v; }
                }
                "allow_receipt" => {
                    if let Ok(v) = serde_json::from_value(value) { self.allow_receipt = v; }
                }
                "allow_issue" => {
                    if let Ok(v) = serde_json::from_value(value) { self.allow_issue = v; }
                }
                "allow_transfer_in" => {
                    if let Ok(v) = serde_json::from_value(value) { self.allow_transfer_in = v; }
                }
                "allow_transfer_out" => {
                    if let Ok(v) = serde_json::from_value(value) { self.allow_transfer_out = v; }
                }
                "allow_adjustment" => {
                    if let Ok(v) = serde_json::from_value(value) { self.allow_adjustment = v; }
                }
                "default_inventory_account_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.default_inventory_account_id = v; }
                }
                "status" => {
                    if let Ok(v) = serde_json::from_value(value) { self.status = v; }
                }
                "notes" => {
                    if let Ok(v) = serde_json::from_value(value) { self.notes = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for Warehouse {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "Warehouse"
    }
}

impl backbone_core::PersistentEntity for Warehouse {
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

impl backbone_orm::EntityRepoMeta for Warehouse {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("provider_id".to_string(), "uuid".to_string());
        m.insert("outlet_id".to_string(), "uuid".to_string());
        m.insert("address_id".to_string(), "uuid".to_string());
        m.insert("default_inventory_account_id".to_string(), "uuid".to_string());
        m.insert("warehouse_type".to_string(), "warehouse_type".to_string());
        m.insert("status".to_string(), "warehouse_status".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["code", "name"]
    }
}

/// Builder for Warehouse entity
///
/// Provides a fluent API for constructing Warehouse instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct WarehouseBuilder {
    provider_id: Option<Uuid>,
    outlet_id: Option<Uuid>,
    code: Option<String>,
    name: Option<String>,
    description: Option<String>,
    warehouse_type: Option<WarehouseType>,
    is_main: Option<bool>,
    address_id: Option<Uuid>,
    contact_name: Option<String>,
    contact_phone: Option<String>,
    contact_email: Option<String>,
    total_capacity: Option<Decimal>,
    capacity_unit: Option<String>,
    allow_receipt: Option<bool>,
    allow_issue: Option<bool>,
    allow_transfer_in: Option<bool>,
    allow_transfer_out: Option<bool>,
    allow_adjustment: Option<bool>,
    default_inventory_account_id: Option<Uuid>,
    status: Option<WarehouseStatus>,
    notes: Option<String>,
}

impl WarehouseBuilder {
    /// Set the provider_id field (required)
    pub fn provider_id(mut self, value: Uuid) -> Self {
        self.provider_id = Some(value);
        self
    }

    /// Set the outlet_id field (optional)
    pub fn outlet_id(mut self, value: Uuid) -> Self {
        self.outlet_id = Some(value);
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

    /// Set the warehouse_type field (default: `WarehouseType::default()`)
    pub fn warehouse_type(mut self, value: WarehouseType) -> Self {
        self.warehouse_type = Some(value);
        self
    }

    /// Set the is_main field (default: `false`)
    pub fn is_main(mut self, value: bool) -> Self {
        self.is_main = Some(value);
        self
    }

    /// Set the address_id field (optional)
    pub fn address_id(mut self, value: Uuid) -> Self {
        self.address_id = Some(value);
        self
    }

    /// Set the contact_name field (optional)
    pub fn contact_name(mut self, value: String) -> Self {
        self.contact_name = Some(value);
        self
    }

    /// Set the contact_phone field (optional)
    pub fn contact_phone(mut self, value: String) -> Self {
        self.contact_phone = Some(value);
        self
    }

    /// Set the contact_email field (optional)
    pub fn contact_email(mut self, value: String) -> Self {
        self.contact_email = Some(value);
        self
    }

    /// Set the total_capacity field (optional)
    pub fn total_capacity(mut self, value: Decimal) -> Self {
        self.total_capacity = Some(value);
        self
    }

    /// Set the capacity_unit field (optional)
    pub fn capacity_unit(mut self, value: String) -> Self {
        self.capacity_unit = Some(value);
        self
    }

    /// Set the allow_receipt field (default: `true`)
    pub fn allow_receipt(mut self, value: bool) -> Self {
        self.allow_receipt = Some(value);
        self
    }

    /// Set the allow_issue field (default: `true`)
    pub fn allow_issue(mut self, value: bool) -> Self {
        self.allow_issue = Some(value);
        self
    }

    /// Set the allow_transfer_in field (default: `true`)
    pub fn allow_transfer_in(mut self, value: bool) -> Self {
        self.allow_transfer_in = Some(value);
        self
    }

    /// Set the allow_transfer_out field (default: `true`)
    pub fn allow_transfer_out(mut self, value: bool) -> Self {
        self.allow_transfer_out = Some(value);
        self
    }

    /// Set the allow_adjustment field (default: `true`)
    pub fn allow_adjustment(mut self, value: bool) -> Self {
        self.allow_adjustment = Some(value);
        self
    }

    /// Set the default_inventory_account_id field (optional)
    pub fn default_inventory_account_id(mut self, value: Uuid) -> Self {
        self.default_inventory_account_id = Some(value);
        self
    }

    /// Set the status field (default: `WarehouseStatus::default()`)
    pub fn status(mut self, value: WarehouseStatus) -> Self {
        self.status = Some(value);
        self
    }

    /// Set the notes field (optional)
    pub fn notes(mut self, value: String) -> Self {
        self.notes = Some(value);
        self
    }

    /// Build the Warehouse entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<Warehouse, String> {
        let provider_id = self.provider_id.ok_or_else(|| "provider_id is required".to_string())?;
        let code = self.code.ok_or_else(|| "code is required".to_string())?;
        let name = self.name.ok_or_else(|| "name is required".to_string())?;

        Ok(Warehouse {
            id: Uuid::new_v4(),
            provider_id,
            outlet_id: self.outlet_id,
            code,
            name,
            description: self.description,
            warehouse_type: self.warehouse_type.unwrap_or(WarehouseType::default()),
            is_main: self.is_main.unwrap_or(false),
            address_id: self.address_id,
            contact_name: self.contact_name,
            contact_phone: self.contact_phone,
            contact_email: self.contact_email,
            total_capacity: self.total_capacity,
            capacity_unit: self.capacity_unit,
            allow_receipt: self.allow_receipt.unwrap_or(true),
            allow_issue: self.allow_issue.unwrap_or(true),
            allow_transfer_in: self.allow_transfer_in.unwrap_or(true),
            allow_transfer_out: self.allow_transfer_out.unwrap_or(true),
            allow_adjustment: self.allow_adjustment.unwrap_or(true),
            default_inventory_account_id: self.default_inventory_account_id,
            status: self.status.unwrap_or(WarehouseStatus::default()),
            notes: self.notes,
            metadata: AuditMetadata::default(),
        })
    }
}
