//! Metre-grid factory assemblies. Item and power networks are separate, bounded,
//! deterministic graphs; only conduits carry traffic between machine ports.
use super::{BlockId as B, World, mark_chunk_and_neighbors_dirty};
use glam::{IVec3, Vec3};
use std::collections::{HashMap, HashSet, VecDeque};
mod extraction;
pub mod recipes;
pub(crate) use extraction::tick_world;

pub const RESOURCE_COUNT: usize = 19;
pub const CAPACITY: u32 = 64 * 64;
pub const MAX_NODES: usize = 1024;
const DIRS: [IVec3; 6] = [
    IVec3::X,
    IVec3::NEG_X,
    IVec3::Y,
    IVec3::NEG_Y,
    IVec3::Z,
    IVec3::NEG_Z,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Kind {
    Generator,
    Crusher,
    Furnace,
    Feeder,
    Storage,
    Pipe,
    Cable,
    Bioreactor,
    AlloyKiln,
    Fabricator,
    Cultivator,
    MiningDrill,
    Pump,
    FluidPipe,
    Tank,
    Washer,
}
impl Kind {
    pub const ALL: [Self; 16] = [
        Self::Generator,
        Self::Crusher,
        Self::Furnace,
        Self::Feeder,
        Self::Storage,
        Self::Pipe,
        Self::Cable,
        Self::Bioreactor,
        Self::AlloyKiln,
        Self::Fabricator,
        Self::Cultivator,
        Self::MiningDrill,
        Self::Pump,
        Self::FluidPipe,
        Self::Tank,
        Self::Washer,
    ];
    pub fn block(self) -> B {
        match self {
            Self::Generator => B::CoalGenerator,
            Self::Crusher => B::Crusher,
            Self::Furnace => B::ElectricFurnace,
            Self::Feeder => B::FeedHopper,
            Self::Storage => B::StorageCrate,
            Self::Pipe => B::ItemPipe,
            Self::Cable => B::PowerCable,
            Self::Bioreactor => B::Bioreactor,
            Self::AlloyKiln => B::AlloyKiln,
            Self::Fabricator => B::Fabricator,
            Self::Cultivator => B::Cultivator,
            Self::MiningDrill => B::MiningDrill,
            Self::Pump => B::WaterPump,
            Self::FluidPipe => B::FluidPipe,
            Self::Tank => B::WaterTank,
            Self::Washer => B::OreWasher,
        }
    }
    pub fn from_block(b: B) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.block() == b)
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Generator => "Coal generator",
            Self::Crusher => "Ore crusher",
            Self::Furnace => "Electric furnace",
            Self::Feeder => "Feed hopper",
            Self::Storage => "Output crate",
            Self::Pipe => "Item pipe",
            Self::Cable => "Power cable",
            Self::Bioreactor => "Fungal bioreactor",
            Self::AlloyKiln => "Alloy kiln",
            Self::Fabricator => "Component fabricator",
            Self::Cultivator => "Mycelial cultivator",
            Self::MiningDrill => "Mining drill",
            Self::Pump => "Water pump",
            Self::FluidPipe => "Fluid pipe",
            Self::Tank => "Water tank",
            Self::Washer => "Ore washer",
        }
    }
    pub fn recipe(self) -> &'static [(usize, u32)] {
        match self {
            Self::Generator => &[(6, 128), (8, 128)],
            Self::Crusher => &[(6, 128), (7, 64), (8, 128)],
            Self::Furnace => &[(6, 128), (8, 128), (4, 64)],
            Self::Feeder => &[(7, 64)],
            Self::Storage => &[(8, 64)],
            Self::Pipe => &[(7, 64)],
            Self::Cable => &[(6, 64)],
            Self::Bioreactor => &[(15, 256), (16, 128), (17, 64), (13, 256)],
            Self::AlloyKiln => &[(8, 256), (6, 128)],
            Self::Fabricator => &[(14, 128), (8, 128), (12, 64)],
            Self::Cultivator => &[(14, 128), (16, 64), (13, 128)],
            Self::MiningDrill => &[(15, 256), (16, 128), (17, 64)],
            Self::Pump => &[(14, 128), (16, 64)],
            Self::FluidPipe => &[(6, 64), (7, 64)],
            Self::Tank => &[(8, 128), (7, 64)],
            Self::Washer => &[(15, 128), (16, 128), (14, 128)],
        }
    }
    pub fn yield_count(self) -> u32 {
        if matches!(self, Self::Pipe | Self::Cable | Self::FluidPipe) {
            8
        } else {
            1
        }
    }
    pub fn accepts(self, i: usize) -> bool {
        match self {
            Self::Generator => i == 3,
            Self::Bioreactor => i == 13,
            Self::Crusher | Self::Washer => i < 3,
            Self::Furnace => (9..12).contains(&i),
            Self::Feeder => i < RESOURCE_COUNT,
            Self::Storage => i < RESOURCE_COUNT,
            Self::AlloyKiln => matches!(i, 3 | 6 | 7 | 8),
            Self::Fabricator => matches!(i, 4 | 5 | 6 | 12 | 18),
            Self::Cultivator => matches!(i, 4 | 13),
            _ => false,
        }
    }
    pub fn conduit(self) -> bool {
        matches!(self, Self::Pipe | Self::Cable | Self::FluidPipe)
    }
    pub fn powered(self) -> bool {
        self.processor() || matches!(self, Self::MiningDrill | Self::Pump)
    }
    pub fn water_port(self) -> bool {
        matches!(self, Self::Pump | Self::Tank | Self::Washer)
    }
    fn generator(self) -> bool {
        matches!(self, Self::Generator | Self::Bioreactor)
    }
}

