use super::*;
use crate::world::WorldType;
use engine_worldgen::station::{self, Kind, Plan};

const ORIGIN: Vec3 = Vec3::new(64.0, 600.0, -40.0);

/// A world with one built habitat whose gas grid has settled its geometry.
fn built() -> (World, Plan) {
    let mut world = World::new(WorldType::Superflat, engine_core::VoxelScalePreset::OneMeter);
    assert!(register(&mut world, glam::IVec2::new(-3, 2), ORIGIN, Vec3::ZERO));
    let player = Player::new();
    for _ in 0..4000 {
        tick(&mut world, &player);
        let h = &world.orbital.habitats.habitats[0];
        if h.ready && h.dirty.is_empty() && h.gas.iter().all(|q| q[0] >= 0.0) { break; }
    }
    let h = &world.orbital.habitats.habitats[0];
    assert!(h.ready && h.dirty.is_empty(), "habitat did not finish building");
    let plan = station::plan(h.wing.expect("new habitats carry a wing"));
    (world, plan)
}

fn run(world: &mut World, seconds: u32) {
    let player = Player::new();
    for _ in 0..seconds * 120 { tick(world, &player); }
}

/// Pressure at the head-height centre of a plan cell.
fn pressure(world: &World, cell: IVec3) -> f64 {
    let at = ORIGIN.as_ivec3() + geometry::wing_origin() + cell;
    readings_at(world, at.as_dvec3() + glam::DVec3::splat(0.5)).map_or(-1.0, |r| r.0)
}

fn head(space: &station::Space) -> IVec3 {
    ((space.min + space.max) / 2).with_y(1)
}

#[test]
fn grid_cells_round_trip_at_negative_coordinates() {
    for grid in [Grid::default(), Grid { min: IVec3::new(-7, -40, -13), size: IVec3::new(5, 9, 31) }] {
        for i in 0..grid.cells() {
            assert_eq!(grid.index(grid.local(i)), Some(i));
        }
        assert_eq!(grid.index(grid.min - IVec3::X), None);
        assert_eq!(grid.index(grid.min + grid.size), None);
        assert_eq!(grid.index(grid.min + grid.size - IVec3::ONE), Some(grid.cells() - 1));
    }
}

#[test]
fn wing_rooms_hold_air_and_the_damaged_section_stays_vacuum() {
    let (mut world, plan) = built();
    let h = &world.orbital.habitats.habitats[0];
    assert!(h.grid.valid() && h.grid.cells() > geometry::LEGACY_CELLS);
    let stock = |world: &World| world.orbital.habitats.habitats[0].gas.iter().map(|q| q[0] + q[1]).sum::<f64>();
    let before = stock(&world);
    run(&mut world, 30);
    assert!(stock(&world) > before * 0.99, "sealed station lost {} of {before}", before - stock(&world));
    for space in &plan.spaces {
        let p = pressure(&world, head(space));
        if space.kind == Kind::Damaged {
            assert!(p.abs() < 500.0, "damaged section holds {p} Pa");
        } else {
            assert!(p > 95_000.0, "{:?} holds {p} Pa", space.kind);
        }
    }
    // The drum still breathes and the endcap doorway joins it to the wing.
    assert!(readings_at(&world, (ORIGIN + Vec3::new(15.5, 1.5, 0.5)).as_dvec3()).is_some_and(|r| r.0 > 95_000.0));
    assert!(readings_at(&world, (ORIGIN + Vec3::new(27.5, 1.5, 0.5)).as_dvec3()).is_some_and(|r| r.0 > 95_000.0));
    persistence::validate(&world.orbital.habitats).unwrap();
}

#[test]
fn opening_the_bulkhead_vents_the_spine_through_the_breach() {
    let (mut world, plan) = built();
    let corridor = plan.spaces.iter().find(|s| s.kind == Kind::Corridor).unwrap();
    let start = pressure(&world, head(corridor));
    let (a, b) = plan.bulkhead;
    let mut edits = Vec::new();
    for x in a.x..=b.x { for y in a.y..=b.y { for z in a.z..=b.z {
        let cell = ORIGIN.as_ivec3() + geometry::wing_origin() + IVec3::new(x, y, z);
        assert_eq!(world.get(cell), B::SteelBlock);
        edits.push((cell, B::Air));
    } } }
    world.set_many_mechanism(&edits);
    run(&mut world, 60);
    let after = pressure(&world, head(corridor));
    // Conductance is a bounded gameplay rate: the whole station and drum drain together.
    assert!(after < start * 0.9, "corridor fell only from {start} to {after} Pa");
    assert!(world.orbital.habitats.habitats[0].escaped[0] > 0.0);
}

#[test]
fn wing_floor_pulls_down_and_the_drum_still_spins() {
    let (world, plan) = built();
    let corridor = plan.spaces.iter().find(|s| s.kind == Kind::Corridor).unwrap();
    let at = (ORIGIN.as_ivec3() + geometry::wing_origin() + head(corridor)).as_vec3();
    assert_eq!(acceleration(&world, at), Some(Vec3::NEG_Y * 9.81));
    let drum = acceleration(&world, ORIGIN + Vec3::new(18.0, 6.0, 4.0)).unwrap();
    assert!(drum.z > 1.0 && drum.y.abs() < 0.01);
}

