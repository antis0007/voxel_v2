use super::super::factory::Kind;
use super::SPECIES;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;
#[derive(Deserialize)]
pub struct Species {
    pub name: String,
    pub color: [f32; 3],
    pub rise_m_s: f32,
    pub deposition_s: f32,
    pub decay_s: f32,
    pub harm_mg_m3: f32,
}
#[derive(Deserialize)]
pub struct Fuel {
    pub resource: u32,
    pub rates: [u32; SPECIES],
}
#[derive(Deserialize)]
pub struct Emission {
    pub kind: Kind,
    pub rates: [u32; SPECIES],
}
#[derive(Deserialize, Serialize, Debug, PartialEq)]
pub struct SteamTurbine {
    pub steam_mg_per_energy: u32,
    pub max_energy_per_tick: u32,
}

#[derive(Deserialize, Serialize, Debug, PartialEq)]
pub struct FiredBoiler {
    pub steam_mg_per_energy: u32,
    pub max_fuel_per_tick: u32,
}

#[derive(Deserialize)]
pub struct Config {
    pub species: [Species; SPECIES],
    pub emissions: Vec<Emission>,
    pub fuels: Vec<Fuel>,
    pub diffusion_m2_s: f32,
    pub filter_capture_per_mille: u32,
    pub scrubber_capture_per_mille: u32,
    pub rain_capture_s: f32,
    pub repair_cost: Vec<(usize, u32)>,
    pub steam_turbine: SteamTurbine,
    pub fired_boiler: FiredBoiler,
}
pub static CONFIG: LazyLock<Config> = LazyLock::new(|| {
    let c: Config = ron::from_str(include_str!("../../../../../assets/industry.ron"))
        .unwrap_or_else(|e| panic!("assets/industry.ron: {e}"));
    assert!(c.diffusion_m2_s.is_finite() && (0.0..=0.05).contains(&c.diffusion_m2_s));
    assert!(c.filter_capture_per_mille <= 1000 && c.scrubber_capture_per_mille <= 1000);
    for s in &c.species {
        assert!(s.rise_m_s.is_finite() && s.rise_m_s.abs() <= 0.5);
        assert!(
            [s.deposition_s, s.decay_s, s.harm_mg_m3]
                .iter()
                .all(|v| v.is_finite() && *v >= 0.0)
        );
        assert!(
            s.color
                .iter()
                .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
        );
    }
    assert!(c.emissions.iter().all(|e| e.rates.iter().all(|&v| v <= 64)));
    let t = &c.steam_turbine;
    assert!(t.max_energy_per_tick > 0 && t.max_energy_per_tick <= 64);
    // Even a lossless pipe cannot make a self-powering boiler loop.
    assert!(u64::from(t.steam_mg_per_energy) * u64::from(crate::world::factory::industry::PROCESS_POWER)
        > u64::from(crate::world::factory::industry::PROCESS_MG));
    assert!(t.steam_mg_per_energy <= crate::world::factory::industry::GAS_CAPACITY);
    let b = &c.fired_boiler;
    assert!(b.max_fuel_per_tick > 0 && b.max_fuel_per_tick <= 64);
    // Recovered electricity cannot exceed the fuel's paid energy.
    assert!(b.steam_mg_per_energy > 0 && b.steam_mg_per_energy <= t.steam_mg_per_energy);
    assert!(c.fuels.iter().all(|f| f.rates.iter().all(|&v| v <= 64)));
    assert!(crate::world::factory::power::COMBUSTION_FUELS.iter().all(|&(id, _, _)|
        c.fuels.iter().filter(|f| f.resource == id as u32).count() == 1));
    c
});
