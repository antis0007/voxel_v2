//! Save-format 31 extension. All pending work and fractional credits persist.
use super::*;
use anyhow::{Result, ensure};
use engine_core::environment::ENVIRONMENT_MODEL;

fn put(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}
fn put64(out: &mut Vec<u8>, value: u64) {
    put(out, value as u32);
    put(out, (value >> 32) as u32);
}
fn get64(read: &mut impl FnMut() -> Result<u32>) -> Result<u64> {
    Ok(u64::from(read()?) | (u64::from(read()?) << 32))
}
fn put_key(out: &mut Vec<u8>, k: Key) {
    for v in [k.0, k.1, k.2] {
        put(out, v as u32);
    }
}
fn get_key(read: &mut impl FnMut() -> Result<u32>) -> Result<Key> {
    Ok((read()? as i32, read()? as i32, read()? as i32))
}

impl State {
    pub fn encode(&self, out: &mut Vec<u8>) {
        put(out, ENVIRONMENT_MODEL);
        for rate in [self.biological_rate, self.erosion_rate, self.evaporation_um_s] {
            put(out, rate);
        }
        for v in [
            self.tick,
            self.target_tick,
            self.fixed_steps,
            self.ledger.initial_soil_water,
            self.ledger.evaporated_water,
            self.ledger.transpired_water,
            self.ledger.atmospheric_biomass,
            self.ledger.respired_biomass,
            self.ledger.eroded_mg,
            self.ledger.deposited_mg,
            self.ledger.removed_sediment_mg,
            self.ledger.imported_litter_mg,
            self.ledger.imported_litter_nutrient_mg,
            self.ledger.edited_water_added_ul,
            self.ledger.edited_water_removed_ul,
        ] {
            put64(out, v);
        }
        put(out, self.patches.len() as u32);
        for (&p, patch) in &self.patches {
            put_key(out, p);
            put(out, patch.material as u32);
            put(out, u32::from(patch.extent));
            put(out, patch.species);
            put64(out, patch.revision);
            put64(out, patch.last_tick);
            let s = patch.soil;
            for v in [
                s.water,
                s.infiltration_credit,
                s.flux_carry,
                s.erosion_work,
                s.nutrient,
                s.biomass,
                s.plant_nutrient,
                s.litter,
                s.litter_nutrient,
                s.growth_credit,
            ] {
                put64(out, v);
            }
        }
        put(out, self.sediment.len() as u32);
        for (&p, stock) in &self.sediment {
            put_key(out, p);
            put64(out, stock.suspended);
            put64(out, stock.bed);
        }
        put(out, self.hydraulic_work.len() as u32);
        for (&p, &work) in &self.hydraulic_work {
            put_key(out, p);
            put64(out, work);
        }
        put(out, self.water_deficit.len() as u32);
        let mut fractions: Vec<_> = self.water_deficit.iter().collect();
        fractions.sort_unstable_by_key(|(p, _)| **p);
        for (&p, &deficit) in fractions {
            put_key(out, p);
            put64(out, deficit);
        }
        put(out, self.due.len() as u32);
        for &(due, p) in &self.due {
            put_key(out, p);
            put64(out, due);
        }
    }

