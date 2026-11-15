//! Gas handling shares factory placement, inventories and authority.
use super::{DIRS, Factory, Kind, Node, World};
use crate::world::{
    industry::{self, FUEL, OXYGEN, SPECIES, STEAM, State, config::CONFIG},
    inventory::{Item, ItemStack, SurvivalInventory},
};
use glam::IVec3;
mod routing;
pub mod turbine;
pub mod boiler;
pub use routing::route;

pub const GAS_CAPACITY: u32 = 131_072;
pub const WASTE_CAPACITY: u32 = 1_000_000;
pub const FILTER_CAPACITY: u32 = 8192;
pub const WATER_UNIT_MG: u32 = crate::world::effects::WATER_UNIT_UL as u32;
pub const FUEL_CELL_MG: u32 = 512;
pub(crate) const PROCESS_MG: u32 = 1152;
pub(crate) const PROCESS_POWER: u32 = 24;
/// Chemical tank tiers; each step holds four times the gas and moves it four
/// times faster per face. The upgrade is applied in place and keeps the stock.
pub const TANK_TIERS: [&str; 3] = ["Basic", "Advanced", "Elite"];
pub(crate) const PORT_FLOW_MG: u32 = 4096;

fn tank_scale(n: &Node) -> u32 {
    if n.kind == Kind::GasTank { 4u32.pow(n.upgrade.min(2)) } else { 1 }
}

/// Gas a node can hold; only chemical tanks grow with their tier.
pub fn gas_capacity(n: &Node) -> u32 {
    GAS_CAPACITY * tank_scale(n)
}

/// Largest mass one face of this node pushes per gas step.
pub fn port_flow(n: &Node) -> u32 {
    PORT_FLOW_MG * tank_scale(n)
}

pub fn tank_tier_name(tier: u32) -> &'static str {
    TANK_TIERS[tier.min(2) as usize]
}

pub fn processor(n: &Node) -> bool {
    n.kind == Kind::WetScrubber && n.mode != 0
}

pub fn process_water(factory: &mut Factory) {
    let mut spent = 0;
    for n in factory.nodes.values_mut().filter(|n| processor(n)) {
        if !n.enabled || n.ruptured {
            continue;
        }
        let refining = n.mode == 3;
        if n.energy < PROCESS_POWER {
            n.status = "Needs power";
        } else if refining && n.input[super::ETHANOL] < 16 {
            n.status = "Needs bio-ethanol";
        } else if !refining && n.water == 0 && n.separator_water_mg < PROCESS_MG {
            n.status = "Needs water";
        } else if n.exhaust.iter().sum::<u32>() > GAS_CAPACITY - PROCESS_MG {
            n.status = "Gas slots full";
        } else {
            n.energy -= PROCESS_POWER;
            spent += PROCESS_POWER;
            if refining {
                n.input[super::ETHANOL] -= 16;
                n.exhaust[FUEL] += 512;
                n.status = "Refining fuel";
            } else {
                if n.separator_water_mg < PROCESS_MG {
                    n.water -= 1;
                    n.separator_water_mg += WATER_UNIT_MG;
                }
                n.separator_water_mg -= PROCESS_MG;
            }
            if n.mode == 1 {
                n.exhaust[FUEL] += 128;
                n.exhaust[OXYGEN] += PROCESS_MG - 128;
                n.status = "Separating water";
            } else if n.mode == 2 {
                n.exhaust[STEAM] += PROCESS_MG;
                n.status = "Making steam";
            }
            n.activity = 1.0;
        }
    }
    factory.stats.power(0, spent);
}

pub(super) fn replace_bucket(inventory: &mut SurvivalInventory, from: u32, to: u32) -> bool {
    if let Some(stack) = inventory.pack.iter_mut().chain(&mut inventory.hotbar)
        .flatten().find(|s| s.item == Item::Bucket(from) && s.count == 1)
    {
        stack.item = Item::Bucket(to);
        true
    } else {
        false
    }
}

pub fn fill_fuel_cell(n: &mut Node, inventory: &mut SurvivalInventory) -> Result<(), &'static str> {
    if n.exhaust[FUEL] < FUEL_CELL_MG {
        return Err("Needs fuel gas");
    }
    if !replace_bucket(inventory, 0, crate::world::water_tools::FUEL_CELL) {
        return Err("Needs an empty bucket");
    }
    n.exhaust[FUEL] -= FUEL_CELL_MG;
    Ok(())
}

pub(crate) fn use_fuel_cell(inventory: &mut SurvivalInventory) -> bool {
    replace_bucket(inventory, crate::world::water_tools::FUEL_CELL, 0)
}
pub fn component(k: Kind) -> bool {
    matches!(
        k,
        Kind::ExhaustDuct
            | Kind::Chimney
            | Kind::DustFilter
            | Kind::WetScrubber
            | Kind::GasTank
            | Kind::GasGenerator
    )
}
pub fn port(kind: Kind) -> bool {
    component(kind) || CONFIG.emissions.iter().any(|d| d.kind == kind)
}
pub fn blocked(n: &Node) -> bool {
    !turbine::active(n) && !boiler::active(n) && n.exhaust.iter().copied().sum::<u32>() > gas_capacity(n) - 16_384
}
pub fn default_fuel() -> u32 {
    3
}
pub fn produce(n: &mut Node, work: u32) {
    let Some(def) = CONFIG.emissions.iter().find(|d| d.kind == n.kind) else {
        return;
    };
    if n.kind == Kind::Generator && n.mode == 1 {
        return;
    }
    let rates = if n.kind == Kind::Generator {
        CONFIG
            .fuels
            .iter()
            .find(|f| f.resource == n.fuel_resource)
            .map_or(def.rates, |f| f.rates)
    } else {
        def.rates
    };
    for i in 0..SPECIES {
        n.exhaust[i] = n.exhaust[i].saturating_add(rates[i].saturating_mul(work));
    }
}
pub fn rupture(world: &mut World, centre: glam::Vec3, radius: f32) {
    let scale = world.voxel_scale.meters();
    for (&p, n) in &mut world.factory.nodes {
        if component(n.kind)
            && (p.as_vec3() * scale + glam::Vec3::splat(0.5)).distance(centre)
                <= radius.max(0.0) + 0.87
        {
            n.ruptured = true;
            n.gas_vent = true;
            n.enabled = true;
            n.status = "Leaking";
        }
    }
}