#[derive(Clone)]
pub struct Node {
    pub kind: Kind,
    pub input: [u32; RESOURCE_COUNT],
    pub output: [u32; RESOURCE_COUNT],
    pub energy: u32,
    pub burn: u32,
    pub progress: u32,
    pub recipe: Option<usize>,
    pub enabled: bool,
    pub status: &'static str,
    pub mode: u32,
    pub upgrade: u32,
    pub filter: Option<usize>,
    pub phase: f32,
    pub activity: f32,
    pub water: u32,      // 512 units per cubic metre at every voxel scale
    pub excavation: u32, // next cell in the drill shaft
    work_clock: f32,
    pub draining: bool,
}
impl Node {
    fn new(kind: Kind) -> Self {
        Self {
            kind,
            input: [0; RESOURCE_COUNT],
            output: [0; RESOURCE_COUNT],
            energy: 0,
            burn: 0,
            progress: 0,
            recipe: None,
            enabled: true,
            status: "Ready",
            mode: 0,
            upgrade: 0,
            filter: None,
            phase: 0.0,
            activity: 0.0,
            water: 0,
            excavation: 0,
            work_clock: 0.0,
            draining: false,
        }
    }
}

#[derive(Default)]
pub struct Factory {
    pub nodes: HashMap<IVec3, Node>,
    pub kits: [u32; 16],
    pub focus: Option<IVec3>,
    pub bench: Option<IVec3>,
    accumulator: f32,
    topology: bool,
    power: Vec<Network>,
    items: Vec<Network>,
    fluids: Vec<Network>,
    cursor: usize,
}

#[derive(Clone, Default)]
pub struct View {
    pub kits: [u32; 16],
    pub bench: bool,
    pub focused: Option<(IVec3, Node)>,
    pub count: usize,
}
#[derive(Clone, Copy)]
pub enum Action {
    Craft(Kind),
    Select(Kind),
    CraftSword(u32),
    SelectSword(u32),
    CookRations,
    EatRation,
    Load(IVec3),
    Collect(IVec3),
    Toggle(IVec3),
    Upgrade,
    SetRecipe(IVec3, u32),
    SetFilter(IVec3, Option<usize>),
    UpgradeMachine(IVec3),
    Wand,
    SelectSpell(u32),
    SetWandSlot(u32,u32),
    SwapWandSlots(u32,u32),
    SetWandMode(u32),
    Drain(IVec3),
}

struct Network {
    ports: Vec<IVec3>,
    conduits: Vec<IVec3>,
}

