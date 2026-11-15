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
    if mode.contains(':') { return false; }
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
    // `gilded:<view>`: authored Gilded Cave set pieces, in realm metres.
    if let Some(name) = mode.strip_prefix("gilded:") {
        return engine_worldgen::gilded::review_view(cfg.seed, name);
    }
    if let Some(name) = mode.strip_prefix("anchor:") {
        use engine_worldgen::ruins::{Kind, near};
        static ANCHOR: OnceLock<Option<(Vec3, Vec3)>> = OnceLock::new();
        return *ANCHOR.get_or_init(|| {
            let kind = match name {
                "port" | "freight" | "wreck" | "terminal" | "studio-port" => Kind::Port,
                "office" | "street" | "concourse" | "descent" | "return" | "studio-office" => Kind::Offices,
                _ => return None,
            };
            let district = [1450.0, 2500.0, 3900.0].into_iter().find_map(|radius| {
                near(cfg, Vec3::ZERO, radius).into_iter()
                    .filter(|d| d.kind == kind && (!name.starts_with("studio-") || d.review_view("studio", cfg.generator_version).is_some()))
                    .min_by(|a, b| a.centre.length_squared().total_cmp(&b.centre.length_squared()))
            })?;
            if name.starts_with("studio-") {
                return district.review_view("studio", cfg.generator_version);
            }
            if matches!(name, "concourse" | "descent" | "return" | "wreck" | "terminal") {
                return district.review_view(name, cfg.generator_version);
            }
            let (eye, target) = match name {
                "street" => (Vec3::new(-2.0, 2.3, 54.0), Vec3::new(6.0, 11.0, 11.0)),
                "freight" => (Vec3::new(0.0, 9.0, 57.0), Vec3::new(35.0, 4.0, 36.0)),
                _ => (Vec3::new(110.0, 102.0, 137.0), Vec3::new(0.0, 18.0, 0.0)),
            };
            Some((district.world(eye), district.world(target)))
        });
    }
    if let Some(name)=mode.strip_prefix("vista:"){return engine_worldgen::vistas::review_view(cfg,name);}
    if cfg.world_type == world::WorldType::Manyfold {
        use engine_worldgen::manyfold::weave::point;
        return match mode {
            "manyfold:shell-entry" | "manyfold:shell-exit" => {
                let (entry, exit) = engine_worldgen::manyfold::maze::mouths(cfg.generator_version)?;
                let (mouth, sign) = if mode == "manyfold:shell-entry" { (entry, 1.0) } else { (exit, -1.0) };
                let eye = mouth.center - mouth.into_maze * (2.4 * sign) + Vec3::Z * 0.8;
                Some((eye, mouth.center + mouth.into_maze * (2.0 * sign)))
            }
            "manyfold:crypt" => Some((point(0, -2.3, -3.2, -15.9), point(0, 0.0, -2.7, -22.3))),
            "manyfold:waiting" => Some((point(3, 0.0, -3.2, -15.9), point(3, 0.0, -2.8, -22.8))),
            "manyfold:descent" => Some((point(0, -8.4, 1.8, -5.5), point(0, -8.4, 4.4, -10.2))),
            "manyfold:vista" => Some((point(0, 0.0, 1.8, -2.0), point(3, 0.0, 5.0, 12.0))),
            "manyfold:street" => Some((point(0, 1.0, 1.8, 0.0), point(0, 0.0, 4.2, 17.0))),
            "manyfold:seal" => Some((point(0, -1.0, 1.8, -4.5), point(0, 1.0, 0.7, -11.0))),
            _ => None,
        };
    }
    if mode == "observatory" {
        return world::undercrown::plan(cfg).v2.as_ref().map(|layout| layout.observatory_view());
    }
    if mode == "observatory-relic" {
        let gallery = world::undercrown::plan(cfg).v2.as_ref()?.flooded_observatory()?;
        let relic = gallery.reliquary();
        return Some((relic + Vec3::new(-3.0, 1.7, 0.0), relic + Vec3::Y * 0.85));
    }
    if let Some(name) = mode.strip_prefix("place:") {
        if name == "bridge" { return engine_worldgen::surface::bridge_review_view(cfg); }
        // Each benchmark process has one immutable world configuration.
        static PLACE: OnceLock<Option<(Vec3,Vec3)>> = OnceLock::new();
        return *PLACE.get_or_init(|| engine_worldgen::valley::place_review_view(cfg,name)
            .or_else(|| world::sites::place_review_view(cfg,name)));
    }
    if let Some(name) = mode.strip_prefix("room:") {
        static ROOM: OnceLock<Option<(Vec3,Vec3)>> = OnceLock::new();
        return *ROOM.get_or_init(|| world::dungeon::inhabited_review_view(cfg, name));
    }
    if mode == "cosmos" {
        static COSMOS: OnceLock<Option<(Vec3, Vec3)>> = OnceLock::new();
        return *COSMOS.get_or_init(|| world::dungeon::cosmos_review_view(cfg, Vec3::new(300.0, 0.0, 0.0)));
    }
    if let Some(k) = mode.strip_prefix("complex:") {
        let views = world::dungeon::review_views(cfg, Vec3::new(300.0, 0.0, 0.0));
        return views.get(k.parse::<usize>().ok()?).copied();
    }
    if let Some(label) = mode.strip_prefix("undercrown-prop:") {
        // A prop seen from 2.5 m at standing eye height, aimed at its model.
        let plan = world::undercrown::plan(cfg);
        let prop = plan.props.iter().find(|p| p.label == label)?;
        let (lo, hi) = plan.model_box(prop)?;
        let target = (lo + hi) * 0.5;
        return Some((Vec3::new(target.x - 1.8, lo.y + 1.7, target.z - 1.8), target));
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
    if mode.starts_with("manyfold:shell-") {
        let up = -engine_worldgen::manyfold::acceleration_for(cfg.generator_version, eye).normalize_or_zero();
        player.gravity_frame = glam::Quat::from_rotation_arc(Vec3::Y, up);
    } else if matches!(mode, "manyfold:waiting" | "manyfold:crypt" | "manyfold:descent") {
        let radial = eye - engine_worldgen::manyfold::CENTER;
        player.gravity_frame = glam::Quat::from_rotation_z(radial.x.atan2(-radial.y));
    }
    let eye_offset = player.eye(cfg.voxel_scale) - player.pos;
    player.pos = eye / scale - eye_offset;
    player.precise_pos = player.pos.as_dvec3();
    let direction = player.gravity_frame.conjugate() * (target - eye).normalize();
    player.yaw = direction.x.atan2(-direction.z);
    player.pitch = direction.y.asin();
    player.underwater = false;
    static PLAN_RECEIPT: OnceLock<()> = OnceLock::new();
    if mode.starts_with("gilded:") {
        PLAN_RECEIPT.get_or_init(|| println!("[gilded-plan] {}", engine_worldgen::gilded::plan_summary(cfg.seed)));
    }
    static RECEIPT: OnceLock<()> = OnceLock::new();
    RECEIPT.get_or_init(|| println!("[dungeon-review] mode={mode} generator={} voxel_m={scale} eye_m={eye:?} target_m={target:?} camera_only=true", cfg.generator_version));
    true
}

/// Live panel previews re-resolve the selected room against the active world.
pub(crate) fn workbench_view(player: &mut Player, cfg: &WorldSampleConfig, mode: &str) -> bool {
    let selected = if let Some(room) = mode.strip_prefix("room:") {
        world::dungeon::inhabited_review_view(cfg, room)
    } else if (mode.starts_with("manyfold:") && cfg.world_type == world::WorldType::Manyfold)
        || mode.starts_with("complex:")
    {
        view(mode, cfg)
    } else {
        None
    };
    let Some((eye, target)) = selected else { return false; };
    player.vel = Vec3::ZERO;
    player.on_ground = false;
    place(player, cfg, mode, eye, target)
}
