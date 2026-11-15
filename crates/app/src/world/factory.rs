//! Metre-grid factory assemblies. Item and power networks are separate, bounded,
//! deterministic graphs; only conduits carry traffic between machine ports.
use super::{BlockId as B, World, mark_chunk_and_neighbors_dirty};
use super::industry::SPECIES;
use glam::{IVec3, Vec3};
use std::collections::{HashMap, HashSet, VecDeque};
mod extraction;
mod persistence;
#[cfg(all(test, feature = "multiplayer"))]
mod network_tests;
mod catalog;
mod chest_io;
pub mod industry;
pub mod blocks;
pub mod biomatter;
pub mod chips;
pub mod assembler;
pub mod induction;
pub mod multiblock;
pub mod bases;
pub mod overview;
pub mod fuel_order;
pub mod stock_order;
pub mod pile;
pub mod power;
pub mod portable;
pub mod quarry;
pub mod placer;
mod quick_stack;
pub mod sides;
pub mod ports;
pub mod stats;
pub mod recipes;
mod process_table;
pub mod transit;
mod routes;
#[cfg(test)]
mod packing_tests;
#[cfg(test)]
mod authority_tests;
#[cfg(test)]
mod block_tests;
#[cfg(test)]
mod routing_reference;
#[cfg(test)]
mod routing_tests;
pub(crate) use extraction::tick_world;

// Keep the factory API stable while the catalogue owns machine definitions.
#[allow(unused_imports)]
pub use catalog::{Kind, MachineDef, MACHINES};
pub use engine_content::factory_resources::RESOURCE_COUNT;
pub const COKE: usize = 19;
pub const CREOSOTE: usize = 20;
pub const ETHANOL: usize = 21;
pub const GOLD_CONCENTRATE: usize = 22;
pub const GOLD_INGOT: usize = 23;
pub const FEATHERS: usize = 24;
pub use persistence::{FACTORY_FORMAT, LEGACY_RESOURCE_COUNT, resources_in_save};
#[cfg(feature = "multiplayer")]
use persistence::{read_counts, read_gas_counts};
#[cfg(feature = "multiplayer")]
pub(crate) use persistence::read_resources;
pub use engine_content::fabrication::CAPACITY;
const TICK_SECONDS: f32 = 1.0 / engine_content::fabrication::TICKS_PER_SECOND as f32;
pub const MAX_NODES: usize = 1024;
pub(crate) const DIRS: [IVec3; 6] = [
    IVec3::X,
    IVec3::NEG_X,
    IVec3::Y,
    IVec3::NEG_Y,
    IVec3::Z,
    IVec3::NEG_Z,
];

#[derive(Clone)]
#[cfg_attr(feature = "multiplayer", derive(serde::Serialize, serde::Deserialize))]
pub struct Node {
    pub kind: Kind,
    /// Derived from the chip bay's saved production order; not serialized here.
    pub lab_recipe: Option<u8>,
    #[cfg_attr(feature = "multiplayer", serde(deserialize_with = "read_resources"))]
    pub input: [u32; RESOURCE_COUNT],
    #[cfg_attr(feature = "multiplayer", serde(deserialize_with = "read_resources"))]
    pub output: [u32; RESOURCE_COUNT],
    /// Saved from format 11. Whole blocks use item counts, not resource units.
    pub blocks: blocks::Buffers,
    #[cfg_attr(feature = "multiplayer", serde(default))]
    pub placer: placer::Buffer,
    pub energy: u32,
    pub burn: u32,
    pub progress: u32,
    pub recipe: Option<usize>,
    pub enabled: bool,
#[cfg_attr(feature="multiplayer", serde(deserialize_with="crate::multiplayer::text::string"))]
    pub status: crate::world::interaction::Text,
    pub mode: u32,
    pub upgrade: u32,
    pub filter: Option<usize>,
    pub phase: f32,
    pub activity: f32,
    pub water: u32,      // 512 units per cubic metre at every voxel scale
    pub excavation: u32, // next cell in the drill shaft
    pub(super) work_clock: f32,
    pub draining: bool,
    /// Signed power flow last tick in units (= kW): positive while a
    /// generator produces or a battery charges, negative while drawing.
    pub flow: i32,
    /// Multiblock controllers and bricks: part of a complete shell.
    pub formed: bool,
    /// Controllers: (bricks found, bricks needed) for the best orientation.
    pub structure: (u16, u16),
    /// Controllers: outward face of the oven door / tuyere or direction of the quarry pit.
    pub facing: IVec3,
    /// Saved from format 10. Assembler: recipe index + 1 (0 = none).
    /// Battery: I/O limit in kW (0 = the cell's full rate).
    /// Mining drill or quarry: depth in metres (0 = 24 m).
    /// Supported processors: tagged stock reserve, or legacy fuel batch.
    pub target: u32,
    /// Saved from format 10. Assembler: finished items that are not
    /// resources (machine kits), waiting to be collected.
    pub stock: u32,
    /// Induction ports: the port that holds the formed matrix's energy.
    pub pool: Option<IVec3>,
    /// Induction matrix master: capacity (units) and transfer rate (units per tick).
    pub store_cap: u32,
    pub store_rate: u32,
    /// Item face roles, two bits per `DIRS` entry: 0 both, 1 in, 2 out, 3 off.
    /// Factory format 13. Conduits stay 0; closed conduit faces stay in `target`.
    #[cfg_attr(feature = "multiplayer", serde(default))]
    pub faces: u32,
    #[cfg_attr(feature = "multiplayer", serde(default, deserialize_with = "read_gas_counts"))]
    pub exhaust: [u32; SPECIES],
    #[cfg_attr(feature = "multiplayer", serde(default, deserialize_with = "read_gas_counts"))]
    pub residue: [u32; SPECIES],
    #[cfg_attr(feature = "multiplayer", serde(default))]
    pub filter_credit: u32,
    #[cfg_attr(feature = "multiplayer", serde(default))]
    pub gas_vent: bool,
    #[cfg_attr(feature = "multiplayer", serde(default = "industry::default_fuel"))]
    pub fuel_resource: u32,
    #[cfg_attr(feature = "multiplayer", serde(default))]
    pub ruptured: bool,
    #[cfg_attr(feature = "multiplayer", serde(default))]
    pub gas_vent_side: u8,
    #[cfg_attr(feature = "multiplayer", serde(default))]
    pub gas_vent_species: u8,
    #[cfg_attr(feature = "multiplayer", serde(default))]
    pub separator_water_mg: u32,
}
impl Node {
    fn new(kind: Kind) -> Self {
        Self {
            kind,
            lab_recipe: None,
            input: [0; RESOURCE_COUNT],
            output: [0; RESOURCE_COUNT],
            blocks: blocks::Buffers::default(),
            placer: placer::Buffer::default(),
            energy: 0,
            burn: 0,
            progress: 0,
            recipe: None,
            enabled: kind != Kind::Quarry,
            status: "Ready",
            mode: if matches!(kind, Kind::Storage | Kind::Quarry) { 1 } else { 0 },
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
            faces: 0,
            exhaust: [0; SPECIES],
            residue: [0; SPECIES],
            filter_credit: 0,
            gas_vent: false,
            fuel_resource: industry::default_fuel(),
            ruptured: false,
            gas_vent_side: 0,
            gas_vent_species: 0,
            separator_water_mg: 0,
        }
    }
}

