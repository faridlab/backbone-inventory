use chrono::{DateTime, Utc, NaiveDate};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use rust_decimal::Decimal;

use super::LandedCostState;
use super::GlPostingState;
use super::AuditMetadata;

/// Strongly-typed ID for LandedCost
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LandedCostId(pub Uuid);

impl LandedCostId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for LandedCostId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for LandedCostId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for LandedCostId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<LandedCostId> for Uuid {
    fn from(id: LandedCostId) -> Self { id.0 }
}

impl AsRef<Uuid> for LandedCostId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for LandedCostId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LandedCost {
    pub id: Uuid,
    pub lc_number: String,
    pub branch_id: Option<Uuid>,
    pub target_receipt_id: Uuid,
    pub transfer_id: Option<Uuid>,
    pub currency: String,
    pub posting_date: NaiveDate,
    pub state: LandedCostState,
    pub amount_total: Decimal,
    pub posting_state: GlPostingState,
    pub journal_id: Option<Uuid>,
    pub accounting_post_id: Option<Uuid>,
    pub posted_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl LandedCost {
    /// Create a builder for LandedCost
    pub fn builder() -> LandedCostBuilder {
        <LandedCostBuilder as Default>::default()
    }

    /// Create a new LandedCost with required fields
    pub fn new(lc_number: String, target_receipt_id: Uuid, currency: String, posting_date: NaiveDate, state: LandedCostState, amount_total: Decimal, posting_state: GlPostingState) -> Self {
        Self {
            id: Uuid::new_v4(),
            lc_number,
            branch_id: None,
            target_receipt_id,
            transfer_id: None,
            currency,
            posting_date,
            state,
            amount_total,
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
    pub fn typed_id(&self) -> LandedCostId {
        LandedCostId(self.id)
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

    /// Set the branch_id field (chainable)
    pub fn with_branch_id(mut self, value: Uuid) -> Self {
        self.branch_id = Some(value);
        self
    }

    /// Set the transfer_id field (chainable)
    pub fn with_transfer_id(mut self, value: Uuid) -> Self {
        self.transfer_id = Some(value);
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
                "lc_number" => {
                    if let Ok(v) = serde_json::from_value(value) { self.lc_number = v; }
                }
                "branch_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.branch_id = v; }
                }
                "target_receipt_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.target_receipt_id = v; }
                }
                "transfer_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.transfer_id = v; }
                }
                "currency" => {
                    if let Ok(v) = serde_json::from_value(value) { self.currency = v; }
                }
                "posting_date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.posting_date = v; }
                }
                "state" => {
                    if let Ok(v) = serde_json::from_value(value) { self.state = v; }
                }
                "amount_total" => {
                    if let Ok(v) = serde_json::from_value(value) { self.amount_total = v; }
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

impl super::Entity for LandedCost {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "LandedCost"
    }
}

impl backbone_core::PersistentEntity for LandedCost {
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

impl backbone_orm::EntityRepoMeta for LandedCost {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("branch_id".to_string(), "uuid".to_string());
        m.insert("target_receipt_id".to_string(), "uuid".to_string());
        m.insert("transfer_id".to_string(), "uuid".to_string());
        m.insert("journal_id".to_string(), "uuid".to_string());
        m.insert("accounting_post_id".to_string(), "uuid".to_string());
        m.insert("state".to_string(), "landed_cost_state".to_string());
        m.insert("posting_state".to_string(), "gl_posting_state".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["lc_number", "currency"]
    }
}

/// Builder for LandedCost entity
///
/// Provides a fluent API for constructing LandedCost instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct LandedCostBuilder {
    lc_number: Option<String>,
    branch_id: Option<Uuid>,
    target_receipt_id: Option<Uuid>,
    transfer_id: Option<Uuid>,
    currency: Option<String>,
    posting_date: Option<NaiveDate>,
    state: Option<LandedCostState>,
    amount_total: Option<Decimal>,
    posting_state: Option<GlPostingState>,
    journal_id: Option<Uuid>,
    accounting_post_id: Option<Uuid>,
    posted_at: Option<DateTime<Utc>>,
    notes: Option<String>,
}

impl LandedCostBuilder {
    /// Set the lc_number field (required)
    pub fn lc_number(mut self, value: String) -> Self {
        self.lc_number = Some(value);
        self
    }

    /// Set the branch_id field (optional)
    pub fn branch_id(mut self, value: Uuid) -> Self {
        self.branch_id = Some(value);
        self
    }

    /// Set the target_receipt_id field (required)
    pub fn target_receipt_id(mut self, value: Uuid) -> Self {
        self.target_receipt_id = Some(value);
        self
    }

    /// Set the transfer_id field (optional)
    pub fn transfer_id(mut self, value: Uuid) -> Self {
        self.transfer_id = Some(value);
        self
    }

    /// Set the currency field (default: `"IDR".to_string()`)
    pub fn currency(mut self, value: String) -> Self {
        self.currency = Some(value);
        self
    }

    /// Set the posting_date field (required)
    pub fn posting_date(mut self, value: NaiveDate) -> Self {
        self.posting_date = Some(value);
        self
    }

    /// Set the state field (default: `LandedCostState::default()`)
    pub fn state(mut self, value: LandedCostState) -> Self {
        self.state = Some(value);
        self
    }

    /// Set the amount_total field (default: `Decimal::from(0)`)
    pub fn amount_total(mut self, value: Decimal) -> Self {
        self.amount_total = Some(value);
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

    /// Build the LandedCost entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<LandedCost, String> {
        let lc_number = self.lc_number.ok_or_else(|| "lc_number is required".to_string())?;
        let target_receipt_id = self.target_receipt_id.ok_or_else(|| "target_receipt_id is required".to_string())?;
        let posting_date = self.posting_date.ok_or_else(|| "posting_date is required".to_string())?;

        Ok(LandedCost {
            id: Uuid::new_v4(),
            lc_number,
            branch_id: self.branch_id,
            target_receipt_id,
            transfer_id: self.transfer_id,
            currency: self.currency.unwrap_or("IDR".to_string()),
            posting_date,
            state: self.state.unwrap_or_default(),
            amount_total: self.amount_total.unwrap_or(Decimal::from(0)),
            posting_state: self.posting_state.unwrap_or_default(),
            journal_id: self.journal_id,
            accounting_post_id: self.accounting_post_id,
            posted_at: self.posted_at,
            notes: self.notes,
            metadata: AuditMetadata::default(),
        })
    }
}
