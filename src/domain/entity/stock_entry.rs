use chrono::{DateTime, Utc, NaiveDate};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use super::StockEntryType;
use super::DocStatus;
use super::GlPostingState;
use super::AuditMetadata;

/// Strongly-typed ID for StockEntry
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StockEntryId(pub Uuid);

impl StockEntryId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for StockEntryId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for StockEntryId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for StockEntryId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<StockEntryId> for Uuid {
    fn from(id: StockEntryId) -> Self { id.0 }
}

impl AsRef<Uuid> for StockEntryId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for StockEntryId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct StockEntry {
    pub id: Uuid,
    pub entry_number: String,
    pub company_id: Uuid,
    pub stock_entry_type: StockEntryType,
    pub from_warehouse_id: Option<Uuid>,
    pub to_warehouse_id: Option<Uuid>,
    pub posting_date: NaiveDate,
    pub status: DocStatus,
    pub posting_state: GlPostingState,
    pub notes: Option<String>,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl StockEntry {
    /// Create a builder for StockEntry
    pub fn builder() -> StockEntryBuilder {
        StockEntryBuilder::default()
    }

    /// Create a new StockEntry with required fields
    pub fn new(entry_number: String, company_id: Uuid, stock_entry_type: StockEntryType, posting_date: NaiveDate, status: DocStatus, posting_state: GlPostingState) -> Self {
        Self {
            id: Uuid::new_v4(),
            entry_number,
            company_id,
            stock_entry_type,
            from_warehouse_id: None,
            to_warehouse_id: None,
            posting_date,
            status,
            posting_state,
            notes: None,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> StockEntryId {
        StockEntryId(self.id)
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

    /// Set the from_warehouse_id field (chainable)
    pub fn with_from_warehouse_id(mut self, value: Uuid) -> Self {
        self.from_warehouse_id = Some(value);
        self
    }

    /// Set the to_warehouse_id field (chainable)
    pub fn with_to_warehouse_id(mut self, value: Uuid) -> Self {
        self.to_warehouse_id = Some(value);
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
                "entry_number" => {
                    if let Ok(v) = serde_json::from_value(value) { self.entry_number = v; }
                }
                "company_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.company_id = v; }
                }
                "stock_entry_type" => {
                    if let Ok(v) = serde_json::from_value(value) { self.stock_entry_type = v; }
                }
                "from_warehouse_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.from_warehouse_id = v; }
                }
                "to_warehouse_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.to_warehouse_id = v; }
                }
                "posting_date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.posting_date = v; }
                }
                "status" => {
                    if let Ok(v) = serde_json::from_value(value) { self.status = v; }
                }
                "posting_state" => {
                    if let Ok(v) = serde_json::from_value(value) { self.posting_state = v; }
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

impl super::Entity for StockEntry {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "StockEntry"
    }
}

impl backbone_core::PersistentEntity for StockEntry {
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

impl backbone_orm::EntityRepoMeta for StockEntry {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("company_id".to_string(), "uuid".to_string());
        m.insert("from_warehouse_id".to_string(), "uuid".to_string());
        m.insert("to_warehouse_id".to_string(), "uuid".to_string());
        m.insert("stock_entry_type".to_string(), "stock_entry_type".to_string());
        m.insert("status".to_string(), "doc_status".to_string());
        m.insert("posting_state".to_string(), "gl_posting_state".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["entry_number"]
    }
}

/// Builder for StockEntry entity
///
/// Provides a fluent API for constructing StockEntry instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct StockEntryBuilder {
    entry_number: Option<String>,
    company_id: Option<Uuid>,
    stock_entry_type: Option<StockEntryType>,
    from_warehouse_id: Option<Uuid>,
    to_warehouse_id: Option<Uuid>,
    posting_date: Option<NaiveDate>,
    status: Option<DocStatus>,
    posting_state: Option<GlPostingState>,
    notes: Option<String>,
}

impl StockEntryBuilder {
    /// Set the entry_number field (required)
    pub fn entry_number(mut self, value: String) -> Self {
        self.entry_number = Some(value);
        self
    }

    /// Set the company_id field (required)
    pub fn company_id(mut self, value: Uuid) -> Self {
        self.company_id = Some(value);
        self
    }

    /// Set the stock_entry_type field (default: `StockEntryType::default()`)
    pub fn stock_entry_type(mut self, value: StockEntryType) -> Self {
        self.stock_entry_type = Some(value);
        self
    }

    /// Set the from_warehouse_id field (optional)
    pub fn from_warehouse_id(mut self, value: Uuid) -> Self {
        self.from_warehouse_id = Some(value);
        self
    }

    /// Set the to_warehouse_id field (optional)
    pub fn to_warehouse_id(mut self, value: Uuid) -> Self {
        self.to_warehouse_id = Some(value);
        self
    }

    /// Set the posting_date field (required)
    pub fn posting_date(mut self, value: NaiveDate) -> Self {
        self.posting_date = Some(value);
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

    /// Set the notes field (optional)
    pub fn notes(mut self, value: String) -> Self {
        self.notes = Some(value);
        self
    }

    /// Build the StockEntry entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<StockEntry, String> {
        let entry_number = self.entry_number.ok_or_else(|| "entry_number is required".to_string())?;
        let company_id = self.company_id.ok_or_else(|| "company_id is required".to_string())?;
        let posting_date = self.posting_date.ok_or_else(|| "posting_date is required".to_string())?;

        Ok(StockEntry {
            id: Uuid::new_v4(),
            entry_number,
            company_id,
            stock_entry_type: self.stock_entry_type.unwrap_or(StockEntryType::default()),
            from_warehouse_id: self.from_warehouse_id,
            to_warehouse_id: self.to_warehouse_id,
            posting_date,
            status: self.status.unwrap_or(DocStatus::default()),
            posting_state: self.posting_state.unwrap_or(GlPostingState::default()),
            notes: self.notes,
            metadata: AuditMetadata::default(),
        })
    }
}
