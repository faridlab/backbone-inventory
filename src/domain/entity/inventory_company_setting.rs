use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use super::InventoryCostMethod;
use super::ValuationPolicy;
use super::AuditMetadata;

/// Strongly-typed ID for InventoryCompanySetting
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct InventoryCompanySettingId(pub Uuid);

impl InventoryCompanySettingId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for InventoryCompanySettingId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for InventoryCompanySettingId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for InventoryCompanySettingId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<InventoryCompanySettingId> for Uuid {
    fn from(id: InventoryCompanySettingId) -> Self { id.0 }
}

impl AsRef<Uuid> for InventoryCompanySettingId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for InventoryCompanySettingId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct InventoryCompanySetting {
    pub id: Uuid,
    pub company_id: Uuid,
    pub cost_method: InventoryCostMethod,
    pub valuation_policy: ValuationPolicy,
    pub anglo_saxon_accounting: bool,
    pub stock_interim_delivered_account_id: Option<Uuid>,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl InventoryCompanySetting {
    /// Create a builder for InventoryCompanySetting
    pub fn builder() -> InventoryCompanySettingBuilder {
        <InventoryCompanySettingBuilder as Default>::default()
    }

    /// Create a new InventoryCompanySetting with required fields
    pub fn new(company_id: Uuid, cost_method: InventoryCostMethod, valuation_policy: ValuationPolicy, anglo_saxon_accounting: bool) -> Self {
        Self {
            id: Uuid::new_v4(),
            company_id,
            cost_method,
            valuation_policy,
            anglo_saxon_accounting,
            stock_interim_delivered_account_id: None,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> InventoryCompanySettingId {
        InventoryCompanySettingId(self.id)
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

    /// Set the stock_interim_delivered_account_id field (chainable)
    pub fn with_stock_interim_delivered_account_id(mut self, value: Uuid) -> Self {
        self.stock_interim_delivered_account_id = Some(value);
        self
    }

    // ==========================================================
    // Partial Update
    // ==========================================================

    /// Apply partial updates from a map of field name to JSON value
    pub fn apply_patch(&mut self, fields: std::collections::HashMap<String, serde_json::Value>) {
        for (key, value) in fields {
            match key.as_str() {
                "company_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.company_id = v; }
                }
                "cost_method" => {
                    if let Ok(v) = serde_json::from_value(value) { self.cost_method = v; }
                }
                "valuation_policy" => {
                    if let Ok(v) = serde_json::from_value(value) { self.valuation_policy = v; }
                }
                "anglo_saxon_accounting" => {
                    if let Ok(v) = serde_json::from_value(value) { self.anglo_saxon_accounting = v; }
                }
                "stock_interim_delivered_account_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.stock_interim_delivered_account_id = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for InventoryCompanySetting {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "InventoryCompanySetting"
    }
}

impl backbone_core::PersistentEntity for InventoryCompanySetting {
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

impl backbone_orm::EntityRepoMeta for InventoryCompanySetting {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("company_id".to_string(), "uuid".to_string());
        m.insert("stock_interim_delivered_account_id".to_string(), "uuid".to_string());
        m.insert("cost_method".to_string(), "inventory_cost_method".to_string());
        m.insert("valuation_policy".to_string(), "valuation_policy".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &[]
    }
    fn company_field() -> Option<&'static str> {
        Some("company_id")
    }
}

/// Builder for InventoryCompanySetting entity
///
/// Provides a fluent API for constructing InventoryCompanySetting instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct InventoryCompanySettingBuilder {
    company_id: Option<Uuid>,
    cost_method: Option<InventoryCostMethod>,
    valuation_policy: Option<ValuationPolicy>,
    anglo_saxon_accounting: Option<bool>,
    stock_interim_delivered_account_id: Option<Uuid>,
}

impl InventoryCompanySettingBuilder {
    /// Set the company_id field (required)
    pub fn company_id(mut self, value: Uuid) -> Self {
        self.company_id = Some(value);
        self
    }

    /// Set the cost_method field (default: `InventoryCostMethod::default()`)
    pub fn cost_method(mut self, value: InventoryCostMethod) -> Self {
        self.cost_method = Some(value);
        self
    }

    /// Set the valuation_policy field (default: `ValuationPolicy::default()`)
    pub fn valuation_policy(mut self, value: ValuationPolicy) -> Self {
        self.valuation_policy = Some(value);
        self
    }

    /// Set the anglo_saxon_accounting field (default: `false`)
    pub fn anglo_saxon_accounting(mut self, value: bool) -> Self {
        self.anglo_saxon_accounting = Some(value);
        self
    }

    /// Set the stock_interim_delivered_account_id field (optional)
    pub fn stock_interim_delivered_account_id(mut self, value: Uuid) -> Self {
        self.stock_interim_delivered_account_id = Some(value);
        self
    }

    /// Build the InventoryCompanySetting entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<InventoryCompanySetting, String> {
        let company_id = self.company_id.ok_or_else(|| "company_id is required".to_string())?;

        Ok(InventoryCompanySetting {
            id: Uuid::new_v4(),
            company_id,
            cost_method: self.cost_method.unwrap_or_default(),
            valuation_policy: self.valuation_policy.unwrap_or_default(),
            anglo_saxon_accounting: self.anglo_saxon_accounting.unwrap_or(false),
            stock_interim_delivered_account_id: self.stock_interim_delivered_account_id,
            metadata: AuditMetadata::default(),
        })
    }
}
