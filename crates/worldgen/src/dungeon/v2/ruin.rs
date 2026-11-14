//! Generator 52 neglect: moss on up-facing stone, vines hung from cornices,
//! ribs and beams, rubble and fallen ashlar along the wall foot, ferns by
//! the lamps, and tapered roots in overgrown rooms. How much of each a room
//! gets follows its damp (`wet`). Moss replaces a surface voxel and never
//! adds one; vines, rubble and roots need voxels of half a metre or less and
//! keep to the wall band or above head height, outside door clearances.
use super::{Complex, Damage, Job, Kind, Room};
use crate::{BlockId as B, hash2, hash3, value_noise3};
use glam::Vec3;

/// First generator with the ruin layer.
pub const VERSION: u32 = crate::gates::GIANTS_AND_RUINS_GENERATOR;
/// Lowest a vine hangs over the floor beside a wall, metres.
const WALL_VINE: f32 = 2.2;
/// Lowest a vine hangs over the floor away from the walls, metres.
const OPEN_VINE: f32 = 2.8;
/// Depth of the rubble band along the wall foot, metres.
const BAND: f32 = 1.5;

/// Damp of a complex's passages, 0..1.
pub fn job_wet(job: Job) -> f32 {
    match job {
        Job::Warren => 0.6,
        Job::Undercroft => 0.45,
        Job::Catacombs | Job::Quarantine | Job::Workshop => 0.3,
        Job::Sanctum => 0.2,
        Job::HollowFloor => 0.15,
        Job::ForgeHalls | Job::Orrery => 0.0,
    }
}

/// Damp of a room, 0..1: hot rooms, kept rooms and seals are dry.
pub fn wet(c: &Complex, r: &Room) -> f32 {
    if matches!(c.job, Job::ForgeHalls | Job::Orrery) || r.sealed {
        return 0.0;
    }
    let kind = match r.kind {
        Kind::Forge | Kind::Foundry | Kind::Orrery | Kind::Core | Kind::Library | Kind::Scriptorium
        | Kind::Refectory | Kind::Infirmary | Kind::Armory => return 0.0,
        Kind::Garden => 0.45,
        Kind::Cistern | Kind::Sluice => 0.35,
        _ => 0.1,
    };
    let damage = match r.damage {
        Damage::Overgrown => 0.5,
        Damage::Flooded => 0.3,
        Damage::Collapsed | Damage::Breach => 0.2,
        Damage::Intact => 0.0,
    };
    (job_wet(c.job) * 0.5 + kind + damage).min(1.0)
}

/// Stones moss takes to; planks, tiles, metals and lamps stay clean.
fn mossable(b: B) -> bool {
    matches!(b, B::Stone | B::StoneBricks | B::WeatheredBricks | B::Cobblestone | B::Gravel | B::Limestone | B::Basalt)
}

/// Moss patch test: clumps a couple of metres across, thickest at the walls.
fn patch(m: Vec3, v: f32, salt: u32, wet: f32, dwall: f32) -> bool {
    let n = value_noise3(m.x * 0.45, m.y * 0.9, m.z * 0.45, salt.wrapping_add(52));
    let p = (m / v).floor().as_ivec3();
    let grain = hash3(p.x, p.y, p.z, salt.wrapping_add(53)) * 0.12;
    let edge = (1.0 - dwall / 2.5).clamp(0.0, 1.0);
    n + grain + wet * 0.4 + edge * 0.45 > 1.05
}

/// Moss over an up-facing stone voxel. `above` samples the voxel over `m`.
pub fn moss(b: B, m: Vec3, v: f32, salt: u32, wet: f32, dwall: f32, above: impl FnOnce(Vec3) -> B) -> B {
    if wet <= 0.0 || !mossable(b) || !patch(m, v, salt, wet, dwall.max(0.0)) {
        return b;
    }
    if above(m + Vec3::Y * v).is_solid() { b } else { B::GroveMoss }
}

/// A strand hanging from `top` (the underside of something solid) down to
/// `floor`, or a shorter strand; `None` where this column has none.
fn strand(m: Vec3, v: f32, fy: f32, (top, floor): (f32, f32), wet: f32, salt: u32) -> Option<B> {
    if fy >= top || fy < floor {
        return None;
    }
    let curtain = value_noise3(m.x * 0.3, 0.0, m.z * 0.3, salt.wrapping_add(55));
    if curtain + wet * 0.5 < 1.0 {
        return None;
    }
    let (px, pz) = ((m.x / v).floor() as i32, (m.z / v).floor() as i32);
    let s = hash2(px, pz, salt.wrapping_add(54));
    if s < 0.4 {
        return None;
    }
    let length = (0.3 + s * 2.4) * (0.3 + curtain);
    if fy < top - length {
        return None;
    }
    let py = (fy / v).floor() as i32;
    Some(if hash3(px, py, pz, salt.wrapping_add(56)) > 0.78 { B::Bush } else { B::Leaves })
}

