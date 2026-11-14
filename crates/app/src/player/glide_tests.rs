//! Glide frame through the real player controller on a flat world.
use crate::player::{Input, Player, update_player};
use crate::world::{World, WorldType};
use engine_core::VoxelScalePreset;
use glam::{IVec3, Vec3};

const STEP: f32 = crate::simulation::STEP_SECONDS;

fn surface(world: &World) -> i32 {
    let reach = (400.0 / world.voxel_scale.meters()) as i32;
    (-reach..reach).rev().find(|&y| world.get(IVec3::new(0, y, 0)).is_solid()).map_or(0, |y| y + 1)
}

struct Flight {
    distance_m: f32,
    harm: f32,
    seconds: f32,
    landed: bool,
}

/// Drops from `metres`, taps Jump once after `open_after` seconds of falling
/// (never if `None`) and runs until landing or 60 s.
fn drop_from(world: &World, metres: f32, frame: bool, open_after: Option<f32>, pitch: f32) -> Flight {
    let m = world.voxel_scale.meters();
    let mut player = Player::new();
    player.pos = Vec3::new(0.5, surface(world) as f32 + metres / m, 0.5);
    player.precise_pos = player.pos.as_dvec3();
    player.glide.available = frame;
    player.pitch = pitch;
    let start = player.pos;
    let mut input = Input::default();
    let mut harm = 0.0;
    let mut t = 0.0;
    while t < 60.0 {
        input.jump = open_after.is_some_and(|after| (after..after + 0.05).contains(&t));
        update_player(&mut player, &mut input, world, STEP);
        if let Some((amount, _)) = player.take_pending_harm() {
            harm += amount;
        }
        t += STEP;
        if player.on_ground {
            break;
        }
    }
    let d = (player.pos - start) * m;
    Flight { distance_m: Vec3::new(d.x, 0.0, d.z).length(), harm, seconds: t, landed: player.on_ground }
}

#[test]
fn an_open_frame_turns_a_lethal_drop_into_a_long_safe_glide() {
    for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::HalfMeter] {
        let world = World::new(WorldType::Superflat, scale);
        let fall = drop_from(&world, 60.0, false, Some(0.4), 0.0);
        let glide = drop_from(&world, 60.0, true, Some(0.4), 0.0);
        assert!(fall.landed && glide.landed, "{scale:?}: never landed");
        assert!(fall.harm > 0.0, "{scale:?}: a 60 m fall must hurt");
        assert_eq!(glide.harm, 0.0, "{scale:?}: trimmed landing hurt for {}", glide.harm);
        assert!(fall.distance_m < 1.0, "{scale:?}: falling drifted {} m", fall.distance_m);
        // Height spent opening the frame and gathering speed costs some range.
        assert!(glide.distance_m > 150.0, "{scale:?}: glided only {} m", glide.distance_m);
        assert!(glide.seconds > fall.seconds * 4.0, "{scale:?}: {} s vs {} s", glide.seconds, fall.seconds);
    }
}

#[test]
fn without_the_frame_jump_does_nothing_in_the_air() {
    let world = World::new(WorldType::Superflat, VoxelScalePreset::OneMeter);
    let a = drop_from(&world, 30.0, false, Some(0.3), 0.0);
    let b = drop_from(&world, 30.0, false, None, 0.0);
    assert_eq!((a.distance_m, a.harm), (b.distance_m, b.harm));
}

#[test]
fn pumping_pitch_never_climbs_above_the_launch_height() {
    // Dive for speed, then flare: still air can trade speed for height but never add energy.
    let world = World::new(WorldType::Superflat, VoxelScalePreset::OneMeter);
    let mut player = Player::new();
    player.pos = Vec3::new(0.5, surface(&world) as f32 + 80.0, 0.5);
    player.precise_pos = player.pos.as_dvec3();
    player.glide.available = true;
    let launch = player.pos.y;
    let mut input = Input::default();
    let mut peak = f32::MIN;
    let mut opened = false;
    for i in 0..120 * 40 {
        let t = i as f32 * STEP;
        input.jump = (0.3..0.35).contains(&t);
        player.pitch = if (t % 6.0) < 3.0 { -0.9 } else { 0.6 };
        update_player(&mut player, &mut input, &world, STEP);
        opened |= player.glide.deployed;
        if opened {
            peak = peak.max(player.pos.y);
        }
        if player.on_ground {
            break;
        }
    }
    assert!(opened, "the frame never opened");
    assert!(peak < launch, "climbed to {peak} from {launch}");
}
