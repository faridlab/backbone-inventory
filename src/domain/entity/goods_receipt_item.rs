use chrono::{DateTime, Utc, NaiveDate};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use rust_decimal::Decimal;

use super::QCResult;
use super::ReceiptItemStatus;
use super::AuditMetadata;

/// Strongly-typed ID for GoodsReceiptItem
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GoodsReceiptItemId(pub Uuid);

impl GoodsReceiptItemId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for GoodsReceiptItemId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for GoodsReceiptItemId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for GoodsReceiptItemId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<GoodsReceiptItemId> for Uuid {
    fn from(id: GoodsReceiptItemId) -> Self { id.0 }
}

impl AsRef<Uuid> for GoodsReceiptItemId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for GoodsReceiptItemId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct GoodsReceiptItem {
    pub id: Uuid,
    pub goods_receipt_id: Uuid,
    pub product_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub purchase_order_item_id: Option<Uuid>,
    pub ordered_quantity: Decimal,
    pub received_quantity: Decimal,
    pub accepted_quantity: Decimal,
    pub rejected_quantity: Decimal,
    pub variance_quantity: Decimal,
    pub uom: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_uom_quantity: Option<Decimal>,
    pub conversion_factor: Decimal,
    pub unit_cost: Decimal,
    pub total_cost: Decimal,
    pub currency: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub batch_number: Option<String>,
    pub serial_numbers: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manufacture_date: Option<NaiveDate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiry_date: Option<NaiveDate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warehouse_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bin_location: Option<String>,
    pub requires_qc: bool,
    pub qc_result: QCResult,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qc_date: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qc_by: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qc_notes: Option<String>,
    pub qc_parameters: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rejection_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rejection_action: Option<String>,
    pub status: ReceiptItemStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stock_movement_id: Option<Uuid>,
    pub is_posted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub posted_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl GoodsReceiptItem {
    /// Create a builder for GoodsReceiptItem
    pub fn builder() -> GoodsReceiptItemBuilder {
        GoodsReceiptItemBuilder::default()
    }

    /// Create a new GoodsReceiptItem with required fields
    pub fn new(goods_receipt_id: Uuid, product_id: Uuid, ordered_quantity: Decimal, received_quantity: Decimal, accepted_quantity: Decimal, rejected_quantity: Decimal, variance_quantity: Decimal, uom: String, conversion_factor: Decimal, unit_cost: Decimal, total_cost: Decimal, currency: String, serial_numbers: serde_json::Value, requires_qc: bool, qc_result: QCResult, qc_parameters: serde_json::Value, status: ReceiptItemStatus, is_posted: bool) -> Self {
        Self {
            id: Uuid::new_v4(),
            goods_receipt_id,
            product_id,
            purchase_order_item_id: None,
            ordered_quantity,
            received_quantity,
            accepted_quantity,
            rejected_quantity,
            variance_quantity,
            uom,
            base_uom_quantity: None,
            conversion_factor,
            unit_cost,
            total_cost,
            currency,
            batch_number: None,
            serial_numbers,
            manufacture_date: None,
            expiry_date: None,
            warehouse_id: None,
            bin_location: None,
            requires_qc,
            qc_result,
            qc_date: None,
            qc_by: None,
            qc_notes: None,
            qc_parameters,
            rejection_reason: None,
            rejection_action: None,
            status,
            stock_movement_id: None,
            is_posted,
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
    pub fn typed_id(&self) -> GoodsReceiptItemId {
        GoodsReceiptItemId(self.id)
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
    pub fn status(&self) -> &ReceiptItemStatus {
        &self.status
    }


    // ==========================================================
    // Fluent Setters (with_* for optional fields)
    // ==========================================================

    /// Set the purchase_order_item_id field (chainable)
    pub fn with_purchase_order_item_id(mut self, value: Uuid) -> Self {
        self.purchase_order_item_id = Some(value);
        self
    }

    /// Set the base_uom_quantity field (chainable)
    pub fn with_base_uom_quantity(mut self, value: Decimal) -> Self {
        self.base_uom_quantity = Some(value);
        self
    }

    /// Set the batch_number field (chainable)
    pub fn with_batch_number(mut self, value: String) -> Self {
        self.batch_number = Some(value);
        self
    }

    /// Set the manufacture_date field (chainable)
    pub fn with_manufacture_date(mut self, value: NaiveDate) -> Self {
        self.manufacture_date = Some(value);
        self
    }

    /// Set the expiry_date field (chainable)
    pub fn with_expiry_date(mut self, value: NaiveDate) -> Self {
        self.expiry_date = Some(value);
        self
    }

    /// Set the warehouse_id field (chainable)
    pub fn with_warehouse_id(mut self, value: Uuid) -> Self {
        self.warehouse_id = Some(value);
        self
    }

    /// Set the bin_location field (chainable)
    pub fn with_bin_location(mut self, value: String) -> Self {
        self.bin_location = Some(value);
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

    /// Set the rejection_reason field (chainable)
    pub fn with_rejection_reason(mut self, value: String) -> Self {
        self.rejection_reason = Some(value);
        self
    }

    /// Set the rejection_action field (chainable)
    pub fn with_rejection_action(mut self, value: String) -> Self {
        self.rejection_action = Some(value);
        self
    }

    /// Set the stock_movement_id field (chainable)
    pub fn with_stock_movement_id(mut self, value: Uuid) -> Self {
        self.stock_movement_id = Some(value);
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
                "goods_receipt_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.goods_receipt_id = v; }
                }
                "product_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.product_id = v; }
                }
                "purchase_order_item_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.purchase_order_item_id = v; }
                }
                "ordered_quantity" => {
                    if let Ok(v) = serde_json::from_value(value) { self.ordered_quantity = v; }
                }
                "received_quantity" => {
                    if let Ok(v) = serde_json::from_value(value) { self.received_quantity = v; }
                }
                "accepted_quantity" => {
                    if let Ok(v) = serde_json::from_value(value) { self.accepted_quantity = v; }
                }
                "rejected_quantity" => {
                    if let Ok(v) = serde_json::from_value(value) { self.rejected_quantity = v; }
                }
                "variance_quantity" => {
                    if let Ok(v) = serde_json::from_value(value) { self.variance_quantity = v; }
                }
                "uom" => {
                    if let Ok(v) = serde_json::from_value(value) { self.uom = v; }
                }
                "base_uom_quantity" => {
                    if let Ok(v) = serde_json::from_value(value) { self.base_uom_quantity = v; }
                }
                "conversion_factor" => {
                    if let Ok(v) = serde_json::from_value(value) { self.conversion_factor = v; }
                }
                "unit_cost" => {
                    if let Ok(v) = serde_json::from_value(value) { self.unit_cost = v; }
                }
                "total_cost" => {
                    if let Ok(v) = serde_json::from_value(value) { self.total_cost = v; }
                }
                "currency" => {
                    if let Ok(v) = serde_json::from_value(value) { self.currency = v; }
                }
                "batch_number" => {
                    if let Ok(v) = serde_json::from_value(value) { self.batch_number = v; }
                }
                "serial_numbers" => {
                    if let Ok(v) = serde_json::from_value(value) { self.serial_numbers = v; }
                }
                "manufacture_date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.manufacture_date = v; }
                }
                "expiry_date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.expiry_date = v; }
                }
                "warehouse_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.warehouse_id = v; }
                }
                "bin_location" => {
                    if let Ok(v) = serde_json::from_value(value) { self.bin_location = v; }
                }
                "requires_qc" => {
                    if let Ok(v) = serde_json::from_value(value) { self.requires_qc = v; }
                }
                "qc_result" => {
                    if let Ok(v) = serde_json::from_value(value) { self.qc_result = v; }
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
                "qc_parameters" => {
                    if let Ok(v) = serde_json::from_value(value) { self.qc_parameters = v; }
                }
                "rejection_reason" => {
                    if let Ok(v) = serde_json::from_value(value) { self.rejection_reason = v; }
                }
                "rejection_action" => {
                    if let Ok(v) = serde_json::from_value(value) { self.rejection_action = v; }
                }
                "status" => {
                    if let Ok(v) = serde_json::from_value(value) { self.status = v; }
                }
                "stock_movement_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.stock_movement_id = v; }
                }
                "is_posted" => {
                    if let Ok(v) = serde_json::from_value(value) { self.is_posted = v; }
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

impl super::Entity for GoodsReceiptItem {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "GoodsReceiptItem"
    }
}

impl backbone_core::PersistentEntity for GoodsReceiptItem {
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

impl backbone_orm::EntityRepoMeta for GoodsReceiptItem {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("goods_receipt_id".to_string(), "uuid".to_string());
        m.insert("product_id".to_string(), "uuid".to_string());
        m.insert("purchase_order_item_id".to_string(), "uuid".to_string());
        m.insert("warehouse_id".to_string(), "uuid".to_string());
        m.insert("stock_movement_id".to_string(), "uuid".to_string());
        m.insert("qc_result".to_string(), "qc_result".to_string());
        m.insert("status".to_string(), "receipt_item_status".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["uom", "currency"]
    }
}

/// Builder for GoodsReceiptItem entity
///
/// Provides a fluent API for constructing GoodsReceiptItem instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct GoodsReceiptItemBuilder {
    goods_receipt_id: Option<Uuid>,
    product_id: Option<Uuid>,
    purchase_order_item_id: Option<Uuid>,
    ordered_quantity: Option<Decimal>,
    received_quantity: Option<Decimal>,
    accepted_quantity: Option<Decimal>,
    rejected_quantity: Option<Decimal>,
    variance_quantity: Option<Decimal>,
    uom: Option<String>,
    base_uom_quantity: Option<Decimal>,
    conversion_factor: Option<Decimal>,
    unit_cost: Option<Decimal>,
    total_cost: Option<Decimal>,
    currency: Option<String>,
    batch_number: Option<String>,
    serial_numbers: Option<serde_json::Value>,
    manufacture_date: Option<NaiveDate>,
    expiry_date: Option<NaiveDate>,
    warehouse_id: Option<Uuid>,
    bin_location: Option<String>,
    requires_qc: Option<bool>,
    qc_result: Option<QCResult>,
    qc_date: Option<DateTime<Utc>>,
    qc_by: Option<Uuid>,
    qc_notes: Option<String>,
    qc_parameters: Option<serde_json::Value>,
    rejection_reason: Option<String>,
    rejection_action: Option<String>,
    status: Option<ReceiptItemStatus>,
    stock_movement_id: Option<Uuid>,
    is_posted: Option<bool>,
    posted_at: Option<DateTime<Utc>>,
    notes: Option<String>,
}

impl GoodsReceiptItemBuilder {
    /// Set the goods_receipt_id field (required)
    pub fn goods_receipt_id(mut self, value: Uuid) -> Self {
        self.goods_receipt_id = Some(value);
        self
    }

