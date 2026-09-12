use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use rust_decimal::Decimal;

use super::MoveState;
use super::GlPostingState;
use super::Priority;
use super::ProcureMethod;
use super::AuditMetadata;

/// Strongly-typed ID for StockMove
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StockMoveId(pub Uuid);

impl StockMoveId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for StockMoveId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for StockMoveId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for StockMoveId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<StockMoveId> for Uuid {
    fn from(id: StockMoveId) -> Self { id.0 }
}

impl AsRef<Uuid> for StockMoveId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for StockMoveId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct StockMove {
    pub id: Uuid,
    pub name: String,
    pub state: MoveState,
    pub posting_state: GlPostingState,
    pub priority: Priority,
    pub create_date: DateTime<Utc>,
    pub date: DateTime<Utc>,
    pub item_id: Uuid,
    pub demand_qty: Decimal,
    pub quantity: Decimal,
    pub price_unit: Decimal,
    pub forced_value: Option<Decimal>,
    pub procure_method: ProcureMethod,
    pub picking_id: Option<Uuid>,
    pub origin: Option<String>,
    pub location_id: Uuid,
    pub location_dest_id: Uuid,
    pub partner_id: Option<Uuid>,
    pub rule_id: Option<Uuid>,
    pub warehouse_id: Option<Uuid>,
    pub orderpoint_id: Option<Uuid>,
    pub move_orig_ids: Vec<Uuid>,
    pub move_dest_ids: Vec<Uuid>,
    pub is_inventory: bool,
    pub scrapped: bool,
    pub propagate_cancel: bool,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl StockMove {
    /// Create a builder for StockMove
    pub fn builder() -> StockMoveBuilder {
        <StockMoveBuilder as Default>::default()
    }

    /// Create a new StockMove with required fields
    pub fn new(name: String, state: MoveState, posting_state: GlPostingState, priority: Priority, create_date: DateTime<Utc>, date: DateTime<Utc>, item_id: Uuid, demand_qty: Decimal, quantity: Decimal, price_unit: Decimal, procure_method: ProcureMethod, location_id: Uuid, location_dest_id: Uuid, move_orig_ids: Vec<Uuid>, move_dest_ids: Vec<Uuid>, is_inventory: bool, scrapped: bool, propagate_cancel: bool) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            state,
            posting_state,
            priority,
            create_date,
            date,
            item_id,
            demand_qty,
            quantity,
            price_unit,
            forced_value: None,
            procure_method,
            picking_id: None,
            origin: None,
            location_id,
            location_dest_id,
            partner_id: None,
            rule_id: None,
            warehouse_id: None,
            orderpoint_id: None,
            move_orig_ids,
            move_dest_ids,
            is_inventory,
            scrapped,
            propagate_cancel,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> StockMoveId {
        StockMoveId(self.id)
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

    /// Set the forced_value field (chainable)
    pub fn with_forced_value(mut self, value: Decimal) -> Self {
        self.forced_value = Some(value);
        self
    }

    /// Set the picking_id field (chainable)
    pub fn with_picking_id(mut self, value: Uuid) -> Self {
        self.picking_id = Some(value);
        self
    }

    /// Set the origin field (chainable)
    pub fn with_origin(mut self, value: String) -> Self {
        self.origin = Some(value);
        self
    }

    /// Set the partner_id field (chainable)
    pub fn with_partner_id(mut self, value: Uuid) -> Self {
        self.partner_id = Some(value);
        self
    }

    /// Set the rule_id field (chainable)
    pub fn with_rule_id(mut self, value: Uuid) -> Self {
        self.rule_id = Some(value);
        self
    }

    /// Set the warehouse_id field (chainable)
    pub fn with_warehouse_id(mut self, value: Uuid) -> Self {
        self.warehouse_id = Some(value);
        self
    }

    /// Set the orderpoint_id field (chainable)
    pub fn with_orderpoint_id(mut self, value: Uuid) -> Self {
        self.orderpoint_id = Some(value);
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
                "posting_state" => {
                    if let Ok(v) = serde_json::from_value(value) { self.posting_state = v; }
                }
                "priority" => {
                    if let Ok(v) = serde_json::from_value(value) { self.priority = v; }
                }
                "create_date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.create_date = v; }
                }
                "date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.date = v; }
                }
                "item_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.item_id = v; }
                }
                "demand_qty" => {
                    if let Ok(v) = serde_json::from_value(value) { self.demand_qty = v; }
                }
                "quantity" => {
                    if let Ok(v) = serde_json::from_value(value) { self.quantity = v; }
                }
                "price_unit" => {
                    if let Ok(v) = serde_json::from_value(value) { self.price_unit = v; }
                }
                "forced_value" => {
                    if let Ok(v) = serde_json::from_value(value) { self.forced_value = v; }
                }
                "procure_method" => {
                    if let Ok(v) = serde_json::from_value(value) { self.procure_method = v; }
                }
                "picking_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.picking_id = v; }
                }
                "origin" => {
                    if let Ok(v) = serde_json::from_value(value) { self.origin = v; }
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
                "rule_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.rule_id = v; }
                }
                "warehouse_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.warehouse_id = v; }
                }
                "orderpoint_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.orderpoint_id = v; }
                }
                "move_orig_ids" => {
                    if let Ok(v) = serde_json::from_value(value) { self.move_orig_ids = v; }
                }
                "move_dest_ids" => {
                    if let Ok(v) = serde_json::from_value(value) { self.move_dest_ids = v; }
                }
                "is_inventory" => {
                    if let Ok(v) = serde_json::from_value(value) { self.is_inventory = v; }
                }
                "scrapped" => {
                    if let Ok(v) = serde_json::from_value(value) { self.scrapped = v; }
                }
                "propagate_cancel" => {
                    if let Ok(v) = serde_json::from_value(value) { self.propagate_cancel = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for StockMove {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "StockMove"
    }
}

impl backbone_core::PersistentEntity for StockMove {
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

impl backbone_orm::EntityRepoMeta for StockMove {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("item_id".to_string(), "uuid".to_string());
        m.insert("picking_id".to_string(), "uuid".to_string());
        m.insert("location_id".to_string(), "uuid".to_string());
        m.insert("location_dest_id".to_string(), "uuid".to_string());
        m.insert("partner_id".to_string(), "uuid".to_string());
        m.insert("rule_id".to_string(), "uuid".to_string());
        m.insert("warehouse_id".to_string(), "uuid".to_string());
        m.insert("orderpoint_id".to_string(), "uuid".to_string());
        m.insert("state".to_string(), "move_state".to_string());
        m.insert("posting_state".to_string(), "gl_posting_state".to_string());
        m.insert("priority".to_string(), "priority".to_string());
        m.insert("procure_method".to_string(), "procure_method".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["name"]
    }
    fn relations() -> &'static [(&'static str, &'static str, &'static str)] {
        &[("picking", "transfers", "pickingId")]
    }
}

