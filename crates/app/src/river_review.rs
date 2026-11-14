//! Opt-in native observation of generated headwaters. No terrain edits or pools.
use crate::{player::Player, world::{World, WorldSampleConfig}};
use glam::{IVec3, Vec2, Vec3};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
static TRANSFERRED: AtomicU64 = AtomicU64::new(0);
static FALLEN: AtomicU64 = AtomicU64::new(0);
static SOURCE: OnceLock<Option<Vec3>> = OnceLock::new();
static SECTION: OnceLock<(Vec3, Vec3, f32)> = OnceLock::new();
static SECTION_NET: AtomicI64 = AtomicI64::new(0);
static LOCAL_TRANSFERRED: AtomicU64 = AtomicU64::new(0);

pub fn transfer(from: IVec3, to: IVec3, amount: u64) {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    if !*ENABLED.get_or_init(|| std::env::var_os("VOXEL_RIVER_REVIEW").is_some()) { return; }
    TRANSFERRED.fetch_add(amount, Ordering::Relaxed);
    if to.y < from.y { FALLEN.fetch_add(amount, Ordering::Relaxed); }
    let Some(&(center, downstream, scale)) = SECTION.get() else { return; };
    let a = (from.as_vec3() + Vec3::splat(0.5)) * scale - center;
    let b = (to.as_vec3() + Vec3::splat(0.5)) * scale - center;
    if a.xz().length_squared() < 16.0 * 16.0 && (-5.0..=2.0).contains(&a.y) {
        LOCAL_TRANSFERRED.fetch_add(amount, Ordering::Relaxed);
    }
    let da = a.dot(downstream);
    let db = b.dot(downstream);
    // A signed cross-section counts net throughput, not every internal face.
    if (da < 0.0) == (db < 0.0) { return; }
    let crossing = a.lerp(b, da / (da - db));
    if crossing.xz().length_squared() <= 8.0 * 8.0 && (-5.0..=2.0).contains(&crossing.y) {
        let signed = amount.min(i64::MAX as u64) as i64;
        SECTION_NET.fetch_add(if db > da { signed } else { -signed }, Ordering::Relaxed);
    }
}

fn source(cfg: &WorldSampleConfig) -> Option<Vec3> {
    *SOURCE.get_or_init(|| {
        if std::env::var_os("VOXEL_RIVER_REVIEW").is_none() { return None; }
        let mut sources = if engine_worldgen::geo::active(cfg) {
            engine_worldgen::geo::river_sources(cfg, Vec2::ZERO, 32_000.0)
        } else { engine_worldgen::drainage::springs_near(cfg, (0, 0)) };
        sources.sort_by(|a, b| a.position_m.xz().length_squared().total_cmp(&b.position_m.xz().length_squared()));
        let selected = sources.first()?.position_m;
        if engine_worldgen::geo::active(cfg) {
            let planet = engine_worldgen::geo::planet_for(cfg);
            if let Some((i, _)) = planet.node.iter().enumerate().min_by(|(_, a), (_, b)| {
                (Vec2::from_array(**a) - selected.xz()).length_squared()
                    .total_cmp(&(Vec2::from_array(**b) - selected.xz()).length_squared())
            }) {
                let receiver = Vec2::from_array(planet.node[planet.receiver[i] as usize]);
                let d = (receiver - selected.xz()).normalize_or_zero();
                if d.length_squared() > 0.5 {
                    let downstream = Vec3::new(d.x, 0.0, d.y);
                    let center = selected + downstream * 6.0;
                    let _ = SECTION.set((center, downstream, cfg.voxel_meters()));
                    println!("[river-review] section_m={center:?} downstream={downstream:?} half_width_m=8");
                }
            }
        }
        println!("[river-review] natural_headwater_m={selected:?} generator={} seed={} sources={} terrain_edits=0", cfg.generator_version, cfg.seed, sources.len());
        Some(selected)
    })
}

pub fn camera(player: &mut Player, cfg: &WorldSampleConfig) -> bool {
    let Some(source) = source(cfg) else { return false; };
    let scale = cfg.voxel_meters();
    let offset = Vec3::new(13.0, 8.0, 16.0);
    player.pos = (source + offset) / scale;
    let direction = -offset.normalize();
    player.yaw = direction.x.atan2(-direction.z);
    player.pitch = direction.y.asin();
    true
}

pub fn observe(world: &World) {
    let Some(source) = source(&world.sample_config()) else { return; };
    let scale = world.voxel_scale.meters();
    let p = (source / scale).floor().as_ivec3();
    let mut water = 0u64;
    let mut moving = 0usize;
    let mut highest_speed = 0.0_f32;
    for z in -6..=6 {
        for x in -6..=6 {
            for y in -4..=1 {
                let q = p + IVec3::new(x, y, z);
                water = water.saturating_add(world.water_microlitres(q));
                let speed = world.water_velocity(q).length();
                moving += usize::from(speed > 0.001);
                highest_speed = highest_speed.max(speed);
            }
        }
    }
    let section_net = SECTION_NET.load(Ordering::Relaxed);
    static PREVIOUS: std::sync::Mutex<(u64, i64)> = std::sync::Mutex::new((0, 0));
    let mut section_lps = 0.0;
    if let Ok(mut previous) = PREVIOUS.lock() {
        let steps = world.environment.fixed_steps.saturating_sub(previous.0);
        if steps > 0 { section_lps = (section_net - previous.1) as f64 / 1_000_000.0 * 120.0 / steps as f64; }
        *previous = (world.environment.fixed_steps, section_net);
    }
    println!("[river-review] sampled_ul={water} wave_cells={moving} max_wave_speed={highest_speed:.5} external_added_ul={} transferred_ul={} fallen_ul={} local_transferred_ul={} section_net_ul={section_net} section_lps={section_lps:.4} pending={}", world.environment.ledger.edited_water_added_ul, TRANSFERRED.load(Ordering::Relaxed), FALLEN.load(Ordering::Relaxed), LOCAL_TRANSFERRED.load(Ordering::Relaxed), world.activity.pending());
}

use glam::Vec3Swizzles;
