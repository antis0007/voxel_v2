use super::*;
use crate::player::{Input, Player, update_player};
use crate::world::{World, WorldType};

const STEP: f32 = 1.0 / 120.0;
const START: u64 = 40_000;

fn world() -> World {
    let mut world = World::new(WorldType::Planet, engine_core::VoxelScalePreset::OneMeter);
    world.orbital.planet_tick = START;
    world
}

fn player_at(world: &World, height: f64, flying: bool) -> Player {
    let mut player = Player::new();
    player.flying = flying;
    player.precise_pos = moon_ground_point(world.orbital.planet_tick, world.seed, height).unwrap();
    player.pos = player.precise_pos.as_vec3();
    player
}

/// Steps with the clock running, as the fixed loop does.
fn run(world: &mut World, player: &mut Player, input: &mut Input, seconds: f32) {
    for _ in 0..(seconds * 120.0).round() as u32 {
        world.orbital.planet_tick += 1;
        update_player(player, input, world, STEP);
    }
}

fn height(world: &World, player: &Player) -> f64 {
    let anchor = player.anchor.expect("anchored");
    anchor.pos.length() - ground_radius(world.seed, anchor.body, anchor.pos.normalize())
}

#[test]
fn foreign_body_is_the_moon_inside_its_sphere_only() {
    let moon = system().unwrap().id("moon").unwrap();
    assert_eq!(foreign_body(moon_point(START, 6_000.0).unwrap(), START), Some(moon));
    assert_eq!(foreign_body(moon_point(START, 30_000.0).unwrap(), START), None);
    assert_eq!(foreign_body(DVec3::new(0.0, 60_000.0, 0.0), START), None);
}

#[test]
fn frame_round_trips_atlas_points() {
    let moon = system().unwrap().id("moon").unwrap();
    let frame = Frame::at(moon, START).unwrap();
    let local = DVec3::new(1_200.0, 3_900.0, -700.0);
    assert!((frame.to_body(frame.to_atlas(local)) - local).length() < 1e-6);
}

#[test]
fn entering_the_sphere_keeps_the_view() {
    let mut world = world();
    let mut player = player_at(&world, 2_000.0, true);
    player.yaw = 0.7;
    player.pitch = -0.2;
    let look = player.forward_dir();
    run(&mut world, &mut player, &mut Input::default(), STEP);
    assert!(player.anchor.is_some());
    assert!(player.forward_dir().dot(look) > 0.999, "view jumped");
}

#[test]
fn an_anchored_player_rides_the_moon_while_the_clock_runs() {
    let mut world = world();
    let mut player = player_at(&world, 0.5, false);
    let mut input = Input::default();
    run(&mut world, &mut player, &mut input, 1.0);
    let start = player.anchor.unwrap().pos;
    run(&mut world, &mut player, &mut input, 10.0);
    assert!(player.on_ground);
    assert!((player.anchor.unwrap().pos - start).length() < 0.05, "slid on the surface");
    assert!(height(&world, &player).abs() < 0.01);
}

#[test]
fn a_fall_lands_and_stays_on_the_ground() {
    let mut world = world();
    let mut player = player_at(&world, 50.0, false);
    run(&mut world, &mut player, &mut Input::default(), 12.0);
    assert!(player.on_ground && height(&world, &player).abs() < 0.01);
}

#[test]
fn walking_follows_the_cratered_surface() {
    let mut world = world();
    let mut player = player_at(&world, 0.0, false);
    let mut input = Input::default();
    run(&mut world, &mut player, &mut input, 1.0);
    let start = player.anchor.unwrap().pos;
    input.forward = true;
    let mut lowest: f64 = 0.0;
    for _ in 0..20 {
        run(&mut world, &mut player, &mut input, 1.0);
        lowest = lowest.min(height(&world, &player));
    }
    let travelled = (player.anchor.unwrap().pos - start).length();
    assert!(travelled > 70.0, "travelled {travelled}");
    assert!(lowest > -0.01, "sank {lowest} m below ground");
}

