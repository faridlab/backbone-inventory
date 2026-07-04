use chrono::{DateTime, Utc, NaiveDate};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use rust_decimal::Decimal;

use super::DocStatus;
use super::GlPostingState;
use super::AuditMetadata;

/// Strongly-typed ID for DeliveryNote
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DeliveryNoteId(pub Uuid);

impl DeliveryNoteId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for DeliveryNoteId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for DeliveryNoteId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for DeliveryNoteId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<DeliveryNoteId> for Uuid {
    fn from(id: DeliveryNoteId) -> Self { id.0 }
}

impl AsRef<Uuid> for DeliveryNoteId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for DeliveryNoteId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DeliveryNote {
    pub id: Uuid,
    pub delivery_number: String,
    pub company_id: Uuid,
    pub branch_id: Option<Uuid>,
    pub customer_id: Uuid,
    pub source_so_id: Option<Uuid>,
    pub warehouse_id: Uuid,
    pub posting_date: NaiveDate,
    pub total_cogs: Decimal,
    pub cogs_account_id: Uuid,
    pub inventory_account_id: Uuid,
    pub status: DocStatus,
    pub posting_state: GlPostingState,
    pub journal_id: Option<Uuid>,
    pub accounting_post_id: Option<Uuid>,
    pub posted_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl DeliveryNote {
    /// Create a builder for DeliveryNote
    pub fn builder() -> DeliveryNoteBuilder {
        DeliveryNoteBuilder::default()
    }

    /// Create a new DeliveryNote with required fields
    pub fn new(delivery_number: String, company_id: Uuid, customer_id: Uuid, warehouse_id: Uuid, posting_date: NaiveDate, total_cogs: Decimal, cogs_account_id: Uuid, inventory_account_id: Uuid, status: DocStatus, posting_state: GlPostingState) -> Self {
        Self {
            id: Uuid::new_v4(),
            delivery_number,
            company_id,
            branch_id: None,
            customer_id,
            source_so_id: None,
            warehouse_id,
            posting_date,
            total_cogs,
            cogs_account_id,
            inventory_account_id,
            status,
            posting_state,
            journal_id: None,
            accounting_post_id: None,
            posted_at: None,
            notes: None,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> DeliveryNoteId {
        DeliveryNoteId(self.id)
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

    /// Set the branch_id field (chainable)
    pub fn with_branch_id(mut self, value: Uuid) -> Self {
        self.branch_id = Some(value);
        self
    }

    /// Set the source_so_id field (chainable)
    pub fn with_source_so_id(mut self, value: Uuid) -> Self {
        self.source_so_id = Some(value);
        self
    }

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
                "delivery_number" => {
                    if let Ok(v) = serde_json::from_value(value) { self.delivery_number = v; }
                }
                "company_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.company_id = v; }
                }
                "branch_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.branch_id = v; }
                }
                "customer_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.customer_id = v; }
                }
                "source_so_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.source_so_id = v; }
                }
                "warehouse_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.warehouse_id = v; }
                }
                "posting_date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.posting_date = v; }
                }
                "total_cogs" => {
                    if let Ok(v) = serde_json::from_value(value) { self.total_cogs = v; }
                }
                "cogs_account_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.cogs_account_id = v; }
                }
                "inventory_account_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.inventory_account_id = v; }
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

impl super::Entity for DeliveryNote {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "DeliveryNote"
    }
}

