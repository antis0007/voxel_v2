//! The Gilded Cave: a pocket realm entered through a placed gilded gate. One
//! authored procession, warm to cold: a sand stair down from the arrival
//! landing, a domed palace gallery with side vaults, a cavern of coin dunes,
//! and a teal grotto where a stair winds up a rock pillar to a lit summit.
//! Everything is in world metres so the layout is the same at every voxel
//! scale; the realm runs along +z from the arrival gate at the origin.
//!
//! No historical world is a Gilded Cave, so this first layout needs no
//! generator gate. Any later change to it must gate on `generator_version`.
use crate::{hash3, value_noise2, value_noise3, BlockId, ColumnSample, WorldSampleConfig};
use glam::{IVec3, Vec3};
use std::f32::consts::{PI, TAU};

/// Solid bedrock below this height.
pub const BEDROCK_M: f32 = -48.0;
/// Top of the solid roof; the column height.
pub const ROOF_TOP_M: f32 = 40.0;

/// Floor of the palace gallery and the treasure hall.
const HALL_Y: f32 = -12.0;
/// The arrival stair descends along z between these.
const STAIR_Z: (f32, f32) = (4.0, 16.0);
const GALLERY_Z: (f32, f32) = (16.0, 70.0);
const GALLERY_HALF: f32 = 5.0;
const BAY: f32 = 6.0;
/// Side vaults open off the gallery at these z centres, one each side.
const VAULTS: [f32; 4] = [25.0, 37.0, 49.0, 61.0];
/// Treasure hall: an ellipsoid of these half-axes centred on the hall floor.
const HOARD_Z: f32 = 100.0;
const HOARD_R: Vec3 = Vec3::new(30.0, 22.0, 28.0);
/// The grotto: still water over a rock floor around the summit pillar.
const POOL_Y: f32 = -20.0;
const POOL_FLOOR: f32 = -22.0;
const GROTTO_Z: f32 = 172.0;
const GROTTO_R: f32 = 26.0;
const GROTTO_H: f32 = 42.0;
const STAIR_IN: f32 = 4.0;
const STAIR_OUT: f32 = 5.6;
const PITCH: f32 = 9.0;
const SUMMIT_Y: f32 = 6.0;

const ROCK: u32 = 0x474f_4c44;
const VAULT: u32 = 0x5641_554c;
const DUNE: u32 = 0x4455_4e45;
const CACHE: u32 = 0x4341_4348;
const PILLAR: u32 = 0x5049_4c52;
const LIGHT: u32 = 0x4c49_4754;

pub fn column(cfg: &WorldSampleConfig, _x: i32, _z: i32) -> ColumnSample {
    ColumnSample {
        deposit_m: 0.0,
        deposit_bed_surface: BlockId::OchreSandstone,
        height: cfg.blocks_for_height(ROOF_TOP_M),
        surface: BlockId::OchreSandstone,
        filler: BlockId::OchreSandstone,
        vegetation_density: 0.0,
        cave: Default::default(),
    }
}

/// Feet position in voxels: on the arrival landing beside the gate.
pub fn spawn(cfg: &WorldSampleConfig) -> Vec3 {
    Vec3::new(0.5, 0.02, 1.5) / cfg.voxel_meters()
}

/// The generated return gate: a 1 m block in the landing floor at the origin.
pub fn gate_at(m: Vec3) -> bool {
    (0.0..1.0).contains(&m.x) && (0.0..1.0).contains(&m.z) && (-1.0..0.0).contains(&m.y)
}

/// The voxel holding a point given in metres.
fn cell(at: Vec3, vm: f32) -> IVec3 {
    (at / vm).floor().as_ivec3()
}

fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Stair tread height: one rise per run, `s` metres per step.
fn descent(z: f32, z0: f32, top: f32, bottom: f32, s: f32) -> f32 {
    if z < z0 {
        return top;
    }
    (top - ((z - z0) / s).floor() * s - s).max(bottom)
}

/// Wall torch on the wall at `-x` (support toward -x) or `+x`.
fn wall_torch(toward_neg_x: bool) -> BlockId {
    if toward_neg_x { BlockId::TorchWallEast } else { BlockId::TorchWallWest }
}

// --- Arrival landing and stair -------------------------------------------------

fn stair_floor(z: f32, s: f32) -> f32 {
    descent(z, STAIR_Z.0, 0.0, HALL_Y, s)
}

