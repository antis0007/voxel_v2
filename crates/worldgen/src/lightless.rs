//! The Lightless: closed caverns entered through a placed shaft. Three
//! gallery decks joined by chasms, dense ore in the walls, foxfire refuges on
//! the floors and a lava floor at the bottom. Everything is in world metres so
//! the layout is the same at every voxel scale.
//!
//! No historical world is a Lightless realm, so this first layout needs no
//! generator gate. Any later change to it must gate on `generator_version`.
use crate::{hash3, value_noise2, value_noise3, BlockId, ColumnSample, WorldSampleConfig};
use glam::{IVec3, Vec3};

/// Solid bedrock below this height.
pub const BEDROCK_M: f32 = -60.0;
/// No cavern opens below this height; lava fills the lowest open cells.
pub const FLOOR_M: f32 = -50.0;
/// Lava fills open cells below this height.
pub const LAVA_M: f32 = -42.0;
/// No cavern opens above this height.
pub const ROOF_M: f32 = 140.0;
/// Top of the solid roof; the column height.
pub const ROOF_TOP_M: f32 = 156.0;
/// Gallery decks: wide, low caverns centred on these heights.
pub const DECKS: [f32; 3] = [0.0, 46.0, 94.0];
/// Arrival chamber around the generated return shaft at the origin.
const CHAMBER_R: f32 = 9.0;
const CHAMBER_H: f32 = 5.5;

const CAVE: u32 = 0x4c49_4748;
const CHASM: u32 = 0x4348_4153;
const VEIN: u32 = 0x5645_494e;
const FOXFIRE: u32 = 0x464f_5846;
const PILLAR: u32 = 0x5049_4c4c;

pub fn column(cfg: &WorldSampleConfig, _x: i32, _z: i32) -> ColumnSample {
    ColumnSample {
        deposit_m: 0.0,
        deposit_bed_surface: BlockId::LightlessStone,
        height: cfg.blocks_for_height(ROOF_TOP_M),
        surface: BlockId::LightlessStone,
        filler: BlockId::LightlessStone,
        vegetation_density: 0.0,
        cave: Default::default(),
    }
}

/// Feet position in voxels: on the chamber floor beside the shaft.
pub fn spawn(cfg: &WorldSampleConfig) -> Vec3 {
    Vec3::new(2.5, 0.02, 0.5) / cfg.voxel_meters()
}

/// The generated return shaft: a 1 m block in the chamber floor at the origin.
pub fn shaft_at(m: Vec3) -> bool {
    (0.0..1.0).contains(&m.x) && (0.0..1.0).contains(&m.z) && (-1.0..0.0).contains(&m.y)
}

fn chamber(m: Vec3) -> Option<bool> {
    let r = m.x.hypot(m.z);
    if r > CHAMBER_R + 2.0 || m.y < -3.0 || m.y > CHAMBER_H + 3.0 {
        return None;
    }
    // Domed room: open inside, solid floor below y = 0.
    let dome = CHAMBER_H + 2.5 * (1.0 - (r / CHAMBER_R).powi(2)).max(0.0);
    Some(m.y >= 0.0 && r < CHAMBER_R && m.y < dome)
}

