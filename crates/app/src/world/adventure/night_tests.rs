use super::*;
use crate::player::Player;
use crate::world::WorldType;
use engine_core::VoxelScalePreset;

const SEED: u32 = 2_738_958_700;
const MIDNIGHT: f32 = 0.0;
const NOON: f32 = 0.5;

fn flat_world(scale: VoxelScalePreset) -> (World, Vec3) {
    let w = World::with_seed(WorldType::Superflat, scale, SEED);
    let s = scale.meters();
    let feet = Vec3::new(0.5 * s, (w.terrain_height_at(0, 0) + 1) as f32 * s, 0.5 * s);
    (w, feet)
}

fn set_time(w: &mut World, time_of_day: f32) {
    let cfg = w.sample_config();
    w.sandbox.climate.sync(cfg, time_of_day, 0.0);
}

fn run(a: &mut Adventure, w: &World, feet: Vec3, passes: u32) {
    for _ in 0..passes {
        tick(a, w, feet, Budget::UNBOUNDED);
    }
}

fn bodies(a: &Adventure) -> Vec<&Enemy> {
    a.enemies.iter().filter(|e| is_night_body(e)).collect()
}

#[test]
fn night_spawns_fill_the_ring_up_to_the_cap() {
    for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::QuarterMeter] {
        for level in [Difficulty::Normal, Difficulty::Hard] {
            let (mut w, feet) = flat_world(scale);
            set_time(&mut w, MIDNIGHT);
            let mut a = Adventure::default();
            a.set_difficulty(level);
            run(&mut a, &w, feet, 120);
            let found = bodies(&a);
            assert_eq!(found.len(), cap(level), "{scale:?} {level:?}");
            for (i, e) in found.iter().enumerate() {
                let r = flat(e.pos - feet).length();
                assert!((RING_MIN_M - 1.0..=RING_MAX_M + 1.0).contains(&r), "{scale:?}: ring {r}");
                assert!(habitat::listed(e.kind), "kind {} has no spawn row", e.kind);
                assert!(free(&w, e.pos, e.kind));
                // Slots are pass numbers, so earlier slots were the crowd at spawn time.
                let crowd = found[..i].iter().filter(|o| (o.pos - e.pos).length() < AREA_M).count();
                assert!(crowd < AREA_CAP, "{scale:?}: area cap");
            }
            assert!(a.records.is_empty(), "night bodies never write site records");
        }
    }
}

#[test]
fn night_spawns_are_identical_across_runs_with_one_seed() {
    let trace = || {
        let (mut w, feet) = flat_world(VoxelScalePreset::OneMeter);
        set_time(&mut w, MIDNIGHT);
        let mut a = Adventure::default();
        a.set_difficulty(Difficulty::Hard);
        let mut out = Vec::new();
        for pass in 0..200u32 {
            // Moving the player exercises despawn and respawn along the way.
            let feet = feet + Vec3::X * (pass / 40) as f32 * 30.0;
            tick(&mut a, &w, feet, Budget::UNBOUNDED);
            out.extend(bodies(&a).iter().map(|e| (pass, e.site, e.slot, e.kind, e.pos.to_array().map(f32::to_bits))));
        }
        out
    };
    let first = trace();
    assert!(!first.is_empty());
    assert_eq!(first, trace());
}

#[test]
fn night_bodies_despawn_beyond_the_distance() {
    let (mut w, feet) = flat_world(VoxelScalePreset::OneMeter);
    set_time(&mut w, MIDNIGHT);
    let mut a = Adventure::default();
    run(&mut a, &w, feet, 60);
    assert!(!bodies(&a).is_empty());
    let far = feet + Vec3::new(DESPAWN_M + RING_MAX_M + 1.0, 0.0, 0.0);
    w.rules.mob_spawning = false;
    tick(&mut a, &w, far, Budget::UNBOUNDED);
    assert!(bodies(&a).is_empty(), "every body is farther than {DESPAWN_M} m");
}

#[test]
fn light_and_daylight_keep_spawns_away() {
    let (mut w, feet) = flat_world(VoxelScalePreset::OneMeter);
    set_time(&mut w, NOON);
    let mut a = Adventure::default();
    run(&mut a, &w, feet, 120);
    assert!(bodies(&a).is_empty(), "open sky at noon");

    // A 12 m torch grid leaves no ground cell farther than 8.5 m from a light.
    let ground = (feet.y).floor() as i32;
    let torches: Vec<_> = (-4..=4)
        .flat_map(|x| (-4..=4).map(move |z| (IVec3::new(x * 12, ground, z * 12), BlockId::Torch)))
        .collect();
    w.set_many(&torches);
    assert!(lit(&w, feet + Vec3::new(30.0, 0.0, 6.0)));
    assert!(!lit(&w, feet + Vec3::new(80.0, 0.0, 0.0)));
    set_time(&mut w, MIDNIGHT);
    run(&mut a, &w, feet, 120);
    assert!(bodies(&a).is_empty(), "torch light");
}

