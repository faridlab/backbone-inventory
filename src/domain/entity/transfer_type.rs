use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "transfer_type", rename_all = "snake_case")]
pub enum TransferType {
    Internal,
    InterProvider,
    ConsignmentOut,
    ConsignmentReturn,
}

impl std::fmt::Display for TransferType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Internal => write!(f, "internal"),
            Self::InterProvider => write!(f, "inter_provider"),
            Self::ConsignmentOut => write!(f, "consignment_out"),
            Self::ConsignmentReturn => write!(f, "consignment_return"),
        }
    }
}

impl FromStr for TransferType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "internal" => Ok(Self::Internal),
            "inter_provider" => Ok(Self::InterProvider),
            "consignment_out" => Ok(Self::ConsignmentOut),
            "consignment_return" => Ok(Self::ConsignmentReturn),
            _ => Err(format!("Unknown TransferType variant: {}", s)),
        }
    }
}

impl Default for TransferType {
    fn default() -> Self {
        Self::Internal
    }
}
