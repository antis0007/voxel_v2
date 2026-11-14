use super::*;
use crate::player::{DROWN_DAMAGE_PER_S, Input, update_player};
use crate::world::WorldType;
use engine_core::VoxelScalePreset;

const STEP: f32 = crate::simulation::STEP_SECONDS;
const SCALES: [VoxelScalePreset; 3] = [
    VoxelScalePreset::OneMeter,
    VoxelScalePreset::HalfMeter,
    VoxelScalePreset::QuarterMeter,
];

fn world(scale: VoxelScalePreset) -> World {
    World::new(WorldType::Superflat, scale)
}

/// First open cell above the ground at the origin column.
fn surface(world: &World) -> i32 {
    let reach = (400.0 / world.voxel_scale.meters()) as i32;
    (-reach..reach)
        .rev()
        .find(|&y| world.get(IVec3::new(0, y, 0)).is_solid())
        .map_or(0, |y| y + 1)
}

/// Drops a body `metres` onto the origin column; returns the summed harm and
/// the body once it stands on the ground or has floated for two seconds.
fn drop_onto(world: &World, metres: f32) -> (Option<(f32, Cause)>, Player) {
    let scale = world.voxel_scale.meters();
    let mut player = Player::new();
    player.pos = Vec3::new(0.5, surface(world) as f32 + metres / scale, 0.5);
    player.precise_pos = player.pos.as_dvec3();
    let mut input = Input::default();
    let mut total: Option<(f32, Cause)> = None;
    let mut floated = 0;
    for _ in 0..2400 {
        update_player(&mut player, &mut input, world, STEP);
        if let Some((amount, cause)) = player.take_pending_harm() {
            total = Some((total.map_or(0.0, |(sum, _)| sum) + amount, cause));
        }
        floated += usize::from(player.swimming);
        if player.on_ground || floated >= 240 {
            break;
        }
    }
    (total, player)
}

/// A dry drop that must end standing on the ground.
fn fall(world: &World, metres: f32) -> Option<(f32, Cause)> {
    let (harm, player) = drop_onto(world, metres);
    assert!(player.on_ground && !player.swimming, "body never landed at {:?}", player.pos);
    harm
}

#[test]
fn fall_damage_starts_past_the_safe_height() {
    let speed = |metres: f32| (2.0 * GRAVITY * metres).sqrt();
    assert_eq!(fall_damage(speed(SAFE_FALL_METRES - 0.1)), 0.0);
    let ten = fall_damage(speed(10.0));
    assert!((ten - (10.0 - SAFE_FALL_METRES) * FALL_DAMAGE_PER_METRE).abs() < 1e-3, "{ten}");
}

#[test]
fn fall_damage_is_per_metre_at_every_voxel_scale() {
    let mut taken = Vec::new();
    for scale in SCALES {
        let world = world(scale);
        assert_eq!(fall(&world, 2.0), None, "{scale:?}: a 2 m drop is safe");
        let (amount, cause) = fall(&world, 10.0).expect("a 10 m drop hurts");
        assert_eq!(cause, Cause::Fall);
        let expected = (10.0 - SAFE_FALL_METRES) * FALL_DAMAGE_PER_METRE;
        assert!((amount - expected).abs() < 3.0, "{scale:?}: {amount} vs {expected}");
        taken.push(amount);
    }
    let spread = taken.iter().fold(0.0f32, |m, a| m.max((a - taken[0]).abs()));
    assert!(spread < 2.0, "fall damage differs across voxel scales: {taken:?}");
}

#[test]
fn landing_in_water_cancels_the_fall() {
    // 0.5 m stands the body on the floor with wet feet; 2.5 m floats it.
    for (depth_m, floats) in [(0.5, false), (2.5, true)] {
        for scale in SCALES {
            let mut world = world(scale);
            let floor = surface(&world);
            let depth = (depth_m / scale.meters()).round() as i32;
            if !floats && depth as f32 * scale.meters() > depth_m {
                // A whole 1 m cell of water already floats the body.
                continue;
            }
            let half = (1.0 / scale.meters()).ceil() as i32;
            let mut pool = Vec::new();
            for y in floor..floor + depth {
                for z in -half..=half {
                    for x in -half..=half {
                        pool.push((IVec3::new(x, y, z), BlockId::Water));
                    }
                }
            }
            world.set_many(&pool);
            let (harm, player) = drop_onto(&world, 14.0);
            assert_eq!(player.swimming, floats, "{scale:?} {depth_m} m: body at {:?}", player.pos);
            assert!(floats || player.on_ground, "{scale:?} {depth_m} m: body never landed");
            assert_eq!(harm, None, "{scale:?} {depth_m} m: water broke the fall");
        }
    }
}

#[test]
fn breath_runs_out_then_drowning_ticks_and_surfacing_refills() {
    let mut player = Player::new();
    player.underwater = true;
    let to_empty = (AIR_SECONDS / STEP).round() as usize;
    for _ in 0..to_empty - 2 {
        player.update_air(STEP);
    }
    assert_eq!(player.take_pending_harm(), None, "breath lasts {AIR_SECONDS} s");
    assert!(player.breath() > 0.0 && player.breath() < 0.01);
    for _ in 0..122 {
        player.update_air(STEP);
    }
    let (amount, cause) = player.take_pending_harm().expect("drowning");
    assert_eq!(cause, Cause::Drowned);
    assert!((amount - DROWN_DAMAGE_PER_S).abs() < 0.1, "one second of drowning: {amount}");
    assert_eq!(player.breath(), 0.0);
    player.underwater = false;
    for _ in 0..120 {
        player.update_air(STEP);
    }
    assert_eq!(player.breath(), 1.0, "one second of air refills the meter");
}

#[test]
fn flight_holds_breath() {
    let mut player = Player::new();
    player.underwater = true;
    player.flying = true;
    for _ in 0..(AIR_SECONDS / STEP) as usize * 2 {
        player.update_air(STEP);
    }
    assert_eq!(player.take_pending_harm(), None);
    assert_eq!(player.breath(), 1.0);
}

#[test]
fn lava_and_fire_burn_on_contact() {
    assert!(BlockId::Lava.contact_damage() > BlockId::Fire.contact_damage());
    assert!(BlockId::Fire.contact_damage() > 0.0);
    assert_eq!(BlockId::Water.contact_damage(), 0.0);
    assert_eq!(BlockId::Air.contact_damage(), 0.0);
    for (block, label) in [(BlockId::Lava, "Lava"), (BlockId::Fire, "Fire")] {
        let mut world = world(VoxelScalePreset::OneMeter);
        let floor = surface(&world);
        world.set(IVec3::new(0, floor, 0), block);
        let mut player = Player::new();
        player.pos = Vec3::new(0.5, floor as f32, 0.5);
        contact(&mut player, &world, IVec3::ZERO, STEP);
        let (amount, cause) = player.take_pending_harm().expect("contact damage");
        assert_eq!(cause, Cause::Contact(block));
        assert_eq!(cause.label(), label);
        assert!((amount - block.contact_damage() * STEP).abs() < 1e-6);
        player.flying = true;
        contact(&mut player, &world, IVec3::ZERO, STEP);
        assert_eq!(player.take_pending_harm(), None, "flight is exempt");
    }
}
