use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use rust_decimal::Decimal;
use super::AuditMetadata;

/// Strongly-typed ID for LandedCostAdjustmentLine
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LandedCostAdjustmentLineId(pub Uuid);

impl LandedCostAdjustmentLineId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for LandedCostAdjustmentLineId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for LandedCostAdjustmentLineId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for LandedCostAdjustmentLineId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<LandedCostAdjustmentLineId> for Uuid {
    fn from(id: LandedCostAdjustmentLineId) -> Self { id.0 }
}

impl AsRef<Uuid> for LandedCostAdjustmentLineId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for LandedCostAdjustmentLineId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LandedCostAdjustmentLine {
    pub id: Uuid,
    pub lc_id: Uuid,
    pub company_id: Uuid,
    pub move_line_id: Uuid,
    pub cost_line_id: Uuid,
    pub share: Decimal,
    pub additional_landed_cost: Decimal,
    pub remaining_qty: Decimal,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl LandedCostAdjustmentLine {
    /// Create a builder for LandedCostAdjustmentLine
    pub fn builder() -> LandedCostAdjustmentLineBuilder {
        <LandedCostAdjustmentLineBuilder as Default>::default()
    }

    /// Create a new LandedCostAdjustmentLine with required fields
    pub fn new(lc_id: Uuid, company_id: Uuid, move_line_id: Uuid, cost_line_id: Uuid, share: Decimal, additional_landed_cost: Decimal, remaining_qty: Decimal) -> Self {
        Self {
            id: Uuid::new_v4(),
            lc_id,
            company_id,
            move_line_id,
            cost_line_id,
            share,
            additional_landed_cost,
            remaining_qty,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> LandedCostAdjustmentLineId {
        LandedCostAdjustmentLineId(self.id)
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
    // Partial Update
    // ==========================================================

    /// Apply partial updates from a map of field name to JSON value
    pub fn apply_patch(&mut self, fields: std::collections::HashMap<String, serde_json::Value>) {
        for (key, value) in fields {
            match key.as_str() {
                "lc_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.lc_id = v; }
                }
                "company_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.company_id = v; }
                }
                "move_line_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.move_line_id = v; }
                }
                "cost_line_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.cost_line_id = v; }
                }
                "share" => {
                    if let Ok(v) = serde_json::from_value(value) { self.share = v; }
                }
                "additional_landed_cost" => {
                    if let Ok(v) = serde_json::from_value(value) { self.additional_landed_cost = v; }
                }
                "remaining_qty" => {
                    if let Ok(v) = serde_json::from_value(value) { self.remaining_qty = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for LandedCostAdjustmentLine {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "LandedCostAdjustmentLine"
    }
}

impl backbone_core::PersistentEntity for LandedCostAdjustmentLine {
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

impl backbone_orm::EntityRepoMeta for LandedCostAdjustmentLine {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("lc_id".to_string(), "uuid".to_string());
        m.insert("company_id".to_string(), "uuid".to_string());
        m.insert("move_line_id".to_string(), "uuid".to_string());
        m.insert("cost_line_id".to_string(), "uuid".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &[]
    }
    fn company_field() -> Option<&'static str> {
        Some("company_id")
    }
}

/// Builder for LandedCostAdjustmentLine entity
///
/// Provides a fluent API for constructing LandedCostAdjustmentLine instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct LandedCostAdjustmentLineBuilder {
    lc_id: Option<Uuid>,
    company_id: Option<Uuid>,
    move_line_id: Option<Uuid>,
    cost_line_id: Option<Uuid>,
    share: Option<Decimal>,
    additional_landed_cost: Option<Decimal>,
    remaining_qty: Option<Decimal>,
}

impl LandedCostAdjustmentLineBuilder {
    /// Set the lc_id field (required)
    pub fn lc_id(mut self, value: Uuid) -> Self {
        self.lc_id = Some(value);
        self
    }

    /// Set the company_id field (required)
    pub fn company_id(mut self, value: Uuid) -> Self {
        self.company_id = Some(value);
        self
    }

    /// Set the move_line_id field (required)
    pub fn move_line_id(mut self, value: Uuid) -> Self {
        self.move_line_id = Some(value);
        self
    }

    /// Set the cost_line_id field (required)
    pub fn cost_line_id(mut self, value: Uuid) -> Self {
        self.cost_line_id = Some(value);
        self
    }

    /// Set the share field (required)
    pub fn share(mut self, value: Decimal) -> Self {
        self.share = Some(value);
        self
    }

    /// Set the additional_landed_cost field (default: `Decimal::from(0)`)
    pub fn additional_landed_cost(mut self, value: Decimal) -> Self {
        self.additional_landed_cost = Some(value);
        self
    }

    /// Set the remaining_qty field (default: `Decimal::from(0)`)
    pub fn remaining_qty(mut self, value: Decimal) -> Self {
        self.remaining_qty = Some(value);
        self
    }

    /// Build the LandedCostAdjustmentLine entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<LandedCostAdjustmentLine, String> {
        let lc_id = self.lc_id.ok_or_else(|| "lc_id is required".to_string())?;
        let company_id = self.company_id.ok_or_else(|| "company_id is required".to_string())?;
        let move_line_id = self.move_line_id.ok_or_else(|| "move_line_id is required".to_string())?;
        let cost_line_id = self.cost_line_id.ok_or_else(|| "cost_line_id is required".to_string())?;
        let share = self.share.ok_or_else(|| "share is required".to_string())?;

        Ok(LandedCostAdjustmentLine {
            id: Uuid::new_v4(),
            lc_id,
            company_id,
            move_line_id,
            cost_line_id,
            share,
            additional_landed_cost: self.additional_landed_cost.unwrap_or(Decimal::from(0)),
            remaining_qty: self.remaining_qty.unwrap_or(Decimal::from(0)),
            metadata: AuditMetadata::default(),
        })
    }
}
