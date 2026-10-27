//! The shipped held wand plus opt-in art candidates; no world/save data changes.
use std::sync::LazyLock;

pub struct Item {
    pub name: &'static str,
    pub geometry: &'static [u8],
    pub muzzle: [f32; 3],
}

pub const DEFAULT: Item = Item {
    name: "Artificer tally wand",
    geometry: include_bytes!("../../../assets/runtime/viewmodels/artificer_tally/wand.glb"),
    muzzle: [0.006, 0.338, 0.0],
};
pub const DEFAULT_ATLAS: &[u8; 256 * 256 * 4] = include_bytes!("../../../assets/runtime/viewmodels/artificer_tally/atlas.rgba");
pub const DEFAULT_HAND: &[u8] = include_bytes!("../../../assets/runtime/viewmodels/hand_v5/hand.glb");
pub const DEFAULT_HAND_ATLAS: &[u8; 256 * 256 * 4] = include_bytes!("../../../assets/runtime/viewmodels/hand_v5/atlas.rgba");

pub fn current() -> &'static Item { selected().unwrap_or(&DEFAULT) }

pub const ATLAS: &[u8; 256 * 256 * 4] = include_bytes!("../../../assets/source/wand_ecosystem/minimal_v2/atlas.rgba");
pub const HAND_ATLAS: &[u8; 256 * 256 * 4] = include_bytes!("../../../assets/source/wand_ecosystem/minimal_v2/hand_atlas.rgba");
pub const HAND: &[u8] = include_bytes!("../../../assets/source/wand_ecosystem/hand_v3/plane_hand.glb");

pub fn selected() -> Option<&'static Item> {
    static ITEM: LazyLock<Option<Item>> = LazyLock::new(|| {
        if !std::env::args().any(|arg| arg.starts_with("--benchmark-")) { return None; }
        let name = std::env::var("VOXEL_REVIEW_ITEM").ok()?;
        let item = match name.as_str() {
            "" => return None,
            "artificer_tally" => DEFAULT,
            "bone_needle" => Item {
                name: "bone_needle",
                geometry: include_bytes!("../../../assets/source/wand_ecosystem/minimal_v2/bone_needle.glb"),
                muzzle: [-0.013, 0.396, 0.0],
            },
            "runic_key" => Item {
                name: "runic_key",
                geometry: include_bytes!("../../../assets/source/wand_ecosystem/minimal_v2/runic_key.glb"),
                muzzle: [0.0, 0.351, 0.0],
            },
            "cinder_rod" => Item {
                name: "cinder_rod",
                geometry: include_bytes!("../../../assets/source/wand_ecosystem/minimal_v2/cinder_rod.glb"),
                muzzle: [-0.003, 0.38165, 0.0],
            },
            _ => { eprintln!("[viewmodel-review] unknown item {name:?}; keeping normal kit"); return None; }
        };
        eprintln!("[viewmodel-review] {} (diagnostic only)", item.name);
        Some(item)
    });
    ITEM.as_ref()
}
