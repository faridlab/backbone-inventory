use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "package_use", rename_all = "snake_case")]
pub enum PackageUse {
    Disposable,
    Reusable,
}

impl std::fmt::Display for PackageUse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Disposable => write!(f, "disposable"),
            Self::Reusable => write!(f, "reusable"),
        }
    }
}

impl FromStr for PackageUse {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "disposable" => Ok(Self::Disposable),
            "reusable" => Ok(Self::Reusable),
            _ => Err(format!("Unknown PackageUse variant: {}", s)),
        }
    }
}

impl Default for PackageUse {
    fn default() -> Self {
        Self::Disposable
    }
}
