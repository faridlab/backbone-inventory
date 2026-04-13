use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "batch_status", rename_all = "snake_case")]
pub enum BatchStatus {
    Available,
    Quarantine,
    Expired,
    Depleted,
}

impl std::fmt::Display for BatchStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Available => write!(f, "available"),
            Self::Quarantine => write!(f, "quarantine"),
            Self::Expired => write!(f, "expired"),
            Self::Depleted => write!(f, "depleted"),
        }
    }
}

impl FromStr for BatchStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "available" => Ok(Self::Available),
            "quarantine" => Ok(Self::Quarantine),
            "expired" => Ok(Self::Expired),
            "depleted" => Ok(Self::Depleted),
            _ => Err(format!("Unknown BatchStatus variant: {}", s)),
        }
    }
}

impl Default for BatchStatus {
    fn default() -> Self {
        Self::Available
    }
}
