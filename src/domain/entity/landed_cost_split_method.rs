use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "landed_cost_split_method", rename_all = "snake_case")]
pub enum LandedCostSplitMethod {
    Quantity,
    Value,
    Weight,
}

impl std::fmt::Display for LandedCostSplitMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Quantity => write!(f, "quantity"),
            Self::Value => write!(f, "value"),
            Self::Weight => write!(f, "weight"),
        }
    }
}

impl FromStr for LandedCostSplitMethod {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "quantity" => Ok(Self::Quantity),
            "value" => Ok(Self::Value),
            "weight" => Ok(Self::Weight),
            _ => Err(format!("Unknown LandedCostSplitMethod variant: {}", s)),
        }
    }
}

impl Default for LandedCostSplitMethod {
    fn default() -> Self {
        Self::Quantity
    }
}