fn arrival_inside(p: IVec3, m: Vec3, vm: f32, s: f32) -> Option<BlockId> {
    let ax = m.x.abs();
    if ax < 4.5 && (-4.5..STAIR_Z.0).contains(&m.z) && m.y >= 0.0
        && m.y < 6.0 + 1.5 * (1.0 - (m.x / 4.5).powi(2))
    {
        for at in [Vec3::new(-3.5, 0.5, -3.5), Vec3::new(3.5, 0.5, -3.5), Vec3::new(-3.5, 0.5, 2.5), Vec3::new(3.5, 0.5, 2.5)] {
            if p == cell(at, vm) {
                return Some(BlockId::Torch);
            }
        }
        return Some(BlockId::Air);
    }
    if ax < 3.5 && (STAIR_Z.0..STAIR_Z.1).contains(&m.z) {
        let f = stair_floor(m.z, s);
        if m.y >= f && m.y < f + 6.5 {
            for z in [6.5f32, 10.5, 14.5] {
                let y = stair_floor(z, s) + 2.5;
                for side in [-1.0f32, 1.0] {
                    if p == cell(Vec3::new(side * (3.5 - vm * 0.5), y, z), vm) {
                        return Some(wall_torch(side < 0.0));
                    }
                }
            }
            return Some(BlockId::Air);
        }
    }
    None
}

fn arrival_shell(m: Vec3, s: f32) -> Option<BlockId> {
    let ax = m.x.abs();
    if ax > 5.5 || m.z < -5.5 || m.z > STAIR_Z.1 + 0.5 || m.y < HALL_Y - 1.5 || m.y > 9.0 {
        return None;
    }
    let floor = if m.z < STAIR_Z.0 { 0.0 } else { stair_floor(m.z, s) };
    if m.y < floor && m.y >= floor - 1.0 {
        return Some(if m.z < STAIR_Z.0 { BlockId::StarTile } else { BlockId::CarvedSandstone });
    }
    let hf = m.y - floor;
    if m.z < STAIR_Z.0 {
        // Landing walls: carved ashlar with a gilded band at eye height.
        return Some(if (2.5..3.5).contains(&hf) { BlockId::GildedRelief } else { BlockId::CarvedSandstone });
    }
    None
}

// --- Palace gallery and side vaults ------------------------------------------

fn gallery_ceiling(m: Vec3) -> f32 {
    let dz = (m.z - GALLERY_Z.0).rem_euclid(BAY) - BAY * 0.5;
    let dome = (1.0 - (dz / (BAY * 0.5)).powi(2)).max(0.0) * (1.0 - (m.x / GALLERY_HALF).powi(2)).max(0.0);
    HALL_Y + 5.5 + 4.0 * dome
}

/// Room kind of one vault: 0 treasury, 1 pit trap, 2 coin store, 3 collapse.
fn vault_kind(seed: u32, zc: f32, side: f32) -> u32 {
    let h = hash3(zc as i32, side as i32, 0, seed ^ VAULT);
    ((h * 4.0) as u32).min(3)
}

/// Height of a vault's coin heap, highest against the back wall.
fn vault_heap(seed: u32, sx: f32, dz: f32) -> f32 {
    let back = smooth(6.0, 14.0, sx);
    2.6 * back * (1.0 - (dz / 4.0).powi(2)) + 0.6 * value_noise2(sx / 2.0, dz / 2.0, seed ^ DUNE)
}