pub fn burn_gas(factory: &mut Factory) {
    for n in factory
        .nodes
        .values_mut()
        .filter(|n| n.kind == Kind::GasGenerator)
    {
        if !n.enabled || n.ruptured || blocked(n) {
            continue;
        }
        if turbine::active(n) {
            turbine::generate(n);
            continue;
        }
        let fuel = n.exhaust[3]
            .min(128)
            .min(super::power::GENERATOR_BUFFER.saturating_sub(n.energy) * 8);
        let fuel = fuel / 8 * 8;
        if fuel == 0 {
            n.status = "Needs fuel gas";
            continue;
        }
        n.exhaust[3] -= fuel;
        n.exhaust[1] += fuel / 32;
        n.exhaust[4] += fuel / 8;
        n.energy += fuel / 8;
        n.flow = (fuel / 8) as i32;
        n.status = "Generating";
    }
}

pub fn pack_waste(n: &mut Node, inventory: &mut SurvivalInventory) {
    for i in 0..SPECIES {
        if n.residue[i] == 0 {
            continue;
        }
        let count = n.residue[i].div_ceil(1000);
        let (inserted, _) = inventory.try_insert(ItemStack::new(
            Item::Block(crate::world::BlockId::IndustrialWaste),
            count,
        ));
        n.residue[i] = n.residue[i].saturating_sub(inserted * 1000);
    }
}

pub fn clean(world: &mut World, p: IVec3, player: glam::Vec3) {
    let scale = world.voxel_scale.meters();
    if ((p.as_vec3() - player) * scale).length() > 7.0 {
        return;
    }
    let keys: Vec<_> = world
        .industry
        .deposits
        .keys()
        .copied()
        .filter(|k| ((IVec3::new(k.0, k.1, k.2) - p).as_vec3() * scale).length() <= 3.0)
        .collect();
    for k in keys {
        let Some(m) = world.industry.deposits.get_mut(&k) else {
            continue;
        };
        for i in 0..SPECIES {
            if m[i] == 0 {
                continue;
            }
            let count = m[i].div_ceil(1000).min(64) as u32;
            let (n, _) = world.inventory.try_insert(ItemStack::new(
                Item::Block(crate::world::BlockId::IndustrialWaste),
                count,
            ));
            let removed = u64::from(n) * 1000;
            let actual = m[i].min(removed);
            m[i] -= actual;
            world.industry.packed[i] += actual;
        }
    }
    world
        .industry
        .deposits
        .retain(|_, m| industry::total(m) > 0);
}

pub(crate) fn capture(n: &mut Node) {
    if n.ruptured {
        n.status = "Leaking";
        return;
    }
    if !n.enabled {
        n.status = "Paused";
        return;
    }
    if processor(n) {
        return;
    }
    let filter = n.kind == Kind::DustFilter;
    if !filter && n.kind != Kind::WetScrubber {
        return;
    }
    if n.residue.iter().sum::<u32>() >= WASTE_CAPACITY {
        n.status = "Waste full";
        return;
    }
    if filter && n.filter_credit == 0 && n.input[4] >= 16 {
        n.input[4] -= 16;
        n.filter_credit = FILTER_CAPACITY;
    }
    if (filter && n.filter_credit == 0) || (!filter && (n.water == 0 || n.energy < 1)) {
        n.status = if filter {
            "Needs filter media"
        } else {
            "Needs water / power"
        };
        return;
    }
    let efficiency = if filter {
        CONFIG.filter_capture_per_mille
    } else {
        CONFIG.scrubber_capture_per_mille
    };
    let mut room = WASTE_CAPACITY - n.residue.iter().sum::<u32>();
    let mut used = 0;
    let mut treatment = if filter {
        n.filter_credit
    } else {
        (n.water * 64).min(4096)
    };
    for i in 0..3 {
        if filter && i == 2 {
            continue;
        }
        let amount = (n.exhaust[i] * efficiency / 1000).min(room).min(treatment);
        treatment -= amount;
        n.exhaust[i] -= amount;
        n.residue[i] += amount;
        room -= amount;
        used += amount;
        if filter {
            n.filter_credit -= amount;
        }
    }
    if used > 0 {
        n.status = "Filtering";
        n.activity = 1.0;
        if !filter {
            n.water -= used.div_ceil(64);
            n.energy -= 1;
        }
    } else {
        n.status = "Ready";
    }
}

pub(crate) fn outlet(world: &World, p: IVec3) -> Option<IVec3> {
    [0, 1, 2, 4, 5].into_iter().find_map(|side| face_outlet(world, p, side))
}

pub(crate) fn face_outlet(world: &World, p: IVec3, side: usize) -> Option<IVec3> {
    let size = super::step(world);
    let direction = DIRS[side];
    let face = p + (IVec3::ONE - direction.abs()) * (size / 2)
        + direction.max(IVec3::ZERO) * size + direction.min(IVec3::ZERO);
    industry::geometry::open(world, face).then_some(face)
}
