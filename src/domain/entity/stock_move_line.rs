use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use rust_decimal::Decimal;

use super::MoveState;
use super::AuditMetadata;

/// Strongly-typed ID for StockMoveLine
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StockMoveLineId(pub Uuid);

impl StockMoveLineId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for StockMoveLineId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for StockMoveLineId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for StockMoveLineId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<StockMoveLineId> for Uuid {
    fn from(id: StockMoveLineId) -> Self { id.0 }
}

impl AsRef<Uuid> for StockMoveLineId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for StockMoveLineId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct StockMoveLine {
    pub id: Uuid,
    pub quantity: Decimal,
    pub quantity_product_uom_id: Option<Uuid>,
    pub picked: bool,
    pub lot_id: Option<Uuid>,
    pub package_id: Option<Uuid>,
    pub result_package_id: Option<Uuid>,
    pub owner_id: Option<Uuid>,
    pub state: MoveState,
    pub date: DateTime<Utc>,
    pub move_id: Uuid,
    pub picking_id: Option<Uuid>,
    pub location_id: Uuid,
    pub location_dest_id: Uuid,
    pub item_id: Uuid,
    pub company_id: Uuid,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl StockMoveLine {
    /// Create a builder for StockMoveLine
    pub fn builder() -> StockMoveLineBuilder {
        <StockMoveLineBuilder as Default>::default()
    }

    /// Create a new StockMoveLine with required fields
    pub fn new(quantity: Decimal, picked: bool, state: MoveState, date: DateTime<Utc>, move_id: Uuid, location_id: Uuid, location_dest_id: Uuid, item_id: Uuid, company_id: Uuid) -> Self {
        Self {
            id: Uuid::new_v4(),
            quantity,
            quantity_product_uom_id: None,
            picked,
            lot_id: None,
            package_id: None,
            result_package_id: None,
            owner_id: None,
            state,
            date,
            move_id,
            picking_id: None,
            location_id,
            location_dest_id,
            item_id,
            company_id,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> StockMoveLineId {
        StockMoveLineId(self.id)
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

    /// Set the quantity_product_uom_id field (chainable)
    pub fn with_quantity_product_uom_id(mut self, value: Uuid) -> Self {
        self.quantity_product_uom_id = Some(value);
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

    /// Set the result_package_id field (chainable)
    pub fn with_result_package_id(mut self, value: Uuid) -> Self {
        self.result_package_id = Some(value);
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

    // ==========================================================
    // Partial Update
    // ==========================================================

    /// Apply partial updates from a map of field name to JSON value
    pub fn apply_patch(&mut self, fields: std::collections::HashMap<String, serde_json::Value>) {
        for (key, value) in fields {
            match key.as_str() {
                "quantity" => {
                    if let Ok(v) = serde_json::from_value(value) { self.quantity = v; }
                }
                "quantity_product_uom_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.quantity_product_uom_id = v; }
                }
                "picked" => {
                    if let Ok(v) = serde_json::from_value(value) { self.picked = v; }
                }
                "lot_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.lot_id = v; }
                }
                "package_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.package_id = v; }
                }
                "result_package_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.result_package_id = v; }
                }
                "owner_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.owner_id = v; }
                }
                "state" => {
                    if let Ok(v) = serde_json::from_value(value) { self.state = v; }
                }
                "date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.date = v; }
                }
                "move_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.move_id = v; }
                }
                "picking_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.picking_id = v; }
                }
                "location_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.location_id = v; }
                }
                "location_dest_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.location_dest_id = v; }
                }
                "item_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.item_id = v; }
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

impl super::Entity for StockMoveLine {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "StockMoveLine"
    }
}

impl backbone_core::PersistentEntity for StockMoveLine {
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

impl backbone_orm::EntityRepoMeta for StockMoveLine {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("quantity_product_uom_id".to_string(), "uuid".to_string());
        m.insert("lot_id".to_string(), "uuid".to_string());
        m.insert("package_id".to_string(), "uuid".to_string());
        m.insert("result_package_id".to_string(), "uuid".to_string());
        m.insert("owner_id".to_string(), "uuid".to_string());
        m.insert("move_id".to_string(), "uuid".to_string());
        m.insert("picking_id".to_string(), "uuid".to_string());
        m.insert("location_id".to_string(), "uuid".to_string());
        m.insert("location_dest_id".to_string(), "uuid".to_string());
        m.insert("item_id".to_string(), "uuid".to_string());
        m.insert("company_id".to_string(), "uuid".to_string());
        m.insert("state".to_string(), "move_state".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &[]
    }
    fn company_field() -> Option<&'static str> {
        Some("company_id")
    }
    fn relations() -> &'static [(&'static str, &'static str, &'static str)] {
        &[("move", "stock_moves", "moveId"), ("picking", "transfers", "pickingId")]
    }
}

