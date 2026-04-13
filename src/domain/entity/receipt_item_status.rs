use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "receipt_item_status", rename_all = "snake_case")]
pub enum ReceiptItemStatus {
    Pending,
    Received,
    Partial,
    Rejected,
    Returned,
}

impl std::fmt::Display for ReceiptItemStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Received => write!(f, "received"),
            Self::Partial => write!(f, "partial"),
            Self::Rejected => write!(f, "rejected"),
            Self::Returned => write!(f, "returned"),
        }
    }
}

impl FromStr for ReceiptItemStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(Self::Pending),
            "received" => Ok(Self::Received),
            "partial" => Ok(Self::Partial),
            "rejected" => Ok(Self::Rejected),
            "returned" => Ok(Self::Returned),
            _ => Err(format!("Unknown ReceiptItemStatus variant: {}", s)),
        }
    }
}

impl Default for ReceiptItemStatus {
    fn default() -> Self {
        Self::Pending
    }
}
