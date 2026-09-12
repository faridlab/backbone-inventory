use chrono::{DateTime, Utc, NaiveDate};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use rust_decimal::Decimal;
use super::AuditMetadata;

/// Strongly-typed ID for Quant
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct QuantId(pub Uuid);

impl QuantId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for QuantId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for QuantId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for QuantId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<QuantId> for Uuid {
    fn from(id: QuantId) -> Self { id.0 }
}

impl AsRef<Uuid> for QuantId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for QuantId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Quant {
    pub id: Uuid,
    pub item_id: Uuid,
    pub location_id: Uuid,
    pub lot_id: Option<Uuid>,
    pub package_id: Option<Uuid>,
    pub owner_id: Option<Uuid>,
    pub quantity: Decimal,
    pub reserved_quantity: Decimal,
    pub available_quantity: Decimal,
    pub in_date: Option<DateTime<Utc>>,
    pub inventory_quantity: Option<Decimal>,
    pub inventory_diff_quantity: Option<Decimal>,
    pub inventory_quantity_set: bool,
    pub inventory_date: Option<NaiveDate>,
    pub sn_duplicated: bool,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl Quant {
    /// Create a builder for Quant
    pub fn builder() -> QuantBuilder {
        <QuantBuilder as Default>::default()
    }

    /// Create a new Quant with required fields
    pub fn new(item_id: Uuid, location_id: Uuid, quantity: Decimal, reserved_quantity: Decimal, available_quantity: Decimal, inventory_quantity_set: bool, sn_duplicated: bool) -> Self {
        Self {
            id: Uuid::new_v4(),
            item_id,
            location_id,
            lot_id: None,
            package_id: None,
            owner_id: None,
            quantity,
            reserved_quantity,
            available_quantity,
            in_date: None,
            inventory_quantity: None,
            inventory_diff_quantity: None,
            inventory_quantity_set,
            inventory_date: None,
            sn_duplicated,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> QuantId {
        QuantId(self.id)
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

    /// Set the in_date field (chainable)
    pub fn with_in_date(mut self, value: DateTime<Utc>) -> Self {
        self.in_date = Some(value);
        self
    }

    /// Set the inventory_quantity field (chainable)
    pub fn with_inventory_quantity(mut self, value: Decimal) -> Self {
        self.inventory_quantity = Some(value);
        self
    }

    /// Set the inventory_diff_quantity field (chainable)
    pub fn with_inventory_diff_quantity(mut self, value: Decimal) -> Self {
        self.inventory_diff_quantity = Some(value);
        self
    }

    /// Set the inventory_date field (chainable)
    pub fn with_inventory_date(mut self, value: NaiveDate) -> Self {
        self.inventory_date = Some(value);
        self
    }

    // ==========================================================
    // Partial Update
    // ==========================================================

    /// Apply partial updates from a map of field name to JSON value
    pub fn apply_patch(&mut self, fields: std::collections::HashMap<String, serde_json::Value>) {
        for (key, value) in fields {
            match key.as_str() {
                "item_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.item_id = v; }
                }
                "location_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.location_id = v; }
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
                "quantity" => {
                    if let Ok(v) = serde_json::from_value(value) { self.quantity = v; }
                }
                "reserved_quantity" => {
                    if let Ok(v) = serde_json::from_value(value) { self.reserved_quantity = v; }
                }
                "available_quantity" => {
                    if let Ok(v) = serde_json::from_value(value) { self.available_quantity = v; }
                }
                "in_date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.in_date = v; }
                }
                "inventory_quantity" => {
                    if let Ok(v) = serde_json::from_value(value) { self.inventory_quantity = v; }
                }
                "inventory_diff_quantity" => {
                    if let Ok(v) = serde_json::from_value(value) { self.inventory_diff_quantity = v; }
                }
                "inventory_quantity_set" => {
                    if let Ok(v) = serde_json::from_value(value) { self.inventory_quantity_set = v; }
                }
                "inventory_date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.inventory_date = v; }
                }
                "sn_duplicated" => {
                    if let Ok(v) = serde_json::from_value(value) { self.sn_duplicated = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for Quant {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "Quant"
    }
}

impl backbone_core::PersistentEntity for Quant {
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

impl backbone_orm::EntityRepoMeta for Quant {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("item_id".to_string(), "uuid".to_string());
        m.insert("location_id".to_string(), "uuid".to_string());
        m.insert("lot_id".to_string(), "uuid".to_string());
        m.insert("package_id".to_string(), "uuid".to_string());
        m.insert("owner_id".to_string(), "uuid".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &[]
    }
}

/// Builder for Quant entity
///
/// Provides a fluent API for constructing Quant instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct QuantBuilder {
    item_id: Option<Uuid>,
    location_id: Option<Uuid>,
    lot_id: Option<Uuid>,
    package_id: Option<Uuid>,
    owner_id: Option<Uuid>,
    quantity: Option<Decimal>,
    reserved_quantity: Option<Decimal>,
    available_quantity: Option<Decimal>,
    in_date: Option<DateTime<Utc>>,
    inventory_quantity: Option<Decimal>,
    inventory_diff_quantity: Option<Decimal>,
    inventory_quantity_set: Option<bool>,
    inventory_date: Option<NaiveDate>,
    sn_duplicated: Option<bool>,
}

impl QuantBuilder {
    /// Set the item_id field (required)
    pub fn item_id(mut self, value: Uuid) -> Self {
        self.item_id = Some(value);
        self
    }

    /// Set the location_id field (required)
    pub fn location_id(mut self, value: Uuid) -> Self {
        self.location_id = Some(value);
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

    /// Set the quantity field (default: `Decimal::from(0)`)
    pub fn quantity(mut self, value: Decimal) -> Self {
        self.quantity = Some(value);
        self
    }

    /// Set the reserved_quantity field (default: `Decimal::from(0)`)
    pub fn reserved_quantity(mut self, value: Decimal) -> Self {
        self.reserved_quantity = Some(value);
        self
    }

    /// Set the available_quantity field (default: `Decimal::from(0)`)
    pub fn available_quantity(mut self, value: Decimal) -> Self {
        self.available_quantity = Some(value);
        self
    }

    /// Set the in_date field (optional)
    pub fn in_date(mut self, value: DateTime<Utc>) -> Self {
        self.in_date = Some(value);
        self
    }

    /// Set the inventory_quantity field (optional)
    pub fn inventory_quantity(mut self, value: Decimal) -> Self {
        self.inventory_quantity = Some(value);
        self
    }

    /// Set the inventory_diff_quantity field (optional)
    pub fn inventory_diff_quantity(mut self, value: Decimal) -> Self {
        self.inventory_diff_quantity = Some(value);
        self
    }

    /// Set the inventory_quantity_set field (default: `false`)
    pub fn inventory_quantity_set(mut self, value: bool) -> Self {
        self.inventory_quantity_set = Some(value);
        self
    }

    /// Set the inventory_date field (optional)
    pub fn inventory_date(mut self, value: NaiveDate) -> Self {
        self.inventory_date = Some(value);
        self
    }

    /// Set the sn_duplicated field (default: `false`)
    pub fn sn_duplicated(mut self, value: bool) -> Self {
        self.sn_duplicated = Some(value);
        self
    }

    /// Build the Quant entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<Quant, String> {
        let item_id = self.item_id.ok_or_else(|| "item_id is required".to_string())?;
        let location_id = self.location_id.ok_or_else(|| "location_id is required".to_string())?;

        Ok(Quant {
            id: Uuid::new_v4(),
            item_id,
            location_id,
            lot_id: self.lot_id,
            package_id: self.package_id,
            owner_id: self.owner_id,
            quantity: self.quantity.unwrap_or(Decimal::from(0)),
            reserved_quantity: self.reserved_quantity.unwrap_or(Decimal::from(0)),
            available_quantity: self.available_quantity.unwrap_or(Decimal::from(0)),
            in_date: self.in_date,
            inventory_quantity: self.inventory_quantity,
            inventory_diff_quantity: self.inventory_diff_quantity,
            inventory_quantity_set: self.inventory_quantity_set.unwrap_or(false),
            inventory_date: self.inventory_date,
            sn_duplicated: self.sn_duplicated.unwrap_or(false),
            metadata: AuditMetadata::default(),
        })
    }
}
