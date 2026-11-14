//! Isolated native flight review through ordinary fixed-step movement.
use crate::{player::{Input, Player}, world::{World, WorldType}};
use glam::Vec2;
use std::sync::{Mutex, OnceLock};

#[derive(Default)]
struct State { ticks: u64, phase: u8, pending: Option<&'static str>, landed: bool }
static STATE: Mutex<State> = Mutex::new(State { ticks: 0, phase: 0, pending: None, landed: false });

#[derive(Default)]
struct Walk {
    next: usize,
    seconds: f32,
    grounded_steps: usize,
    last_jump: f32,
    pending: Option<String>,
}

static WALK: Mutex<Walk> = Mutex::new(Walk {
    next: 0, seconds: 0.0, grounded_steps: 0, last_jump: 0.0, pending: None,
});

/// Metre coordinates as `x,z;x,z;...`, using the benchmark world and its movement.
fn walk_route() -> &'static [Vec2] {
    static ROUTE: OnceLock<Vec<Vec2>> = OnceLock::new();
    ROUTE.get_or_init(|| {
        std::env::var("VOXEL_PLANET_WALK_REVIEW").ok().and_then(|route| {
            route.split(';').map(|point| {
                let (x, z) = point.split_once(',')?;
                let point = Vec2::new(x.parse().ok()?, z.parse().ok()?);
                point.is_finite().then_some(point)
            }).collect::<Option<Vec<_>>>()
        }).filter(|route| route.len() >= 2).unwrap_or_default()
    })
}

pub fn walking() -> bool { !walk_route().is_empty() }

pub fn install_walk(world: &World, player: &mut Player) -> bool {
    if world.world_type != WorldType::Planet || !walking() { return false; }
    let scale = world.voxel_scale.meters();
    let start = walk_route()[0] / scale;
    let height = world.terrain_height_at(start.x.floor() as i32, start.y.floor() as i32);
    *player = Player::new();
    player.pos = glam::Vec3::new(start.x, height as f32 + 1.2, start.y);
    player.precise_pos = player.pos.as_dvec3();
    *WALK.lock().expect("walk state") = Walk { next: 1, last_jump: -1.0, ..Walk::default() };
    eprintln!("[planet-walk] seed={} generator={} route_m={:?}", world.seed, world.generator_version, walk_route());
    true
}

fn walk_input(world: &World, player: &mut Player, input: &mut Input) {
    let mut walk = WALK.lock().expect("walk state");
    if walk.next == 0 { return; }
    input.clear();
    let Some(target) = walk_route().get(walk.next) else { return; };
    let scale = world.voxel_scale.meters();
    let at = Vec2::new(player.pos.x, player.pos.z) * scale;
    let toward = *target - at;
    walk.seconds += crate::simulation::STEP_SECONDS;
    walk.grounded_steps += usize::from(player.on_ground);
    player.yaw = toward.x.atan2(-toward.y);
    player.pitch = -0.10;
    if toward.length() <= 1.3 || walk.seconds >= 25.0 {
        let arrived = toward.length() <= 1.3;
        eprintln!("[planet-walk] leg={} arrived={arrived} distance_left_m={:.2} position_m={:?} grounded_steps={} seconds={:.2} flying={}",
            walk.next, toward.length(), player.pos * scale, walk.grounded_steps, walk.seconds, player.flying);
        walk.pending = Some(format!("walk-{:02}", walk.next));
        walk.next = if arrived { walk.next + 1 } else { walk_route().len() };
        walk.seconds = 0.0;
        walk.grounded_steps = 0;
        walk.last_jump = -1.0;
        return;
    }
    input.forward = true;
    let direction = toward.normalize_or_zero();
    let step = glam::Vec3::new(direction.x, 0.0, direction.y) * (0.9 / scale);
    if player.on_ground && walk.seconds - walk.last_jump >= 0.8
        && world.get((player.pos + step).floor().as_ivec3()).is_solid()
    {
        input.jump = true;
        walk.last_jump = walk.seconds;
    }
}

pub fn enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| crate::parse_benchmark_config().enabled
        && std::env::var("VOXEL_PLANET_FLIGHT_REVIEW").as_deref() == Ok("1"))
}

fn moon_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var("VOXEL_PLANET_MOON_REVIEW").as_deref() == Ok("1"))
}

/// Teleports `height` metres above the moon's ground, released from any earlier anchor.
fn place(world: &World, player: &mut Player, height: f64, flying: bool) {
    let scale = world.voxel_scale.meters() as f64;
    let Some(point) = super::influence::moon_ground_point(world.orbital.planet_tick, world.seed, height) else { return };
    player.anchor = None;
    player.precise_pos = point / scale;
    player.pos = player.precise_pos.as_vec3();
    player.vel = glam::Vec3::ZERO;
    player.flying = flying;
}