/// Builder for StockMoveLine entity
///
/// Provides a fluent API for constructing StockMoveLine instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct StockMoveLineBuilder {
    quantity: Option<Decimal>,
    quantity_product_uom_id: Option<Uuid>,
    picked: Option<bool>,
    lot_id: Option<Uuid>,
    package_id: Option<Uuid>,
    result_package_id: Option<Uuid>,
    owner_id: Option<Uuid>,
    state: Option<MoveState>,
    date: Option<DateTime<Utc>>,
    move_id: Option<Uuid>,
    picking_id: Option<Uuid>,
    location_id: Option<Uuid>,
    location_dest_id: Option<Uuid>,
    item_id: Option<Uuid>,
    company_id: Option<Uuid>,
}

impl StockMoveLineBuilder {
    /// Set the quantity field (default: `Decimal::from(0)`)
    pub fn quantity(mut self, value: Decimal) -> Self {
        self.quantity = Some(value);
        self
    }

    /// Set the quantity_product_uom_id field (optional)
    pub fn quantity_product_uom_id(mut self, value: Uuid) -> Self {
        self.quantity_product_uom_id = Some(value);
        self
    }

    /// Set the picked field (default: `false`)
    pub fn picked(mut self, value: bool) -> Self {
        self.picked = Some(value);
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

    /// Set the result_package_id field (optional)
    pub fn result_package_id(mut self, value: Uuid) -> Self {
        self.result_package_id = Some(value);
        self
    }

    /// Set the owner_id field (optional)
    pub fn owner_id(mut self, value: Uuid) -> Self {
        self.owner_id = Some(value);
        self
    }

    /// Set the state field (default: `MoveState::default()`)
    pub fn state(mut self, value: MoveState) -> Self {
        self.state = Some(value);
        self
    }

    /// Set the date field (default: `Utc::now()`)
    pub fn date(mut self, value: DateTime<Utc>) -> Self {
        self.date = Some(value);
        self
    }

    /// Set the move_id field (required)
    pub fn move_id(mut self, value: Uuid) -> Self {
        self.move_id = Some(value);
        self
    }

    /// Set the picking_id field (optional)
    pub fn picking_id(mut self, value: Uuid) -> Self {
        self.picking_id = Some(value);
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

    /// Set the item_id field (required)
    pub fn item_id(mut self, value: Uuid) -> Self {
        self.item_id = Some(value);
        self
    }

    /// Set the company_id field (required)
    pub fn company_id(mut self, value: Uuid) -> Self {
        self.company_id = Some(value);
        self
    }

    /// Build the StockMoveLine entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<StockMoveLine, String> {
        let move_id = self.move_id.ok_or_else(|| "move_id is required".to_string())?;
        let location_id = self.location_id.ok_or_else(|| "location_id is required".to_string())?;
        let location_dest_id = self.location_dest_id.ok_or_else(|| "location_dest_id is required".to_string())?;
        let item_id = self.item_id.ok_or_else(|| "item_id is required".to_string())?;
        let company_id = self.company_id.ok_or_else(|| "company_id is required".to_string())?;

        Ok(StockMoveLine {
            id: Uuid::new_v4(),
            quantity: self.quantity.unwrap_or(Decimal::from(0)),
            quantity_product_uom_id: self.quantity_product_uom_id,
            picked: self.picked.unwrap_or(false),
            lot_id: self.lot_id,
            package_id: self.package_id,
            result_package_id: self.result_package_id,
            owner_id: self.owner_id,
            state: self.state.unwrap_or_default(),
            date: self.date.unwrap_or(Utc::now()),
            move_id,
            picking_id: self.picking_id,
            location_id,
            location_dest_id,
            item_id,
            company_id,
            metadata: AuditMetadata::default(),
        })
    }
}
