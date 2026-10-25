//! Validated ecological content. No per-cell allocation or runtime file IO.
use crate::BlockId;
use engine_core::environment::SoilLaw;
use std::sync::LazyLock;

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct Substrate {
    pub material: BlockId,
    pub law: SoilLaw,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct Species {
    pub id: u32,
    pub name: String,
    pub cover: BlockId,
    pub moisture_min: u32,
    pub moisture_max: u32,
    pub temperature_min_c: i32,
    pub temperature_max_c: i32,
    pub shade_tolerance: u32,
    pub growth_mg_m2_s: u32,
    pub water_ul_per_mg: u32,
    pub mature_mg_m2: u32,
    pub root_resistance_kj_kg: u32,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct OrganicInput {
    pub material: BlockId,
    pub dry_mg_m3: u32,
    pub nutrient_per_mille: u32,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct Definition {
    pub model: u32,
    pub biological_rate: u32,
    pub erosion_rate: u32,
    pub evaporation_um_s: u32,
    pub substrates: Vec<Substrate>,
    pub species: Vec<Species>,
    pub organic_inputs: Vec<OrganicInput>,
}

impl Definition {
    pub fn validate(&self) -> Result<(), String> {
        if self.model != engine_core::environment::ENVIRONMENT_MODEL
            || !(1..=3_600).contains(&self.biological_rate)
            || !(1..=3_600).contains(&self.erosion_rate)
            || self.evaporation_um_s > 10
        {
            return Err("Invalid environmental model/rates".into());
        }
        for (i, substrate) in self.substrates.iter().enumerate() {
            let law = substrate.law;
            if self.substrates[..i].iter().any(|s| s.material == substrate.material)
                || law.porosity_per_mille > 900
                || law.retention_per_mille > 1_000
                || law.density_kg_m3 == 0
                || law.density_kg_m3 > 10_000
                || law.erosion_resistance_kj_kg == 0
                || law.infiltration_um_s > 10_000
                || law.drainage_um_s > 10_000
                || u64::from(law.density_kg_m3) * 1_000
                    > engine_core::environment::SEDIMENT_GRAIN_KG_M3
                        * u64::from(1_000 - law.porosity_per_mille.min(1_000))
            {
                return Err("Invalid or repeated substrate".into());
            }
        }
        for (i, species) in self.species.iter().enumerate() {
            if species.id == 0
                || self.species[..i].iter().any(|s| s.id == species.id)
                || species.moisture_min >= species.moisture_max
                || species.moisture_max > 1_000
                || species.temperature_min_c >= species.temperature_max_c
                || species.water_ul_per_mg == 0
                || species.mature_mg_m2 == 0
                || species.shade_tolerance > 1_000
                || species.growth_mg_m2_s > 100_000
            {
                return Err("Invalid or repeated species".into());
            }
        }
        for (i, input) in self.organic_inputs.iter().enumerate() {
            if input.dry_mg_m3 == 0
                || input.dry_mg_m3 > 1_000_000_000
                || input.nutrient_per_mille > 1_000
                || self.organic_inputs[..i].iter().any(|other| other.material == input.material)
            {
                return Err("Invalid or repeated compost material".into());
            }
        }
        Ok(())
    }
    pub fn soil(&self, material: BlockId) -> Option<SoilLaw> {
        self.substrates.iter().find(|s| s.material == material).map(|s| s.law)
    }
    pub fn species(&self, id: u32) -> Option<&Species> {
        self.species.iter().find(|s| s.id == id)
    }
}

// Fail closed. Tests report the actual parse/validation error; a broken pack
// cannot quietly enable a different model or corrupt an existing world.
pub static DEFINITIONS: LazyLock<Result<Definition, String>> = LazyLock::new(|| {
    let definition: Definition =
        ron::from_str(include_str!("../../../assets/environment.ron")).map_err(|e| e.to_string())?;
    definition.validate()?;
    Ok(definition)
});

pub fn definition() -> Option<&'static Definition> {
    DEFINITIONS.as_ref().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn environment_content_round_trips() {
        let definition = DEFINITIONS.as_ref().unwrap();
        let encoded = ron::to_string(definition).unwrap();
        let decoded: Definition = ron::from_str(&encoded).unwrap();
        decoded.validate().unwrap();
        assert_eq!(decoded.species.len(), definition.species.len());
        let dirt = definition.soil(BlockId::Dirt).unwrap();
        for cover in [BlockId::Grass, BlockId::SnowGrass] {
            let covered = definition.soil(cover).unwrap();
            assert_eq!(covered.density_kg_m3, dirt.density_kg_m3);
            assert_eq!(covered.porosity_per_mille, dirt.porosity_per_mille);
            assert_eq!(covered.retention_per_mille, dirt.retention_per_mille);
        }
    }
}
