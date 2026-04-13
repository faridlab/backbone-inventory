use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "transfer_status", rename_all = "snake_case")]
pub enum TransferStatus {
    Draft,
    PendingApproval,
    Approved,
    InTransit,
    PartialReceived,
    Received,
    Completed,
    Cancelled,
}

impl std::fmt::Display for TransferStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Draft => write!(f, "draft"),
            Self::PendingApproval => write!(f, "pending_approval"),
            Self::Approved => write!(f, "approved"),
            Self::InTransit => write!(f, "in_transit"),
            Self::PartialReceived => write!(f, "partial_received"),
            Self::Received => write!(f, "received"),
            Self::Completed => write!(f, "completed"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl FromStr for TransferStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "draft" => Ok(Self::Draft),
            "pending_approval" => Ok(Self::PendingApproval),
            "approved" => Ok(Self::Approved),
            "in_transit" => Ok(Self::InTransit),
            "partial_received" => Ok(Self::PartialReceived),
            "received" => Ok(Self::Received),
            "completed" => Ok(Self::Completed),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(format!("Unknown TransferStatus variant: {}", s)),
        }
    }
}

impl Default for TransferStatus {
    fn default() -> Self {
        Self::Draft
    }
}