fn gallery_inside(seed: u32, p: IVec3, m: Vec3, vm: f32) -> Option<BlockId> {
    let ax = m.x.abs();
    let hf = m.y - HALL_Y;
    if (GALLERY_Z.0..GALLERY_Z.1).contains(&m.z) && ax < GALLERY_HALF && hf >= 0.0 && m.y < gallery_ceiling(m) {
        // Columns at each bay line, a step in from the walls.
        let bay = ((m.z - GALLERY_Z.0) / BAY).floor();
        let zb = GALLERY_Z.0 + bay * BAY;
        if bay > 0.0 && (ax - 3.5).abs() < 0.5 && (m.z - (zb + 0.5)).abs() < 0.5 {
            return Some(if (3.5..4.5).contains(&hf) { BlockId::GildedRelief } else { BlockId::CarvedSandstone });
        }
        for side in [-1.0f32, 1.0] {
            if bay > 0.0 && p == cell(Vec3::new(side * (GALLERY_HALF - vm * 0.5), HALL_Y + 2.5, zb + 0.5), vm) {
                return Some(wall_torch(side < 0.0));
            }
        }
        return Some(BlockId::Air);
    }
    for zc in VAULTS {
        let dz = m.z - zc;
        if dz.abs() >= 5.0 || ax >= 14.0 {
            continue;
        }
        let side = m.x.signum();
        let sx = ax;
        // Doorway through the gallery wall.
        if (GALLERY_HALF..6.0).contains(&sx) && dz.abs() < 1.0 && (0.0..3.0).contains(&hf) {
            return Some(BlockId::Air);
        }
        if sx < 6.0 || dz.abs() >= 4.0 {
            continue;
        }
        let kind = vault_kind(seed, zc, side);
        let back = Vec3::new(side * 13.5, HALL_Y + 0.5, zc + 0.5);
        if (0.0..5.0).contains(&hf) {
            if p == cell(Vec3::new(side * (14.0 - vm * 0.5), HALL_Y + 2.5, zc - 2.5), vm) {
                return Some(wall_torch(side < 0.0));
            }
            match kind {
                0 => {
                    for at in [back, back + Vec3::new(0.0, 0.0, 2.0), back - Vec3::new(0.0, 0.0, 2.0)] {
                        if p == cell(at, vm) {
                            return Some(if at == back { BlockId::SurveyCache } else { BlockId::SalvageCrate });
                        }
                    }
                }
                2 => {
                    let top = HALL_Y + vault_heap(seed, sx, dz);
                    if m.y < top {
                        return Some(BlockId::GoldCoins);
                    }
                    let crest = HALL_Y + vault_heap(seed, 12.5, 0.0);
                    if p == cell(Vec3::new(side * 12.5, crest + vm * 0.5, zc + 0.5), vm) {
                        return Some(BlockId::SurveyCache);
                    }
                }
                3 => {
                    let rubble = 1.6 * value_noise2(m.x / 1.7, m.z / 1.7, seed ^ VAULT) * smooth(7.0, 12.0, sx);
                    if hf < rubble {
                        return Some(BlockId::OchreSandstone);
                    }
                    if p == cell(Vec3::new(side * 7.5, HALL_Y + 0.5, zc + 2.5), vm) {
                        return Some(BlockId::SalvageCrate);
                    }
                }
                _ => {}
            }
            return Some(BlockId::Air);
        }
        // Pit trap: a hidden drop under a mirage floor, with a cache below.
        if kind == 1 && (8.0..12.0).contains(&sx) && dz.abs() < 2.0 && (HALL_Y - 4.0..HALL_Y - 1.0).contains(&m.y) {
            if p == cell(Vec3::new(side * 10.5, HALL_Y - 3.5, zc + 0.5), vm) {
                return Some(BlockId::SurveyCache);
            }
            return Some(BlockId::Air);
        }
    }
    None
}

fn gallery_shell(seed: u32, m: Vec3) -> Option<BlockId> {
    let ax = m.x.abs();
    let hf = m.y - HALL_Y;
    if m.z < GALLERY_Z.0 - 1.0 || m.z > GALLERY_Z.1 + 1.0 || ax > 15.0 || hf < -6.0 || hf > 11.0 {
        return None;
    }
    for zc in VAULTS {
        let dz = m.z - zc;
        if dz.abs() > 5.0 || ax < GALLERY_HALF {
            continue;
        }
        // Jeweled frame around each doorway.
        if (GALLERY_HALF..6.0).contains(&ax) && dz.abs() < 1.8 && hf < 3.8 {
            return Some(BlockId::JeweledGold);
        }
        if ax >= 6.0 {
            let kind = vault_kind(seed, zc, m.x.signum());
            if kind == 1 && (8.0..12.0).contains(&ax) && dz.abs() < 2.0 {
                if (-1.0..0.0).contains(&hf) {
                    return Some(BlockId::MirageBrick);
                }
                if (-5.0..-4.0).contains(&hf) {
                    return Some(BlockId::GoldCoins);
                }
            }
            if (-1.0..0.0).contains(&hf) {
                return Some(if kind == 0 { BlockId::StarTile } else { BlockId::CarvedSandstone });
            }
            if hf >= 5.0 {
                return Some(BlockId::GildedRelief);
            }
            if hf >= 0.0 {
                return Some(if kind == 0 { BlockId::InlaidMarble } else { BlockId::LapisBrick });
            }
            return None;
        }
    }
    if ax < GALLERY_HALF && (-1.0..0.0).contains(&hf) {
        return Some(if ax > 3.5 { BlockId::LozengeTile } else { BlockId::ZigzagTile });
    }
    if ax < GALLERY_HALF + 1.5 && hf >= 0.0 {
        if m.y >= gallery_ceiling(Vec3::new(m.x.clamp(-GALLERY_HALF + 0.01, GALLERY_HALF - 0.01), m.y, m.z)) && ax < GALLERY_HALF {
            return Some(BlockId::ScaleTile);
        }
        return Some(if hf < 3.0 {
            BlockId::LapisBrick
        } else if hf < 4.0 {
            BlockId::GildedRelief
        } else if hf < 9.5 {
            BlockId::CarvedSandstone
        } else {
            BlockId::ScaleTile
        });
    }
    None
}

