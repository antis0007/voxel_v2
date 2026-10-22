//! Metre-grid factory assemblies. Item and power networks are separate, bounded,
//! deterministic graphs; only conduits carry traffic between machine ports.
use super::{BlockId as B, World, mark_chunk_and_neighbors_dirty};
use glam::{IVec3, Vec3};
use std::collections::{HashMap, HashSet, VecDeque};
mod extraction;
pub mod assembler;
pub mod induction;
pub mod multiblock;
pub mod overview;
pub mod pile;
pub mod power;
pub mod portable;
pub mod stats;
pub mod recipes;
pub mod transit;
pub(crate) use extraction::tick_world;

pub const RESOURCE_COUNT: usize = 22;
/// Resources that existed before save version 20 (coke, creosote and
/// bio-ethanol were appended after them).
pub const LEGACY_RESOURCE_COUNT: usize = 19;
pub const COKE: usize = 19;
pub const CREOSOTE: usize = 20;
pub const ETHANOL: usize = 21;
pub const CAPACITY: u32 = 64 * 64;
pub const MAX_NODES: usize = 1024;
/// Layout version `Factory::encode` writes (save version 20 and later).
#[cfg(test)]
pub const FACTORY_FORMAT: u8 = 10;
const DIRS: [IVec3; 6] = [
    IVec3::X,
    IVec3::NEG_X,
    IVec3::Y,
    IVec3::NEG_Y,
    IVec3::Z,
    IVec3::NEG_Z,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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
    Battery,
    CokeOven,
    BlastFurnace,
    CokeBrick,
    BlastBrick,
    InductionCasing,
    InductionPort,
    InductionCell,
    InductionProvider,
    Transformer,
    Assembler,
}
impl Kind {
    pub const COUNT: usize = 27;
    /// Kinds that existed before save version 20.
    pub const LEGACY_COUNT: usize = 16;
    /// Number of kinds a factory layout version stores.
    pub fn count_in_format(format: u8) -> usize {
        match format {
            10.. => Self::COUNT,
            9 => 21,
            8 => Self::LEGACY_COUNT,
            7 => 11,
            _ => 8,
        }
    }
    pub const ALL: [Self; Self::COUNT] = [
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
        Self::Battery,
        Self::CokeOven,
        Self::BlastFurnace,
        Self::CokeBrick,
        Self::BlastBrick,
        Self::InductionCasing,
        Self::InductionPort,
        Self::InductionCell,
        Self::InductionProvider,
        Self::Transformer,
        Self::Assembler,
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
            Self::Battery => B::BatteryBank,
            Self::CokeOven => B::CokeOven,
            Self::BlastFurnace => B::BlastFurnace,
            Self::CokeBrick => B::CokeOvenBrick,
            Self::BlastBrick => B::BlastBrick,
            Self::InductionCasing => B::InductionCasing,
            Self::InductionPort => B::InductionPort,
            Self::InductionCell => B::InductionCell,
            Self::InductionProvider => B::InductionProvider,
            Self::Transformer => B::Transformer,
            Self::Assembler => B::Assembler,
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
            Self::Battery => "Battery bank",
            Self::CokeOven => "Coke oven door",
            Self::BlastFurnace => "Blast furnace tuyere",
            Self::CokeBrick => "Coke oven brick",
            Self::BlastBrick => "Blast brick",
            Self::InductionCasing => "Induction casing",
            Self::InductionPort => "Induction port",
            Self::InductionCell => "Induction cell",
            Self::InductionProvider => "Induction provider",
            Self::Transformer => "Transformer",
            Self::Assembler => "Assembling machine",
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
            Self::Battery => &[(6, 128), (7, 128), (8, 64)],
            Self::CokeOven => &[(8, 128), (3, 64)],
            Self::BlastFurnace => &[(8, 256), (COKE, 128)],
            // Bricks come from the crafting grid (clay, sand, stone); this is
            // the workbench fallback for players who have ingots but no clay.
            Self::CokeBrick => &[(3, 16)],
            Self::BlastBrick => &[(COKE, 16)],
            Self::InductionCasing => &[(15, 64), (8, 64)],
            Self::InductionPort => &[(15, 128), (16, 128), (17, 64)],
            Self::InductionCell => &[(16, 256), (15, 128), (17, 64)],
            Self::InductionProvider => &[(16, 128), (14, 128), (17, 64)],
            Self::Transformer => &[(16, 128), (6, 128), (8, 64)],
            Self::Assembler => &[(8, 192), (16, 64), (12, 64)],
        }
    }
    pub fn yield_count(self) -> u32 {
        if matches!(self, Self::Pipe | Self::Cable | Self::FluidPipe) {
            8
        } else if self == Self::InductionCasing {
            2
        } else if self.structural() {
            1
        } else {
            1
        }
    }
    pub fn accepts(self, i: usize) -> bool {
        match self {
            Self::Generator => matches!(i, 3 | 13 | COKE | CREOSOTE | ETHANOL) || i == pile::FUEL,
            Self::Bioreactor => i == 13,
            Self::Crusher | Self::Washer => i < 3,
            Self::Furnace => (9..12).contains(&i) || i < 3,
            Self::CokeOven => i == 3,
            Self::BlastFurnace => matches!(i, 2 | 8 | COKE),
            Self::Feeder => i < RESOURCE_COUNT,
            Self::Storage => i < RESOURCE_COUNT,
            Self::Assembler => i < RESOURCE_COUNT,
            Self::AlloyKiln => matches!(i, 3 | 6 | 7 | 8),
            Self::Fabricator => matches!(i, 4 | 5 | 6 | 7 | 12 | 13 | 18),
            Self::Cultivator => matches!(i, 4 | 13),
            _ => false,
        }
    }
    /// Multiblock casing. Not a port, not a machine: it only gives shape to a
    /// coke oven or blast furnace.
    pub fn structural(self) -> bool {
        matches!(
            self,
            Self::CokeBrick
                | Self::BlastBrick
                | Self::InductionCasing
                | Self::InductionCell
                | Self::InductionProvider
        )
    }
    /// Holds grid energy in a pool: batteries, transformers, matrix ports.
    pub fn stores_power(self) -> bool {
        matches!(self, Self::Battery | Self::Transformer | Self::InductionPort)
    }
    pub fn conduit(self) -> bool {
        matches!(self, Self::Pipe | Self::Cable | Self::FluidPipe)
    }
    pub fn powered(self) -> bool {
        self.processor() || matches!(self, Self::MiningDrill | Self::Pump | Self::Assembler)
    }
    /// Machines that take the three-step upgrade: drives for consumers,
    /// cell chemistry for batteries.
    pub fn upgradable(self) -> bool {
        self.powered() || matches!(self, Self::Battery | Self::Transformer | Self::Cable)
    }
    pub fn upgrade_cost(self, level: u32) -> &'static [(usize, u32)] {
        match (self, level) {
            // Per cable piece: steel-armoured HV, then a resonant superconductor.
            (Self::Cable, 0) => &[(15, 16), (7, 8)],
            (Self::Cable, _) => &[(17, 8), (16, 16)],
            (Self::Transformer, 0) => &[(16, 128), (15, 64)],
            (Self::Transformer, _) => &[(16, 256), (17, 64)],
            (Self::Battery, 0) => &[(14, 128), (16, 64), (8, 64)],
            (Self::Battery, _) => &[(15, 128), (16, 128), (17, 64)],
            (_, 0) => &[(14, 128), (16, 64)],
            _ => &[(15, 128), (16, 128), (17, 64)],
        }
    }
    pub fn water_port(self) -> bool {
        matches!(
            self,
            Self::Pump | Self::Tank | Self::Washer | Self::Generator
        )
    }
    pub(crate) fn generator(self) -> bool {
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
    /// Signed power flow last tick in units (= kW): positive while a
    /// generator produces or a battery charges, negative while drawing.
    pub flow: i32,
    /// Multiblock controllers and bricks: part of a complete shell.
    pub formed: bool,
    /// Controllers: (bricks found, bricks needed) for the best orientation.
    pub structure: (u16, u16),
    /// Controllers: outward face of the oven door / tuyere.
    pub facing: IVec3,
    /// Saved from format 10. Assembler: recipe index + 1 (0 = none).
    /// Battery: I/O limit in kW (0 = the cell's full rate).
    pub target: u32,
    /// Saved from format 10. Assembler: finished items that are not
    /// resources (machine kits), waiting to be collected.
    pub stock: u32,
    /// Induction ports: the port that holds the formed matrix's energy.
    pub pool: Option<IVec3>,
    /// Induction matrix master: capacity (units) and transfer rate (units per tick).
    pub store_cap: u32,
    pub store_rate: u32,
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
            flow: 0,
            formed: false,
            structure: (0, 0),
            facing: IVec3::NEG_Z,
            target: 0,
            stock: 0,
            pool: None,
            store_cap: 0,
            store_rate: 0,
        }
    }
}

