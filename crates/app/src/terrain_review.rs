//! Opt-in generated-world review only. No save is opened by the benchmark path.
use glam::{IVec3, Vec3};
use crate::{player::Player, world::{BlockId, World}};
use std::sync::{Mutex, OnceLock};

pub fn option(name: &str) -> bool {
    std::env::var(name).is_ok_and(|v| v == "1")
}

/// Capture the ordinary loading display while its simulation is running.
pub fn generation_capture() -> Option<std::path::PathBuf> {
    if !option("VOXEL_CAPTURE_GENERATION") { return None; }
    static START: OnceLock<std::time::Instant> = OnceLock::new();
    static TICK: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(u64::MAX);
    let tick = (START.get_or_init(std::time::Instant::now).elapsed().as_secs_f64() * 2.0) as u64;
    if TICK.swap(tick, std::sync::atomic::Ordering::Relaxed) == tick { return None; }
    let root = std::env::var_os("VOXEL_CAPTURE_DIR").map(std::path::PathBuf::from)
        .unwrap_or_else(|| "target/benchmark-frames".into());
    Some(root.join(format!("generation-{tick:04}.png")))
}

/// Start the real-input sprint replay on naturally generated Badlands.
pub fn place_badlands_sprint(world: &World, player: &mut Player) -> bool {
    use engine_worldgen::landscape::Biome;
    let cfg = world.sample_config();
    let mut offsets: Vec<_> = (-8_i32..=8).flat_map(|z| (-8_i32..=8).map(move |x| (x, z))).collect();
    offsets.sort_by_key(|&(x, z)| x * x + z * z);
    for (dx, dz) in offsets {
        let (x, z) = (568 + dx * 256, -410 + dz * 256);
        if engine_worldgen::landscape::sample(&cfg, x, z).biome != Biome::Badlands {
            continue;
        }
        let y = world.terrain_height_at(x, z) + 1;
        if y <= cfg.water_level() || (0..=2).any(|dy| world.get(IVec3::new(x, y + dy, z)).is_solid()) {
            continue;
        }
        player.pos = Vec3::new(x as f32 + 0.5, y as f32, z as f32 + 0.5);
        player.precise_pos = player.pos.as_dvec3();
        player.vel = Vec3::ZERO;
        player.yaw = std::f32::consts::FRAC_PI_2;
        player.pitch = 0.0;
        player.flying = false;
        println!("[badlands-sprint] seed={} generator={} start={:?} facing=east",
            world.seed, world.generator_version, player.pos);
        return true;
    }
    false
}

#[derive(Default)]
pub struct SprintRoute {
    progress_x: Option<f32>,
    blocked_seconds: f32,
    detour_seconds: f32,
    attempts: u32,
}

impl SprintRoute {
    pub fn steer(&mut self, player: &Player, input: &mut crate::player::Input, dt: f32) {
        // Keep holding sprint east. Skirt walls using the normal strafe and
        // jump controls so a natural obstacle cannot turn this into an idle run.
        if self.detour_seconds > 0.0 {
            self.detour_seconds -= dt;
            input.right = self.attempts % 8 < 4;
            input.left = !input.right;
            input.jump |= player.vel.x < 1.0;
            self.progress_x = Some(player.pos.x);
            return;
        }
        let progress_x = *self.progress_x.get_or_insert(player.pos.x);
        if player.pos.x - progress_x >= 0.5 {
            self.progress_x = Some(player.pos.x);
            self.blocked_seconds = 0.0;
            return;
        }
        self.blocked_seconds += dt;
        if self.blocked_seconds >= 0.4 {
            self.blocked_seconds = 0.0;
            self.detour_seconds = 1.8;
            self.attempts += 1;
            println!("[badlands-sprint] detour={} position={:?}", self.attempts, player.pos);
        }
    }
}
pub fn fixed_exposure() -> Option<f32> {
    static VALUE: OnceLock<Option<f32>> = OnceLock::new();
    *VALUE.get_or_init(|| std::env::var("VOXEL_TERRAIN_EXPOSURE").ok()
        .and_then(|v| v.parse::<f32>().ok()).filter(|v| v.is_finite() && *v > 0.0))
}
pub fn light_mode() -> &'static str {
    static VALUE: OnceLock<String> = OnceLock::new();
    VALUE.get_or_init(|| std::env::var("VOXEL_TERRAIN_LIGHT").unwrap_or_default()).as_str()
}
pub fn wind_override() -> Option<f32> {
    static VALUE: OnceLock<Option<f32>> = OnceLock::new();
    *VALUE.get_or_init(|| std::env::var("VOXEL_TERRAIN_WIND").ok()
        .and_then(|v| v.parse::<f32>().ok()).filter(|v| v.is_finite()).map(|v| v.clamp(0.0, 20.0)))
}