    /// Set the product_id field (required)
    pub fn product_id(mut self, value: Uuid) -> Self {
        self.product_id = Some(value);
        self
    }

    /// Set the purchase_order_item_id field (optional)
    pub fn purchase_order_item_id(mut self, value: Uuid) -> Self {
        self.purchase_order_item_id = Some(value);
        self
    }

    /// Set the ordered_quantity field (required)
    pub fn ordered_quantity(mut self, value: Decimal) -> Self {
        self.ordered_quantity = Some(value);
        self
    }

    /// Set the received_quantity field (required)
    pub fn received_quantity(mut self, value: Decimal) -> Self {
        self.received_quantity = Some(value);
        self
    }

    /// Set the accepted_quantity field (default: `Decimal::from(0)`)
    pub fn accepted_quantity(mut self, value: Decimal) -> Self {
        self.accepted_quantity = Some(value);
        self
    }

    /// Set the rejected_quantity field (default: `Decimal::from(0)`)
    pub fn rejected_quantity(mut self, value: Decimal) -> Self {
        self.rejected_quantity = Some(value);
        self
    }

    /// Set the variance_quantity field (required)
    pub fn variance_quantity(mut self, value: Decimal) -> Self {
        self.variance_quantity = Some(value);
        self
    }

    /// Set the uom field (required)
    pub fn uom(mut self, value: String) -> Self {
        self.uom = Some(value);
        self
    }

