//! The shipped held wand plus opt-in art candidates; no world/save data changes.
use flate2::read::GzDecoder;
use std::io::Read;
use std::sync::LazyLock;

const ATLAS_BYTES: usize = 256 * 256 * 4;

fn unpack_atlas(compressed: &[u8]) -> Box<[u8; ATLAS_BYTES]> {
    let mut pixels = Vec::with_capacity(ATLAS_BYTES);
    GzDecoder::new(compressed)
        .read_to_end(&mut pixels)
        .expect("embedded viewmodel atlas");
    pixels.into_boxed_slice().try_into().ok().expect("viewmodel atlas size")
}

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
pub static DEFAULT_ATLAS: LazyLock<Box<[u8; ATLAS_BYTES]>> = LazyLock::new(|| {
    unpack_atlas(include_bytes!("../../../assets/runtime/viewmodels/artificer_tally/atlas.rgba.gz"))
});
pub const DEFAULT_HAND: &[u8] = include_bytes!("../../../assets/runtime/viewmodels/hand_v6/hand.glb");
pub static DEFAULT_HAND_ATLAS: LazyLock<Box<[u8; ATLAS_BYTES]>> = LazyLock::new(|| {
    unpack_atlas(include_bytes!("../../../assets/runtime/viewmodels/hand_v6/atlas.rgba.gz"))
});

pub fn current() -> &'static Item { selected().unwrap_or(&DEFAULT) }

pub static ATLAS: LazyLock<Box<[u8; ATLAS_BYTES]>> = LazyLock::new(|| {
    unpack_atlas(include_bytes!("../../../assets/source/wand_ecosystem/minimal_v2/atlas.rgba.gz"))
});
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
