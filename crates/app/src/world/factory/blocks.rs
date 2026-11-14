//! Whole building blocks stored by machines, separate from resource units.
use super::{CAPACITY, DIRS, Kind, Node, sides};
use crate::simulation::Budget;
use crate::world::{World, block_data, scanner};
use crate::world::inventory::{Item, ItemStack, SurvivalInventory};
use engine_content::BlockId as B;
use glam::IVec3;

pub const TICKS: u32 = 16;
pub const POWER: u32 = 8;

/// Move whole-block output into a touching processor or chest.
pub(super) fn export(world: &mut World, p: IVec3, n: &mut Node, size: i32, deadline: Budget) {
    if n.blocks.output.iter().all(|&count| count == 0) {
        return;
    }
    let mut allowance = 16;
    for (face, d) in DIRS.into_iter().enumerate() {
        if !matches!(sides::role(n, face), sides::Role::Both | sides::Role::Out) {
            continue;
        }
        if n.kind.excavates() && deadline.has_time() {
            route_ore(world, p + d * size, face ^ 1, n, size);
        }
        let outside = if d.min_element() < 0 { -1 } else { size };
        for a in 0..size {
            for b in 0..size {
                let q = p + match face / 2 {
                    0 => IVec3::new(outside, a, b),
                    1 => IVec3::new(a, outside, b),
                    _ => IVec3::new(a, b, outside),
                };
                if !matches!(world.block_data.entries.get(&block_data::key(q)), Some(block_data::Entry::Chest(_)))
                    || world.get(q).chest_tier().is_none()
                {
                    continue;
                }
                if !deadline.has_time() {
                    return;
                }
                let policy = scanner::policy::get(world, q);
                let mut slots = [world.block_data.chest(q)];
                let mut output = n.blocks.output;
                let before = allowance;
                for &material in Material::ALL {
                    let count = &mut output[material.index()];
                    if *count == 0 || allowance == 0 {
                        continue;
                    }
                    let item = material.item();
                    let amount = (*count).min(allowance);
                    let mut stack = ItemStack::new(item, amount);
                    scanner::policy::deposit(
                        &mut slots,
                        std::slice::from_ref(&policy),
                        &mut stack,
                        world.inventory.stack_limit(item),
                        scanner::Reach::Smart,
                    );
                    let moved = amount - stack.count;
                    *count -= moved;
                    allowance -= moved;
                }
                if allowance != before {
                    let [slots] = slots;
                    world.block_data.set_chest(q, slots);
                    n.blocks.output = output;
                }
                if allowance == 0 {
                    return;
                }
            }
        }
    }
}

fn route_ore(world: &mut World, p: IVec3, face: usize, source: &mut Node, size: i32) {
    let Some(dest) = world.factory.nodes.get(&p) else { return };
    if !matches!(sides::role(dest, face), sides::Role::Both | sides::Role::In) {
        return;
    }
    let volume = super::placer::cell_volume(size);
    let quartz = Material::GoldQuartz.index();
    if matches!(dest.kind, Kind::Crusher | Kind::Washer) && !super::placer::sluice(dest) {
        let moved = source.blocks.output[quartz].min(CAPACITY - dest.blocks.input[quartz]);
        source.blocks.output[quartz] -= moved;
        world.factory.nodes.get_mut(&p).unwrap().blocks.input[quartz] += moved;
    }
    for (material, resource) in raw_ores() {
        let dest = &world.factory.nodes[&p];
        if !accepts_raw_ore(dest, resource) { continue; }
        let room = CAPACITY.saturating_sub(dest.input[resource] + world.factory.input_reserved(p, resource));
        let count = &mut source.blocks.output[material.index()];
        let moved = (*count).min(room / volume);
        *count -= moved;
        world.factory.nodes.get_mut(&p).unwrap().input[resource] += moved * volume;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "multiplayer", derive(serde::Serialize, serde::Deserialize))]
pub struct Buffers {
    #[cfg_attr(feature = "multiplayer", serde(serialize_with = "write_block_counts", deserialize_with = "read_block_counts"))]
    pub input: [u32; Material::COUNT],
    #[cfg_attr(feature = "multiplayer", serde(serialize_with = "write_block_counts", deserialize_with = "read_block_counts"))]
    pub output: [u32; Material::COUNT],
}

impl Default for Buffers {
    fn default() -> Self {
        Self { input: [0; Material::COUNT], output: [0; Material::COUNT] }
    }
}

#[cfg(feature = "multiplayer")]
fn write_block_counts<S: serde::Serializer>(counts: &[u32; Material::COUNT], s: S) -> Result<S::Ok, S::Error> {
    use serde::ser::SerializeTuple;
    let mut tuple = s.serialize_tuple(counts.len())?;
    for count in counts { tuple.serialize_element(count)?; }
    tuple.end()
}

