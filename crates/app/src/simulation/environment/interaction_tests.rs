use super::tests::uniform;
use super::*;
use engine_core::VoxelScalePreset;

#[test]
fn environment_quiet_settling_area_reduces_downstream_sediment() {
    let run = |pause| {
        let mut w = uniform(VoxelScalePreset::QuarterMeter, B::Stone);
        let p = IVec3::splat(8);
        let q = p + IVec3::X;
        w.set(p, B::Air);
        w.set(q, B::Air);
        w.set_water_volume(p, 4);
        w.environment.sediment.insert(env::key(p), 1_000_000.into());
        let mut blocks = HashSet::new();
        let mut chunks = HashSet::new();
        for _ in 0..pause {
            sediment::process(&mut w, p, &mut blocks, &mut chunks);
        }
        let initial = w.water_microlitres(p);
        assert!(w.apply_water_pair(p, 2, q, 2, 2));
        assert_eq!(w.water_microlitres(p) + w.water_microlitres(q), initial);
        assert_eq!(w.environment.sediment_at(p) + w.environment.sediment_at(q), 1_000_000);
        w.environment.sediment_at(q)
    };
    assert!(run(120) < run(0) / 4);
}

#[test]
fn environment_roots_stop_at_a_real_barrier_and_conserve_resource_stocks() {
    let mut w = uniform(VoxelScalePreset::QuarterMeter, B::Stone);
    let p = IVec3::splat(8);
    let below = p - IVec3::Y;
    w.set(p, B::Dirt);
    w.set(below, B::Dirt);
    w.set(p - IVec3::Y * 3, B::Dirt);
    let mut patch = w.environment.patches[&env::key(p)].clone();
    patch.soil.biomass = 1_000;
    patch.soil.water = 0;
    patch.soil.nutrient = 0;
    let source = w.environment.patches.get_mut(&env::key(below)).unwrap();
    source.soil.water = 3_000_000;
    source.soil.nutrient = 1_000;
    let deep = w.environment.patches.get_mut(&env::key(p - IVec3::Y * 3)).unwrap();
    deep.soil.water = 2_000_000;
    deep.soil.nutrient = 2_000;
    let before_deep = deep.soil;
    let species = engine_content::environment::definition().unwrap().species(1).unwrap();
    roots::supply(&mut w, p, &mut patch, species, 6_250_000, 1);
    let source = &w.environment.patches[&env::key(below)];
    assert!(patch.soil.water > 0 && patch.soil.nutrient > 0);
    assert_eq!(patch.soil.water + source.soil.water, 3_000_000);
    assert_eq!(patch.soil.nutrient + source.soil.nutrient, 1_000);
    assert_eq!(w.environment.patches[&env::key(p - IVec3::Y * 3)].soil, before_deep);
}

#[test]
fn environment_roof_query_and_column_invalidation_follow_edited_geometry() {
    let mut w = uniform(VoxelScalePreset::QuarterMeter, B::Stone);
    let p = IVec3::splat(8);
    w.set(p, B::Dirt);
    for y in 1..8 {
        w.set(p + IVec3::Y * y, B::Air);
    }
    let mut sources = Residency::default();
    assert!(light::prepare(&mut w, &mut sources, p));
    assert_eq!(light::shade(&w, p), 1_000);
    let unrelated = IVec3::new(9, 8, 8);
    w.set(unrelated, B::Dirt);
    assert!(w.environment.light.contains_key(&env::key(p)));
    w.set(p + IVec3::Y * 8, B::Air);
    assert!(!w.environment.light.contains_key(&env::key(p)));
    assert!(light::prepare(&mut w, &mut sources, p));
    assert_eq!(light::shade(&w, p), 1_000); // The next actual roof remains.
    let patch = w.environment.patches.get_mut(&env::key(p)).unwrap();
    patch.species = 1;
    patch.soil.biomass = 10_000;
    patch.soil.plant_nutrient = 250;
    let mut blocks = HashSet::new();
    let mut chunks = HashSet::new();
    soil::process(&mut w, p, 1, &mut blocks, &mut chunks);
    assert!(w.environment.patches[&env::key(p)].soil.biomass < 10_000);
}