/// Cavern field. True where the rock is open.
pub fn open(seed: u32, m: Vec3) -> bool {
    if m.y < FLOOR_M || m.y > ROOF_M {
        return false;
    }
    if let Some(open) = chamber(m) {
        return open;
    }
    // Squash vertically so the caverns are halls, not tubes.
    let q = Vec3::new(m.x, m.y * 1.7, m.z);
    let n = 0.55 * value_noise3(q.x / 58.0, q.y / 58.0, q.z / 58.0, seed ^ CAVE)
        + 0.30 * value_noise3(q.x / 21.0, q.y / 21.0, q.z / 21.0, seed ^ CAVE.rotate_left(7))
        + 0.15 * value_noise3(q.x / 8.0, q.y / 8.0, q.z / 8.0, seed ^ CAVE.rotate_left(13));
    let deck = DECKS.iter().map(|&d| (-((m.y - d) / 13.0).powi(2)).exp()).fold(0.0f32, f32::max);
    // Close the field towards the floor and the roof.
    let edge = ((FLOOR_M + 10.0 - m.y).max(0.0) + (m.y - (ROOF_M - 12.0)).max(0.0)) / 10.0;
    let mut value = n + deck * 0.20 - edge * 0.3;
    // Chasms: rare round shafts that cut through every deck.
    let c = value_noise2(m.x / 96.0, m.z / 96.0, seed ^ CHASM);
    if c > 0.78 {
        value += (c - 0.78) * 2.6;
    }
    if value < 0.64 {
        return false;
    }
    // Stone pillars stand in the open halls on a jittered 14 m grid.
    let cell = (Vec3::new(m.x, 0.0, m.z) / 14.0).floor();
    let (cx, cz) = (cell.x as i32, cell.z as i32);
    if hash3(cx, 0, cz, seed ^ PILLAR) > 0.55 {
        let jx = (cell.x + 0.25 + 0.5 * hash3(cx, 1, cz, seed ^ PILLAR)) * 14.0;
        let jz = (cell.z + 0.25 + 0.5 * hash3(cx, 2, cz, seed ^ PILLAR)) * 14.0;
        let radius = 1.1 + 1.4 * hash3(cx, 3, cz, seed ^ PILLAR);
        if (m.x - jx).hypot(m.z - jz) < radius {
            return false;
        }
    }
    true
}

/// Ore in the walls: richer and rarer with depth. Veins are about 2 m thick.
fn ore(seed: u32, m: Vec3) -> Option<BlockId> {
    let v = value_noise3(m.x / 5.5, m.y / 4.0, m.z / 5.5, seed ^ VEIN);
    if v < 0.80 {
        return None;
    }
    // One mineral per 24 m pocket, chosen by depth band.
    let cell = (m / 24.0).floor().as_ivec3();
    let pick = hash3(cell.x, cell.y, cell.z, seed ^ VEIN.rotate_left(5));
    let block = if m.y < -10.0 {
        [BlockId::Magnetite, BlockId::GoldQuartz, BlockId::Crystal, BlockId::Hematite]
    } else if m.y < 60.0 {
        [BlockId::Hematite, BlockId::Chalcopyrite, BlockId::Magnetite, BlockId::Cassiterite]
    } else {
        [BlockId::Cassiterite, BlockId::Malachite, BlockId::Chalcopyrite, BlockId::Hematite]
    };
    Some(block[((pick * 4.0) as usize).min(3)])
}

/// Camp cells, metres. At most one lost miners' camp per cell.
const CAMP_CELL_M: f32 = 112.0;
const CAMP_R: f32 = 4.6;
const CAMP: u32 = 0x4341_4d50;

/// A lost miners' camp: the floor point it was pitched on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camp {
    pub floor: Vec3,
    /// A second cache was left behind.
    pub spare: bool,
}

