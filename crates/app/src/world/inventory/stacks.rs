//! Stack limits per item class, loaded from `assets/stacks.ron`.
use super::Item;
use crate::world::BlockId;

/// Which preset column decides an item's stack limit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum StackClass {
    /// Carries per-item state (wear, spells, charge, contents); never merges past one.
    Unique,
    /// Stations, caches and model blocks other than pipes, cables and bricks.
    Assembly,
    Component,
    Material,
    Small,
    Food,
    Potion,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Preset {
    pub name: String,
    pub assembly: u32,
    pub component: u32,
    pub material: u32,
    pub small: u32,
    pub food: u32,
    pub potion: u32,
    /// Exact encoded item identities; unique equipment still has a cap of one.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub overrides: Vec<((u32, u32), u32)>,
}

impl Preset {
    pub fn item_limit(&self, item: Item) -> u32 {
        if class(item) == StackClass::Unique { return 1; }
        self.overrides.iter().find(|(key, _)| *key == item.encode_u32())
            .map_or_else(|| self.limit(class(item)), |(_, cap)| (*cap).clamp(1, 16384))
    }
    pub fn limit(&self, class: StackClass) -> u32 {
        match class {
            StackClass::Unique => 1,
            StackClass::Assembly => self.assembly,
            StackClass::Component => self.component,
            StackClass::Material => self.material,
            StackClass::Small => self.small,
            StackClass::Food => self.food,
            StackClass::Potion => self.potion,
        }
        .max(1)
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
struct Table {
    active: String,
    /// Resource ids whose class differs from `Material`.
    resources: Vec<(u8, StackClass)>,
    presets: Vec<Preset>,
}

const STACKS_RON: &str = include_str!("../../../../../assets/stacks.ron");

static TABLE: std::sync::LazyLock<Table> = std::sync::LazyLock::new(|| {
    let table: Table = ron::from_str(STACKS_RON).unwrap_or_else(|error| panic!("assets/stacks.ron: {error}"));
    assert!(!table.presets.is_empty(), "assets/stacks.ron: no presets");
    table
});

pub fn preset(name: &str) -> &'static Preset {
    TABLE.presets.iter().find(|p| p.name == name).unwrap_or(&TABLE.presets[0])
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Balance {
    Classic,
    #[default]
    Balanced,
    Builder,
}
impl Balance {
    pub const ALL: [Self; 3] = [Self::Classic, Self::Balanced, Self::Builder];
    pub fn name(self) -> &'static str { match self { Self::Classic => "Classic", Self::Balanced => "Balanced", Self::Builder => "Builder" } }
    pub fn code(self) -> u32 { match self { Self::Classic => 0, Self::Balanced => 1, Self::Builder => 2 } }
    pub fn decode(n: u32) -> anyhow::Result<Self> {
        Self::ALL.get(n as usize).copied().ok_or_else(|| anyhow::anyhow!("Invalid stack preset"))
    }
    pub fn limit(self, item: Item, voxel_m: f32) -> u32 {
        let multiplier = if matches!(item, Item::Block(b) if !b.is_model()) { 64 / volume_units(voxel_m) } else { 1 };
        preset(self.name()).item_limit(item).saturating_mul(multiplier)
    }
}

/// Integer minimum-volume units: 1/64 cubic metre. Supported world scales
/// are exactly 1, 1/2 and 1/4 metre; brush size counts occupied cells.
pub fn volume_units(voxel_m: f32) -> u32 {
    if voxel_m <= 0.25 { 1 } else if voxel_m <= 0.5 { 8 } else { 64 }
}

/// The preset in play; every client compiles the same table.
pub fn active() -> &'static Preset {
    preset(&TABLE.active)
}

/// Pre-overhaul limits; recipe bench rules stay on these.
pub fn classic() -> &'static Preset {
    preset("Classic")
}

