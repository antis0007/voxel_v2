//! Crop rows loaded from `assets/crops.ron`.
use crate::world::BlockId;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Crop {
    pub crop: BlockId,
    pub sprout: BlockId,
    pub soil: Vec<BlockId>,
    pub light_min: u8,
    pub light_max: u8,
    pub stages: u8,
    pub roll_ticks: u32,
    pub dry_per_mille: u32,
    pub wet_per_mille: u32,
    pub yield_min: u32,
    pub yield_max: u32,
    pub wild_seed_per_mille: u32,
    /// A mature plot becomes this tree instead of swapping to `crop`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tree: Option<TreeShape>,
    /// A mature plot adds one `crop` cell on top instead of ripening once.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stalk: Option<Stalk>,
    /// Water must be within this horizontal radius, down to one metre below.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub irrigation_metres: Option<f32>,
    /// Where the plant cell sits relative to its support.
    #[serde(default, skip_serializing_if = "Planting::is_soil")]
    pub planting: Planting,
    /// Use on a ripe plant picks it back to `sprout` and keeps the plot.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub regrow: bool,
    /// A mature stem sets one `crop` block on free ground beside it, then grows again.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub gourd: bool,
    /// Harvest pays this resource index instead of `crop`, plus one `crop` as seed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub produce: Option<u8>,
    /// Eating one `crop` item restores these.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub food: Option<Bite>,
    /// Work a mature plant does around it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aura: Option<Aura>,
    /// A mature working flower plants a copy on free soil beside it, this chance per 2 s beat.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub spread_per_mille: u32,
}

fn is_zero(v: &u32) -> bool {
    *v == 0
}

/// Placement of the plant cell; `soil` is the block it rests on or hangs from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Planting {
    /// Air cell on top of soil.
    #[default]
    Soil,
    /// Water cell on top of soil, with water above.
    Submerged,
    /// Water cell on top of soil, with air above (paddies, shallows).
    Paddy,
    /// Air cell on the water surface; `soil` lists the water block.
    Floating,
    /// Air cell beside a soil block, on any of its four sides.
    Side,
    /// Air cell under a soil block.
    Hanging,
    /// Set only by a grown tree's crown; never planted from an item.
    Grafted,
}

impl Planting {
    pub fn is_soil(&self) -> bool {
        *self == Self::Soil
    }
    /// Offsets from the plant cell to the cells that may support it.
    pub fn supports(self) -> &'static [glam::IVec3] {
        use glam::IVec3 as V;
        const BELOW: [V; 1] = [V::NEG_Y];
        const ABOVE: [V; 1] = [V::Y];
        const SIDES: [V; 4] = [V::X, V::NEG_X, V::Z, V::NEG_Z];
        const CROWN: [V; 6] = [V::Y, V::NEG_Y, V::X, V::NEG_X, V::Z, V::NEG_Z];
        match self {
            Self::Side => &SIDES,
            Self::Hanging => &ABOVE,
            Self::Grafted => &CROWN,
            _ => &BELOW,
        }
    }
    /// The block the plant cell holds before planting.
    pub fn medium(self) -> BlockId {
        if matches!(self, Self::Submerged | Self::Paddy) { BlockId::Water } else { BlockId::Air }
    }
}

/// Hunger, health and saturation one eaten item restores.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Bite {
    pub hunger: f32,
    pub health: f32,
    pub saturation: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Aura {
    /// Open-sky wand mana per second to a player within `radius_metres`.
    /// With `feed`, it also eats loose items and turns each into mana.
    Mana {
        per_second: f32,
        radius_metres: f32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        feed: Option<Feed>,
    },
    /// Moves loose stacks within `radius_metres` into a touching chest.
    Collect { radius_metres: f32 },
    /// Harvests and replants ripe plots within `radius_metres` into a touching chest.
    Harvest { radius_metres: f32 },
    /// Each second, spends `mana` from the nearest habitat receiver within
    /// `radius_metres` to bring one growing plot's next roll `ticks` sooner.
    Tend { radius_metres: f32, ticks: u32, mana: u32 },
    /// Growing plots within `radius_metres` count as watered.
    Mist { radius_metres: f32 },
}

/// Items a mana flower eats: one loose `item` within `reach_metres` per mana pulse,
/// worth `mana_per_item`. Eaten only when a receiver or player can take the mana.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Feed {
    pub item: crate::world::inventory::Item,
    pub mana_per_item: u32,
    pub reach_metres: f32,
}

/// Column crop such as bamboo; height is metres so cells follow the voxel scale.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Stalk {
    pub max_metres: f32,
}