/// The cell's camp, if the miners found a flat open floor on the two upper
/// decks. Searched once per cell and memoised; a pure function of the seed.
pub fn camp(seed: u32, cell: (i32, i32)) -> Option<Camp> {
    use std::cell::RefCell;
    use std::collections::HashMap;
    thread_local! {
        static MEMO: RefCell<HashMap<(u32, i32, i32), Option<Camp>>> = RefCell::new(HashMap::new());
    }
    if let Some(hit) = MEMO.with(|m| m.borrow().get(&(seed, cell.0, cell.1)).copied()) {
        return hit;
    }
    let found = (|| {
        if hash3(cell.0, 7, cell.1, seed ^ CAMP) < 0.35 {
            return None;
        }
        (0..12).find_map(|k| {
            let x = (cell.0 as f32 + 0.2 + 0.6 * hash3(cell.0, k, cell.1, seed ^ CAMP ^ 1)) * CAMP_CELL_M;
            let z = (cell.1 as f32 + 0.2 + 0.6 * hash3(cell.0, k, cell.1, seed ^ CAMP ^ 2)) * CAMP_CELL_M;
            let deck = DECKS[k as usize % 2];
            // Walk down to the first floor under open air.
            let floor = (0..32).map(|i| deck + 8.0 - i as f32 * 0.5).find(|&y| {
                open(seed, Vec3::new(x, y, z)) && !open(seed, Vec3::new(x, y - 0.5, z))
            })?;
            let clear = [(0.0, 0.0), (3.4, 0.0), (-3.4, 0.0), (0.0, 3.4), (0.0, -3.4)].iter().all(|&(dx, dz)| {
                (1..6).all(|j| open(seed, Vec3::new(x + dx, floor + j as f32 * 0.5, z + dz)))
            });
            (clear && floor > LAVA_M + 4.0).then(|| Camp {
                floor: Vec3::new(x, floor, z),
                spare: hash3(cell.0, 9, cell.1, seed ^ CAMP) > 0.6,
            })
        })
    })();
    MEMO.with(|m| {
        let mut m = m.borrow_mut();
        if m.len() > 4096 {
            m.clear();
        }
        m.insert((seed, cell.0, cell.1), found);
    });
    found
}

/// Camp cells near `m`, for the life simulation and the map.
pub fn camps_near(seed: u32, m: Vec3, reach_cells: i32) -> Vec<Camp> {
    let c = ((m.x / CAMP_CELL_M).floor() as i32, (m.z / CAMP_CELL_M).floor() as i32);
    let mut out = Vec::new();
    for dz in -reach_cells..=reach_cells {
        for dx in -reach_cells..=reach_cells {
            out.extend(camp(seed, (c.0 + dx, c.1 + dz)));
        }
    }
    out
}

/// Plank floor, four dead lantern posts, a crate stack and the caches.
fn camp_block(seed: u32, m: Vec3) -> Option<BlockId> {
    let cell = ((m.x / CAMP_CELL_M).floor() as i32, (m.z / CAMP_CELL_M).floor() as i32);
    let c = camp(seed, cell)?;
    let d = m - c.floor;
    let r = d.x.hypot(d.z);
    if r > CAMP_R || !(-1.0..3.4).contains(&d.y) {
        return None;
    }
    if d.y < 0.0 {
        return Some(BlockId::Planks);
    }
    let post = [(1.0f32, 1.0f32), (-1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)]
        .iter()
        .any(|&(sx, sz)| (d.x - sx * 2.55).abs() < 0.35 && (d.z - sz * 2.55).abs() < 0.35);
    if post && d.y < 2.7 {
        return Some(BlockId::Log);
    }
    let cache = |cx: f32, cz: f32| (d.x - cx).abs() < 0.5 && (d.z - cz).abs() < 0.5 && d.y < 0.9;
    if cache(1.0, 0.5) || (c.spare && cache(-1.0, -0.5)) {
        return Some(BlockId::SurveyCache);
    }
    if (d.x + 1.2).abs() < 0.5 && (d.z - 1.4).abs() < 0.5 && d.y < 0.9 {
        return Some(BlockId::StorageCrate);
    }
    Some(BlockId::Air)
}

