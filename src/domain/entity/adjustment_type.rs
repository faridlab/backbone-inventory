use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "adjustment_type", rename_all = "snake_case")]
pub enum AdjustmentType {
    StockCount,
    Damage,
    Expiry,
    Loss,
    Correction,
}

impl std::fmt::Display for AdjustmentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StockCount => write!(f, "stock_count"),
            Self::Damage => write!(f, "damage"),
            Self::Expiry => write!(f, "expiry"),
            Self::Loss => write!(f, "loss"),
            Self::Correction => write!(f, "correction"),
        }
    }
}

impl FromStr for AdjustmentType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "stock_count" => Ok(Self::StockCount),
            "damage" => Ok(Self::Damage),
            "expiry" => Ok(Self::Expiry),
            "loss" => Ok(Self::Loss),
            "correction" => Ok(Self::Correction),
            _ => Err(format!("Unknown AdjustmentType variant: {}", s)),
        }
    }
}

impl Default for AdjustmentType {
    fn default() -> Self {
        Self::StockCount
    }
}
