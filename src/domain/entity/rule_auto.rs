use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "rule_auto", rename_all = "snake_case")]
pub enum RuleAuto {
    Manual,
    Transparent,
}

impl std::fmt::Display for RuleAuto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Manual => write!(f, "manual"),
            Self::Transparent => write!(f, "transparent"),
        }
    }
}

impl FromStr for RuleAuto {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "manual" => Ok(Self::Manual),
            "transparent" => Ok(Self::Transparent),
            _ => Err(format!("Unknown RuleAuto variant: {}", s)),
        }
    }
}

impl Default for RuleAuto {
    fn default() -> Self {
        Self::Manual
    }
}
