//! Drifting air matter for the sunless realms: ash, embers and spores in Hell,
//! frost glitter on the cold Nether plain, slow dust in the Manyfold and Hall.
use super::Model;
use crate::player::Player;
use crate::render::ChunkCache;
use crate::world::{BlockId, World, WorldType, chunk_coord_from_world_block};
use engine_core::hash::hash_u32;
use engine_worldgen::nether::Biome;
use glam::{DVec2, Vec3};

const TILE_M: f32 = 4.0;
const RADIUS_M: f32 = 18.0;
const COLUMN_M: f32 = 16.0;

/// One family of particles. `rise` is metres per second, negative to fall.
#[derive(Clone, Copy)]
struct Mote {
    /// Share of the four slots per tile that carry this family.
    share: f32,
    rise: f32,
    sway: f32,
    radius: f32,
    color: [f32; 3],
    glow: f32,
    alpha: f32,
    /// Embers pulse and die; dust holds steady.
    flicker: bool,
}

const ASH: Mote = Mote { share: 0.55, rise: -0.42, sway: 0.34, radius: 0.020, color: [0.36, 0.33, 0.32], glow: 0.05, alpha: 0.70, flicker: false };
const EMBER: Mote = Mote { share: 0.22, rise: 1.15, sway: 0.22, radius: 0.013, color: [1.0, 0.42, 0.10], glow: 1.4, alpha: 0.85, flicker: true };
const SPORE: Mote = Mote { share: 0.60, rise: 0.10, sway: 0.50, radius: 0.016, color: [0.95, 0.74, 0.30], glow: 0.55, alpha: 0.55, flicker: false };
const GRIT: Mote = Mote { share: 0.30, rise: -0.25, sway: 0.60, radius: 0.012, color: [0.20, 0.19, 0.20], glow: 0.03, alpha: 0.60, flicker: false };
const FROST: Mote = Mote { share: 0.40, rise: -0.18, sway: 0.40, radius: 0.012, color: [0.70, 0.86, 0.95], glow: 0.35, alpha: 0.60, flicker: true };
const FOXFIRE_SPORE: Mote = Mote { share: 0.05, rise: 0.06, sway: 0.45, radius: 0.010, color: [0.36, 0.95, 0.78], glow: 0.9, alpha: 0.6, flicker: true };
const DUST: Mote = Mote { share: 0.38, rise: 0.035, sway: 0.28, radius: 0.011, color: [0.78, 0.74, 0.66], glow: 0.22, alpha: 0.38, flicker: false };

/// The two families a place carries; slots alternate between them.
fn families(world: &World, x_m: f32, z_m: f32) -> [Mote; 2] {
    let cfg = world.sample_config();
    match world.world_type {
        WorldType::Nether if cfg.is_hell() => match engine_worldgen::nether::biome(&cfg, x_m, z_m) {
            Some(Biome::FungalHollows) => [SPORE, Mote { share: 0.15, ..ASH }],
            Some(Biome::CinderRifts) => [Mote { share: 0.55, ..EMBER }, Mote { share: 0.35, ..ASH }],
            Some(Biome::BasaltScarps) => [GRIT, Mote { share: 0.08, ..EMBER }],
            Some(Biome::AshBarrens) => [Mote { share: 0.85, ..ASH }, Mote { share: 0.10, ..EMBER }],
            None => [ASH, EMBER],
        },
        WorldType::Nether => [FROST, Mote { share: 0.0, ..FROST }],
        // Lightless: fine grit sifting from the roof and rare foxfire spores, the only
        // motes that show without a lamp.
        WorldType::Realm(engine_voxel::RealmKind::Lightless) => [Mote { share: 0.22, alpha: 0.45, ..GRIT }, FOXFIRE_SPORE],
        // Highfold: pollen blown off the island tops and the odd pale seed tuft.
        WorldType::Realm(engine_voxel::RealmKind::Highfold) => [
            Mote { share: 0.18, rise: 0.12, sway: 0.75, color: [0.96, 0.92, 0.70], glow: 0.15, alpha: 0.40, ..DUST },
            Mote { share: 0.06, rise: 0.25, sway: 0.9, radius: 0.02, color: [1.0, 1.0, 0.96], glow: 0.1, alpha: 0.5, ..DUST },
        ],
        // Gilded Cave: sand dust in the lamplight and a few glinting flecks.
        WorldType::Realm(engine_voxel::RealmKind::Gilded) => [
            Mote { share: 0.30, color: [0.86, 0.70, 0.48], ..DUST },
            Mote { share: 0.05, rise: 0.04, color: [1.0, 0.80, 0.34], glow: 0.9, alpha: 0.6, ..EMBER },
        ],
        _ => [DUST, Mote { share: 0.0, ..DUST }],
    }
}