struct Route { cap: IVec3, last_stage: i32 }

/// Pours a water block into the air ahead of the camera every 20 s after warmup.
fn flood(player: &Player, world: &mut World, elapsed: f32,
    dirty_blocks: &mut std::collections::HashSet<IVec3>,
    dirty_chunks: &mut std::collections::HashSet<IVec3>) {
    static LAST: Mutex<i32> = Mutex::new(-1);
    let stage = ((elapsed - 15.0) / 20.0).floor() as i32;
    let Ok(mut last) = LAST.lock() else { return };
    if stage < 0 || *last == stage { return; }
    *last = stage;
    let scale = world.voxel_scale.meters();
    let forward = Vec3::new(player.yaw.sin(), 0.0, -player.yaw.cos());
    let center = (player.pos + forward * (10.0 / scale) + Vec3::Y * (3.0 / scale)).floor().as_ivec3();
    let r = (4.0 / scale) as i32;
    let mut placed = 0;
    for y in 0..r { for z in -r..r { for x in -r..r {
        let p = center + IVec3::new(x, y, z);
        if world.get(p) != BlockId::Air { continue; }
        world.set_water_volume(p, 8);
        dirty_blocks.insert(p);
        crate::world::mark_chunk_and_neighbors_dirty(dirty_chunks, p);
        placed += 1;
    }}}
    eprintln!("[cave-flood] stage={stage} center={center:?} placed={placed}");
}
/// Called after the fixed camera is reset each frame. Movement is a pure
/// elapsed-time offset; edit steps use World::set_many and normal remeshing.
pub fn frame(player: &mut Player, world: &mut World, elapsed: f32,
    dirty_blocks: &mut std::collections::HashSet<IVec3>,
    dirty_chunks: &mut std::collections::HashSet<IVec3>) {
    material_distance_fixture(player, world, dirty_chunks);
    static ROUTE: OnceLock<bool> = OnceLock::new();
    static EDIT: OnceLock<bool> = OnceLock::new();
    static FLOOD: OnceLock<bool> = OnceLock::new();
    let route = *ROUTE.get_or_init(|| option("VOXEL_TERRAIN_ROUTE"));
    let edit = *EDIT.get_or_init(|| option("VOXEL_TERRAIN_EDITS"));
    if *FLOOD.get_or_init(|| option("VOXEL_CAVE_FLOOD")) {
        flood(player, world, elapsed, dirty_blocks, dirty_chunks);
    }
    if !route && !edit { return; }
    let scale = world.voxel_scale.meters();
    let t = (elapsed - 15.0).max(0.0);
    if edit {
        static STATE: OnceLock<Mutex<Route>> = OnceLock::new();
        let mut state = STATE.get_or_init(|| {
            // Deliberately straddle negative X and a vertical chunk boundary.
            let x = -1;
            let z = (player.pos.z - 10.0 / scale).floor() as i32;
            let y = world.terrain_height_at(x, z);
            let support_y = y.div_euclid(32) * 32 + 31;
            Mutex::new(Route {cap: IVec3::new(x, support_y + 1, z), last_stage: -1})
        }).lock().unwrap();
        let stage = (t / 6.0) as i32;
        if state.last_stage != stage {
            let cap = state.cap;
            let mode = stage % 6;
            let capped = mode == 1;
            let mut edits = Vec::new();
            if state.last_stage < 0 {
                for x in -4..=3 { for z in -4..=4 { for y in -4..=5 {
                    edits.push((IVec3::new(x, cap.y + y, cap.z + z), BlockId::Air));
                }}}
            }
            for x in -1..=0 {
                for z in -2..=2 {
                    let p = IVec3::new(x, cap.y, cap.z + z);
                    for y in 2..=4 { edits.push((p - IVec3::Y * y, BlockId::Dirt)); }
                    edits.push((p - IVec3::Y, if mode == 4 { BlockId::Air } else { BlockId::Grass }));
                    edits.push((p, if capped { BlockId::Basalt } else { BlockId::Air }));
                    edits.push((p + IVec3::Y * 2, if mode == 3 { BlockId::Stone } else { BlockId::Air }));
                }
            }
            world.set_many(&edits);
            for &(p, _) in &edits {
                dirty_blocks.insert(p);
                crate::world::mark_chunk_and_neighbors_dirty(dirty_chunks, p);
            }
            eprintln!("[terrain-edit] stage={stage} cap={cap:?} capped={capped} mode={mode} revision={}", world.revision());
            state.last_stage = stage;
        }
        player.pos = Vec3::new(-0.5, state.cap.y as f32 + 4.0 / scale, state.cap.z as f32 + 8.0 / scale);
        player.yaw = 0.0;
        player.pitch = -0.48;
        // Leave the four-chunk resident render radius for twelve seconds,
        // editing while away, then return to demand the edited mesh again.
        if matches!((t / 6.0) as i32, 6 | 7) { player.pos.x += 300.0 / scale; }
    }
    if route {
        // 40 s cycle: forward walk, sidestep, pan, grazing traverse, flight.
        let phase = t.rem_euclid(40.0);
        let (offset, yaw, pitch) = if phase < 8.0 {
            (Vec3::new(0.0, 0.0, -phase * 0.35), 0.0, 0.0)
        } else if phase < 16.0 {
            (Vec3::new((phase - 8.0) * 0.35, 0.0, -2.8), 0.0, 0.0)
        } else if phase < 24.0 {
            (Vec3::new(2.8, 0.0, -2.8), (phase - 16.0) * 0.08, 0.0)
        } else if phase < 32.0 {
            (Vec3::new(2.8 - (phase - 24.0) * 0.7, -0.8, -2.8), 0.64, 0.13)
        } else {
            let rise = ((phase - 32.0) * std::f32::consts::PI / 8.0).sin() * 28.0;
            (Vec3::new(-2.8, rise, -2.8), 0.64, -0.1)
        };
        player.pos += offset / scale;
        player.yaw += yaw;
        player.pitch += pitch;
    }
}