pub fn class(item: Item) -> StackClass {
    match item {
        Item::Assembled { .. } | Item::Cast(_) | Item::Bucket(_) | Item::PowerKit(_) => StackClass::Unique,
        Item::Weapon(_) | Item::Armor(_, _) | Item::Tool(_) | Item::Wand { .. } => StackClass::Unique,
        Item::Part { .. } => StackClass::Component,
        Item::Food(food) if food.is_potion() => StackClass::Potion,
        Item::Food(_) => StackClass::Food,
        Item::Resource(id) => TABLE
            .resources
            .iter()
            .find(|(r, _)| *r == id)
            .map_or(StackClass::Material, |(_, class)| *class),
        Item::Block(block) => {
            let station = matches!(
                block,
                BlockId::Workbench | BlockId::Smelter | BlockId::SalvageCrate | BlockId::SurveyCache | BlockId::CultivatorCache
            );
            let bulk_model = matches!(
                block,
                BlockId::ItemPipe | BlockId::PowerCable | BlockId::FluidPipe | BlockId::CokeOvenBrick | BlockId::BlastBrick
            );
            if station || (block.is_model() && !bulk_model) { StackClass::Assembly } else { StackClass::Material }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::inventory::{ArmorTier, EquipSlot, FoodKind};

    #[test]
    fn stacks_ron_round_trips() {
        let text = ron::ser::to_string_pretty(&*TABLE, ron::ser::PrettyConfig::new()).unwrap().replace('\r', "");
        let file = STACKS_RON.replace('\r', "");
        let body = file.find("\n(\n").map_or(file.as_str(), |i| &file[i + 1..]);
        assert_eq!(text.trim_end(), body.trim_end(), "rerun dump_stacks_ron");
    }

    #[test]
    #[ignore]
    fn dump_stacks_ron() {
        println!("{}", ron::ser::to_string_pretty(&*TABLE, ron::ser::PrettyConfig::new()).unwrap());
    }

    #[test]
    fn active_preset_exists_and_classic_keeps_old_limits() {
        assert!(TABLE.presets.iter().any(|p| p.name == TABLE.active));
        let c = classic();
        assert_eq!(c.name, "Classic");
        assert_eq!(c.limit(class(Item::Block(BlockId::Cobblestone))), 64);
        assert_eq!(c.limit(class(Item::Block(BlockId::Workbench))), 16);
        assert_eq!(c.limit(class(Item::Resource(6))), 64);
        assert_eq!(c.limit(class(Item::Food(FoodKind::FungalRation))), 16);
    }

    /// Built-in presets retain the expected progression in carrying capacity.
    #[test]
    fn every_preset_is_at_least_classic() {
        let c = classic();
        for p in &TABLE.presets {
            for class in [StackClass::Assembly, StackClass::Component, StackClass::Material, StackClass::Small, StackClass::Food, StackClass::Potion] {
                assert!(p.limit(class) >= c.limit(class), "{} {class:?}", p.name);
            }
        }
    }

    #[test]
    fn stateful_items_never_stack() {
        for item in [
            Item::Wand { id: 3, look: 1 },
            Item::Tool(8),
            Item::Weapon(2),
            Item::Armor(EquipSlot::Head, ArmorTier::Steel),
            Item::Bucket(5),
            Item::Assembled { id: 1, schema: 0 },
        ] {
            assert_eq!(item.max_stack(), 1, "{item:?}");
        }
    }

    #[test]
    fn balanced_limits() {
        assert_eq!(Item::Block(BlockId::Cobblestone).max_stack(), active().material);
        assert_eq!(Item::Resource(9).max_stack(), active().small);
        assert_eq!(Item::Resource(12).max_stack(), active().component);
        assert_eq!(Item::Food(FoodKind::FungalRation).max_stack(), active().food);
    }

    #[test]
    fn exact_overrides_preserve_unique_identity_limits() {
        let mut rules = active().clone();
        let copper = Item::Resource(6);
        let wand = Item::Wand { id: 3, look: 1 };
        rules.overrides = vec![(copper.encode_u32(), 512), (wand.encode_u32(), 512)];
        assert_eq!(rules.item_limit(copper), 512);
        assert_eq!(rules.item_limit(wand), 1);
        assert_eq!(rules.item_limit(Item::Resource(7)), rules.material);
    }
}