/// Fixed-size counts; serde/default implementations also support more than 32 kinds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KindCounts(pub [u32; Kind::COUNT]);
impl Default for KindCounts { fn default()->Self {Self([0;Kind::COUNT])} }
impl std::ops::Deref for KindCounts {type Target=[u32;Kind::COUNT];fn deref(&self)->&Self::Target{&self.0}}
impl std::ops::DerefMut for KindCounts {fn deref_mut(&mut self)->&mut Self::Target{&mut self.0}}
#[derive(Default)]
#[cfg_attr(feature = "multiplayer", derive(serde::Serialize, serde::Deserialize))]
pub struct Factory {
    #[cfg_attr(feature = "multiplayer", serde(skip))]
    drill_delivery: bool,
    #[cfg_attr(feature = "multiplayer", serde(skip))]
    furnace_ingot: bool,
    /// Consumed into the persistent workshop record after a real pipe delivery.
    #[cfg_attr(feature = "multiplayer", serde(skip))]
    pub(crate) workshop_delivery: bool,
    pub nodes: crate::fast_hash::FastMap<IVec3, Node>,
    pub kits: KindCounts,
    /// Stable machine groups for dashboards; rebuilt from `nodes`, never saved.
    #[cfg_attr(feature = "multiplayer", serde(skip))]
    pub bases: bases::Bases,
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
    /// Live reserve destinations, derived on the factory clock and shown to guests.
    #[cfg_attr(feature = "multiplayer", serde(default, skip_serializing_if = "Vec::is_empty"))]
    stock_receivers: Vec<(IVec3, IVec3)>,
    /// Route trees for `items`; cleared whenever the networks are rebuilt.
    #[cfg_attr(feature = "multiplayer", serde(skip))]
    routes: routes::Routes,
    /// Drill and pump positions, reused by every fixed step.
    #[cfg_attr(feature = "multiplayer", serde(skip))]
    machine_scratch: Vec<IVec3>,
    /// Tests: route with the uncached reference pass instead.
    #[cfg(test)]
    #[cfg_attr(feature = "multiplayer", serde(skip))]
    reference_routing: bool,
    /// Measured state of each power network, parallel to `power`.
    power_stats: Vec<power::NetStats>,
    /// Cable or port -> index into `power`.
    power_net: HashMap<IVec3, usize>,
    /// Factorio-style production and power history.
    pub stats: stats::Production,
    /// The same history per base in `bases`; runtime only.
    #[cfg_attr(feature = "multiplayer", serde(skip))]
    pub by_base: stats::ByBase,
    /// Player-designed controllers and their library.
    #[cfg_attr(feature="multiplayer", serde(skip))]
    pub chips: chips::Bay,
}

#[derive(Clone, Default)]
pub struct View {
    pub workshop_alerts: Vec<WorkshopAlert>,
    pub air: [f32; SPECIES],
    pub contamination_mg: u64,
    pub wind: [f32; 2],
    pub workshop: Option<super::workshop::Progress>,
    pub orbital_cell: bool,
    pub orbital_delivery: Option<(u32, IVec3)>,
    pub meridian_terminal: Option<IVec3>,
    pub iron_line: Option<&'static str>,
    pub fabrication: std::sync::Arc<chips::production::Production>,
    pub fabrication_site: Option<IVec3>,
    pub fabrication_node: Option<Node>,
    pub habitats: super::habitats::View,
    pub kits: KindCounts,
    pub bench: bool,
    pub focused: Option<(IVec3, Node)>,
    pub cells_per_metre: i32,
    /// Cable network of the focused machine.
    pub focused_grid: Option<power::NetStats>,
    /// Faces with a real connection on the focused conduit.
    pub focused_links: u8,
    /// Energy pool (stored, capacity, rate) of a focused storage assembly.
    pub focused_pool: Option<(u32, u32, u32)>,
    /// Factory health: what every assembly is doing, and the main limit.
    pub health: overview::Summary,
    pub outposts: Vec<overview::Outpost>,
    pub fuel_order: Option<fuel_order::View>,
    pub stock_reserve: Option<stock_order::View>,
    /// Production totals and the span (s) they cover: last minute, last hour.
    pub minute: (stats::Sample, f32),
    pub hour: (stats::Sample, f32),
    /// Generation and draw per second over the last minute (units).
    pub power_history: Vec<(u64, u64)>,
    pub count: usize,
    /// Chip library snapshot and the focused machine's controller.
    pub chips: Option<std::sync::Arc<engine_chip::library::Library>>,
    pub controller: Option<chips::Diagnostics>,
    pub bench_chips: bool,
    /// Chip bay clock (1/120 s ticks), for lot progress.
    pub chips_now: u64,
}

#[derive(Clone, Copy)]
pub struct WorkshopAlert {
    pub source: IVec3,
    pub target: IVec3,
    pub kind: Kind,
    pub health: overview::Health,
}

#[derive(Clone, Copy)]
#[cfg_attr(feature = "multiplayer", derive(serde::Serialize, serde::Deserialize))]
pub enum Action {
    ToolWorkshop(super::tool_workshop::Action),
    Resonance(super::adventure::resonance::Action),
    Habitat(super::habitats::Action),
    /// Machine-specific target, such as quarry depth or assembler recipe.
    SetTarget(IVec3, u32),
    Craft(Kind),
    Select(Kind),
    CraftSword(u32),
    SelectSword(u32),
    CookRations,
    EatRation,
    Load(IVec3),
    Collect(IVec3),
    /// Move `amount` units between an input buffer and the pack.
    /// `deposit` sends pack → input. `u32::MAX` means the whole compartment.
    BufferMove(IVec3, usize, u32, bool),
    ContainerClick(IVec3, usize, bool, bool),
    Toggle(IVec3),
    /// Close or reopen one face of a conduit (face index into `DIRS`).
    ToggleSide(IVec3, u8),
    Upgrade,
    SetRecipe(IVec3, u32),
    SetFilter(IVec3, Option<usize>),
    UpgradeMachine(IVec3),
    Wand,
    SetWandSlot(u32, u32),
    SwapWandSlots(u32, u32),
    /// Replace the held wand's deck with a rune deck packed by `adventure::runes::pack`.
    InscribeWand(u64),
    /// Begin a ritual circle from a phrase packed by `adventure::runes::pack_phrase`.
    DrawCircle(u64),
    /// Bind a phrase packed by `adventure::runes::pack_program` to the held wand for the
    /// native rune path; `u64::MAX` releases the binding.
    BindRunes(u64),
    SetWandMode(u32),
    Drain(IVec3),
    /// Fit a material into a tool part. Parameters are `Part`, `Material`.
    FitToolPart(u8, u8),
    ToggleToolModifier(u8),
    ClearToolParts,
    /// Move pack resources a storage container already holds into it.
    QuickStack(IVec3),
    /// Item face role on a machine. Face is a `DIRS` index; role is 0 both, 1 in, 2 out, 3 off.
    SetFace(IVec3, u8, u8),
    PackWaste(IVec3),
    VentGas(IVec3),
    GasOutlet(IVec3, u8, u8),
    FillFuelCell(IVec3),
    RepairGas(IVec3),
    CleanPollution(IVec3),
    SetQuarryDirection(IVec3, u8),
    SetPlacer(IVec3, bool),
    FlushSediment(IVec3),
    InstallMithril(IVec3),
}

#[cfg_attr(feature = "multiplayer", derive(serde::Serialize, serde::Deserialize))]
struct Network {
    ports: Vec<IVec3>,
    conduits: Vec<IVec3>,
    /// Cable tier (the `upgrade` of every cable in it); 0 for pipes.
    tier: u32,
    /// Built once with the network. Path searches borrow this instead of
    /// allocating a new set for every ingot.
    pipes: HashSet<IVec3>,
    /// Directed (conduit, neighbour) pairs whose shared face a conduit closed.
    closed: HashSet<(IVec3, IVec3)>,
    /// Item pipe edges that must not pull or push out of the machine.
    #[cfg_attr(feature = "multiplayer", serde(default))]
    no_extract: HashSet<(IVec3, IVec3)>,
    /// Item pipe edges that must not push into the machine.
    #[cfg_attr(feature = "multiplayer", serde(default))]
    no_insert: HashSet<(IVec3, IVec3)>,
}
impl Network {
    fn new(ports: Vec<IVec3>, conduits: Vec<IVec3>) -> Self {
        let pipes = conduits.iter().copied().collect();
        Self {
            ports,
            conduits,
            pipes,
            closed: HashSet::new(),
            no_extract: HashSet::new(),
            no_insert: HashSet::new(),
            tier: 0,
        }
    }
}

/// Fraction of a water-port machine's buffer that is full.
pub fn tank_level(n: &Node) -> f32 {
    n.water as f32 / extraction::WATER_CAPACITY as f32
}

pub fn step(world: &World) -> i32 {
    (1.0 / world.voxel_scale.meters()).round() as i32
}