#[test]
fn a_jump_rises_and_comes_back() {
    let mut world = world();
    let mut player = player_at(&world, 0.0, false);
    let mut input = Input::default();
    run(&mut world, &mut player, &mut input, 1.0);
    input.jump = true;
    run(&mut world, &mut player, &mut input, 0.05);
    input.jump = false;
    run(&mut world, &mut player, &mut input, 0.5);
    assert!(height(&world, &player) > 0.5);
    run(&mut world, &mut player, &mut input, 6.0);
    assert!(player.on_ground);
}

/// Metres a flying player sinks over 5 s of hovering 100 m above the moon.
fn hover_drop(mode: crate::world::rules::GameMode) -> f64 {
    let mut world = world();
    world.rules.mode = mode;
    let mut player = player_at(&world, 100.0, true);
    run(&mut world, &mut player, &mut Input::default(), 0.1);
    let before = height(&world, &player);
    run(&mut world, &mut player, &mut Input::default(), 5.0);
    before - height(&world, &player)
}

#[test]
fn hovering_sags_slightly_toward_the_moon() {
    let drop = hover_drop(crate::world::rules::GameMode::Survival);
    assert!(drop > 0.05 && drop < 2.0, "drop {drop} m");
}

#[test]
fn creative_hovering_holds_its_height() {
    let drop = hover_drop(crate::world::rules::GameMode::Creative);
    assert!(drop.abs() < 0.05, "drop {drop} m");
}

fn moon() -> BodyId { system().unwrap().id("moon").unwrap() }

fn creative() -> World {
    let mut world = world();
    world.rules.mode = crate::world::rules::GameMode::Creative;
    // The fixed loop copies this from the mode every step.
    world.adventure.creative = true;
    world
}

/// Position, line of sight and camera up in the moon's axes at the current tick.
fn seen_from_moon(world: &World, player: &Player) -> (DVec3, DVec3, DVec3) {
    let frame = Frame::at(moon(), world.orbital.planet_tick).unwrap();
    let point = player.precise_pos * f64::from(world.voxel_scale.meters());
    let to_body = frame.axes(point).inverse();
    (frame.to_body(point), to_body * player.forward_dir().as_dvec3(), to_body * player.up_dir().as_dvec3())
}

/// Turns the view toward a moon-axes direction.
fn look_along(world: &World, player: &mut Player, body_dir: DVec3) {
    let frame = Frame::at(moon(), world.orbital.planet_tick).unwrap();
    let point = player.precise_pos * f64::from(world.voxel_scale.meters());
    let local = player.gravity_frame.as_dquat().inverse() * (frame.axes(point) * body_dir.normalize());
    player.yaw = local.x.atan2(-local.z) as f32;
    player.pitch = (local.y.clamp(-1.0, 1.0).asin() as f32).clamp(-1.54, 1.54);
}

#[test]
fn approaching_the_moon_keeps_its_motion_continuous() {
    let mut world = creative();
    let soi = system().unwrap().soi(moon()).unwrap();
    let mut player = Player::new();
    player.flying = true;
    player.flight.camera_direction = true;
    player.precise_pos = moon_point(START, soi * 3.0).unwrap();
    player.pos = player.precise_pos.as_vec3();
    let mut input = Input::default();
    input.forward = true;
    let mut previous: Option<(DVec3, DVec3)> = None;
    let mut worst: f64 = 0.0;
    for step in 0..120 * 40 {
        let (pos, _, _) = seen_from_moon(&world, &player);
        look_along(&world, &mut player, -pos);
        run(&mut world, &mut player, &mut input, STEP);
        let (next, _, _) = seen_from_moon(&world, &player);
        let velocity = (next - pos) * 120.0;
        // The first second spins the cruise up from rest.
        if let Some((_, last)) = previous && step > 120 {
            worst = worst.max((velocity - last).length());
        }
        previous = Some((next, velocity));
        if player.anchor.is_some() && next.length() < soi * 0.9 { break; }
    }
    assert!(player.anchor.is_some_and(|a| a.pos.length() < soi), "never reached the sphere");
    assert!(worst < 60.0, "velocity relative to the moon jumped {worst:.1} m/s in one step");
}

