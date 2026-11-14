use super::*;
use crate::player::{Input, Player};
use crate::world::{World, WorldType};

const STEP: f32 = 1.0 / 120.0;
const START: u64 = 40_000;

fn world() -> World {
    let mut world = World::new(WorldType::Planet, engine_core::VoxelScalePreset::OneMeter);
    world.orbital.planet_tick = START;
    world
}

fn system() -> &'static System {
    super::super::influence::system().unwrap()
}

/// A flying player `altitude` metres above home's datum, straight up from the atlas origin.
fn above_home(altitude: f64) -> Player {
    let mut player = Player::new();
    player.flying = true;
    player.precise_pos = DVec3::new(0.0, altitude, 0.0);
    player.pos = player.precise_pos.as_vec3();
    player
}

/// Steps with the clock running; returns the seconds taken when `until` held, if it did.
fn run(world: &mut World, player: &mut Player, input: &Input, seconds: f32, until: impl Fn(&Player) -> bool) -> Option<f32> {
    for i in 0..(seconds * 120.0).round() as u32 {
        world.orbital.planet_tick += 1;
        if !step(player, input, world, STEP) {
            return Some(i as f32 * STEP);
        }
        if until(player) {
            return Some(i as f32 * STEP);
        }
    }
    None
}

#[test]
fn homes_reach_holds_the_moon_and_its_approach_band() {
    let system = system();
    let reach = home_reach(system).unwrap();
    let moon = system.id("moon").unwrap();
    let band = 2.0 * system.soi(moon).unwrap();
    let mut snapshot = Snapshot::default();
    system.evaluate(START, &mut snapshot);
    let home = system.id("home").unwrap();
    let orbit = (snapshot.position(moon) - snapshot.position(home)).length();
    assert!(orbit * 1.03 + band < reach * RETURN_FRACTION, "moon band to {} m, reach {reach} m", orbit + band);
}

#[test]
fn near_home_the_atlas_keeps_the_player() {
    let world = world();
    let mut player = above_home(40_000.0);
    assert!(!step(&mut player, &Input::default(), &world, STEP));
    assert!(player.void.is_none());
}

#[test]
fn entering_open_space_keeps_the_view() {
    let world = world();
    let mut player = above_home(home_reach(system()).unwrap() * 1.1);
    player.yaw = 0.6;
    player.pitch = 0.2;
    let look = player.forward_dir();
    assert!(step(&mut player, &Input::default(), &world, STEP));
    assert!(player.void.is_some());
    assert!(player.forward_dir().dot(look) > 0.999, "view jumped");
}

#[test]
fn a_hovering_player_holds_still_in_space_while_home_turns_under_them() {
    let mut world = world();
    let mut player = above_home(home_reach(system()).unwrap() * 1.2);
    let idle = Input::default();
    run(&mut world, &mut player, &idle, 3.0, |_| false);
    let (pos, atlas_m) = (player.void.unwrap().pos, player.precise_pos);
    assert!(run(&mut world, &mut player, &idle, 5.0, |_| false).is_none());
    let drift = (player.void.unwrap().pos - pos).length();
    assert!(drift < 5.0, "drifted {drift} m in space");
    assert!((player.precise_pos - atlas_m).length() > 1_000.0, "atlas pose should follow home's spin");
}

/// Turns the player's view to look at `target` from its current system position.
fn face(player: &mut Player, target: DVec3) {
    let mut v = player.void.unwrap();
    let dir = (target - v.pos).normalize();
    v.upright = DQuat::from_rotation_arc(DVec3::NEG_Z, dir);
    player.void = Some(v);
    player.yaw = 0.0;
    player.pitch = 0.0;
}

#[test]
fn flying_out_to_the_giant_takes_seconds_and_hands_over_at_its_approach_band() {
    let mut world = world();
    let mut player = above_home(home_reach(system()).unwrap() * 1.2);
    assert!(step(&mut player, &Input::default(), &world, STEP));
    let system = system();
    let giant = system.id("giant").unwrap();
    let mut forward = Input::default();
    forward.forward = true;
    let mut took = 0.0;
    for _ in 0..12 {
        let mut snapshot = Snapshot::default();
        system.evaluate(world.orbital.planet_tick, &mut snapshot);
        face(&mut player, snapshot.position(giant));
        match run(&mut world, &mut player, &forward, 10.0, |_| false) {
            Some(t) => {
                took += t;
                break;
            }
            None => took += 10.0,
        }
    }
    assert!(player.void.is_none(), "still in open space after {took} s");
    assert!(took < 60.0, "took {took} s");
    assert_eq!(super::super::influence::approached_body(player.precise_pos, world.orbital.planet_tick), Some(giant));
}

#[test]
fn flying_home_returns_to_the_atlas_upright() {
    let mut world = world();
    let mut player = above_home(home_reach(system()).unwrap() * 1.5);
    assert!(step(&mut player, &Input::default(), &world, STEP));
    let system = system();
    let home = system.id("home").unwrap();
    let mut forward = Input::default();
    forward.forward = true;
    let mut snapshot = Snapshot::default();
    system.evaluate(world.orbital.planet_tick, &mut snapshot);
    face(&mut player, snapshot.position(home));
    assert!(run(&mut world, &mut player, &forward, 30.0, |_| false).is_some(), "never returned");
    assert!(player.void.is_none());
    assert_eq!(player.gravity_frame, Quat::IDENTITY);
    assert!(atlas::to_body(player.precise_pos).length() < home_reach(system).unwrap());
}
