use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "rule_action", rename_all = "snake_case")]
pub enum RuleAction {
    Pull,
    Push,
    PullPush,
}

impl std::fmt::Display for RuleAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pull => write!(f, "pull"),
            Self::Push => write!(f, "push"),
            Self::PullPush => write!(f, "pull_push"),
        }
    }
}

impl FromStr for RuleAction {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pull" => Ok(Self::Pull),
            "push" => Ok(Self::Push),
            "pull_push" => Ok(Self::PullPush),
            _ => Err(format!("Unknown RuleAction variant: {}", s)),
        }
    }
}

impl Default for RuleAction {
    fn default() -> Self {
        Self::Pull
    }
}