    /// Set the base_uom_quantity field (optional)
    pub fn base_uom_quantity(mut self, value: Decimal) -> Self {
        self.base_uom_quantity = Some(value);
        self
    }

    /// Set the conversion_factor field (default: `Decimal::from(1)`)
    pub fn conversion_factor(mut self, value: Decimal) -> Self {
        self.conversion_factor = Some(value);
        self
    }

    /// Set the unit_cost field (required)
    pub fn unit_cost(mut self, value: Decimal) -> Self {
        self.unit_cost = Some(value);
        self
    }

    /// Set the total_cost field (required)
    pub fn total_cost(mut self, value: Decimal) -> Self {
        self.total_cost = Some(value);
        self
    }

    /// Set the currency field (default: `"IDR".to_string()`)
    pub fn currency(mut self, value: String) -> Self {
        self.currency = Some(value);
        self
    }

    /// Set the batch_number field (optional)
    pub fn batch_number(mut self, value: String) -> Self {
        self.batch_number = Some(value);
        self
    }

    /// Set the serial_numbers field (default: `serde_json::json!([])`)
    pub fn serial_numbers(mut self, value: serde_json::Value) -> Self {
        self.serial_numbers = Some(value);
        self
    }

    /// Set the manufacture_date field (optional)
    pub fn manufacture_date(mut self, value: NaiveDate) -> Self {
        self.manufacture_date = Some(value);
        self
    }

