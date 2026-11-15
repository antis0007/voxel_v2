//! Factory persistence boundary: historical layout widths, network array
//! compatibility and binary codecs. Gameplay owns the live Factory; this module
//! only translates that state without changing historical save or format gates.
use super::{
    CAPACITY, COKE, CREOSOTE, ETHANOL, Factory, GOLD_CONCENTRATE, GOLD_INGOT, Kind, MAX_NODES,
    Node, RESOURCE_COUNT, SPECIES, anchor, assembler, biomatter, blocks, extraction, fuel_order,
    induction, industry, multiblock, pile, placer, power, quarry, recipes, sides, stock_order,
};
use glam::IVec3;

#[cfg(feature = "multiplayer")]
mod network;
#[cfg(feature = "multiplayer")]
pub(crate) use network::read_resources;
#[cfg(feature = "multiplayer")]
pub(super) use network::{read_counts, read_gas_counts};

/// Layout version `Factory::encode` writes for current saves.
pub const FACTORY_FORMAT: u8 = 21;

pub(super) const PRE_PLACER_RESOURCES: usize = 22;
const PLACER_RESOURCES: usize = 23;
/// Resources from save version 63 (gold ingots) until feathers.
const INGOT_RESOURCES: usize = 24;
/// Resources that existed before save version 20 (coke, creosote and
/// bio-ethanol were appended after them).
pub const LEGACY_RESOURCE_COUNT: usize = 19;
pub fn resources_in_save(version: u8) -> usize {
    resources_in_format(match version {
        66.. => 20,
        63..=65 => 18,
        61..=62 => 17,
        20..=60 => 9,
        _ => 8,
    })
}

// Factory format and world save version are distinct numbering schemes.
// Every binary reader/writer resolves its resource width through this owner.
fn resources_in_format(format: u8) -> usize {
    match format {
        20.. => RESOURCE_COUNT,
        18..=19 => INGOT_RESOURCES,
        17 => PLACER_RESOURCES,
        9..=16 => PRE_PLACER_RESOURCES,
        7..=8 => LEGACY_RESOURCE_COUNT,
        _ => 14,
    }
}

const LEGACY_GAS_SPECIES: usize = 5;
fn gas_species_in_format(format: u8) -> usize {
    if format >= 21 {
        SPECIES
    } else {
        LEGACY_GAS_SPECIES
    }
}

impl Kind {
    /// Number of kinds a factory layout version stores.
    pub fn count_in_format(format: u8) -> usize {
        match format {
            16.. => Self::COUNT,
            14..=15 => 39,
            12..=13 => 33,
            10..=11 => 27,
            9 => 21,
            8 => Self::LEGACY_COUNT,
            7 => 11,
            _ => 8,
        }
    }
}