#[cfg(feature = "multiplayer")]
fn read_block_counts<'de, D: serde::Deserializer<'de>>(d: D) -> Result<[u32; Material::COUNT], D::Error> {
    super::read_counts(d, &[Material::LEGACY_COUNT, Material::QUARRY_COUNT, Material::PRE_MINERAL_COUNT])
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Material {
    Stone,
    Dirt,
    Grass,
    Sand,
    Gravel,
    Limestone,
    Basalt,
    CopperOre,
    TinOre,
    IronOre,
    CoalOre,
    Crystal,
    SnowGrass,
    Sandstone,
    Clay,
    Mud,
    Ice,
    Log,
    Leaves,
    PineLeaves,
    FungalSoil,
    MushroomStem,
    GlowCap,
    RedCap,
    Bush,
    AmberCap,
    UmberCap,
    Paydirt,
    GoldBlock,
    GoldQuartz,
    Chalcopyrite,
    Malachite,
    Hematite,
    Magnetite,
    Cassiterite,
}

impl Material {
    pub(super) const LEGACY_COUNT: usize = 7;
    const QUARRY_COUNT: usize = Self::Paydirt as usize;
    const PRE_MINERAL_COUNT: usize = Self::Chalcopyrite as usize;
    pub const COUNT: usize = Self::ALL.len();
    pub const ALL: &'static [Self] = &[
        Self::Stone,
        Self::Dirt,
        Self::Grass,
        Self::Sand,
        Self::Gravel,
        Self::Limestone,
        Self::Basalt,
        Self::CopperOre,
        Self::TinOre,
        Self::IronOre,
        Self::CoalOre,
        Self::Crystal,
        Self::SnowGrass,
        Self::Sandstone,
        Self::Clay,
        Self::Mud,
        Self::Ice,
        Self::Log,
        Self::Leaves,
        Self::PineLeaves,
        Self::FungalSoil,
        Self::MushroomStem,
        Self::GlowCap,
        Self::RedCap,
        Self::Bush,
        Self::AmberCap,
        Self::UmberCap,
        Self::Paydirt,
        Self::GoldBlock,
        Self::GoldQuartz,
        Self::Chalcopyrite,
        Self::Malachite,
        Self::Hematite,
        Self::Magnetite,
        Self::Cassiterite,
    ];

    pub fn from_block(block: B) -> Option<Self> {
        Self::ALL.iter().copied().find(|material| material.block() == block)
    }

    pub fn block(self) -> B {
        match self {
            Self::Stone => B::Stone,
            Self::Dirt => B::Dirt,
            Self::Grass => B::Grass,
            Self::Sand => B::Sand,
            Self::Gravel => B::Gravel,
            Self::Limestone => B::Limestone,
            Self::Basalt => B::Basalt,
            Self::CopperOre => B::CopperOre,
            Self::TinOre => B::TinOre,
            Self::IronOre => B::IronOre,
            Self::CoalOre => B::CoalOre,
            Self::Crystal => B::Crystal,
            Self::SnowGrass => B::SnowGrass,
            Self::Sandstone => B::Sandstone,
            Self::Clay => B::Clay,
            Self::Mud => B::Mud,
            Self::Ice => B::Ice,
            Self::Log => B::Log,
            Self::Leaves => B::Leaves,
            Self::PineLeaves => B::PineLeaves,
            Self::FungalSoil => B::FungalSoil,
            Self::MushroomStem => B::MushroomStem,
            Self::GlowCap => B::GlowCap,
            Self::RedCap => B::RedCap,
            Self::Bush => B::Bush,
            Self::AmberCap => B::AmberCap,
            Self::UmberCap => B::UmberCap,
            Self::Paydirt => B::Paydirt,
            Self::GoldBlock => B::GoldBlock,
            Self::GoldQuartz => B::GoldQuartz,
            Self::Chalcopyrite => B::Chalcopyrite,
            Self::Malachite => B::Malachite,
            Self::Hematite => B::Hematite,
            Self::Magnetite => B::Magnetite,
            Self::Cassiterite => B::Cassiterite,
        }
    }

    pub fn item(self) -> Item {
        Item::Block(self.block())
    }

    pub fn index(self) -> usize {
        self as usize
    }
}

/// Automatic mineral crushing starts when no ore batch is active.
pub fn recipe(n: &Node) -> Option<(Material, Material)> {
    if n.kind != Kind::Crusher {
        return None;
    }
    if n.blocks.input[Material::Stone.index()] > 0 {
        Some((Material::Stone, Material::Gravel))
    } else if n.blocks.input[Material::Gravel.index()] > 0 {
        Some((Material::Gravel, Material::Sand))
    } else {
        None
    }
}

/// Runs one block-processing step and returns committed energy use.
pub fn step(n: &mut Node) -> u32 {
    let Some((input, output)) = recipe(n) else {
        n.progress = 0;
        return 0;
    };
    if n.blocks.output[output.index()] >= CAPACITY {
        n.status = "Output blocked";
        return 0;
    }
    let steps = (1 + n.upgrade).min(TICKS - n.progress);
    let cost = POWER * steps;
    if n.energy < cost {
        n.status = "Needs cable / power";
        return 0;
    }
    n.energy -= cost;
    n.progress += steps;
    n.status = "Processing";
    if n.progress == TICKS {
        n.blocks.input[input.index()] -= 1;
        n.blocks.output[output.index()] += 1;
        n.progress = 0;
    }
    cost
}

