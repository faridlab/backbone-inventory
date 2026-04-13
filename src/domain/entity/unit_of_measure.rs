use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "unit_of_measure", rename_all = "snake_case")]
pub enum UnitOfMeasure {
    Pcs,
    Kg,
    G,
    Liter,
    Ml,
    Pack,
    Box,
    Bottle,
    Gallon,
    Roll,
    Sheet,
}

impl std::fmt::Display for UnitOfMeasure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pcs => write!(f, "pcs"),
            Self::Kg => write!(f, "kg"),
            Self::G => write!(f, "g"),
            Self::Liter => write!(f, "liter"),
            Self::Ml => write!(f, "ml"),
            Self::Pack => write!(f, "pack"),
            Self::Box => write!(f, "box"),
            Self::Bottle => write!(f, "bottle"),
            Self::Gallon => write!(f, "gallon"),
            Self::Roll => write!(f, "roll"),
            Self::Sheet => write!(f, "sheet"),
        }
    }
}

impl FromStr for UnitOfMeasure {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pcs" => Ok(Self::Pcs),
            "kg" => Ok(Self::Kg),
            "g" => Ok(Self::G),
            "liter" => Ok(Self::Liter),
            "ml" => Ok(Self::Ml),
            "pack" => Ok(Self::Pack),
            "box" => Ok(Self::Box),
            "bottle" => Ok(Self::Bottle),
            "gallon" => Ok(Self::Gallon),
            "roll" => Ok(Self::Roll),
            "sheet" => Ok(Self::Sheet),
            _ => Err(format!("Unknown UnitOfMeasure variant: {}", s)),
        }
    }
}

impl Default for UnitOfMeasure {
    fn default() -> Self {
        Self::Pcs
    }
}
