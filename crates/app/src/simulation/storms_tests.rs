use super::*;
use crate::simulation::climate::Climate;
use crate::world::{WorldType, inventory::Item};
use engine_core::VoxelScalePreset;
use std::collections::HashSet;
use std::sync::Arc;

/// Floor top cell in the middle of the test chunk.
const FLOOR: IVec3 = IVec3::new(16, 7, 16);

/// One resident chunk: stone up to y = 7, air above, at a fixed ambient.
fn field(celsius: f32) -> World {
    let mut w = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 4021);
    let mut blocks = Box::new([B::Air; 32 * 32 * 32]);
    for y in 0..=FLOOR.y {
        for z in 0..32 {
            for x in 0..32 {
                blocks[ChunkData::local_index(x, y, z)] = B::Stone;
            }
        }
    }
    w.chunks.insert(IVec3::ZERO, Arc::new(ChunkData {
        modified_bounds: None,
        structural_bounds: None,
        blocks,
        light: Box::new([0; 32 * 32 * 32]),
        torches: HashSet::new(),
    }));
    w.sandbox.climate = Climate::flat(celsius);
    w
}

fn put(w: &mut World, p: IVec3, b: B, light: u8) {
    let mut c = w.chunks.get_mut(&IVec3::ZERO).expect("resident chunk");
    let c = Arc::make_mut(&mut c);
    c.blocks[ChunkData::local_index(p.x, p.y, p.z)] = b;
    c.light[ChunkData::local_index(p.x, p.y, p.z)] = light;
}

fn snow_cells(w: &World) -> usize {
    (0..32).flat_map(|z| (0..32).map(move |x| IVec3::new(x, FLOOR.y + 1, z))).filter(|&p| w.get(p) == B::SnowLayer).count()
}

#[test]
fn storms_read_untouched_terrain_only_when_cold() {
    // Chunk (3, 0, 3) is not resident: reading it runs the generator per voxel.
    let w = field(10.0);
    let column = IVec3::new(100, FLOOR.y + 1, 100);
    assert_eq!(scan(&w, column, 1.0, false, Budget::UNBOUNDED), None, "warm: generated terrain is not read");
    assert!(scan(&w, column, 1.0, true, Budget::UNBOUNDED).is_some(), "cold: it is, to settle snow");
    // Resident chunks are always read, so placed snow can still melt in warm air.
    assert_eq!(scan(&w, FLOOR + IVec3::Y, 1.0, false, Budget::UNBOUNDED), Some(FLOOR));
}

#[test]
fn storms_lightning_lights_fuel_from_above_and_keeps_the_block() {
    let mut w = field(10.0);
    let log = FLOOR + IVec3::Y;
    put(&mut w, log, B::Log, 0);
    ignite(&mut w, log);
    assert_eq!(w.get(log), B::Log, "thermal combustion burns the log, not the strike");
    assert_eq!(w.get(log + IVec3::Y), B::Fire);
    assert!(w.sandbox.cell(log + IVec3::Y).temperature > 500.0, "fire carries heat to ignite the log");

    // Stone has no fuel: nothing lights.
    let mut bare = field(10.0);
    ignite(&mut bare, FLOOR);
    assert_eq!(bare.get(FLOOR + IVec3::Y), B::Air);
}

#[test]
fn storms_lightning_spares_a_ledge_above_the_strike() {
    let at = Vec3::new(10.5, 8.0, 10.5);
    assert!(hits(Vec3::new(11.0, 8.0, 10.0), at), "standing on the struck surface");
    assert!(!hits(Vec3::new(11.0, 20.0, 10.0), at), "cliff edge 12 m above the struck base");
    assert!(!hits(Vec3::new(11.0, 4.0, 10.0), at), "floor under a struck roof");
    assert!(!hits(Vec3::new(14.0, 8.0, 10.5), at), "3.5 m away");
}

#[test]
fn storms_snow_settles_only_when_cold_and_open() {
    let above = FLOOR + IVec3::Y;
    let cold = field(-10.0);
    assert_eq!(surface(&cold, above, 1.0), Some(FLOOR));
    assert_eq!(change(&cold, FLOOR, 1.0, 0), Some((above, B::Air, B::SnowLayer)));
    assert_eq!(change(&cold, FLOOR, 0.0, 0), None, "no snow without precipitation");
    assert_eq!(change(&field(5.0), FLOOR, 1.0, 0), None, "rain does not settle");

    // A roof is the open surface; the floor under it never sees the sky.
    let mut roofed = field(-10.0);
    put(&mut roofed, FLOOR + IVec3::Y * 6, B::Stone, 0);
    assert_eq!(surface(&roofed, above, 1.0), Some(FLOOR + IVec3::Y * 6));

    let mut w = field(-10.0);
    for clock in 0..4000 {
        cover(&mut w, above, 1.0, clock, Budget::UNBOUNDED);
    }
    assert!(snow_cells(&w) > 20, "snow cells: {}", snow_cells(&w));
    let mut warm = field(5.0);
    for clock in 0..4000 {
        cover(&mut warm, above, 1.0, clock, Budget::UNBOUNDED);
    }
    assert_eq!(snow_cells(&warm), 0);
}

