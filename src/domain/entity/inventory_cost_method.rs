use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "inventory_cost_method", rename_all = "snake_case")]
pub enum InventoryCostMethod {
    Average,
    Fifo,
    Standard,
}

impl std::fmt::Display for InventoryCostMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Average => write!(f, "average"),
            Self::Fifo => write!(f, "fifo"),
            Self::Standard => write!(f, "standard"),
        }
    }
}

impl FromStr for InventoryCostMethod {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "average" => Ok(Self::Average),
            "fifo" => Ok(Self::Fifo),
            "standard" => Ok(Self::Standard),
            _ => Err(format!("Unknown InventoryCostMethod variant: {}", s)),
        }
    }
}

impl Default for InventoryCostMethod {
    fn default() -> Self {
        Self::Average
    }
}
