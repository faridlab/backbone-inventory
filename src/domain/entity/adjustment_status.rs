use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "adjustment_status", rename_all = "snake_case")]
pub enum AdjustmentStatus {
    Draft,
    Counting,
    Verified,
    PendingApproval,
    Approved,
    Rejected,
    Posted,
    Cancelled,
}

impl std::fmt::Display for AdjustmentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Draft => write!(f, "draft"),
            Self::Counting => write!(f, "counting"),
            Self::Verified => write!(f, "verified"),
            Self::PendingApproval => write!(f, "pending_approval"),
            Self::Approved => write!(f, "approved"),
            Self::Rejected => write!(f, "rejected"),
            Self::Posted => write!(f, "posted"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl FromStr for AdjustmentStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "draft" => Ok(Self::Draft),
            "counting" => Ok(Self::Counting),
            "verified" => Ok(Self::Verified),
            "pending_approval" => Ok(Self::PendingApproval),
            "approved" => Ok(Self::Approved),
            "rejected" => Ok(Self::Rejected),
            "posted" => Ok(Self::Posted),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(format!("Unknown AdjustmentStatus variant: {}", s)),
        }
    }
}

impl Default for AdjustmentStatus {
    fn default() -> Self {
        Self::Draft
    }
}
