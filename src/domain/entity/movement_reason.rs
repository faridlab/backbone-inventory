use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "movement_reason", rename_all = "snake_case")]
pub enum MovementReason {
    Purchase,
    Sale,
    Transfer,
    Adjustment,
    Damage,
    Expiry,
    Theft,
    CountVariance,
    Other,
}

impl std::fmt::Display for MovementReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Purchase => write!(f, "purchase"),
            Self::Sale => write!(f, "sale"),
            Self::Transfer => write!(f, "transfer"),
            Self::Adjustment => write!(f, "adjustment"),
            Self::Damage => write!(f, "damage"),
            Self::Expiry => write!(f, "expiry"),
            Self::Theft => write!(f, "theft"),
            Self::CountVariance => write!(f, "count_variance"),
            Self::Other => write!(f, "other"),
        }
    }
}

impl FromStr for MovementReason {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "purchase" => Ok(Self::Purchase),
            "sale" => Ok(Self::Sale),
            "transfer" => Ok(Self::Transfer),
            "adjustment" => Ok(Self::Adjustment),
            "damage" => Ok(Self::Damage),
            "expiry" => Ok(Self::Expiry),
            "theft" => Ok(Self::Theft),
            "count_variance" => Ok(Self::CountVariance),
            "other" => Ok(Self::Other),
            _ => Err(format!("Unknown MovementReason variant: {}", s)),
        }
    }
}

impl Default for MovementReason {
    fn default() -> Self {
        Self::Purchase
    }
}