#[test]
fn environment_compost_consumes_a_real_item_and_cycles_its_finite_nutrients() {
    let mut w = uniform(VoxelScalePreset::QuarterMeter, B::Stone);
    let p = IVec3::splat(8);
    w.set(p, B::Dirt);
    w.set(p + IVec3::Y, B::Air);
    assert_eq!(w.environment.patches[&env::key(p)].soil.nutrient, 0);
    w.inventory.hotbar[0] =
        Some(crate::world::inventory::ItemStack::new(crate::world::inventory::Item::Block(B::Leaves), 1));
    assert!(env::cultivation::compost(&mut w, p, 0));
    assert!(w.inventory.hotbar[0].is_none());
    let before = w.environment.patches[&env::key(p)].soil;
    assert_eq!(before.litter, w.environment.ledger.imported_litter_mg);
    assert_eq!(before.litter_nutrient, w.environment.ledger.imported_litter_nutrient_mg);
    assert!(before.litter_nutrient > 0);
    assert!(!env::cultivation::compost(&mut w, p, 0));
    assert_eq!(w.environment.patches[&env::key(p)].soil, before);
}

#[test]
fn environment_construction_displaces_exact_water_and_rejects_a_sealed_cell() {
    let mut w = uniform(VoxelScalePreset::QuarterMeter, B::Stone);
    let p = IVec3::splat(8);
    let q = p + IVec3::X;
    w.set(p, B::Air);
    let amount = 5_000_017;
    assert!(w.set_water_microlitres(p, amount));
    w.environment.sediment.insert(env::key(p), 1_000_000.into());
    w.set(p, B::Stone);
    assert_eq!(w.get(p), B::Water);
    assert_eq!(w.water_microlitres(p), amount);
    assert_eq!(w.environment.sediment_at(p), 1_000_000);
    w.set(q, B::Air);
    w.set(p, B::Stone);
    assert_eq!(w.get(p), B::Stone);
    assert_eq!(w.water_microlitres(q), amount);
    assert_eq!(w.environment.sediment_at(q), 1_000_000);
    assert_eq!(w.environment.sediment_at(p), 0);
}

#[test]
fn environment_sediment_becomes_geometry_once_without_duplicating_stock() {
    let mut w = uniform(VoxelScalePreset::QuarterMeter, B::Stone);
    let p = IVec3::splat(8);
    w.set(p, B::Air);
    let mass = engine_core::environment::cell_volume(w.voxel_scale) * 1600 / 1000;
    w.environment.sediment.insert(env::key(p), mass.into());
    let mut blocks = HashSet::new();
    let mut chunks = HashSet::new();
    sediment::process(&mut w, p, &mut blocks, &mut chunks);
    assert_eq!(w.get(p), B::Sand);
    assert_eq!(w.environment.sediment_at(p), 0);
    assert_eq!(w.environment.ledger.deposited_mg, mass);
    assert_eq!(w.environment.patches[&env::key(p)].soil.nutrient, 0);
}

#[test]
fn environment_water_edit_releases_existing_pores_and_accounts_only_for_new_water() {
    let mut w = uniform(VoxelScalePreset::QuarterMeter, B::Stone);
    let p = IVec3::splat(8);
    w.set(p, B::Dirt);
    w.environment.patches.get_mut(&env::key(p)).unwrap().soil.water = 1_234_567;
    w.set(p, B::Water);
    let volume = engine_core::environment::cell_volume(w.voxel_scale);
    assert_eq!(w.water_microlitres(p), volume);
    assert_eq!(w.environment.patches[&env::key(p)].soil.water, 0);
    assert_eq!(w.environment.ledger.edited_water_added_ul, volume - 1_234_567);
    w.set(p, B::Air);
    assert_eq!(w.environment.ledger.edited_water_removed_ul, volume);
}