    pub fn decode(
        read: &mut impl FnMut() -> Result<u32>,
        scale: crate::fluid::VoxelScalePreset,
    ) -> Result<Self> {
        ensure!(read()? == ENVIRONMENT_MODEL, "Unsupported environmental model");
        let mut state = Self::default();
        state.cells_per_metre = (1.0 / scale.meters()) as i32;
        state.biological_rate = read()?;
        state.erosion_rate = read()?;
        state.evaporation_um_s = read()?;
        ensure!(
            (1..=3_600).contains(&state.biological_rate)
                && (1..=3_600).contains(&state.erosion_rate)
                && state.evaporation_um_s <= 10,
            "Invalid saved process rates"
        );
        state.tick = get64(read)?;
        state.target_tick = get64(read)?;
        state.fixed_steps = get64(read)?;
        ensure!(
            state.tick <= state.target_tick && state.target_tick <= state.fixed_steps / 120,
            "Invalid environmental clock"
        );
        state.ledger = Ledger {
            initial_soil_water: get64(read)?,
            evaporated_water: get64(read)?,
            transpired_water: get64(read)?,
            atmospheric_biomass: get64(read)?,
            respired_biomass: get64(read)?,
            eroded_mg: get64(read)?,
            deposited_mg: get64(read)?,
            removed_sediment_mg: get64(read)?,
            imported_litter_mg: get64(read)?,
            imported_litter_nutrient_mg: get64(read)?,
            edited_water_added_ul: get64(read)?,
            edited_water_removed_ul: get64(read)?,
        };
        let count = read()? as usize;
        ensure!(count <= MAX_PATCHES, "Too many soil patches");
        let definition = engine_content::environment::definition()
            .ok_or_else(|| anyhow::anyhow!("Environmental content unavailable"))?;
        let volume = engine_core::environment::cell_volume(scale);
        for _ in 0..count {
            let p = get_key(read)?;
            let material_id = read()?;
            let material = BlockId::from_u32(material_id);
            ensure!(
                material as u32 == material_id && definition.soil(material).is_some(),
                "Invalid soil material"
            );
            let extent = read()?;
            ensure!(extent == 1 || extent == state.cells_per_metre as u32, "Invalid patch bounds");
            if extent > 1 {
                ensure!(
                    [p.0, p.1, p.2].iter().all(|v| v.rem_euclid(extent as i32) == 0),
                    "Unaligned soil patch"
                );
            }
            let species = read()?;
            ensure!(species == 0 || definition.species(species).is_some(), "Invalid plant species");
            let revision = get64(read)?;
            let last_tick = get64(read)?;
            ensure!(last_tick <= state.target_tick.max(state.tick + 1), "Invalid patch time");
            let soil = Soil {
                water: get64(read)?,
                infiltration_credit: get64(read)?,
                flux_carry: get64(read)?,
                erosion_work: get64(read)?,
                nutrient: get64(read)?,
                biomass: get64(read)?,
                plant_nutrient: get64(read)?,
                litter: get64(read)?,
                litter_nutrient: get64(read)?,
                growth_credit: get64(read)?,
            };
            let capacity = volume * u64::from(extent).pow(3);
            ensure!(soil.growth_credit < 400_000, "Invalid plant growth credit");
            ensure!(
                soil.plant_nutrient <= soil.biomass && soil.litter_nutrient <= soil.litter,
                "Nutrient tracer exceeds its biological stock"
            );
            ensure!(
                soil.water <= capacity && soil.infiltration_credit <= capacity && soil.flux_carry < 10,
                "Invalid soil water/credit"
            );
            ensure!(
                [
                    soil.nutrient,
                    soil.biomass,
                    soil.plant_nutrient,
                    soil.litter,
                    soil.litter_nutrient,
                    soil.erosion_work
                ]
                .iter()
                .all(|&v| v <= 1_000_000_000_000),
                "Invalid soil stock"
            );
            ensure!(
                state
                    .patches
                    .insert(p, Patch { material, species, revision, last_tick, soil, extent: extent as u8 })
                    .is_none(),
                "Duplicate soil owner"
            );
        }
        for (&p, patch) in &state.patches {
            ensure!(patch.extent > 1 || state.owner(position(p)) == p, "Overlapping soil owners");
        }
        let count = read()? as usize;
        ensure!(count <= MAX_SEDIMENT, "Too many sediment cells");
        for _ in 0..count {
            let p = get_key(read)?;
            let stock = Sediment { suspended: get64(read)?, bed: get64(read)? };
            ensure!(
                stock.suspended <= 1_000_000_000_000
                    && stock.bed <= 1_000_000_000_000
                    && stock.mass() > 0
                    && stock.mass() <= 1_000_000_000_000,
                "Invalid sediment stock"
            );
            ensure!(state.sediment.insert(p, stock).is_none(), "Duplicate sediment owner");
        }
        let count = read()? as usize;
        ensure!(count <= MAX_EVENTS, "Too many hydraulic work records");
        for _ in 0..count {
            let p = get_key(read)?;
            let work = get64(read)?;
            ensure!(work > 0 && work <= 1_000_000_000_000_000, "Invalid hydraulic work");
            ensure!(state.hydraulic_work.insert(p, work).is_none(), "Duplicate hydraulic work owner");
        }
        let count = read()? as usize;
        ensure!(count <= MAX_FRACTIONS, "Too many fractional fluid cells");
        for _ in 0..count {
            let p = get_key(read)?;
            let deficit = get64(read)?;
            ensure!(deficit > 0 && deficit < state.quantum(), "Invalid fluid fraction");
            ensure!(state.water_deficit.insert(p, deficit).is_none(), "Duplicate fluid fraction");
        }
        let count = read()? as usize;
        ensure!(count <= MAX_EVENTS, "Too many environmental events");
        for _ in 0..count {
            let p = get_key(read)?;
            let due = get64(read)?;
            ensure!(due > state.tick && !state.scheduled.contains_key(&p), "Invalid environmental schedule");
            ensure!(state.schedule(p, due), "Environmental schedule capacity");
        }
        Ok(state)
    }
}
