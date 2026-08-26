use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use super::RuleAction;
use super::RuleAuto;
use super::ProcureMethod;
use super::AuditMetadata;

/// Strongly-typed ID for RouteRule
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RouteRuleId(pub Uuid);

impl RouteRuleId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for RouteRuleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for RouteRuleId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for RouteRuleId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<RouteRuleId> for Uuid {
    fn from(id: RouteRuleId) -> Self { id.0 }
}

impl AsRef<Uuid> for RouteRuleId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for RouteRuleId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct RouteRule {
    pub id: Uuid,
    pub name: String,
    pub active: bool,
    pub sequence: i32,
    pub action: RuleAction,
    pub auto: RuleAuto,
    pub procure_method: ProcureMethod,
    pub delay: i32,
    pub location_src_id: Option<Uuid>,
    pub location_dest_id: Uuid,
    pub picking_type_id: Uuid,
    pub route_id: Uuid,
    pub warehouse_id: Option<Uuid>,
    pub company_id: Option<Uuid>,
    pub propagate_cancel: bool,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl RouteRule {
    /// Create a builder for RouteRule
    pub fn builder() -> RouteRuleBuilder {
        <RouteRuleBuilder as Default>::default()
    }

    /// Create a new RouteRule with required fields
    pub fn new(name: String, active: bool, sequence: i32, action: RuleAction, auto: RuleAuto, procure_method: ProcureMethod, delay: i32, location_dest_id: Uuid, picking_type_id: Uuid, route_id: Uuid, propagate_cancel: bool) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            active,
            sequence,
            action,
            auto,
            procure_method,
            delay,
            location_src_id: None,
            location_dest_id,
            picking_type_id,
            route_id,
            warehouse_id: None,
            company_id: None,
            propagate_cancel,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> RouteRuleId {
        RouteRuleId(self.id)
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

    /// Set the location_src_id field (chainable)
    pub fn with_location_src_id(mut self, value: Uuid) -> Self {
        self.location_src_id = Some(value);
        self
    }

    /// Set the warehouse_id field (chainable)
    pub fn with_warehouse_id(mut self, value: Uuid) -> Self {
        self.warehouse_id = Some(value);
        self
    }

    /// Set the company_id field (chainable)
    pub fn with_company_id(mut self, value: Uuid) -> Self {
        self.company_id = Some(value);
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
                "active" => {
                    if let Ok(v) = serde_json::from_value(value) { self.active = v; }
                }
                "sequence" => {
                    if let Ok(v) = serde_json::from_value(value) { self.sequence = v; }
                }
                "action" => {
                    if let Ok(v) = serde_json::from_value(value) { self.action = v; }
                }
                "auto" => {
                    if let Ok(v) = serde_json::from_value(value) { self.auto = v; }
                }
                "procure_method" => {
                    if let Ok(v) = serde_json::from_value(value) { self.procure_method = v; }
                }
                "delay" => {
                    if let Ok(v) = serde_json::from_value(value) { self.delay = v; }
                }
                "location_src_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.location_src_id = v; }
                }
                "location_dest_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.location_dest_id = v; }
                }
                "picking_type_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.picking_type_id = v; }
                }
                "route_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.route_id = v; }
                }
                "warehouse_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.warehouse_id = v; }
                }
                "company_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.company_id = v; }
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

impl super::Entity for RouteRule {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "RouteRule"
    }
}

impl backbone_core::PersistentEntity for RouteRule {
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

impl backbone_orm::EntityRepoMeta for RouteRule {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("location_src_id".to_string(), "uuid".to_string());
        m.insert("location_dest_id".to_string(), "uuid".to_string());
        m.insert("picking_type_id".to_string(), "uuid".to_string());
        m.insert("route_id".to_string(), "uuid".to_string());
        m.insert("warehouse_id".to_string(), "uuid".to_string());
        m.insert("company_id".to_string(), "uuid".to_string());
        m.insert("action".to_string(), "rule_action".to_string());
        m.insert("auto".to_string(), "rule_auto".to_string());
        m.insert("procure_method".to_string(), "procure_method".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["name"]
    }
    fn company_field() -> Option<&'static str> {
        Some("company_id")
    }
    fn relations() -> &'static [(&'static str, &'static str, &'static str)] {
        &[("route", "routes", "routeId")]
    }
}

