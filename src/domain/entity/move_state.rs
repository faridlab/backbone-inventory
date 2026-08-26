use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "move_state", rename_all = "snake_case")]
pub enum MoveState {
    Draft,
    Waiting,
    Confirmed,
    PartiallyAvailable,
    Assigned,
    Done,
    Cancel,
}

impl std::fmt::Display for MoveState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Draft => write!(f, "draft"),
            Self::Waiting => write!(f, "waiting"),
            Self::Confirmed => write!(f, "confirmed"),
            Self::PartiallyAvailable => write!(f, "partially_available"),
            Self::Assigned => write!(f, "assigned"),
            Self::Done => write!(f, "done"),
            Self::Cancel => write!(f, "cancel"),
        }
    }
}

impl FromStr for MoveState {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "draft" => Ok(Self::Draft),
            "waiting" => Ok(Self::Waiting),
            "confirmed" => Ok(Self::Confirmed),
            "partially_available" => Ok(Self::PartiallyAvailable),
            "assigned" => Ok(Self::Assigned),
            "done" => Ok(Self::Done),
            "cancel" => Ok(Self::Cancel),
            _ => Err(format!("Unknown MoveState variant: {}", s)),
        }
    }
}

impl Default for MoveState {
    fn default() -> Self {
        Self::Draft
    }
}
