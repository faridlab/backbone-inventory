use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "putaway_sublocation", rename_all = "snake_case")]
pub enum PutawaySublocation {
    No,
    LastUsed,
    ClosestLocation,
}

impl std::fmt::Display for PutawaySublocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::No => write!(f, "no"),
            Self::LastUsed => write!(f, "last_used"),
            Self::ClosestLocation => write!(f, "closest_location"),
        }
    }
}

impl FromStr for PutawaySublocation {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "no" => Ok(Self::No),
            "last_used" => Ok(Self::LastUsed),
            "closest_location" => Ok(Self::ClosestLocation),
            _ => Err(format!("Unknown PutawaySublocation variant: {}", s)),
        }
    }
}

impl Default for PutawaySublocation {
    fn default() -> Self {
        Self::No
    }
}