impl backbone_core::PersistentEntity for DeliveryNote {
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

impl backbone_orm::EntityRepoMeta for DeliveryNote {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("company_id".to_string(), "uuid".to_string());
        m.insert("branch_id".to_string(), "uuid".to_string());
        m.insert("customer_id".to_string(), "uuid".to_string());
        m.insert("source_so_id".to_string(), "uuid".to_string());
        m.insert("warehouse_id".to_string(), "uuid".to_string());
        m.insert("cogs_account_id".to_string(), "uuid".to_string());
        m.insert("inventory_account_id".to_string(), "uuid".to_string());
        m.insert("journal_id".to_string(), "uuid".to_string());
        m.insert("accounting_post_id".to_string(), "uuid".to_string());
        m.insert("status".to_string(), "doc_status".to_string());
        m.insert("posting_state".to_string(), "gl_posting_state".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["delivery_number"]
    }
}

/// Builder for DeliveryNote entity
///
/// Provides a fluent API for constructing DeliveryNote instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct DeliveryNoteBuilder {
    delivery_number: Option<String>,
    company_id: Option<Uuid>,
    branch_id: Option<Uuid>,
    customer_id: Option<Uuid>,
    source_so_id: Option<Uuid>,
    warehouse_id: Option<Uuid>,
    posting_date: Option<NaiveDate>,
    total_cogs: Option<Decimal>,
    cogs_account_id: Option<Uuid>,
    inventory_account_id: Option<Uuid>,
    status: Option<DocStatus>,
    posting_state: Option<GlPostingState>,
    journal_id: Option<Uuid>,
    accounting_post_id: Option<Uuid>,
    posted_at: Option<DateTime<Utc>>,
    notes: Option<String>,
}

impl DeliveryNoteBuilder {
    /// Set the delivery_number field (required)
    pub fn delivery_number(mut self, value: String) -> Self {
        self.delivery_number = Some(value);
        self
    }

    /// Set the company_id field (required)
    pub fn company_id(mut self, value: Uuid) -> Self {
        self.company_id = Some(value);
        self
    }

    /// Set the branch_id field (optional)
    pub fn branch_id(mut self, value: Uuid) -> Self {
        self.branch_id = Some(value);
        self
    }

    /// Set the customer_id field (required)
    pub fn customer_id(mut self, value: Uuid) -> Self {
        self.customer_id = Some(value);
        self
    }

    /// Set the source_so_id field (optional)
    pub fn source_so_id(mut self, value: Uuid) -> Self {
        self.source_so_id = Some(value);
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

    /// Set the total_cogs field (default: `Decimal::from(0)`)
    pub fn total_cogs(mut self, value: Decimal) -> Self {
        self.total_cogs = Some(value);
        self
    }

    /// Set the cogs_account_id field (required)
    pub fn cogs_account_id(mut self, value: Uuid) -> Self {
        self.cogs_account_id = Some(value);
        self
    }

    /// Set the inventory_account_id field (required)
    pub fn inventory_account_id(mut self, value: Uuid) -> Self {
        self.inventory_account_id = Some(value);
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

    /// Set the notes field (optional)
    pub fn notes(mut self, value: String) -> Self {
        self.notes = Some(value);
        self
    }

    /// Build the DeliveryNote entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<DeliveryNote, String> {
        let delivery_number = self.delivery_number.ok_or_else(|| "delivery_number is required".to_string())?;
        let company_id = self.company_id.ok_or_else(|| "company_id is required".to_string())?;
        let customer_id = self.customer_id.ok_or_else(|| "customer_id is required".to_string())?;
        let warehouse_id = self.warehouse_id.ok_or_else(|| "warehouse_id is required".to_string())?;
        let posting_date = self.posting_date.ok_or_else(|| "posting_date is required".to_string())?;
        let cogs_account_id = self.cogs_account_id.ok_or_else(|| "cogs_account_id is required".to_string())?;
        let inventory_account_id = self.inventory_account_id.ok_or_else(|| "inventory_account_id is required".to_string())?;

        Ok(DeliveryNote {
            id: Uuid::new_v4(),
            delivery_number,
            company_id,
            branch_id: self.branch_id,
            customer_id,
            source_so_id: self.source_so_id,
            warehouse_id,
            posting_date,
            total_cogs: self.total_cogs.unwrap_or(Decimal::from(0)),
            cogs_account_id,
            inventory_account_id,
            status: self.status.unwrap_or(DocStatus::default()),
            posting_state: self.posting_state.unwrap_or(GlPostingState::default()),
            journal_id: self.journal_id,
            accounting_post_id: self.accounting_post_id,
            posted_at: self.posted_at,
            notes: self.notes,
            metadata: AuditMetadata::default(),
        })
    }
}