// --- Treasure hall ------------------------------------------------------------

fn hoard_open(seed: u32, m: Vec3) -> bool {
    let up = if m.y >= HALL_Y { (m.y - HALL_Y) / HOARD_R.y } else { (m.y - HALL_Y) / 4.0 };
    let d = Vec3::new(m.x / HOARD_R.x, up, (m.z - HOARD_Z) / HOARD_R.z).length();
    let n = value_noise3(m.x / 7.0, m.y / 7.0, m.z / 7.0, seed ^ ROCK);
    d < 0.86 + 0.16 * n
}

fn on_causeway(m: Vec3) -> bool {
    m.x.abs() < 3.0 && (GALLERY_Z.1..HOARD_Z + HOARD_R.z).contains(&m.z)
}

/// Top of the coin dunes; piles climb toward the cavern walls.
fn dune_top(seed: u32, x: f32, z: f32) -> f32 {
    let edge = (x / HOARD_R.x).hypot((z - HOARD_Z) / HOARD_R.z);
    let dune = 0.8
        + 2.6 * value_noise2(x / 8.0, z / 8.0, seed ^ DUNE)
        + 1.0 * value_noise2(x / 3.1, z / 3.1, seed ^ DUNE.rotate_left(9))
        + 5.5 * smooth(0.45, 0.95, edge);
    // The causeway runs in a trench between the dunes.
    let trench = smooth(3.0, 7.0, x.abs());
    HALL_Y - 1.5 + dune * (0.25 + 0.75 * trench)
}

/// Rock pillars on a jittered 16 m grid, clear of the causeway.
fn hoard_pillar(seed: u32, m: Vec3) -> bool {
    let c = (Vec3::new(m.x, 0.0, m.z - HOARD_Z) / 16.0).floor();
    let (cx, cz) = (c.x as i32, c.z as i32);
    if hash3(cx, 0, cz, seed ^ PILLAR) < 0.35 {
        return false;
    }
    let px = (c.x + 0.25 + 0.5 * hash3(cx, 1, cz, seed ^ PILLAR)) * 16.0;
    let pz = HOARD_Z + (c.z + 0.25 + 0.5 * hash3(cx, 2, cz, seed ^ PILLAR)) * 16.0;
    if px.abs() < 7.0 {
        return false;
    }
    let r = 1.5 + 0.9 * hash3(cx, 3, cz, seed ^ PILLAR) + 0.3 * (m.y * 0.4).sin();
    (m.x - px).hypot(m.z - pz) < r
}