impl Factory {
    #[cfg(test)]
    pub fn encode(&self, bytes: &mut Vec<u8>) {
        self.encode_format(bytes, FACTORY_FORMAT);
    }
    /// Writes an older layout (format 8: 16 kinds, 19 resources). Saves never
    /// go backwards; this exists so tests can build genuine legacy files.
    pub fn encode_format(&self, bytes: &mut Vec<u8>, format: u8) {
        let kinds = Kind::count_in_format(format);
        // The existing fixture writer emits the extended layout even for a
        // lower requested format. Only decoding supports pre-8 layouts.
        let resources = resources_in_format(format.max(8));
        let mut put = |n: u32| bytes.extend_from_slice(&n.to_le_bytes());
        for &n in &self.kits[..kinds] {
            put(n);
        }
        put(self.nodes.len() as u32);
        let mut nodes: Vec<_> = self.nodes.iter().collect();
        nodes.sort_by_key(|(p, _)| (p.x, p.y, p.z));
        for (p, n) in nodes {
            for v in p.to_array() {
                put(v as u32);
            }
            put(n.kind as u32);
            for (i, v) in n.input.into_iter().enumerate().take(resources) {
                // Items still in a pipe are saved as delivered: capacity was
                // reserved at launch, so this always fits.
                put(v + self.reserved.get(&(*p, i)).copied().unwrap_or(0));
            }
            for v in n.output.into_iter().take(resources) {
                put(v);
            }
            for v in [
                n.energy,
                n.burn,
                n.progress,
                n.recipe.map_or(0, |i| i as u32 + 1),
                u32::from(n.enabled),
            ] {
                put(v);
            }
            for v in [
                n.mode,
                n.upgrade,
                n.filter.map_or(0, |i| i as u32 + 1),
                n.water,
                n.excavation,
            ] {
                put(v);
            }
            if format >= 10 {
                // Reserved shipments are saved in destination stock, so their
                // receipts must be reflected in the corresponding saved order.
                put(fuel_order::saved_target(self, *p, n));
                put(n.stock);
            }
            if format >= 11 {
                blocks::encode(n, &mut put, format);
            }
            if format >= 13 {
                put(n.faces);
            }
            if format >= 14 {
                let species = gas_species_in_format(format);
                for v in n.exhaust[..species].iter().chain(&n.residue[..species]) {
                    put(*v);
                }
                put(n.filter_credit);
                put(u32::from(n.gas_vent));
                put(n.fuel_resource);
                put(u32::from(n.ruptured));
                if format >= 21 {
                    put(u32::from(n.gas_vent_side));
                    put(u32::from(n.gas_vent_species));
                    put(n.separator_water_mg);
                }
            }
            if format >= 16 && n.kind == Kind::Quarry {
                put(quarry::direction(n) as u32);
            }
            if format >= 17 {
                placer::encode(n, &mut put);
            }
        }
    }
    pub fn decode(
        read: &mut impl FnMut() -> anyhow::Result<u32>,
        size: i32,
        version: u8,
    ) -> anyhow::Result<Self> {
        use anyhow::ensure;
        let mut factory = Self::default();
        let extended = version >= 7;
        let kinds = Kind::count_in_format(version);
        let resources = resources_in_format(version);
        for k in &mut factory.kits[..kinds] {
            *k = read()?;
            ensure!(*k <= 65536, "Invalid factory kit count");
        }
        let count = read()? as usize;
        ensure!(count <= MAX_NODES, "Factory exceeds assembly limit");
        for _ in 0..count {
            let p = IVec3::new(read()? as i32, read()? as i32, read()? as i32);
            ensure!(
                p.to_array().iter().all(|v| v.unsigned_abs() <= 1_000_000) && anchor(p, size) == p,
                "Invalid assembly coordinate"
            );
            let k = read()? as usize;
            ensure!(k < kinds, "Unknown factory assembly");
            let mut n = Node::new(Kind::ALL[k]);
            for i in 0..resources {
                n.input[i] = read()?;
                ensure!(
                    n.input[i] <= CAPACITY
                        && (n.input[i] == 0
                            || n.kind == Kind::Fabricator
                            || (n.kind != Kind::Feeder && n.kind.accepts(i))),
                    "Invalid machine input"
                );
            }
            for i in 0..resources {
                n.output[i] = read()?;
                let valid = match n.kind {
                    Kind::Feeder => n.kind.accepts(i),
                    Kind::Crusher => (9..12).contains(&i) || i == GOLD_CONCENTRATE,
                    Kind::Washer => (9..12).contains(&i) || i == GOLD_CONCENTRATE,
                    Kind::MiningDrill => i < 4 || i == 5,
                    Kind::Furnace => (6..9).contains(&i) || i == 15 || i == GOLD_INGOT,
                    Kind::AlloyKiln => matches!(i, 14 | 15),
                    Kind::Fabricator => true, // keep buffered items after catalog edits
                    Kind::Cultivator => matches!(i, 4 | 13 | ETHANOL),
                    Kind::CokeOven => matches!(i, COKE | CREOSOTE),
                    Kind::BlastFurnace => matches!(i, 8 | 15),
                    Kind::Assembler => true, // validated against its target below
                    _ => false,
                };
                ensure!(
                    n.output[i] <= CAPACITY && (n.output[i] == 0 || valid),
                    "Invalid machine output"
                );
            }
            n.energy = read()?;
            n.burn = read()?;
            n.progress = read()?;
            let recipe = read()?;
            let mut enabled = read()?;
            if extended {
                n.mode = read()?;
                n.upgrade = read()?;
                let filter = read()?;
                ensure!(
                    n.mode < n.kind.modes().len().max(1) as u32
                        && n.upgrade <= n.kind.max_upgrade()
                        && filter <= RESOURCE_COUNT as u32,
                    "Invalid machine configuration"
                );
                ensure!(
                    n.upgrade == 0 || n.kind.upgradable(),
                    "Invalid drive upgrade"
                );
                ensure!(
                    filter == 0 || matches!(n.kind, Kind::Feeder | Kind::Storage | Kind::Pipe),
                    "Invalid machine filter"
                );
                n.filter = filter.checked_sub(1).map(|v| v as usize);
            }
            if version >= 8 {
                n.water = read()?;
                n.excavation = read()?;
                ensure!(
                    n.water <= extraction::WATER_CAPACITY
                        && (version >= 17
                            || n.water.is_multiple_of(64 / (size * size * size) as u32))
                        && (n.water == 0 || n.kind.water_port()),
                    "Invalid fluid buffer"
                );
                ensure!(
                    n.excavation
                        <= match n.kind {
                            Kind::SmelteryController => 15_624,
                            k if k.excavates() && version >= 15 =>
                                quarry::max_limit(size, n.mode) as u32,
                            Kind::MiningDrill => quarry::limit(size, n.mode, 0) as u32,
                            _ => 0,
                        },
                    "Invalid drill depth"
                );
            }
            if version >= 10 {
                n.target = read()?;
                n.stock = read()?;
                let obsolete_fabrication_order = n.kind == Kind::Fabricator
                    && stock_order::saved_order(n.target).is_some()
                    && stock_order::read(n.kind, n.mode, n.target).is_none();
                if obsolete_fabrication_order {
                    n.target = 0;
                    enabled = 0;
                }
                let target_ok = match n.kind {
                    Kind::CastingTable => {
                        n.target == 256 || n.target <= engine_tools::PARTS.part_kinds.len() as u32
                    }
                    Kind::SmelteryController => n.target <= RESOURCE_COUNT as u32,
                    Kind::Assembler => n.target == 0 || assembler::recipe(&n).is_some(),
                    Kind::Battery => n.target <= power::battery_rate(n.upgrade),
                    k if k.excavates() && version >= 15 => n.target <= quarry::MAX_DEPTH_M as u32,
                    Kind::Cultivator => {
                        n.target == 0
                            || (n.mode == fuel_order::MODE && n.target <= fuel_order::MAX_TARGET)
                    }
                    k if sides::is_conduit(k) => n.target <= sides::SIDE_BITS,
                    _ => n.target == 0,
                };
                ensure!(
                    target_ok || stock_order::read(n.kind, n.mode, n.target).is_some(),
                    "Invalid machine target"
                );
                ensure!(
                    n.stock <= assembler::STOCK_LIMIT
                        && (n.stock == 0 || n.kind == Kind::Assembler),
                    "Invalid assembler stock"
                );
                if n.kind == Kind::Assembler {
                    let output = assembler::recipe(&n).map(|r| r.output.item);
                    ensure!(
                        n.stock == 0 || assembler::stock_item(&n).is_some(),
                        "Assembler stock has no output identity"
                    );
                    ensure!(
                        n.output.iter().enumerate().all(|(i, &v)| v == 0
                            || output == Some(super::super::inventory::Item::Resource(i as u8))),
                        "Assembler output does not match target"
                    );
                    ensure!(
                        n.progress < assembler::CRAFT_TICKS
                            && (n.progress == 0 || output.is_some()),
                        "Invalid assembler progress"
                    );
                }
            }
            if version >= 11 {
                blocks::decode(&mut n, read, version)?;
            }
            if version >= 13 {
                n.faces = read()?;
                ensure!(
                    n.faces <= 0xFFF
                        && (n.faces == 0 || (!n.kind.conduit() && !n.kind.structural())),
                    "Invalid machine faces"
                );
            }
            if version >= 14 {
                let species = gas_species_in_format(version);
                for v in n.exhaust[..species]
                    .iter_mut()
                    .chain(&mut n.residue[..species])
                {
                    *v = read()?;
                }
                n.filter_credit = read()?;
                let vent = read()?;
                ensure!(
                    vent <= 1 && (vent == 0 || industry::port(n.kind)),
                    "Invalid gas vent"
                );
                n.gas_vent = vent != 0;
                n.fuel_resource = read()?;
                let ruptured = read()?;
                ensure!(
                    ruptured <= 1 && (ruptured == 0 || industry::component(n.kind)),
                    "Invalid gas damage"
                );
                n.ruptured = ruptured != 0;
                ensure!(!n.ruptured || n.gas_vent, "Ruptured component without leak");
                if version >= 21 {
                    let side = read()?;
                    let gas = read()?;
                    let water = read()?;
                    ensure!(
                        side <= 6
                            && gas <= SPECIES as u32
                            && ((side == 0 && gas == 0) || industry::port(n.kind))
                            && (water == 0
                                || (matches!(
                                    n.kind,
                                    Kind::WetScrubber | Kind::GasGenerator | Kind::Generator
                                ) && water < industry::WATER_UNIT_MG)),
                        "Invalid gas outlet or process water"
                    );
                    n.gas_vent_side = side as u8;
                    n.gas_vent_species = gas as u8;
                    n.separator_water_mg = water;
                    ensure!(
                        !matches!(n.kind, Kind::GasGenerator | Kind::Generator)
                            || industry::turbine::condensate(&n)
                                <= u64::from(extraction::WATER_CAPACITY)
                                    * u64::from(industry::WATER_UNIT_MG),
                        "Invalid condensate capacity"
                    );
                }
                ensure!(
                    matches!(n.fuel_resource, 3 | 13 | 19 | 20 | 21),
                    "Invalid combustion fuel"
                );
                ensure!(
                    n.exhaust.iter().map(|&v| u64::from(v)).sum::<u64>()
                        <= industry::gas_capacity(&n) as u64
                        && n.residue.iter().map(|&v| u64::from(v)).sum::<u64>()
                            <= industry::WASTE_CAPACITY as u64
                        && n.filter_credit <= industry::FILTER_CAPACITY,
                    "Invalid industrial stocks"
                );
            }
            if version >= 16 && n.kind == Kind::Quarry {
                n.facing = quarry::DIRECTIONS
                    .get(read()? as usize)
                    .ok_or_else(|| anyhow::anyhow!("Invalid quarry direction"))?
                    .1;
            }
            if version >= 17 {
                placer::decode(&mut n, size, read)?;
            }
            // The old gold mode charged for a block and had no recipe ID.
            // Its unconsumed concentrate remains in the input for normal smelting.
            if version < 18 && n.kind == Kind::Furnace && n.mode == recipes::GOLD_MODE {
                n.progress = 0;
            }
            let processing = n.kind.powered();
            ensure!(
                n.energy
                    <= if n.kind.generator() {
                        power::GENERATOR_BUFFER
                    } else if processing {
                        power::CONSUMER_BUFFER
                    } else if n.kind == Kind::Battery {
                        power::battery_capacity(n.upgrade)
                    } else if n.kind == Kind::Transformer {
                        power::transformer_rate(n.upgrade) * 2
                    } else if matches!(n.kind, Kind::InductionPort | Kind::InductionCell) {
                        induction::MAX_CAPACITY
                    } else {
                        0
                    },
                "Invalid stored power"
            );
            ensure!(
                n.burn
                    <= if n.kind == Kind::Generator {
                        if n.mode == 1 {
                            pile::CAPACITY
                        } else {
                            power::MAX_COMBUSTION_BURN
                        }
                    } else if n.kind == Kind::SmelteryController {
                        2400
                    } else if n.kind == Kind::CokeOven {
                        multiblock::COKE_OVEN_MAX_C
                    } else if n.kind == Kind::BlastFurnace {
                        multiblock::BLAST_MAX_C
                    } else if n.kind.excavates() {
                        (size * size * size) as u32
                    } else if n.kind == Kind::Bioreactor {
                        800
                    } else {
                        0
                    },
                "Invalid stored fuel"
            );
            ensure!(
                enabled <= 1
                    && (recipe > 0
                        || n.progress == 0
                        || n.kind == Kind::Assembler
                        || (n.kind == Kind::Generator && n.progress <= pile::MAX_HEAT)
                        || (n.kind == Kind::SmelteryController
                            && n.progress < super::super::smeltery::pool_capacity(size))
                        || (n.kind == Kind::Cultivator
                            && biomatter::recipe(n.mode).is_some_and(|r| n.progress < r.ticks))
                        || placer::valid_progress(&n)
                        || (n.kind == Kind::Crusher
                            && n.progress < blocks::TICKS
                            && blocks::recipe(&n).is_some())),
                "Invalid {:?} state at {p}: progress {}, recipe {recipe}, enabled {enabled}",
                n.kind,
                n.progress
            );
            n.enabled = enabled != 0;
            if recipe > 0 {
                ensure!(!placer::sluice(&n), "Unexpected placer recipe");
                let i = (recipe - 1) as usize;
                let process = recipes::process(n.kind, i)
                    .ok_or_else(|| anyhow::anyhow!("Invalid processing recipe"))?;
                if n.kind == Kind::Fabricator && n.progress >= process.ticks {
                    n.progress = 0;
                }
                ensure!(n.progress < process.ticks, "Invalid processing progress");
                ensure!(
                    n.kind.modes().is_empty()
                        || n.kind == Kind::Furnace
                        || (n.kind == Kind::Washer && n.mode == 0)
                        || i == n.mode as usize,
                    "Recipe does not match machine mode"
                );
                n.recipe = Some(i);
            }
            // Saving settles reserved fuel shipments into storage. Restore
            // their completion receipt even if the live callback never ran.
            factory.workshop_delivery |=
                n.kind == Kind::Cultivator && n.mode == fuel_order::MODE && n.target == 1;
            ensure!(
                factory.nodes.insert(p, n).is_none(),
                "Duplicate factory assembly"
            );
        }
        Ok(factory)
    }
}

#[cfg(test)]
mod tests;
