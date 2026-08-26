use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use rust_decimal::Decimal;

use super::LandedCostSplitMethod;
use super::AuditMetadata;

/// Strongly-typed ID for LandedCostLine
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LandedCostLineId(pub Uuid);

impl LandedCostLineId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for LandedCostLineId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for LandedCostLineId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for LandedCostLineId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<LandedCostLineId> for Uuid {
    fn from(id: LandedCostLineId) -> Self { id.0 }
}

impl AsRef<Uuid> for LandedCostLineId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for LandedCostLineId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LandedCostLine {
    pub id: Uuid,
    pub lc_id: Uuid,
    pub company_id: Uuid,
    pub name: String,
    pub account_id: Uuid,
    pub split_method: LandedCostSplitMethod,
    pub amount: Decimal,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl LandedCostLine {
    /// Create a builder for LandedCostLine
    pub fn builder() -> LandedCostLineBuilder {
        <LandedCostLineBuilder as Default>::default()
    }

    /// Create a new LandedCostLine with required fields
    pub fn new(lc_id: Uuid, company_id: Uuid, name: String, account_id: Uuid, split_method: LandedCostSplitMethod, amount: Decimal) -> Self {
        Self {
            id: Uuid::new_v4(),
            lc_id,
            company_id,
            name,
            account_id,
            split_method,
            amount,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> LandedCostLineId {
        LandedCostLineId(self.id)
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
                "name" => {
                    if let Ok(v) = serde_json::from_value(value) { self.name = v; }
                }
                "account_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.account_id = v; }
                }
                "split_method" => {
                    if let Ok(v) = serde_json::from_value(value) { self.split_method = v; }
                }
                "amount" => {
                    if let Ok(v) = serde_json::from_value(value) { self.amount = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for LandedCostLine {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "LandedCostLine"
    }
}

impl backbone_core::PersistentEntity for LandedCostLine {
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

impl backbone_orm::EntityRepoMeta for LandedCostLine {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("lc_id".to_string(), "uuid".to_string());
        m.insert("company_id".to_string(), "uuid".to_string());
        m.insert("account_id".to_string(), "uuid".to_string());
        m.insert("split_method".to_string(), "landed_cost_split_method".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["name"]
    }
    fn company_field() -> Option<&'static str> {
        Some("company_id")
    }
    fn relations() -> &'static [(&'static str, &'static str, &'static str)] {
        &[("landedCost", "landed_costs", "lcId")]
    }
}

/// Builder for LandedCostLine entity
///
/// Provides a fluent API for constructing LandedCostLine instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct LandedCostLineBuilder {
    lc_id: Option<Uuid>,
    company_id: Option<Uuid>,
    name: Option<String>,
    account_id: Option<Uuid>,
    split_method: Option<LandedCostSplitMethod>,
    amount: Option<Decimal>,
}

impl LandedCostLineBuilder {
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

    /// Set the name field (required)
    pub fn name(mut self, value: String) -> Self {
        self.name = Some(value);
        self
    }

    /// Set the account_id field (required)
    pub fn account_id(mut self, value: Uuid) -> Self {
        self.account_id = Some(value);
        self
    }

    /// Set the split_method field (default: `LandedCostSplitMethod::default()`)
    pub fn split_method(mut self, value: LandedCostSplitMethod) -> Self {
        self.split_method = Some(value);
        self
    }

    /// Set the amount field (required)
    pub fn amount(mut self, value: Decimal) -> Self {
        self.amount = Some(value);
        self
    }

    /// Build the LandedCostLine entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<LandedCostLine, String> {
        let lc_id = self.lc_id.ok_or_else(|| "lc_id is required".to_string())?;
        let company_id = self.company_id.ok_or_else(|| "company_id is required".to_string())?;
        let name = self.name.ok_or_else(|| "name is required".to_string())?;
        let account_id = self.account_id.ok_or_else(|| "account_id is required".to_string())?;
        let amount = self.amount.ok_or_else(|| "amount is required".to_string())?;

        Ok(LandedCostLine {
            id: Uuid::new_v4(),
            lc_id,
            company_id,
            name,
            account_id,
            split_method: self.split_method.unwrap_or_default(),
            amount,
            metadata: AuditMetadata::default(),
        })
    }
}