/// Atlas point `height` metres above the moon's ground on its side facing home.
fn near_side(height: f64) -> DVec3 {
    let frame = Frame::at(moon(), START).unwrap();
    let toward = (frame.body_rot.inverse() * (frame.home_pos - frame.body_pos)).normalize();
    frame.to_atlas(toward * (ground_radius(world().seed, moon(), toward) + height))
}

#[test]
fn leaving_toward_home_turns_the_horizon_about_a_fixed_line_of_sight() {
    let mut world = creative();
    let mut player = Player::new();
    player.flying = true;
    player.flight.camera_direction = true;
    player.precise_pos = near_side(50.0);
    player.pos = player.precise_pos.as_vec3();
    let mut input = Input::default();
    // A teleport arrives with home's up; half a turn at the tilt rate settles it.
    run(&mut world, &mut player, &mut input, 4.0);
    assert!(player.anchor.is_some());
    let (pos, _, _) = seen_from_moon(&world, &player);
    look_along(&world, &mut player, pos);
    input.forward = true;
    input.sprint = true;
    let (_, mut look, mut up) = seen_from_moon(&world, &player);
    let start_up = up;
    let (mut worst_look, mut worst_up): (f64, f64) = (0.0, 0.0);
    for _ in 0..120 * 60 {
        run(&mut world, &mut player, &mut input, STEP);
        let (_, next_look, next_up) = seen_from_moon(&world, &player);
        worst_look = worst_look.max(next_look.angle_between(look));
        worst_up = worst_up.max(next_up.angle_between(up));
        (look, up) = (next_look, next_up);
        if player.anchor.is_none() { break; }
    }
    assert!(player.anchor.is_none(), "still anchored");
    assert_eq!(player.gravity_frame, Quat::IDENTITY);
    assert!(up.angle_between(start_up) > 2.5, "the upright never turned to home");
    assert!(worst_up < TILT_RATE * f64::from(STEP) * 1.1, "camera up jumped {worst_up:.3} rad in one step");
    // Home and the moon spin at different rates, so only steps are bounded, not the total.
    assert!(worst_look < 0.003, "line of sight jumped {worst_look:.4} rad in one step");
}

#[test]
fn cruise_above_the_terrain_is_as_fast_sideways_as_upward() {
    let mut world = creative();
    // Alt holds the precise speed, which does not grow with height while climbing.
    let mut physical = |input: Input| {
        let mut player = Player::new();
        player.flying = true;
        player.flight.camera_direction = true;
        player.precise_pos = DVec3::new(3_000.0, 10_000.0, -2_000.0);
        player.pos = player.precise_pos.as_vec3();
        let mut input = input;
        run(&mut world, &mut player, &mut input, 2.0);
        let before = atlas::to_body(player.precise_pos);
        run(&mut world, &mut player, &mut input, 1.0);
        assert!(player.anchor.is_none());
        (atlas::to_body(player.precise_pos) - before).length()
    };
    let precise = |forward: bool| {
        let mut input = Input::default();
        input.alt_down = true;
        input.forward = forward;
        input.jump = !forward;
        input
    };
    let sideways = physical(precise(true));
    let upward = physical(precise(false));
    assert!((sideways / upward - 1.0).abs() < 0.03, "sideways {sideways:.0} m/s, upward {upward:.0} m/s");
}

#[test]
fn climbing_out_of_the_sphere_returns_to_home_space() {
    let mut world = world();
    let mut player = player_at(&world, 10.0, true);
    let mut input = Input::default();
    input.jump = true;
    input.sprint = true;
    run(&mut world, &mut player, &mut input, 0.1);
    assert!(player.anchor.is_some());
    for _ in 0..120 {
        run(&mut world, &mut player, &mut input, 1.0);
        if player.anchor.is_none() {
            break;
        }
    }
    assert!(player.anchor.is_none(), "still anchored");
    assert_eq!(player.gravity_frame, Quat::IDENTITY);
    assert!(player.precise_pos.is_finite());
}
