use chrono::{DateTime, Utc, NaiveDate};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use rust_decimal::Decimal;

use super::OrderpointTrigger;
use super::AuditMetadata;

/// Strongly-typed ID for ReorderingRule
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ReorderingRuleId(pub Uuid);

impl ReorderingRuleId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for ReorderingRuleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for ReorderingRuleId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for ReorderingRuleId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<ReorderingRuleId> for Uuid {
    fn from(id: ReorderingRuleId) -> Self { id.0 }
}

impl AsRef<Uuid> for ReorderingRuleId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for ReorderingRuleId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ReorderingRule {
    pub id: Uuid,
    pub name: String,
    pub trigger: OrderpointTrigger,
    pub active: bool,
    pub snoozed_until: Option<NaiveDate>,
    pub item_id: Uuid,
    pub location_id: Uuid,
    pub warehouse_id: Uuid,
    pub item_min_qty: Decimal,
    pub item_max_qty: Decimal,
    pub route_id: Option<Uuid>,
    pub qty_on_hand: Decimal,
    pub qty_forecast: Decimal,
    pub qty_to_order: Decimal,
    pub qty_to_order_manual: Decimal,
    pub lead_days: Decimal,
    pub deadline_date: Option<NaiveDate>,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl ReorderingRule {
    /// Create a builder for ReorderingRule
    pub fn builder() -> ReorderingRuleBuilder {
        <ReorderingRuleBuilder as Default>::default()
    }

    /// Create a new ReorderingRule with required fields
    pub fn new(name: String, trigger: OrderpointTrigger, active: bool, item_id: Uuid, location_id: Uuid, warehouse_id: Uuid, item_min_qty: Decimal, item_max_qty: Decimal, qty_on_hand: Decimal, qty_forecast: Decimal, qty_to_order: Decimal, qty_to_order_manual: Decimal, lead_days: Decimal) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            trigger,
            active,
            snoozed_until: None,
            item_id,
            location_id,
            warehouse_id,
            item_min_qty,
            item_max_qty,
            route_id: None,
            qty_on_hand,
            qty_forecast,
            qty_to_order,
            qty_to_order_manual,
            lead_days,
            deadline_date: None,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> ReorderingRuleId {
        ReorderingRuleId(self.id)
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

    /// Set the snoozed_until field (chainable)
    pub fn with_snoozed_until(mut self, value: NaiveDate) -> Self {
        self.snoozed_until = Some(value);
        self
    }

    /// Set the route_id field (chainable)
    pub fn with_route_id(mut self, value: Uuid) -> Self {
        self.route_id = Some(value);
        self
    }

    /// Set the deadline_date field (chainable)
    pub fn with_deadline_date(mut self, value: NaiveDate) -> Self {
        self.deadline_date = Some(value);
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
                "trigger" => {
                    if let Ok(v) = serde_json::from_value(value) { self.trigger = v; }
                }
                "active" => {
                    if let Ok(v) = serde_json::from_value(value) { self.active = v; }
                }
                "snoozed_until" => {
                    if let Ok(v) = serde_json::from_value(value) { self.snoozed_until = v; }
                }
                "item_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.item_id = v; }
                }
                "location_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.location_id = v; }
                }
                "warehouse_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.warehouse_id = v; }
                }
                "item_min_qty" => {
                    if let Ok(v) = serde_json::from_value(value) { self.item_min_qty = v; }
                }
                "item_max_qty" => {
                    if let Ok(v) = serde_json::from_value(value) { self.item_max_qty = v; }
                }
                "route_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.route_id = v; }
                }
                "qty_on_hand" => {
                    if let Ok(v) = serde_json::from_value(value) { self.qty_on_hand = v; }
                }
                "qty_forecast" => {
                    if let Ok(v) = serde_json::from_value(value) { self.qty_forecast = v; }
                }
                "qty_to_order" => {
                    if let Ok(v) = serde_json::from_value(value) { self.qty_to_order = v; }
                }
                "qty_to_order_manual" => {
                    if let Ok(v) = serde_json::from_value(value) { self.qty_to_order_manual = v; }
                }
                "lead_days" => {
                    if let Ok(v) = serde_json::from_value(value) { self.lead_days = v; }
                }
                "deadline_date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.deadline_date = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for ReorderingRule {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "ReorderingRule"
    }
}

