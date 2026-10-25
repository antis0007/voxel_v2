//! Groundcover reaches one physical metre through connected porous substrate.
//! Root access is derived from live geometry, not stored on every fine cell.
use super::*;

pub(super) fn supply(
    world: &mut World,
    p: IVec3,
    patch: &mut env::Patch,
    species: &engine_content::environment::Species,
    capacity: u64,
    seconds: u32,
) {
    if patch.soil.biomass == 0 {
        return;
    }
    let area = engine_core::environment::face_area_cm2(world.voxel_scale);
    let growth = u64::from(species.growth_mg_m2_s)
        * area
        * u64::from(world.environment.biological_rate)
        * u64::from(seconds)
        / 10_000;
    let mut needed_nutrient = (growth / 40).saturating_sub(patch.soil.nutrient);
    let mut needed_water =
        (growth * u64::from(species.water_ul_per_mg)).min(capacity / 2).saturating_sub(patch.soil.water);
    for depth in 1..world.environment.cells_per_metre {
        if needed_nutrient == 0 && needed_water == 0 {
            break;
        }
        let q = p - IVec3::Y * depth;
        if engine_content::environment::definition().is_none_or(|d| d.soil(world.get(q)).is_none()) {
            break;
        }
        if !world.environment.can_wake(&[q]) || !ensure_patch(world, q) {
            break;
        }
        let owner = world.environment.owner(q);
        if let Some(source) = world.environment.patches.get_mut(&owner) {
            let nutrient = source.soil.nutrient.min(needed_nutrient);
            let water = source.soil.water.min(needed_water);
            source.soil.nutrient -= nutrient;
            source.soil.water -= water;
            patch.soil.nutrient += nutrient;
            patch.soil.water += water;
            needed_nutrient -= nutrient;
            needed_water -= water;
            if nutrient + water > 0 {
                source.revision += 1;
                let delay = super::soil::interval(world, q);
                world.environment.wake_after(q, delay);
            }
        }
    }
}
