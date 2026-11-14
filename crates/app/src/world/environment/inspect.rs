//! Actionable observations; exact stored quantities remain available to tools.
use super::*;
use crate::world::{
    World,
    inventory::{Item, ItemStack},
};

#[derive(Clone, Copy, Debug)]
pub struct Inspection {
    pub condition: &'static str,
    pub water_ul: u64,
    pub sediment_mg: u64,
    pub pollution_mg: u64,
    pub air: [f32; super::super::industry::SPECIES],
}

pub fn at(world: &World, p: IVec3) -> Option<Inspection> {
    let pollution_mg=world.industry.contamination(p,world.voxel_scale.meters());
    let air=world.industry.concentration(p+IVec3::Y,world.voxel_scale.meters());
    if pollution_mg > 0 || air.iter().zip(&super::super::industry::config::CONFIG.species)
        .any(|(&v, species)| v > 1.0 && species.harm_mg_m3 > 0.0) {
        return Some(Inspection { condition: "Pollution", water_ul: world.water_microlitres(p), sediment_mg: 0, pollution_mg, air });
    }
    if !State::enabled(world.generator_version) { return None; }
    let mass = sediment_cell_mass(world);
    let stock = world.environment.sediment_at(p);
    let observation = |condition| Inspection {
        condition,
        water_ul: world.environment.patches.get(&world.environment.owner(p))
            .map_or_else(|| world.water_microlitres(p), |patch| patch.soil.water),
        sediment_mg: stock,
        pollution_mg, air,
    };
    if stock >= mass {
        return Some(observation("Flow Blocked"));
    }
    if stock > mass / 8 {
        return Some(observation("Sediment"));
    }
    let Some(patch) = world.environment.patches.get(&world.environment.owner(p)) else {
        return engine_content::environment::definition()?.soil(world.get(p)).map(|_| observation("Soil"));
    };
    if patch.soil.biomass > 0
        && world.environment.light.get(&world.environment.owner(p)).and_then(|q| q.shade).is_some_and(
            |shade| {
                engine_content::environment::definition()
                    .and_then(|d| d.species(patch.species))
                    .is_some_and(|s| shade > s.shade_tolerance)
            },
        )
    {
        return Some(observation("Shaded"));
    }
    let capacity = engine_content::environment::definition()?
        .soil(patch.material)?
        .capacity(patch.scale(world.voxel_scale));
    let moisture = patch.soil.water * 1_000 / capacity.max(1);
    Some(if moisture < 150 {
        observation("Dry")
    } else if moisture > 850 {
        observation("Waterlogged")
    } else if patch.soil.nutrient == 0 && patch.soil.biomass == 0 {
        observation("Depleted")
    } else if patch.soil.biomass > 0 {
        observation("Rooted")
    } else {
        observation("Moist")
    })
}

fn sediment_cell_mass(world: &World) -> u64 {
    engine_core::environment::cell_volume(world.voxel_scale) * 1_600 / 1_000
}

pub fn intake_units(world: &World, p: IVec3) -> u8 {
    if !State::enabled(world.generator_version) {
        return 8;
    }
    let mass = sediment_cell_mass(world);
    8 - (world.environment.sediment_at(p).saturating_mul(8) / mass).min(8) as u8
}

/// A normal inventory transaction, not a sediment-delete button. Leftovers
/// stay at the intake; a full backpack leaves all environmental stock intact.
pub fn clear(world: &mut World, p: IVec3) -> bool {
    if !State::enabled(world.generator_version) {
        return false;
    }
    let mass = sediment_cell_mass(world);
    let stock = world.environment.sediment_at(p);
    if stock < mass {
        return false;
    }
    let stack = ItemStack { item: Item::Block(BlockId::Sand), count: 1 };
    if !world.inventory.can_insert(stack) {
        world.expedition.say("Inventory Full");
        return true;
    }
    let (inserted, _) = world.inventory.try_insert(stack);
    if inserted == 1 {
        if stock == mass {
            world.environment.sediment.remove(&key(p));
        } else {
            world.environment.sediment.get_mut(&key(p)).unwrap().remove(mass);
        }
        world.environment.ledger.removed_sediment_mg += mass;
        world.environment.wake(p);
        world.activity.wake(p);
        world.sync_from_inventory();
        world.expedition.say("Sediment Recovered");
    }
    true
}
