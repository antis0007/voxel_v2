//! Fuel-fired steam uses paid heat and exact liquid mass.
use super::{GAS_CAPACITY, Kind, Node, WATER_UNIT_MG};
use crate::world::{factory::power::COMBUSTION_FUELS, industry::{SPECIES, STEAM, config::CONFIG}};

pub const MODE: u32 = 5;

pub fn active(n: &Node) -> bool { n.kind == Kind::Generator && n.mode == MODE }

fn rates(fuel: u32) -> [u32; SPECIES] {
    let mut rates = CONFIG.fuels.iter().find(|f| f.resource == fuel)
        .map_or([0; SPECIES], |f| f.rates);
    // Water supplies all boiler steam; fuel still produces its other pollutants.
    rates[STEAM] = CONFIG.fired_boiler.steam_mg_per_energy;
    rates
}

pub(in crate::world::factory) fn step(n: &mut Node) {
    n.flow = 0;
    if !n.enabled { n.status = "Paused"; return; }
    let b = &CONFIG.fired_boiler;
    let unit = u64::from(WATER_UNIT_MG);
    let mass = u64::from(n.water) * unit + u64::from(n.separator_water_mg);
    if mass < u64::from(b.steam_mg_per_energy) { n.status = "Needs Water"; return; }
    let fuel = if n.burn > 0 { Some((n.fuel_resource as usize, 0, n.burn)) }
        else { COMBUSTION_FUELS.iter().copied().find(|&(id, portion, _)| n.input[id] >= portion) };
    let Some((id, portion, credit)) = fuel else { n.status = "Needs Fuel"; return; };
    let rates = rates(id as u32);
    let room = GAS_CAPACITY.saturating_sub(n.exhaust.iter().sum());
    let work = b.max_fuel_per_tick.min(credit).min(room / rates.iter().sum::<u32>())
        .min((mass / u64::from(b.steam_mg_per_energy)).min(u64::from(u32::MAX)) as u32);
    if work == 0 { n.status = "Exhaust Full"; return; }
    n.input[id] -= portion;
    n.fuel_resource = id as u32;
    n.burn = credit - work;
    let remaining = mass - u64::from(work) * u64::from(b.steam_mg_per_energy);
    n.water = (remaining / unit) as u32;
    n.separator_water_mg = (remaining % unit) as u32;
    for (stored, rate) in n.exhaust.iter_mut().zip(rates) { *stored += work * rate; }
    n.activity = 1.0;
    n.status = "Making Steam";
}

#[cfg(test)]
mod tests;
