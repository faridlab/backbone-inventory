use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "service_tracking_type", rename_all = "snake_case")]
pub enum ServiceTrackingType {
    TaskGlobalProject,
    TaskInProject,
    ProjectOnly,
    Manual,
}

impl std::fmt::Display for ServiceTrackingType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TaskGlobalProject => write!(f, "task_global_project"),
            Self::TaskInProject => write!(f, "task_in_project"),
            Self::ProjectOnly => write!(f, "project_only"),
            Self::Manual => write!(f, "manual"),
        }
    }
}

impl FromStr for ServiceTrackingType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "task_global_project" => Ok(Self::TaskGlobalProject),
            "task_in_project" => Ok(Self::TaskInProject),
            "project_only" => Ok(Self::ProjectOnly),
            "manual" => Ok(Self::Manual),
            _ => Err(format!("Unknown ServiceTrackingType variant: {}", s)),
        }
    }
}

impl Default for ServiceTrackingType {
    fn default() -> Self {
        Self::Manual
    }
}
