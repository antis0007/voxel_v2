//! Station rows from `assets/stations.ron` and `assets/brewing.ron`.
use crate::world::inventory::{ArmorTier, FoodKind, Item};
use std::sync::LazyLock;

const STATIONS_RON: &str = include_str!("../../../../../assets/stations.ron");
const BREWING_RON: &str = include_str!("../../../../../assets/brewing.ron");

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Repair {
    pub item: Item,
    pub material: Item,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ArmorRepair {
    pub tier: ArmorTier,
    pub material: Item,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Salvage {
    pub item: Item,
    pub material: Item,
    pub count: u32,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Cut {
    pub input: Item,
    pub output: Item,
    pub count: u32,
}

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Stations {
    pub repair_per_unit: f32,
    pub levels_per_unit: u32,
    pub anvil_combine_bonus: f32,
    pub anvil_combine_levels: u32,
    pub grindstone_combine_bonus: f32,
    pub repair: Vec<Repair>,
    pub armor_repair: Vec<ArmorRepair>,
    pub salvage: Vec<Salvage>,
    pub stonecutter: Vec<Cut>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Brew {
    pub base: Item,
    pub modifier: Item,
    pub output: FoodKind,
    pub count: u32,
}

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Brewing {
    pub seconds: f32,
    pub recipes: Vec<Brew>,
}

fn load<T: Default + serde::de::DeserializeOwned>(name: &str, text: &str) -> T {
    ron::from_str(text).unwrap_or_else(|error| {
        eprintln!("[stations] assets/{name}: {error}");
        T::default()
    })
}

/// The assets are compiled in and checked by tests; a parse failure leaves the station inert.
pub static STATIONS: LazyLock<Stations> = LazyLock::new(|| load("stations.ron", STATIONS_RON));
pub static BREWING: LazyLock<Brewing> = LazyLock::new(|| load("brewing.ron", BREWING_RON));

impl Stations {
    /// The material that repairs `item` on an anvil.
    pub fn material(&self, item: Item) -> Option<Item> {
        match item {
            Item::Armor(_, tier) => self.armor_repair.iter().find(|r| r.tier == tier).map(|r| r.material),
            _ => self.repair.iter().find(|r| r.item == item).map(|r| r.material),
        }
    }

    pub fn cuts(&self, input: Item) -> impl Iterator<Item = &Cut> {
        self.stonecutter.iter().filter(move |c| c.input == input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trips<T: serde::Serialize>(table: &T, file: &str, open: &str) {
        let text = ron::ser::to_string_pretty(table, ron::ser::PrettyConfig::new()).unwrap().replace('\r', "");
        let file = file.replace('\r', "");
        let body = file.find(open).map_or(file.as_str(), |i| &file[i + 1..]);
        assert_eq!(text.trim_end(), body.trim_end());
    }

    #[test]
    fn stations_ron_round_trips() {
        round_trips(&*STATIONS, STATIONS_RON, "\n(\n");
        assert!(!STATIONS.repair.is_empty() && !STATIONS.stonecutter.is_empty() && !STATIONS.salvage.is_empty());
    }

    #[test]
    fn brewing_ron_round_trips() {
        round_trips(&*BREWING, BREWING_RON, "\n(\n");
        assert!(BREWING.seconds > 0.0 && !BREWING.recipes.is_empty());
        for row in &BREWING.recipes {
            assert!(row.output.is_potion(), "{row:?}");
            assert!(crate::world::inventory::food::get(row.output).is_some_and(|f| !f.effects.is_empty()));
        }
    }

    #[test]
    #[ignore]
    fn dump_stations_ron() {
        println!("{}", ron::ser::to_string_pretty(&*STATIONS, ron::ser::PrettyConfig::new()).unwrap());
        println!("{}", ron::ser::to_string_pretty(&*BREWING, ron::ser::PrettyConfig::new()).unwrap());
    }
}
