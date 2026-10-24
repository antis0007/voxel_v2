//! Opt-in cameras for the actual generated deep vault; never edits the world.
//! Camera routes inspect rendering only, not collision or player traversal.
use crate::{player::Player, world::{self, WorldSampleConfig}};
use glam::Vec3;
use std::sync::OnceLock;

pub fn camera(player: &mut Player, cfg: &WorldSampleConfig, elapsed: f32) -> bool {
    static MODE: OnceLock<String> = OnceLock::new();
    let mode = MODE.get_or_init(|| std::env::var("VOXEL_REVIEW_DUNGEON").unwrap_or_default());
    if let Some((eye, target)) = view(mode, cfg) {
        return place(player, cfg, mode, eye, target);
    }
    if mode.is_empty() || cfg.generator_version < 28 { return false; }
    let floor = world::sites::holy_ground(cfg) - 79.0;
    let jib_side = if cfg.seed.wrapping_add(303) & 1 == 0 { 1.0 } else { -1.0 };
    // Positions and targets are room-local metres relative to the west mouth.
    let (eye, target) = match mode.as_str() {
        "lower" => (Vec3::new(47.0, -4.3, -11.0 * jib_side), Vec3::new(39.0, 6.0, 6.0 * jib_side)),
        "reverse" => (Vec3::new(59.0, 1.7, 0.0), Vec3::new(39.0, 6.0, jib_side)),
        "roof" => (Vec3::new(43.0, 1.7, 0.0), Vec3::new(47.0, 13.0, 3.0 * jib_side)),
        "motion" => {
            // Slow entry-to-exit dolly, then a held view of the jib. No teleports
            // or motion before streaming warmup; timings are recorded by CLI.
            let t = (elapsed - 14.0).clamp(0.0, 24.0);
            (Vec3::new(35.0 + t, 1.7, 0.0), Vec3::new(47.0, 6.0, 5.5 * jib_side))
        }
        _ => (Vec3::new(35.0, 1.7, 0.0), Vec3::new(51.0, 6.0, 2.0 * jib_side)),
    };
    let scale = cfg.voxel_meters();
    let offset = Vec3::Y * floor;
    // Player.pos is the feet position; derive the current eye offset rather
    // than treating the benchmark's legacy "clearance" as an eye coordinate.
    let eye_offset = player.eye(cfg.voxel_scale) - player.pos;
    player.pos = (eye + offset) / scale - eye_offset;
    player.precise_pos = player.pos.as_dvec3();
    let direction = (target - eye).normalize();
    player.yaw = direction.x.atan2(-direction.z);
    player.pitch = direction.y.asin();
    player.underwater = world::sample_block(cfg, player.eye(cfg.voxel_scale).floor().as_ivec3())
        == world::BlockId::Water;
    static RECEIPT: OnceLock<()> = OnceLock::new();
    RECEIPT.get_or_init(|| println!("[dungeon-review] mode={mode} generator={} voxel_m={scale} hall_floor_m={floor} eye_m={:?} camera_only=true", cfg.generator_version, eye + offset));
    true
}

/// `complex:<k>`: view k of the generated complex nearest (300, 0).
/// `undercrown:<room name>`: from a near corner of that Undercrown room
/// toward its middle. Both work for any generator version, so the same seed
/// and mode give a before/after pair across a generator bump.
fn view(mode: &str, cfg: &WorldSampleConfig) -> Option<(Vec3, Vec3)> {
    if mode == "cosmos" {
        return world::dungeon::cosmos_review_view(cfg, Vec3::new(300.0, 0.0, 0.0));
    }
    if let Some(k) = mode.strip_prefix("complex:") {
        let views = world::dungeon::review_views(cfg, Vec3::new(300.0, 0.0, 0.0));
        return views.get(k.parse::<usize>().ok()?).copied();
    }
    let name = mode.strip_prefix("undercrown:")?;
    let plan = world::undercrown::plan(cfg);
    let r = plan.rooms.iter().find(|r| r.name == name)?;
    let (cx, cz) = ((r.x0 + r.x1) * 0.5, (r.z0 + r.z1) * 0.5);
    let reach = Vec3::new(cx - r.x0, 0.0, cz - r.z0).clamp_length_max(9.0);
    let eye = Vec3::new(cx, r.floor + 1.7, cz) - reach;
    Some((eye, Vec3::new(cx, r.floor + (r.headroom * 0.3).min(4.0), cz) + reach * 0.5))
}

fn place(player: &mut Player, cfg: &WorldSampleConfig, mode: &str, eye: Vec3, target: Vec3) -> bool {
    let scale = cfg.voxel_meters();
    let eye_offset = player.eye(cfg.voxel_scale) - player.pos;
    player.pos = eye / scale - eye_offset;
    player.precise_pos = player.pos.as_dvec3();
    let direction = (target - eye).normalize();
    player.yaw = direction.x.atan2(-direction.z);
    player.pitch = direction.y.asin();
    player.underwater = false;
    static RECEIPT: OnceLock<()> = OnceLock::new();
    RECEIPT.get_or_init(|| println!("[dungeon-review] mode={mode} generator={} voxel_m={scale} eye_m={eye:?} target_m={target:?} camera_only=true", cfg.generator_version));
    true
}