pub fn input(world: &World, player: &mut Player, input: &mut Input) {
    if world.world_type == WorldType::Planet && walking() {
        walk_input(world, player, input);
        return;
    }
    if !enabled() || world.world_type != WorldType::Planet { return; }
    let Ok(mut state) = STATE.lock() else { return; };
    input.clear();
    state.ticks += 1;
    let height = player.pos.y * world.voxel_scale.meters();
    player.flight.speed = 64.0;
    player.flight.boost = 4.0;
    match state.phase {
        0 if state.ticks >= 240 && moon_enabled() => { state.phase = 10; state.ticks = 0; }
        10 => {
            place(world, player, 12_000.0, true);
            player.pitch = -0.6;
            if state.ticks >= 120 { state.pending = Some("07-moon-far"); state.phase = 11; state.ticks = 0; }
        }
        11 if state.ticks == 1 => { place(world, player, 600.0, false); player.pitch = -1.2; }
        11 if state.ticks == 420 => state.pending = Some("08-moon-fall"),
        11 if state.ticks == 440 => { place(world, player, 0.0, false); state.phase = 12; state.ticks = 0; }
        12 if state.ticks == 1 => { player.pitch = 0.05; }
        12 if state.ticks == 420 => state.pending = Some("09-moon-surface"),
        12 if state.ticks == 430 => { player.pitch = 0.9; }
        12 if state.ticks == 520 => { state.pending = Some("10-moon-sky"); state.phase = 13; state.ticks = 0; }
        13 => {
            player.pitch = -0.15;
            input.forward = true;
            if state.ticks == 600 { state.pending = Some("11-moon-walk"); state.phase = 14; }
        }
        0 if state.ticks >= 240 => {
            state.pending = Some("01-ground"); state.phase = 1;
            player.flying = true;
            player.pitch = -0.25;
        }
        1 => {
            player.flying = true;
            input.jump = true; input.sprint = true;
            if height >= 1000.0 { state.pending = Some("02-ascent"); state.phase = 2; }
        }
        2 => {
            player.flying = true;
            input.jump = true; input.sprint = true;
            player.pitch = -1.50;
            if height >= 24_000.0 {
                state.pending = Some("03-space"); state.phase = 3; state.ticks = 0;
            }
        }
        3 => {
            // Traverse a face boundary in the actual movement path.
            player.flying = true; input.forward = true; input.sprint = true;
            if state.ticks >= 1200 {
                state.pending = Some("04-face-crossing"); state.phase = 4;
            }
        }
        4 => {
            player.flying = true; input.sneak = true; input.sprint = true;
            player.pitch = -0.55;
            if height < 500.0 { state.pending = Some("05-descent"); state.phase = 5; }
        }
        5 => {
            input.sneak = true;
            if player.on_ground {
                state.phase = 6; state.landed = true; state.ticks = 0;
            }
        }
        6 if state.ticks >= 240 => { state.pending = Some("06-landed"); state.phase = 7; }
        _ => {}
    }
}

pub fn capture(world: &World, player: &Player) -> Option<std::path::PathBuf> {
    if world.world_type == WorldType::Planet && walking() {
        let name = WALK.lock().ok()?.pending.take()?;
        let root = std::path::PathBuf::from(std::env::var_os("VOXEL_CAPTURE_DIR")?);
        return Some(root.join(format!("{name}.png")));
    }
    if !enabled() { return None; }
    let Ok(mut state) = STATE.lock() else { return None; };
    static FRAMES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    if FRAMES.fetch_add(1, std::sync::atomic::Ordering::Relaxed) % 600 == 0 {
        eprintln!("[planet-flight] phase={} ticks={} anchored={} ground={}", state.phase, state.ticks, player.anchor.is_some(), player.on_ground);
    }
    let name = state.pending.take()?;
    let root = std::path::PathBuf::from(std::env::var_os("VOXEL_CAPTURE_DIR")?);
    let (face, local) = engine_core::cube_sphere::atlas::split(player.precise_pos * world.voxel_scale.meters() as f64);
    eprintln!("[planet-flight] stage={name} face={face:?} altitude_m={:.3} position={:?} grounded={} landed={} anchor={:?}",
        local.y, player.precise_pos, player.on_ground, state.landed, player.anchor.map(|a| a.pos));
    Some(root.join(format!("{name}.png")))
}
