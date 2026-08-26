use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "transfer_state", rename_all = "snake_case")]
pub enum TransferState {
    Draft,
    Waiting,
    Confirmed,
    Assigned,
    Done,
    Cancel,
}

impl std::fmt::Display for TransferState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Draft => write!(f, "draft"),
            Self::Waiting => write!(f, "waiting"),
            Self::Confirmed => write!(f, "confirmed"),
            Self::Assigned => write!(f, "assigned"),
            Self::Done => write!(f, "done"),
            Self::Cancel => write!(f, "cancel"),
        }
    }
}

impl FromStr for TransferState {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "draft" => Ok(Self::Draft),
            "waiting" => Ok(Self::Waiting),
            "confirmed" => Ok(Self::Confirmed),
            "assigned" => Ok(Self::Assigned),
            "done" => Ok(Self::Done),
            "cancel" => Ok(Self::Cancel),
            _ => Err(format!("Unknown TransferState variant: {}", s)),
        }
    }
}

impl Default for TransferState {
    fn default() -> Self {
        Self::Draft
    }
}
