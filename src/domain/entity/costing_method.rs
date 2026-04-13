use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "costing_method", rename_all = "snake_case")]
pub enum CostingMethod {
    Standard,
    LastPurchase,
    WeightedAverage,
    Fifo,
}

impl std::fmt::Display for CostingMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Standard => write!(f, "standard"),
            Self::LastPurchase => write!(f, "last_purchase"),
            Self::WeightedAverage => write!(f, "weighted_average"),
            Self::Fifo => write!(f, "fifo"),
        }
    }
}

impl FromStr for CostingMethod {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "standard" => Ok(Self::Standard),
            "last_purchase" => Ok(Self::LastPurchase),
            "weighted_average" => Ok(Self::WeightedAverage),
            "fifo" => Ok(Self::Fifo),
            _ => Err(format!("Unknown CostingMethod variant: {}", s)),
        }
    }
}

impl Default for CostingMethod {
    fn default() -> Self {
        Self::WeightedAverage
    }
}