/// Trunk and crown of a grown sapling, metres; cells follow the voxel scale.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TreeShape {
    pub trunk_metres: f32,
    pub crown_radius_metres: f32,
    pub crown_half_height_metres: f32,
    /// Crown cells that bear this crop row's fruit, one in `fruit_every` leaf cells.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fruit: Option<BlockId>,
    /// Prop roots arching from the trunk base to the ground.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub roots: Option<BlockId>,
}

impl Crop {
    pub fn mature(&self) -> u8 {
        self.stages.saturating_sub(1)
    }
}

const CROPS_RON: &str = include_str!("../../../../../assets/crops.ron");

/// Rows in file order; a plot stores its row index.
/// The asset is compiled in and checked by a test; a parse failure leaves no crop plantable.
pub static CROPS: std::sync::LazyLock<Vec<Crop>> = std::sync::LazyLock::new(|| {
    ron::from_str(CROPS_RON).unwrap_or_else(|error| {
        eprintln!("[farming] assets/crops.ron: {error}");
        Vec::new()
    })
});

pub fn by_seed(block: BlockId) -> Option<(u8, &'static Crop)> {
    CROPS.iter().enumerate()
        .find(|(_, c)| c.crop == block && c.planting != Planting::Grafted)
        .map(|(i, c)| (i as u8, c))
}

/// The grafted row a tree's fruit cell grows from.
pub fn grafted(fruit: BlockId) -> Option<(u8, &'static Crop)> {
    CROPS.iter().enumerate()
        .find(|(_, c)| c.crop == fruit && c.planting == Planting::Grafted)
        .map(|(i, c)| (i as u8, c))
}

/// The row whose ripe `block` regrows after picking; grafted fruit rows come first.
pub fn regrowing(block: BlockId) -> Option<(u8, &'static Crop)> {
    let rows = || CROPS.iter().enumerate().filter(|(_, c)| c.regrow && c.crop == block);
    rows().find(|(_, c)| c.planting == Planting::Grafted).or_else(|| rows().next())
        .map(|(i, c)| (i as u8, c))
}

/// What eating one `block` item restores, from the first row that lists it.
pub fn bite(block: BlockId) -> Option<Bite> {
    CROPS.iter().filter(|c| c.crop == block).find_map(|c| c.food)
}

pub fn get(index: u8) -> Option<&'static Crop> {
    CROPS.get(index as usize)
}

#[cfg(test)]
mod crop_table {
    use super::*;

    #[test]
    fn crops_ron_round_trips() {
        let text = ron::ser::to_string_pretty(&*CROPS, ron::ser::PrettyConfig::new()).unwrap().replace('\r', "");
        let file = CROPS_RON.replace('\r', "");
        let body = file.find("\n[\n").map_or(file.as_str(), |i| &file[i + 1..]);
        assert_eq!(text.trim_end(), body.trim_end(), "rerun dump_crops_ron");
    }

    #[test]
    fn crop_rows_are_growable_and_unique() {
        assert!(!CROPS.is_empty());
        for (i, c) in CROPS.iter().enumerate() {
            assert!(c.stages >= 2 && c.roll_ticks > 0 && !c.soil.is_empty(), "row {i}");
            assert!(c.light_min <= c.light_max && c.light_max <= 15, "row {i}");
            assert!(c.dry_per_mille <= 1000 && c.wet_per_mille <= 1000, "row {i}");
            assert!(c.yield_min >= 1 && c.yield_min <= c.yield_max, "row {i}");
            assert!(c.tree.is_some() || c.crop != c.sprout || c.planting == Planting::Floating || c.aura.is_some(), "row {i}");
            let plantable = |o: &Crop| o.planting != Planting::Grafted;
            assert!(!plantable(c) || CROPS[..i].iter().filter(|o| plantable(o)).all(|o| o.crop != c.crop),
                "row {i} duplicates a seed");
            if let Some(fruit) = c.tree.and_then(|t| t.fruit) {
                assert!(grafted(fruit).is_some(), "row {i} bears {fruit:?} without a grafted row");
            }
        }
    }

    #[test]
    #[ignore = "writes assets/crops.ron"]
    fn dump_crops_ron() {
        let header = CROPS_RON.split("\n[").next().unwrap_or("");
        let text = ron::ser::to_string_pretty(&*CROPS, ron::ser::PrettyConfig::new()).unwrap();
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/crops.ron");
        std::fs::write(path, format!("{header}\n{text}\n")).unwrap();
    }
}
