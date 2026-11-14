use super::*;
use crate::simulation::residency::Residency;
use crate::simulation::sandbox::{ThermalCell, key};
use crate::world::World;
use engine_core::VoxelScalePreset;
use engine_voxel::ChunkData;
use std::collections::HashSet;
use std::sync::Arc;

/// One resident chunk of `material` with a flat ambient.
fn uniform(material: BlockId, ambient_c: f32) -> World {
    let mut w = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 157);
    w.chunks.insert(
        IVec3::ZERO,
        Arc::new(ChunkData {
            modified_bounds: None,
            structural_bounds: None,
            blocks: Box::new([material; 32 * 32 * 32]),
            light: Box::new([0; 32 * 32 * 32]),
            torches: HashSet::new(),
        }),
    );
    w.sandbox.climate = Climate::flat(ambient_c);
    w
}

fn hills() -> WorldSampleConfig {
    WorldSampleConfig {
        planet: None,
        generator_version: engine_worldgen::CURRENT_GENERATOR_VERSION,
        seed: 2738958700,
        world_type: WorldType::Hills,
        voxel_scale: VoxelScalePreset::OneMeter,
    }
}

fn surface(cfg: &WorldSampleConfig, x: i32, z: i32) -> IVec3 {
    IVec3::new(x, engine_worldgen::terrain_height(cfg, x, z) + 1, z)
}

#[test]
fn climate_is_flat_without_terrain() {
    let mut c = Climate::default();
    assert_eq!(c.ambient_c(IVec3::new(5, 900, -3)), FLAT_C);
    c.sync(WorldSampleConfig { world_type: WorldType::Superflat, ..hills() }, 0.5, 0.0);
    assert_eq!(c.ambient_c(IVec3::new(5, 900, -3)), FLAT_C);
}

#[test]
fn climate_follows_biome_altitude_depth_and_day() {
    let cfg = hills();
    let mut c = Climate::default();
    c.sync(cfg, WARMEST, 0.0);
    let air: Vec<f32> = (-8..=8).map(|i| c.ambient_c(surface(&cfg, i * 400, 0))).collect();
    let (lo, hi) = air.iter().fold((f32::MAX, f32::MIN), |(a, b), &t| (a.min(t), b.max(t)));
    assert!(hi - lo > 8.0, "biomes differ: {lo:.1}..{hi:.1}");

    let p = surface(&cfg, 0, 0);
    let deep = p - IVec3::Y * 20;
    let afternoon = (c.ambient_c(p), c.ambient_c(deep));
    c.sync(cfg, WARMEST + 0.5, 0.0);
    let night = (c.ambient_c(p), c.ambient_c(deep));
    assert!(afternoon.0 - night.0 >= 6.0, "air swings: {afternoon:?} {night:?}");
    assert!((afternoon.1 - night.1).abs() < 1e-3, "20 m of ground holds a steady temperature");
    c.sync(cfg, WARMEST, 1.0);
    assert!(c.ambient_c(p) < afternoon.0, "cloud cover damps the afternoon peak");

    // Air high above a column is colder than air at its foot, ground below is warmer.
    c.sync(cfg, 0.5, 0.0);
    assert!(c.ambient_c(p + IVec3::Y * 150) < c.ambient_c(p + IVec3::Y));
    assert!(c.ambient_c(p - IVec3::Y * 200) > c.ambient_c(p - IVec3::Y * 20));
}

#[test]
fn climate_keeps_natural_water_above_freezing() {
    let cfg = hills();
    let mut c = Climate::default();
    c.sync(cfg, 0.1, 0.0);
    let cold = (-40..=40)
        .flat_map(|i| (-4..=4).map(move |j| (i * 300, j * 300)))
        .map(|(x, z)| surface(&cfg, x, z) + IVec3::Y * 120)
        .find(|&p| c.ambient_c(p) < -2.0)
        .expect("high air somewhere is below freezing");
    assert!(c.ambient_for(cold, BlockId::Ice) < 0.0);
    assert_eq!(c.ambient_for(cold, BlockId::Water), WATER_FLOOR_C);
}

/// Steps the cell at `at` and its recorded neighbours, like the scheduler.
fn run(w: &mut World, at: IVec3, steps: usize, mut hold: impl FnMut(&mut World), stop: impl Fn(&World) -> bool) -> Option<usize> {
    let mut sources = Residency::default();
    for i in 0..steps {
        hold(w);
        for d in std::iter::once(IVec3::ZERO).chain([IVec3::X, IVec3::NEG_X, IVec3::Y, IVec3::NEG_Y, IVec3::Z, IVec3::NEG_Z]) {
            let q = at + d;
            if w.sandbox.cells.contains_key(&key(q)) {
                super::super::thermal::step(&mut sources, w, q, &mut HashSet::new(), &mut HashSet::new(), &mut HashSet::new());
            }
        }
        w.activity.tick = w.activity.tick.wrapping_add(1);
        if stop(w) {
            return Some(i + 1);
        }
    }
    None
}

#[test]
fn frost_ice_thaws_in_warm_air_after_its_latent_heat() {
    let mut w = uniform(BlockId::Air, 20.0);
    let p = IVec3::splat(8);
    w.set(p, BlockId::Ice);
    w.sandbox.set(p, ThermalCell { temperature: -5.0, ..Default::default() });
    let steps = run(&mut w, p, 4_000, |_| {}, |w| w.get(p) != BlockId::Ice).expect("ice melts at 20 °C");
    assert_eq!(w.get(p), BlockId::Water);
    // At 12.5 thermal steps per second (1 m cells) this is 30 to 90 s.
    assert!((375..=1_125).contains(&steps), "melted after {steps} steps");
}

#[test]
fn ice_and_natural_water_below_freezing_settle_without_changing_phase() {
    let mut w = uniform(BlockId::Air, -10.0);
    let (ice, water) = (IVec3::splat(8), IVec3::new(20, 8, 8));
    w.set(ice, BlockId::Ice);
    w.sandbox.set(ice, ThermalCell { temperature: 0.5, ..Default::default() });
    w.set(water, BlockId::Water);
    w.sandbox.set(water, ThermalCell { temperature: 6.0, ..Default::default() });
    for at in [ice, water] {
        let gone = run(&mut w, at, 20_000, |_| {}, |w| !w.sandbox.cells.contains_key(&key(at)));
        assert!(gone.is_some(), "{at}: a cell at its ambient leaves the sparse lattice");
    }
    assert_eq!((w.get(ice), w.get(water)), (BlockId::Ice, BlockId::Water));
    assert!(w.sandbox.cell(ice).temperature < -9.0);
    assert_eq!(w.sandbox.cell_for(water, BlockId::Water).temperature, WATER_FLOOR_C);
}

#[test]
fn water_beside_cold_ice_freezes_after_its_latent_heat() {
    let mut w = uniform(BlockId::Stone, 10.0);
    let (ice, water) = (IVec3::splat(8), IVec3::new(9, 8, 8));
    w.set(ice, BlockId::Ice);
    w.set(water, BlockId::Water);
    // A held -30 °C sink, like a frost spell's core.
    let hold = |w: &mut World| {
        w.sandbox.set(ice, ThermalCell { temperature: -30.0, ..Default::default() });
    };
    let steps = run(&mut w, ice, 6_000, hold, |w| w.get(water) == BlockId::Ice).expect("water freezes");
    assert!(steps > 50, "the latent heat goes first: {steps}");
}
