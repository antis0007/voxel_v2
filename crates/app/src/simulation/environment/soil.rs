use super::*;
use engine_core::environment::{cell_volume, face_area_cm2, surface_flux};
use engine_sim::environment::{PlantLaw, drain_over, infiltrate_over, vegetation};

pub(super) const MAX_SECONDS: u64 = 8;
/// Slow soil has its own bounded interval. Immediate fluid contact and edits
/// wake earlier; the accumulated time is integrated in physical seconds.
pub(super) fn interval(world: &World, p: IVec3) -> u64 {
    let Some(patch) = world.environment.patches.get(&world.environment.owner(p)) else {
        return 1;
    };
    if world.water_volume(p + IVec3::Y) > 0 {
        return 1;
    }
    let Some(law) = engine_content::environment::definition().and_then(|d| d.soil(patch.material)) else {
        return 1;
    };
    // Limit the maximum outgoing face flux to a quarter of storage per solve.
    let flux = u64::from(law.drainage_um_s) * face_area_cm2(patch.scale(world.voxel_scale)) * 6 / 10;
    let stable = (law.capacity(patch.scale(world.voxel_scale)) / (4 * flux.max(1))).max(1);
    let biological = if patch.soil.biomass + patch.soil.litter > 0 { 4 } else { MAX_SECONDS };
    stable.min(biological)
}

