//! Terrain weights for the cloud shell's vortex-street obstacles.
//!
//! The march sheds streets from hashed lattice sites (`cloud_flow_shared.wgsl`).
//! From orbit the shell keeps those sites but weights each by the ground under
//! it: isolated peaks and islands shed strong streets, flat ground and open sea none.

use super::chart::{HALF_M, PITCH_M};
use engine_voxel::WorldSampleConfig;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

/// `FLOW_CELL_M` in cloud_flow_shared.wgsl.
pub const CELL_M: f64 = 9_000.0;
/// Lattice window, `GLOBE_OBSTACLE_*` in globe_shell.wgsl: every cell a face
/// point or its 3x3 neighbourhood can touch.
pub const X0: i32 = -4;
pub const Z0: i32 = -4;
pub const COLUMNS: i32 = 45;
pub const ROWS: i32 = 8;
pub const CELLS: usize = (COLUMNS * ROWS) as usize;
const RING_M: f64 = 1_500.0;
const RING: usize = 8;
const FOOTPRINT_M: f32 = 400.0;

/// `flow_hash_cell1` in cloud_flow_shared.wgsl, bit for bit.
pub fn flow_hash(c: (i32, i32), salt: i32) -> f32 {
    let mut h = c.0.wrapping_mul(1619).wrapping_add(c.1.wrapping_mul(31337))
        .wrapping_add(salt.wrapping_mul(6971)).wrapping_add(1_013_904_223) as u32;
    h ^= h >> 16;
    h = h.wrapping_mul(0x045d_9f3b);
    h ^= h >> 16;
    h = h.wrapping_mul(0xac4d_1c7f);
    h ^= h >> 16;
    h as f32 / 4_294_967_296.0
}

/// `cloud_vortex_site` in cloud_flow_shared.wgsl, in chart metres.
pub fn site(c: (i32, i32)) -> (f64, f64) {
    let jitter = (flow_hash(c, 7412), flow_hash(c, 7412 + 7919));
    ((f64::from(c.0) + f64::from(jitter.0)) * CELL_M, (f64::from(c.1) + f64::from(jitter.1)) * CELL_M)
}

/// Index of lattice cell `c` in the uploaded table.
pub fn index(c: (i32, i32)) -> Option<usize> {
    let (x, z) = (c.0 - X0, c.1 - Z0);
    ((0..COLUMNS).contains(&x) && (0..ROWS).contains(&z)).then(|| (x * ROWS + z) as usize)
}

/// Street strength from a site's height and its ring, all in metres above sea.
/// Islands and steep isolated peaks shed streets; plains, plateaus and sea do not.
pub fn weight(centre_m: f64, ring_m: &[f64]) -> f32 {
    if centre_m <= 0.0 || ring_m.is_empty() { return 0.0; }
    let n = ring_m.len() as f64;
    let wet = ring_m.iter().filter(|h| **h <= 0.0).count() as f64 / n;
    let mean = ring_m.iter().map(|h| h.max(0.0)).sum::<f64>() / n;
    let island = smoothstep(0.45, 0.75, wet);
    let peak = smoothstep(150.0, 600.0, centre_m - mean);
    (1.3 * island).max(1.5 * peak).min(1.6) as f32
}

fn smoothstep(a: f64, b: f64, x: f64) -> f64 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Surface height above sea level in metres at chart point `(x, z)`, or `None` off the faces.
fn height_above_sea(cfg: &WorldSampleConfig, x: f64, z: f64) -> Option<f64> {
    let face = (x / PITCH_M).round();
    if !(0.0..6.0).contains(&face) || (x - face * PITCH_M).abs() > HALF_M || z.abs() > HALF_M { return None; }
    let vm = f64::from(cfg.voxel_meters());
    let (vx, vz) = ((x / vm).floor() as i32, (z / vm).floor() as i32);
    let height = if engine_worldgen::planetary::active(cfg) {
        engine_worldgen::planetary::column_with_water(cfg, vx, vz, FOOTPRINT_M).0.height
    } else {
        super::bake::surface_at(cfg, vx, vz, f64::from(FOOTPRINT_M)).0
    };
    Some(f64::from(height - cfg.water_level()) * vm)
}

/// The full table for one world; sites off every face weigh zero.
pub fn bake(cfg: &WorldSampleConfig) -> Vec<f32> {
    let mut out = vec![0.0; CELLS];
    for cx in X0..X0 + COLUMNS {
        for cz in Z0..Z0 + ROWS {
            let (x, z) = site((cx, cz));
            let Some(centre) = height_above_sea(cfg, x, z) else { continue };
            let ring: Vec<f64> = (0..RING).filter_map(|k| {
                let a = k as f64 * std::f64::consts::TAU / RING as f64;
                height_above_sea(cfg, x + a.cos() * RING_M, z + a.sin() * RING_M)
            }).collect();
            if let Some(i) = index((cx, cz)) { out[i] = weight(centre, &ring); }
        }
    }
    out
}

type Key = String;
enum Slot { Baking, Ready(Arc<[f32]>) }

fn table() -> &'static Mutex<HashMap<Key, Slot>> {
    static TABLE: OnceLock<Mutex<HashMap<Key, Slot>>> = OnceLock::new();
    TABLE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Cached weights for `cfg`; the first call starts a background bake and returns `None`.
pub fn weights(cfg: &WorldSampleConfig) -> Option<Arc<[f32]>> {
    let key = super::cache::folder_name(cfg);
    let mut map = table().lock().ok()?;
    match map.get(&key) {
        Some(Slot::Ready(w)) => return Some(w.clone()),
        Some(Slot::Baking) => return None,
        None => {}
    }
    map.insert(key.clone(), Slot::Baking);
    drop(map);
    let (cfg, slot) = (*cfg, key.clone());
    let spawned = std::thread::Builder::new().name("cloud obstacles".into()).spawn(move || {
        let weights: Arc<[f32]> = bake(&cfg).into();
        if let Ok(mut map) = table().lock() { map.insert(slot, Slot::Ready(weights)); }
    });
    if spawned.is_err() && let Ok(mut map) = table().lock() { map.remove(&key); }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_matches_the_shader_lattice() {
        // Values from the WGSL integer mix; a change in either side breaks the sites.
        let h = flow_hash((0, 0), 7411);
        assert!((0.0..1.0).contains(&h));
        assert_eq!(flow_hash((3, -2), 7412), flow_hash((3, -2), 7412));
        assert_ne!(flow_hash((3, -2), 7412), flow_hash((-2, 3), 7412));
    }

    #[test]
    fn the_window_covers_every_face_neighbourhood() {
        for face in 0..6 {
            for &(x, z) in &[(-HALF_M, -HALF_M), (HALF_M, HALF_M), (0.0, 0.0)] {
                let c = (((f64::from(face) * PITCH_M + x) / CELL_M).floor() as i32,
                    (z / CELL_M).floor() as i32);
                for dx in -1..=1 {
                    for dz in -1..=1 {
                        assert!(index((c.0 + dx, c.1 + dz)).is_some(), "face {face} {x} {z}");
                    }
                }
            }
        }
    }

    #[test]
    fn islands_and_peaks_shed_streets_plains_and_sea_do_not() {
        let sea = [-50.0; 8];
        let plain = [40.0; 8];
        assert!(weight(80.0, &sea) > 1.0);
        assert!(weight(900.0, &plain) > 1.0);
        assert_eq!(weight(60.0, &plain), 0.0);
        assert_eq!(weight(-20.0, &sea), 0.0);
    }
}
