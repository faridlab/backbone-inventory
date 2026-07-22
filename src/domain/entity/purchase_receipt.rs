use chrono::{DateTime, Utc, NaiveDate};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use rust_decimal::Decimal;

use super::DocStatus;
use super::GlPostingState;
use super::AuditMetadata;

/// Strongly-typed ID for PurchaseReceipt
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PurchaseReceiptId(pub Uuid);

impl PurchaseReceiptId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for PurchaseReceiptId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for PurchaseReceiptId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for PurchaseReceiptId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<PurchaseReceiptId> for Uuid {
    fn from(id: PurchaseReceiptId) -> Self { id.0 }
}

impl AsRef<Uuid> for PurchaseReceiptId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for PurchaseReceiptId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PurchaseReceipt {
    pub id: Uuid,
    pub receipt_number: String,
    pub company_id: Uuid,
    pub branch_id: Option<Uuid>,
    pub supplier_id: Uuid,
    pub source_po_id: Option<Uuid>,
    pub warehouse_id: Uuid,
    pub posting_date: NaiveDate,
    pub currency: String,
    pub total_value: Decimal,
    pub inventory_account_id: Uuid,
    pub grir_account_id: Uuid,
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

impl PurchaseReceipt {
    /// Create a builder for PurchaseReceipt
    pub fn builder() -> PurchaseReceiptBuilder {
        PurchaseReceiptBuilder::default()
    }

    /// Create a new PurchaseReceipt with required fields
    pub fn new(receipt_number: String, company_id: Uuid, supplier_id: Uuid, warehouse_id: Uuid, posting_date: NaiveDate, currency: String, total_value: Decimal, inventory_account_id: Uuid, grir_account_id: Uuid, status: DocStatus, posting_state: GlPostingState) -> Self {
        Self {
            id: Uuid::new_v4(),
            receipt_number,
            company_id,
            branch_id: None,
            supplier_id,
            source_po_id: None,
            warehouse_id,
            posting_date,
            currency,
            total_value,
            inventory_account_id,
            grir_account_id,
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
    pub fn typed_id(&self) -> PurchaseReceiptId {
        PurchaseReceiptId(self.id)
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

    /// Set the source_po_id field (chainable)
    pub fn with_source_po_id(mut self, value: Uuid) -> Self {
        self.source_po_id = Some(value);
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
                "receipt_number" => {
                    if let Ok(v) = serde_json::from_value(value) { self.receipt_number = v; }
                }
                "company_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.company_id = v; }
                }
                "branch_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.branch_id = v; }
                }
                "supplier_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.supplier_id = v; }
                }
                "source_po_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.source_po_id = v; }
                }
                "warehouse_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.warehouse_id = v; }
                }
                "posting_date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.posting_date = v; }
                }
                "currency" => {
                    if let Ok(v) = serde_json::from_value(value) { self.currency = v; }
                }
                "total_value" => {
                    if let Ok(v) = serde_json::from_value(value) { self.total_value = v; }
                }
                "inventory_account_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.inventory_account_id = v; }
                }
                "grir_account_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.grir_account_id = v; }
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

impl super::Entity for PurchaseReceipt {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "PurchaseReceipt"
    }
}

impl backbone_core::PersistentEntity for PurchaseReceipt {
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

impl backbone_orm::EntityRepoMeta for PurchaseReceipt {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("company_id".to_string(), "uuid".to_string());
        m.insert("branch_id".to_string(), "uuid".to_string());
        m.insert("supplier_id".to_string(), "uuid".to_string());
        m.insert("source_po_id".to_string(), "uuid".to_string());
        m.insert("warehouse_id".to_string(), "uuid".to_string());
        m.insert("inventory_account_id".to_string(), "uuid".to_string());
        m.insert("grir_account_id".to_string(), "uuid".to_string());
        m.insert("journal_id".to_string(), "uuid".to_string());
        m.insert("accounting_post_id".to_string(), "uuid".to_string());
        m.insert("status".to_string(), "doc_status".to_string());
        m.insert("posting_state".to_string(), "gl_posting_state".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["receipt_number", "currency"]
    }
    fn company_field() -> Option<&'static str> {
        Some("company_id")
    }
}

/// Builder for PurchaseReceipt entity
///
/// Provides a fluent API for constructing PurchaseReceipt instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct PurchaseReceiptBuilder {
    receipt_number: Option<String>,
    company_id: Option<Uuid>,
    branch_id: Option<Uuid>,
    supplier_id: Option<Uuid>,
    source_po_id: Option<Uuid>,
    warehouse_id: Option<Uuid>,
    posting_date: Option<NaiveDate>,
    currency: Option<String>,
    total_value: Option<Decimal>,
    inventory_account_id: Option<Uuid>,
    grir_account_id: Option<Uuid>,
    status: Option<DocStatus>,
    posting_state: Option<GlPostingState>,
    journal_id: Option<Uuid>,
    accounting_post_id: Option<Uuid>,
    posted_at: Option<DateTime<Utc>>,
    notes: Option<String>,
}

impl PurchaseReceiptBuilder {
    /// Set the receipt_number field (required)
    pub fn receipt_number(mut self, value: String) -> Self {
        self.receipt_number = Some(value);
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

    /// Set the supplier_id field (required)
    pub fn supplier_id(mut self, value: Uuid) -> Self {
        self.supplier_id = Some(value);
        self
    }

    /// Set the source_po_id field (optional)
    pub fn source_po_id(mut self, value: Uuid) -> Self {
        self.source_po_id = Some(value);
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

    /// Set the currency field (default: `"IDR".to_string()`)
    pub fn currency(mut self, value: String) -> Self {
        self.currency = Some(value);
        self
    }

    /// Set the total_value field (default: `Decimal::from(0)`)
    pub fn total_value(mut self, value: Decimal) -> Self {
        self.total_value = Some(value);
        self
    }

    /// Set the inventory_account_id field (required)
    pub fn inventory_account_id(mut self, value: Uuid) -> Self {
        self.inventory_account_id = Some(value);
        self
    }

    /// Set the grir_account_id field (required)
    pub fn grir_account_id(mut self, value: Uuid) -> Self {
        self.grir_account_id = Some(value);
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

    /// Build the PurchaseReceipt entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<PurchaseReceipt, String> {
        let receipt_number = self.receipt_number.ok_or_else(|| "receipt_number is required".to_string())?;
        let company_id = self.company_id.ok_or_else(|| "company_id is required".to_string())?;
        let supplier_id = self.supplier_id.ok_or_else(|| "supplier_id is required".to_string())?;
        let warehouse_id = self.warehouse_id.ok_or_else(|| "warehouse_id is required".to_string())?;
        let posting_date = self.posting_date.ok_or_else(|| "posting_date is required".to_string())?;
        let inventory_account_id = self.inventory_account_id.ok_or_else(|| "inventory_account_id is required".to_string())?;
        let grir_account_id = self.grir_account_id.ok_or_else(|| "grir_account_id is required".to_string())?;

        Ok(PurchaseReceipt {
            id: Uuid::new_v4(),
            receipt_number,
            company_id,
            branch_id: self.branch_id,
            supplier_id,
            source_po_id: self.source_po_id,
            warehouse_id,
            posting_date,
            currency: self.currency.unwrap_or("IDR".to_string()),
            total_value: self.total_value.unwrap_or(Decimal::from(0)),
            inventory_account_id,
            grir_account_id,
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
