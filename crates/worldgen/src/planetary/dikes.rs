//! Generator 298: dikes, sheets of basalt that cut the older rock.
//!
//! Magma rising under tension opens a crack perpendicular to the least
//! compressive stress and freezes in it (Anderson 1951). The sheet is
//! therefore near vertical, a few metres wide, and younger than every bed it
//! crosses: the cross-cutting relationship a field geologist dates rock by.
//! Two settings produce them here, both read from the plate solve:
//!
//! - Rift swarms. Where plates pull apart, extension is across the boundary,
//!   so dikes strike along it. The 293 fissures are the same cracks left
//!   open at the surface.
//! - Radial swarms. A volcano's own load turns the stress field around it,
//!   and dikes run outward from the vent like spokes (Odé 1957; Spanish
//!   Peaks, Ship Rock).
//!
//! A long dike is a chain of shorter, slightly offset segments, so one
//! segment per grid cell is the natural unit and a voxel tests one candidate.
//! Most segments stop under the soil; the rest crop out as a basalt stripe.
//!
//! Limits of the model: segments are straight and vertical with a small
//! waver, widths are game-scale, the host rock is not baked or hardened at
//! the contact, and the surface is not raised into the wall a resistant dike
//! weathers out as.
use super::{direction, get};
use crate::volcano::Kind;
use crate::{WorldSampleConfig, hash2, value_noise2};
use engine_core::cube_sphere::atlas;
use std::cell::RefCell;
use std::collections::HashMap;
use std::f32::consts::FRAC_PI_2;

/// Side of the grid cell that holds one segment, metres.
const CELL: f32 = 192.0;
/// Extension stress above which a rift swarm intrudes (the fissure threshold).
const RIFTING: f32 = 0.25;
/// Radial swarms reach this many edifice radii from the vent.
const RADIAL_REACH: f64 = 3.5;
/// Soil and loose cover a blind segment stops under, metres.
const COVER_M: f32 = 3.0;

/// One dike segment, in world metres on the cell's face chart.
#[derive(Clone, Copy)]
struct Segment {
    cx: f32,
    cz: f32,
    /// Unit vector along strike.
    cos: f32,
    sin: f32,
    half_len: f32,
    half_w: f32,
    /// Depth below the surface where the sheet stops; zero crops out.
    roof: f32,
    phase: f32,
}

impl Segment {
    fn cuts(&self, x: f32, y: f32, z: f32, depth: f32) -> bool {
        if depth < self.roof { return false; }
        let (dx, dz) = (x - self.cx, z - self.cz);
        let along = dx * self.cos + dz * self.sin;
        let t = along / self.half_len;
        if t.abs() >= 1.0 { return false; }
        // A sheet wavers along strike and with depth; it does not lean.
        let waver = (along * 0.045 + self.phase).sin() * 1.2
            + (value_noise2(along * 0.05, y * 0.03, 29_801) - 0.5) * 1.6;
        let across = dz * self.cos - dx * self.sin - waver;
        // Blunt lens in plan: full width through the middle, closed at the tips.
        across.abs() < self.half_w * (1.0 - t * t * t * t)
    }
}

/// Stress setting at a cell centre: how likely a segment is, and its strike.
fn setting(cfg: &WorldSampleConfig, x: f32, z: f32, roll: f32) -> Option<f32> {
    let world = get(cfg);
    let at = |x: f32, z: f32| direction(cfg, f64::from(x), f64::from(z));
    let here = at(x, z);
    // Chart axes on the sphere, for bearings toward a vent.
    let (east, south) = (at(x + 64.0, z) - here, at(x, z + 64.0) - here);
    let radial = world.volcanoes.vents.iter()
        .filter(|v| v.kind != Kind::Cinder)
        .filter_map(|v| {
            let to = v.dir - here;
            let radii = to.length() * atlas::RADIUS_M / v.radius_m;
            // Outside the summit cone, fading to the edge of the swarm.
            let chance = 0.55 * (1.0 - (radii - 0.6) / (RADIAL_REACH - 0.6));
            (radii > 0.6 && chance > 0.0)
                .then(|| (chance as f32, to.dot(south).atan2(to.dot(east)) as f32))
        })
        .max_by(|a, b| a.0.total_cmp(&b.0));
    if let Some((chance, bearing)) = radial {
        return (roll < chance).then_some(bearing);
    }
    let plate = world.plates.at(here);
    let rift = (plate.rift as f32).clamp(0.0, 1.0);
    if rift <= RIFTING || roll >= 0.15 + 0.5 * rift { return None; }
    let boundary = |x: f32, z: f32| world.plates.at(at(x, z)).boundary_m;
    let gx = (boundary(x + 400.0, z) - boundary(x - 400.0, z)) as f32;
    let gz = (boundary(x, z + 400.0) - boundary(x, z - 400.0)) as f32;
    // The boundary runs across the gradient of the distance to it.
    (gx.hypot(gz) > 1.0).then(|| gz.atan2(gx) + FRAC_PI_2)
}

