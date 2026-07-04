use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "valuation_method", rename_all = "snake_case")]
pub enum ValuationMethod {
    MovingAverage,
    Fifo,
}

impl std::fmt::Display for ValuationMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MovingAverage => write!(f, "moving_average"),
            Self::Fifo => write!(f, "fifo"),
        }
    }
}

impl FromStr for ValuationMethod {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "moving_average" => Ok(Self::MovingAverage),
            "fifo" => Ok(Self::Fifo),
            _ => Err(format!("Unknown ValuationMethod variant: {}", s)),
        }
    }
}

impl Default for ValuationMethod {
    fn default() -> Self {
        Self::MovingAverage
    }
}