pub(super) fn raw_ores() -> impl Iterator<Item = (Material, usize)> {
    Material::ALL.iter().copied().filter_map(|material| {
        crate::world::expedition::ore_resource(material.block()).map(|resource| (material, resource))
    })
}

fn accepts_raw_ore(n: &Node, resource: usize) -> bool {
    matches!(n.kind, Kind::Crusher | Kind::Washer | Kind::Furnace | Kind::SmelteryController
        | Kind::CokeOven | Kind::BlastFurnace | Kind::Generator | Kind::Fabricator)
        && !super::placer::sluice(n) && n.accepts(resource)
}

pub fn load(inventory: &mut SurvivalInventory, n: &mut Node, size: i32, reserved: &[u32; super::RESOURCE_COUNT]) {
    if n.kind == Kind::Assembler {
        if let Some(recipe) = super::assembler::recipe(n) {
            for (material, _) in super::assembler::block_needs(recipe) {
                let input = &mut n.blocks.input[material.index()];
                let count = inventory.count_item(material.item()).min(CAPACITY.saturating_sub(*input));
                if count > 0 && inventory.remove_item(material.item(), count) { *input += count; }
            }
        }
        return;
    }
    let volume = super::placer::cell_volume(size);
    for (material, resource) in raw_ores() {
        if !accepts_raw_ore(n, resource) { continue; }
        let room = CAPACITY.saturating_sub(n.input[resource] + reserved[resource]);
        let count = inventory.count_item(material.item()).min(room / volume);
        if count > 0 && inventory.remove_item(material.item(), count) {
            n.input[resource] += count * volume;
        }
    }
    let materials: &[Material] = match n.kind {
        Kind::Crusher => &[Material::Stone, Material::Gravel, Material::GoldQuartz],
        Kind::Cultivator if super::biomatter::recipe(n.mode).is_some() => &[Material::Grass],
        Kind::Washer if super::placer::sluice(n) => &[Material::Paydirt],
        Kind::Washer => &[Material::GoldQuartz],
        _ => return,
    };
    for &material in materials {
        let index = material.index();
        let room = CAPACITY.saturating_sub(n.blocks.input[index]);
        let count = inventory.count_item(material.item()).min(room);
        if count > 0 && inventory.remove_item(material.item(), count) {
            n.blocks.input[index] += count;
        }
    }
}

pub fn collect(inventory: &mut SurvivalInventory, n: &mut Node) {
    for &material in Material::ALL {
        let index = material.index();
        for buffer in [&mut n.blocks.input[index], &mut n.blocks.output[index]] {
            let count = *buffer;
            let (moved, _) = inventory.try_insert(ItemStack {
                item: material.item(),
                count,
            });
            *buffer -= moved;
        }
    }
}

/// Simulates a full content refund without changing the live inventory.
pub fn refund_fits(inventory: &mut SurvivalInventory, n: &Node) -> bool {
    Material::ALL.iter().copied().all(|material| {
        let index = material.index();
        let count = n.blocks.input[index].saturating_add(n.blocks.output[index]);
        inventory
            .try_insert(ItemStack {
                item: material.item(),
                count,
            })
            .1
            == 0
    })
}

pub fn encode(n: &Node, put: &mut impl FnMut(u32), format: u8) {
    for buffer in [n.blocks.input, n.blocks.output] {
        for count in buffer.into_iter().take(saved_count(format)) {
            put(count);
        }
    }
}

fn saved_count(format: u8) -> usize {
    match format {
        19.. => Material::COUNT,
        17..=18 => Material::PRE_MINERAL_COUNT,
        15..=16 => Material::QUARRY_COUNT,
        _ => Material::LEGACY_COUNT,
    }
}

pub fn decode(n: &mut Node, read: &mut impl FnMut() -> anyhow::Result<u32>, format: u8) -> anyhow::Result<()> {
    for buffer in [&mut n.blocks.input, &mut n.blocks.output] {
        for count in buffer.iter_mut().take(saved_count(format)) {
            *count = read()?;
            anyhow::ensure!(*count <= CAPACITY, "Invalid machine block buffer");
        }
    }
    for &material in Material::ALL {
        let index = material.index();
        anyhow::ensure!(
            n.blocks.input[index] == 0
                || match n.kind {
                    Kind::Crusher => matches!(material, Material::Stone | Material::Gravel | Material::GoldQuartz),
                    Kind::Cultivator => material == Material::Grass,
                    Kind::Washer => matches!(material, Material::Paydirt | Material::GoldQuartz),
                    Kind::Assembler => true,
                    _ => false,
                },
            "Invalid machine block input"
        );
        anyhow::ensure!(
            n.blocks.output[index] == 0
                || n.kind.excavates()
                || (n.kind == Kind::Washer && material == Material::Sand)
                || (n.kind == Kind::Furnace && material == Material::GoldBlock)
                || (n.kind == Kind::Crusher && matches!(material, Material::Gravel | Material::Sand)),
            "Invalid machine block output"
        );
    }
    Ok(())
}