pub fn step(world: &World) -> i32 {
    (1.0 / world.voxel_scale.meters()).round() as i32
}
pub fn anchor(p: IVec3, size: i32) -> IVec3 {
    IVec3::new(
        p.x.div_euclid(size),
        p.y.div_euclid(size),
        p.z.div_euclid(size),
    ) * size
}
pub fn cells(p: IVec3, size: i32) -> Vec<IVec3> {
    let mut out = Vec::with_capacity((size * size * size) as usize);
    for y in 0..size {
        for z in 0..size {
            for x in 0..size {
                out.push(p + IVec3::new(x, y, z));
            }
        }
    }
    out
}
fn near(world: &World, p: IVec3, player: Vec3) -> bool {
    ((p.as_vec3() + Vec3::splat(step(world) as f32 * 0.5)) - player).length()
        * world.voxel_scale.meters()
        <= 7.0
}
fn at_bench(world: &World, player: Vec3) -> bool {
    world
        .factory
        .bench
        .is_some_and(|p| near(world, p, player) && world.get(p) == B::Workbench)
}
pub fn view(world: &World, player: Vec3) -> View {
    View {
        kits: world.factory.kits,
        bench: at_bench(world, player),
        count: world.factory.nodes.len(),
        focused: world
            .factory
            .focus
            .filter(|p| near(world, *p, player))
            .and_then(|p| world.factory.nodes.get(&p).cloned().map(|n| (p, n))),
    }
}

