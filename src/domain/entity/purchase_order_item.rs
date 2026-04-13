use chrono::{DateTime, Utc, NaiveDate};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use rust_decimal::Decimal;

use super::UnitOfMeasure;
use super::AuditMetadata;

/// Strongly-typed ID for PurchaseOrderItem
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PurchaseOrderItemId(pub Uuid);

impl PurchaseOrderItemId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for PurchaseOrderItemId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for PurchaseOrderItemId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for PurchaseOrderItemId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<PurchaseOrderItemId> for Uuid {
    fn from(id: PurchaseOrderItemId) -> Self { id.0 }
}

impl AsRef<Uuid> for PurchaseOrderItemId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for PurchaseOrderItemId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PurchaseOrderItem {
    pub id: Uuid,
    pub purchase_order_id: Uuid,
    pub provider_id: Uuid,
    pub line_number: i32,
    pub product_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub uom: UnitOfMeasure,
    pub quantity_ordered: Decimal,
    pub base_uom: UnitOfMeasure,
    pub base_quantity: Decimal,
    pub currency: String,
    pub unit_price: Decimal,
    pub discount_percent: Decimal,
    pub discount_amount: Decimal,
    pub subtotal: Decimal,
    pub tax_percent: Decimal,
    pub tax_amount: Decimal,
    pub total_amount: Decimal,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_date: Option<NaiveDate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warehouse_id: Option<Uuid>,
    pub is_cancelled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cancellation_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    pub data: serde_json::Value,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl PurchaseOrderItem {
    /// Create a builder for PurchaseOrderItem
    pub fn builder() -> PurchaseOrderItemBuilder {
        PurchaseOrderItemBuilder::default()
    }

    /// Create a new PurchaseOrderItem with required fields
    pub fn new(purchase_order_id: Uuid, provider_id: Uuid, line_number: i32, product_id: Uuid, uom: UnitOfMeasure, quantity_ordered: Decimal, base_uom: UnitOfMeasure, base_quantity: Decimal, currency: String, unit_price: Decimal, discount_percent: Decimal, discount_amount: Decimal, subtotal: Decimal, tax_percent: Decimal, tax_amount: Decimal, total_amount: Decimal, is_cancelled: bool, data: serde_json::Value) -> Self {
        Self {
            id: Uuid::new_v4(),
            purchase_order_id,
            provider_id,
            line_number,
            product_id,
            description: None,
            uom,
            quantity_ordered,
            base_uom,
            base_quantity,
            currency,
            unit_price,
            discount_percent,
            discount_amount,
            subtotal,
            tax_percent,
            tax_amount,
            total_amount,
            expected_date: None,
            warehouse_id: None,
            is_cancelled,
            cancellation_reason: None,
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
    pub fn typed_id(&self) -> PurchaseOrderItemId {
        PurchaseOrderItemId(self.id)
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

    /// Set the description field (chainable)
    pub fn with_description(mut self, value: String) -> Self {
        self.description = Some(value);
        self
    }

    /// Set the expected_date field (chainable)
    pub fn with_expected_date(mut self, value: NaiveDate) -> Self {
        self.expected_date = Some(value);
        self
    }

    /// Set the warehouse_id field (chainable)
    pub fn with_warehouse_id(mut self, value: Uuid) -> Self {
        self.warehouse_id = Some(value);
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
                "purchase_order_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.purchase_order_id = v; }
                }
                "provider_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.provider_id = v; }
                }
                "line_number" => {
                    if let Ok(v) = serde_json::from_value(value) { self.line_number = v; }
                }
                "product_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.product_id = v; }
                }
                "description" => {
                    if let Ok(v) = serde_json::from_value(value) { self.description = v; }
                }
                "uom" => {
                    if let Ok(v) = serde_json::from_value(value) { self.uom = v; }
                }
                "quantity_ordered" => {
                    if let Ok(v) = serde_json::from_value(value) { self.quantity_ordered = v; }
                }
                "base_uom" => {
                    if let Ok(v) = serde_json::from_value(value) { self.base_uom = v; }
                }
                "base_quantity" => {
                    if let Ok(v) = serde_json::from_value(value) { self.base_quantity = v; }
                }
                "currency" => {
                    if let Ok(v) = serde_json::from_value(value) { self.currency = v; }
                }
                "unit_price" => {
                    if let Ok(v) = serde_json::from_value(value) { self.unit_price = v; }
                }
                "discount_percent" => {
                    if let Ok(v) = serde_json::from_value(value) { self.discount_percent = v; }
                }
                "discount_amount" => {
                    if let Ok(v) = serde_json::from_value(value) { self.discount_amount = v; }
                }
                "subtotal" => {
                    if let Ok(v) = serde_json::from_value(value) { self.subtotal = v; }
                }
                "tax_percent" => {
                    if let Ok(v) = serde_json::from_value(value) { self.tax_percent = v; }
                }
                "tax_amount" => {
                    if let Ok(v) = serde_json::from_value(value) { self.tax_amount = v; }
                }
                "total_amount" => {
                    if let Ok(v) = serde_json::from_value(value) { self.total_amount = v; }
                }
                "expected_date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.expected_date = v; }
                }
                "warehouse_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.warehouse_id = v; }
                }
                "is_cancelled" => {
                    if let Ok(v) = serde_json::from_value(value) { self.is_cancelled = v; }
                }
                "cancellation_reason" => {
                    if let Ok(v) = serde_json::from_value(value) { self.cancellation_reason = v; }
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

impl super::Entity for PurchaseOrderItem {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "PurchaseOrderItem"
    }
}

impl backbone_core::PersistentEntity for PurchaseOrderItem {
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

impl backbone_orm::EntityRepoMeta for PurchaseOrderItem {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("purchase_order_id".to_string(), "uuid".to_string());
        m.insert("provider_id".to_string(), "uuid".to_string());
        m.insert("product_id".to_string(), "uuid".to_string());
        m.insert("warehouse_id".to_string(), "uuid".to_string());
        m.insert("uom".to_string(), "unit_of_measure".to_string());
        m.insert("base_uom".to_string(), "unit_of_measure".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["currency"]
    }
}

/// Builder for PurchaseOrderItem entity
///
/// Provides a fluent API for constructing PurchaseOrderItem instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct PurchaseOrderItemBuilder {
    purchase_order_id: Option<Uuid>,
    provider_id: Option<Uuid>,
    line_number: Option<i32>,
    product_id: Option<Uuid>,
    description: Option<String>,
    uom: Option<UnitOfMeasure>,
    quantity_ordered: Option<Decimal>,
    base_uom: Option<UnitOfMeasure>,
    base_quantity: Option<Decimal>,
    currency: Option<String>,
    unit_price: Option<Decimal>,
    discount_percent: Option<Decimal>,
    discount_amount: Option<Decimal>,
    subtotal: Option<Decimal>,
    tax_percent: Option<Decimal>,
    tax_amount: Option<Decimal>,
    total_amount: Option<Decimal>,
    expected_date: Option<NaiveDate>,
    warehouse_id: Option<Uuid>,
    is_cancelled: Option<bool>,
    cancellation_reason: Option<String>,
    notes: Option<String>,
    data: Option<serde_json::Value>,
}

impl PurchaseOrderItemBuilder {
    /// Set the purchase_order_id field (required)
    pub fn purchase_order_id(mut self, value: Uuid) -> Self {
        self.purchase_order_id = Some(value);
        self
    }