fn hoard_inside(seed: u32, p: IVec3, m: Vec3, vm: f32) -> Option<BlockId> {
    let ax = m.x.abs();
    let hf = m.y - HALL_Y;
    // Grand arch from the gallery into the cavern.
    let arch = (GALLERY_Z.1..GALLERY_Z.1 + 8.0).contains(&m.z) && ax < 3.5 && hf >= 0.0 && hf < 5.5 + 1.5 * (1.0 - (m.x / 3.5).powi(2));
    if !arch && !hoard_open(seed, m) {
        return None;
    }
    if hoard_pillar(seed, m) {
        return Some(BlockId::OchreSandstone);
    }
    if on_causeway(m) {
        if hf < 0.0 {
            return Some(if ax < 2.0 { BlockId::ZigzagTile } else { BlockId::CarvedSandstone });
        }
        if ax >= 2.0 && hf < 1.0 {
            return Some(BlockId::CarvedSandstone);
        }
        // Torches stand on the kerbs every 6 m.
        let kz = ((m.z - GALLERY_Z.1) / 6.0).floor() * 6.0 + GALLERY_Z.1 + 3.5;
        for side in [-1.0f32, 1.0] {
            if p == cell(Vec3::new(side * 2.5, HALL_Y + 1.0 + vm * 0.5, kz), vm) {
                return Some(BlockId::Torch);
            }
        }
        if ax < 2.0 || hf >= 1.0 {
            let top = dune_top(seed, m.x, m.z);
            if ax >= 3.0 && m.y < top {
                return Some(BlockId::GoldCoins);
            }
            return Some(BlockId::Air);
        }
    }
    let top = dune_top(seed, m.x, m.z);
    if m.y < top {
        // Jeweled crusts on some crests; the rest is loose coin.
        if m.y > top - 1.0 && value_noise2(m.x / 4.0, m.z / 4.0, seed ^ DUNE.rotate_left(3)) > 0.82 {
            return Some(BlockId::JeweledGold);
        }
        // Half-buried caches on a 7 m grid.
        let g = (Vec3::new(m.x, 0.0, m.z) / 7.0).floor();
        let (gx, gz) = (g.x as i32, g.z as i32);
        let roll = hash3(gx, 5, gz, seed ^ CACHE);
        if roll > 0.70 {
            let cx = g.x * 7.0 + 3.5;
            let cz = g.z * 7.0 + 3.5;
            if cx.abs() > 4.0 && p == cell(Vec3::new(cx, dune_top(seed, cx, cz) - vm * 0.5, cz), vm) {
                return Some(if roll > 0.92 { BlockId::SurveyCache } else { BlockId::SalvageCrate });
            }
        }
        return Some(BlockId::GoldCoins);
    }
    // Stacks of gold blocks and standing torches on the dunes.
    let g = (Vec3::new(m.x, 0.0, m.z) / 5.0).floor();
    let (gx, gz) = (g.x as i32, g.z as i32);
    if hash3(gx, 7, gz, seed ^ CACHE) > 0.86 {
        let sx = g.x * 5.0 + 2.5;
        let sz = g.z * 5.0 + 2.5;
        let base = dune_top(seed, sx, sz);
        if (m.x - sx).abs() < 0.5 && (m.z - sz).abs() < 0.5 && m.y >= base - vm && m.y < base + 1.5 {
            return Some(BlockId::GoldBlock);
        }
    }
    let t = (Vec3::new(m.x, 0.0, m.z) / 9.0).floor();
    let (tx, tz) = (t.x as i32, t.z as i32);
    if hash3(tx, 9, tz, seed ^ LIGHT) > 0.40 {
        let lx = t.x * 9.0 + 2.5 + 4.0 * hash3(tx, 10, tz, seed ^ LIGHT);
        let lz = t.z * 9.0 + 2.5 + 4.0 * hash3(tx, 11, tz, seed ^ LIGHT);
        if lx.abs() > 4.0 && p == cell(Vec3::new(lx, dune_top(seed, lx, lz) + vm * 0.5, lz), vm) {
            return Some(BlockId::Torch);
        }
    }
    Some(BlockId::Air)
}

/// Sunstone clusters hang from the cavern roof on an 11 m grid.
fn hoard_shell(seed: u32, m: Vec3, vm: f32) -> Option<BlockId> {
    if m.y < HALL_Y + 8.0 || !hoard_open(seed, m - Vec3::Y * vm) || hoard_open(seed, m) {
        return None;
    }
    let g = (Vec3::new(m.x, 0.0, m.z) / 11.0).floor();
    let (gx, gz) = (g.x as i32, g.z as i32);
    if hash3(gx, 13, gz, seed ^ LIGHT) < 0.45 {
        return None;
    }
    let cx = g.x * 11.0 + 5.5;
    let cz = g.z * 11.0 + 5.5;
    ((m.x - cx).hypot(m.z - cz) < 1.4).then_some(BlockId::Sunstone)
}

// --- Passage and grotto ---------------------------------------------------------

const PASSAGE_Z: (f32, f32) = (124.0, 150.0);