#[test]
fn storms_snow_melts_when_warm_or_lit() {
    let snow = FLOOR + IVec3::Y;
    let mut w = field(5.0);
    put(&mut w, snow, B::SnowLayer, 0);
    assert_eq!(surface(&w, snow, 1.0), Some(snow));
    assert_eq!(change(&w, snow, 0.0, 0), Some((snow, B::SnowLayer, B::Air)));
    let mut cold = field(-10.0);
    put(&mut cold, snow, B::SnowLayer, 0);
    assert_eq!(change(&cold, snow, 1.0, 0), None, "snow does not pile up in one cell");
    put(&mut cold, snow, B::SnowLayer, 12);
    assert_eq!(change(&cold, snow, 1.0, 0), Some((snow, B::SnowLayer, B::Air)), "a torch melts it");
    assert_eq!(Item::canonical_block(B::SnowLayer), None, "breaking snow gives nothing");
}

#[test]
fn storms_still_water_freezes_and_its_ice_thaws() {
    let mut w = field(-10.0);
    put(&mut w, FLOOR, B::Water, 0);
    assert_eq!(change(&w, FLOOR, 0.0, 0), Some((FLOOR, B::Water, B::Ice)));
    assert_eq!(change(&w, FLOOR, 0.0, 999), None, "freezing is a slow roll");
    put(&mut w, FLOOR + IVec3::X, B::Air, 0);
    assert_eq!(change(&w, FLOOR, 0.0, 0), None, "water with an open side is moving");
    put(&mut w, FLOOR + IVec3::X, B::Stone, 0);
    let ice = effects::replace(&mut w, FLOOR, B::Water, B::Ice);
    assert!(ice.is_ok(), "the world accepts the freeze");
    let _ = ice.map(|commit| commit.publish_voxel(&w, &mut Vec::new()));
    assert_eq!(w.get(FLOOR), B::Ice);

    let mut warm = field(8.0);
    put(&mut warm, FLOOR, B::Ice, 0);
    put(&mut warm, FLOOR - IVec3::Y, B::Water, 0);
    assert_eq!(change(&warm, FLOOR, 0.0, 0), Some((FLOOR, B::Ice, B::Water)));
    put(&mut warm, FLOOR - IVec3::Y, B::Stone, 0);
    assert_eq!(change(&warm, FLOOR, 0.0, 0), None, "ice on ground is not a pond skin");
}

#[test]
fn storms_lightning_column_is_deterministic_and_in_range() {
    let centre = IVec3::new(-300, 40, 1200);
    let mut strikes = 0;
    for clock in 0..240_000u64 {
        let a = strike_column(77, clock, centre, 0.5, 0.9);
        assert_eq!(a, strike_column(77, clock, centre, 0.5, 0.9));
        assert_eq!(strike_column(77, clock, centre, 0.5, THUNDER_CUMULONIMBUS - 0.01), None);
        if let Some(c) = a {
            strikes += 1;
            assert_eq!(c.y, centre.y);
            let d = (c - centre).as_vec3().length() * 0.5;
            assert!(d <= STRIKE_RANGE_M + 1.0, "{d} m");
        }
    }
    assert!((50..200).contains(&strikes), "{strikes} strikes in 2000 s");
}

#[test]
fn storms_work_stops_at_the_budget() {
    let mut w = field(-10.0);
    for clock in 0..4000 {
        cover(&mut w, FLOOR + IVec3::Y, 1.0, clock, Budget::wall(std::time::Duration::ZERO));
    }
    assert_eq!(snow_cells(&w), 0);
}

#[test]
fn storms_flash_brightens_the_sky_and_fades() {
    let base = LightingState::from_time(0.9);
    let lit = base.with_flash(1.0);
    assert!(lit.ambient_color[2] > base.ambient_color[2] + 0.5);
    assert_eq!(base.with_flash(0.0).sky_top, base.sky_top);
    let mut day = DayNightCycle::default();
    day.storm.flash_s = FLASH_SECONDS;
    assert_eq!(day.storm.flash(), 1.0);
    day.storm.flash_s = 0.0;
    assert_eq!(day.storm.flash(), 0.0);
}
