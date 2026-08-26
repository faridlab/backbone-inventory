use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "location_usage", rename_all = "snake_case")]
pub enum LocationUsage {
    Supplier,
    View,
    Internal,
    Customer,
    Inventory,
    Production,
    Transit,
}

impl std::fmt::Display for LocationUsage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Supplier => write!(f, "supplier"),
            Self::View => write!(f, "view"),
            Self::Internal => write!(f, "internal"),
            Self::Customer => write!(f, "customer"),
            Self::Inventory => write!(f, "inventory"),
            Self::Production => write!(f, "production"),
            Self::Transit => write!(f, "transit"),
        }
    }
}

impl FromStr for LocationUsage {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "supplier" => Ok(Self::Supplier),
            "view" => Ok(Self::View),
            "internal" => Ok(Self::Internal),
            "customer" => Ok(Self::Customer),
            "inventory" => Ok(Self::Inventory),
            "production" => Ok(Self::Production),
            "transit" => Ok(Self::Transit),
            _ => Err(format!("Unknown LocationUsage variant: {}", s)),
        }
    }
}

impl Default for LocationUsage {
    fn default() -> Self {
        Self::Internal
    }
}
