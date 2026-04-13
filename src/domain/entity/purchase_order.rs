use chrono::{DateTime, Utc, NaiveDate};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use rust_decimal::Decimal;

use super::PurchaseOrderType;
use super::PurchaseOrderStatus;
use super::AuditMetadata;

/// Strongly-typed ID for PurchaseOrder
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PurchaseOrderId(pub Uuid);

impl PurchaseOrderId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for PurchaseOrderId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for PurchaseOrderId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for PurchaseOrderId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<PurchaseOrderId> for Uuid {
    fn from(id: PurchaseOrderId) -> Self { id.0 }
}

impl AsRef<Uuid> for PurchaseOrderId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for PurchaseOrderId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PurchaseOrder {
    pub id: Uuid,
    pub provider_id: Uuid,
    pub po_number: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_number: Option<String>,
    pub po_type: PurchaseOrderType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supplier_id: Option<Uuid>,
    pub supplier_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supplier_contact_person: Option<String>,
    pub warehouse_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delivery_address: Option<String>,
    pub order_date: NaiveDate,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_date: Option<NaiveDate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delivery_date: Option<NaiveDate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiry_date: Option<NaiveDate>,
    pub currency: String,
    pub subtotal: Decimal,
    pub discount_percent: Decimal,
    pub discount_amount: Decimal,
    pub tax_percent: Decimal,
    pub tax_amount: Decimal,
    pub shipping_cost: Decimal,
    pub other_cost: Decimal,
    pub total_amount: Decimal,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_terms: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_due_date: Option<NaiveDate>,
    pub status: PurchaseOrderStatus,
    pub requires_approval: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub submitted_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub submitted_by: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approved_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approved_by: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rejected_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rejected_by: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rejection_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cancelled_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cancelled_by: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cancellation_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<DateTime<Utc>>,
    pub attachments: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supplier_notes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub internal_notes: Option<String>,
    pub data: serde_json::Value,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl PurchaseOrder {
    /// Create a builder for PurchaseOrder
    pub fn builder() -> PurchaseOrderBuilder {
        PurchaseOrderBuilder::default()
    }

    /// Create a new PurchaseOrder with required fields
    pub fn new(provider_id: Uuid, po_number: String, po_type: PurchaseOrderType, supplier_name: String, warehouse_id: Uuid, order_date: NaiveDate, currency: String, subtotal: Decimal, discount_percent: Decimal, discount_amount: Decimal, tax_percent: Decimal, tax_amount: Decimal, shipping_cost: Decimal, other_cost: Decimal, total_amount: Decimal, status: PurchaseOrderStatus, requires_approval: bool, attachments: serde_json::Value, data: serde_json::Value) -> Self {
        Self {
            id: Uuid::new_v4(),
            provider_id,
            po_number,
            reference_number: None,
            po_type,
            supplier_id: None,
            supplier_name,
            supplier_contact_person: None,
            warehouse_id,
            delivery_address: None,
            order_date,
            expected_date: None,
            delivery_date: None,
            expiry_date: None,
            currency,
            subtotal,
            discount_percent,
            discount_amount,
            tax_percent,
            tax_amount,
            shipping_cost,
            other_cost,
            total_amount,
            payment_terms: None,
            payment_due_date: None,
            status,
            requires_approval,
            submitted_at: None,
            submitted_by: None,
            approved_at: None,
            approved_by: None,
            rejected_at: None,
            rejected_by: None,
            rejection_reason: None,
            cancelled_at: None,
            cancelled_by: None,
            cancellation_reason: None,
            completed_at: None,
            attachments,
            supplier_notes: None,
            internal_notes: None,
            data,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> PurchaseOrderId {
        PurchaseOrderId(self.id)
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
    pub fn status(&self) -> &PurchaseOrderStatus {
        &self.status
    }


    // ==========================================================
    // Fluent Setters (with_* for optional fields)
    // ==========================================================

    /// Set the reference_number field (chainable)
    pub fn with_reference_number(mut self, value: String) -> Self {
        self.reference_number = Some(value);
        self
    }

    /// Set the supplier_id field (chainable)
    pub fn with_supplier_id(mut self, value: Uuid) -> Self {
        self.supplier_id = Some(value);
        self
    }

    /// Set the supplier_contact_person field (chainable)
    pub fn with_supplier_contact_person(mut self, value: String) -> Self {
        self.supplier_contact_person = Some(value);
        self
    }

    /// Set the delivery_address field (chainable)
    pub fn with_delivery_address(mut self, value: String) -> Self {
        self.delivery_address = Some(value);
        self
    }

    /// Set the expected_date field (chainable)
    pub fn with_expected_date(mut self, value: NaiveDate) -> Self {
        self.expected_date = Some(value);
        self
    }

    /// Set the delivery_date field (chainable)
    pub fn with_delivery_date(mut self, value: NaiveDate) -> Self {
        self.delivery_date = Some(value);
        self
    }

    /// Set the expiry_date field (chainable)
    pub fn with_expiry_date(mut self, value: NaiveDate) -> Self {
        self.expiry_date = Some(value);
        self
    }

    /// Set the payment_terms field (chainable)
    pub fn with_payment_terms(mut self, value: String) -> Self {
        self.payment_terms = Some(value);
        self
    }

    /// Set the payment_due_date field (chainable)
    pub fn with_payment_due_date(mut self, value: NaiveDate) -> Self {
        self.payment_due_date = Some(value);
        self
    }

    /// Set the submitted_at field (chainable)
    pub fn with_submitted_at(mut self, value: DateTime<Utc>) -> Self {
        self.submitted_at = Some(value);
        self
    }

    /// Set the submitted_by field (chainable)
    pub fn with_submitted_by(mut self, value: Uuid) -> Self {
        self.submitted_by = Some(value);
        self
    }

    /// Set the approved_at field (chainable)
    pub fn with_approved_at(mut self, value: DateTime<Utc>) -> Self {
        self.approved_at = Some(value);
        self
    }

    /// Set the approved_by field (chainable)
    pub fn with_approved_by(mut self, value: Uuid) -> Self {
        self.approved_by = Some(value);
        self
    }

    /// Set the rejected_at field (chainable)
    pub fn with_rejected_at(mut self, value: DateTime<Utc>) -> Self {
        self.rejected_at = Some(value);
        self
    }

    /// Set the rejected_by field (chainable)
    pub fn with_rejected_by(mut self, value: Uuid) -> Self {
        self.rejected_by = Some(value);
        self
    }

    /// Set the rejection_reason field (chainable)
    pub fn with_rejection_reason(mut self, value: String) -> Self {
        self.rejection_reason = Some(value);
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

    /// Set the completed_at field (chainable)
    pub fn with_completed_at(mut self, value: DateTime<Utc>) -> Self {
        self.completed_at = Some(value);
        self
    }

    /// Set the supplier_notes field (chainable)
    pub fn with_supplier_notes(mut self, value: String) -> Self {
        self.supplier_notes = Some(value);
        self
    }

    /// Set the internal_notes field (chainable)
    pub fn with_internal_notes(mut self, value: String) -> Self {
        self.internal_notes = Some(value);
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
                "po_number" => {
                    if let Ok(v) = serde_json::from_value(value) { self.po_number = v; }
                }
                "reference_number" => {
                    if let Ok(v) = serde_json::from_value(value) { self.reference_number = v; }
                }
                "po_type" => {
                    if let Ok(v) = serde_json::from_value(value) { self.po_type = v; }
                }
                "supplier_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.supplier_id = v; }
                }
                "supplier_name" => {
                    if let Ok(v) = serde_json::from_value(value) { self.supplier_name = v; }
                }
                "supplier_contact_person" => {
                    if let Ok(v) = serde_json::from_value(value) { self.supplier_contact_person = v; }
                }
                "warehouse_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.warehouse_id = v; }
                }
                "delivery_address" => {
                    if let Ok(v) = serde_json::from_value(value) { self.delivery_address = v; }
                }
                "order_date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.order_date = v; }
                }
                "expected_date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.expected_date = v; }
                }
                "delivery_date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.delivery_date = v; }
                }
                "expiry_date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.expiry_date = v; }
                }
                "currency" => {
                    if let Ok(v) = serde_json::from_value(value) { self.currency = v; }
                }
                "subtotal" => {
                    if let Ok(v) = serde_json::from_value(value) { self.subtotal = v; }
                }
                "discount_percent" => {
                    if let Ok(v) = serde_json::from_value(value) { self.discount_percent = v; }
                }
                "discount_amount" => {
                    if let Ok(v) = serde_json::from_value(value) { self.discount_amount = v; }
                }
                "tax_percent" => {
                    if let Ok(v) = serde_json::from_value(value) { self.tax_percent = v; }
                }
                "tax_amount" => {
                    if let Ok(v) = serde_json::from_value(value) { self.tax_amount = v; }
                }
                "shipping_cost" => {
                    if let Ok(v) = serde_json::from_value(value) { self.shipping_cost = v; }
                }
                "other_cost" => {
                    if let Ok(v) = serde_json::from_value(value) { self.other_cost = v; }
                }
                "total_amount" => {
                    if let Ok(v) = serde_json::from_value(value) { self.total_amount = v; }
                }
                "payment_terms" => {
                    if let Ok(v) = serde_json::from_value(value) { self.payment_terms = v; }
                }
                "payment_due_date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.payment_due_date = v; }
                }
                "status" => {
                    if let Ok(v) = serde_json::from_value(value) { self.status = v; }
                }
                "requires_approval" => {
                    if let Ok(v) = serde_json::from_value(value) { self.requires_approval = v; }
                }
                "submitted_at" => {
                    if let Ok(v) = serde_json::from_value(value) { self.submitted_at = v; }
                }
                "submitted_by" => {
                    if let Ok(v) = serde_json::from_value(value) { self.submitted_by = v; }
                }
                "approved_at" => {
                    if let Ok(v) = serde_json::from_value(value) { self.approved_at = v; }
                }
                "approved_by" => {
                    if let Ok(v) = serde_json::from_value(value) { self.approved_by = v; }
                }
                "rejected_at" => {
                    if let Ok(v) = serde_json::from_value(value) { self.rejected_at = v; }
                }
                "rejected_by" => {
                    if let Ok(v) = serde_json::from_value(value) { self.rejected_by = v; }
                }
                "rejection_reason" => {
                    if let Ok(v) = serde_json::from_value(value) { self.rejection_reason = v; }
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
                "completed_at" => {
                    if let Ok(v) = serde_json::from_value(value) { self.completed_at = v; }
                }
                "attachments" => {
                    if let Ok(v) = serde_json::from_value(value) { self.attachments = v; }
                }
                "supplier_notes" => {
                    if let Ok(v) = serde_json::from_value(value) { self.supplier_notes = v; }
                }
                "internal_notes" => {
                    if let Ok(v) = serde_json::from_value(value) { self.internal_notes = v; }
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

impl super::Entity for PurchaseOrder {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "PurchaseOrder"
    }
}

impl backbone_core::PersistentEntity for PurchaseOrder {
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

impl backbone_orm::EntityRepoMeta for PurchaseOrder {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("provider_id".to_string(), "uuid".to_string());
        m.insert("supplier_id".to_string(), "uuid".to_string());
        m.insert("warehouse_id".to_string(), "uuid".to_string());
        m.insert("po_type".to_string(), "purchase_order_type".to_string());
        m.insert("status".to_string(), "purchase_order_status".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["po_number", "supplier_name", "currency"]
    }
}

/// Builder for PurchaseOrder entity
///
/// Provides a fluent API for constructing PurchaseOrder instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct PurchaseOrderBuilder {
    provider_id: Option<Uuid>,
    po_number: Option<String>,
    reference_number: Option<String>,
    po_type: Option<PurchaseOrderType>,
    supplier_id: Option<Uuid>,
    supplier_name: Option<String>,
    supplier_contact_person: Option<String>,
    warehouse_id: Option<Uuid>,
    delivery_address: Option<String>,
    order_date: Option<NaiveDate>,
    expected_date: Option<NaiveDate>,
    delivery_date: Option<NaiveDate>,
    expiry_date: Option<NaiveDate>,
    currency: Option<String>,
    subtotal: Option<Decimal>,
    discount_percent: Option<Decimal>,
    discount_amount: Option<Decimal>,
    tax_percent: Option<Decimal>,
    tax_amount: Option<Decimal>,
    shipping_cost: Option<Decimal>,
    other_cost: Option<Decimal>,
    total_amount: Option<Decimal>,
    payment_terms: Option<String>,
    payment_due_date: Option<NaiveDate>,
    status: Option<PurchaseOrderStatus>,
    requires_approval: Option<bool>,
    submitted_at: Option<DateTime<Utc>>,
    submitted_by: Option<Uuid>,
    approved_at: Option<DateTime<Utc>>,
    approved_by: Option<Uuid>,
    rejected_at: Option<DateTime<Utc>>,
    rejected_by: Option<Uuid>,
    rejection_reason: Option<String>,
    cancelled_at: Option<DateTime<Utc>>,
    cancelled_by: Option<Uuid>,
    cancellation_reason: Option<String>,
    completed_at: Option<DateTime<Utc>>,
    attachments: Option<serde_json::Value>,
    supplier_notes: Option<String>,
    internal_notes: Option<String>,
    data: Option<serde_json::Value>,
}

impl PurchaseOrderBuilder {
    /// Set the provider_id field (required)
    pub fn provider_id(mut self, value: Uuid) -> Self {
        self.provider_id = Some(value);
        self
    }

    /// Set the po_number field (required)
    pub fn po_number(mut self, value: String) -> Self {
        self.po_number = Some(value);
        self
    }

    /// Set the reference_number field (optional)
    pub fn reference_number(mut self, value: String) -> Self {
        self.reference_number = Some(value);
        self
    }

    /// Set the po_type field (default: `PurchaseOrderType::default()`)
    pub fn po_type(mut self, value: PurchaseOrderType) -> Self {
        self.po_type = Some(value);
        self
    }

    /// Set the supplier_id field (optional)
    pub fn supplier_id(mut self, value: Uuid) -> Self {
        self.supplier_id = Some(value);
        self
    }

    /// Set the supplier_name field (required)
    pub fn supplier_name(mut self, value: String) -> Self {
        self.supplier_name = Some(value);
        self
    }

    /// Set the supplier_contact_person field (optional)
    pub fn supplier_contact_person(mut self, value: String) -> Self {
        self.supplier_contact_person = Some(value);
        self
    }

    /// Set the warehouse_id field (required)
    pub fn warehouse_id(mut self, value: Uuid) -> Self {
        self.warehouse_id = Some(value);
        self
    }

    /// Set the delivery_address field (optional)
    pub fn delivery_address(mut self, value: String) -> Self {
        self.delivery_address = Some(value);
        self
    }

    /// Set the order_date field (required)
    pub fn order_date(mut self, value: NaiveDate) -> Self {
        self.order_date = Some(value);
        self
    }

    /// Set the expected_date field (optional)
    pub fn expected_date(mut self, value: NaiveDate) -> Self {
        self.expected_date = Some(value);
        self
    }

    /// Set the delivery_date field (optional)
    pub fn delivery_date(mut self, value: NaiveDate) -> Self {
        self.delivery_date = Some(value);
        self
    }

    /// Set the expiry_date field (optional)
    pub fn expiry_date(mut self, value: NaiveDate) -> Self {
        self.expiry_date = Some(value);
        self
    }

    /// Set the currency field (default: `"IDR".to_string()`)
    pub fn currency(mut self, value: String) -> Self {
        self.currency = Some(value);
        self
    }

    /// Set the subtotal field (default: `Decimal::from(0)`)
    pub fn subtotal(mut self, value: Decimal) -> Self {
        self.subtotal = Some(value);
        self
    }

    /// Set the discount_percent field (default: `Decimal::from(0)`)
    pub fn discount_percent(mut self, value: Decimal) -> Self {
        self.discount_percent = Some(value);
        self
    }

    /// Set the discount_amount field (default: `Decimal::from(0)`)
    pub fn discount_amount(mut self, value: Decimal) -> Self {
        self.discount_amount = Some(value);
        self
    }

    /// Set the tax_percent field (default: `Decimal::from(0)`)
    pub fn tax_percent(mut self, value: Decimal) -> Self {
        self.tax_percent = Some(value);
        self
    }

    /// Set the tax_amount field (default: `Decimal::from(0)`)
    pub fn tax_amount(mut self, value: Decimal) -> Self {
        self.tax_amount = Some(value);
        self
    }

    /// Set the shipping_cost field (default: `Decimal::from(0)`)
    pub fn shipping_cost(mut self, value: Decimal) -> Self {
        self.shipping_cost = Some(value);
        self
    }

    /// Set the other_cost field (default: `Decimal::from(0)`)
    pub fn other_cost(mut self, value: Decimal) -> Self {
        self.other_cost = Some(value);
        self
    }

    /// Set the total_amount field (default: `Decimal::from(0)`)
    pub fn total_amount(mut self, value: Decimal) -> Self {
        self.total_amount = Some(value);
        self
    }

    /// Set the payment_terms field (optional)
    pub fn payment_terms(mut self, value: String) -> Self {
        self.payment_terms = Some(value);
        self
    }

    /// Set the payment_due_date field (optional)
    pub fn payment_due_date(mut self, value: NaiveDate) -> Self {
        self.payment_due_date = Some(value);
        self
    }

    /// Set the status field (default: `PurchaseOrderStatus::default()`)
    pub fn status(mut self, value: PurchaseOrderStatus) -> Self {
        self.status = Some(value);
        self
    }

    /// Set the requires_approval field (default: `true`)
    pub fn requires_approval(mut self, value: bool) -> Self {
        self.requires_approval = Some(value);
        self
    }

    /// Set the submitted_at field (optional)
    pub fn submitted_at(mut self, value: DateTime<Utc>) -> Self {
        self.submitted_at = Some(value);
        self
    }

    /// Set the submitted_by field (optional)
    pub fn submitted_by(mut self, value: Uuid) -> Self {
        self.submitted_by = Some(value);
        self
    }

    /// Set the approved_at field (optional)
    pub fn approved_at(mut self, value: DateTime<Utc>) -> Self {
        self.approved_at = Some(value);
        self
    }

    /// Set the approved_by field (optional)
    pub fn approved_by(mut self, value: Uuid) -> Self {
        self.approved_by = Some(value);
        self
    }

    /// Set the rejected_at field (optional)
    pub fn rejected_at(mut self, value: DateTime<Utc>) -> Self {
        self.rejected_at = Some(value);
        self
    }

    /// Set the rejected_by field (optional)
    pub fn rejected_by(mut self, value: Uuid) -> Self {
        self.rejected_by = Some(value);
        self
    }

    /// Set the rejection_reason field (optional)
    pub fn rejection_reason(mut self, value: String) -> Self {
        self.rejection_reason = Some(value);
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

    /// Set the completed_at field (optional)
    pub fn completed_at(mut self, value: DateTime<Utc>) -> Self {
        self.completed_at = Some(value);
        self
    }

    /// Set the attachments field (default: `serde_json::json!([])`)
    pub fn attachments(mut self, value: serde_json::Value) -> Self {
        self.attachments = Some(value);
        self
    }

    /// Set the supplier_notes field (optional)
    pub fn supplier_notes(mut self, value: String) -> Self {
        self.supplier_notes = Some(value);
        self
    }

    /// Set the internal_notes field (optional)
    pub fn internal_notes(mut self, value: String) -> Self {
        self.internal_notes = Some(value);
        self
    }

    /// Set the data field (default: `serde_json::json!({"total_items":0,"total_quantity":0,"quantity_received":0,"quantity_outstanding":0,"is_fully_received":false,"receipt_count":0})`)
    pub fn data(mut self, value: serde_json::Value) -> Self {
        self.data = Some(value);
        self
    }

    /// Build the PurchaseOrder entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<PurchaseOrder, String> {
        let provider_id = self.provider_id.ok_or_else(|| "provider_id is required".to_string())?;
        let po_number = self.po_number.ok_or_else(|| "po_number is required".to_string())?;
        let supplier_name = self.supplier_name.ok_or_else(|| "supplier_name is required".to_string())?;
        let warehouse_id = self.warehouse_id.ok_or_else(|| "warehouse_id is required".to_string())?;
        let order_date = self.order_date.ok_or_else(|| "order_date is required".to_string())?;

        Ok(PurchaseOrder {
            id: Uuid::new_v4(),
            provider_id,
            po_number,
            reference_number: self.reference_number,
            po_type: self.po_type.unwrap_or(PurchaseOrderType::default()),
            supplier_id: self.supplier_id,
            supplier_name,
            supplier_contact_person: self.supplier_contact_person,
            warehouse_id,
            delivery_address: self.delivery_address,
            order_date,
            expected_date: self.expected_date,
            delivery_date: self.delivery_date,
            expiry_date: self.expiry_date,
            currency: self.currency.unwrap_or("IDR".to_string()),
            subtotal: self.subtotal.unwrap_or(Decimal::from(0)),
            discount_percent: self.discount_percent.unwrap_or(Decimal::from(0)),
            discount_amount: self.discount_amount.unwrap_or(Decimal::from(0)),
            tax_percent: self.tax_percent.unwrap_or(Decimal::from(0)),
            tax_amount: self.tax_amount.unwrap_or(Decimal::from(0)),
            shipping_cost: self.shipping_cost.unwrap_or(Decimal::from(0)),
            other_cost: self.other_cost.unwrap_or(Decimal::from(0)),
            total_amount: self.total_amount.unwrap_or(Decimal::from(0)),
            payment_terms: self.payment_terms,
            payment_due_date: self.payment_due_date,
            status: self.status.unwrap_or(PurchaseOrderStatus::default()),
            requires_approval: self.requires_approval.unwrap_or(true),
            submitted_at: self.submitted_at,
            submitted_by: self.submitted_by,
            approved_at: self.approved_at,
            approved_by: self.approved_by,
            rejected_at: self.rejected_at,
            rejected_by: self.rejected_by,
            rejection_reason: self.rejection_reason,
            cancelled_at: self.cancelled_at,
            cancelled_by: self.cancelled_by,
            cancellation_reason: self.cancellation_reason,
            completed_at: self.completed_at,
            attachments: self.attachments.unwrap_or(serde_json::json!([])),
            supplier_notes: self.supplier_notes,
            internal_notes: self.internal_notes,
            data: self.data.unwrap_or(serde_json::json!({"total_items":0,"total_quantity":0,"quantity_received":0,"quantity_outstanding":0,"is_fully_received":false,"receipt_count":0})),
            metadata: AuditMetadata::default(),
        })
    }
}
