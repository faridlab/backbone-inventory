use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "transfer_item_status", rename_all = "snake_case")]
pub enum TransferItemStatus {
    Pending,
    Shipped,
    PartialReceived,
    Received,
    Rejected,
}

impl std::fmt::Display for TransferItemStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Shipped => write!(f, "shipped"),
            Self::PartialReceived => write!(f, "partial_received"),
            Self::Received => write!(f, "received"),
            Self::Rejected => write!(f, "rejected"),
        }
    }
}

impl FromStr for TransferItemStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(Self::Pending),
            "shipped" => Ok(Self::Shipped),
            "partial_received" => Ok(Self::PartialReceived),
            "received" => Ok(Self::Received),
            "rejected" => Ok(Self::Rejected),
            _ => Err(format!("Unknown TransferItemStatus variant: {}", s)),
        }
    }
}

impl Default for TransferItemStatus {
    fn default() -> Self {
        Self::Pending
    }
}
