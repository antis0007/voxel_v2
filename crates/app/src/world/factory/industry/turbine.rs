//! Steam conversion retains exact liquid mass and paid energy.
use super::{Kind, Node, WATER_UNIT_MG, replace_bucket};
use crate::world::{factory::{extraction::WATER_CAPACITY, power::GENERATOR_BUFFER},
    industry::{STEAM, config::CONFIG}, inventory::SurvivalInventory, water_tools};

pub fn active(n: &Node) -> bool {
    n.kind == Kind::GasGenerator && n.mode == 1
}

pub fn condensate(n: &Node) -> u64 {
    u64::from(n.water) * u64::from(WATER_UNIT_MG) + u64::from(n.separator_water_mg)
}

fn store(n: &mut Node, mass: u64) {
    n.water = (mass / u64::from(WATER_UNIT_MG)) as u32;
    n.separator_water_mg = (mass % u64::from(WATER_UNIT_MG)) as u32;
}

pub(super) fn generate(n: &mut Node) {
    let t = &CONFIG.steam_turbine;
    let mass = condensate(n);
    let capacity = u64::from(WATER_CAPACITY) * u64::from(WATER_UNIT_MG);
    let room = capacity.saturating_sub(mass) / u64::from(t.steam_mg_per_energy);
    let energy = t.max_energy_per_tick.min(n.exhaust[STEAM] / t.steam_mg_per_energy)
        .min(GENERATOR_BUFFER.saturating_sub(n.energy)).min(room.min(u64::from(u32::MAX)) as u32);
    if energy == 0 {
        n.status = if n.energy >= GENERATOR_BUFFER { "Power Full" }
            else if room == 0 { "Condensate Full" } else { "No Steam" };
        return;
    }
    let used = energy * t.steam_mg_per_energy;
    n.exhaust[STEAM] -= used;
    store(n, mass + u64::from(used));
    n.energy += energy;
    n.flow = energy as i32;
    n.activity = 1.0;
    n.status = "Generating";
}

pub fn bottle(n: &mut Node, inventory: &mut SurvivalInventory) -> Result<(), &'static str> {
    let mass = condensate(n);
    if mass == 0 { return Ok(()); }
    let amount = mass.min(u64::from(water_tools::CAPACITY_UL)) as u32;
    if !replace_bucket(inventory, 0, amount) { return Err("Empty Bucket Required"); }
    store(n, mass - u64::from(amount));
    n.status = "Bottled";
    Ok(())
}

#[cfg(test)]
mod tests;
