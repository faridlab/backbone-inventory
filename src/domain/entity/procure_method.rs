use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "procure_method", rename_all = "snake_case")]
pub enum ProcureMethod {
    MakeToStock,
    MakeToOrder,
    MtsElseMto,
}

impl std::fmt::Display for ProcureMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MakeToStock => write!(f, "make_to_stock"),
            Self::MakeToOrder => write!(f, "make_to_order"),
            Self::MtsElseMto => write!(f, "mts_else_mto"),
        }
    }
}

impl FromStr for ProcureMethod {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "make_to_stock" => Ok(Self::MakeToStock),
            "make_to_order" => Ok(Self::MakeToOrder),
            "mts_else_mto" => Ok(Self::MtsElseMto),
            _ => Err(format!("Unknown ProcureMethod variant: {}", s)),
        }
    }
}

impl Default for ProcureMethod {
    fn default() -> Self {
        Self::MakeToStock
    }
}
