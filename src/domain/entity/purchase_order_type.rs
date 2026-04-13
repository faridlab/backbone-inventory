use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "purchase_order_type", rename_all = "snake_case")]
pub enum PurchaseOrderType {
    Standard,
    Urgent,
}

impl std::fmt::Display for PurchaseOrderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Standard => write!(f, "standard"),
            Self::Urgent => write!(f, "urgent"),
        }
    }
}

impl FromStr for PurchaseOrderType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "standard" => Ok(Self::Standard),
            "urgent" => Ok(Self::Urgent),
            _ => Err(format!("Unknown PurchaseOrderType variant: {}", s)),
        }
    }
}

impl Default for PurchaseOrderType {
    fn default() -> Self {
        Self::Standard
    }
}