pub fn action(world: &mut World, player: Vec3, action: Action) -> Option<B> {
    match action {
        Action::CraftSword(tier) => {
            let owned = world.adventure.owns_weapon(tier);
            if !at_bench(world, player) {
                world.expedition.say("Use a workbench to assemble a sword.");
            } else if !(1..=2).contains(&tier) {
                world.expedition.say("Unknown sword recipe.");
            } else if owned {
                world.expedition.say("You already carry this sword.");
            } else if world.expedition.pay(super::adventure::sword_recipe(tier)) {
                world.adventure.add_weapon(tier);
                world.adventure.select_weapon(tier);
                world.expedition.say(format!("{} crafted and equipped.", super::adventure::sword_name(tier)));
            } else {
                world.expedition.say("Missing sword ingredients. Check the recipe in Crafting.");
            }
        }
        Action::SelectSword(tier) => {
            if world.adventure.select_weapon(tier) {
                world.expedition.say(if tier == 0 { "Sword put away. Left-click still strikes." } else { "Sword equipped. Left-click to strike." });
            }
        }
        Action::CookRations => {
            let recipe = super::adventure::ration_recipe();
            if !at_bench(world, player) {
                world.expedition.say("Use a workbench to prepare fungal rations.");
            } else if world.adventure.rations > 65533 {
                world.expedition.say("Ration pack is full.");
            } else if world.expedition.pay(recipe) {
                world.adventure.rations += 2;
                world.expedition.say("Prepared two fungal rations. Eat them from Pack to restore hunger and health.");
            } else {
                world.expedition.say("Need fungal biomass and lumen fibre to prepare rations.");
            }
        }
        Action::EatRation => {
            if !world.eat_food(super::inventory::FoodKind::FungalRation, None) {
                if world.adventure.rations == 0 {
                    world.expedition.say("No rations. Prepare them from fungal biomass and lumen fibre.");
                } else {
                    world.expedition.say("Hunger is already full.");
                }
            }
        }
        Action::Select(kind) => {
            if world.factory.kits[kind as usize] > 0 {
                return Some(kind.block());
            }
            world.expedition.say("Craft a kit first.");
        }
        Action::Craft(kind) => {
            if !at_bench(world, player) {
                world
                    .expedition
                    .say("Use F at a workbench, then open E nearby to craft.");
            } else if world.factory.kits[kind as usize] > 4096 - kind.yield_count() {
                world.expedition.say("Kit inventory is full.");
            } else if !world.inventory.can_insert(super::inventory::ItemStack {
                item: super::inventory::Item::Block(kind.block()),
                count: kind.yield_count(),
            }) {
                world
                    .inventory
                    .notify_overflow(super::inventory::Item::Block(kind.block()));
                world
                    .expedition
                    .say("Inventory full — make room in your pack before crafting.");
            } else if world.expedition.pay(kind.recipe()) {
                world.factory.kits[kind as usize] += kind.yield_count();
                world.reconcile_legacy_to_inventory();
                world.expedition.say(format!(
                    "Crafted {} × {}. Right-click to place on the metre grid.",
                    kind.yield_count(),
                    kind.name()
                ));
                return Some(kind.block());
            } else {
                world
                    .expedition
                    .say("Missing ingredients. See the workbench recipe list in E.");
            }
        }
        Action::Upgrade => {
            if !at_bench(world, player) {
                world
                    .expedition
                    .say("Return to your workbench to assemble tools.");
            } else {
                let tier = world.expedition.tool_tier;
                let recipe: &[(usize, u32)] = if tier == 0 {
                    &[(6, 128), (7, 64), (4, 128)]
                } else {
                    &[(8, 256), (5, 128), (4, 128)]
                };
                if tier < 2 && world.expedition.pay(recipe) {
                    world.expedition.tool_tier += 1;
                    world.expedition.say(
                        "Bore upgraded. F excavates connected rock; carry lumen fibre for fuel.",
                    );
                } else {
                    world
                        .expedition
                        .say("Check the bore recipe in your field journal.");
                }
            }
        }
        Action::SetWandSlot(slot,spell) => {world.adventure.set_wand_slot(slot,spell);}
        Action::SwapWandSlots(a,b) => {world.adventure.swap_wand_slots(a,b);}
        Action::SetWandMode(mode) => {if mode<=3 {world.adventure.loadout.mode=mode;}}
        Action::SelectSpell(spell) => {
            world.adventure.set_wand_slot(world.adventure.loadout.cursor as u32,spell);
        }
        Action::Wand => {
            if at_bench(world, player)
                && world.adventure.wand < 3
                && world
                    .expedition
                    .pay(super::adventure::wand_recipe(world.adventure.wand))
            {
                world.adventure.wand += 1;
                world.expedition.say("Wand ready. Arrange spells in Gear. Hold R to cast; T skips to the next slot. Upgrades add slots and shorten recharge.");
            } else {
                world
                    .expedition
                    .say("Use a nearby workbench with the wand ingredients shown in E.");
            }
        }
        Action::Load(p)
        | Action::Collect(p)
        | Action::Toggle(p)
        | Action::SetRecipe(p, _)
        | Action::SetFilter(p, _)
        | Action::UpgradeMachine(p)
        | Action::Drain(p) => {
            if !near(world, p, player) {
                world.expedition.say("Move within 7 metres of the machine.");
                return None;
            }
            if let Some(n) = world.factory.nodes.get_mut(&p) {
                match action {
                    Action::SetRecipe(_, mode) if (mode as usize) < n.kind.modes().len() => {
                        n.mode = mode;
                        n.recipe = None;
                        n.progress = 0;
                    }
                    Action::SetFilter(_, filter)
                        if matches!(n.kind, Kind::Feeder | Kind::Storage)
                            && filter.is_none_or(|i| i < RESOURCE_COUNT) =>
                    {
                        n.filter = filter
                    }
                    Action::UpgradeMachine(_) if n.kind.powered() && n.upgrade < 2 => {
                        let recipe: &[(usize, u32)] = if n.upgrade == 0 {
                            &[(14, 128), (16, 64)]
                        } else {
                            &[(15, 128), (16, 128), (17, 64)]
                        };
                        if world.expedition.pay(recipe) {
                            n.upgrade += 1;
                            world.expedition.say("Drive upgraded: faster processing with proportionally higher power demand.");
                        } else {
                            world.expedition.say("Missing drive upgrade ingredients.");
                        }
                    }
                    Action::Drain(_) if n.kind.water_port() => {
                        n.draining = !n.draining;
                        n.enabled = false;
                    }
                    Action::Toggle(_) => {
                        n.enabled = !n.enabled;
                        n.draining = false;
                    }
                    Action::Load(_) => {
                        for i in 0..RESOURCE_COUNT {
                            if !n.accepts(i) {
                                continue;
                            }
                            let dst = if n.kind == Kind::Feeder {
                                &mut n.output[i]
                            } else {
                                &mut n.input[i]
                            };
                            let amount = world.expedition.resources[i].min(CAPACITY - *dst);
                            world.expedition.resources[i] -= amount;
                            *dst += amount;
                        }
                        world.expedition.say("Loaded matching supplies. Item pipes feed connected machines automatically.");
                    }
                    Action::Collect(_) => {
                        // Collect also unloads inputs. Pausing first lets the player safely reconfigure a line.
                        for i in 0..RESOURCE_COUNT {
                            for src in [&mut n.input[i], &mut n.output[i]] {
                                let amount = (*src).min(64_000_000 - world.expedition.resources[i]);
                                world.expedition.resources[i] += amount;
                                *src -= amount;
                            }
                        }
                        world.expedition.say("Machine inventories collected.");
                    }
                    _ => {}
                }
            }
        }
    }
    world.reconcile_legacy_to_inventory();
    world.sync_from_inventory();
    None
}