/// Builder for StockMove entity
///
/// Provides a fluent API for constructing StockMove instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct StockMoveBuilder {
    name: Option<String>,
    state: Option<MoveState>,
    posting_state: Option<GlPostingState>,
    priority: Option<Priority>,
    create_date: Option<DateTime<Utc>>,
    date: Option<DateTime<Utc>>,
    item_id: Option<Uuid>,
    demand_qty: Option<Decimal>,
    quantity: Option<Decimal>,
    price_unit: Option<Decimal>,
    forced_value: Option<Decimal>,
    procure_method: Option<ProcureMethod>,
    picking_id: Option<Uuid>,
    origin: Option<String>,
    location_id: Option<Uuid>,
    location_dest_id: Option<Uuid>,
    partner_id: Option<Uuid>,
    rule_id: Option<Uuid>,
    warehouse_id: Option<Uuid>,
    orderpoint_id: Option<Uuid>,
    move_orig_ids: Option<Vec<Uuid>>,
    move_dest_ids: Option<Vec<Uuid>>,
    is_inventory: Option<bool>,
    scrapped: Option<bool>,
    propagate_cancel: Option<bool>,
}

impl StockMoveBuilder {
    /// Set the name field (required)
    pub fn name(mut self, value: String) -> Self {
        self.name = Some(value);
        self
    }

    /// Set the state field (default: `MoveState::default()`)
    pub fn state(mut self, value: MoveState) -> Self {
        self.state = Some(value);
        self
    }

    /// Set the posting_state field (default: `GlPostingState::default()`)
    pub fn posting_state(mut self, value: GlPostingState) -> Self {
        self.posting_state = Some(value);
        self
    }

    /// Set the priority field (default: `Priority::default()`)
    pub fn priority(mut self, value: Priority) -> Self {
        self.priority = Some(value);
        self
    }

    /// Set the create_date field (default: `Utc::now()`)
    pub fn create_date(mut self, value: DateTime<Utc>) -> Self {
        self.create_date = Some(value);
        self
    }

    /// Set the date field (default: `Utc::now()`)
    pub fn date(mut self, value: DateTime<Utc>) -> Self {
        self.date = Some(value);
        self
    }