impl backbone_core::PersistentEntity for ReorderingRule {
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

impl backbone_orm::EntityRepoMeta for ReorderingRule {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("item_id".to_string(), "uuid".to_string());
        m.insert("location_id".to_string(), "uuid".to_string());
        m.insert("warehouse_id".to_string(), "uuid".to_string());
        m.insert("route_id".to_string(), "uuid".to_string());
        m.insert("trigger".to_string(), "orderpoint_trigger".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["name"]
    }
}

/// Builder for ReorderingRule entity
///
/// Provides a fluent API for constructing ReorderingRule instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct ReorderingRuleBuilder {
    name: Option<String>,
    trigger: Option<OrderpointTrigger>,
    active: Option<bool>,
    snoozed_until: Option<NaiveDate>,
    item_id: Option<Uuid>,
    location_id: Option<Uuid>,
    warehouse_id: Option<Uuid>,
    item_min_qty: Option<Decimal>,
    item_max_qty: Option<Decimal>,
    route_id: Option<Uuid>,
    qty_on_hand: Option<Decimal>,
    qty_forecast: Option<Decimal>,
    qty_to_order: Option<Decimal>,
    qty_to_order_manual: Option<Decimal>,
    lead_days: Option<Decimal>,
    deadline_date: Option<NaiveDate>,
}

impl ReorderingRuleBuilder {
    /// Set the name field (required)
    pub fn name(mut self, value: String) -> Self {
        self.name = Some(value);
        self
    }

    /// Set the trigger field (default: `OrderpointTrigger::default()`)
    pub fn trigger(mut self, value: OrderpointTrigger) -> Self {
        self.trigger = Some(value);
        self
    }

    /// Set the active field (default: `true`)
    pub fn active(mut self, value: bool) -> Self {
        self.active = Some(value);
        self
    }

    /// Set the snoozed_until field (optional)
    pub fn snoozed_until(mut self, value: NaiveDate) -> Self {
        self.snoozed_until = Some(value);
        self
    }

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

    /// Set the warehouse_id field (required)
    pub fn warehouse_id(mut self, value: Uuid) -> Self {
        self.warehouse_id = Some(value);
        self
    }

    /// Set the item_min_qty field (default: `Decimal::from(0)`)
    pub fn item_min_qty(mut self, value: Decimal) -> Self {
        self.item_min_qty = Some(value);
        self
    }

    /// Set the item_max_qty field (default: `Decimal::from(0)`)
    pub fn item_max_qty(mut self, value: Decimal) -> Self {
        self.item_max_qty = Some(value);
        self
    }

    /// Set the route_id field (optional)
    pub fn route_id(mut self, value: Uuid) -> Self {
        self.route_id = Some(value);
        self
    }

    /// Set the qty_on_hand field (default: `Decimal::from(0)`)
    pub fn qty_on_hand(mut self, value: Decimal) -> Self {
        self.qty_on_hand = Some(value);
        self
    }

    /// Set the qty_forecast field (default: `Decimal::from(0)`)
    pub fn qty_forecast(mut self, value: Decimal) -> Self {
        self.qty_forecast = Some(value);
        self
    }

    /// Set the qty_to_order field (default: `Decimal::from(0)`)
    pub fn qty_to_order(mut self, value: Decimal) -> Self {
        self.qty_to_order = Some(value);
        self
    }

    /// Set the qty_to_order_manual field (default: `Decimal::from(0)`)
    pub fn qty_to_order_manual(mut self, value: Decimal) -> Self {
        self.qty_to_order_manual = Some(value);
        self
    }

    /// Set the lead_days field (default: `Decimal::from(0)`)
    pub fn lead_days(mut self, value: Decimal) -> Self {
        self.lead_days = Some(value);
        self
    }

    /// Set the deadline_date field (optional)
    pub fn deadline_date(mut self, value: NaiveDate) -> Self {
        self.deadline_date = Some(value);
        self
    }

    /// Build the ReorderingRule entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<ReorderingRule, String> {
        let name = self.name.ok_or_else(|| "name is required".to_string())?;
        let item_id = self.item_id.ok_or_else(|| "item_id is required".to_string())?;
        let location_id = self.location_id.ok_or_else(|| "location_id is required".to_string())?;
        let warehouse_id = self.warehouse_id.ok_or_else(|| "warehouse_id is required".to_string())?;

        Ok(ReorderingRule {
            id: Uuid::new_v4(),
            name,
            trigger: self.trigger.unwrap_or_default(),
            active: self.active.unwrap_or(true),
            snoozed_until: self.snoozed_until,
            item_id,
            location_id,
            warehouse_id,
            item_min_qty: self.item_min_qty.unwrap_or(Decimal::from(0)),
            item_max_qty: self.item_max_qty.unwrap_or(Decimal::from(0)),
            route_id: self.route_id,
            qty_on_hand: self.qty_on_hand.unwrap_or(Decimal::from(0)),
            qty_forecast: self.qty_forecast.unwrap_or(Decimal::from(0)),
            qty_to_order: self.qty_to_order.unwrap_or(Decimal::from(0)),
            qty_to_order_manual: self.qty_to_order_manual.unwrap_or(Decimal::from(0)),
            lead_days: self.lead_days.unwrap_or(Decimal::from(0)),
            deadline_date: self.deadline_date,
            metadata: AuditMetadata::default(),
        })
    }
}
