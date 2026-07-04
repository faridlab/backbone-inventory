use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "stock_entry_type", rename_all = "snake_case")]
pub enum StockEntryType {
    Transfer,
    Repack,
    MaterialIssue,
    MaterialReceipt,
}

impl std::fmt::Display for StockEntryType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Transfer => write!(f, "transfer"),
            Self::Repack => write!(f, "repack"),
            Self::MaterialIssue => write!(f, "material_issue"),
            Self::MaterialReceipt => write!(f, "material_receipt"),
        }
    }
}

impl FromStr for StockEntryType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "transfer" => Ok(Self::Transfer),
            "repack" => Ok(Self::Repack),
            "material_issue" => Ok(Self::MaterialIssue),
            "material_receipt" => Ok(Self::MaterialReceipt),
            _ => Err(format!("Unknown StockEntryType variant: {}", s)),
        }
    }
}

impl Default for StockEntryType {
    fn default() -> Self {
        Self::Transfer
    }
}
