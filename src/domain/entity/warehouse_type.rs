use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "warehouse_type", rename_all = "snake_case")]
pub enum WarehouseType {
    Main,
    Outlet,
    Transit,
    Production,
    ReturnStorage,
    Quarantine,
}

impl std::fmt::Display for WarehouseType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Main => write!(f, "main"),
            Self::Outlet => write!(f, "outlet"),
            Self::Transit => write!(f, "transit"),
            Self::Production => write!(f, "production"),
            Self::ReturnStorage => write!(f, "return_storage"),
            Self::Quarantine => write!(f, "quarantine"),
        }
    }
}

impl FromStr for WarehouseType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "main" => Ok(Self::Main),
            "outlet" => Ok(Self::Outlet),
            "transit" => Ok(Self::Transit),
            "production" => Ok(Self::Production),
            "return_storage" => Ok(Self::ReturnStorage),
            "quarantine" => Ok(Self::Quarantine),
            _ => Err(format!("Unknown WarehouseType variant: {}", s)),
        }
    }
}

impl Default for WarehouseType {
    fn default() -> Self {
        Self::Main
    }
}
