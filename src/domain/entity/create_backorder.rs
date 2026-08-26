use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "create_backorder", rename_all = "snake_case")]
pub enum CreateBackorder {
    Ask,
    Always,
    Never,
}

impl std::fmt::Display for CreateBackorder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ask => write!(f, "ask"),
            Self::Always => write!(f, "always"),
            Self::Never => write!(f, "never"),
        }
    }
}

impl FromStr for CreateBackorder {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "ask" => Ok(Self::Ask),
            "always" => Ok(Self::Always),
            "never" => Ok(Self::Never),
            _ => Err(format!("Unknown CreateBackorder variant: {}", s)),
        }
    }
}

impl Default for CreateBackorder {
    fn default() -> Self {
        Self::Ask
    }
}