#[derive(Serialize)]
struct OldHabitat {
    origin: IVec3, berth: Vec3, cell: glam::IVec2, build_cursor: usize, ready: bool,
    gas: Vec<[f64; 2]>, tank: [f64; 2], cabin: [f64; 2], escaped: [f64; 2],
    cycle: i8, doors_open: [bool; 2],
}
#[derive(Serialize)]
struct OldState { habitats: Vec<OldHabitat> }

#[test]
fn saves_from_before_the_wing_keep_the_drum_grid() {
    let old = OldState { habitats: vec![OldHabitat {
        origin: IVec3::new(10, 18_000, -4), berth: Vec3::new(10.0, 70.0, -4.0), cell: glam::IVec2::new(1, -1),
        build_cursor: geometry::LEGACY_CELLS, ready: true, gas: vec![[3.0, 7.0]; geometry::LEGACY_CELLS],
        tank: [1.0, 2.0], cabin: [3.0, 4.0], escaped: [0.0; 2], cycle: -1, doors_open: [false, true],
    }] };
    let state: State = ron::from_str(&ron::to_string(&old).unwrap()).unwrap();
    persistence::validate(&state).unwrap();
    let h = &state.habitats[0];
    assert_eq!((h.grid, h.wing), (Grid::default(), None));
    assert_eq!(h.gas[h.index(IVec3::new(12, 1, 0)).unwrap()], [3.0, 7.0]);
    assert_eq!(geometry::material(None, Vec3::new(40.5, 0.5, 0.5)), B::Air);
}

#[test]
fn wing_habitat_survives_the_save_footer() {
    let (world, _) = built();
    let mut words = Vec::new();
    persistence::encode(&world.orbital.habitats, &mut words);
    let mut at = 0;
    let state = persistence::decode(&mut || {
        let word = u32::from_le_bytes(words[at..at + 4].try_into().unwrap());
        at += 4;
        Ok(word)
    }).unwrap();
    let (a, b) = (&state.habitats[0], &world.orbital.habitats.habitats[0]);
    assert_eq!((a.grid, a.wing, a.gas.len()), (b.grid, b.wing, b.gas.len()));
    assert_eq!(a.gas, b.gas);
}

#[test]
fn wing_inhabitants_spawn_once_near_the_player_and_stay_dead() {
    let (mut world, plan) = built();
    let h = &world.orbital.habitats.habitats[0];
    assert!(h.populated);
    assert_eq!(h.residents.len(), plan.markers.len().min(residents::MAX));
    let mut player = Player::new();
    player.pos = ORIGIN + Vec3::new(31.0, 1.0, 0.0);
    player.precise_pos = player.pos.as_dvec3();
    for _ in 0..200 { tick(&mut world, &player); }
    let live = |w: &World| w.adventure.enemies.iter().filter(|e| e.slot >= residents::SLOT_BASE).count();
    assert!(live(&world) > 0, "no wing inhabitant materialised");
    for e in &mut world.adventure.enemies { if e.slot >= residents::SLOT_BASE { e.health = 0.0; } }
    let enemies = std::mem::take(&mut world.adventure.enemies);
    residents::remember(&mut world, &enemies);
    assert!(world.orbital.habitats.habitats[0].residents.is_empty());
    for _ in 0..200 { tick(&mut world, &player); }
    assert_eq!(live(&world), 0, "dead inhabitants respawned");
}

#[test]
fn a_habitat_saved_before_the_wing_gains_one_and_keeps_its_air() {
    let mut world = World::new(WorldType::Superflat, engine_core::VoxelScalePreset::OneMeter);
    assert!(register(&mut world, glam::IVec2::new(-3, 2), ORIGIN, Vec3::ZERO));
    {
        let h = &mut world.orbital.habitats.habitats[0];
        h.wing = None;
        h.grid = Grid::default();
    }
    let player = Player::new();
    let settled = |w: &World| {
        let h = &w.orbital.habitats.habitats[0];
        h.ready && h.dirty.is_empty() && h.gas.iter().all(|q| q[0] >= 0.0)
    };
    // The drum builds the old way; the retrofit starts on the first tick after it is ready.
    for _ in 0..4000 {
        tick(&mut world, &player);
        if world.orbital.habitats.habitats[0].wing.is_some() { break; }
    }
    assert!(world.orbital.habitats.habitats[0].retrofit, "retrofit did not start");
    let drum_cell = ORIGIN + Vec3::new(15.5, 1.5, 0.5);
    let patch = (ORIGIN.as_ivec3() + IVec3::new(15, 1, 0), B::StoneBricks);
    world.set_many_mechanism(&[patch]);
    for _ in 0..8000 {
        tick(&mut world, &player);
        let h = &world.orbital.habitats.habitats[0];
        if !h.retrofit && settled(&world) { break; }
    }
    let h = &world.orbital.habitats.habitats[0];
    assert!(h.wing.is_some() && !h.retrofit && settled(&world), "retrofit did not finish");
    assert_eq!(world.get(patch.0), B::StoneBricks, "retrofit overwrote a player block");
    let plan = station::plan(h.wing.unwrap());
    run(&mut world, 20);
    let drum_after = readings_at(&world, drum_cell.as_dvec3() + glam::DVec3::Y * 4.0).unwrap().0;
    assert!(drum_after > 95_000.0, "drum holds {drum_after} Pa");
    for space in plan.spaces.iter().filter(|s| s.kind != Kind::Damaged) {
        let p = pressure(&world, head(space));
        assert!(p > 90_000.0, "{:?} holds {p} Pa", space.kind);
    }
    persistence::validate(&world.orbital.habitats).unwrap();
}