    /// Set the expiry_date field (optional)
    pub fn expiry_date(mut self, value: NaiveDate) -> Self {
        self.expiry_date = Some(value);
        self
    }

    /// Set the warehouse_id field (optional)
    pub fn warehouse_id(mut self, value: Uuid) -> Self {
        self.warehouse_id = Some(value);
        self
    }

    /// Set the bin_location field (optional)
    pub fn bin_location(mut self, value: String) -> Self {
        self.bin_location = Some(value);
        self
    }

    /// Set the requires_qc field (default: `false`)
    pub fn requires_qc(mut self, value: bool) -> Self {
        self.requires_qc = Some(value);
        self
    }

    /// Set the qc_result field (default: `QCResult::default()`)
    pub fn qc_result(mut self, value: QCResult) -> Self {
        self.qc_result = Some(value);
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

    /// Set the qc_parameters field (default: `serde_json::json!({})`)
    pub fn qc_parameters(mut self, value: serde_json::Value) -> Self {
        self.qc_parameters = Some(value);
        self
    }

    /// Set the rejection_reason field (optional)
    pub fn rejection_reason(mut self, value: String) -> Self {
        self.rejection_reason = Some(value);
        self
    }

    /// Set the rejection_action field (optional)
    pub fn rejection_action(mut self, value: String) -> Self {
        self.rejection_action = Some(value);
        self
    }

    /// Set the status field (default: `ReceiptItemStatus::default()`)
    pub fn status(mut self, value: ReceiptItemStatus) -> Self {
        self.status = Some(value);
        self
    }

    /// Set the stock_movement_id field (optional)
    pub fn stock_movement_id(mut self, value: Uuid) -> Self {
        self.stock_movement_id = Some(value);
        self
    }

    /// Set the is_posted field (default: `false`)
    pub fn is_posted(mut self, value: bool) -> Self {
        self.is_posted = Some(value);
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

    /// Build the GoodsReceiptItem entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<GoodsReceiptItem, String> {
        let goods_receipt_id = self.goods_receipt_id.ok_or_else(|| "goods_receipt_id is required".to_string())?;
        let product_id = self.product_id.ok_or_else(|| "product_id is required".to_string())?;
        let ordered_quantity = self.ordered_quantity.ok_or_else(|| "ordered_quantity is required".to_string())?;
        let received_quantity = self.received_quantity.ok_or_else(|| "received_quantity is required".to_string())?;
        let variance_quantity = self.variance_quantity.ok_or_else(|| "variance_quantity is required".to_string())?;
        let uom = self.uom.ok_or_else(|| "uom is required".to_string())?;
        let unit_cost = self.unit_cost.ok_or_else(|| "unit_cost is required".to_string())?;
        let total_cost = self.total_cost.ok_or_else(|| "total_cost is required".to_string())?;

        Ok(GoodsReceiptItem {
            id: Uuid::new_v4(),
            goods_receipt_id,
            product_id,
            purchase_order_item_id: self.purchase_order_item_id,
            ordered_quantity,
            received_quantity,
            accepted_quantity: self.accepted_quantity.unwrap_or(Decimal::from(0)),
            rejected_quantity: self.rejected_quantity.unwrap_or(Decimal::from(0)),
            variance_quantity,
            uom,
            base_uom_quantity: self.base_uom_quantity,
            conversion_factor: self.conversion_factor.unwrap_or(Decimal::from(1)),
            unit_cost,
            total_cost,
            currency: self.currency.unwrap_or("IDR".to_string()),
            batch_number: self.batch_number,
            serial_numbers: self.serial_numbers.unwrap_or(serde_json::json!([])),
            manufacture_date: self.manufacture_date,
            expiry_date: self.expiry_date,
            warehouse_id: self.warehouse_id,
            bin_location: self.bin_location,
            requires_qc: self.requires_qc.unwrap_or(false),
            qc_result: self.qc_result.unwrap_or(QCResult::default()),
            qc_date: self.qc_date,
            qc_by: self.qc_by,
            qc_notes: self.qc_notes,
            qc_parameters: self.qc_parameters.unwrap_or(serde_json::json!({})),
            rejection_reason: self.rejection_reason,
            rejection_action: self.rejection_action,
            status: self.status.unwrap_or(ReceiptItemStatus::default()),
            stock_movement_id: self.stock_movement_id,
            is_posted: self.is_posted.unwrap_or(false),
            posted_at: self.posted_at,
            notes: self.notes,
            metadata: AuditMetadata::default(),
        })
    }
}