/// What hangs in a room's void: vines down the wall face from the cornice
/// or vault, and from ribs and beams (`rib` is the rib's underside when
/// this column holds one). `ceil` is the ceiling over this column.
pub fn hang(c: &Complex, r: &Room, m: Vec3, fy: f32, v: f32, dwall: f32, ceil: f32, rib: Option<f32>) -> Option<B> {
    let wet = wet(c, r);
    if v > 0.5 || wet < 0.2 || matches!(r.kind, Kind::Chasm | Kind::Pitfall | Kind::Labyrinth) {
        return None;
    }
    if dwall >= 0.0 && dwall < v {
        return strand(m, v, fy, (ceil, WALL_VINE), wet, r.salt);
    }
    let open = r.kind != Kind::Reveal && !super::gallery::has(c, r);
    match rib {
        Some(under) if open && wet >= 0.5 => strand(m, v, fy, (under, OPEN_VINE), wet * 0.5 - 0.1, r.salt.wrapping_add(1)),
        _ => None,
    }
}

/// Rooms whose wall foot is solid floor all round, so rubble lies on stone.
fn grounded(r: &Room) -> bool {
    !r.sealed && !matches!(r.kind, Kind::Chasm | Kind::Orrery | Kind::Core | Kind::Pitfall | Kind::Labyrinth
        | Kind::Library | Kind::Scriptorium | Kind::Refectory | Kind::Infirmary | Kind::Armory)
}

/// How much has fallen along the walls, 0..1.
fn fallen(r: &Room) -> f32 {
    match r.damage {
        Damage::Collapsed | Damage::Breach => 1.0,
        Damage::Overgrown => 0.6,
        Damage::Intact => 0.4,
        Damage::Flooded => 0.0,
    }
}

/// Scree, fallen ashlar and ferns along the wall foot, lowest toward the
/// room. `lamp` is the distance along the wall to the nearest lit pier.
pub fn foot(c: &Complex, r: &Room, m: Vec3, fy: f32, v: f32, dwall: f32, lamp: f32) -> Option<B> {
    if v > 0.5 || !grounded(r) || !(0.0..BAND).contains(&dwall) || !(0.0..1.2).contains(&fy) {
        return None;
    }
    let (px, py, pz) = ((m.x / v).floor() as i32, (fy / v).floor() as i32, (m.z / v).floor() as i32);
    let wet = wet(c, r);
    // Ferns at the foot of a lit pier in damp rooms.
    if wet >= 0.3 && lamp < 1.6 && dwall < 1.0 {
        let tuft = hash2(px, pz, r.salt.wrapping_add(57));
        let tall = 0.2 + tuft * 0.6 * (1.0 - lamp / 1.6);
        if tuft > 0.35 && fy < tall {
            return Some(if hash3(px, py, pz, r.salt) > 0.4 { B::Bush } else { B::Leaves });
        }
    }
    let amount = fallen(r);
    if amount <= 0.0 {
        return None;
    }
    if let Some(b) = ashlar(c, r, m, fy, v, amount) {
        return Some(b);
    }
    // Scree banked against the wall in drifts along it.
    let drift = value_noise3(m.x * 0.35, 0.0, m.z * 0.35, r.salt.wrapping_add(58));
    let chip = hash3(px, py, pz, r.salt.wrapping_add(59));
    let height = amount * (drift * 2.2 - 0.6) * (1.0 - dwall / BAND) - chip * 0.2;
    if fy >= height {
        return None;
    }
    Some(if chip > 0.7 { c.pal.alt } else if chip > 0.35 { B::Cobblestone } else { B::Gravel })
}