#[test]
fn a_wall_keeps_the_far_side_dark_for_spawns() {
    let (mut w, feet) = flat_world(VoxelScalePreset::OneMeter);
    let y = feet.y.floor() as i32;
    let torch = IVec3::new(0, y, 0);
    w.set(torch, BlockId::Torch);
    let far = Vec3::new(4.5, feet.y, 0.5);
    assert!(lit(&w, far), "open air");
    let wall: Vec<_> = (-3..=3).map(|z| (IVec3::new(2, y, z), BlockId::Stone)).collect();
    w.set_many(&wall);
    assert!(!lit(&w, far), "the wall blocks the lamp");
    w.set(IVec3::new(2, y, 0), BlockId::Air);
    assert!(lit(&w, far), "a gap in the wall lets the lamp around");
}

#[test]
fn a_roof_counts_as_darkness_by_day() {
    let (mut w, feet) = flat_world(VoxelScalePreset::OneMeter);
    let cell = (feet + Vec3::X * 30.0).floor().as_ivec3();
    assert!(!roofed(&w, cell));
    w.set_many(&[(cell + IVec3::Y * 4, BlockId::Stone), (cell + IVec3::Y * 5, BlockId::Stone)]);
    assert!(roofed(&w, cell));
}

#[test]
fn rules_and_peaceful_stop_night_spawns() {
    let (mut w, feet) = flat_world(VoxelScalePreset::OneMeter);
    set_time(&mut w, MIDNIGHT);
    w.rules.mob_spawning = false;
    let mut a = Adventure::default();
    run(&mut a, &w, feet, 120);
    assert!(bodies(&a).is_empty(), "mob spawning off");
    w.rules.mob_spawning = true;
    run(&mut a, &w, feet, 60);
    assert!(!bodies(&a).is_empty());
    a.set_difficulty(Difficulty::Peaceful);
    tick(&mut a, &w, feet, Budget::UNBOUNDED);
    assert!(a.enemies.iter().all(|e| !hostile(e)), "Peaceful clears hostiles");
}

/// A Hills world position beside an encounter site, with the site's id.
fn beside_encounter(w: &World) -> (IVec3, Vec3) {
    let cfg = w.sample_config();
    for i in 0..16 {
        for j in 0..16 {
            let (x, z) = (i * 48 - 384, j * 48 - 384);
            let probe = Vec3::new(x as f32, w.terrain_height_at(x, z) as f32, z as f32);
            let found = engine_worldgen::sites::encounters(&cfg, probe);
            if let Some((id, center, _)) = found.into_iter().find(|(id, c, _)| id.z == 0 && c.y > probe.y - 4.0) {
                return (id, center + Vec3::new(16.0, 0.0, 0.0));
            }
        }
    }
    panic!("no surface encounter near the origin for seed {SEED}");
}

fn simulate(level: Difficulty, nights: u32) -> (IVec3, usize, usize) {
    let mut w = World::with_seed(WorldType::Hills, VoxelScalePreset::OneMeter, SEED);
    let (site, feet) = beside_encounter(&w);
    let mut player = Player::new();
    player.pos = feet / w.voxel_scale.meters();
    w.adventure.set_difficulty(level);
    let (mut at_site, mut hostiles) = (0, 0);
    for night in 0..nights {
        set_time(&mut w, MIDNIGHT + night as f32 * 1e-3);
        for _ in 0..600 {
            super::super::tick(&mut w, &mut player, crate::simulation::STEP_SECONDS, false, false, false);
            let a = &w.adventure;
            at_site = at_site.max(a.enemies.iter().filter(|e| e.site == site).count());
            hostiles = hostiles.max(a.enemies.iter().filter(|e| hostile(e)).count());
        }
    }
    (site, at_site, hostiles)
}

#[test]
fn peaceful_spawns_no_hostiles_over_several_nights() {
    let (site, at_site, _) = simulate(Difficulty::Normal, 1);
    assert!(at_site > 0, "Normal fills encounter site {site:?}");
    let (_, at_site, hostiles) = simulate(Difficulty::Peaceful, 4);
    assert_eq!((at_site, hostiles), (0, 0));
}

#[test]
fn peaceful_calms_a_provoked_resident_instead_of_removing_it() {
    let (w, feet) = flat_world(VoxelScalePreset::OneMeter);
    let mut a = Adventure { difficulty: Difficulty::Peaceful, ..Adventure::default() };
    let mut boar = Enemy::new(IVec3::new(0, 0, 6), 0, super::super::fauna::BOAR, feet + Vec3::X * 4.0, 0);
    boar.hurt(1.0);
    assert!(boar.hostile());
    a.enemies.push(boar);
    run(&mut a, &w, feet, 1);
    assert_eq!(a.enemies.len(), 1, "the boar stays");
    assert!(!a.enemies[0].hostile(), "and grazes again");
}
