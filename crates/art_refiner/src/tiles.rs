//! Tile lists: block names and presets resolved through engine_content.
use anyhow::{Result, bail};
use engine_content::{ALL_BLOCK_IDS, BlockId};
use glam::IVec3;

pub const PRESETS: &[(&str, &str)] = &[
    ("stones", "stone,cobblestone,gravel,basalt,limestone,sandstone,stonebricks,weatheredbricks"),
    ("ground", "grass,dirt,mud,clay,sand,snowgrass,ice,ash"),
    ("ores", "coalore,ironore,copperore,tinore,goldquartz,crystal,voidstone,stone"),
    ("wood", "planks,log,timberpost,timberbeamx,rooftiles,limeplaster,bookcase,ladder"),
    ("industry", "concrete,steelplate,rustedsteel,hazardpaint,windowglass,stonebricks,chest,workbench"),
    ("metals", "copperblock,silverblock,goldblock,platinumblock,mithrilblock,workbench,smelter,crusher"),
    ("gilded", "ochresandstone,grottostone,goldcoins,jeweledgold,zigzagtile,lozengetile,startile,lapisbrick,gildedrelief,inlaidmarble,carvedsandstone,scaletile,sunstone,gildedgate,goldblock,sandstone"),
];

pub struct Spec {
    pub name: String,
    pub kind: i32,
    pub block: u32,
    pub linear: [f32; 3],
    pub rough: f32,
    pub refl: f32,
    pub metallic: f32,
}

fn srgb_to_linear(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

/// Voxel architecture families 54..=58 move to 64..=68, as the vertex stage does,
/// so they do not collide with rigid model finishes.
fn family_kind(kind: i32) -> i32 {
    if (54..=58).contains(&kind) { kind + 10 } else { kind }
}

/// Roughness the fragment stage gives plated metals (kinds 47..=51) by default.
fn metal_roughness(kind: i32) -> f32 {
    match kind {
        47 => 0.30,
        48 => 0.12,
        49 => 0.16,
        50 => 0.15,
        _ => 0.08,
    }
}

fn spec(id: BlockId) -> Spec {
    let base = id.color(IVec3::ZERO);
    let surface = id.surface();
    let kind = family_kind(surface.texture_kind as i32);
    let metal = (47..=51).contains(&kind);
    // Same conversion as the vertex stage; unset rock uses the dielectric default.
    let mesh_refl = (0.5 * (surface.specular.clamp(0.0, 1.0) / 0.04).sqrt()).clamp(0.0, 1.0);
    // Fragment-stage glass override uses the material ID, not masonry defaults.
    let window = id == BlockId::WindowGlass;
    Spec {
        name: format!("{id:?}"),
        kind,
        block: surface.material_id,
        linear: base.map(srgb_to_linear),
        rough: if window { 0.10 } else if metal { metal_roughness(kind) } else { surface.roughness.clamp(0.08, 1.0) },
        refl: if window { 0.55 } else if kind == 3 { 0.5 } else { mesh_refl },
        metallic: if metal { 1.0 } else { 0.0 },
    }
}

fn find(name: &str) -> Result<BlockId> {
    let key = name.trim().to_ascii_lowercase();
    match ALL_BLOCK_IDS.iter().find(|id| format!("{id:?}").to_ascii_lowercase() == key) {
        Some(id) => Ok(*id),
        None => bail!("unknown block {name:?}; names are the BlockId variants, case-insensitive"),
    }
}

/// `list` is a preset name, `kinds`, or comma separated block names.
pub fn resolve(list: &str) -> Result<Vec<Spec>> {
    if list == "kinds" {
        let mut seen = std::collections::BTreeSet::new();
        return Ok(ALL_BLOCK_IDS.iter().filter(|id| seen.insert(id.surface().texture_kind)).map(|id| spec(*id)).collect());
    }
    let names = PRESETS.iter().find(|(n, _)| *n == list).map_or(list, |(_, v)| v);
    names.split(',').filter(|n| !n.trim().is_empty()).map(|n| find(n).map(spec)).collect()
}