    /// Set the provider_id field (required)
    pub fn provider_id(mut self, value: Uuid) -> Self {
        self.provider_id = Some(value);
        self
    }

    /// Set the line_number field (required)
    pub fn line_number(mut self, value: i32) -> Self {
        self.line_number = Some(value);
        self
    }

    /// Set the product_id field (required)
    pub fn product_id(mut self, value: Uuid) -> Self {
        self.product_id = Some(value);
        self
    }

    /// Set the description field (optional)
    pub fn description(mut self, value: String) -> Self {
        self.description = Some(value);
        self
    }

    /// Set the uom field (required)
    pub fn uom(mut self, value: UnitOfMeasure) -> Self {
        self.uom = Some(value);
        self
    }

    /// Set the quantity_ordered field (required)
    pub fn quantity_ordered(mut self, value: Decimal) -> Self {
        self.quantity_ordered = Some(value);
        self
    }

    /// Set the base_uom field (required)
    pub fn base_uom(mut self, value: UnitOfMeasure) -> Self {
        self.base_uom = Some(value);
        self
    }

    /// Set the base_quantity field (required)
    pub fn base_quantity(mut self, value: Decimal) -> Self {
        self.base_quantity = Some(value);
        self
    }

    /// Set the currency field (default: `"IDR".to_string()`)
    pub fn currency(mut self, value: String) -> Self {
        self.currency = Some(value);
        self
    }

