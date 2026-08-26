use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "reservation_method", rename_all = "snake_case")]
pub enum ReservationMethod {
    AtConfirm,
    Manual,
    ByDate,
}

impl std::fmt::Display for ReservationMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AtConfirm => write!(f, "at_confirm"),
            Self::Manual => write!(f, "manual"),
            Self::ByDate => write!(f, "by_date"),
        }
    }
}

impl FromStr for ReservationMethod {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "at_confirm" => Ok(Self::AtConfirm),
            "manual" => Ok(Self::Manual),
            "by_date" => Ok(Self::ByDate),
            _ => Err(format!("Unknown ReservationMethod variant: {}", s)),
        }
    }
}

impl Default for ReservationMethod {
    fn default() -> Self {
        Self::AtConfirm
    }
}
