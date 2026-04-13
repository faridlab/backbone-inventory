use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use rust_decimal::Decimal;

use super::ProductType;
use super::UnitOfMeasure;
use super::CostingMethod;
use super::ProductStatus;
use super::AuditMetadata;

/// Strongly-typed ID for Product
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProductId(pub Uuid);

impl ProductId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for ProductId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for ProductId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for ProductId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<ProductId> for Uuid {
    fn from(id: ProductId) -> Self { id.0 }
}

impl AsRef<Uuid> for ProductId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for ProductId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Product {
    pub id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<Uuid>,
    pub sku: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub barcode: Option<String>,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub product_type: ProductType,
    pub category_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub brand: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manufacturer: Option<String>,
    pub base_uom: UnitOfMeasure,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub purchase_uom: Option<UnitOfMeasure>,
    pub purchase_uom_ratio: Decimal,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secondary_uom: Option<UnitOfMeasure>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secondary_uom_ratio: Option<Decimal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weight_kg: Option<Decimal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume_liter: Option<Decimal>,
    pub costing_method: CostingMethod,
    pub standard_cost: Decimal,
    pub last_purchase_cost: Decimal,
    pub average_cost: Decimal,
    pub currency: String,
    pub is_stockable: bool,
    pub track_batches: bool,
    pub track_expiry: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shelf_life_days: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiry_warning_days: Option<i32>,
    pub min_stock: Decimal,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_stock: Option<Decimal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reorder_quantity: Option<Decimal>,
    pub safety_stock: Decimal,
    pub lead_time_days: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_per_kg: Option<Decimal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_per_order: Option<Decimal>,
    pub applicable_services: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_conditions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
    pub status: ProductStatus,
    pub tags: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    pub data: serde_json::Value,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl Product {
    /// Create a builder for Product
    pub fn builder() -> ProductBuilder {
        ProductBuilder::default()
    }

    /// Create a new Product with required fields
    pub fn new(sku: String, name: String, product_type: ProductType, category_id: Uuid, base_uom: UnitOfMeasure, purchase_uom_ratio: Decimal, costing_method: CostingMethod, standard_cost: Decimal, last_purchase_cost: Decimal, average_cost: Decimal, currency: String, is_stockable: bool, track_batches: bool, track_expiry: bool, min_stock: Decimal, safety_stock: Decimal, lead_time_days: i32, applicable_services: serde_json::Value, status: ProductStatus, tags: serde_json::Value, data: serde_json::Value) -> Self {
        Self {
            id: Uuid::new_v4(),
            provider_id: None,
            sku,
            barcode: None,
            name,
            description: None,
            product_type,
            category_id,
            brand: None,
            manufacturer: None,
            base_uom,
            purchase_uom: None,
            purchase_uom_ratio,
            secondary_uom: None,
            secondary_uom_ratio: None,
            weight_kg: None,
            volume_liter: None,
            costing_method,
            standard_cost,
            last_purchase_cost,
            average_cost,
            currency,
            is_stockable,
            track_batches,
            track_expiry,
            shelf_life_days: None,
            expiry_warning_days: None,
            min_stock,
            max_stock: None,
            reorder_quantity: None,
            safety_stock,
            lead_time_days,
            usage_per_kg: None,
            usage_per_order: None,
            applicable_services,
            storage_conditions: None,
            image_url: None,
            status,
            tags,
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
    pub fn typed_id(&self) -> ProductId {
        ProductId(self.id)
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
    pub fn status(&self) -> &ProductStatus {
        &self.status
    }


    // ==========================================================
    // Fluent Setters (with_* for optional fields)
    // ==========================================================

    /// Set the provider_id field (chainable)
    pub fn with_provider_id(mut self, value: Uuid) -> Self {
        self.provider_id = Some(value);
        self
    }

    /// Set the barcode field (chainable)
    pub fn with_barcode(mut self, value: String) -> Self {
        self.barcode = Some(value);
        self
    }

    /// Set the description field (chainable)
    pub fn with_description(mut self, value: String) -> Self {
        self.description = Some(value);
        self
    }

    /// Set the brand field (chainable)
    pub fn with_brand(mut self, value: String) -> Self {
        self.brand = Some(value);
        self
    }

    /// Set the manufacturer field (chainable)
    pub fn with_manufacturer(mut self, value: String) -> Self {
        self.manufacturer = Some(value);
        self
    }

    /// Set the purchase_uom field (chainable)
    pub fn with_purchase_uom(mut self, value: UnitOfMeasure) -> Self {
        self.purchase_uom = Some(value);
        self
    }

    /// Set the secondary_uom field (chainable)
    pub fn with_secondary_uom(mut self, value: UnitOfMeasure) -> Self {
        self.secondary_uom = Some(value);
        self
    }

    /// Set the secondary_uom_ratio field (chainable)
    pub fn with_secondary_uom_ratio(mut self, value: Decimal) -> Self {
        self.secondary_uom_ratio = Some(value);
        self
    }

    /// Set the weight_kg field (chainable)
    pub fn with_weight_kg(mut self, value: Decimal) -> Self {
        self.weight_kg = Some(value);
        self
    }

    /// Set the volume_liter field (chainable)
    pub fn with_volume_liter(mut self, value: Decimal) -> Self {
        self.volume_liter = Some(value);
        self
    }

    /// Set the shelf_life_days field (chainable)
    pub fn with_shelf_life_days(mut self, value: i32) -> Self {
        self.shelf_life_days = Some(value);
        self
    }

    /// Set the expiry_warning_days field (chainable)
    pub fn with_expiry_warning_days(mut self, value: i32) -> Self {
        self.expiry_warning_days = Some(value);
        self
    }

    /// Set the max_stock field (chainable)
    pub fn with_max_stock(mut self, value: Decimal) -> Self {
        self.max_stock = Some(value);
        self
    }

    /// Set the reorder_quantity field (chainable)
    pub fn with_reorder_quantity(mut self, value: Decimal) -> Self {
        self.reorder_quantity = Some(value);
        self
    }

    /// Set the usage_per_kg field (chainable)
    pub fn with_usage_per_kg(mut self, value: Decimal) -> Self {
        self.usage_per_kg = Some(value);
        self
    }

    /// Set the usage_per_order field (chainable)
    pub fn with_usage_per_order(mut self, value: Decimal) -> Self {
        self.usage_per_order = Some(value);
        self
    }

    /// Set the storage_conditions field (chainable)
    pub fn with_storage_conditions(mut self, value: String) -> Self {
        self.storage_conditions = Some(value);
        self
    }

    /// Set the image_url field (chainable)
    pub fn with_image_url(mut self, value: String) -> Self {
        self.image_url = Some(value);
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
                "sku" => {
                    if let Ok(v) = serde_json::from_value(value) { self.sku = v; }
                }
                "barcode" => {
                    if let Ok(v) = serde_json::from_value(value) { self.barcode = v; }
                }
                "name" => {
                    if let Ok(v) = serde_json::from_value(value) { self.name = v; }
                }
                "description" => {
                    if let Ok(v) = serde_json::from_value(value) { self.description = v; }
                }
                "product_type" => {
                    if let Ok(v) = serde_json::from_value(value) { self.product_type = v; }
                }
                "category_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.category_id = v; }
                }
                "brand" => {
                    if let Ok(v) = serde_json::from_value(value) { self.brand = v; }
                }
                "manufacturer" => {
                    if let Ok(v) = serde_json::from_value(value) { self.manufacturer = v; }
                }
                "base_uom" => {
                    if let Ok(v) = serde_json::from_value(value) { self.base_uom = v; }
                }
                "purchase_uom" => {
                    if let Ok(v) = serde_json::from_value(value) { self.purchase_uom = v; }
                }
                "purchase_uom_ratio" => {
                    if let Ok(v) = serde_json::from_value(value) { self.purchase_uom_ratio = v; }
                }
                "secondary_uom" => {
                    if let Ok(v) = serde_json::from_value(value) { self.secondary_uom = v; }
                }
                "secondary_uom_ratio" => {
                    if let Ok(v) = serde_json::from_value(value) { self.secondary_uom_ratio = v; }
                }
                "weight_kg" => {
                    if let Ok(v) = serde_json::from_value(value) { self.weight_kg = v; }
                }
                "volume_liter" => {
                    if let Ok(v) = serde_json::from_value(value) { self.volume_liter = v; }
                }
                "costing_method" => {
                    if let Ok(v) = serde_json::from_value(value) { self.costing_method = v; }
                }
                "standard_cost" => {
                    if let Ok(v) = serde_json::from_value(value) { self.standard_cost = v; }
                }
                "last_purchase_cost" => {
                    if let Ok(v) = serde_json::from_value(value) { self.last_purchase_cost = v; }
                }
                "average_cost" => {
                    if let Ok(v) = serde_json::from_value(value) { self.average_cost = v; }
                }
                "currency" => {
                    if let Ok(v) = serde_json::from_value(value) { self.currency = v; }
                }
                "is_stockable" => {
                    if let Ok(v) = serde_json::from_value(value) { self.is_stockable = v; }
                }
                "track_batches" => {
                    if let Ok(v) = serde_json::from_value(value) { self.track_batches = v; }
                }
                "track_expiry" => {
                    if let Ok(v) = serde_json::from_value(value) { self.track_expiry = v; }
                }
                "shelf_life_days" => {
                    if let Ok(v) = serde_json::from_value(value) { self.shelf_life_days = v; }
                }
                "expiry_warning_days" => {
                    if let Ok(v) = serde_json::from_value(value) { self.expiry_warning_days = v; }
                }
                "min_stock" => {
                    if let Ok(v) = serde_json::from_value(value) { self.min_stock = v; }
                }
                "max_stock" => {
                    if let Ok(v) = serde_json::from_value(value) { self.max_stock = v; }
                }
                "reorder_quantity" => {
                    if let Ok(v) = serde_json::from_value(value) { self.reorder_quantity = v; }
                }
                "safety_stock" => {
                    if let Ok(v) = serde_json::from_value(value) { self.safety_stock = v; }
                }
                "lead_time_days" => {
                    if let Ok(v) = serde_json::from_value(value) { self.lead_time_days = v; }
                }
                "usage_per_kg" => {
                    if let Ok(v) = serde_json::from_value(value) { self.usage_per_kg = v; }
                }
                "usage_per_order" => {
                    if let Ok(v) = serde_json::from_value(value) { self.usage_per_order = v; }
                }
                "applicable_services" => {
                    if let Ok(v) = serde_json::from_value(value) { self.applicable_services = v; }
                }
                "storage_conditions" => {
                    if let Ok(v) = serde_json::from_value(value) { self.storage_conditions = v; }
                }
                "image_url" => {
                    if let Ok(v) = serde_json::from_value(value) { self.image_url = v; }
                }
                "status" => {
                    if let Ok(v) = serde_json::from_value(value) { self.status = v; }
                }
                "tags" => {
                    if let Ok(v) = serde_json::from_value(value) { self.tags = v; }
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

impl super::Entity for Product {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "Product"
    }
}

impl backbone_core::PersistentEntity for Product {
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

impl backbone_orm::EntityRepoMeta for Product {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("provider_id".to_string(), "uuid".to_string());
        m.insert("category_id".to_string(), "uuid".to_string());
        m.insert("product_type".to_string(), "product_type".to_string());
        m.insert("base_uom".to_string(), "unit_of_measure".to_string());
        m.insert("purchase_uom".to_string(), "unit_of_measure".to_string());
        m.insert("secondary_uom".to_string(), "unit_of_measure".to_string());
        m.insert("costing_method".to_string(), "costing_method".to_string());
        m.insert("status".to_string(), "product_status".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["sku", "name", "currency"]
    }
}

/// Builder for Product entity
///
/// Provides a fluent API for constructing Product instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct ProductBuilder {
    provider_id: Option<Uuid>,
    sku: Option<String>,
    barcode: Option<String>,
    name: Option<String>,
    description: Option<String>,
    product_type: Option<ProductType>,
    category_id: Option<Uuid>,
    brand: Option<String>,
    manufacturer: Option<String>,
    base_uom: Option<UnitOfMeasure>,
    purchase_uom: Option<UnitOfMeasure>,
    purchase_uom_ratio: Option<Decimal>,
    secondary_uom: Option<UnitOfMeasure>,
    secondary_uom_ratio: Option<Decimal>,
    weight_kg: Option<Decimal>,
    volume_liter: Option<Decimal>,
    costing_method: Option<CostingMethod>,
    standard_cost: Option<Decimal>,
    last_purchase_cost: Option<Decimal>,
    average_cost: Option<Decimal>,
    currency: Option<String>,
    is_stockable: Option<bool>,
    track_batches: Option<bool>,
    track_expiry: Option<bool>,
    shelf_life_days: Option<i32>,
    expiry_warning_days: Option<i32>,
    min_stock: Option<Decimal>,
    max_stock: Option<Decimal>,
    reorder_quantity: Option<Decimal>,
    safety_stock: Option<Decimal>,
    lead_time_days: Option<i32>,
    usage_per_kg: Option<Decimal>,
    usage_per_order: Option<Decimal>,
    applicable_services: Option<serde_json::Value>,
    storage_conditions: Option<String>,
    image_url: Option<String>,
    status: Option<ProductStatus>,
    tags: Option<serde_json::Value>,
    notes: Option<String>,
    data: Option<serde_json::Value>,
}

impl ProductBuilder {
    /// Set the provider_id field (optional)
    pub fn provider_id(mut self, value: Uuid) -> Self {
        self.provider_id = Some(value);
        self
    }

    /// Set the sku field (required)
    pub fn sku(mut self, value: String) -> Self {
        self.sku = Some(value);
        self
    }

    /// Set the barcode field (optional)
    pub fn barcode(mut self, value: String) -> Self {
        self.barcode = Some(value);
        self
    }

    /// Set the name field (required)
    pub fn name(mut self, value: String) -> Self {
        self.name = Some(value);
        self
    }

    /// Set the description field (optional)
    pub fn description(mut self, value: String) -> Self {
        self.description = Some(value);
        self
    }

    /// Set the product_type field (default: `ProductType::default()`)
    pub fn product_type(mut self, value: ProductType) -> Self {
        self.product_type = Some(value);
        self
    }

    /// Set the category_id field (required)
    pub fn category_id(mut self, value: Uuid) -> Self {
        self.category_id = Some(value);
        self
    }

    /// Set the brand field (optional)
    pub fn brand(mut self, value: String) -> Self {
        self.brand = Some(value);
        self
    }

    /// Set the manufacturer field (optional)
    pub fn manufacturer(mut self, value: String) -> Self {
        self.manufacturer = Some(value);
        self
    }

    /// Set the base_uom field (default: `UnitOfMeasure::default()`)
    pub fn base_uom(mut self, value: UnitOfMeasure) -> Self {
        self.base_uom = Some(value);
        self
    }

    /// Set the purchase_uom field (optional)
    pub fn purchase_uom(mut self, value: UnitOfMeasure) -> Self {
        self.purchase_uom = Some(value);
        self
    }

    /// Set the purchase_uom_ratio field (default: `Decimal::from(1)`)
    pub fn purchase_uom_ratio(mut self, value: Decimal) -> Self {
        self.purchase_uom_ratio = Some(value);
        self
    }

    /// Set the secondary_uom field (optional)
    pub fn secondary_uom(mut self, value: UnitOfMeasure) -> Self {
        self.secondary_uom = Some(value);
        self
    }

    /// Set the secondary_uom_ratio field (optional)
    pub fn secondary_uom_ratio(mut self, value: Decimal) -> Self {
        self.secondary_uom_ratio = Some(value);
        self
    }

    /// Set the weight_kg field (optional)
    pub fn weight_kg(mut self, value: Decimal) -> Self {
        self.weight_kg = Some(value);
        self
    }

    /// Set the volume_liter field (optional)
    pub fn volume_liter(mut self, value: Decimal) -> Self {
        self.volume_liter = Some(value);
        self
    }

    /// Set the costing_method field (default: `CostingMethod::default()`)
    pub fn costing_method(mut self, value: CostingMethod) -> Self {
        self.costing_method = Some(value);
        self
    }

    /// Set the standard_cost field (default: `Decimal::from(0)`)
    pub fn standard_cost(mut self, value: Decimal) -> Self {
        self.standard_cost = Some(value);
        self
    }

    /// Set the last_purchase_cost field (default: `Decimal::from(0)`)
    pub fn last_purchase_cost(mut self, value: Decimal) -> Self {
        self.last_purchase_cost = Some(value);
        self
    }

    /// Set the average_cost field (default: `Decimal::from(0)`)
    pub fn average_cost(mut self, value: Decimal) -> Self {
        self.average_cost = Some(value);
        self
    }

    /// Set the currency field (default: `"IDR".to_string()`)
    pub fn currency(mut self, value: String) -> Self {
        self.currency = Some(value);
        self
    }

    /// Set the is_stockable field (default: `true`)
    pub fn is_stockable(mut self, value: bool) -> Self {
        self.is_stockable = Some(value);
        self
    }

    /// Set the track_batches field (default: `false`)
    pub fn track_batches(mut self, value: bool) -> Self {
        self.track_batches = Some(value);
        self
    }

    /// Set the track_expiry field (default: `false`)
    pub fn track_expiry(mut self, value: bool) -> Self {
        self.track_expiry = Some(value);
        self
    }

    /// Set the shelf_life_days field (optional)
    pub fn shelf_life_days(mut self, value: i32) -> Self {
        self.shelf_life_days = Some(value);
        self
    }

    /// Set the expiry_warning_days field (optional)
    pub fn expiry_warning_days(mut self, value: i32) -> Self {
        self.expiry_warning_days = Some(value);
        self
    }

    /// Set the min_stock field (default: `Decimal::from(0)`)
    pub fn min_stock(mut self, value: Decimal) -> Self {
        self.min_stock = Some(value);
        self
    }

    /// Set the max_stock field (optional)
    pub fn max_stock(mut self, value: Decimal) -> Self {
        self.max_stock = Some(value);
        self
    }

    /// Set the reorder_quantity field (optional)
    pub fn reorder_quantity(mut self, value: Decimal) -> Self {
        self.reorder_quantity = Some(value);
        self
    }

    /// Set the safety_stock field (default: `Decimal::from(0)`)
    pub fn safety_stock(mut self, value: Decimal) -> Self {
        self.safety_stock = Some(value);
        self
    }

    /// Set the lead_time_days field (default: `0`)
    pub fn lead_time_days(mut self, value: i32) -> Self {
        self.lead_time_days = Some(value);
        self
    }

    /// Set the usage_per_kg field (optional)
    pub fn usage_per_kg(mut self, value: Decimal) -> Self {
        self.usage_per_kg = Some(value);
        self
    }

    /// Set the usage_per_order field (optional)
    pub fn usage_per_order(mut self, value: Decimal) -> Self {
        self.usage_per_order = Some(value);
        self
    }

    /// Set the applicable_services field (default: `serde_json::json!([])`)
    pub fn applicable_services(mut self, value: serde_json::Value) -> Self {
        self.applicable_services = Some(value);
        self
    }

    /// Set the storage_conditions field (optional)
    pub fn storage_conditions(mut self, value: String) -> Self {
        self.storage_conditions = Some(value);
        self
    }

    /// Set the image_url field (optional)
    pub fn image_url(mut self, value: String) -> Self {
        self.image_url = Some(value);
        self
    }

    /// Set the status field (default: `ProductStatus::default()`)
    pub fn status(mut self, value: ProductStatus) -> Self {
        self.status = Some(value);
        self
    }

    /// Set the tags field (default: `serde_json::json!([])`)
    pub fn tags(mut self, value: serde_json::Value) -> Self {
        self.tags = Some(value);
        self
    }

    /// Set the notes field (optional)
    pub fn notes(mut self, value: String) -> Self {
        self.notes = Some(value);
        self
    }

    /// Set the data field (default: `serde_json::json!({})`)
    pub fn data(mut self, value: serde_json::Value) -> Self {
        self.data = Some(value);
        self
    }

    /// Build the Product entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<Product, String> {
        let sku = self.sku.ok_or_else(|| "sku is required".to_string())?;
        let name = self.name.ok_or_else(|| "name is required".to_string())?;
        let category_id = self.category_id.ok_or_else(|| "category_id is required".to_string())?;

        Ok(Product {
            id: Uuid::new_v4(),
            provider_id: self.provider_id,
            sku,
            barcode: self.barcode,
            name,
            description: self.description,
            product_type: self.product_type.unwrap_or(ProductType::default()),
            category_id,
            brand: self.brand,
            manufacturer: self.manufacturer,
            base_uom: self.base_uom.unwrap_or(UnitOfMeasure::default()),
            purchase_uom: self.purchase_uom,
            purchase_uom_ratio: self.purchase_uom_ratio.unwrap_or(Decimal::from(1)),
            secondary_uom: self.secondary_uom,
            secondary_uom_ratio: self.secondary_uom_ratio,
            weight_kg: self.weight_kg,
            volume_liter: self.volume_liter,
            costing_method: self.costing_method.unwrap_or(CostingMethod::default()),
            standard_cost: self.standard_cost.unwrap_or(Decimal::from(0)),
            last_purchase_cost: self.last_purchase_cost.unwrap_or(Decimal::from(0)),
            average_cost: self.average_cost.unwrap_or(Decimal::from(0)),
            currency: self.currency.unwrap_or("IDR".to_string()),
            is_stockable: self.is_stockable.unwrap_or(true),
            track_batches: self.track_batches.unwrap_or(false),
            track_expiry: self.track_expiry.unwrap_or(false),
            shelf_life_days: self.shelf_life_days,
            expiry_warning_days: self.expiry_warning_days,
            min_stock: self.min_stock.unwrap_or(Decimal::from(0)),
            max_stock: self.max_stock,
            reorder_quantity: self.reorder_quantity,
            safety_stock: self.safety_stock.unwrap_or(Decimal::from(0)),
            lead_time_days: self.lead_time_days.unwrap_or(0),
            usage_per_kg: self.usage_per_kg,
            usage_per_order: self.usage_per_order,
            applicable_services: self.applicable_services.unwrap_or(serde_json::json!([])),
            storage_conditions: self.storage_conditions,
            image_url: self.image_url,
            status: self.status.unwrap_or(ProductStatus::default()),
            tags: self.tags.unwrap_or(serde_json::json!([])),
            notes: self.notes,
            data: self.data.unwrap_or(serde_json::json!({})),
            metadata: AuditMetadata::default(),
        })
    }
}