    /// Set the unit_price field (required)
    pub fn unit_price(mut self, value: Decimal) -> Self {
        self.unit_price = Some(value);
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

    /// Set the subtotal field (required)
    pub fn subtotal(mut self, value: Decimal) -> Self {
        self.subtotal = Some(value);
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

    /// Set the total_amount field (required)
    pub fn total_amount(mut self, value: Decimal) -> Self {
        self.total_amount = Some(value);
        self
    }

    /// Set the expected_date field (optional)
    pub fn expected_date(mut self, value: NaiveDate) -> Self {
        self.expected_date = Some(value);
        self
    }

    /// Set the warehouse_id field (optional)
    pub fn warehouse_id(mut self, value: Uuid) -> Self {
        self.warehouse_id = Some(value);
        self
    }

    /// Set the is_cancelled field (default: `false`)
    pub fn is_cancelled(mut self, value: bool) -> Self {
        self.is_cancelled = Some(value);
        self
    }

    /// Set the cancellation_reason field (optional)
    pub fn cancellation_reason(mut self, value: String) -> Self {
        self.cancellation_reason = Some(value);
        self
    }

    /// Set the notes field (optional)
    pub fn notes(mut self, value: String) -> Self {
        self.notes = Some(value);
        self
    }

    /// Set the data field (default: `serde_json::json!({"sku":null,"product_name":null,"supplier_sku":null,"quantity_received":0,"quantity_outstanding":0,"quantity_rejected":0,"is_fully_received":false})`)
    pub fn data(mut self, value: serde_json::Value) -> Self {
        self.data = Some(value);
        self
    }

    /// Build the PurchaseOrderItem entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<PurchaseOrderItem, String> {
        let purchase_order_id = self.purchase_order_id.ok_or_else(|| "purchase_order_id is required".to_string())?;
        let provider_id = self.provider_id.ok_or_else(|| "provider_id is required".to_string())?;
        let line_number = self.line_number.ok_or_else(|| "line_number is required".to_string())?;
        let product_id = self.product_id.ok_or_else(|| "product_id is required".to_string())?;
        let uom = self.uom.ok_or_else(|| "uom is required".to_string())?;
        let quantity_ordered = self.quantity_ordered.ok_or_else(|| "quantity_ordered is required".to_string())?;
        let base_uom = self.base_uom.ok_or_else(|| "base_uom is required".to_string())?;
        let base_quantity = self.base_quantity.ok_or_else(|| "base_quantity is required".to_string())?;
        let unit_price = self.unit_price.ok_or_else(|| "unit_price is required".to_string())?;
        let subtotal = self.subtotal.ok_or_else(|| "subtotal is required".to_string())?;
        let total_amount = self.total_amount.ok_or_else(|| "total_amount is required".to_string())?;

        Ok(PurchaseOrderItem {
            id: Uuid::new_v4(),
            purchase_order_id,
            provider_id,
            line_number,
            product_id,
            description: self.description,
            uom,
            quantity_ordered,
            base_uom,
            base_quantity,
            currency: self.currency.unwrap_or("IDR".to_string()),
            unit_price,
            discount_percent: self.discount_percent.unwrap_or(Decimal::from(0)),
            discount_amount: self.discount_amount.unwrap_or(Decimal::from(0)),
            subtotal,
            tax_percent: self.tax_percent.unwrap_or(Decimal::from(0)),
            tax_amount: self.tax_amount.unwrap_or(Decimal::from(0)),
            total_amount,
            expected_date: self.expected_date,
            warehouse_id: self.warehouse_id,
            is_cancelled: self.is_cancelled.unwrap_or(false),
            cancellation_reason: self.cancellation_reason,
            notes: self.notes,
            data: self.data.unwrap_or(serde_json::json!({"sku":null,"product_name":null,"supplier_sku":null,"quantity_received":0,"quantity_outstanding":0,"quantity_rejected":0,"is_fully_received":false})),
            metadata: AuditMetadata::default(),
        })
    }
}
