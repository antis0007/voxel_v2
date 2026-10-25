//! Bounded continuation of an authoritative vertical light query. Missing
//! geometry suspends the ray, rather than being interpreted as sky or a roof.
use super::*;
use crate::world::environment::light::{MAX_QUERIES, Query};
use crate::world::environment::sky::{MAX_CHUNKS, Summary};

pub(super) fn prepare(world: &mut World, sources: &mut Residency, p: IVec3) -> bool {
    let Some(definition) = engine_content::environment::definition() else {
        return false;
    };
    if definition.soil(world.get(p)).is_none() || covered(world.get(p + IVec3::Y)) {
        return true;
    }
    let k = env::key(p);
    if !world.environment.light.contains_key(&k) {
        if world.environment.light.len() >= MAX_QUERIES {
            // Evict a completed derived query only; unfinished work keeps its cursor.
            let Some(old) = world.environment.light.iter().find(|(_, q)| q.shade.is_some()).map(|(&k, _)| k)
            else {
                return false;
            };
            world.environment.light.remove(&old);
        }
        world.environment.light.insert(k, Query { next_y: p.y + 1, leaf_cells: 0, shade: None });
    }
    let mut query = world.environment.light[&k];
    if query.shade.is_some() {
        return true;
    }
    let top = world.sample_config().max_world_y();
    // At most one new 32^3-cell summary per service; other columns reuse it.
    // A ray can cross cached empty chunks without loading them again.
    let mut built = false;
    for _ in 0..64 {
        if query.next_y >= top {
            // One metre of foliage transmits half the diffuse sky availability.
            query.shade = Some(
                (1_000.0
                    * (1.0
                        - 0.5f64
                            .powf(query.leaf_cells as f64 / f64::from(world.environment.cells_per_metre))))
                .round() as u32,
            );
            break;
        }
        let q = IVec3::new(p.x, query.next_y, p.z);
        let c = env::key(chunk(q));
        if !world.environment.sky.contains_key(&c) {
            if built || !sources.request_for(world, chunk(q), chunk(p)) {
                break;
            }
            let Some(data) = sources.chunk_data(world, chunk(q)) else {
                break;
            };
            let summary = Summary::new(data, world.environment.fixed_steps);
            if world.environment.sky.len() >= MAX_CHUNKS
                && let Some(old) =
                    world.environment.sky.iter().min_by_key(|(k, s)| (s.touched, **k)).map(|(&k, _)| k)
                {
                    world.environment.sky.remove(&old);
                }
            world.environment.sky.insert(c, summary);
            built = true;
        }
        let Some(summary) = world.environment.sky.get_mut(&c) else {
            return false;
        };
        summary.touched = world.environment.fixed_steps;
        let (blocked, leaves) = summary.above(q);
        if blocked {
            query.shade = Some(1_000);
            break;
        }
        query.leaf_cells += leaves;
        query.next_y = (q.y.div_euclid(32) + 1) * 32;
    }
    let done = query.shade.is_some();
    world.environment.light.insert(k, query);
    done
}

pub(super) fn shade(world: &World, p: IVec3) -> u32 {
    if covered(world.get(p + IVec3::Y)) {
        return 1_000;
    }
    world.environment.light.get(&env::key(p)).and_then(|q| q.shade).unwrap_or(1_000)
}
fn covered(material: B) -> bool {
    // The implemented cohorts are terrestrial. Submerged ground cannot
    // establish them; aquatic plants need their own light/respiration laws.
    material == B::Water || (material.is_solid() && !matches!(material, B::Leaves | B::PineLeaves | B::Bush))
}