/// Physical-scale material test bed for generated benchmark worlds only.
fn material_distance_fixture(player: &mut Player, world: &mut World,
    dirty_chunks: &mut std::collections::HashSet<IVec3>) {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    if !*ENABLED.get_or_init(|| option("VOXEL_MATERIAL_DISTANCE_REVIEW")) { return; }
    static PLACED: OnceLock<i32> = OnceLock::new();
    let scale = world.voxel_scale.meters();
    let ground = *PLACED.get_or_init(|| {
        let ground = world.terrain_height_at(0, 0);
        let cell = |m: f32| (m / scale).round() as i32;
        let mut edits = Vec::new();
        for z in cell(-64.0)..cell(8.0) {
            for x in cell(-12.0)..cell(12.0) {
                let block = if x < cell(-4.0) { BlockId::Stone }
                    else if x < cell(4.0) { BlockId::StoneBricks }
                    else { BlockId::Cobblestone };
                edits.push((IVec3::new(x, ground, z), block));
                let wall = x == cell(-12.0) || x == cell(4.0) || z == cell(-48.0);
                for y in 1..=cell(3.0) {
                    edits.push((IVec3::new(x, ground + y, z),
                        if wall && y <= cell(1.0) { block } else { BlockId::Air }));
                }
            }
        }
        world.set_many(&edits);
        for &(position, _) in &edits {
            crate::world::mark_chunk_and_neighbors_dirty(dirty_chunks, position);
        }
        eprintln!("[material-distance-review] scale_m={scale} ground={ground} edits={} lanes=stone,stonebrick,cobblestone length_m=72", edits.len());
        ground
    });
    player.pos.y = ground as f32 + 1.0 + 0.15 / scale;
}
