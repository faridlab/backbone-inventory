use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "product_status", rename_all = "snake_case")]
pub enum ProductStatus {
    Active,
    Inactive,
    Discontinued,
}

impl std::fmt::Display for ProductStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::Inactive => write!(f, "inactive"),
            Self::Discontinued => write!(f, "discontinued"),
        }
    }
}

impl FromStr for ProductStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "active" => Ok(Self::Active),
            "inactive" => Ok(Self::Inactive),
            "discontinued" => Ok(Self::Discontinued),
            _ => Err(format!("Unknown ProductStatus variant: {}", s)),
        }
    }
}

impl Default for ProductStatus {
    fn default() -> Self {
        Self::Active
    }
}
