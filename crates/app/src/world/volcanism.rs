//! Eruptions from active planet vents and earthquakes along plate boundaries.
//!
//! Stateless: every event is a hash of the world seed, the vent and the game
//! clock, so a reload or a peer replays the same schedule.
use crate::world::{BlockId, DayNightCycle, World, WorldType, effects};
use engine_core::{Budget, cube_sphere::{Projection, atlas}};
use engine_worldgen::volcano::{self, Kind, Vent};
use glam::{DVec3, IVec3, Vec3};
use std::collections::HashSet;
use std::sync::atomic::{AtomicU32, Ordering};

/// Seconds between eruption chances for one vent.
const ERUPTION_EPOCH_S: f64 = 900.0;
const ERUPTION_S: f64 = 50.0;
/// Seconds between quake chances in one tectonic cell.
const QUAKE_EPOCH_S: f64 = 140.0;
const QUAKE_S: f64 = 5.0;
/// Vents further than this neither erupt nor shake the camera.
const REACH_M: f64 = 2_500.0;
/// Lava cells a peak eruption adds per fixed step; ash cells every few steps.
const LAVA_PER_STEP: u32 = 1;

/// Camera tremor in metres, published for the frame thread.
static TREMOR_M: AtomicU32 = AtomicU32::new(0);

fn hash(a: u64, b: u64, seed: u32) -> f64 {
    let mut h = a.wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ b.wrapping_mul(0xc2b2_ae3d_27d4_eb4f) ^ u64::from(seed);
    h ^= h >> 31;
    h = h.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    h ^= h >> 29;
    (h >> 11) as f64 / (1u64 << 53) as f64
}

fn vent_key(v: &Vent) -> u64 {
    let q = (v.dir * 1.0e6).round().as_i64vec3();
    (q.x as u64).wrapping_mul(73_856_093) ^ (q.y as u64).wrapping_mul(19_349_663) ^ (q.z as u64).wrapping_mul(83_492_791)
}

/// 0..1 eruption strength of one vent at game time `t`.
pub fn eruption(seed: u32, v: &Vent, t: f64) -> f64 {
    if !v.active {
        return 0.0;
    }
    let epoch = (t / ERUPTION_EPOCH_S).floor();
    let chance = match v.kind { Kind::Cinder => 0.35, Kind::Strato => 0.25, Kind::Shield => 0.30, Kind::Caldera => 0.05 };
    if hash(vent_key(v), epoch as u64, seed) > chance {
        return 0.0;
    }
    let into = t - epoch * ERUPTION_EPOCH_S;
    if into > ERUPTION_S {
        return 0.0;
    }
    (std::f64::consts::PI * into / ERUPTION_S).sin().powf(0.6)
}

/// 0..1 quake magnitude felt at `dir`, from the tectonic cell around it.
pub fn quake(seed: u32, dir: DVec3, t: f64) -> f64 {
    quake_with(seed, dir, t, volcano::tectonics(seed, dir))
}

fn quake_with(seed: u32, dir: DVec3, t: f64, plates: volcano::Tectonics) -> f64 {
    let fault = (-(plates.boundary_m / 1_200.0).powi(2)).exp() * (0.3 + 0.7 * plates.convergence.abs());
    if fault < 0.05 {
        return 0.0;
    }
    let cell = (dir * atlas::RADIUS_M / 3_000.0).floor().as_i64vec3();
    let key = (cell.x as u64).wrapping_mul(31) ^ (cell.y as u64).wrapping_mul(1_009) ^ (cell.z as u64).wrapping_mul(65_537);
    let epoch = (t / QUAKE_EPOCH_S).floor();
    let roll = hash(key, epoch as u64, seed.wrapping_add(17));
    if roll > 0.18 * fault {
        return 0.0;
    }
    let into = t - epoch * QUAKE_EPOCH_S - roll * (QUAKE_EPOCH_S - QUAKE_S);
    if !(0.0..QUAKE_S).contains(&into) {
        return 0.0;
    }
    // Sharp onset, slow coda.
    let envelope = (into / 0.4).min(1.0) * (-(into / (QUAKE_S * 0.45))).exp();
    envelope * (0.4 + 0.6 * hash(key, epoch as u64, seed.wrapping_add(29))) * fault
}

fn direction(m: Vec3) -> DVec3 {
    let (face, local) = atlas::split(DVec3::new(f64::from(m.x), 0.0, f64::from(m.z)));
    Projection::Tangent.to_sphere(face, local.x / atlas::HALF_M, local.z / atlas::HALF_M).try_normalize().unwrap_or(DVec3::Y)
}

fn game_seconds(day: &DayNightCycle) -> f64 {
    let length = f64::from(day.day_length_seconds());
    f64::from(day.day_index()) * length + f64::from(day.time_of_day()) * length
}

