use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "picking_code", rename_all = "snake_case")]
pub enum PickingCode {
    Incoming,
    Outgoing,
    Internal,
}

impl std::fmt::Display for PickingCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Incoming => write!(f, "incoming"),
            Self::Outgoing => write!(f, "outgoing"),
            Self::Internal => write!(f, "internal"),
        }
    }
}

impl FromStr for PickingCode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "incoming" => Ok(Self::Incoming),
            "outgoing" => Ok(Self::Outgoing),
            "internal" => Ok(Self::Internal),
            _ => Err(format!("Unknown PickingCode variant: {}", s)),
        }
    }
}

impl Default for PickingCode {
    fn default() -> Self {
        Self::Incoming
    }
}
