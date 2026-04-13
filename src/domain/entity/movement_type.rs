use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "movement_type", rename_all = "snake_case")]
pub enum MovementType {
    Receipt,
    Issue,
    TransferIn,
    TransferOut,
    AdjustmentIn,
    AdjustmentOut,
    Consumption,
    ReturnIn,
    ReturnOut,
    WriteOff,
}

impl std::fmt::Display for MovementType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Receipt => write!(f, "receipt"),
            Self::Issue => write!(f, "issue"),
            Self::TransferIn => write!(f, "transfer_in"),
            Self::TransferOut => write!(f, "transfer_out"),
            Self::AdjustmentIn => write!(f, "adjustment_in"),
            Self::AdjustmentOut => write!(f, "adjustment_out"),
            Self::Consumption => write!(f, "consumption"),
            Self::ReturnIn => write!(f, "return_in"),
            Self::ReturnOut => write!(f, "return_out"),
            Self::WriteOff => write!(f, "write_off"),
        }
    }
}

impl FromStr for MovementType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "receipt" => Ok(Self::Receipt),
            "issue" => Ok(Self::Issue),
            "transfer_in" => Ok(Self::TransferIn),
            "transfer_out" => Ok(Self::TransferOut),
            "adjustment_in" => Ok(Self::AdjustmentIn),
            "adjustment_out" => Ok(Self::AdjustmentOut),
            "consumption" => Ok(Self::Consumption),
            "return_in" => Ok(Self::ReturnIn),
            "return_out" => Ok(Self::ReturnOut),
            "write_off" => Ok(Self::WriteOff),
            _ => Err(format!("Unknown MovementType variant: {}", s)),
        }
    }
}

impl Default for MovementType {
    fn default() -> Self {
        Self::Receipt
    }
}