/// Returns true for every factory placement request, including a rejected one.
pub fn place(
    world: &mut World,
    cell: IVec3,
    b: B,
    player: &crate::player::Player,
    blocks: &mut HashSet<IVec3>,
    chunks: &mut HashSet<IVec3>,
) -> bool {
    let Some(kind) = Kind::from_block(b) else {
        return false;
    };
    let size = step(world);
    let p = anchor(cell, size);
    if world.factory.nodes.contains_key(&p) {
        return true;
    }
    if world.factory.nodes.len() >= MAX_NODES || world.factory.kits[kind as usize] == 0 {
        world
            .expedition
            .say("No crafted kits available, or the 1024-part factory limit was reached.");
        return true;
    }
    let body = cells(p, size);
    let cfg = world.sample_config();
    if p.y < cfg.min_world_y() || p.y + size > cfg.max_world_y() {
        world
            .expedition
            .say("Assembly exceeds the world's vertical bounds.");
        return true;
    }
    let (lo, hi) = super::player_aabb(player.pos, world.voxel_scale, player.scale, player.crouch_t);
    let a = p.as_vec3();
    let z = a + Vec3::splat(size as f32);
    if (lo.cmplt(z) & hi.cmpgt(a)).all() || body.iter().any(|c| world.get(*c) != B::Air) {
        world
            .expedition
            .say("Assembly needs a clear 1 × 1 × 1 metre space outside your body.");
        return true;
    }
    let edits: Vec<_> = body.iter().map(|c| (*c, b)).collect();
    world.set_many(&edits);
    if body.iter().any(|c| world.get(*c) != b) {
        // All assemblies fit one chunk; a failed resident edit never earns a node.
        world
            .expedition
            .say("Assembly could not be placed; kit retained.");
        return true;
    }
    world.factory.kits[kind as usize] -= 1;
    let _ = world
        .inventory
        .remove_item(super::inventory::Item::Block(kind.block()), 1);
    world.sync_from_inventory();
    world.factory.nodes.insert(p, Node::new(kind));
    world.factory.topology = false;
    for c in body {
        blocks.insert(c);
        mark_chunk_and_neighbors_dirty(chunks, c);
    }
    true
}

pub fn dismantle(
    world: &mut World,
    cell: IVec3,
    blocks: &mut HashSet<IVec3>,
    chunks: &mut HashSet<IVec3>,
) -> bool {
    let size = step(world);
    let p = anchor(cell, size);
    let Some(node) = world.factory.nodes.remove(&p) else {
        return false;
    };
    if node.water > 0 {
        world.factory.nodes.insert(p, node);
        world
            .expedition
            .say("Drain this assembly from its machine panel before packing it up.");
        return true;
    }
    let body = cells(p, size);
    world.set_many(&body.iter().map(|c| (*c, B::Air)).collect::<Vec<_>>());
    if body.iter().any(|c| world.get(*c) != B::Air) {
        world.factory.nodes.insert(p, node);
        return true;
    }
    world.factory.kits[node.kind as usize] += 1;
    // Drive parts are recoverable, just like the assembly and its inventories.
    if node.upgrade >= 1 {
        world.expedition.add(14, 128);
        world.expedition.add(16, 64);
    }
    if node.upgrade >= 2 {
        world.expedition.add(15, 128);
        world.expedition.add(16, 128);
        world.expedition.add(17, 64);
    }
    for i in 0..RESOURCE_COUNT {
        world.expedition.add(i, node.input[i] + node.output[i]);
    }
    world.factory.topology = false;
    if world.factory.focus == Some(p) {
        world.factory.focus = None;
    }
    for c in body {
        blocks.insert(c);
        mark_chunk_and_neighbors_dirty(chunks, c);
    }
    world
        .expedition
        .say("Recovered assembly and contents. Stored energy dissipated.");
    true
}