#[derive(Default)]
pub struct Factory {
    pub nodes: crate::fast_hash::FastMap<IVec3, Node>,
    pub kits: [u32; Kind::COUNT],
    pub focus: Option<IVec3>,
    pub bench: Option<IVec3>,
    accumulator: f32,
    topology: bool,
    power: Vec<Network>,
    items: Vec<Network>,
    fluids: Vec<Network>,
    cursor: usize,
    /// Piles that melted down since the world last collected them.
    meltdowns: Vec<IVec3>,
    /// Items currently travelling through item pipes.
    pub packets: Vec<transit::Packet>,
    /// Amount in flight toward each (machine, resource); counts against capacity.
    reserved: HashMap<(IVec3, usize), u32>,
    /// Items that lost their route (pipe or machine removed) and go back to the player.
    refunds: Vec<(usize, u32)>,
    /// Item pipe cell -> index into `items`.
    item_net: HashMap<IVec3, usize>,
    /// Measured state of each power network, parallel to `power`.
    power_stats: Vec<power::NetStats>,
    /// Cable or port -> index into `power`.
    power_net: HashMap<IVec3, usize>,
    /// Factorio-style production and power history.
    pub stats: stats::Production,
}

#[derive(Clone, Default)]
pub struct View {
    pub habitats: super::habitats::View,
    pub kits: [u32; Kind::COUNT],
    pub bench: bool,
    pub focused: Option<(IVec3, Node)>,
    /// Cable network of the focused machine.
    pub focused_grid: Option<power::NetStats>,
    /// Energy pool (stored, capacity, rate) of a focused storage assembly.
    pub focused_pool: Option<(u32, u32, u32)>,
    /// Factory health: what every assembly is doing, and the main limit.
    pub health: overview::Summary,
    /// Production totals and the span (s) they cover: last minute, last hour.
    pub minute: (stats::Sample, f32),
    pub hour: (stats::Sample, f32),
    /// Generation and draw per second over the last minute (units).
    pub power_history: Vec<(u64, u64)>,
    pub count: usize,
}
#[derive(Clone, Copy)]
pub enum Action {
    Habitat(super::habitats::Action),
    /// Assembler recipe (index + 1, 0 clears) or battery I/O limit in kW.
    SetTarget(IVec3, u32),
    Craft(Kind),
    Select(Kind),
    CraftSword(u32),
    SelectSword(u32),
    CookRations,
    EatRation,
    Load(IVec3),
    Collect(IVec3),
    /// Move `amount` units of a resource between a container and the pack.
    /// `deposit` sends pack → container. `u32::MAX` means the whole compartment.
    BufferMove(IVec3, usize, u32, bool),
    Toggle(IVec3),
    Upgrade,
    SetRecipe(IVec3, u32),
    SetFilter(IVec3, Option<usize>),
    UpgradeMachine(IVec3),
    Wand,
    SelectSpell(u32),
    SetWandSlot(u32, u32),
    SwapWandSlots(u32, u32),
    SetWandMode(u32),
    Drain(IVec3),
    /// Fit a material into a tool part. Parameters are `Part`, `Material`.
    FitToolPart(u8, u8),
    ToggleToolModifier(u8),
    ClearToolParts,
}

struct Network {
    ports: Vec<IVec3>,
    conduits: Vec<IVec3>,
    /// Cable tier (the `upgrade` of every cable in it); 0 for pipes.
    tier: u32,
    /// Built once with the network. Path searches borrow this instead of
    /// allocating a new set for every ingot.
    pipes: HashSet<IVec3>,
}
impl Network {
    fn new(ports: Vec<IVec3>, conduits: Vec<IVec3>) -> Self {
        let pipes = conduits.iter().copied().collect();
        Self {
            ports,
            conduits,
            pipes,
            tier: 0,
        }
    }
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
        habitats: super::habitats::view(world, player * world.voxel_scale.meters()),
        kits: world.factory.kits,
        bench: at_bench(world, player),
        count: world.factory.nodes.len(),
        focused: world
            .factory
            .focus
            .filter(|p| near(world, *p, player))
            .and_then(|p| world.factory.nodes.get(&p).cloned().map(|n| (p, n))),
        focused_grid: world.factory.focus.and_then(|p| world.factory.grid(p)),
        focused_pool: world.factory.focus.and_then(|p| world.factory.pool_of(p)),
        health: overview::summarize(&world.factory),
        minute: world.factory.stats.window(false),
        hour: world.factory.stats.window(true),
        power_history: world
            .factory
            .stats
            .seconds
            .iter()
            .map(|s| (s.generated, s.drawn))
            .collect(),
    }
}

