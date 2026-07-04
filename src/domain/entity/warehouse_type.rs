use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "warehouse_type", rename_all = "snake_case")]
pub enum WarehouseType {
    Stock,
    Transit,
    Wip,
    FinishedGoods,
    Rejected,
}

impl std::fmt::Display for WarehouseType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stock => write!(f, "stock"),
            Self::Transit => write!(f, "transit"),
            Self::Wip => write!(f, "wip"),
            Self::FinishedGoods => write!(f, "finished_goods"),
            Self::Rejected => write!(f, "rejected"),
        }
    }
}

impl FromStr for WarehouseType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "stock" => Ok(Self::Stock),
            "transit" => Ok(Self::Transit),
            "wip" => Ok(Self::Wip),
            "finished_goods" => Ok(Self::FinishedGoods),
            "rejected" => Ok(Self::Rejected),
            _ => Err(format!("Unknown WarehouseType variant: {}", s)),
        }
    }
}

impl Default for WarehouseType {
    fn default() -> Self {
        Self::Stock
    }
}
