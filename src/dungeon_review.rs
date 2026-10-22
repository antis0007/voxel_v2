//! Opt-in cameras for the actual generated deep vault; never edits the world.
//! Camera routes inspect rendering only, not collision or player traversal.
use crate::{player::Player, world::{self, WorldSampleConfig}};
use glam::Vec3;
use std::sync::OnceLock;

pub fn camera(player: &mut Player, cfg: &WorldSampleConfig, elapsed: f32) -> bool {
    static MODE: OnceLock<String> = OnceLock::new();
    let mode = MODE.get_or_init(|| std::env::var("VOXEL_REVIEW_DUNGEON").unwrap_or_default());
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
            let t = (elapsed - 14.0).max(0.0).min(24.0);
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