impl Factory {
    pub fn encode(&self, bytes: &mut Vec<u8>) {
        let mut put = |n: u32| bytes.extend_from_slice(&n.to_le_bytes());
        for n in self.kits {
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
            for v in n.input {
                put(v);
            }
            for v in n.output {
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
        let kinds = if version >= 8 {
            Kind::ALL.len()
        } else if extended {
            11
        } else {
            8
        };
        let resources = if extended { RESOURCE_COUNT } else { 14 };
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
                        && (n.input[i] == 0 || (n.kind != Kind::Feeder && n.kind.accepts(i))),
                    "Invalid machine input"
                );
            }
            for i in 0..resources {
                n.output[i] = read()?;
                let valid = match n.kind {
                    Kind::Feeder => n.kind.accepts(i),
                    Kind::Crusher | Kind::Washer => (9..12).contains(&i),
                    Kind::MiningDrill => i < 4 || i == 5,
                    Kind::Furnace => (6..9).contains(&i),
                    Kind::AlloyKiln => matches!(i, 14 | 15),
                    Kind::Fabricator => matches!(i, 16 | 17),
                    Kind::Cultivator => matches!(i, 4 | 13),
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
            let enabled = read()?;
            if extended {
                n.mode = read()?;
                n.upgrade = read()?;
                let filter = read()?;
                ensure!(
                    n.mode < n.kind.modes().len().max(1) as u32
                        && n.upgrade <= 2
                        && filter <= RESOURCE_COUNT as u32,
                    "Invalid machine configuration"
                );
                ensure!(n.upgrade == 0 || n.kind.powered(), "Invalid drive upgrade");
                ensure!(
                    filter == 0 || matches!(n.kind, Kind::Feeder | Kind::Storage),
                    "Invalid machine filter"
                );
                n.filter = filter.checked_sub(1).map(|v| v as usize);
            }
            if version >= 8 {
                n.water = read()?;
                n.excavation = read()?;
                ensure!(
                    n.water <= extraction::WATER_CAPACITY
                        && n.water % (64 / (size * size * size) as u32) == 0
                        && (n.water == 0 || n.kind.water_port()),
                    "Invalid fluid buffer"
                );
                ensure!(
                    n.excavation <= (24 * size * 3 * size * 3 * size) as u32
                        && (n.excavation == 0 || n.kind == Kind::MiningDrill),
                    "Invalid drill depth"
                );
            }
            let processing = n.kind.powered();
            ensure!(
                n.energy
                    <= if n.kind.generator() {
                        2000
                    } else if processing {
                        64
                    } else {
                        0
                    },
                "Invalid stored power"
            );
            ensure!(
                n.burn
                    <= if n.kind == Kind::Generator {
                        400
                    } else if n.kind == Kind::MiningDrill {
                        (size * size * size) as u32
                    } else if n.kind == Kind::Bioreactor {
                        800
                    } else {
                        0
                    },
                "Invalid stored fuel"
            );
            ensure!(
                enabled <= 1 && (recipe > 0 || n.progress == 0),
                "Invalid machine state"
            );
            n.enabled = enabled != 0;
            if recipe > 0 {
                let i = (recipe - 1) as usize;
                let process = recipes::process(n.kind, i)
                    .ok_or_else(|| anyhow::anyhow!("Invalid processing recipe"))?;
                ensure!(n.progress < process.ticks, "Invalid processing progress");
                ensure!(
                    n.kind.modes().is_empty() || i == n.mode as usize,
                    "Recipe does not match machine mode"
                );
                n.recipe = Some(i);
            }
            ensure!(
                factory.nodes.insert(p, n).is_none(),
                "Duplicate factory assembly"
            );
        }
        Ok(factory)
    }
    fn networks(&self, conduit: Kind, size: i32) -> Vec<Network> {
        let mut visited = HashSet::new();
        let mut nets = Vec::new();
        let mut starts: Vec<_> = self
            .nodes
            .iter()
            .filter(|(_, n)| n.kind == conduit)
            .map(|(p, _)| *p)
            .collect();
        starts.sort_by_key(|p| (p.x, p.y, p.z));
        for p in starts {
            if !visited.insert(p) {
                continue;
            }
            let mut queue = VecDeque::from([p]);
            let mut ports = HashSet::new();
            let mut conduits = Vec::new();
            while let Some(p) = queue.pop_front() {
                conduits.push(p);
                for d in DIRS {
                    let q = p + d * size;
                    if let Some(n) = self.nodes.get(&q) {
                        if n.kind == conduit {
                            if visited.insert(q) {
                                queue.push_back(q);
                            }
                        } else if !matches!(n.kind, Kind::Pipe | Kind::Cable | Kind::FluidPipe) {
                            ports.insert(q);
                        }
                    }
                }
            }
            let mut ports: Vec<_> = ports.into_iter().collect();
            ports.sort_by_key(|p| (p.x, p.y, p.z));
            nets.push(Network { ports, conduits });
        }
        nets
    }
    pub fn tick(&mut self, dt: f32, size: i32) {
        for n in self.nodes.values_mut() {
            if n.enabled && matches!(n.status, "Processing" | "Generating") {
                n.phase = (n.phase + dt * (1 + n.upgrade) as f32) % 1000.0;
            }
            n.activity = (n.activity - dt * 2.0).max(0.0);
        }
        self.accumulator += dt.clamp(0.0, 0.25);
        if self.accumulator < 0.25 {
            return;
        }
        self.accumulator -= 0.25;
        if !self.topology {
            self.power = self.networks(Kind::Cable, size);
            self.items = self.networks(Kind::Pipe, size);
            self.fluids = self.networks(Kind::FluidPipe, size);
            self.topology = true;
        }
        for n in self.nodes.values_mut() {
            n.status = if n.enabled { "Idle" } else { "Paused" };
            if !n.enabled || !n.kind.generator() {
                continue;
            }
            let (fuel, portion, capacity, rate) = if n.kind == Kind::Generator {
                (3, 16, 400, 16)
            } else {
                (13, 32, 800, 32)
            };
            if n.energy <= 2000 - rate {
                if n.burn == 0 && n.input[fuel] >= portion {
                    n.input[fuel] -= portion;
                    n.burn = capacity;
                }
                let energy = n.burn.min(rate);
                n.burn -= energy;
                n.energy += energy;
                n.status = if energy > 0 {
                    "Generating"
                } else {
                    "Needs fuel"
                };
            } else {
                n.status = "Buffer full";
            }
        }
        // Rotation gives every consumer a chance under an undersized supply.
        for network in &self.power {
            let net = &network.ports;
            let mut flow = false;
            if net.is_empty() {
                continue;
            }
            for k in 0..net.len() {
                let p = net[(k + self.cursor) % net.len()];
                let n = &self.nodes[&p];
                if !n.enabled || !n.kind.powered() {
                    continue;
                }
                let mut need = 64_u32.saturating_sub(n.energy);
                for g in net {
                    if need == 0 {
                        break;
                    }
                    let source = self.nodes.get_mut(g).unwrap();
                    if !source.enabled || !source.kind.generator() {
                        continue;
                    }
                    let transfer = need.min(source.energy);
                    flow |= transfer > 0;
                    source.energy -= transfer;
                    need -= transfer;
                    self.nodes.get_mut(&p).unwrap().energy += transfer;
                }
            }
            if flow {
                for p in &network.conduits {
                    self.nodes.get_mut(p).unwrap().activity = 1.0;
                }
            }
        }
        extraction::route_water(self);
        self.cursor = self.cursor.wrapping_add(1);
        for n in self.nodes.values_mut() {
            if !n.enabled || !n.kind.processor() {
                continue;
            }
            if n.recipe.is_none() {
                n.recipe = n.choose_recipe();
            }
            let Some(recipe) = n.process() else {
                n.status = "Needs input";
                continue;
            };
            let (out, yield_n) = recipe.output;
            if !recipe.inputs.iter().all(|&(i, n_in)| n.input[i] >= n_in) {
                n.recipe = None;
                n.progress = 0;
                n.status = "Needs input";
                continue;
            }
            if n.output[out] > CAPACITY - yield_n {
                n.status = "Output blocked";
                continue;
            }
            if n.kind == Kind::Washer && n.water < 128 {
                n.status = "Needs water";
                continue;
            }
            let steps = (1 + n.upgrade).min(recipe.ticks - n.progress);
            let cost = recipe.power * steps;
            if n.energy < cost {
                n.status = "Needs cable / power";
                continue;
            }
            n.energy -= cost;
            n.progress += steps;
            n.status = "Processing";
            if n.progress >= recipe.ticks {
                for (i, amount) in recipe.inputs {
                    // A living culture is a catalyst: fibre and power grow new
                    // biomass while the seed remains in the cultivator.
                    if n.kind == Kind::Cultivator && n.mode == 0 && i == 13 {
                        continue;
                    }
                    n.input[i] -= amount;
                }
                if n.kind == Kind::Washer {
                    n.water -= 128;
                }
                n.output[out] += yield_n;
                n.progress = 0;
                n.recipe = None;
            }
        }
        // Only outputs travel. Material filters make the processing graph acyclic:
        // feeder -> crusher -> furnace -> crate, and feeder -> generator.
        // One unit per source per tick; backpressure retains everything at source.
        let mut moved = HashSet::new();
        for network in &self.items {
            let net = &network.ports;
            let mut flow = false;
            if net.is_empty() {
                continue;
            }
            for k in 0..net.len() {
                let p = net[(k + self.cursor) % net.len()];
                if moved.contains(&p) || !self.nodes[&p].enabled {
                    continue;
                }
                'items: for i in 0..RESOURCE_COUNT {
                    if self.nodes[&p].kind == Kind::Feeder
                        && self.nodes[&p].filter.is_some_and(|filter| filter != i)
                    {
                        continue;
                    }
                    let available = self.nodes[&p].output[i].min(64);
                    if available == 0 {
                        continue;
                    }
                    // Supply processors before storage so finished components can
                    // continue through a shared bus without being intercepted.
                    let mut destinations: Vec<_> = (0..net.len())
                        .map(|offset| net[(offset + self.cursor) % net.len()])
                        .collect();
                    destinations.sort_by_key(|q| self.nodes[q].kind == Kind::Storage);
                    for q in destinations {
                        let dest = &self.nodes[&q];
                        if p == q || !dest.enabled || dest.kind == Kind::Feeder || !dest.accepts(i)
                        {
                            continue;
                        }
                        let amount = available.min(CAPACITY - dest.input[i]);
                        if amount == 0 {
                            continue;
                        }
                        self.nodes.get_mut(&p).unwrap().output[i] -= amount;
                        self.nodes.get_mut(&q).unwrap().input[i] += amount;
                        flow = true;
                        moved.insert(p);
                        break 'items;
                    }
                }
            }
            if flow {
                for p in &network.conduits {
                    self.nodes.get_mut(p).unwrap().activity = 1.0;
                }
            }
        }
    }
}

