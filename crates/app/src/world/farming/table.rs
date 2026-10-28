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
}

impl Crop {
    pub fn mature(&self) -> u8 {
        self.stages.saturating_sub(1)
    }
}

const CROPS_RON: &str = include_str!("../../../../../assets/crops.ron");

/// Rows in file order; a plot stores its row index.
pub static CROPS: std::sync::LazyLock<Vec<Crop>> = std::sync::LazyLock::new(|| {
    ron::from_str(CROPS_RON).unwrap_or_else(|error| panic!("assets/crops.ron: {error}"))
});

pub fn by_seed(block: BlockId) -> Option<(u8, &'static Crop)> {
    CROPS.iter().enumerate().find(|(_, c)| c.crop == block).map(|(i, c)| (i as u8, c))
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
            assert_ne!(c.crop, c.sprout, "row {i}");
            assert!(CROPS[..i].iter().all(|o| o.crop != c.crop), "row {i} duplicates a seed");
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