pub fn block(cfg: &WorldSampleConfig, p: IVec3) -> BlockId {
    let vm = cfg.voxel_meters();
    let m = (p.as_vec3() + Vec3::splat(0.5)) * vm;
    if m.y < BEDROCK_M {
        return BlockId::Bedrock;
    }
    if m.y > ROOF_TOP_M {
        return BlockId::Air;
    }
    if shaft_at(m) {
        return BlockId::LightlessShaft;
    }
    let seed = cfg.seed;
    if let Some(b) = camp_block(seed, m) {
        return b;
    }
    if open(seed, m) {
        return if m.y < LAVA_M { BlockId::Lava } else { BlockId::Air };
    }
    // Floor surfaces: foxfire refuges in rare patches, and a ring in the chamber.
    let above = m + Vec3::Y * vm;
    if open(seed, above) && above.y >= LAVA_M {
        let r = m.x.hypot(m.z);
        let ring = (r - (CHAMBER_R - 1.2)).abs() < 0.6 && hash3(p.x / 4, 0, p.z / 4, seed ^ FOXFIRE) > 0.6;
        let patch = value_noise2(m.x / 9.0, m.z / 9.0, seed ^ FOXFIRE) > 0.83;
        if (ring && m.y > -1.0 && m.y < 0.0) || (patch && r > CHAMBER_R + 2.0) {
            return BlockId::FoxfireMoss;
        }
    }
    ore(seed, m).unwrap_or(BlockId::LightlessStone)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(scale: crate::VoxelScalePreset) -> WorldSampleConfig {
        WorldSampleConfig {
            generator_version: crate::CURRENT_GENERATOR_VERSION,
            seed: 7,
            world_type: crate::WorldType::Realm(crate::RealmKind::Lightless),
            voxel_scale: scale,
            planet: None,
        }
    }

    /// The crossing needs a clear body and footing at the arrival point, and a
    /// shaft to return by, at every voxel scale.
    #[test]
    fn arrival_is_clear_with_footing_and_a_return_shaft() {
        for scale in [crate::VoxelScalePreset::OneMeter, crate::VoxelScalePreset::QuarterMeter] {
            let c = cfg(scale);
            let feet = spawn(&c);
            let cell = feet.floor().as_ivec3();
            let up = (2.0 / c.voxel_meters()).ceil() as i32;
            for dy in 0..=up {
                assert_eq!(block(&c, cell + IVec3::Y * dy), BlockId::Air, "body clear at {scale:?}");
            }
            assert!(block(&c, cell - IVec3::Y).is_solid(), "footing at {scale:?}");
            assert_eq!(block(&c, IVec3::new(0, -1, 0)), BlockId::LightlessShaft);
        }
    }

    /// Enough of the band is open to explore and enough is rock to mine.
    #[test]
    fn caverns_and_ore_are_balanced() {
        let (mut open_n, mut ore_n, mut total) = (0, 0, 0);
        for i in 0..40_000u32 {
            let m = Vec3::new(
                hash3(i as i32, 0, 1, 11) * 900.0 - 450.0,
                hash3(i as i32, 0, 2, 11) * (ROOF_M - FLOOR_M) + FLOOR_M,
                hash3(i as i32, 0, 3, 11) * 900.0 - 450.0,
            );
            total += 1;
            if open(7, m) {
                open_n += 1;
            } else if ore(7, m).is_some() {
                ore_n += 1;
            }
        }
        let open = open_n as f32 / total as f32;
        let ore = ore_n as f32 / (total - open_n) as f32;
        println!("open {open:.3} ore {ore:.3}");
        assert!((0.12..0.45).contains(&open), "open fraction {open}");
        assert!((0.01..0.10).contains(&ore), "ore fraction {ore}");
    }

    /// Camps exist, sit on a plank floor under open air and hold a cache.
    #[test]
    fn camps_have_a_floor_and_a_cache() {
        let c = cfg(crate::VoxelScalePreset::OneMeter);
        let camps = camps_near(c.seed, Vec3::ZERO, 4);
        assert!(camps.len() >= 4, "only {} camps in 9x9 cells", camps.len());
        for camp in camps {
            let cache = ((camp.floor + Vec3::new(1.0, 0.2, 0.5)) / c.voxel_meters()).floor().as_ivec3();
            assert_eq!(block(&c, cache), BlockId::SurveyCache);
            assert_eq!(block(&c, cache - IVec3::Y), BlockId::Planks);
            assert_eq!(block(&c, cache + IVec3::Y * 2), BlockId::Air);
        }
    }
}
