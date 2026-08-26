use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "landed_cost_state", rename_all = "snake_case")]
pub enum LandedCostState {
    Draft,
    Done,
    Cancel,
}

impl std::fmt::Display for LandedCostState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Draft => write!(f, "draft"),
            Self::Done => write!(f, "done"),
            Self::Cancel => write!(f, "cancel"),
        }
    }
}

impl FromStr for LandedCostState {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "draft" => Ok(Self::Draft),
            "done" => Ok(Self::Done),
            "cancel" => Ok(Self::Cancel),
            _ => Err(format!("Unknown LandedCostState variant: {}", s)),
        }
    }
}

impl Default for LandedCostState {
    fn default() -> Self {
        Self::Draft
    }
}
