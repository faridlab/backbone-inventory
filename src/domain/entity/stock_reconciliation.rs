use chrono::{DateTime, Utc, NaiveDate};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use rust_decimal::Decimal;

use super::DocStatus;
use super::GlPostingState;
use super::AuditMetadata;

/// Strongly-typed ID for StockReconciliation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StockReconciliationId(pub Uuid);

impl StockReconciliationId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for StockReconciliationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for StockReconciliationId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for StockReconciliationId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<StockReconciliationId> for Uuid {
    fn from(id: StockReconciliationId) -> Self { id.0 }
}

impl AsRef<Uuid> for StockReconciliationId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for StockReconciliationId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct StockReconciliation {
    pub id: Uuid,
    pub recon_number: String,
    pub company_id: Uuid,
    pub warehouse_id: Uuid,
    pub posting_date: NaiveDate,
    pub net_difference: Decimal,
    pub inventory_account_id: Uuid,
    pub adjustment_account_id: Uuid,
    pub status: DocStatus,
    pub posting_state: GlPostingState,
    pub journal_id: Option<Uuid>,
    pub accounting_post_id: Option<Uuid>,
    pub posted_at: Option<DateTime<Utc>>,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl StockReconciliation {
    /// Create a builder for StockReconciliation
    pub fn builder() -> StockReconciliationBuilder {
        StockReconciliationBuilder::default()
    }

    /// Create a new StockReconciliation with required fields
    pub fn new(recon_number: String, company_id: Uuid, warehouse_id: Uuid, posting_date: NaiveDate, net_difference: Decimal, inventory_account_id: Uuid, adjustment_account_id: Uuid, status: DocStatus, posting_state: GlPostingState) -> Self {
        Self {
            id: Uuid::new_v4(),
            recon_number,
            company_id,
            warehouse_id,
            posting_date,
            net_difference,
            inventory_account_id,
            adjustment_account_id,
            status,
            posting_state,
            journal_id: None,
            accounting_post_id: None,
            posted_at: None,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> StockReconciliationId {
        StockReconciliationId(self.id)
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
    pub fn status(&self) -> &DocStatus {
        &self.status
    }


    // ==========================================================
    // Fluent Setters (with_* for optional fields)
    // ==========================================================

    /// Set the journal_id field (chainable)
    pub fn with_journal_id(mut self, value: Uuid) -> Self {
        self.journal_id = Some(value);
        self
    }

    /// Set the accounting_post_id field (chainable)
    pub fn with_accounting_post_id(mut self, value: Uuid) -> Self {
        self.accounting_post_id = Some(value);
        self
    }

    /// Set the posted_at field (chainable)
    pub fn with_posted_at(mut self, value: DateTime<Utc>) -> Self {
        self.posted_at = Some(value);
        self
    }

    // ==========================================================
    // Partial Update
    // ==========================================================

    /// Apply partial updates from a map of field name to JSON value
    pub fn apply_patch(&mut self, fields: std::collections::HashMap<String, serde_json::Value>) {
        for (key, value) in fields {
            match key.as_str() {
                "recon_number" => {
                    if let Ok(v) = serde_json::from_value(value) { self.recon_number = v; }
                }
                "company_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.company_id = v; }
                }
                "warehouse_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.warehouse_id = v; }
                }
                "posting_date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.posting_date = v; }
                }
                "net_difference" => {
                    if let Ok(v) = serde_json::from_value(value) { self.net_difference = v; }
                }
                "inventory_account_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.inventory_account_id = v; }
                }
                "adjustment_account_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.adjustment_account_id = v; }
                }
                "status" => {
                    if let Ok(v) = serde_json::from_value(value) { self.status = v; }
                }
                "posting_state" => {
                    if let Ok(v) = serde_json::from_value(value) { self.posting_state = v; }
                }
                "journal_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.journal_id = v; }
                }
                "accounting_post_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.accounting_post_id = v; }
                }
                "posted_at" => {
                    if let Ok(v) = serde_json::from_value(value) { self.posted_at = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for StockReconciliation {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "StockReconciliation"
    }
}

impl backbone_core::PersistentEntity for StockReconciliation {
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

impl backbone_orm::EntityRepoMeta for StockReconciliation {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("company_id".to_string(), "uuid".to_string());
        m.insert("warehouse_id".to_string(), "uuid".to_string());
        m.insert("inventory_account_id".to_string(), "uuid".to_string());
        m.insert("adjustment_account_id".to_string(), "uuid".to_string());
        m.insert("journal_id".to_string(), "uuid".to_string());
        m.insert("accounting_post_id".to_string(), "uuid".to_string());
        m.insert("status".to_string(), "doc_status".to_string());
        m.insert("posting_state".to_string(), "gl_posting_state".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["recon_number"]
    }
}

/// Builder for StockReconciliation entity
///
/// Provides a fluent API for constructing StockReconciliation instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct StockReconciliationBuilder {
    recon_number: Option<String>,
    company_id: Option<Uuid>,
    warehouse_id: Option<Uuid>,
    posting_date: Option<NaiveDate>,
    net_difference: Option<Decimal>,
    inventory_account_id: Option<Uuid>,
    adjustment_account_id: Option<Uuid>,
    status: Option<DocStatus>,
    posting_state: Option<GlPostingState>,
    journal_id: Option<Uuid>,
    accounting_post_id: Option<Uuid>,
    posted_at: Option<DateTime<Utc>>,
}

impl StockReconciliationBuilder {
    /// Set the recon_number field (required)
    pub fn recon_number(mut self, value: String) -> Self {
        self.recon_number = Some(value);
        self
    }

    /// Set the company_id field (required)
    pub fn company_id(mut self, value: Uuid) -> Self {
        self.company_id = Some(value);
        self
    }

    /// Set the warehouse_id field (required)
    pub fn warehouse_id(mut self, value: Uuid) -> Self {
        self.warehouse_id = Some(value);
        self
    }

    /// Set the posting_date field (required)
    pub fn posting_date(mut self, value: NaiveDate) -> Self {
        self.posting_date = Some(value);
        self
    }

    /// Set the net_difference field (default: `Decimal::from(0)`)
    pub fn net_difference(mut self, value: Decimal) -> Self {
        self.net_difference = Some(value);
        self
    }

    /// Set the inventory_account_id field (required)
    pub fn inventory_account_id(mut self, value: Uuid) -> Self {
        self.inventory_account_id = Some(value);
        self
    }

    /// Set the adjustment_account_id field (required)
    pub fn adjustment_account_id(mut self, value: Uuid) -> Self {
        self.adjustment_account_id = Some(value);
        self
    }

    /// Set the status field (default: `DocStatus::default()`)
    pub fn status(mut self, value: DocStatus) -> Self {
        self.status = Some(value);
        self
    }

    /// Set the posting_state field (default: `GlPostingState::default()`)
    pub fn posting_state(mut self, value: GlPostingState) -> Self {
        self.posting_state = Some(value);
        self
    }

    /// Set the journal_id field (optional)
    pub fn journal_id(mut self, value: Uuid) -> Self {
        self.journal_id = Some(value);
        self
    }

    /// Set the accounting_post_id field (optional)
    pub fn accounting_post_id(mut self, value: Uuid) -> Self {
        self.accounting_post_id = Some(value);
        self
    }

    /// Set the posted_at field (optional)
    pub fn posted_at(mut self, value: DateTime<Utc>) -> Self {
        self.posted_at = Some(value);
        self
    }

    /// Build the StockReconciliation entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<StockReconciliation, String> {
        let recon_number = self.recon_number.ok_or_else(|| "recon_number is required".to_string())?;
        let company_id = self.company_id.ok_or_else(|| "company_id is required".to_string())?;
        let warehouse_id = self.warehouse_id.ok_or_else(|| "warehouse_id is required".to_string())?;
        let posting_date = self.posting_date.ok_or_else(|| "posting_date is required".to_string())?;
        let inventory_account_id = self.inventory_account_id.ok_or_else(|| "inventory_account_id is required".to_string())?;
        let adjustment_account_id = self.adjustment_account_id.ok_or_else(|| "adjustment_account_id is required".to_string())?;

        Ok(StockReconciliation {
            id: Uuid::new_v4(),
            recon_number,
            company_id,
            warehouse_id,
            posting_date,
            net_difference: self.net_difference.unwrap_or(Decimal::from(0)),
            inventory_account_id,
            adjustment_account_id,
            status: self.status.unwrap_or(DocStatus::default()),
            posting_state: self.posting_state.unwrap_or(GlPostingState::default()),
            journal_id: self.journal_id,
            accounting_post_id: self.accounting_post_id,
            posted_at: self.posted_at,
            metadata: AuditMetadata::default(),
        })
    }
}