pub fn action(world: &mut World, player: Vec3, action: Action) -> Option<B> {
    match action {
        Action::Habitat(action) => {
            super::habitats::action(world, player * world.voxel_scale.meters(), action);
        }
        Action::CraftSword(tier) => {
            let owned = world.adventure.owns_weapon(tier);
            if !at_bench(world, player) {
                world
                    .expedition
                    .say("Use a workbench to assemble a weapon.");
            } else if !matches!(tier, 1 | 2 | 5) {
                world.expedition.say("Unknown weapon recipe.");
            } else if owned {
                world.expedition.say("You already carry this weapon.");
            } else if world.expedition.pay(super::adventure::sword_recipe(tier)) {
                world.adventure.add_weapon(tier);
                world.adventure.select_weapon(tier);
                world.expedition.say(format!(
                    "{} crafted and equipped.",
                    super::adventure::sword_name(tier)
                ));
            } else {
                world
                    .expedition
                    .say("Missing sword ingredients. Check the recipe in Crafting.");
            }
        }
        Action::SelectSword(tier) => {
            if world.adventure.select_weapon(tier) {
                world.expedition.say(if tier == 0 {
                    "Sword put away. Left-click still strikes."
                } else {
                    "Sword equipped. Left-click to strike."
                });
            }
        }
        Action::FitToolPart(part, material) => {
            use super::adventure::tinkering::{self, Material, Part};
            let Some(part) = Part::from_u8(part) else {
                world.expedition.say("Unknown tool part.");
                return None;
            };
            let Some(material) = Material::from_u8(material) else {
                world.expedition.say("Unknown fitting material.");
                return None;
            };
            if !at_bench(world, player) {
                world
                    .expedition
                    .say("Fit tool parts at a workbench. Press F at one first.");
            } else if world.adventure.weapon == 0 {
                world
                    .expedition
                    .say("Equip a melee weapon before fitting its parts.");
            } else {
                let current = match part {
                    Part::Head => world.adventure.tool_head,
                    Part::Binding => world.adventure.tool_bind,
                    Part::Grip => world.adventure.tool_grip,
                };
                if current == material.as_u8() {
                    world.expedition.say(format!(
                        "{} is already fitted in the {}.",
                        material.name(),
                        part.label().to_lowercase()
                    ));
                } else if world.expedition.pay(tinkering::part_cost(part, material)) {
                    world.adventure.fit_part(part, material);
                    world.expedition.say(format!(
                        "{} {} fitted. {}",
                        material.name(),
                        part.label().to_lowercase(),
                        material.trait_detail()
                    ));
                } else {
                    world
                        .expedition
                        .say("Missing fitting materials. Check the Forge costs.");
                }
            }
        }
        Action::ToggleToolModifier(bit) => {
            use super::adventure::tinkering;
            let Some(modifier) = tinkering::modifier(bit) else {
                world.expedition.say("Unknown tool modifier.");
                return None;
            };
            if !at_bench(world, player) {
                world
                    .expedition
                    .say("Modify tools at a workbench. Press F at one first.");
            } else if world.adventure.weapon == 0 {
                world
                    .expedition
                    .say("Equip a melee weapon before modifying it.");
            } else if world.adventure.has_modifier(bit) {
                world.adventure.toggle_modifier(bit);
                world
                    .expedition
                    .say(format!("{} modifier removed.", modifier.name));
            } else if world.expedition.pay(modifier.cost) {
                world.adventure.toggle_modifier(bit);
                world
                    .expedition
                    .say(format!("{} installed. {}", modifier.name, modifier.detail));
            } else {
                world
                    .expedition
                    .say("Missing modifier materials. Check the Forge costs.");
            }
        }
        Action::ClearToolParts => {
            if !at_bench(world, player) {
                world.expedition.say("Strip fittings at a workbench.");
            } else {
                world.adventure.clear_fittings();
                world
                    .expedition
                    .say("Fittings stripped back to copper. Installed modifiers were kept.");
            }
        }
        Action::CookRations => {
            let recipe = super::adventure::ration_recipe();
            if !at_bench(world, player) {
                world
                    .expedition
                    .say("Use a workbench to prepare fungal rations.");
            } else if world.adventure.rations > 65533 {
                world.expedition.say("Ration pack is full.");
            } else if world.expedition.pay(recipe) {
                world.adventure.rations += 2;
                world.expedition.say(
                    "Prepared two fungal rations. Eat them from Pack to restore hunger and health.",
                );
            } else {
                world
                    .expedition
                    .say("Need fungal biomass and lumen fibre to prepare rations.");
            }
        }
        Action::EatRation => {
            if !world.eat_food(super::inventory::FoodKind::FungalRation, None) {
                if world.adventure.rations == 0 {
                    world
                        .expedition
                        .say("No rations. Prepare them from fungal biomass and lumen fibre.");
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
        Action::SetWandSlot(slot, spell) => {
            world.adventure.set_wand_slot(slot, spell);
        }
        Action::SwapWandSlots(a, b) => {
            world.adventure.swap_wand_slots(a, b);
        }
        Action::SetWandMode(mode) => {
            if mode <= 3 {
                world.adventure.loadout.mode = mode;
            }
        }
        Action::SelectSpell(spell) => {
            world
                .adventure
                .set_wand_slot(world.adventure.loadout.cursor as u32, spell);
        }
        Action::Wand => {
            if at_bench(world, player)
                && world.adventure.wand < 3
                && world
                    .expedition
                    .pay(super::adventure::wand_recipe(world.adventure.wand))
            {
                world.adventure.wand += 1;
                let tier = world.adventure.wand;
                let learned = world.adventure.grant_upgrade_spells(tier);
                let log = world.adventure.spell_log;
                super::adventure::wands::seed_unlocked_slots_known(
                    &mut world.adventure.loadout,
                    tier,
                    &|id| log.grants(id),
                );
                if !learned.is_empty() {
                    let names: Vec<_> = learned
                        .iter()
                        .map(|&id| super::adventure::spell_name(id))
                        .collect();
                    world
                        .expedition
                        .say(format!("Spells discovered: {}.", names.join(", ")));
                }
                world.expedition.say("Wand ready. Arrange spells in Gear. Hold R to cast; T skips to the next slot. Upgrades add slots and shorten recharge.");
            } else {
                world
                    .expedition
                    .say("Use a nearby workbench with the wand ingredients shown in E.");
            }
        }
        Action::Load(p)
        | Action::Collect(p)
        | Action::BufferMove(p, _, _, _)
        | Action::Toggle(p)
        | Action::SetRecipe(p, _)
        | Action::SetFilter(p, _)
        | Action::UpgradeMachine(p)
        | Action::SetTarget(p, _)
        | Action::Drain(p) => {
            if !near(world, p, player) {
                world.expedition.say("Move within 7 metres of the machine.");
                return None;
            }
            if let Some(n) = world.factory.nodes.get_mut(&p) {
                match action {
                    Action::SetRecipe(_, mode)
                        if (n.kind == Kind::Furnace && (9..12).contains(&mode))
                            || (mode as usize) < n.kind.modes().len() =>
                    {
                        if n.kind == Kind::Furnace && mode >= 9 {
                            n.mode = 0;
                            n.recipe = Some(mode as usize);
                        } else if matches!(n.kind, Kind::CokeOven | Kind::BlastFurnace)
                            && n.recipe.is_some()
                        {
                            // The charge is already in the hearth.
                            world
                                .expedition
                                .say("Wait for the current batch before changing the recipe.");
                            return None;
                        } else {
                            n.mode = mode;
                            n.recipe = None;
                        }
                        // A generator's progress is its heat; switching mode
                        // must not be a free way to dump it.
                        if !matches!(n.kind, Kind::Generator | Kind::Battery) {
                            n.progress = 0;
                        }
                        if n.kind == Kind::Storage && n.mode == 2 && n.filter.is_none() {
                            n.filter = n
                                .input
                                .iter()
                                .enumerate()
                                .filter(|(_, count)| **count > 0)
                                .max_by_key(|(_, count)| *count)
                                .map(|(i, _)| i);
                        }
                    }
                    Action::BufferMove(_, item, amount, deposit)
                        if n.kind == Kind::Storage && item < RESOURCE_COUNT =>
                    {
                        if deposit {
                            if n.accepts(item) {
                                let room = CAPACITY.saturating_sub(n.input[item]);
                                let moved =
                                    amount.min(room).min(world.expedition.resources[item]);
                                if moved > 0 && n.mode == 2 && n.filter.is_none() {
                                    n.filter = Some(item);
                                }
                                world.expedition.resources[item] -= moved;
                                n.input[item] += moved;
                            } else {
                                world
                                    .expedition
                                    .say("This container will not take that item.");
                            }
                        } else {
                            let have = n.input[item];
                            let moved = amount.min(have);
                            n.input[item] -= moved;
                            world.expedition.resources[item] =
                                world.expedition.resources[item].saturating_add(moved);
                            if n.mode == 2 && n.input.iter().all(|count| *count == 0) {
                                n.filter = None;
                            }
                        }
                    }
                    Action::SetFilter(_, filter)
                        if matches!(n.kind, Kind::Feeder | Kind::Storage | Kind::Pipe)
                            && filter.is_none_or(|i| i < RESOURCE_COUNT) =>
                    {
                        n.filter = filter
                    }
                    Action::SetTarget(_, target) => match n.kind {
                        Kind::Assembler
                            if target as usize <= assembler::recipe_count()
                                && (target == 0
                                    || assembler::automatable(
                                        &super::inventory::all_recipes()[target as usize - 1],
                                    )) =>
                        {
                            if n.target != target && n.stock == 0 && n.output.iter().all(|v| *v == 0) {
                                n.target = target;
                                n.progress = 0;
                            } else if n.target != target {
                                world.expedition.say("Collect or route the current output before changing the assembler recipe.");
                            }
                        }
                        Kind::Battery if target <= power::battery_rate(n.upgrade) => {
                            n.target = target;
                        }
                        _ => {}
                    },
                    Action::UpgradeMachine(_) if n.kind == Kind::Cable && n.upgrade < 2 => {
                        // Upgrade the whole connected line of this tier, one
                        // piece at a time while materials last.
                        let tier = n.upgrade;
                        let size = step(world);
                        let mut line = vec![p];
                        let mut seen = HashSet::from([p]);
                        let mut k = 0;
                        while k < line.len() {
                            let c = line[k];
                            k += 1;
                            for d in DIRS {
                                let q = c + d * size;
                                if world.factory.nodes.get(&q).is_some_and(|o| {
                                    o.kind == Kind::Cable && o.upgrade == tier
                                }) && seen.insert(q)
                                {
                                    line.push(q);
                                }
                            }
                        }
                        let cost = Kind::Cable.upgrade_cost(tier);
                        let mut done = 0;
                        for c in &line {
                            if !world.expedition.pay(cost) {
                                break;
                            }
                            world.factory.nodes.get_mut(c).unwrap().upgrade = tier + 1;
                            done += 1;
                        }
                        world.factory.topology = false;
                        world.expedition.say(format!(
                            "{done} of {} cable pieces upgraded to {} ({}).",
                            line.len(),
                            power::cable_tier_name(tier + 1),
                            power::fmt_power(power::cable_capacity(tier + 1) as f64)
                        ));
                        world.reconcile_legacy_to_inventory();
                        world.sync_from_inventory();
                        return None;
                    }
                    Action::UpgradeMachine(_) if n.kind.upgradable() && n.upgrade < 2 => {
                        if world.expedition.pay(n.kind.upgrade_cost(n.upgrade)) {
                            n.upgrade += 1;
                            if n.kind == Kind::Battery {
                                world.expedition.say(format!(
                                    "Cells replaced: {} · {} at up to {}.",
                                    power::battery_tier_name(n.upgrade),
                                    power::fmt_energy(power::battery_capacity(n.upgrade) as f64),
                                    power::fmt_power(power::battery_rate(n.upgrade) as f64)
                                ));
                            } else {
                                world.expedition.say("Drive upgraded: faster processing with proportionally higher power demand.");
                            }
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
                    Action::Collect(_) if n.kind == Kind::Assembler && n.stock > 0 => {
                        if let Some(kind) = assembler::stock_kind(n) {
                            let count = n.stock;
                            n.stock = 0;
                            world.factory.kits[kind as usize] += count;
                            world.expedition.say(format!("Collected {count} × {}.", kind.name()));
                        }
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
    held: Option<super::inventory::Item>,
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
    let portable = held.and_then(|i| match i {
        super::inventory::Item::PowerKit(k) if k.kind == kind && world.inventory.count_item(i) > 0 => Some(k),
        _ => None,
    });
    if world.factory.nodes.len() >= MAX_NODES || (portable.is_none() && world.factory.kits[kind as usize] == 0) {
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
    let item = portable.map(super::inventory::Item::PowerKit)
        .unwrap_or(super::inventory::Item::Block(kind.block()));
    let _ = world.inventory.remove_item(item, 1);
    world.sync_from_inventory();
    world.factory.nodes.insert(p, portable.map_or_else(|| Node::new(kind), |k| k.node()));
    world.factory.form_matrices(size);
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
    if !world.factory.nodes.contains_key(&p) { return false; }
    // Publish charge to individual cells before any topology edit. Removing a
    // port cannot remove the pool, and removing a cell takes only its charge.
    world.factory.scatter_matrix_energy();
    world.factory.topology = false;
    let Some(node) = world.factory.nodes.remove(&p) else {
        return false;
    };
    if node.water > 0 {
        world.factory.nodes.insert(p, node);
        world.factory.form_matrices(size);
        world
            .expedition
            .say("Drain this assembly from its machine panel before packing it up.");
        return true;
    }
    if node.kind == Kind::InductionCell && node.energy > induction::CELL_CAPACITY {
        world.factory.nodes.insert(p, node);
        world.factory.form_matrices(size);
        world.expedition.say("Restore matrix capacity before packing this cell: excess legacy charge is held offline.");
        return true;
    }
    let portable = portable::PowerKit::from_node(&node);
    use super::inventory::{Item, ItemStack};
    let mut inventory = world.inventory.clone();
    let item = portable.map(Item::PowerKit).unwrap_or(Item::Block(node.kind.block()));
    let mut fits = inventory.try_insert(ItemStack::new(item, 1)).1 == 0;
    if let Some(kind) = assembler::stock_kind(&node) {
        if node.stock > 0 {
            fits &= inventory.try_insert(ItemStack { item: Item::Block(kind.block()), count: node.stock }).1 == 0;
        }
    }
    let mut refunds = std::array::from_fn::<_, RESOURCE_COUNT, _>(|i| node.input[i] + node.output[i]);
    for level in 0..if portable.is_some() { 0 } else { node.upgrade } {
        for &(i, n) in node.kind.upgrade_cost(level) { refunds[i] += n; }
    }
    // Hearth inputs are reserved at batch start, unlike electrical processors.
    if matches!(node.kind, Kind::CokeOven | Kind::BlastFurnace) {
        if let Some(process) = node.process() {
            for (i, n) in process.inputs { refunds[i] += n; }
        }
    }
    for (i, amount) in refunds.into_iter().enumerate() {
        let total = amount + inventory.resource_remainders[i];
        if total >= 64 {
            fits &= inventory.try_insert(ItemStack { item: Item::Resource(i as u8), count: total / 64 }).1 == 0;
        }
        inventory.resource_remainders[i] = total % 64;
    }
    if !fits {
        world.factory.nodes.insert(p, node);
        world.factory.form_matrices(size);
        world.expedition.say("Make room for the assembly and all its contents before packing it up.");
        return true;
    }
    let body = cells(p, size);
    world.set_many(&body.iter().map(|c| (*c, B::Air)).collect::<Vec<_>>());
    if body.iter().any(|c| world.get(*c) != B::Air) {
        world.factory.nodes.insert(p, node);
        world.factory.form_matrices(size);
        return true;
    }
    world.inventory = inventory;
    world.sync_from_inventory();
    world.factory.topology = false;
    world.factory.form_matrices(size);
    if world.factory.focus == Some(p) {
        world.factory.focus = None;
    }
    for c in body {
        blocks.insert(c);
        mark_chunk_and_neighbors_dirty(chunks, c);
    }
    world
        .expedition
        .say(if portable.is_some() { "Packed assembly with its charge, upgrades and settings." }
            else { "Recovered assembly and contents." });
    true
}

impl Factory {
    /// Habitat ports share crate acceptance and in-flight capacity reservations.
    pub(crate) fn habitat_exchange(&mut self, p: IVec3, resource: usize, feed: &mut u32, output: &mut u32, capacity: u32) {
        let accepts = self.container_takes(p, resource);
        let incoming = self.reserved.get(&(p, resource)).copied().unwrap_or(0);
        let Some(node) = self.nodes.get_mut(&p) else { return; };
        if node.kind != Kind::Storage || !node.enabled { return; }
        let moved = 64.min(capacity.saturating_sub(*feed)).min(node.input[13]);
        node.input[13] -= moved;
        *feed += moved;
        if accepts {
            let room = CAPACITY.saturating_sub(node.input[resource].saturating_add(incoming));
            let moved = (*output).min(room);
            node.input[resource] += moved;
            *output -= moved;
            if moved > 0 && node.mode == 2 && node.filter.is_none() { node.filter = Some(resource); }
        }
    }
    #[cfg(test)]
    pub fn encode(&self, bytes: &mut Vec<u8>) {
        self.encode_format(bytes, FACTORY_FORMAT);
    }
    /// Writes an older layout (format 8: 16 kinds, 19 resources). Saves never
    /// go backwards; this exists so tests can build genuine legacy files.
    pub fn encode_format(&self, bytes: &mut Vec<u8>, format: u8) {
        let kinds = Kind::count_in_format(format);
        let resources = if format >= 9 {
            RESOURCE_COUNT
        } else {
            LEGACY_RESOURCE_COUNT
        };
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
                put(n.target);
                put(n.stock);
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
        let resources = if version >= 9 {
            RESOURCE_COUNT
        } else if extended {
            LEGACY_RESOURCE_COUNT
        } else {
            14
        };
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
                    Kind::Fabricator => matches!(i, 12 | 16 | 17 | 18),
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
                ensure!(n.upgrade == 0 || n.kind.upgradable(), "Invalid drive upgrade");
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
            if version >= 10 {
                n.target = read()?;
                n.stock = read()?;
                let target_ok = match n.kind {
                    Kind::Assembler => n.target == 0 || assembler::recipe(&n).is_some(),
                    Kind::Battery => n.target <= power::battery_rate(n.upgrade),
                    _ => n.target == 0,
                };
                ensure!(target_ok, "Invalid machine target");
                ensure!(
                    n.stock <= assembler::STOCK_LIMIT && (n.stock == 0 || n.kind == Kind::Assembler),
                    "Invalid assembler stock"
                );
                if n.kind == Kind::Assembler {
                    let output = assembler::recipe(&n).map(|r| r.output.item);
                    ensure!(n.stock == 0 || assembler::stock_kind(&n).is_some(), "Assembler stock has no kit identity");
                    ensure!(n.output.iter().enumerate().all(|(i, &v)| v == 0 || output == Some(super::inventory::Item::Resource(i as u8))), "Assembler output does not match target");
                    ensure!(n.progress < assembler::CRAFT_TICKS && (n.progress == 0 || output.is_some()), "Invalid assembler progress");
                }
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
                    } else if n.kind == Kind::CokeOven {
                        multiblock::COKE_OVEN_MAX_C
                    } else if n.kind == Kind::BlastFurnace {
                        multiblock::BLAST_MAX_C
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
                enabled <= 1
                    && (recipe > 0
                        || n.progress == 0
                        || n.kind == Kind::Assembler
                        || (n.kind == Kind::Generator && n.progress <= pile::MAX_HEAT)),
                "Invalid machine state"
            );
            n.enabled = enabled != 0;
            if recipe > 0 {
                let i = (recipe - 1) as usize;
                let process = recipes::process(n.kind, i)
                    .ok_or_else(|| anyhow::anyhow!("Invalid processing recipe"))?;
                ensure!(n.progress < process.ticks, "Invalid processing progress");
                ensure!(
                    n.kind.modes().is_empty()
                        || n.kind == Kind::Furnace
                        || i == n.mode as usize,
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
            // Cables of different tiers do not join: a transformer bridges them.
            let tier = if conduit == Kind::Cable { self.nodes[&p].upgrade } else { 0 };
            let mut queue = VecDeque::from([p]);
            let mut ports = HashSet::new();
            let mut conduits = Vec::new();
            while let Some(p) = queue.pop_front() {
                conduits.push(p);
                for d in DIRS {
                    let q = p + d * size;
                    if let Some(n) = self.nodes.get(&q) {
                        if n.kind == conduit {
                            if (conduit != Kind::Cable || n.upgrade == tier) && visited.insert(q) {
                                queue.push_back(q);
                            }
                        } else if !n.kind.conduit() && !n.kind.structural() {
                            ports.insert(q);
                        }
                    }
                }
            }
            let mut ports: Vec<_> = ports.into_iter().collect();
            ports.sort_by_key(|p| (p.x, p.y, p.z));
            let mut net = Network::new(ports, conduits);
            net.tier = tier;
            nets.push(net);
        }
        nets
    }
    pub fn tick(&mut self, dt: f32, size: i32) {
        for n in self.nodes.values_mut() {
            if n.enabled
                && matches!(
                    n.status,
                    "Processing" | "Generating" | "Coking" | "Smelting" | "Charging" | "Discharging"
                )
            {
                n.phase = (n.phase + dt * (1 + n.upgrade) as f32) % 1000.0;
            }
            n.activity = (n.activity - dt * 2.0).max(0.0);
        }
        self.advance_packets(dt);
        self.accumulator += dt.clamp(0.0, 0.25);
        if self.accumulator < 0.25 {
            return;
        }
        self.accumulator -= 0.25;
        if !self.topology {
            self.power = self.networks(Kind::Cable, size);
            self.items = self.networks(Kind::Pipe, size);
            self.fluids = self.networks(Kind::FluidPipe, size);
            self.item_net.clear();
            for (ni, net) in self.items.iter().enumerate() {
                for c in &net.conduits {
                    self.item_net.insert(*c, ni);
                }
            }
            self.power_net.clear();
            for (ni, net) in self.power.iter().enumerate() {
                for c in net.conduits.iter().chain(&net.ports) {
                    self.power_net.entry(*c).or_insert(ni);
                }
            }
            self.form_multiblocks(size);
            self.form_matrices(size);
            self.validate_packets();
            self.topology = true;
        }
        // Buffers before this tick's work; the difference afterwards is what
        // the factory made and used (production statistics).
        let before: Vec<(IVec3, [u32; RESOURCE_COUNT], [u32; RESOURCE_COUNT], u32)> = self
            .nodes
            .iter()
            .filter(|(_, n)| !matches!(n.kind, Kind::Storage | Kind::Feeder) && !n.kind.conduit())
            .map(|(p, n)| (*p, n.input, n.output, n.stock))
            .collect();
        for (&p, n) in self.nodes.iter_mut() {
            n.status = if n.enabled { "Idle" } else { "Paused" };
            n.flow = 0;
            if n.kind == Kind::Generator && !pile::is_pile(n) {
                pile::cool_idle(n);
            }
            if !n.enabled || !n.kind.generator() {
                if pile::is_pile(n) {
                    pile::cool_idle(n);
                }
                continue;
            }
            let before = n.energy;
            if pile::is_pile(n) {
                let outcome = pile::step(n);
                n.status = outcome.status();
                n.flow = n.energy.saturating_sub(before) as i32;
                if outcome == pile::Outcome::Meltdown {
                    self.meltdowns.push(p);
                }
                continue;
            }
            let biomass = n.kind == Kind::Generator && n.mode == 2;
            let rate = if biomass {
                power::BIOMASS_RATE
            } else if n.kind == Kind::Generator {
                16
            } else {
                32
            };
            if n.energy <= power::GENERATOR_BUFFER - rate {
                if n.burn == 0 {
                    if biomass {
                        let (fuel, portion, energy) = power::BIOMASS_FUEL;
                        if n.input[fuel] >= portion {
                            n.input[fuel] -= portion;
                            n.burn = energy;
                        }
                    } else if n.kind == Kind::Generator {
                        // Best fuel first: coke, coal, bio-ethanol, creosote.
                        if let Some(&(fuel, portion, energy)) = power::COMBUSTION_FUELS
                            .iter()
                            .find(|f| n.input[f.0] >= f.1)
                        {
                            n.input[fuel] -= portion;
                            n.burn = energy;
                        }
                    } else if n.input[13] >= 32 {
                        n.input[13] -= 32;
                        n.burn = 800;
                    }
                }
                let energy = n.burn.min(rate);
                n.burn -= energy;
                n.energy += energy;
                n.flow = energy as i32;
                n.status = if energy > 0 {
                    "Generating"
                } else {
                    "Needs fuel"
                };
            } else {
                n.status = "Buffer full";
            }
        }
        // Count each generator's committed production once, including isolated
        // generators. Network membership is not a source of fresh energy.
        let generated = self.nodes.values().filter(|n| n.kind.generator())
            .map(|n| n.flow.max(0) as u32).sum();
        self.stats.power(generated, 0);
        self.run_multiblocks();
        self.distribute_power();
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
            self.stats.power(0, cost);
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
        self.run_assemblers();
        for (p, input, output, stock) in before {
            let Some(n) = self.nodes.get(&p) else {
                continue;
            };
            for i in 0..RESOURCE_COUNT {
                if n.input[i] < input[i] {
                    self.stats.used(i, input[i] - n.input[i]);
                }
                if n.output[i] > output[i] {
                    self.stats.made(i, n.output[i] - output[i]);
                }
            }
            if n.stock > stock {
                self.stats.kits(n.stock - stock);
            }
        }
        self.stats.tick();
        self.run_extractors(size);
        self.run_distributors(size);
        // Only outputs travel. Material filters make the processing graph acyclic:
        // feeder -> crusher -> furnace -> crate, and feeder -> generator.
        // One unit per source per tick; backpressure retains everything at source.
        let mut moved = HashSet::new();
        // Reused for every launch. Storage sorts after processors, then by
        // distance, so a bus feeds the nearest machine that can take the item
        // instead of walking the whole port list and pathfinding each one.
        let mut near: Vec<(u8, i32, i32, IVec3)> = Vec::new();
        for network in &self.items {
            let net = &network.ports;
            let mut flow = false;
            if net.is_empty() {
                continue;
            }
            let port_count = net.len() as i32;
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
                    near.clear();
                    for (offset, &q) in net.iter().enumerate() {
                        if q == p {
                            continue;
                        }
                        let dest = &self.nodes[&q];
                        if !dest.enabled || dest.kind == Kind::Feeder || !self.container_takes(q, i)
                        {
                            continue;
                        }
                        let reserved = self.reserved.get(&(q, i)).copied().unwrap_or(0);
                        if dest.input[i].saturating_add(reserved) >= CAPACITY {
                            continue;
                        }
                        let dist = (q.x - p.x).abs() + (q.y - p.y).abs() + (q.z - p.z).abs();
                        let tie = (offset as i32 + self.cursor as i32) % port_count;
                        let storage = u8::from(dest.kind == Kind::Storage);
                        near.push((storage, dist, tie, q));
                    }
                    near.sort_unstable_by_key(|row| (row.0, row.1, row.2));
                    for &(_, _, _, q) in &near {
                        let room = CAPACITY.saturating_sub(
                            self.nodes[&q].input[i]
                                + self.reserved.get(&(q, i)).copied().unwrap_or(0),
                        );
                        let amount = available.min(room);
                        if amount == 0 {
                            continue;
                        }
                        if self.packets.len() >= transit::MAX_PACKETS {
                            break 'items;
                        }
                        let Some(path) = transit::find_path(network, p, q, size, None) else {
                            continue;
                        };
                        self.nodes.get_mut(&p).unwrap().output[i] -= amount;
                        *self.reserved.entry((q, i)).or_insert(0) += amount;
                        self.packets.push(transit::Packet {
                            item: i,
                            amount,
                            src: p,
                            dst: q,
                            path,
                            progress: 0.0,
                        });
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

impl Factory {
    /// Positions of piles that melted down since the last call.
    pub fn take_meltdowns(&mut self) -> Vec<IVec3> {
        std::mem::take(&mut self.meltdowns)
    }
}

/// What the look-at panel shows for the assembly under the crosshair.
#[derive(Clone)]
pub struct Probe {
    pub pos: IVec3,
    pub node: Node,
    /// The cable network the assembly is on, if any.
    pub grid: Option<power::NetStats>,
    /// Bricks: the controller of the shell they belong to, when formed.
    pub shell_of: Option<Kind>,
    /// Storage: (stored, capacity, rate) of its energy pool.
    pub pool: Option<(u32, u32, u32)>,
}

pub fn probe(world: &World, cell: IVec3) -> Option<Probe> {
    let pos = anchor(cell, step(world));
    let node = world.factory.nodes.get(&pos)?;
    let shell_of = if node.kind.structural() && node.formed {
        Some(if node.kind == Kind::CokeBrick {
            Kind::CokeOven
        } else {
            Kind::BlastFurnace
        })
    } else {
        None
    };
    Some(Probe {
        pos,
        node: node.clone(),
        grid: world.factory.grid(pos),
        shell_of,
        pool: world.factory.pool_of(pos),
    })
}

/// Review and test fixtures: place a machine without a kit or a player.
pub fn fixture_place(world: &mut World, cell: IVec3, kind: Kind) -> Option<&mut Node> {
    let size = step(world);
    let p = anchor(cell, size);
    let edits: Vec<_> = cells(p, size)
        .into_iter()
        .map(|c| (c, kind.block()))
        .collect();
    world.set_many(&edits);
    world.factory.nodes.insert(p, Node::new(kind));
    world.factory.topology = false;
    world.factory.nodes.get_mut(&p)
}

/// Display colour of a resource, shared by pipe traffic and machine models.
pub fn resource_color(i: usize) -> [f32; 3] {
    match i {
        0 => [0.74, 0.45, 0.22],
        1 => [0.62, 0.70, 0.74],
        2 => [0.62, 0.34, 0.24],
        3 => [0.16, 0.16, 0.18],
        4 => [0.80, 0.92, 0.36],
        5 => [0.72, 0.46, 1.00],
        6 => [0.95, 0.55, 0.25],
        7 => [0.80, 0.86, 0.90],
        8 => [0.72, 0.74, 0.78],
        9 => [0.70, 0.42, 0.22],
        10 => [0.58, 0.64, 0.68],
        11 => [0.52, 0.30, 0.24],
        12 => [0.25, 0.85, 0.72],
        13 => [0.45, 0.82, 0.32],
        14 => [0.78, 0.56, 0.24],
        15 => [0.55, 0.62, 0.72],
        16 => [1.00, 0.62, 0.30],
        17 => [0.40, 0.95, 1.00],
        _ => [0.90, 0.90, 0.90],
    }
}

impl Factory {
    /// Items that lost their route since the last call.
    pub fn take_refunds(&mut self) -> Vec<(usize, u32)> {
        std::mem::take(&mut self.refunds)
    }

    fn release(&mut self, dst: IVec3, item: usize, amount: u32) {
        if let Some(r) = self.reserved.get_mut(&(dst, item)) {
            *r = r.saturating_sub(amount);
            if *r == 0 {
                self.reserved.remove(&(dst, item));
            }
        }
    }

    fn advance_packets(&mut self, dt: f32) {
        if self.packets.is_empty() {
            return;
        }
        let travel = transit::SPEED * dt.clamp(0.0, 0.25);
        let mut i = 0;
        while i < self.packets.len() {
            self.packets[i].progress += travel;
            if self.packets[i].arrived() {
                let packet = self.packets.swap_remove(i);
                self.deliver(packet);
            } else {
                i += 1;
            }
        }
    }

    fn deliver(&mut self, packet: transit::Packet) {
        self.release(packet.dst, packet.item, packet.amount);
        let leftover = match self.nodes.get_mut(&packet.dst) {
            Some(n) if n.accepts(packet.item) => {
                if n.kind == Kind::Storage && n.mode == 2 && n.filter.is_none() {
                    n.filter = Some(packet.item);
                }
                let room = CAPACITY.saturating_sub(n.input[packet.item]);
                let take = packet.amount.min(room);
                n.input[packet.item] += take;
                packet.amount - take
            }
            _ => packet.amount,
        };
        if leftover > 0 {
            self.refunds.push((packet.item, leftover));
        }
    }

    /// Drop packets whose route no longer exists and return their items.
    fn validate_packets(&mut self) {
        let pipes: HashSet<IVec3> = self
            .items
            .iter()
            .flat_map(|n| n.conduits.iter().copied())
            .collect();
        let packets = std::mem::take(&mut self.packets);
        for packet in packets {
            let inner = &packet.path[1..packet.path.len().saturating_sub(1)];
            let alive =
                self.nodes.contains_key(&packet.dst) && inner.iter().all(|c| pipes.contains(c));
            if alive {
                self.packets.push(packet);
            } else {
                self.release(packet.dst, packet.item, packet.amount);
                self.refunds.push((packet.item, packet.amount));
            }
        }
    }

    /// A barrel with nothing stored yet will take the first shipment only.
    /// A second resource already on the way does not sneak in behind it.
    fn container_takes(&self, cell: IVec3, item: usize) -> bool {
        let Some(dest) = self.nodes.get(&cell) else {
            return false;
        };
        if !dest.accepts(item) {
            return false;
        }
        if dest.kind == Kind::Storage && dest.mode == 2 && dest.filter.is_none() {
            let empty = dest.input.iter().all(|n| *n == 0);
            if empty && self.packets.iter().any(|p| p.dst == cell && p.item != item) {
                return false;
            }
        }
        true
    }

    /// Extractor pipes pull from adjacent crates and machine outputs.
    fn run_extractors(&mut self, size: i32) {
        let mut extractors: Vec<IVec3> = self
            .nodes
            .iter()
            .filter(|(_, n)| n.kind == Kind::Pipe && n.mode == 1 && n.enabled)
            .map(|(p, _)| *p)
            .collect();
        extractors.sort_by_key(|p| (p.x, p.y, p.z));
        for p in extractors {
            if self.packets.len() >= transit::MAX_PACKETS {
                return;
            }
            let Some(&ni) = self.item_net.get(&p) else {
                continue;
            };
            let network = &self.items[ni];
            let filter = self.nodes[&p].filter;
            'pull: for d in DIRS {
                let s = p + d * size;
                let Some(src) = self.nodes.get(&s) else {
                    continue;
                };
                if matches!(
                    src.kind,
                    Kind::Pipe | Kind::Cable | Kind::FluidPipe | Kind::Feeder
                ) {
                    continue;
                }
                let from_crate = src.kind == Kind::Storage;
                for i in 0..RESOURCE_COUNT {
                    if filter.is_some_and(|f| f != i) {
                        continue;
                    }
                    let have = if from_crate {
                        src.input[i]
                    } else {
                        src.output[i]
                    };
                    let available = have.min(64);
                    if available == 0 {
                        continue;
                    }
                    let mut destinations: Vec<_> = (0..network.ports.len())
                        .map(|k| network.ports[(k + self.cursor) % network.ports.len()])
                        .filter(|q| *q != s)
                        .collect();
                    destinations.sort_by_key(|q| self.nodes[q].kind == Kind::Storage);
                    for q in destinations {
                        let dest = &self.nodes[&q];
                        if !dest.enabled || dest.kind == Kind::Feeder || !self.container_takes(q, i)
                        {
                            continue;
                        }
                        let room = CAPACITY.saturating_sub(
                            dest.input[i] + self.reserved.get(&(q, i)).copied().unwrap_or(0),
                        );
                        let amount = available.min(room);
                        if amount == 0 {
                            continue;
                        }
                        let Some(path) = transit::find_path(network, s, q, size, Some(p)) else {
                            continue;
                        };
                        let source = self.nodes.get_mut(&s).unwrap();
                        if from_crate {
                            source.input[i] -= amount;
                        } else {
                            source.output[i] -= amount;
                        }
                        *self.reserved.entry((q, i)).or_insert(0) += amount;
                        self.packets.push(transit::Packet {
                            item: i,
                            amount,
                            src: s,
                            dst: q,
                            path,
                            progress: 0.0,
                        });
                        self.nodes.get_mut(&p).unwrap().activity = 1.0;
                        break 'pull;
                    }
                }
            }
        }
    }

    /// One distributor pipe per item network moves a single stack from the
    /// fullest container to the emptiest one that will accept it.
    fn run_distributors(&mut self, size: i32) {
        let mut pipes: Vec<IVec3> = self
            .nodes
            .iter()
            .filter(|(_, n)| n.kind == Kind::Pipe && n.mode == 2 && n.enabled)
            .map(|(p, _)| *p)
            .collect();
        pipes.sort_by_key(|p| (p.x, p.y, p.z));
        let mut served = HashSet::new();
        for pipe in pipes {
            if self.packets.len() >= transit::MAX_PACKETS {
                return;
            }
            let Some(&ni) = self.item_net.get(&pipe) else {
                continue;
            };
            if !served.insert(ni) {
                continue;
            }
            let filter = self.nodes[&pipe].filter;
            let storages: Vec<IVec3> = self.items[ni]
                .ports
                .iter()
                .copied()
                .filter(|q| {
                    self.nodes
                        .get(q)
                        .is_some_and(|n| n.kind == Kind::Storage && n.enabled)
                })
                .collect();
            if storages.len() < 2 {
                continue;
            }
            let mut choice: Option<(usize, IVec3, IVec3, u32)> = None;
            for item in 0..RESOURCE_COUNT {
                if filter.is_some_and(|f| f != item) {
                    continue;
                }
                let mut fullest: Option<(IVec3, u32)> = None;
                let mut emptiest: Option<(IVec3, u32)> = None;
                for &q in &storages {
                    let node = &self.nodes[&q];
                    let incoming = self.reserved.get(&(q, item)).copied().unwrap_or(0);
                    let present = node.input[item] + incoming;
                    if node.input[item] > 0 && fullest.is_none_or(|(_, n)| node.input[item] > n) {
                        fullest = Some((q, node.input[item]));
                    }
                    if node.accepts(item) && emptiest.is_none_or(|(_, n)| present < n) {
                        emptiest = Some((q, present));
                    }
                }
                let (Some((src, have)), Some((dst, there))) = (fullest, emptiest) else {
                    continue;
                };
                if src == dst || have <= there {
                    continue;
                }
                let gap = have - there;
                if choice.is_none_or(|(_, _, _, best)| gap > best) {
                    choice = Some((item, src, dst, gap));
                }
            }
            let Some((item, src, dst, gap)) = choice else {
                continue;
            };
            let incoming = self.reserved.get(&(dst, item)).copied().unwrap_or(0);
            let room = CAPACITY.saturating_sub(self.nodes[&dst].input[item] + incoming);
            let have = self.nodes[&src].input[item];
            let amount = gap.min(64).min(have).min(room);
            if amount == 0 {
                continue;
            }
            let Some(path) = transit::find_path(&self.items[ni], src, dst, size, None) else {
                continue;
            };
            self.nodes.get_mut(&src).unwrap().input[item] -= amount;
            *self.reserved.entry((dst, item)).or_insert(0) += amount;
            self.packets.push(transit::Packet {
                item,
                amount,
                src,
                dst,
                path,
                progress: 0.0,
            });
            let pipe_node = self.nodes.get_mut(&pipe).unwrap();
            pipe_node.activity = 1.0;
            pipe_node.status = "Balancing";
        }
    }
}