pub(super) fn process(
    world: &mut World,
    p: IVec3,
    tick: u64,
    blocks: &mut HashSet<IVec3>,
    chunks: &mut HashSet<IVec3>,
) -> bool {
    let Some(definition) = engine_content::environment::definition() else {
        return false;
    };
    // A full table leaves the cell implicit; the event then completes.
    if !ensure_patch(world, p) {
        return false;
    }
    let p = env::position(world.environment.owner(p));
    let Some(mut patch) = world.environment.patches.get(&env::key(p)).cloned() else {
        return false;
    };
    if patch.last_tick == tick {
        return true;
    }
    let Some(law) = definition.soil(patch.material) else {
        return false;
    };
    let material = world.get(p);
    let scale = patch.scale(world.voxel_scale);
    world.environment.repair_initial_moisture(world.sample_config(), p, &mut patch);
    let before = patch.soil;
    let seconds = tick.saturating_sub(patch.last_tick).clamp(1, MAX_SECONDS) as u32;
    let mut pending = false;
    if definition.soil(material).is_none() {
        // Excavation leaves damp residue. Capillary-bound moisture is not a
        // pool: only drainable water seeps out, at the substrate's conductivity.
        // The residue remains an owned, persisted stock until it dries.
        if material == B::Air || (material == B::Water && !world.is_steady_surface_water(p)) {
            let old = world.water_microlitres(p);
            let room = cell_volume(scale).saturating_sub(
                old + engine_core::environment::sediment_volume(world.environment.sediment_at(p)),
            );
            let mobile = patch.soil.water.saturating_sub(law.retained_water(scale));
            let rate = surface_flux(law.drainage_um_s, face_area_cm2(scale), seconds,
                &mut patch.soil.flux_carry);
            let released = mobile.min(room).min(rate);
            if released > 0 {
                patch.soil.water -= released;
                // Reserve the ownership handoff before checking free capacity.
                world.environment.patches.get_mut(&env::key(p)).unwrap().soil.water = patch.soil.water;
                if world.set_water_microlitres(p, old + released) {
                    changed(p, blocks, chunks);
                } else {
                    patch.soil.water += released;
                }
            }
            if material == B::Air && old == 0 {
                let evaporated = surface_flux(world.environment.evaporation_um_s,
                    face_area_cm2(scale), seconds, &mut patch.soil.flux_carry)
                    .min(patch.soil.water);
                patch.soil.water -= evaporated;
                world.environment.ledger.evaporated_water += evaporated;
            }
            pending = patch.soil.water > law.retained_water(scale)
                || (material == B::Air && old == 0 && patch.soil.water > 0
                    && world.environment.evaporation_um_s > 0);
        }
    } else {
        let above = p + IVec3::Y;
        let mut old = world.water_microlitres(above);
        if old > 0 && world.is_steady_surface_water(above) {
            old = 0;
        }
        let water = infiltrate_over(&mut patch.soil, law, scale, old, seconds);
        if water > 0 {
            if world.set_water_microlitres(above, old - water) {
                changed(above, blocks, chunks);
            } else {
                patch.soil.water -= water;
                pending = true;
            }
        }
        pending |= old > 0 && patch.soil.water < law.capacity(scale);
        // Only actual porous face neighbors exchange. A wall, air gap, cave
        // ceiling or different storey never becomes an implicit connection.
        for (q, direction) in env::faces(p, patch.extent) {
            if direction == IVec3::Y {
                continue;
            }
            if patch.soil.water == 0 {
                break;
            }
            if direction == IVec3::NEG_Y && patch.soil.water <= law.retained_water(scale) {
                continue;
            }
            let other_material = world.get(q);
            let Some(other_law) = definition.soil(other_material) else {
                // A trench or exposed mine wall is a real drainage outlet.
                // Dry/bound moisture stays in the substrate; saturated ground
                // supplies a finite seep instead of an inexhaustible source.
                let mobile = patch.soil.water.saturating_sub(law.retained_water(scale));
                if mobile > 0 && matches!(other_material, B::Air | B::Water) {
                    let capacity = law.capacity(scale);
                    let table = engine_worldgen::water_table(&world.sample_config(), q.x, q.z);
                    // Unsaturated soil drains downward. A lateral outlet needs
                    // saturated ground or a face below the aquifer head.
                    if direction != IVec3::NEG_Y && q.y > table
                        && patch.soil.water < capacity.saturating_mul(95) / 100 { continue; }
                    let old = world.water_microlitres(q);
                    let head = if q.y <= table { (table + 1) as f64 } else {
                        p.y as f64 + f64::from(patch.extent)
                            * patch.soil.water as f64 / capacity.max(1) as f64
                    };
                    let equilibrium = ((head - q.y as f64).clamp(0.0, 1.0)
                        * cell_volume(world.voxel_scale) as f64) as u64;
                    let room = world.environment.water_capacity_ul(q).min(equilibrium).saturating_sub(old);
                    let rate = surface_flux(law.drainage_um_s, face_area_cm2(world.voxel_scale),
                        seconds, &mut patch.soil.flux_carry);
                    let seep = mobile.min(room).min(rate);
                    if seep > 0 && world.set_water_microlitres(q, old + seep) {
                        patch.soil.water -= seep;
                        changed(q, blocks, chunks);
                    }
                    pending |= room > 0;
                }
                continue;
            };
            if !world.environment.can_wake(&[q]) || !ensure_patch(world, q) {
                pending = true;
                continue;
            }
            let owner = world.environment.owner(q);
            if let Some(other) = world.environment.patches.get_mut(&owner) {
                let other_scale = other.scale(world.voxel_scale);
                let moved = drain_over(
                    &mut patch.soil,
                    &mut other.soil,
                    law,
                    other_law,
                    scale,
                    other_scale,
                    face_area_cm2(world.voxel_scale),
                    direction == IVec3::NEG_Y,
                    seconds,
                );
                if moved > 0 {
                    other.revision += 1;
                    let delay = interval(world, q);
                    world.environment.wake_after(q, delay);
                    pending = true;
                }
            }
        }
        let exposed = world.get(above) == B::Air;
        if exposed {
            let mut carry = 0;
            let evaporated =
                surface_flux(world.environment.evaporation_um_s, face_area_cm2(scale), seconds, &mut carry)
                    .min(patch.soil.water);
            patch.soil.water -= evaporated;
            world.environment.ledger.evaporated_water += evaporated;
        }
        // Buried mineral storage has no light/climate/colonisation consumer.
        // Its hydraulic exchanges still run; don't evaluate ecology per litre
        // of subsurface water simply because the canonical voxel is small.
        if patch.extent == 1 && (exposed || patch.soil.biomass > 0 || patch.soil.litter > 0) {
            let cfg = world.sample_config();
            let metric = p.as_vec3() * cfg.voxel_meters();
            let shade = super::light::shade(world, p);
            let conditions = engine_worldgen::ecology::Conditions {
                temperature_c: world.sandbox.cell_for(p, material).temperature.round() as i32,
                shade_per_mille: shade,
                moisture_per_mille: (patch.soil.water * 1_000 / law.capacity(scale).max(1)) as u32,
            };
            if patch.soil.biomass == 0 && tick / 30 > patch.last_tick / 30 {
                patch.species = engine_worldgen::ecology::colonist(&cfg, metric, conditions);
            }
            if let Some(species) = definition.species(patch.species) {
                if shade <= species.shade_tolerance {
                    super::roots::supply(world, p, &mut patch, species, law.capacity(scale), seconds);
                }
                let exchange = vegetation(
                    &mut patch.soil,
                    PlantLaw {
                        moisture: [species.moisture_min, species.moisture_max],
                        temperature_c: [species.temperature_min_c, species.temperature_max_c],
                        shade_tolerance: species.shade_tolerance,
                        growth_mg_m2_s: if crate::world::industry::dirty_soil(world,p) {0} else {species.growth_mg_m2_s},
                        water_ul_per_mg: species.water_ul_per_mg,
                        mature_mg_m2: species.mature_mg_m2,
                    },
                    law,
                    scale,
                    conditions.temperature_c,
                    conditions.shade_per_mille,
                    world.environment.biological_rate * seconds,
                );
                world.environment.ledger.transpired_water += exchange.transpired;
                world.environment.ledger.atmospheric_biomass += exchange.atmospheric_biomass;
                world.environment.ledger.respired_biomass += exchange.decomposed;
                let mature = u64::from(species.mature_mg_m2) * face_area_cm2(scale) / 10_000;
                let cover = if patch.soil.biomass > mature / 2 { species.cover } else { B::Dirt };
                let cover = if cover == B::SnowGrass && world.sandbox.cell_for(p, material).temperature > 0.0 { B::Grass } else { cover };
                // Hysteresis avoids repeated remeshing around a threshold.
                if matches!(material, B::Grass | B::Dirt | B::SnowGrass)
                    && (patch.soil.biomass > mature / 2 || patch.soil.biomass < mature / 8)
                    && material != cover
                {
                    world.set(p, cover);
                    patch.material = world.get(p);
                    changed(p, blocks, chunks);
                }
            }
            pending |= (exposed && patch.soil.water > 0)
                || patch.soil.biomass > 0
                || (patch.soil.litter > 0 && patch.soil.water > 0 && conditions.temperature_c > 0);
        }
    }
    if super::prune::inert(&patch, definition.soil(material).is_some()) {
        // An empty record over a dug-out cell regenerates identically.
        world.environment.patches.remove(&env::key(p));
        return pending;
    }
    patch.last_tick = tick;
    if patch.soil != before {
        patch.revision += 1;
    }
    world.environment.patches.insert(env::key(p), patch);
    pending
}
