use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "move_type", rename_all = "snake_case")]
pub enum MoveType {
    Direct,
    One,
}

impl std::fmt::Display for MoveType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Direct => write!(f, "direct"),
            Self::One => write!(f, "one"),
        }
    }
}

impl FromStr for MoveType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "direct" => Ok(Self::Direct),
            "one" => Ok(Self::One),
            _ => Err(format!("Unknown MoveType variant: {}", s)),
        }
    }
}

impl Default for MoveType {
    fn default() -> Self {
        Self::Direct
    }
}
