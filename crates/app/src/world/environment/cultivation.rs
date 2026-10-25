//! Cultivation is a finite inventory-to-soil transfer. Added foliage becomes
//! litter first; warmth and moisture must decompose it before nutrient uptake.
use super::*;
use crate::world::{World, inventory::Item};

pub fn compost(world: &mut World, p: IVec3, hotbar: usize) -> bool {
    if !State::enabled(world.generator_version) {
        return false;
    }
    let Some(stack) = world.inventory.hotbar.get(hotbar).copied().flatten() else {
        return false;
    };
    let Item::Block(material) = stack.item else {
        return false;
    };
    let Some(definition) = engine_content::environment::definition() else {
        return false;
    };
    let Some(input) = definition.organic_inputs.iter().find(|i| i.material == material) else {
        return false;
    };
    let substrate = world.get(p);
    if definition.soil(substrate).is_none() || world.get(p + IVec3::Y).is_solid() {
        return false;
    }
    if !world.environment.can_wake(&[p]) {
        return false;
    }
    let cfg = world.sample_config();
    if !world.environment.create_patch(cfg, p, substrate, true, true) {
        return false;
    }
    let dry =
        engine_core::environment::cell_volume(world.voxel_scale) * u64::from(input.dry_mg_m3) / 1_000_000_000;
    let nutrient = dry * u64::from(input.nutrient_per_mille) / 1_000;
    let key = world.environment.owner(p);
    let patch = world.environment.patches.get_mut(&key).unwrap();
    // At most a 5 cm litter layer at 200 kg/m3 dry bulk density.
    let layer_limit = engine_core::environment::face_area_cm2(world.voxel_scale) * 1_000;
    if patch.soil.litter + dry > layer_limit {
        world.expedition.say("This ground cannot accept more compost yet.");
        return true;
    }
    if !world.inventory.consume_from_hotbar(hotbar, stack.item) {
        return false;
    }
    patch.soil.litter += dry;
    patch.soil.litter_nutrient += nutrient;
    patch.revision += 1;
    world.environment.ledger.imported_litter_mg += dry;
    world.environment.ledger.imported_litter_nutrient_mg += nutrient;
    world.environment.wake(p);
    world.sync_from_inventory();
    world.expedition.say("Compost added. Keep the ground moist so it can decompose.");
    true
}
