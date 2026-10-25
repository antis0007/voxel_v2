//! Metre-sized soil aggregation only where every fine voxel is homogeneous
//! and buried. Surface, mixed and edited topology retains fine owners.
use super::*;
use engine_sim::environment::Soil;

pub(super) fn buried(world: &mut World, p: IVec3) -> bool {
    let n = world.environment.cells_per_metre;
    if n <= 1 || world.environment.patches.len() + (n * n * n) as usize > env::MAX_PATCHES {
        return false;
    }
    let origin = IVec3::new(p.x.div_euclid(n) * n, p.y.div_euclid(n) * n, p.z.div_euclid(n) * n);
    let material = world.get(p);
    if engine_content::environment::definition().is_none_or(|d| d.soil(material).is_none()) {
        return false;
    }
    let mut cells = Vec::with_capacity((n * n * n) as usize);
    for y in 0..n {
        for z in 0..n {
            for x in 0..n {
                let q = origin + IVec3::new(x, y, z);
                if !world.chunks.contains_key(&chunk(q))
                    || world.get(q) != material
                    || world.environment.patches.contains_key(&env::key(q))
                {
                    return false;
                }
                cells.push(q);
            }
        }
    }
    if env::faces(origin, n as u8)
        .into_iter()
        .any(|(q, _)| !world.chunks.contains_key(&chunk(q)) || !world.get(q).is_solid())
    {
        return false;
    }
    let mut combined = Soil::default();
    for q in cells {
        world.environment.create_patch(world.sample_config(), q, material, false, true);
        let Some(patch) = world.environment.patches.remove(&env::key(q)) else {
            return false;
        };
        combined.water += patch.soil.water;
        combined.nutrient += patch.soil.nutrient;
    }
    world.environment.patches.insert(
        env::key(origin),
        env::Patch {
            material,
            soil: combined,
            extent: n as u8,
            species: 0,
            revision: 0,
            last_tick: world
                .environment
                .event_tick
                .map_or(world.environment.target_tick, |tick| tick.saturating_sub(1)),
        },
    );
    true
}