    /// Set the item_id field (required)
    pub fn item_id(mut self, value: Uuid) -> Self {
        self.item_id = Some(value);
        self
    }

    /// Set the demand_qty field (default: `Decimal::from(0)`)
    pub fn demand_qty(mut self, value: Decimal) -> Self {
        self.demand_qty = Some(value);
        self
    }

    /// Set the quantity field (default: `Decimal::from(0)`)
    pub fn quantity(mut self, value: Decimal) -> Self {
        self.quantity = Some(value);
        self
    }

    /// Set the price_unit field (default: `Decimal::from(0)`)
    pub fn price_unit(mut self, value: Decimal) -> Self {
        self.price_unit = Some(value);
        self
    }

    /// Set the forced_value field (optional)
    pub fn forced_value(mut self, value: Decimal) -> Self {
        self.forced_value = Some(value);
        self
    }

    /// Set the procure_method field (default: `ProcureMethod::default()`)
    pub fn procure_method(mut self, value: ProcureMethod) -> Self {
        self.procure_method = Some(value);
        self
    }

    /// Set the picking_id field (optional)
    pub fn picking_id(mut self, value: Uuid) -> Self {
        self.picking_id = Some(value);
        self
    }

    /// Set the origin field (optional)
    pub fn origin(mut self, value: String) -> Self {
        self.origin = Some(value);
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

    /// Set the rule_id field (optional)
    pub fn rule_id(mut self, value: Uuid) -> Self {
        self.rule_id = Some(value);
        self
    }

    /// Set the warehouse_id field (optional)
    pub fn warehouse_id(mut self, value: Uuid) -> Self {
        self.warehouse_id = Some(value);
        self
    }

    /// Set the orderpoint_id field (optional)
    pub fn orderpoint_id(mut self, value: Uuid) -> Self {
        self.orderpoint_id = Some(value);
        self
    }

    /// Set the move_orig_ids field (required)
    pub fn move_orig_ids(mut self, value: Vec<Uuid>) -> Self {
        self.move_orig_ids = Some(value);
        self
    }

    /// Set the move_dest_ids field (required)
    pub fn move_dest_ids(mut self, value: Vec<Uuid>) -> Self {
        self.move_dest_ids = Some(value);
        self
    }

    /// Set the is_inventory field (default: `false`)
    pub fn is_inventory(mut self, value: bool) -> Self {
        self.is_inventory = Some(value);
        self
    }

    /// Set the scrapped field (default: `false`)
    pub fn scrapped(mut self, value: bool) -> Self {
        self.scrapped = Some(value);
        self
    }

    /// Set the propagate_cancel field (default: `true`)
    pub fn propagate_cancel(mut self, value: bool) -> Self {
        self.propagate_cancel = Some(value);
        self
    }

    /// Build the StockMove entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<StockMove, String> {
        let name = self.name.ok_or_else(|| "name is required".to_string())?;
        let item_id = self.item_id.ok_or_else(|| "item_id is required".to_string())?;
        let location_id = self.location_id.ok_or_else(|| "location_id is required".to_string())?;
        let location_dest_id = self.location_dest_id.ok_or_else(|| "location_dest_id is required".to_string())?;
        let move_orig_ids = self.move_orig_ids.ok_or_else(|| "move_orig_ids is required".to_string())?;
        let move_dest_ids = self.move_dest_ids.ok_or_else(|| "move_dest_ids is required".to_string())?;

        Ok(StockMove {
            id: Uuid::new_v4(),
            name,
            state: self.state.unwrap_or_default(),
            posting_state: self.posting_state.unwrap_or_default(),
            priority: self.priority.unwrap_or_default(),
            create_date: self.create_date.unwrap_or(Utc::now()),
            date: self.date.unwrap_or(Utc::now()),
            item_id,
            demand_qty: self.demand_qty.unwrap_or(Decimal::from(0)),
            quantity: self.quantity.unwrap_or(Decimal::from(0)),
            price_unit: self.price_unit.unwrap_or(Decimal::from(0)),
            forced_value: self.forced_value,
            procure_method: self.procure_method.unwrap_or_default(),
            picking_id: self.picking_id,
            origin: self.origin,
            location_id,
            location_dest_id,
            partner_id: self.partner_id,
            rule_id: self.rule_id,
            warehouse_id: self.warehouse_id,
            orderpoint_id: self.orderpoint_id,
            move_orig_ids,
            move_dest_ids,
            is_inventory: self.is_inventory.unwrap_or(false),
            scrapped: self.scrapped.unwrap_or(false),
            propagate_cancel: self.propagate_cancel.unwrap_or(true),
            metadata: AuditMetadata::default(),
        })
    }
}
