//! The host's check of joiner-reported bodies.
use super::*;

const TICK: f32 = 1.0 / 30.0;

/// A stone floor high above the superflat ground with a one-voxel wall at x = 10.
fn room() -> (World, Player) {
    let mut world = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 7);
    let mut edits = Vec::new();
    for x in 0..24 {
        for z in 0..12 {
            edits.push((IVec3::new(x, 199, z), BlockId::Stone));
        }
    }
    for y in 200..204 {
        for z in 0..12 {
            edits.push((IVec3::new(10, y, z), BlockId::Stone));
        }
    }
    world.set_many(&edits);
    let mut body = Player::new();
    body.pos = Vec3::new(8.5, 200.0, 5.5);
    (world, body)
}

#[test]
fn walking_and_sprinting_steps_are_accepted() {
    let (world, body) = room();
    for metres in [0.15, 0.3, 0.8] {
        assert!(owner_move(&world, &body, body.pos - Vec3::X * metres, TICK, false), "{metres} m step");
    }
    // A hop within the climb cap.
    assert!(owner_move(&world, &body, body.pos + Vec3::Y * 1.2, TICK, false));
}

#[test]
fn speed_climb_and_walls_are_refused() {
    let (world, body) = room();
    assert!(!owner_move(&world, &body, body.pos - Vec3::X * 30.0, TICK, false), "teleport across the room");
    assert!(!owner_move(&world, &body, body.pos + Vec3::Y * 12.0, TICK, false), "flight without creative");
    assert!(!owner_move(&world, &body, Vec3::new(10.5, 200.0, 5.5), TICK, false), "standing inside the wall");
    assert!(!owner_move(&world, &body, Vec3::new(11.8, 200.0, 5.5), TICK, false), "passing through the wall");
    assert!(!owner_move(&world, &body, body.pos - Vec3::Y * 2.0, TICK, false), "sinking into the floor");
}

#[test]
fn creative_skips_the_caps_but_not_the_walls() {
    let (world, body) = room();
    assert!(owner_move(&world, &body, body.pos + Vec3::Y * 12.0, TICK, true));
    assert!(owner_move(&world, &body, Vec3::new(2.5, 230.0, 5.5), TICK, true));
    assert!(!owner_move(&world, &body, Vec3::new(14.5, 200.0, 5.5), TICK, true));
}
