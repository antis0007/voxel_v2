use super::*;
use crate::{
    fluid::VoxelScalePreset,
    world::{ChunkData, WorldType},
};
use std::sync::Arc;

pub(super) fn uniform(scale: VoxelScalePreset, material: B) -> World {
    let mut world = World::with_seed(WorldType::Superflat, scale, 157);
    world.chunks.insert(
        IVec3::ZERO,
        Arc::new(ChunkData {
            modified_bounds: None,
            structural_bounds: None,
            blocks: Box::new([material; 32 * 32 * 32]),
            light: Box::new([0; 32 * 32 * 32]),
            torches: HashSet::new(),
        }),
    );
    world
}

#[test]
fn environment_buried_soil_uses_one_owner_at_every_resolution() {
    for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::HalfMeter, VoxelScalePreset::QuarterMeter] {
        let mut world = uniform(scale, B::Dirt);
        let n = (1.0 / scale.meters()) as i32;
        let p = IVec3::splat(8);
        assert!(ensure_patch(&mut world, p));
        for y in 0..n {
            for z in 0..n {
                for x in 0..n {
                    assert!(ensure_patch(&mut world, p + IVec3::new(x, y, z)));
                }
            }
        }
        assert_eq!(world.environment.patches.len(), 1);
        let owner = world.environment.owner(p);
        assert_eq!(world.environment.patches[&owner].soil.nutrient, 10_240);
        let water: u64 = world.environment.patches.values().map(|p| p.soil.water).sum();
        world.set(p, B::Air);
        assert_eq!(world.environment.patches.values().map(|p| p.soil.water).sum::<u64>(), water);
        if n > 1 {
            assert!(world.environment.patches.values().all(|p| p.extent == 1));
        }
    }
}

#[test]
fn environment_mixed_wall_prevents_bulk_aggregation() {
    let mut world = uniform(VoxelScalePreset::QuarterMeter, B::Dirt);
    let p = IVec3::splat(8);
    let chunk = world.chunks.get_mut(&IVec3::ZERO).unwrap();
    let mut chunk = chunk;
    Arc::make_mut(&mut chunk).blocks[ChunkData::local_index(9, 8, 8)] = B::Stone;
    drop(chunk);
    assert!(ensure_patch(&mut world, p));
    assert_eq!(world.environment.patches[&env::key(p)].extent, 1);
    let mut blocks = HashSet::new();
    let mut chunks = HashSet::new();
    soil::process(&mut world, p, 1, &mut blocks, &mut chunks);
    assert!(!world.environment.patches.contains_key(&env::key(p + IVec3::X)));
}

#[test]
fn environment_infiltration_and_excavation_conserve_actual_world_water() {
    let mut world = uniform(VoxelScalePreset::OneMeter, B::Stone);
    let p = IVec3::splat(8);
    let above = p + IVec3::Y;
    world.set(p, B::Dirt);
    world.set(above, B::Water);
    let initial = world.environment.patches[&env::key(p)].soil.water + world.water_microlitres(above);
    let mut blocks = HashSet::new();
    let mut chunks = HashSet::new();
    for tick in 1..=1_000 {
        soil::process(&mut world, p, tick, &mut blocks, &mut chunks);
        if tick == 1 {
            assert!(world.environment.patches[&env::key(p)].soil.water > 0);
        }
    }
    let soil_water = world.environment.patches[&env::key(p)].soil.water;
    assert!(soil_water > 0);
    assert_eq!(soil_water + world.water_microlitres(above), initial);
    world.set(p, B::Air);
    soil::process(&mut world, p, 1_001, &mut blocks, &mut chunks);
    let remaining = world.environment.patches.get(&env::key(p)).map_or(0, |patch| patch.soil.water);
    assert_eq!(remaining + world.water_microlitres(p) + world.water_microlitres(above), initial);
}

#[test]
fn environment_sediment_maintenance_is_an_inventory_transaction() {
    let mut world = uniform(VoxelScalePreset::QuarterMeter, B::Stone);
    let p = IVec3::splat(8);
    let mass = engine_core::environment::cell_volume(world.voxel_scale) * 1600 / 1000;
    world.environment.sediment.insert(env::key(p), (mass + 17).into());
    assert_eq!(env::inspect::intake_units(&world, p), 0);
    assert!(env::inspect::clear(&mut world, p));
    assert_eq!(world.environment.sediment_at(p), 17);
    assert_eq!(world.environment.ledger.removed_sediment_mg, mass);
    assert_eq!(world.inventory.count_item(crate::world::inventory::Item::Block(B::Sand)), 1);
}

#[test]
fn environment_sleeping_world_has_no_soil_rule_work() {
    let mut world = uniform(VoxelScalePreset::QuarterMeter, B::Stone);
    let mut sources = Residency::default();
    let mut blocks = HashSet::new();
    let mut chunks = HashSet::new();
    let start = std::time::Instant::now();
    for _ in 0..12_000 {
        update(
            &mut world,
            &mut sources,
            IVec3::splat(16),
            &mut blocks,
            &mut chunks,
            super::super::Budget::UNBOUNDED,
        );
    }
    assert!(world.environment.patches.is_empty());
    assert!(world.environment.serviced <= 100);
    assert_eq!(world.environment.reserved_bytes(), 0);
    assert_eq!(world.environment.lag_seconds(), 0);
    println!(
        "environment sleeping: 12000 fixed steps, {} contact probes, {:?}, 0 soil records",
        world.environment.serviced,
        start.elapsed()
    );
}