/// Collision/picking envelopes for conduit junctions, in voxel coordinates.
/// The same face connections drive the visible tube arms.
pub fn conduit_bounds(world: &World, cell: IVec3) -> Option<Vec<(Vec3, Vec3)>> {
    let size = step(world);
    let p = anchor(cell, size);
    let n = world.factory.nodes.get(&p)?;
    if !matches!(n.kind, Kind::Pipe | Kind::Cable | Kind::FluidPipe) {
        return None;
    }
    let c = p.as_vec3() + Vec3::splat(size as f32 * 0.5);
    let r = if n.kind == Kind::Cable { 0.10 } else { 0.19 } * size as f32;
    let mut bounds = vec![(c - Vec3::splat(r), c + Vec3::splat(r))];
    for d in DIRS {
        if world
            .factory
            .nodes
            .get(&(p + d * size))
            .is_some_and(|other| {
                other.kind == n.kind
                    || !matches!(other.kind, Kind::Pipe | Kind::Cable | Kind::FluidPipe)
            })
        {
            let end = c + d.as_vec3() * size as f32 * 0.5;
            let extent = Vec3::splat(r) * (Vec3::ONE - d.as_vec3().abs());
            bounds.push((c.min(end) - extent, c.max(end) + extent));
        }
    }
    Some(bounds)
}
pub fn solid_point(world: &World, point: Vec3) -> bool {
    let p = point / world.voxel_scale.meters();
    let cell = p.floor().as_ivec3();
    if !world.solid(cell) {
        return false;
    }
    conduit_bounds(world, cell).is_none_or(|bounds| {
        bounds
            .iter()
            .any(|&(a, b)| p.cmpge(a).all() && p.cmple(b).all())
    })
}
pub fn ray_box(origin: Vec3, dir: Vec3, a: Vec3, b: Vec3, max: f32) -> bool {
    let mut low = 0.0_f32;
    let mut high = max;
    for axis in 0..3 {
        if dir[axis].abs() < 1e-6 {
            if origin[axis] < a[axis] || origin[axis] > b[axis] {
                return false;
            }
        } else {
            let x = (a[axis] - origin[axis]) / dir[axis];
            let y = (b[axis] - origin[axis]) / dir[axis];
            low = low.max(x.min(y));
            high = high.min(x.max(y));
        }
    }
    high >= low
}
