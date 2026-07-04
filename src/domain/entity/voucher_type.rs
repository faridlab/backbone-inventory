use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "voucher_type", rename_all = "snake_case")]
pub enum VoucherType {
    PurchaseReceipt,
    DeliveryNote,
    StockEntry,
    StockReconciliation,
}

impl std::fmt::Display for VoucherType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PurchaseReceipt => write!(f, "purchase_receipt"),
            Self::DeliveryNote => write!(f, "delivery_note"),
            Self::StockEntry => write!(f, "stock_entry"),
            Self::StockReconciliation => write!(f, "stock_reconciliation"),
        }
    }
}

impl FromStr for VoucherType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "purchase_receipt" => Ok(Self::PurchaseReceipt),
            "delivery_note" => Ok(Self::DeliveryNote),
            "stock_entry" => Ok(Self::StockEntry),
            "stock_reconciliation" => Ok(Self::StockReconciliation),
            _ => Err(format!("Unknown VoucherType variant: {}", s)),
        }
    }
}

impl Default for VoucherType {
    fn default() -> Self {
        Self::PurchaseReceipt
    }
}