fn passage_floor(z: f32, s: f32) -> f32 {
    descent(z, 130.0, HALL_Y, POOL_Y + 1.0, s)
}

fn passage_inside(p: IVec3, m: Vec3, vm: f32, s: f32) -> Option<BlockId> {
    if !(PASSAGE_Z.0..PASSAGE_Z.1).contains(&m.z) || m.x.abs() >= 1.8 {
        return None;
    }
    let f = passage_floor(m.z, s);
    if m.y < f || m.y >= f + 4.0 + 0.6 * (1.0 - (m.x / 1.8).powi(2)) {
        return None;
    }
    for side in [-1.0f32, 1.0] {
        if p == cell(Vec3::new(side * (1.8 - vm * 0.5), passage_floor(133.5, s) + 2.5, 133.5), vm) {
            return Some(wall_torch(side < 0.0));
        }
    }
    Some(BlockId::Air)
}

fn passage_shell(m: Vec3, s: f32) -> Option<BlockId> {
    if !(PASSAGE_Z.0..PASSAGE_Z.1).contains(&m.z) || m.x.abs() >= 1.8 {
        return None;
    }
    let f = passage_floor(m.z, s);
    (m.y < f && m.y >= f - 1.0).then_some(BlockId::CarvedSandstone)
}

fn grotto_open(seed: u32, m: Vec3) -> bool {
    let hr = m.x.hypot(m.z - GROTTO_Z) / GROTTO_R;
    let n = value_noise3(m.x / 6.0, m.y / 6.0, m.z / 6.0, seed ^ ROCK.rotate_left(11));
    if m.y >= POOL_Y {
        return hr * hr + ((m.y - POOL_Y) / GROTTO_H).powi(2) < 0.88 + 0.14 * n;
    }
    m.y >= POOL_FLOOR && hr < 0.88 + 0.08 * n
}

fn pillar_radius(y: f32) -> f32 {
    3.6 + 0.4 * (y * 0.45).sin()
}

/// Top of the spiral stair tread at this point, if it lies on the stair band.
fn helix_top(m: Vec3, s: f32) -> Option<f32> {
    let dx = m.x;
    let dz = m.z - GROTTO_Z;
    let r = dx.hypot(dz);
    if !(STAIR_IN..STAIR_OUT).contains(&r) {
        return None;
    }
    // Start facing the stepping stones (-z) and climb counter-clockwise.
    let u = (dz.atan2(dx) + PI * 0.5).rem_euclid(TAU);
    let start = POOL_Y + 1.0;
    for turn in 0..4 {
        let raw = start + (u + TAU * turn as f32) / TAU * PITCH;
        if raw > SUMMIT_Y {
            break;
        }
        let top = start + ((raw - start) / s).floor() * s + s;
        if m.y < top && m.y >= top - 0.8 {
            return Some(top);
        }
    }
    None
}

fn grotto_inside(seed: u32, p: IVec3, m: Vec3, vm: f32, s: f32) -> Option<BlockId> {
    if !grotto_open(seed, m) {
        return None;
    }
    let dz = m.z - GROTTO_Z;
    let r = m.x.hypot(dz);
    // The pillar, its flared capital and the summit floor.
    if m.y < SUMMIT_Y - 4.0 && r < pillar_radius(m.y) {
        return Some(BlockId::GrottoStone);
    }
    if (SUMMIT_Y - 4.0..SUMMIT_Y - 1.0).contains(&m.y) && r < STAIR_IN + (m.y - (SUMMIT_Y - 4.0)) * 0.53 {
        return Some(BlockId::GrottoStone);
    }
    if (SUMMIT_Y - 1.0..SUMMIT_Y).contains(&m.y) && r < STAIR_OUT {
        if p == cell(Vec3::new(2.5, SUMMIT_Y - 0.5, GROTTO_Z + 0.5), vm) {
            return Some(BlockId::GildedGate);
        }
        return Some(if r >= 4.6 { BlockId::JeweledGold } else { BlockId::InlaidMarble });
    }
    if m.y >= SUMMIT_Y && r < STAIR_OUT {
        if p == cell(Vec3::new(0.5, SUMMIT_Y + 0.5, GROTTO_Z + 0.5), vm) {
            return Some(BlockId::CarvedSandstone);
        }
        if p == cell(Vec3::new(0.5, SUMMIT_Y + 1.5, GROTTO_Z + 0.5), vm) {
            return Some(BlockId::SurveyCache);
        }
        for at in [Vec3::new(-3.5, 0.0, 0.5), Vec3::new(3.5, 0.0, 0.5), Vec3::new(0.5, 0.0, -3.5), Vec3::new(0.5, 0.0, 3.5)] {
            if p == cell(Vec3::new(at.x, SUMMIT_Y + 0.5, GROTTO_Z + at.z), vm) {
                return Some(BlockId::Torch);
            }
        }
    }
    if helix_top(m, s).is_some() {
        return Some(BlockId::CarvedSandstone);
    }
    // Shore ledge under the passage mouth, and stepping stones to the pillar.
    if m.y < POOL_Y + 1.0 {
        if dz < -20.0 && m.x.abs() < 6.0 {
            return Some(BlockId::GrottoStone);
        }
        for i in 0..7 {
            let zs = GROTTO_Z - 19.5 + 2.4 * i as f32;
            let xs = 1.6 * (i as f32 * 1.7).sin();
            if (m.x - xs).abs() < 1.0 && (m.z - zs).abs() < 1.0 {
                return Some(BlockId::GrottoStone);
            }
        }
    }
    if m.y < POOL_Y {
        return Some(BlockId::Water);
    }
    Some(BlockId::Air)
}

