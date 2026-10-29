//! Durability and mining XP rows from `assets/progression.ron`.
use crate::world::BlockId;
use crate::world::inventory::{ArmorTier, Item};
use std::sync::LazyLock;

const RON: &str = include_str!("../../../../../assets/progression.ron");

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ItemRow {
    pub item: Item,
    pub max: u32,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ArmorRow {
    pub tier: ArmorTier,
    pub max: u32,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BlockXp {
    pub block: BlockId,
    pub xp: u32,
}

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Table {
    /// Tools and weapons; armour durability is per tier and shared by every slot.
    pub durability: Vec<ItemRow>,
    pub armor: Vec<ArmorRow>,
    pub mined_xp: Vec<BlockXp>,
}

/// The asset is compiled in and checked by a test; a parse failure leaves every item unbreakable.
pub static TABLE: LazyLock<Table> = LazyLock::new(|| {
    ron::from_str(RON).unwrap_or_else(|error| {
        eprintln!("[progression] assets/progression.ron: {error}");
        Table::default()
    })
});

pub fn max_durability(item: Item) -> Option<u32> {
    match item {
        Item::Armor(_, tier) => TABLE.armor.iter().find(|r| r.tier == tier).map(|r| r.max),
        _ => TABLE.durability.iter().find(|r| r.item == item).map(|r| r.max),
    }
    .filter(|&max| max > 0)
}

pub fn mined_xp(block: BlockId) -> u32 {
    TABLE.mined_xp.iter().find(|r| r.block == block).map_or(0, |r| r.xp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progression_ron_round_trips() {
        let text = ron::ser::to_string_pretty(&*TABLE, ron::ser::PrettyConfig::new()).unwrap().replace('\r', "");
        let file = RON.replace('\r', "");
        let body = file.find("\n(\n").map_or(file.as_str(), |i| &file[i + 1..]);
        assert_eq!(text.trim_end(), body.trim_end());
        assert!(!TABLE.durability.is_empty() && !TABLE.armor.is_empty() && !TABLE.mined_xp.is_empty());
    }

    #[test]
    #[ignore]
    fn dump_progression_ron() {
        println!("{}", ron::ser::to_string_pretty(&*TABLE, ron::ser::PrettyConfig::new()).unwrap());
    }
}