pub(super) fn draw(mesh: &mut Model, world: &World, player: &Player, chunks: &ChunkCache) {
    if world.ca_view.is_some()
        || !matches!(world.world_type, WorldType::Nether | WorldType::Manyfold | WorldType::Hall | WorldType::Realm(_))
    {
        return;
    }
    let scale = world.voxel_scale.meters();
    let inv_scale = scale.recip();
    let eye_m = player.precise_eye(world.voxel_scale) * f64::from(scale);
    let eye = eye_m.as_vec3();
    let time = world.adventure.time;
    // One steady seeded wind per realm; the shell realms have still air.
    let angle = unit(world.seed ^ 0x4149_5257) * std::f32::consts::TAU;
    let breeze = match world.world_type {
        WorldType::Nether => 0.55,
        WorldType::Realm(engine_voxel::RealmKind::Highfold) => 1.4,
        _ => 0.0,
    };
    let wind = Vec3::new(angle.cos(), 0.0, angle.sin()) * breeze;
    let drift = wind.as_dvec3() * f64::from(time);
    let grid_eye = eye_m - drift;
    let tile = (DVec2::new(grid_eye.x, grid_eye.z) / f64::from(TILE_M)).floor().as_ivec2();

    let previous_alpha = mesh.alpha;
    for z in tile.y - 5..=tile.y + 5 {
        for x in tile.x - 5..=tile.x + 5 {
            let tile_seed = hash_u32(world.seed ^ hash_u32(x as u32) ^ (z as u32).rotate_left(16) ^ 0x5245_4c4d);
            let kinds = families(world, (x as f32 + 0.5) * TILE_M, (z as f32 + 0.5) * TILE_M);
            for i in 0..4_u32 {
                let seed = hash_u32(tile_seed ^ i);
                let mote = kinds[(i & 1) as usize];
                if unit(seed ^ 7) >= mote.share {
                    continue;
                }
                let speed = mote.rise * (0.7 + unit(seed ^ 1) * 0.6);
                let travel = f64::from(unit(seed ^ 2) * COLUMN_M) + f64::from(time) * f64::from(speed);
                let y = eye.y
                    + (travel - f64::from(eye.y) + f64::from(COLUMN_M * 0.5)).rem_euclid(f64::from(COLUMN_M)) as f32
                    - COLUMN_M * 0.5;
                let vertical_fade = ((COLUMN_M * 0.5 - (y - eye.y).abs()) / 2.5).clamp(0.0, 1.0);
                if vertical_fade <= 0.01 {
                    continue;
                }
                let phase = time * (0.5 + unit(seed ^ 8)) + unit(seed ^ 3) * std::f32::consts::TAU;
                let sway = Vec3::new(phase.sin(), (phase * 0.6).sin() * 0.35, (phase * 0.75).cos()) * mote.sway;
                let pos = Vec3::new(
                    ((f64::from(x) + f64::from(unit(seed ^ 4))) * f64::from(TILE_M) + drift.x) as f32,
                    y,
                    ((f64::from(z) + f64::from(unit(seed ^ 5))) * f64::from(TILE_M) + drift.z) as f32,
                ) + sway;
                let to_eye = eye - pos;
                let distance_squared = to_eye.length_squared();
                if !(0.36..RADIUS_M * RADIUS_M).contains(&distance_squared) {
                    continue;
                }
                let distance = distance_squared.sqrt();
                // Embers burn out in bursts; frost only catches the light now and then.
                let pulse = if mote.flicker {
                    let beat = (time * (1.3 + unit(seed ^ 9) * 2.2) + unit(seed ^ 10) * 9.0).sin();
                    (0.35 + 0.65 * beat).clamp(0.0, 1.0)
                } else {
                    1.0
                };
                let fade = ((RADIUS_M - distance) / 5.0).clamp(0.0, 1.0)
                    * vertical_fade
                    * ((distance - 0.6) / 1.4).clamp(0.0, 1.0)
                    * pulse;
                if fade <= 0.02 || !open(chunks, (pos * inv_scale).floor().as_ivec3()) {
                    continue;
                }
                let facing = to_eye / distance;
                let radius = mote.radius * (0.7 + unit(seed ^ 6) * 0.7);
                let right = Vec3::Y.cross(facing).try_normalize().unwrap_or(Vec3::X);
                let up = facing.cross(right) * radius;
                let side = right * radius;
                mesh.alpha = mote.alpha * fade;
                mesh.triangle([pos + up, pos - side, pos + side], facing, mote.color, mote.glow);
                mesh.triangle([pos - up, pos + side, pos - side], facing, mote.color, mote.glow);
            }
        }
    }
    mesh.alpha = previous_alpha;
}