/// A dressed block fallen from the wall: half a metre square, up to one and
/// a quarter long, lying square to the room, sometimes two stacked.
fn ashlar(c: &Complex, r: &Room, m: Vec3, fy: f32, v: f32, amount: f32) -> Option<B> {
    let cell = 2.0;
    let (gx, gz) = ((m.x / cell).floor() as i32, (m.z / cell).floor() as i32);
    let pick = hash2(gx, gz, r.salt.wrapping_add(60));
    if pick < 1.0 - amount * 0.45 {
        return None;
    }
    let long_x = hash2(gx, gz, r.salt.wrapping_add(61)) > 0.5;
    let len = 0.75 + hash2(gx, gz, r.salt.wrapping_add(62)) * 0.5;
    let (cx, cz) = (gx as f32 * cell + 0.6 + pick * 0.8, gz as f32 * cell + 0.6 + hash2(gz, gx, r.salt) * 0.8);
    let (hx, hz) = if long_x { (len * 0.5, 0.25) } else { (0.25, len * 0.5) };
    let stacked = pick > 1.0 - amount * 0.12;
    let top = if stacked { 1.0 } else { 0.5 };
    let inside = (m.x - cx).abs() < hx && (m.z - cz).abs() < hz && fy < top;
    // The upper block sits askew: shifted along its length by a quarter.
    let upper = fy >= 0.5 && {
        let (sx, sz) = if long_x { (cx + 0.25, cz) } else { (cx, cz + 0.25) };
        (m.x - sx).abs() < hx && (m.z - sz).abs() < hz
    };
    let lower = fy < 0.5 && inside;
    if !(lower || (stacked && upper)) {
        return None;
    }
    // Keep the block whole or not at all next to a door clearance.
    let mid = Vec3::new(cx, r.floor + 0.25, cz);
    let near_door = super::kit::doorway(c, mid + Vec3::X * hx) || super::kit::doorway(c, mid - Vec3::X * hx)
        || super::kit::doorway(c, mid + Vec3::Z * hz) || super::kit::doorway(c, mid - Vec3::Z * hz);
    if near_door || super::outline::depth(r, cx - r.x0, cz - r.z0) > BAND - 0.25 {
        return None;
    }
    let p = (m / v).floor().as_ivec3();
    Some(if fy >= 0.5 { c.pal.alt } else if hash3(p.x, p.y, p.z, r.salt.wrapping_add(65)) > 0.85 { B::Cobblestone } else { c.pal.wall })
}

/// Tapered roots for overgrown rooms at fine scales: each stays inside the
/// metre-square column the coarse root fills, so it opens no less floor.
pub fn root(r: &Room, rx: f32, rz: f32, fy: f32, v: f32) -> Option<B> {
    let (gx, gz) = ((rx / 3.0).floor() as i32, (rz / 3.0).floor() as i32);
    let pick = hash2(gx, gz, r.salt.wrapping_add(3));
    if pick <= 0.6 {
        return None;
    }
    let (ox, oz) = (gx as f32 * 3.0 + 1.5, gz as f32 * 3.0 + 1.5);
    let bottom = r.h - 1.0 - pick * 2.5;
    if fy <= bottom {
        return None;
    }
    // 0 at the ceiling, 1 at the tip.
    let t = ((r.h + 1.0 - fy) / (r.h + 1.0 - bottom)).clamp(0.0, 1.0);
    let sway = 0.25 * t;
    let wx = ox + (value_noise3(fy * 0.8, 0.0, 0.5, r.salt.wrapping_add(gx as u32)) - 0.5) * 2.0 * sway;
    let wz = oz + (value_noise3(fy * 0.8, 0.5, 0.0, r.salt.wrapping_add(gz as u32)) - 0.5) * 2.0 * sway;
    let radius = 0.45 * (1.0 - t * t) + v * 0.5;
    let (dx, dz) = (rx - wx, rz - wz);
    let limit = 0.5_f32.max(v);
    if dx.hypot(dz) >= radius || (rx - ox).abs() >= limit || (rz - oz).abs() >= limit {
        return None;
    }
    let (px, py, pz) = ((rx / v).floor() as i32, (fy / v).floor() as i32, (rz / v).floor() as i32);
    let fringe = dx.hypot(dz) > radius - v && hash3(px, py, pz, r.salt.wrapping_add(63)) > 0.55;
    Some(if fringe { B::Leaves } else { B::Log })
}

/// Vines down a passage wall from the springing of its roof, in damp
/// complexes; `roof` is the roof height over this column.
pub fn passage_vine(c: &Complex, m: Vec3, fy: f32, v: f32, side: f32, roof: f32) -> Option<B> {
    let wet = job_wet(c.job);
    if v > 0.5 || wet < 0.2 || side >= v {
        return None;
    }
    strand(m, v, fy, (roof, WALL_VINE), wet * 0.7, c.seed.wrapping_add(64))
}
