use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "qc_result", rename_all = "snake_case")]
pub enum QCResult {
    Pending,
    Passed,
    Failed,
    Conditional,
}

impl std::fmt::Display for QCResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Passed => write!(f, "passed"),
            Self::Failed => write!(f, "failed"),
            Self::Conditional => write!(f, "conditional"),
        }
    }
}

impl FromStr for QCResult {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(Self::Pending),
            "passed" => Ok(Self::Passed),
            "failed" => Ok(Self::Failed),
            "conditional" => Ok(Self::Conditional),
            _ => Err(format!("Unknown QCResult variant: {}", s)),
        }
    }
}

impl Default for QCResult {
    fn default() -> Self {
        Self::Pending
    }
}