/// Builder for RouteRule entity
///
/// Provides a fluent API for constructing RouteRule instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct RouteRuleBuilder {
    name: Option<String>,
    active: Option<bool>,
    sequence: Option<i32>,
    action: Option<RuleAction>,
    auto: Option<RuleAuto>,
    procure_method: Option<ProcureMethod>,
    delay: Option<i32>,
    location_src_id: Option<Uuid>,
    location_dest_id: Option<Uuid>,
    picking_type_id: Option<Uuid>,
    route_id: Option<Uuid>,
    warehouse_id: Option<Uuid>,
    company_id: Option<Uuid>,
    propagate_cancel: Option<bool>,
}

impl RouteRuleBuilder {
    /// Set the name field (required)
    pub fn name(mut self, value: String) -> Self {
        self.name = Some(value);
        self
    }

    /// Set the active field (default: `true`)
    pub fn active(mut self, value: bool) -> Self {
        self.active = Some(value);
        self
    }

    /// Set the sequence field (default: `20`)
    pub fn sequence(mut self, value: i32) -> Self {
        self.sequence = Some(value);
        self
    }

    /// Set the action field (default: `RuleAction::default()`)
    pub fn action(mut self, value: RuleAction) -> Self {
        self.action = Some(value);
        self
    }

    /// Set the auto field (default: `RuleAuto::default()`)
    pub fn auto(mut self, value: RuleAuto) -> Self {
        self.auto = Some(value);
        self
    }

    /// Set the procure_method field (default: `ProcureMethod::default()`)
    pub fn procure_method(mut self, value: ProcureMethod) -> Self {
        self.procure_method = Some(value);
        self
    }

    /// Set the delay field (default: `0`)
    pub fn delay(mut self, value: i32) -> Self {
        self.delay = Some(value);
        self
    }

    /// Set the location_src_id field (optional)
    pub fn location_src_id(mut self, value: Uuid) -> Self {
        self.location_src_id = Some(value);
        self
    }

    /// Set the location_dest_id field (required)
    pub fn location_dest_id(mut self, value: Uuid) -> Self {
        self.location_dest_id = Some(value);
        self
    }

    /// Set the picking_type_id field (required)
    pub fn picking_type_id(mut self, value: Uuid) -> Self {
        self.picking_type_id = Some(value);
        self
    }

    /// Set the route_id field (required)
    pub fn route_id(mut self, value: Uuid) -> Self {
        self.route_id = Some(value);
        self
    }

    /// Set the warehouse_id field (optional)
    pub fn warehouse_id(mut self, value: Uuid) -> Self {
        self.warehouse_id = Some(value);
        self
    }

    /// Set the company_id field (optional)
    pub fn company_id(mut self, value: Uuid) -> Self {
        self.company_id = Some(value);
        self
    }

    /// Set the propagate_cancel field (default: `false`)
    pub fn propagate_cancel(mut self, value: bool) -> Self {
        self.propagate_cancel = Some(value);
        self
    }

    /// Build the RouteRule entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<RouteRule, String> {
        let name = self.name.ok_or_else(|| "name is required".to_string())?;
        let location_dest_id = self.location_dest_id.ok_or_else(|| "location_dest_id is required".to_string())?;
        let picking_type_id = self.picking_type_id.ok_or_else(|| "picking_type_id is required".to_string())?;
        let route_id = self.route_id.ok_or_else(|| "route_id is required".to_string())?;

        Ok(RouteRule {
            id: Uuid::new_v4(),
            name,
            active: self.active.unwrap_or(true),
            sequence: self.sequence.unwrap_or(20),
            action: self.action.unwrap_or_default(),
            auto: self.auto.unwrap_or_default(),
            procure_method: self.procure_method.unwrap_or_default(),
            delay: self.delay.unwrap_or(0),
            location_src_id: self.location_src_id,
            location_dest_id,
            picking_type_id,
            route_id,
            warehouse_id: self.warehouse_id,
            company_id: self.company_id,
            propagate_cancel: self.propagate_cancel.unwrap_or(false),
            metadata: AuditMetadata::default(),
        })
    }
}
