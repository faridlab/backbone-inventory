use chrono::{DateTime, Utc, NaiveDate};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use super::GoodsReceiptStatus;
use super::AuditMetadata;

/// Strongly-typed ID for GoodsReceipt
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GoodsReceiptId(pub Uuid);

impl GoodsReceiptId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for GoodsReceiptId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for GoodsReceiptId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for GoodsReceiptId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<GoodsReceiptId> for Uuid {
    fn from(id: GoodsReceiptId) -> Self { id.0 }
}

impl AsRef<Uuid> for GoodsReceiptId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for GoodsReceiptId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct GoodsReceipt {
    pub id: Uuid,
    pub provider_id: Uuid,
    pub receipt_number: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub purchase_order_id: Option<Uuid>,
    pub supplier_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delivery_note: Option<String>,
    pub warehouse_id: Uuid,
    pub receipt_date: NaiveDate,
    pub currency: String,
    pub requires_qc: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qc_date: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qc_by: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qc_notes: Option<String>,
    pub status: GoodsReceiptStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub received_by: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub received_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub posted_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub posted_by: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cancelled_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cancelled_by: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cancellation_reason: Option<String>,
    pub attachments: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    pub data: serde_json::Value,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl GoodsReceipt {
    /// Create a builder for GoodsReceipt
    pub fn builder() -> GoodsReceiptBuilder {
        GoodsReceiptBuilder::default()
    }

    /// Create a new GoodsReceipt with required fields
    pub fn new(provider_id: Uuid, receipt_number: String, supplier_name: String, warehouse_id: Uuid, receipt_date: NaiveDate, currency: String, requires_qc: bool, status: GoodsReceiptStatus, attachments: serde_json::Value, data: serde_json::Value) -> Self {
        Self {
            id: Uuid::new_v4(),
            provider_id,
            receipt_number,
            purchase_order_id: None,
            supplier_name,
            delivery_note: None,
            warehouse_id,
            receipt_date,
            currency,
            requires_qc,
            qc_date: None,
            qc_by: None,
            qc_notes: None,
            status,
            received_by: None,
            received_at: None,
            posted_at: None,
            posted_by: None,
            cancelled_at: None,
            cancelled_by: None,
            cancellation_reason: None,
            attachments,
            notes: None,
            data,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> GoodsReceiptId {
        GoodsReceiptId(self.id)
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
    pub fn status(&self) -> &GoodsReceiptStatus {
        &self.status
    }


    // ==========================================================
    // Fluent Setters (with_* for optional fields)
    // ==========================================================

    /// Set the purchase_order_id field (chainable)
    pub fn with_purchase_order_id(mut self, value: Uuid) -> Self {
        self.purchase_order_id = Some(value);
        self
    }

    /// Set the delivery_note field (chainable)
    pub fn with_delivery_note(mut self, value: String) -> Self {
        self.delivery_note = Some(value);
        self
    }

    /// Set the qc_date field (chainable)
    pub fn with_qc_date(mut self, value: DateTime<Utc>) -> Self {
        self.qc_date = Some(value);
        self
    }

    /// Set the qc_by field (chainable)
    pub fn with_qc_by(mut self, value: Uuid) -> Self {
        self.qc_by = Some(value);
        self
    }

    /// Set the qc_notes field (chainable)
    pub fn with_qc_notes(mut self, value: String) -> Self {
        self.qc_notes = Some(value);
        self
    }

    /// Set the received_by field (chainable)
    pub fn with_received_by(mut self, value: Uuid) -> Self {
        self.received_by = Some(value);
        self
    }

    /// Set the received_at field (chainable)
    pub fn with_received_at(mut self, value: DateTime<Utc>) -> Self {
        self.received_at = Some(value);
        self
    }

    /// Set the posted_at field (chainable)
    pub fn with_posted_at(mut self, value: DateTime<Utc>) -> Self {
        self.posted_at = Some(value);
        self
    }

    /// Set the posted_by field (chainable)
    pub fn with_posted_by(mut self, value: Uuid) -> Self {
        self.posted_by = Some(value);
        self
    }

    /// Set the cancelled_at field (chainable)
    pub fn with_cancelled_at(mut self, value: DateTime<Utc>) -> Self {
        self.cancelled_at = Some(value);
        self
    }

    /// Set the cancelled_by field (chainable)
    pub fn with_cancelled_by(mut self, value: Uuid) -> Self {
        self.cancelled_by = Some(value);
        self
    }

    /// Set the cancellation_reason field (chainable)
    pub fn with_cancellation_reason(mut self, value: String) -> Self {
        self.cancellation_reason = Some(value);
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
                "provider_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.provider_id = v; }
                }
                "receipt_number" => {
                    if let Ok(v) = serde_json::from_value(value) { self.receipt_number = v; }
                }
                "purchase_order_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.purchase_order_id = v; }
                }
                "supplier_name" => {
                    if let Ok(v) = serde_json::from_value(value) { self.supplier_name = v; }
                }
                "delivery_note" => {
                    if let Ok(v) = serde_json::from_value(value) { self.delivery_note = v; }
                }
                "warehouse_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.warehouse_id = v; }
                }
                "receipt_date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.receipt_date = v; }
                }
                "currency" => {
                    if let Ok(v) = serde_json::from_value(value) { self.currency = v; }
                }
                "requires_qc" => {
                    if let Ok(v) = serde_json::from_value(value) { self.requires_qc = v; }
                }
                "qc_date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.qc_date = v; }
                }
                "qc_by" => {
                    if let Ok(v) = serde_json::from_value(value) { self.qc_by = v; }
                }
                "qc_notes" => {
                    if let Ok(v) = serde_json::from_value(value) { self.qc_notes = v; }
                }
                "status" => {
                    if let Ok(v) = serde_json::from_value(value) { self.status = v; }
                }
                "received_by" => {
                    if let Ok(v) = serde_json::from_value(value) { self.received_by = v; }
                }
                "received_at" => {
                    if let Ok(v) = serde_json::from_value(value) { self.received_at = v; }
                }
                "posted_at" => {
                    if let Ok(v) = serde_json::from_value(value) { self.posted_at = v; }
                }
                "posted_by" => {
                    if let Ok(v) = serde_json::from_value(value) { self.posted_by = v; }
                }
                "cancelled_at" => {
                    if let Ok(v) = serde_json::from_value(value) { self.cancelled_at = v; }
                }
                "cancelled_by" => {
                    if let Ok(v) = serde_json::from_value(value) { self.cancelled_by = v; }
                }
                "cancellation_reason" => {
                    if let Ok(v) = serde_json::from_value(value) { self.cancellation_reason = v; }
                }
                "attachments" => {
                    if let Ok(v) = serde_json::from_value(value) { self.attachments = v; }
                }
                "notes" => {
                    if let Ok(v) = serde_json::from_value(value) { self.notes = v; }
                }
                "data" => {
                    if let Ok(v) = serde_json::from_value(value) { self.data = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for GoodsReceipt {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "GoodsReceipt"
    }
}

impl backbone_core::PersistentEntity for GoodsReceipt {
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

impl backbone_orm::EntityRepoMeta for GoodsReceipt {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("provider_id".to_string(), "uuid".to_string());
        m.insert("purchase_order_id".to_string(), "uuid".to_string());
        m.insert("warehouse_id".to_string(), "uuid".to_string());
        m.insert("status".to_string(), "goods_receipt_status".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["receipt_number", "supplier_name", "currency"]
    }
}

/// Builder for GoodsReceipt entity
///
/// Provides a fluent API for constructing GoodsReceipt instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct GoodsReceiptBuilder {
    provider_id: Option<Uuid>,
    receipt_number: Option<String>,
    purchase_order_id: Option<Uuid>,
    supplier_name: Option<String>,
    delivery_note: Option<String>,
    warehouse_id: Option<Uuid>,
    receipt_date: Option<NaiveDate>,
    currency: Option<String>,
    requires_qc: Option<bool>,
    qc_date: Option<DateTime<Utc>>,
    qc_by: Option<Uuid>,
    qc_notes: Option<String>,
    status: Option<GoodsReceiptStatus>,
    received_by: Option<Uuid>,
    received_at: Option<DateTime<Utc>>,
    posted_at: Option<DateTime<Utc>>,
    posted_by: Option<Uuid>,
    cancelled_at: Option<DateTime<Utc>>,
    cancelled_by: Option<Uuid>,
    cancellation_reason: Option<String>,
    attachments: Option<serde_json::Value>,
    notes: Option<String>,
    data: Option<serde_json::Value>,
}

impl GoodsReceiptBuilder {
    /// Set the provider_id field (required)
    pub fn provider_id(mut self, value: Uuid) -> Self {
        self.provider_id = Some(value);
        self
    }

    /// Set the receipt_number field (required)
    pub fn receipt_number(mut self, value: String) -> Self {
        self.receipt_number = Some(value);
        self
    }

    /// Set the purchase_order_id field (optional)
    pub fn purchase_order_id(mut self, value: Uuid) -> Self {
        self.purchase_order_id = Some(value);
        self
    }

    /// Set the supplier_name field (required)
    pub fn supplier_name(mut self, value: String) -> Self {
        self.supplier_name = Some(value);
        self
    }

    /// Set the delivery_note field (optional)
    pub fn delivery_note(mut self, value: String) -> Self {
        self.delivery_note = Some(value);
        self
    }

    /// Set the warehouse_id field (required)
    pub fn warehouse_id(mut self, value: Uuid) -> Self {
        self.warehouse_id = Some(value);
        self
    }

    /// Set the receipt_date field (required)
    pub fn receipt_date(mut self, value: NaiveDate) -> Self {
        self.receipt_date = Some(value);
        self
    }

    /// Set the currency field (default: `"IDR".to_string()`)
    pub fn currency(mut self, value: String) -> Self {
        self.currency = Some(value);
        self
    }

    /// Set the requires_qc field (default: `false`)
    pub fn requires_qc(mut self, value: bool) -> Self {
        self.requires_qc = Some(value);
        self
    }

    /// Set the qc_date field (optional)
    pub fn qc_date(mut self, value: DateTime<Utc>) -> Self {
        self.qc_date = Some(value);
        self
    }

    /// Set the qc_by field (optional)
    pub fn qc_by(mut self, value: Uuid) -> Self {
        self.qc_by = Some(value);
        self
    }

    /// Set the qc_notes field (optional)
    pub fn qc_notes(mut self, value: String) -> Self {
        self.qc_notes = Some(value);
        self
    }

    /// Set the status field (default: `GoodsReceiptStatus::default()`)
    pub fn status(mut self, value: GoodsReceiptStatus) -> Self {
        self.status = Some(value);
        self
    }

    /// Set the received_by field (optional)
    pub fn received_by(mut self, value: Uuid) -> Self {
        self.received_by = Some(value);
        self
    }

    /// Set the received_at field (optional)
    pub fn received_at(mut self, value: DateTime<Utc>) -> Self {
        self.received_at = Some(value);
        self
    }

    /// Set the posted_at field (optional)
    pub fn posted_at(mut self, value: DateTime<Utc>) -> Self {
        self.posted_at = Some(value);
        self
    }

    /// Set the posted_by field (optional)
    pub fn posted_by(mut self, value: Uuid) -> Self {
        self.posted_by = Some(value);
        self
    }

    /// Set the cancelled_at field (optional)
    pub fn cancelled_at(mut self, value: DateTime<Utc>) -> Self {
        self.cancelled_at = Some(value);
        self
    }

    /// Set the cancelled_by field (optional)
    pub fn cancelled_by(mut self, value: Uuid) -> Self {
        self.cancelled_by = Some(value);
        self
    }

    /// Set the cancellation_reason field (optional)
    pub fn cancellation_reason(mut self, value: String) -> Self {
        self.cancellation_reason = Some(value);
        self
    }

    /// Set the attachments field (default: `serde_json::json!([])`)
    pub fn attachments(mut self, value: serde_json::Value) -> Self {
        self.attachments = Some(value);
        self
    }

    /// Set the notes field (optional)
    pub fn notes(mut self, value: String) -> Self {
        self.notes = Some(value);
        self
    }

    /// Set the data field (default: `serde_json::json!({"total_items":0,"total_quantity_expected":0,"total_quantity_received":0,"total_quantity_accepted":0,"total_quantity_rejected":0,"total_value":0,"qc_completed":false})`)
    pub fn data(mut self, value: serde_json::Value) -> Self {
        self.data = Some(value);
        self
    }

    /// Build the GoodsReceipt entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<GoodsReceipt, String> {
        let provider_id = self.provider_id.ok_or_else(|| "provider_id is required".to_string())?;
        let receipt_number = self.receipt_number.ok_or_else(|| "receipt_number is required".to_string())?;
        let supplier_name = self.supplier_name.ok_or_else(|| "supplier_name is required".to_string())?;
        let warehouse_id = self.warehouse_id.ok_or_else(|| "warehouse_id is required".to_string())?;
        let receipt_date = self.receipt_date.ok_or_else(|| "receipt_date is required".to_string())?;

        Ok(GoodsReceipt {
            id: Uuid::new_v4(),
            provider_id,
            receipt_number,
            purchase_order_id: self.purchase_order_id,
            supplier_name,
            delivery_note: self.delivery_note,
            warehouse_id,
            receipt_date,
            currency: self.currency.unwrap_or("IDR".to_string()),
            requires_qc: self.requires_qc.unwrap_or(false),
            qc_date: self.qc_date,
            qc_by: self.qc_by,
            qc_notes: self.qc_notes,
            status: self.status.unwrap_or(GoodsReceiptStatus::default()),
            received_by: self.received_by,
            received_at: self.received_at,
            posted_at: self.posted_at,
            posted_by: self.posted_by,
            cancelled_at: self.cancelled_at,
            cancelled_by: self.cancelled_by,
            cancellation_reason: self.cancellation_reason,
            attachments: self.attachments.unwrap_or(serde_json::json!([])),
            notes: self.notes,
            data: self.data.unwrap_or(serde_json::json!({"total_items":0,"total_quantity_expected":0,"total_quantity_received":0,"total_quantity_accepted":0,"total_quantity_rejected":0,"total_value":0,"qc_completed":false})),
            metadata: AuditMetadata::default(),
        })
    }
}