fn grotto_shell(seed: u32, m: Vec3, vm: f32) -> Option<BlockId> {
    let r = m.x.hypot(m.z - GROTTO_Z);
    if r > GROTTO_R * 1.2 {
        return None;
    }
    // A sunstone boss in the dome apex lights the summit.
    if r < 2.6 && m.y > SUMMIT_Y + 6.0 && grotto_open(seed, m - Vec3::Y * vm) {
        return Some(BlockId::Sunstone);
    }
    // Foxfire along the walls just above the water.
    if (POOL_Y..POOL_Y + 7.0).contains(&m.y) && r > 6.0 {
        let inward = Vec3::new(-m.x, 0.0, GROTTO_Z - m.z) / r;
        if grotto_open(seed, m + inward * vm)
            && value_noise3(m.x / 3.0, m.y / 3.0, m.z / 3.0, seed ^ LIGHT) > 0.60
        {
            return Some(BlockId::FoxfireMoss);
        }
    }
    None
}

// --- Rock -------------------------------------------------------------------

fn rock(seed: u32, m: Vec3) -> BlockId {
    let grotto = m.z + 6.0 * value_noise2(m.x / 6.0, m.y / 6.0, seed ^ ROCK) > 143.0;
    if grotto {
        return BlockId::GrottoStone;
    }
    // Gold-bearing quartz veins run through the sandstone.
    if value_noise3(m.x / 5.0, m.y / 4.0, m.z / 5.0, seed ^ ROCK.rotate_left(5)) > 0.84 {
        return BlockId::GoldQuartz;
    }
    BlockId::OchreSandstone
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
    if gate_at(m) {
        return BlockId::GildedGate;
    }
    let seed = cfg.seed;
    // Stairs step a quarter metre at fine scales, one voxel at coarse ones.
    let s = vm.max(0.25);
    let inside = if m.z < STAIR_Z.1 + 1.0 {
        arrival_inside(p, m, vm, s)
    } else {
        None
    }
    .or_else(|| gallery_inside(seed, p, m, vm))
    .or_else(|| if m.z > GALLERY_Z.1 - 1.0 && m.z < HOARD_Z + HOARD_R.z * 1.2 { hoard_inside(seed, p, m, vm) } else { None })
    .or_else(|| passage_inside(p, m, vm, s))
    .or_else(|| if m.z > GROTTO_Z - GROTTO_R * 1.2 { grotto_inside(seed, p, m, vm, s) } else { None });
    if let Some(b) = inside {
        return b;
    }
    let shell = arrival_shell(m, s)
        .or_else(|| gallery_shell(seed, m))
        .or_else(|| passage_shell(m, s))
        .or_else(|| if m.z > GALLERY_Z.1 && m.z < HOARD_Z + HOARD_R.z * 1.2 { hoard_shell(seed, m, vm) } else { None })
        .or_else(|| if m.z > GROTTO_Z - GROTTO_R * 1.3 { grotto_shell(seed, m, vm) } else { None });
    shell.unwrap_or_else(|| rock(seed, m))
}
