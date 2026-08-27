use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "storage_allow_new_product", rename_all = "snake_case")]
pub enum StorageAllowNewProduct {
    Mixed,
    Empty,
    Same,
}

impl std::fmt::Display for StorageAllowNewProduct {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Mixed => write!(f, "mixed"),
            Self::Empty => write!(f, "empty"),
            Self::Same => write!(f, "same"),
        }
    }
}

impl FromStr for StorageAllowNewProduct {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "mixed" => Ok(Self::Mixed),
            "empty" => Ok(Self::Empty),
            "same" => Ok(Self::Same),
            _ => Err(format!("Unknown StorageAllowNewProduct variant: {}", s)),
        }
    }
}

impl Default for StorageAllowNewProduct {
    fn default() -> Self {
        Self::Mixed
    }
}