/// Samples environmental inputs when the factory is ready to use them.
pub(crate) fn tick_power(world: &mut World, dt: f32) {
    let size = step(world);
    let mut passive = HashMap::new();
    if world.factory.accumulator + dt.clamp(0.0, TICK_SECONDS) >= TICK_SECONDS {
        for (&p, node) in &world.factory.nodes {
            if node.enabled && node.kind == Kind::Generator && power::passive_rating(node.mode).is_some() {
                passive.insert(p, power::sample_passive(world, p, size, node.mode));
            }
        }
    }
    world.factory.tick_with_power(dt, size, |p| passive.get(&p).copied());
    if std::mem::take(&mut world.factory.drill_delivery) {
        world.goals.note_production(super::goals::Event::DrillDelivered);
    }
    if std::mem::take(&mut world.factory.furnace_ingot) {
        world.goals.note_production(super::goals::Event::FurnaceIngot);
    }
}

impl Factory {
    pub(super) fn input_reserved(&self, pos: IVec3, resource: usize) -> u32 {
        self.reserved.get(&(pos, resource)).copied().unwrap_or(0)
    }
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
pub(crate) fn at_bench(world: &World, player: Vec3) -> bool {
    world
        .factory
        .bench
        .is_some_and(|p| near(world, p, player) && world.get(p) == B::Workbench)
}
pub(super) fn fabrication_site(world: &World, player: Vec3) -> Option<IVec3> {
    world.factory.focus.filter(|p| world.factory.nodes.get(p)
        .is_some_and(|n| n.kind == Kind::Fabricator) && near(world, *p, player))
        .or_else(|| world.factory.nodes.iter()
            .filter(|(p, n)| n.kind == Kind::Fabricator && near(world, **p, player))
            .map(|(p, _)| *p).min_by_key(|p| (p.x, p.y, p.z)))
}

pub fn view(world: &World, player: Vec3, production: bool, logistics: bool) -> View {
    let lab = fabrication_site(world, player);
    let cfg = world.sample_config();
    use engine_worldgen::meridian_terminal as terminal;
    View {
        workshop_alerts: if logistics { workshop_alerts(world, player) } else { Vec::new() },
        air: world.industry.concentration(player.floor().as_ivec3(), world.voxel_scale.meters()),
        contamination_mg: world.factory.focus.map_or(0, |p| world.industry.contamination(p, world.voxel_scale.meters())),
        wind: world.industry.wind,
        workshop: super::workshop::progress(world),
        orbital_cell: super::orbital::cargo::cell_ready(world),
        orbital_delivery: super::orbital::cargo::cell_delivery(world),
        meridian_terminal: terminal::enabled(&cfg).then(|| {
            (Vec3::new(terminal::X, terminal::floor(&cfg), terminal::Z) * cfg.inv_voxel_meters())
                .floor().as_ivec3()
        }),
        iron_line: None,
        habitats: super::habitats::view(world, player * world.voxel_scale.meters()),
        kits: world.factory.kits,
        bench: at_bench(world, player),
        count: world.factory.nodes.len(),
        focused: world
            .factory
            .focus
            .filter(|p| near(world, *p, player))
            .and_then(|p| world.factory.nodes.get(&p).cloned().map(|n| (p, n))),
        cells_per_metre: step(world),
        focused_grid: world.factory.focus.and_then(|p| world.factory.grid(p)),
        focused_links: world.factory.focus.filter(|p| near(world, *p, player)).map_or(0, |p| {
            let size = step(world);
            (0..6).fold(0, |mask, side| mask | (u8::from(world.factory.conduit_join(p, side, size)) << side))
        }),
        focused_pool: world.factory.focus.and_then(|p| world.factory.pool_of(p)),
        health: if production { overview::summarize(&world.factory) } else { Default::default() },
        outposts: Vec::new(),
        fuel_order: if production { fuel_order::view(world, player) } else { None },
        stock_reserve: world.factory.focus.filter(|&p| near(world, p, player))
            .and_then(|p| stock_order::view(&world.factory, p)),
        minute: if production { world.factory.stats.window(false) } else { Default::default() },
        hour: if production { world.factory.stats.window(true) } else { Default::default() },
        power_history: if production { world
            .factory
            .stats
            .seconds
            .iter()
            .map(|s| (s.generated, s.drawn))
            .collect() } else { Vec::new() },
        chips: Some(world.factory.chips.library.clone()),
        fabrication: world.factory.chips.production.clone(),
        fabrication_site: lab,
        fabrication_node: lab.and_then(|p| world.factory.nodes.get(&p).cloned()),
        controller: world.factory.focus.and_then(|p| world.factory.chips.diagnostics(p)),
        bench_chips: at_bench(world, player),
        chips_now: world.factory.chips.now(),
    }
}

fn workshop_alerts(world: &World, player: Vec3) -> Vec<WorkshopAlert> {
    const ALERT_LIMIT: usize = 8;
    let mut alerts = Vec::with_capacity(ALERT_LIMIT);
    let rank = |a: &(f32, IVec3, Kind, overview::Health), b: &(f32, IVec3, Kind, overview::Health)| {
        a.0.total_cmp(&b.0).then_with(|| a.1.to_array().cmp(&b.1.to_array()))
    };
    for (&source, node) in &world.factory.nodes {
        let health = overview::classify(node);
        if health.is_problem() {
            let candidate = (source.as_vec3().distance_squared(player), source, node.kind, health);
            let index = alerts.partition_point(|alert| rank(alert, &candidate).is_lt());
            if index < ALERT_LIMIT {
                if alerts.len() == ALERT_LIMIT { alerts.pop(); }
                alerts.insert(index, candidate);
            }
        }
    }
    alerts.into_iter().map(|(_, source, kind, health)| {
        let factory = &world.factory;
        let node = &factory.nodes[&source];
        // Delivered packets are removed immediately. A live shipment still
        // gives us a proven route and reserved capacity at its receiver.
        let target = if node.status == "Output blocked" {
            factory.packets.iter().filter(|p| {
                p.src == source && node.output.get(p.item).is_some_and(|&n| n != 0)
                    && factory.nodes.get(&p.dst).is_some_and(|receiver| {
                        let reserved = factory.reserved.get(&(p.dst, p.item)).copied().unwrap_or(0);
                        receiver.input[p.item].saturating_add(reserved) >= CAPACITY
                    })
            }).min_by_key(|p| (p.path.len(), p.dst.to_array())).map_or(source, |p| p.dst)
        } else {
            source
        };
        WorkshopAlert { source, target, kind, health }
    }).collect()
}

pub fn action(world: &mut World, player: Vec3, action: Action) -> Option<B> {
    match action {
        Action::ToolWorkshop(a) => {
            if at_bench(world, player) { super::tool_workshop::action(world, a); }
            else { world.expedition.say("Workbench Required"); }
        }
        Action::ContainerClick(p, item, right, shift) => {
            if !near(world, p, player) || item >= RESOURCE_COUNT { return None; }
            let Some(node) = world.factory.nodes.get_mut(&p).filter(|n| n.kind == Kind::Storage) else { return None };
            use super::inventory::{Item, ItemStack};
            let kind = Item::Resource(item as u8);
            if shift {
                let count = node.input[item] / 64;
                let (inserted, _) = world.inventory.try_insert_pack_first(ItemStack::new(kind, count));
                node.input[item] -= inserted * 64;
            } else if let Some(mut cursor) = world.inventory.cursor {
                if cursor.item == kind && node.accepts(item) {
                    let reserved = world.factory.reserved.get(&(p, item)).copied().unwrap_or(0);
                    let room = CAPACITY.saturating_sub(node.input[item].saturating_add(reserved)) / 64;
                    let n = cursor.count.min(room).min(if right { 1 } else { u32::MAX });
                    node.input[item] += n * 64;
                    cursor.count -= n;
                    world.inventory.cursor = (cursor.count > 0).then_some(cursor);
                }
            } else {
                let count = (node.input[item] / 64).min(world.inventory.stack_limit(kind));
                let n = if right { count.div_ceil(2) } else { count };
                if n > 0 {
                    node.input[item] -= n * 64;
                    world.inventory.cursor = Some(ItemStack::new(kind, n));
                }
            }
            world.sync_from_inventory();
        }
        Action::Resonance(action) => {
            let bench = at_bench(world, player);
            super::adventure::resonance::action(world, bench, action);
        }
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
            if !super::adventure::wands::fits_instrument(spell, world.adventure.loadout.mode) {
                world.expedition.say("Concentration requires a grimoire");
            } else {
                world.adventure.set_wand_slot(slot, spell);
            }
        }
        Action::SwapWandSlots(a, b) => {
            world.adventure.swap_wand_slots(a, b);
        }
        Action::DrawCircle(phrase) => {
            let feet = player * world.voxel_scale.meters();
            let mut adventure = std::mem::take(&mut world.adventure);
            let outcome = adventure.draw_circle(world, feet, phrase);
            world.adventure = adventure;
            match outcome {
                Ok(seconds) => world.expedition.say(format!("Ritual: {seconds:.0}s")),
                Err(reason) => world.expedition.say(reason),
            }
        }
        Action::BindRunes(word) => {
            let message = match world.adventure.bind_runes(word) {
                Ok(message) | Err(message) => message,
            };
            world.expedition.say(message);
        }
        Action::InscribeWand(deck) => {
            if let Err(reason) = world.adventure.inscribe_wand(deck) {
                world.expedition.say(reason);
            }
        }
        Action::SetWandMode(mode) => {
            if mode <= 3 && !super::adventure::wands::is_grimoire(world.adventure.loadout.mode) {
                world.adventure.loadout.mode = mode;
            }
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
        Action::ToggleSide(p, side) => {
            if !near(world, p, player) {
                world.expedition.say("Move within 7 metres of the conduit.");
                return None;
            }
            sides::toggle_side(world, p, side);
        }
        Action::SetFace(p, face, role) => {
            if !near(world, p, player) {
                world.expedition.say("Move within 7 metres of the machine.");
                return None;
            }
            sides::set_face(world, p, face, role);
        }
        Action::CleanPollution(p) => { industry::clean(world, p, player); }
        Action::InstallMithril(p) => {
            if world.ca_view.is_some() || !world.rules.mode.interacts()
                || world.adventure.is_dead() || !near(world, p, player) {
                return None;
            }
            if !world.factory.nodes.get(&p).is_some_and(|n| {
                n.kind == Kind::Battery && power::accepts_mithril(n.upgrade)
            }) {
                world.expedition.say("Upgrade to Bronze Cells");
                return None;
            }
            if super::orbital::cargo::install_cell(world) {
                world.factory.nodes.get_mut(&p).unwrap().upgrade = power::MITHRIL_TIER;
                world.expedition.say("Mithril Cell Installed · 64 kWh");
            } else {
                world.expedition.say("Orbital Mithril Delivery Required");
            }
        }
        Action::Load(p)
        | Action::Collect(p)
        | Action::BufferMove(p, _, _, _)
        | Action::QuickStack(p)
        | Action::Toggle(p)
        | Action::SetRecipe(p, _)
        | Action::SetFilter(p, _)
        | Action::UpgradeMachine(p)
        | Action::SetTarget(p, _)
        | Action::SetQuarryDirection(p, _)
        | Action::SetPlacer(p, _)
        | Action::FlushSediment(p)
        | Action::PackWaste(p)
        | Action::RepairGas(p)
        | Action::VentGas(p)
        | Action::GasOutlet(p, _, _)
        | Action::FillFuelCell(p)
        | Action::Drain(p) => {
            if !near(world, p, player) {
                world.expedition.say("Move within 7 metres of the machine.");
                return None;
            }
            let cell_size = step(world);
            if let Some(n) = world.factory.nodes.get_mut(&p) {
                match action {
                    Action::RepairGas(_) if n.ruptured => {
                        if world.rules.mode.creative_tools() || world.expedition.pay(&crate::world::industry::config::CONFIG.repair_cost) {n.ruptured=false;n.gas_vent=false;n.status="Ready";}
                        else {world.expedition.say("Needs repair materials");}
                    }
                    Action::PackWaste(_) => { industry::pack_waste(n, &mut world.inventory); }
                    Action::VentGas(_) if industry::port(n.kind) => {
                        n.gas_vent = n.ruptured || !n.gas_vent;
                    }
                    Action::GasOutlet(_, side, species)
                        if industry::port(n.kind) && side <= 6 && usize::from(species) <= SPECIES =>
                    {
                        n.gas_vent_side = side;
                        n.gas_vent_species = species;
                    }
                    Action::FillFuelCell(_) if n.kind == Kind::GasTank || industry::processor(n) => {
                        if let Err(message) = industry::fill_fuel_cell(n, &mut world.inventory) {
                            world.expedition.say(message);
                        }
                    }
                    Action::SetRecipe(_, _) if fuel_order::is_batch(n)
                        || stock_order::read(n.kind, n.mode, n.target).is_some() => {
                        world.expedition.say("Clear Production Order First");
                    }
                    Action::SetRecipe(_, mode)
                        if (n.kind == Kind::Furnace && (9..12).contains(&mode))
                            || (mode as usize) < n.kind.modes().len() =>
                    {
                        if n.kind == Kind::Generator && n.mode != mode && n.burn > 0 {
                            world.expedition.say("Finish burning the current fuel before switching modes.");
                            return None;
                        }
                        let footprint_changed = n.kind.excavates() && n.mode != mode;
                        if placer::sluice(n) {
                            if n.mode == mode { return None; }
                            n.placer.cancel_batch();
                        }
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
                        // Mode switches retain heat and paid assembler work.
                        if !matches!(n.kind, Kind::Generator | Kind::Battery | Kind::Assembler) {
                            n.progress = 0;
                        }
                        // The cursor indexes a footprint that the mode just changed.
                        if footprint_changed {
                            n.excavation = 0;
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
                        if n.kind != Kind::Feeder && item < RESOURCE_COUNT =>
                    {
                        use super::inventory::resources;
                        if deposit {
                            if n.accepts(item) {
                                let reserved = world.factory.reserved.get(&(p, item)).copied().unwrap_or(0);
                                let room = CAPACITY.saturating_sub(n.input[item].saturating_add(reserved));
                                let moved =
                                    amount.min(room).min(world.expedition.resources[item]);
                                if moved > 0 && resources::exchange(&mut world.inventory, &[(item, moved)], &[]) {
                                    n.input[item] += moved;
                                    if n.kind == Kind::Storage && n.mode == 2 && n.filter.is_none() {
                                        n.filter = Some(item);
                                    }
                                }
                            } else {
                                world
                                    .expedition
                                    .say("Input does not accept this item");
                            }
                        } else {
                            let moved = resources::collect(&mut world.inventory, item, amount.min(n.input[item]));
                            n.input[item] -= moved;
                            if n.kind == Kind::Storage && n.mode == 2 && n.input.iter().all(|count| *count == 0) {
                                n.filter = None;
                            }
                        }
                        world.sync_from_inventory();
                        return None;
                    }
                    Action::SetFilter(_, filter)
                        if matches!(n.kind, Kind::Feeder | Kind::Storage | Kind::Pipe)
                            && filter.is_none_or(|i| i < RESOURCE_COUNT) =>
                    {
                        n.filter = filter
                    }
                    Action::SetPlacer(_, enabled) if n.kind == Kind::Quarry => {
                        if n.placer.enabled != enabled {
                            n.placer.enabled = enabled;
                            n.excavation = 0;
                        }
                    }
                    Action::FlushSediment(_) if matches!(n.kind, Kind::Quarry | Kind::Washer)
                        && !n.enabled =>
                    {
                        if !placer::flush(n, cell_size) {
                            world.expedition.say("Output Full");
                        }
                    }
                    Action::SetQuarryDirection(_, direction) if n.kind == Kind::Quarry => {
                        if let Some(&(_, facing)) = quarry::DIRECTIONS.get(direction as usize)
                            && facing != n.facing
                        {
                            n.facing = facing;
                            n.excavation = 0;
                        }
                    }
                    Action::SetTarget(_, target) => match n.kind {
                        _ if target == 0 && stock_order::read(n.kind, n.mode, n.target).is_some() => {
                            n.target = 0;
                            n.enabled = false;
                            world.expedition.say("Reserve Cleared · Machine Paused");
                        }
                        _ if stock_order::read(n.kind, n.mode, target).is_some() => {
                            if fuel_order::is_batch(n) && n.target > 1 {
                                world.expedition.say("Clear Fuel Request First");
                            } else {
                                n.target = target;
                                n.enabled = true;
                                world.expedition.say("Stock Reserve Set");
                            }
                        }
                        Kind::Cultivator if target == 0 => {
                            if n.target != 0 {
                                n.target = 0;
                                n.enabled = false;
                                world.expedition.say("Fuel Request Cleared");
                            }
                        }
                        Kind::Cultivator if fuel_order::valid_request(target) => {
                            if n.target > 1 || n.recipe.is_some() || n.progress != 0
                                || n.output[ETHANOL] != 0
                                || world.factory.packets.iter().any(|packet| packet.src == p && packet.item == ETHANOL)
                            {
                                world.expedition.say("Fermenter Busy");
                            } else {
                                n.mode = fuel_order::MODE;
                                n.target = target;
                                n.enabled = true;
                                world.expedition.say("Fuel Requested");
                            }
                        }
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
                        k if k.excavates() && target <= quarry::MAX_DEPTH_M as u32 => {
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
                            } else if n.kind == Kind::GasTank {
                                world.expedition.say(format!(
                                    "Tank reinforced: {} · holds {:.0} g.",
                                    industry::tank_tier_name(n.upgrade),
                                    industry::gas_capacity(n) as f32 / 1000.0
                                ));
                            } else {
                                world.expedition.say("Drive upgraded: faster processing with proportionally higher power demand.");
                            }
                        } else {
                            world.expedition.say("Missing drive upgrade ingredients.");
                        }
                    }
                    Action::Drain(_) if n.kind == Kind::GasGenerator => {
                        if let Err(message) = industry::turbine::bottle(n, &mut world.inventory) {
                            world.expedition.say(message);
                        }
                    }
                    Action::Drain(_) if n.water == 0 && n.separator_water_mg > 0 => {
                        if industry::replace_bucket(&mut world.inventory, 0, n.separator_water_mg) {
                            n.separator_water_mg = 0;
                            n.status = "Drained";
                        } else {
                            world.expedition.say("Needs an empty bucket for the remaining water");
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
                        let mut reserved = [0; RESOURCE_COUNT];
                        for i in 0..RESOURCE_COUNT {
                            if !n.accepts(i) {
                                continue;
                            }
                            let dst = if n.kind == Kind::Feeder {
                                &mut n.output[i]
                            } else {
                                &mut n.input[i]
                            };
                            let incoming = if n.kind == Kind::Feeder { 0 } else {
                                world.factory.reserved.get(&(p, i)).copied().unwrap_or(0)
                            };
                            reserved[i] = incoming;
                            let amount = world.expedition.resources[i]
                                .min(CAPACITY.saturating_sub(dst.saturating_add(incoming)));
                            world.expedition.resources[i] -= amount;
                            *dst += amount;
                        }
                        blocks::load(&mut world.inventory, n, cell_size, &reserved);
                        world.expedition.say("Loaded matching supplies. Item pipes feed connected machines automatically.");
                    }
                    Action::Collect(_) if n.kind == Kind::Assembler && n.stock > 0 => {
                        if let Some(item) = assembler::stock_item(n) {
                            let (count, _) = world.inventory.try_insert(super::inventory::ItemStack::new(
                                item, n.stock));
                            n.stock -= count;
                            world.expedition.say(format!("Collected {count} × {}.", item.name()));
                        }
                    }
                    Action::Collect(_) if n.kind == Kind::Cultivator && n.target > 1 => {
                        world.expedition.say("Fuel Request Active · Collect From Storage");
                    }
                    Action::Collect(_) => {
                        // Collect also unloads inputs. Pausing first lets the player safely reconfigure a line.
                        for i in 0..RESOURCE_COUNT {
                            for src in [&mut n.input[i], &mut n.output[i]] {
                                let amount = super::inventory::resources::collect(
                                    &mut world.inventory, i,
                                    (*src).min(64_000_000 - world.expedition.resources[i]));
                                world.expedition.resources[i] += amount;
                                *src -= amount;
                            }
                        }
                        blocks::collect(&mut world.inventory, n);
                        world.expedition.say("Collected what fits. Remaining supplies stay in the machine.");
                    }
                    Action::QuickStack(_) => {
                        let reserved = &world.factory.reserved;
                        quick_stack::quick_stack_reserved(n, &mut world.inventory, &mut world.expedition.resources,
                            |i| reserved.get(&(p, i)).copied().unwrap_or(0));
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
    let item = portable.map(super::inventory::Item::PowerKit)
        .unwrap_or(super::inventory::Item::Block(kind.block()));
    if world.factory.nodes.len() >= MAX_NODES || world.inventory.count_item(item) == 0 {
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
    if (lo.cmplt(z) & hi.cmpgt(a)).all() || body.iter().any(|&c| {
        world.get(c) != B::Air || super::orbital::cargo::security::protected(world, c)
    }) {
        world
            .expedition
            .say("Assembly needs a clear 1 × 1 × 1 metre space outside your body.");
        return true;
    }
    let mut edits: Vec<_> = body.iter().map(|c| (*c, b)).collect();
    world.set_many(&edits);
    if body.iter().any(|c| world.get(*c) != b) || !world.inventory.remove_item(item, 1) {
        // Clear any partial casing. The kit is paid only for a completed assembly.
        edits.retain(|(c, _)| world.get(*c) == b);
        for (_, block) in &mut edits {
            *block = B::Air;
        }
        world.set_many(&edits);
        world
            .expedition
            .say("Assembly could not be placed; kit retained.");
        return true;
    }
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
    super::containers::flush(world);
    let cache_id = world.caches.iter().position(|c| c.cells == [p]);
    // Publish charge to individual cells before any topology edit. Removing a
    // port cannot remove the pool, and removing a cell takes only its charge.
    world.factory.scatter_matrix_energy();
    world.factory.topology = false;
    let Some(node) = world.factory.nodes.remove(&p) else {
        return false;
    };
    if node.ruptured || node.exhaust.iter().chain(&node.residue).any(|&v| v > 0) {
        world.factory.nodes.insert(p, node);
        world.expedition.say("Vent gas, pack waste and repair before dismantling.");
        return true;
    }
    if node.water > 0 || node.separator_water_mg > 0 {
        world.factory.nodes.insert(p, node);
        world.factory.form_matrices(size);
        world
            .expedition
            .say("Drain this assembly from its machine panel before packing it up.");
        return true;
    }
    if node.placer.has_feed() {
        world.factory.nodes.insert(p, node);
        world.factory.form_matrices(size);
        world.expedition.say("Sediment Buffer Occupied");
        return true;
    }
    if node.kind == Kind::InductionCell && node.energy > induction::CELL_CAPACITY {
        world.factory.nodes.insert(p, node);
        world.factory.form_matrices(size);
        world.expedition.say("Restore matrix capacity before packing this cell: excess legacy charge is held offline.");
        return true;
    }
    let portable = match portable::PowerKit::from_node(&node) {
        Ok(kit) => kit,
        Err(error) => {
            world.factory.nodes.insert(p, node);
            world.factory.form_matrices(size);
            world.expedition.say(error.to_string());
            return true;
        }
    };
    use super::inventory::{Item, ItemStack};
    let mut inventory = world.inventory.clone();
    let item = portable.map(Item::PowerKit).unwrap_or(Item::Block(node.kind.block()));
    let mut fits = inventory.try_insert(ItemStack::new(item, 1)).1 == 0;
    if let Some(id) = cache_id {
        for stack in world.caches[id].slots.into_iter().flatten() {
            fits &= inventory.try_insert(stack).1 == 0;
        }
    }
    if let Some(order) = world.factory.chips.production.orders.iter().find(|o| o.site == (p.x, p.y, p.z)) {
        fits &= inventory.try_insert(ItemStack::new(Item::Block(B::Sand), order.sand)).1 == 0;
    }
    if let Some(item) = assembler::stock_item(&node)
        && node.stock > 0 {
            fits &= inventory.try_insert(ItemStack { item, count: node.stock }).1 == 0;
        }
    if let Some(cast) = super::smeltery::installed_cast(&node) {
        fits &= inventory.try_insert(ItemStack::new(cast, 1)).1 == 0;
    }
    fits &= blocks::refund_fits(&mut inventory, &node);
    let mut refunds = std::array::from_fn::<_, RESOURCE_COUNT, _>(|i| node.input[i] + node.output[i]);
    // A departed shipment no longer needs its source, but packing its receiver
    // or any pipe on its route must reclaim it before the inventory can change.
    let interrupted = |packet: &transit::Packet| {
        packet.dst == p || packet.path.iter().skip(1)
            .take(packet.path.len().saturating_sub(2)).any(|cell| *cell == p)
    };
    let shipments: Vec<_> = world.factory.packets.iter().filter(|packet| interrupted(packet))
        .map(|packet| (packet.dst, packet.item, packet.amount)).collect();
    for &(_, item, amount) in &shipments {
        refunds[item] += amount;
    }
    for level in 0..if portable.is_some() { 0 } else { node.upgrade } {
        for &(i, n) in node.kind.upgrade_cost(level) { refunds[i] += n; }
    }
    // Hearth inputs are reserved at batch start, unlike electrical processors.
    if matches!(node.kind, Kind::CokeOven | Kind::BlastFurnace)
        && let Some(process) = node.process() {
            for (i, n) in process.inputs { refunds[i] += n; }
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
    // Commit packet ownership only after every capacity and world-edit check.
    // Failed pickup must keep both the shipments and their reservations live.
    world.factory.packets.retain(|packet| !interrupted(packet));
    for (dst, item, amount) in shipments {
        world.factory.release(dst, item, amount);
    }
    world.inventory = inventory;
    if let Some(id) = cache_id {
        world.caches[id].slots = [None; super::containers::SLOTS];
        if world.inventory.container.as_ref().is_some_and(|(active, _)| *active == id) { world.inventory.container = None; }
    }
    world.sync_from_inventory();
    std::sync::Arc::make_mut(&mut world.factory.chips.production).orders.retain(|o| o.site != (p.x, p.y, p.z));
    world.factory.chips.remove(p);
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
    fn networks(&self, conduit: Kind, size: i32) -> Vec<Network> {
        let starts = self.nodes.iter().filter_map(|(&p, n)| (n.kind == conduit).then_some(p)).collect();
        self.networks_from(conduit, size, starts)
    }

    fn networks_from(&self, conduit: Kind, size: i32, mut starts: Vec<IVec3>) -> Vec<Network> {
        let mut visited = HashSet::new();
        let mut nets = Vec::new();
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
            let mut closed = HashSet::new();
            let mut no_extract = HashSet::new();
            let mut no_insert = HashSet::new();
            while let Some(p) = queue.pop_front() {
                conduits.push(p);
                for (side, d) in DIRS.into_iter().enumerate() {
                    let q = p + d * size;
                    if let Some(n) = self.nodes.get(&q) {
                        if n.kind == conduit {
                            if conduit == Kind::Cable && n.upgrade != tier {
                                continue;
                            }
                            if !self.link_open(p, side, size) {
                                closed.insert((p, q));
                                continue;
                            }
                            if visited.insert(q) {
                                queue.push_back(q);
                            }
                        } else if !n.kind.conduit() && ports::accepts(conduit, n.kind) {
                            if !self.side_open(p, side) {
                                closed.insert((p, q));
                                continue;
                            }
                            // Item roles follow the machine face the pipe touches.
                            // Cables and fluid pipes ignore them.
                            if conduit == Kind::Pipe {
                                match sides::role(n, side ^ 1) {
                                    sides::Role::Off => {
                                        closed.insert((p, q));
                                    }
                                    sides::Role::In => {
                                        ports.insert(q);
                                        no_extract.insert((p, q));
                                    }
                                    sides::Role::Out => {
                                        ports.insert(q);
                                        no_insert.insert((p, q));
                                    }
                                    sides::Role::Both => {
                                        ports.insert(q);
                                    }
                                }
                            } else {
                                ports.insert(q);
                            }
                        }
                    }
                }
            }
            let mut ports: Vec<_> = ports.into_iter().collect();
            ports.sort_by_key(|p| (p.x, p.y, p.z));
            let mut net = Network::new(ports, conduits);
            net.closed = closed;
            net.no_extract = no_extract;
            net.no_insert = no_insert;
            net.tier = tier;
            nets.push(net);
        }
        nets
    }
    #[cfg(test)]
    pub fn tick(&mut self, dt: f32, size: i32) {
        self.tick_with_power(dt, size, |_| None);
    }

    fn tick_with_power(
        &mut self,
        dt: f32,
        size: i32,
        passive: impl Fn(IVec3) -> Option<(u32, &'static str)>,
    ) {
        self.step_controllers((dt * engine_chip::characterize::TICK_HZ as f32).round() as u64);
        for n in self.nodes.values_mut() {
            if n.enabled
                && matches!(
                    n.status,
                    "Processing" | "Generating" | "Coking" | "Smelting" | "Charging" | "Discharging"
                )
            {
                n.phase = (n.phase + dt * (1 + n.upgrade) as f32) % 1000.0;
            }
            if n.activity > 0.0 {
                n.activity = (n.activity - dt * 2.0).max(0.0);
            }
        }
        self.advance_packets(dt);
        self.accumulator += dt.clamp(0.0, TICK_SECONDS);
        if self.accumulator < TICK_SECONDS {
            return;
        }
        self.accumulator -= TICK_SECONDS;
        if !self.topology {
            let [mut cables, mut pipes, mut fluids] = [Vec::new(), Vec::new(), Vec::new()];
            for (&p, n) in &self.nodes {
                match n.kind {
                    Kind::Cable => cables.push(p),
                    Kind::Pipe => pipes.push(p),
                    Kind::FluidPipe => fluids.push(p),
                    _ => {}
                }
            }
            self.power = self.networks_from(Kind::Cable, size, cables);
            self.items = self.networks_from(Kind::Pipe, size, pipes);
            self.fluids = self.networks_from(Kind::FluidPipe, size, fluids);
            self.item_net.clear();
            self.routes.clear();
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
            self.chips.topology_changed();
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
            if n.enabled && n.kind == Kind::Generator && power::passive_rating(n.mode).is_some() {
                let (rate, idle) = passive(p).unwrap_or((0, "Waiting for world data"));
                let output = rate.min(power::GENERATOR_BUFFER.saturating_sub(n.energy));
                n.energy += output;
                n.flow = output as i32;
                n.status = if output > 0 {
                    "Generating"
                } else if rate > 0 {
                    "Buffer full"
                } else {
                    idle
                };
                continue;
            }
            if industry::boiler::active(n) {
                if n.enabled {
                    if self.chips.allows(p) { industry::boiler::step(n); }
                    else { n.status = "Controller Hold"; }
                }
                continue;
            }
            if industry::blocked(n) { n.status = "Exhaust blocked"; continue; }
            if n.kind == Kind::GasGenerator { continue; }
            if !n.enabled || !n.kind.generator() {
                if pile::is_pile(n) {
                    pile::cool_idle(n);
                }
                continue;
            }
            let before = n.energy;
            if pile::is_pile(n) {
                // A controller holding the pile inserts the rods: no fission.
                let outcome = if self.chips.allows(p) { pile::step(n) } else { pile::hold(n) };
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
                            n.fuel_resource=fuel as u32;
                        }
                    } else if n.kind == Kind::Generator {
                        // Best fuel first: coke, coal, bio-ethanol, creosote.
                        if let Some(&(fuel, portion, energy)) = power::COMBUSTION_FUELS
                            .iter()
                            .find(|f| n.input[f.0] >= f.1)
                        {
                            n.input[fuel] -= portion;
                            n.burn = energy;
                            n.fuel_resource=fuel as u32;
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
                industry::produce(n, energy);
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
        industry::burn_gas(self);
        let generated = self.nodes.values().filter(|n| n.kind.generator())
            .map(|n| n.flow.max(0) as u32).sum();
        self.stats.power(generated, 0);
        self.run_multiblocks();
        self.distribute_power();
        extraction::route_water(self);
        industry::process_water(self);
        self.cursor = self.cursor.wrapping_add(1);
        let stock_lines = stock_order::lines(self, size);
        self.stock_receivers.clear();
        self.stock_receivers.extend(stock_lines.iter().map(|(&source, line)| (source, line.dest)));
        self.stock_receivers.sort_unstable_by_key(|(source, _)| source.to_array());
        let energy_mark = self.by_base.energy_mark(&self.nodes);
        for (p, n) in self.nodes.iter_mut() {
            if self.chips.production.owns((p.x, p.y, p.z)) { continue; }
            if industry::blocked(n) { n.status = "Exhaust blocked"; continue; }
            if !n.enabled || !n.kind.processor() {
                continue;
            }
            if !self.chips.allows(*p) {
                n.status = "Held by controller";
                continue;
            }
            if stock_order::read(n.kind, n.mode, n.target).is_some() && n.recipe.is_none() {
                match stock_lines.get(p) {
                    None => { n.status = "Needs filtered storage route"; continue; }
                    Some(line) if !line.needed => {
                        n.status = if line.stocked { "Reserve stocked" } else { "Sending to reserve" };
                        continue;
                    }
                    Some(_) => {}
                }
            }
            if fuel_order::is_batch(n) && n.recipe.is_none() {
                let remaining = n.target - 1;
                if remaining == 0 {
                    n.status = "Fuel Delivered";
                    continue;
                }
                let queued = n.output[ETHANOL].saturating_add(fuel_order::in_flight(&self.packets, *p));
                if queued >= remaining {
                    n.status = "Awaiting Storage";
                    continue;
                }
            }
            if n.kind == Kind::Cultivator && let Some(recipe) = biomatter::recipe(n.mode) {
                let cost = biomatter::step(n, recipe);
                self.stats.power(0, cost);
                continue;
            }
            if let Some(cost) = placer::process(n, size) {
                self.stats.power(0, cost);
                continue;
            }
            if n.recipe.is_none() && !(n.kind == Kind::Crusher && n.progress > 0) {
                n.recipe = n.choose_recipe(size);
            }
            if n.kind == Kind::Crusher
                && n.recipe.is_none()
                && (n.progress > 0 || blocks::recipe(n).is_some())
            {
                let cost = blocks::step(n);
                industry::produce(n, cost);
                self.stats.power(0, cost);
                continue;
            }
            let Some(recipe) = n.process() else {
                n.status = "Needs input";
                continue;
            };
            let (out, yield_n) = recipe.output;
            if !recipe.ready(n, size) {
                n.recipe = None;
                n.progress = 0;
                n.status = "Needs input";
                continue;
            }
            if n.output[out] > CAPACITY - yield_n
                || recipe.byproduct.is_some_and(|(side, amount)| n.output[side] > CAPACITY.saturating_sub(amount))
            {
                n.status = "Output blocked";
                continue;
            }
            if n.kind == Kind::Washer && n.water < 128 {
                n.status = "Needs water";
                continue;
            }
            let mut steps = (1 + n.upgrade).min(recipe.ticks - n.progress);
            if n.kind == Kind::Fabricator {
                steps = steps.min(n.energy / recipe.power);
            }
            let cost = recipe.power * steps;
            if steps == 0 || n.energy < cost {
                n.status = "Needs cable / power";
                continue;
            }
            n.energy -= cost;
            self.stats.power(0, cost);
            n.progress += steps;
            industry::produce(n, steps);
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
                if let Some((material, count)) = recipe.block_cost(size) {
                    n.blocks.input[material.index()] -= count;
                }
                n.output[out] += yield_n;
                self.furnace_ingot |= n.kind == Kind::Furnace && matches!(out, 6..=8);
                if let Some((side, amount)) = recipe.byproduct {
                    n.output[side] += amount;
                }
                n.progress = 0;
                n.recipe = None;
            }
        }
        self.run_assemblers();
        self.by_base.record(&self.nodes, &before, &energy_mark);
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
            if n.stock > stock && matches!(assembler::stock_item(n), Some(super::inventory::Item::Block(b)) if Kind::from_block(b).is_some()) {
                self.stats.kits(n.stock - stock);
            }
        }
        self.by_base.tick(&self.nodes, &mut self.bases, size);
        self.stats.tick();
        if self.items.is_empty() || self.packets.len() >= transit::MAX_PACKETS {
            return;
        }
        #[cfg(test)]
        if self.reference_routing {
            self.reference_route(size);
            return;
        }
        let mut inbound = routes::Inbound::new(&self.packets);
        self.run_extractors(size, &mut inbound, &stock_lines);
        self.run_distributors(size, &mut inbound);
        self.route_outputs(size, &mut inbound, &stock_lines);
    }
}

/// Collision/picking envelopes for conduit junctions, in voxel coordinates.
/// The same face connections drive the visible tube arms.
pub fn conduit_bounds(world: &World, cell: IVec3) -> Option<Vec<(Vec3, Vec3)>> {
    let size = step(world);
    let p = anchor(cell, size);
    let n = world.factory.nodes.get(&p)?;
    if !sides::is_conduit(n.kind) {
        return None;
    }
    let c = p.as_vec3() + Vec3::splat(size as f32 * 0.5);
    let r = if n.kind == Kind::Cable { 0.10 } else if n.kind == Kind::ExhaustDuct {0.25} else { 0.19 } * size as f32;
    let mut bounds = vec![(c - Vec3::splat(r), c + Vec3::splat(r))];
    for (side, d) in DIRS.into_iter().enumerate() {
        if world.factory.conduit_join(p, side, size) {
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
    /// Installed controller: part, revision and output state.
    pub controller: Option<String>,
    pub lab_work: Option<(&'static str, f32)>,
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
        controller: world.factory.chips.summary(pos),
        lab_work: world.factory.chips.production.progress((pos.x, pos.y, pos.z)),
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
    // Review fixtures face the camera (+z) so machine fronts are captured.
    let mut node = Node::new(kind);
    node.facing = IVec3::Z;
    world.factory.nodes.insert(p, node);
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
        let travel = transit::SPEED * dt.clamp(0.0, TICK_SECONDS);
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
        let drill_ore = matches!(packet.item, 0..=3)
            && self.nodes.get(&packet.src).is_some_and(|n| n.kind == Kind::MiningDrill)
            && self.nodes.get(&packet.dst).is_some_and(|n| n.kind == Kind::Storage);
        let workshop_metal = self.nodes.get(&packet.src).is_some_and(|n| {
            matches!((n.kind, packet.item), (Kind::Furnace, 8) | (Kind::AlloyKiln | Kind::BlastFurnace, 15))
        })
            && self.nodes.get(&packet.dst).is_some_and(|n| n.kind == Kind::Storage);
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
        self.workshop_delivery |= workshop_metal && leftover < packet.amount;
        self.drill_delivery |= drill_ore && leftover < packet.amount;
        if packet.item == ETHANOL && leftover < packet.amount
            && self.nodes.get(&packet.dst).is_some_and(|n| n.kind == Kind::Storage)
            && let Some(source) = self.nodes.get_mut(&packet.src).filter(|n| n.kind == Kind::Cultivator)
        {
            if fuel_order::is_batch(source) && source.target > 1 {
                source.target = fuel_order::after_delivery(source.target, packet.amount - leftover);
            }
            self.workshop_delivery = true;
        }
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
        self.takes_unless(cell, item, || self.packets.iter().any(|p| p.dst == cell && p.item != item))
    }

    /// `container_takes`, with `other_inbound` answering whether a different
    /// resource is already headed to `cell`.
    fn takes_unless(&self, cell: IVec3, item: usize, other_inbound: impl FnOnce() -> bool) -> bool {
        let Some(dest) = self.nodes.get(&cell) else {
            return false;
        };
        if !dest.accepts(item) {
            return false;
        }
        if dest.kind == Kind::Storage && dest.mode == 2 && dest.filter.is_none() {
            let empty = dest.input.iter().all(|n| *n == 0);
            if empty && other_inbound() {
                return false;
            }
        }
        true
    }

    /// A port that can take another shipment of `item` right now.
    fn open_for(&self, q: IVec3, item: usize, inbound: &routes::Inbound) -> bool {
        let dest = &self.nodes[&q];
        let reserved = self.reserved.get(&(q, item)).copied().unwrap_or(0);
        dest.enabled
            && dest.kind != Kind::Feeder
            && dest.input[item].saturating_add(reserved) < CAPACITY
            && self.takes_unless(q, item, || inbound.other_than(q, item))
    }

    /// Extractor pipes pull from adjacent crates and machine outputs.
    fn run_extractors(&mut self, size: i32, inbound: &mut routes::Inbound, stock_lines: &HashMap<IVec3, stock_order::Line>) {
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
            'pull: for (side, d) in DIRS.into_iter().enumerate() {
                if !self.side_open(p, side) {
                    continue;
                }
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
                let order = stock_order::read(src.kind, src.mode, src.target);
                let fuel_batch = fuel_order::is_batch(src);
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
                    // Machines before storage, each in rotated port order.
                    let count = network.ports.len();
                    let rotated = (0..2 * count).map(|k| (k >= count, network.ports[(k % count + self.cursor) % count]));
                    for (to_storage, q) in rotated {
                        if q == s {
                            continue;
                        }
                        if i == ETHANOL && fuel_batch && !to_storage {
                            continue;
                        }
                        let reserve = order.filter(|order| order.item == i);
                        if reserve.is_some() && !stock_lines.get(&s).is_some_and(|line| line.dest == q) {
                            continue;
                        }
                        let dest = &self.nodes[&q];
                        if (dest.kind == Kind::Storage) != to_storage
                            || !dest.enabled
                            || dest.kind == Kind::Feeder
                            || !self.takes_unless(q, i, || inbound.other_than(q, i))
                        {
                            continue;
                        }
                        let reserved = self.reserved.get(&(q, i)).copied().unwrap_or(0);
                        let room = reserve.map_or_else(|| CAPACITY.saturating_sub(dest.input[i] + reserved),
                            |order| stock_order::room(order, dest, reserved));
                        let amount = available.min(room);
                        if amount == 0 || (i == ETHANOL && fuel_batch && amount < available) {
                            continue;
                        }
                        let Some(path) = self.routes.path(ni, network, size, s, q, Some(p)) else {
                            continue;
                        };
                        let source = self.nodes.get_mut(&s).unwrap();
                        if from_crate {
                            source.input[i] -= amount;
                        } else {
                            source.output[i] -= amount;
                        }
                        *self.reserved.entry((q, i)).or_insert(0) += amount;
                        inbound.add(q, i);
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
    fn run_distributors(&mut self, size: i32, inbound: &mut routes::Inbound) {
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
            let Some(path) = self.routes.path(ni, &self.items[ni], size, src, dst, None) else {
                continue;
            };
            self.nodes.get_mut(&src).unwrap().input[item] -= amount;
            *self.reserved.entry((dst, item)).or_insert(0) += amount;
            inbound.add(dst, item);
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

    /// Machine outputs travel to the nearest port that can take them.
    /// Material filters make the processing graph acyclic: feeder -> crusher
    /// -> furnace -> crate, and feeder -> generator. One unit per source per
    /// pass; backpressure retains everything at source.
    fn route_outputs(&mut self, size: i32, inbound: &mut routes::Inbound, stock_lines: &HashMap<IVec3, stock_order::Line>) {
        let mut moved = HashSet::new();
        // Storage sorts after processors, then by distance, then by rotated
        // port order; the keys never tie.
        let mut near: Vec<(u8, i32, i32, IVec3)> = Vec::new();
        // Per resource, the ports (offset, is storage) that could take it when
        // first asked. Inputs do not change during this pass and reservations
        // only grow, so a port missing from a list stays unable to take it.
        let mut open: Vec<Option<Vec<(u32, u8)>>> = vec![None; RESOURCE_COUNT];
        for (ni, network) in self.items.iter().enumerate() {
            let net = &network.ports;
            let mut flow = false;
            if net.is_empty() {
                continue;
            }
            open.iter_mut().for_each(|list| *list = None);
            let port_count = net.len() as i32;
            for k in 0..net.len() {
                let p = net[(k + self.cursor) % net.len()];
                let source = &self.nodes[&p];
                if moved.contains(&p) || (!source.enabled && source.kind != Kind::Assembler) {
                    continue;
                }
                let filter = if source.kind == Kind::Feeder { source.filter } else { None };
                let fuel_request = fuel_order::is_batch(source);
                let order = stock_order::read(source.kind, source.mode, source.target);
                // Snapshot the small buffer once; do not hash the same source
                // for every resource, including empty slots.
                'items: for (i, stock) in source.output.into_iter().enumerate() {
                    if filter.is_some_and(|filter| filter != i) {
                        continue;
                    }
                    let available = stock.min(64);
                    if available == 0 {
                        continue;
                    }
                    let list = open[i].get_or_insert_with(|| {
                        net.iter()
                            .enumerate()
                            .filter(|&(_, &q)| self.open_for(q, i, inbound))
                            .map(|(offset, q)| (offset as u32, u8::from(self.nodes[q].kind == Kind::Storage)))
                            .collect()
                    });
                    near.clear();
                    let reserve = order.filter(|order| order.item == i);
                    for &(offset, storage) in list.iter() {
                        if fuel_request && i == ETHANOL && storage == 0 {
                            continue;
                        }
                        let q = net[offset as usize];
                        if q == p || !self.open_for(q, i, inbound) {
                            continue;
                        }
                        if let Some(order) = reserve {
                            let reserved = self.reserved.get(&(q, i)).copied().unwrap_or(0);
                            if !stock_lines.get(&p).is_some_and(|line| line.dest == q)
                                || stock_order::room(order, &self.nodes[&q], reserved) == 0
                            {
                                continue;
                            }
                        }
                        if fuel_request && i == ETHANOL {
                            let reserved = self.reserved.get(&(q, i)).copied().unwrap_or(0);
                            if CAPACITY.saturating_sub(self.nodes[&q].input[i].saturating_add(reserved)) < available {
                                continue;
                            }
                        }
                        let dist = (q.x - p.x).abs() + (q.y - p.y).abs() + (q.z - p.z).abs();
                        let tie = (offset as i32 + self.cursor as i32) % port_count;
                        near.push((storage, dist, tie, q));
                    }
                    let Some(&(_, _, _, best)) = near.iter().min_by_key(|row| (row.0, row.1, row.2)) else {
                        continue;
                    };
                    if self.packets.len() >= transit::MAX_PACKETS {
                        break 'items;
                    }
                    let mut chosen = self.routes.path(ni, network, size, p, best, None).map(|path| (best, path));
                    if chosen.is_none() {
                        near.sort_unstable_by_key(|row| (row.0, row.1, row.2));
                        chosen = near.iter().skip(1).find_map(|&(_, _, _, q)| {
                            self.routes.path(ni, network, size, p, q, None).map(|path| (q, path))
                        });
                    }
                    let Some((q, path)) = chosen else {
                        continue;
                    };
                    let reserved = self.reserved.get(&(q, i)).copied().unwrap_or(0);
                    let dest = &self.nodes[&q];
                    let room = reserve.map_or_else(|| CAPACITY.saturating_sub(dest.input[i] + reserved),
                        |order| stock_order::room(order, dest, reserved));
                    let amount = available.min(room);
                    if let Some(n) = self.nodes.get_mut(&p) {
                        n.output[i] -= amount;
                    }
                    *self.reserved.entry((q, i)).or_insert(0) += amount;
                    inbound.add(q, i);
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
            if flow {
                for p in &network.conduits {
                    if let Some(n) = self.nodes.get_mut(p) {
                        n.activity = 1.0;
                    }
                }
            }
        }
    }
}