/// One fixed step: erupting vents spill lava and ash, quakes set the camera tremor.
pub fn tick(world: &mut World, eye_m: Vec3, day: &DayNightCycle,
    blocks: &mut HashSet<IVec3>, chunks: &mut HashSet<IVec3>, budget: Budget) {
    if world.world_type != WorldType::Planet || world.generator_version < volcano::VERSION || world.ca_view.is_some() {
        TREMOR_M.store(0, Ordering::Relaxed);
        return;
    }
    let (seed, t, here) = (world.seed, game_seconds(day), direction(eye_m));
    let step = (t * 120.0) as u64;
    let metres = f64::from(world.voxel_scale.meters());
    let cfg = world.sample_config();
    let mut tremor = quake_with(seed, here, t, volcano::tectonics_for(&cfg, here)) * 0.12;
    // The vent lattice is coarse; a recheck every half second is plenty.
    let vents = if step.is_multiple_of(60) || tremor > 0.0 { volcano::vents_near_for(&cfg, here, REACH_M) } else { Vec::new() };
    for v in &vents {
        let strength = eruption(seed, v, t);
        if strength <= 0.0 {
            continue;
        }
        let distance = (v.dir - here).length() * atlas::RADIUS_M;
        tremor = tremor.max(strength * 0.08 * (-(distance / 900.0)).exp());
        if !budget.has_time() {
            continue;
        }
        let body = atlas::from_body(v.dir * atlas::RADIUS_M);
        let (x, z) = ((body.x / metres).floor() as i32, (body.z / metres).floor() as i32);
        let top = engine_worldgen::sample_column(&world.sample_config(), x, z).height;
        let spread = (v.crater_radius_m() * 0.5 / metres).max(1.0);
        for k in 0..LAVA_PER_STEP * 60 {
            if hash(vent_key(v), step * 64 + u64::from(k), seed.wrapping_add(41)) > strength / 60.0 {
                continue;
            }
            // A fountain: cells land above the lake and spill over the rim.
            let r = |salt: u64| hash(vent_key(v), step * 64 + u64::from(k) + salt, seed) * 2.0 - 1.0;
            let lift = ((4.0 + 10.0 * strength * (r(3) + 1.0)) / metres) as i32;
            let cell = IVec3::new(x + (r(1) * spread) as i32, top + lift.max(1), z + (r(2) * spread) as i32);
            spill(world, cell, BlockId::Lava, blocks, chunks);
        }
        if step.is_multiple_of(6) {
            let r = |salt: u64| hash(vent_key(v), step + salt, seed.wrapping_add(53)) * 2.0 - 1.0;
            let ash = IVec3::new(x + (r(1) * 60.0 / metres) as i32, top + (40.0 / metres) as i32, z + (r(2) * 60.0 / metres) as i32);
            spill(world, ash, BlockId::Ash, blocks, chunks);
        }
    }
    if !vents.is_empty() || tremor > 0.0 || step.is_multiple_of(60) {
        TREMOR_M.store((tremor as f32).to_bits(), Ordering::Relaxed);
    }
}

fn spill(world: &mut World, cell: IVec3, block: BlockId, blocks: &mut HashSet<IVec3>, chunks: &mut HashSet<IVec3>) {
    if let Ok(commit) = effects::replace(world, cell, BlockId::Air, block) {
        commit.publish(world, blocks, chunks);
    }
}

/// Camera offset in metres for a frame at `time_s`: a few incommensurate wobbles scaled by the tremor.
pub fn shake(time_s: f32) -> Vec3 {
    let a = f32::from_bits(TREMOR_M.load(Ordering::Relaxed));
    if a <= 0.0 {
        return Vec3::ZERO;
    }
    let w = |f: f32, p: f32| (time_s * f + p).sin() * 0.6 + (time_s * f * 2.31 + p * 1.7).sin() * 0.4;
    Vec3::new(w(23.0, 0.0), w(17.0, 1.3) * 0.6, w(29.0, 2.1)) * a
}

#[cfg(test)]
mod tests {
    use super::*;

    fn active_vent() -> Vent {
        (0..64).flat_map(|i| {
            let a = f64::from(i) * 0.7;
            volcano::vents_near(7, DVec3::new(a.cos(), (a * 0.3).sin(), a.sin()).normalize(), 6_000.0)
        }).find(|v| v.active).expect("an active vent")
    }

    #[test]
    fn active_vents_erupt_sometimes_and_dormant_never() {
        let v = active_vent();
        let erupting = (0..400).filter(|e| eruption(7, &v, f64::from(*e) * ERUPTION_EPOCH_S + 20.0) > 0.5).count();
        assert!((10..300).contains(&erupting), "erupted {erupting} of 400 epochs");
        let dormant = Vent { active: false, ..v };
        assert!((0..400).all(|e| eruption(7, &dormant, f64::from(e) * ERUPTION_EPOCH_S + 20.0) == 0.0));
        // Quiet outside the eruption window.
        assert!((0..400).all(|e| eruption(7, &v, f64::from(e) * ERUPTION_EPOCH_S + ERUPTION_S + 1.0) == 0.0));
    }

    #[test]
    fn quakes_cluster_on_plate_boundaries_and_are_repeatable() {
        let (mut near, mut far) = (0, 0);
        for i in 0..3_000 {
            let a = f64::from(i) * 2.399_963;
            let y = 1.0 - 2.0 * (f64::from(i) + 0.5) / 3_000.0;
            let dir = DVec3::new((1.0 - y * y).sqrt() * a.cos(), y, (1.0 - y * y).sqrt() * a.sin());
            let felt = (0..200).any(|k| quake(7, dir, f64::from(k) * 37.0) > 0.0);
            let boundary = volcano::tectonics(7, dir).boundary_m < 800.0;
            match (felt, boundary) { (true, true) => near += 1, (true, false) => far += 1, _ => {} }
            assert_eq!(quake(7, dir, 1234.5), quake(7, dir, 1234.5));
        }
        assert!(near > 0 && near > far * 2, "near {near} far {far}");
    }

    #[test]
    fn shake_is_zero_without_tremor() {
        TREMOR_M.store(0, Ordering::Relaxed);
        assert_eq!(shake(3.0), Vec3::ZERO);
    }
}
