use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "product_type", rename_all = "snake_case")]
pub enum ProductType {
    RawMaterial,
    FinishedGood,
    Consumable,
    SparePart,
    Packaging,
}

impl std::fmt::Display for ProductType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RawMaterial => write!(f, "raw_material"),
            Self::FinishedGood => write!(f, "finished_good"),
            Self::Consumable => write!(f, "consumable"),
            Self::SparePart => write!(f, "spare_part"),
            Self::Packaging => write!(f, "packaging"),
        }
    }
}

impl FromStr for ProductType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "raw_material" => Ok(Self::RawMaterial),
            "finished_good" => Ok(Self::FinishedGood),
            "consumable" => Ok(Self::Consumable),
            "spare_part" => Ok(Self::SparePart),
            "packaging" => Ok(Self::Packaging),
            _ => Err(format!("Unknown ProductType variant: {}", s)),
        }
    }
}

impl Default for ProductType {
    fn default() -> Self {
        Self::RawMaterial
    }
}
