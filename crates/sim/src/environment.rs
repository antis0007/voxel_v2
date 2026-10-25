//! Conservative, property-driven surface processes. The caller admits both
//! owners and commits geometry/stock changes together. No world/block lookup,
//! camera, clock, content names, or random global state belongs here.
use engine_core::{
    VoxelScalePreset,
    environment::{SoilLaw, cell_volume, face_area_cm2, surface_flux},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Soil {
    pub water: u64,
    /// Potential infiltration; this is time credit, NOT an inventory of water.
    pub infiltration_credit: u64,
    pub flux_carry: u64,
    pub erosion_work: u64,
    pub nutrient: u64,
    pub biomass: u64,
    pub plant_nutrient: u64,
    pub litter: u64,
    pub litter_nutrient: u64,
    /// Potential growth in 1/10,000 mg, capped below one nutrient packet.
    /// It prevents fine groundcover from rounding every growth step to zero.
    pub growth_credit: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Exchange {
    pub infiltrated: u64,
    pub evaporated: u64,
    pub transpired: u64,
    pub atmospheric_biomass: u64,
    pub decomposed: u64,
}

/// Grain stocks share a cell but have distinct transport behavior. Resting
/// grains remain in a settling area until admitted hydraulic work lifts them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Sediment {
    pub suspended: u64,
    pub bed: u64,
}
impl From<u64> for Sediment {
    fn from(suspended: u64) -> Self {
        Self { suspended, bed: 0 }
    }
}
impl Sediment {
    pub fn mass(self) -> u64 {
        self.suspended + self.bed
    }
    pub fn remove(&mut self, amount: u64) {
        let bed = amount.min(self.bed);
        self.bed -= bed;
        self.suspended -= (amount - bed).min(self.suspended);
    }
    pub fn settle(&mut self, depth_um: u64, speed_um_s: u32) -> u64 {
        let moved = (self.suspended * u64::from(speed_um_s) / depth_um.max(1))
            .max(u64::from(self.suspended > 0))
            .min(self.suspended);
        self.suspended -= moved;
        self.bed += moved;
        moved
    }
    /// mJ / (kJ/kg) = mg; returned work is consumed once by entrainment.
    pub fn entrain(&mut self, work_mj: u64, resistance_kj_kg: u64) -> u64 {
        let resistance = resistance_kj_kg.max(1);
        let moved = self.bed.min(work_mj / resistance);
        self.bed -= moved;
        self.suspended += moved;
        moved * resistance
    }
}

/// One-second solve; fluxes scale with face area, storage with physical volume.
/// Quantisation retains all uncommitted water at the fluid source. Saturation
/// cannot bank an unlimited future demand.
pub fn infiltrate(soil: &mut Soil, law: SoilLaw, scale: VoxelScalePreset, available: u64) -> u64 {
    let quantum = cell_volume(scale) / 8;
    let capacity = law.capacity(scale);
    let flux = surface_flux(law.infiltration_um_s, face_area_cm2(scale), 1, &mut soil.flux_carry);
    soil.infiltration_credit = (soil.infiltration_credit + flux).min(capacity);
    let moved =
        available.min(capacity.saturating_sub(soil.water)).min(soil.infiltration_credit) / quantum * quantum;
    soil.water += moved;
    soil.infiltration_credit -= moved;
    if available == 0 || soil.water >= capacity {
        soil.infiltration_credit = 0;
    }
    moved
}

/// Contact across an actual porous face, with a common elevation or downward
/// gravity. Call only for connected substrates; a visual wetness never counts.
pub fn drain(
    a: &mut Soil,
    b: &mut Soil,
    a_law: SoilLaw,
    b_law: SoilLaw,
    scale: VoxelScalePreset,
    downward: bool,
) -> u64 {
    drain_between(a, b, a_law, b_law, scale, scale, face_area_cm2(scale), downward)
}

pub fn drain_between(
    a: &mut Soil,
    b: &mut Soil,
    a_law: SoilLaw,
    b_law: SoilLaw,
    a_scale: VoxelScalePreset,
    b_scale: VoxelScalePreset,
    contact_cm2: u64,
    downward: bool,
) -> u64 {
    drain_over(a, b, a_law, b_law, a_scale, b_scale, contact_cm2, downward, 1)
}

/// Bounded soil integration interval, independent of fluid/render resolution.
pub fn drain_over(
    a: &mut Soil,
    b: &mut Soil,
    a_law: SoilLaw,
    b_law: SoilLaw,
    a_scale: VoxelScalePreset,
    b_scale: VoxelScalePreset,
    contact_cm2: u64,
    downward: bool,
    seconds: u32,
) -> u64 {
    let ac = a_law.capacity(a_scale);
    let bc = b_law.capacity(b_scale);
    if ac == 0 || bc == 0 {
        return 0;
    }
    let pressure = if downward {
        a.water.saturating_sub(a_law.retained_water(a_scale)) / 4
    } else {
        let equilibrium = ((a.water + b.water) as u128 * ac as u128 / (ac + bc) as u128) as u64;
        a.water.saturating_sub(equilibrium) / 2
    };
    let rate =
        surface_flux(a_law.drainage_um_s.min(b_law.drainage_um_s), contact_cm2, seconds, &mut a.flux_carry);
    let moved = pressure.min(rate).min(bc.saturating_sub(b.water));
    a.water -= moved;
    b.water += moved;
    moved
}

/// Exact proportional transport with a full-drain fast path. A failed
/// admission calls neither this function nor the water commit.
pub fn advected_stock(stock: u64, source_water: u8, moved_water: u8) -> u64 {
    advected_quantity(stock, u64::from(source_water), u64::from(moved_water))
}
pub fn advected_quantity(stock: u64, source_water: u64, moved_water: u64) -> u64 {
    if source_water == 0 {
        return 0;
    }
    ((stock as u128 * u128::from(moved_water.min(source_water))) / u128::from(source_water)) as u64
}

/// Continuous surface exchange. The admitted free-water owner supports exact
/// microlitres, so a soil need not wait hundreds of seconds for an eighth-cell.
pub fn infiltrate_exact(soil: &mut Soil, law: SoilLaw, scale: VoxelScalePreset, available: u64) -> u64 {
    infiltrate_over(soil, law, scale, available, 1)
}
pub fn infiltrate_over(
    soil: &mut Soil,
    law: SoilLaw,
    scale: VoxelScalePreset,
    available: u64,
    seconds: u32,
) -> u64 {
    let flux = surface_flux(law.infiltration_um_s, face_area_cm2(scale), seconds, &mut soil.flux_carry);
    let moved = available.min(law.capacity(scale).saturating_sub(soil.water)).min(flux);
    soil.water += moved;
    moved
}

#[derive(Clone, Copy, Debug)]
pub struct PlantLaw {
    pub moisture: [u32; 2],
    pub temperature_c: [i32; 2],
    pub shade_tolerance: u32,
    pub growth_mg_m2_s: u32,
    pub water_ul_per_mg: u32,
    pub mature_mg_m2: u32,
}

/// Low vegetation is a cohort. Only biomass thresholds should alter geometry.
/// Nutrients are conserved through uptake, mortality and decomposition;
/// carbon uptake/respiration and atmospheric water are explicit exchanges.
pub fn vegetation(
    soil: &mut Soil,
    plant: PlantLaw,
    law: SoilLaw,
    scale: VoxelScalePreset,
    temperature_c: i32,
    shade: u32,
    biological_rate: u32,
) -> Exchange {
    let mut exchange = Exchange::default();
    let area = face_area_cm2(scale);
    let capacity = law.capacity(scale).max(1);
    let moisture = (soil.water * 1_000 / capacity) as u32;
    let viable = (plant.moisture[0]..=plant.moisture[1]).contains(&moisture)
        && (plant.temperature_c[0]..=plant.temperature_c[1]).contains(&temperature_c)
        && shade <= plant.shade_tolerance;
    let mature = u64::from(plant.mature_mg_m2) * area / 10_000;
    if viable {
        let potential =
            u64::from(plant.growth_mg_m2_s) * area * u64::from(biological_rate) + soil.growth_credit;
        // Nutrient packets prevent rounding from minting nutrient-free biomass.
        let growth = (potential / 10_000)
            .min(mature.saturating_sub(soil.biomass))
            .min(soil.nutrient * 40)
            .min(soil.water / u64::from(plant.water_ul_per_mg.max(1)))
            / 40
            * 40;
        soil.growth_credit = potential % 400_000;
        let water = growth * u64::from(plant.water_ul_per_mg);
        soil.water -= water;
        soil.nutrient -= growth / 40;
        soil.plant_nutrient += growth / 40;
        soil.biomass += growth;
        exchange.transpired = water;
        exchange.atmospheric_biomass = growth - growth / 40;
    } else if soil.biomass > 0 {
        soil.growth_credit = 0;
        let dead = (soil.biomass * u64::from(biological_rate) / 86_400).max(1).min(soil.biomass);
        let nutrient = if dead == soil.biomass {
            soil.plant_nutrient
        } else {
            soil.plant_nutrient * dead / soil.biomass
        };
        soil.biomass -= dead;
        soil.plant_nutrient -= nutrient;
        soil.litter += dead;
        soil.litter_nutrient += nutrient;
    }
    if temperature_c > 0 && soil.water > 0 && soil.litter > 0 {
        let decay = (soil.litter * u64::from(biological_rate) / 259_200).max(1).min(soil.litter);
        let nutrient = if decay == soil.litter {
            soil.litter_nutrient
        } else {
            soil.litter_nutrient * decay / soil.litter
        };
        soil.litter -= decay;
        soil.litter_nutrient -= nutrient;
        soil.nutrient += nutrient;
        exchange.decomposed = decay - nutrient;
    }
    exchange
}

#[cfg(test)]
#[path = "environment_tests.rs"]
mod tests;