fn unit(seed: u32) -> f32 {
    (hash_u32(seed) >> 8) as f32 / 16_777_216.0
}

/// Motes never show inside rock, lava or water; unloaded space stays empty.
fn open(chunks: &ChunkCache, p: glam::IVec3) -> bool {
    chunks
        .cpu_chunks
        .get(&chunk_coord_from_world_block(p))
        .is_some_and(|chunk| {
            let block = chunk.get_world(p);
            !block.is_solid() && !matches!(block, BlockId::Water | BlockId::Lava)
        })
}

/// Hush eyes: a pale pair for each colony member within sight. They hold at
/// the edge of the light, so the pair is what shows; a blink now and then.
pub(super) fn draw_hush(mesh: &mut Model, world: &World, player: &Player) {
    if world.world_type.realm().is_none() || world.ca_view.is_some() {
        return;
    }
    let scale = world.voxel_scale.meters();
    let eye = (player.precise_eye(world.voxel_scale) * f64::from(scale)).as_vec3();
    let time = world.adventure.time;
    let previous_alpha = mesh.alpha;
    for (i, h) in world.lightless.hush.iter().enumerate() {
        let head = Vec3::from_array(h.pos) + Vec3::Y * 0.9;
        let to_eye = eye - head;
        let distance = to_eye.length();
        if !(2.0..36.0).contains(&distance) {
            continue;
        }
        let seed = hash_u32(world.seed ^ (i as u32).wrapping_mul(0x9e37_79b9));
        // Closed for about a fifth of a second every few seconds.
        let beat = (time * (0.21 + unit(seed) * 0.12) + unit(seed ^ 1)).fract();
        if beat < 0.05 {
            continue;
        }
        let facing = to_eye / distance;
        let right = Vec3::Y.cross(facing).try_normalize().unwrap_or(Vec3::X);
        let up = facing.cross(right);
        let fade = ((36.0 - distance) / 8.0).clamp(0.0, 1.0);
        mesh.alpha = 0.9 * fade;
        for side in [-1.0f32, 1.0] {
            let c = head + right * (0.085 * side);
            let (r, u) = (right * 0.034, up * 0.022);
            mesh.triangle([c + u, c - r, c + r], facing, [0.88, 0.84, 1.0], 1.8);
            mesh.triangle([c - u, c + r, c - r], facing, [0.88, 0.84, 1.0], 1.8);
        }
    }
    mesh.alpha = previous_alpha;
}
