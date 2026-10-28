use super::*;
use engine_core::environment::cell_volume;

pub(super) fn process(
    world: &mut World,
    p: IVec3,
    blocks: &mut HashSet<IVec3>,
    chunks: &mut HashSet<IVec3>,
) -> bool {
    let Some(definition) = engine_content::environment::definition() else {
        return false;
    };
    let block = world.get(p);
    let below = p - IVec3::Y;
    let mut work = world.environment.hydraulic_work.get(&env::key(p)).copied().unwrap_or(0);
    let supported = world.get(below).is_solid();
    let mut settling =
        supported && world.environment.sediment.get(&env::key(p)).is_some_and(|s| s.suspended > 0);
    if let Some(grains) = world.environment.sediment.get_mut(&env::key(p)) {
        work -= grains.entrain(work, 2);
        if work == 0 && supported {
            let depth_um = 1_000_000 / world.environment.cells_per_metre as u64;
            grains.settle(depth_um, if block == B::Air { 1_000_000 } else { 10_000 });
            settling = grains.suspended > 0;
        }
    }
    if work == 0 {
        world.environment.hydraulic_work.remove(&env::key(p));
    } else {
        world.environment.hydraulic_work.insert(env::key(p), work);
    }
    let stock = world.environment.sediment_at(p);
    // Settled fines become editable geometry only once a complete cell fits.
    // Existing water is never deleted to make room for a deposit.
    let mass = cell_volume(world.voxel_scale) * 1_600 / 1_000;
    if block == B::Air && stock >= mass && world.get(p - IVec3::Y).is_solid() {
        let original = world.environment.sediment[&env::key(p)];
        world.environment.sediment.get_mut(&env::key(p)).unwrap().remove(mass);
        if stock == mass {
            world.environment.sediment.remove(&env::key(p));
        }
        world.set(p, B::Sand);
        if world.get(p) == B::Sand {
            world.environment.ledger.deposited_mg += mass;
            world.environment.hydraulic_work.remove(&env::key(p));
            changed(p, blocks, chunks);
        } else {
            world.environment.sediment.insert(env::key(p), original);
            return true;
        }
        return false;
    }
    let pending = world.environment.patches.get(&world.environment.owner(below)).is_some_and(|patch| {
        definition
            .soil(patch.material)
            .is_some_and(|law| patch.soil.erosion_work >= law.mass(world.voxel_scale))
    });
    if work == 0 && !pending {
        world.environment.hydraulic_work.remove(&env::key(p));
        return settling;
    }
    let Some(law) = definition.soil(world.get(below)) else {
        world.environment.hydraulic_work.remove(&env::key(p));
        return settling;
    };
    if !ensure_patch(world, below) {
        return true;
    }
    if !world.environment.sediment.contains_key(&env::key(below))
        && world.environment.sediment.len() >= env::MAX_SEDIMENT
    {
        return true;
    }
    let owner = world.environment.owner(below);
    let Some(patch) = world.environment.patches.get_mut(&owner) else {
        // A full patch table leaves the bank implicit; the work dissipates
        // instead of rescheduling this event every second.
        world.environment.hydraulic_work.remove(&env::key(p));
        return settling;
    };
    let roots = definition.species(patch.species).map_or(0, |s| {
        let mature =
            u64::from(s.mature_mg_m2) * engine_core::environment::face_area_cm2(world.voxel_scale) / 10_000;
        u64::from(s.root_resistance_kj_kg) * patch.soil.biomass.min(mature) / mature.max(1)
    });
    // Resistance is specific detachment energy in kJ/kg. mJ / (kJ/kg)
    // is exactly mg. The explicit geological rate only accelerates wear.
    let detached = work.saturating_mul(u64::from(world.environment.erosion_rate))
        / (u64::from(law.erosion_resistance_kj_kg) + roots).max(1);
    patch.soil.erosion_work =
        patch.soil.erosion_work.saturating_add(detached).min(law.mass(world.voxel_scale));
    world.environment.hydraulic_work.remove(&env::key(p));
    if patch.soil.erosion_work < law.mass(world.voxel_scale) {
        return settling;
    }
    let mass = law.mass(world.voxel_scale);
    let residual_water = patch.soil.water;
    if engine_core::environment::sediment_volume(world.environment.sediment_at(below) + mass) + residual_water
        > cell_volume(world.voxel_scale)
    {
        return true;
    }
    world.set(below, B::Air);
    if world.get(below) == B::Air {
        // Detached grains occupy the newly excavated space; they cannot be
        // inserted into an already full water cell above the bank.
        world.environment.sediment.entry(env::key(below)).or_default().suspended += mass;
        world.environment.ledger.eroded_mg += mass;
        if let Some(patch) = world.environment.patches.get_mut(&env::key(below)) {
            patch.soil.erosion_work = 0;
        }
        changed(below, blocks, chunks);
        world.environment.wake(below);
    }
    settling || world.get(below) != B::Air
}
