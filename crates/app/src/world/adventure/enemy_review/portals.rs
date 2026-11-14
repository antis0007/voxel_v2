//! Opt-in crystal gate pair for portal captures (`VOXEL_ENEMY_REVIEW_SCENE=portals`).
//! Gate A faces +Z in a plain yard; gate B, turned 90 degrees, faces +X in a
//! tiled garden 30 m away. Linked through the same find/attune path as play.
use super::{BlockId, IVec3, Player, Vec3, World};
use crate::world::portals::gates;
use std::sync::OnceLock;

pub(super) fn install(world: &mut World, player: &mut Player) {
    let verify = std::env::var_os("VOXEL_PORTAL_VERIFY").is_some();
    if verify && world.world_type == crate::world::WorldType::Manyfold {
        verify_maze(world, player);
        return;
    }
    let v = world.voxel_scale.meters();
    let n = v.recip().round() as i32;
    let mut edits = Vec::new();
    let mut put = |x: i32, y: i32, z: i32, b: BlockId| {
        let p = IVec3::new(x * n, y * n, z * n);
        for dx in 0..n { for dy in 0..n { for dz in 0..n {
            edits.push((p + IVec3::new(dx, dy, dz), b));
        }}}
    };
    for x in -12..=44 { for z in -14..=22 { for y in 79..=88 {
        let garden = (31..=42).contains(&x) && (2..=18).contains(&z);
        let b = match y {
            79 if garden => BlockId::GlazedTiles,
            79 => BlockId::Limestone,
            80..=84 if garden && (x == 36 || x == 40) && (z == 5 || z == 14) => BlockId::StoneBricks,
            85 if garden && (36..=40).contains(&x) && (z == 5 || z == 14) => BlockId::StoneBricks,
            80..=82 if garden && x == 38 && z == 10 => BlockId::Log,
            83..=85 if garden && (37..=39).contains(&x) && (9..=11).contains(&z) => BlockId::Leaves,
            _ => BlockId::Air,
        };
        put(x, y, z, b);
    }}}
    // Ring cells in metres: openings are 2 m wide and 3 m tall.
    for y in 79..=83 { for w in -2..=1 {
        let edge = y == 79 || y == 83 || w == -2 || w == 1;
        if edge {
            put(w, y, 0, gates::FRAME_BLOCK);
            put(30, y, w + 10, gates::FRAME_BLOCK);
        }
    }}
    put(-1, 80, 2, BlockId::Bookcase);
    world.set_many(&edits);
    world.adventure.enemies.clear();
    let scale = world.voxel_scale.meters();
    let a = gates::find(world, IVec3::new(-2 * n, 81 * n, 0), Vec3::NEG_Z);
    let b = gates::find(world, IVec3::new(30 * n, 81 * n, 8 * n), Vec3::NEG_X);
    let mut linked = false;
    if let (Some(a), Some(b)) = (a, b) {
        world.portals.attune(a, true, scale);
        linked = world.portals.attune(b, true, scale) == gates::Outcome::Linked;
    }
    if verify && linked {
        verify_crystal(world, player);
    }
    camera(player, v);
    println!(
        "[portal-review] gates={} linked={linked} portals={} voxel_m={v}",
        u8::from(a.is_some()) + u8::from(b.is_some()),
        world.portals.network().portals.len()
    );
}

fn verify_crystal(world: &mut World, player: &mut Player) {
    use crate::world::portals::traverse;
    let scale = world.voxel_scale.meters();
    // The garden exit is clear. The reverse path has a bookcase in its yard.
    let portal = world.portals.network().portals.iter()
        .find(|portal| portal.entrance.normal().z > 0.9)
        .expect("crystal review needs the yard entrance").clone();
    player.gravity_frame = glam::Quat::IDENTITY;
    let eye_offset = (player.precise_eye(world.voxel_scale) - player.pos.as_dvec3()) * f64::from(scale);
    let entry_eye = portal.entrance.center + glam::DVec3::Y * 0.3;
    let start = entry_eye + portal.entrance.normal() * 0.6;
    let end = entry_eye - portal.entrance.normal() * 1.8;
    let exit_eye = portal.exit.center + portal.exit.normal() * 1.1;
    let obstacle = (exit_eye / f64::from(scale)).floor().as_ivec3();
    let old = world.get(obstacle);
    let place = |p: &mut Player, eye| {
        p.precise_pos = (eye - eye_offset) / f64::from(scale);
        p.pos = p.precise_pos.as_vec3();
    };
    world.set_many(&[(obstacle, BlockId::StoneBricks)]);
    place(player, start);
    let source = player.pos;
    let before = traverse::before(world, player);
    place(player, end);
    let stopped = !traverse::step(world, player, before) && player.pos == source;
    world.set_many(&[(obstacle, old)]);
    let before = traverse::before(world, player);
    place(player, end);
    let clear = traverse::step(world, player, before);
    println!("[portal-verify] crystal voxel_m={scale} blocked={stopped} clear={clear}");
    assert!(stopped && clear, "Crystal portal regression");
}

fn verify_maze(world: &World, player: &mut Player) {
    use engine_worldgen::manyfold::{maze, shell_curve};
    let scale = world.voxel_scale.meters();
    let version = world.generator_version;
    let (source, target) = maze::mouths(version).expect("maze review needs a portal");
    let curve = shell_curve(version).expect("maze review needs the shell");
    let feet = |offset| curve.floor_point(source.center + source.into_maze * offset, 0.08) / scale;
    let before = feet(-0.65);
    player.gravity_frame = glam::Quat::from_rotation_arc(Vec3::Y, source.up);
    player.flying = false;
    player.pos = feet(0.65);
    player.precise_pos = player.pos.as_dvec3();
    crate::manyfold::maze_portal::step(world, player, before);
    let landed = player.pos * scale;
    let crossed = player.manyfold_crossing != 0 && landed.distance(target.center) < 3.0;
    println!("[portal-verify] manyfold voxel_m={scale} generator={version} crossed={crossed} landing_m={landed:?}");
    assert!(crossed, "Manyfold portal regression");
}

pub(super) fn camera(player: &mut Player, v: f32) -> bool {
    static VIEW: OnceLock<String> = OnceLock::new();
    let view = VIEW.get_or_init(|| std::env::var("VOXEL_PORTAL_VIEW").unwrap_or_default());
    let (eye, target) = match view.as_str() {
        "grazing" => (Vec3::new(4.2, 81.6, 2.2), Vec3::new(0.0, 81.4, 0.0)),
        "close" => (Vec3::new(0.2, 81.6, 1.15), Vec3::new(0.0, 81.4, -2.0)),
        // A step from crossing, still outside the ring's depth.
        "threshold" => (Vec3::new(0.2, 81.6, 0.75), Vec3::new(0.0, 81.4, -2.0)),
        "back" => (Vec3::new(0.0, 81.6, -6.0), Vec3::new(0.0, 81.4, 0.5)),
        "far" => (Vec3::new(-3.0, 84.5, 15.0), Vec3::new(0.0, 81.4, 0.0)),
        "garden" => (Vec3::new(44.0, 83.0, 10.0), Vec3::new(30.0, 81.4, 10.0)),
        _ => (Vec3::new(0.0, 81.6, 6.0), Vec3::new(0.0, 81.4, 0.0)),
    };
    player.pos = (eye - Vec3::Y * 1.62) / v;
    player.precise_pos = player.pos.as_dvec3();
    let d = (target - eye).normalize();
    player.yaw = d.x.atan2(-d.z);
    player.pitch = d.y.asin();
    true
}