fn segment(cfg: &WorldSampleConfig, key: (i32, i32)) -> Option<Segment> {
    let h = |k: u32| hash2(key.0, key.1, cfg.seed.wrapping_add(29_810 + k));
    let (mx, mz) = ((key.0 as f32 + 0.5) * CELL, (key.1 as f32 + 0.5) * CELL);
    let strike = setting(cfg, mx, mz, h(1))? + (h(2) - 0.5) * 0.25;
    // Length, jitter and waver together stay inside the cell.
    Some(Segment {
        cx: mx + (h(3) - 0.5) * 24.0,
        cz: mz + (h(4) - 0.5) * 24.0,
        cos: strike.cos(),
        sin: strike.sin(),
        half_len: 42.0 + h(5) * 36.0,
        half_w: 0.7 + h(6) * h(6) * 2.3,
        roof: if h(7) < 0.4 { 0.0 } else { COVER_M },
        phase: h(8) * 6.0,
    })
}

/// Segments by cell for one world, with the last cell kept in hand:
/// neighbouring voxels ask for the same cell.
struct Cache {
    cfg: WorldSampleConfig,
    last: Option<((i32, i32), Option<Segment>)>,
    cells: HashMap<(i32, i32), Option<Segment>>,
}

thread_local! {
    static CACHE: RefCell<Option<Cache>> = const { RefCell::new(None) };
}

fn segment_in(cfg: &WorldSampleConfig, key: (i32, i32)) -> Option<Segment> {
    let hit = CACHE.with(|c| {
        let mut c = c.borrow_mut();
        let c = c.get_or_insert_with(|| Cache { cfg: *cfg, last: None, cells: HashMap::new() });
        if c.cfg != *cfg { *c = Cache { cfg: *cfg, last: None, cells: HashMap::new() }; }
        if let Some((k, v)) = c.last && k == key { return Some(v); }
        let v = *c.cells.get(&key)?;
        c.last = Some((key, v));
        Some(v)
    });
    if let Some(v) = hit { return v; }
    // Built outside the borrow: the build reads the planet solve.
    let v = segment(cfg, key);
    CACHE.with(|c| {
        if let Some(c) = c.borrow_mut().as_mut().filter(|c| c.cfg == *cfg) {
            if c.cells.len() >= 8192 { c.cells.clear(); }
            c.cells.insert(key, v);
            c.last = Some((key, v));
        }
    });
    v
}

/// True where a dike fills this voxel. `surface` is the column's top voxel.
pub(super) fn cuts(cfg: &WorldSampleConfig, p: glam::IVec3, surface: i32) -> bool {
    let (x, z) = (cfg.world_x(p.x), cfg.world_z(p.z));
    let key = ((x / CELL).floor() as i32, (z / CELL).floor() as i32);
    segment_in(cfg, key).is_some_and(|s| {
        s.cuts(x, cfg.world_y(p.y), z, cfg.world_y(surface) - cfg.world_y(p.y))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WorldType;
    use engine_core::VoxelScalePreset;

    /// Review dump: how many cells of a 24 km square hold a segment, and that
    /// the first one fills rock on its own centre line. `DIKE_SEED` picks the
    /// seed; run with `VOXEL_HISTORY_MYR=20` for a quick planet.
    #[test]
    #[ignore = "review dump"]
    fn dump_dike_swarms() {
        let seed = std::env::var("DIKE_SEED").ok().and_then(|s| s.parse().ok()).unwrap_or(74);
        let cfg = WorldSampleConfig { planet: None, generator_version: super::super::DIKE_SWARM_GENERATOR, seed,
            world_type: WorldType::Planet, voxel_scale: VoxelScalePreset::OneMeter };
        let span = (12_000.0 / CELL) as i32;
        let mut found = Vec::new();
        for kz in -span..span {
            for kx in -span..span {
                if let Some(s) = segment_in(&cfg, (kx, kz)) { found.push(s); }
            }
        }
        let cells = (2 * span).pow(2);
        let outcrops = found.iter().filter(|s| s.roof == 0.0).count();
        eprintln!("[dikes] seed {seed}: {} segments in {cells} cells, {outcrops} crop out", found.len());
        // Widest outcrops first: these are the ones a capture can find.
        let mut shown: Vec<&Segment> = found.iter().filter(|s| s.roof == 0.0).collect();
        shown.sort_by(|a, b| b.half_w.total_cmp(&a.half_w));
        for s in shown.iter().take(8) {
            eprintln!("[dikes]   x {:.0} z {:.0} strike {:.0} deg, {:.0} m long, {:.1} m wide",
                s.cx, s.cz, s.sin.atan2(s.cos).to_degrees(), s.half_len * 2.0, s.half_w * 2.0);
        }
        if let Some(s) = found.first() {
            let p = glam::IVec3::new(cfg.blocks_for_height(s.cx), 40, cfg.blocks_for_height(s.cz));
            let hits = (-4..=4).filter(|d| cuts(&cfg, p + glam::IVec3::new(*d, 0, 0), 60)
                || cuts(&cfg, p + glam::IVec3::new(0, 0, *d), 60)).count();
            eprintln!("[dikes]   {hits} of 9 voxels across the first centre are basalt");
            assert!(hits > 0, "a segment must fill rock near its own centre");
        }
    }
}
