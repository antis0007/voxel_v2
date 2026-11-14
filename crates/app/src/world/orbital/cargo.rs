//! Station freight owns items from locker withdrawal through ground collection.
//! Security identities share combat, but never site-clear rewards or respawns.
use super::habitat;
use crate::{player::Player, world::{World, BlockId as B, containers, inventory::{Item, ItemStack}, interaction::Prompt}};
use glam::{IVec3, Vec3};
use serde::{Deserialize, Serialize};
pub mod security;

pub const ROBOT: u32 = 32;
pub const MAX_GUARDS: usize = 6;
pub const LOAD_TICKS: u32 = 45 * 120;
pub const ARRIVAL_TICKS: u32 = LOAD_TICKS + 120 * 120;
const TERMINAL: Vec3 = Vec3::new(13.0, 1.2, 2.0);
const CONTROL: Vec3 = Vec3::new(21.0, 2.0, 0.0);
pub const CELL: Item = Item::Block(B::MithrilBlock);
pub const SALVAGE_MANIFEST: [(Item, u32); 4] = [
    (Item::Resource(15), 16),
    (Item::Resource(12), 8),
    (Item::Resource(5), 4),
    (Item::Weapon(2), 1),
];

#[derive(Clone, Copy, Serialize, Deserialize)]
pub enum Supply {
    Steel,
    Fuel,
}

impl Supply {
    pub const ALL: [Self; 2] = [Self::Steel, Self::Fuel];

    pub fn cost(self) -> ItemStack {
        use crate::world::factory::{ETHANOL, fuel_order};
        match self {
            Self::Steel => ItemStack::new(Item::Resource(15), 4),
            Self::Fuel => ItemStack::new(Item::Resource(ETHANOL as u8), fuel_order::BATCH),
        }
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct State { pub stations: Vec<Station> }

#[derive(Serialize, Deserialize)]
pub struct Station {
    pub origin: IVec3,
    pub berth: Vec3,
    pub fixtures: usize,
    pub ready: bool,
    pub cache: Option<usize>,
    pub shipment: Option<Shipment>,
    pub guards: Vec<Guard>,
    pub pause: u32,
    pub cooldown: u32,
    pub spawn_clock: u32,
    pub produced: u32,
    #[serde(default)]
    pub calibrations: u32,
}
#[derive(Serialize, Deserialize)]
pub struct Shipment {
    pub items: Vec<ItemStack>,
    pub destination: Vec3,
    pub ticks: u32,
    #[serde(default)]
    pub cell_order: bool,
}
#[derive(Serialize, Deserialize)]
pub struct Guard { pub slot: u32, pub pos: Vec3, pub health: f32 }

#[derive(Clone, Default)]
pub struct View {
    pub at_station: bool,
    pub can_dispatch: bool,
    pub receiver_available: bool,
    pub can_collect: bool,
    pub status: &'static str,
    pub seconds: u32,
    pub guards: usize,
    pub paused_seconds: u32,
    pub items: Vec<ItemStack>,
    pub can_order_cell: [bool; 2],
}

fn state(world: &World) -> &State { &world.orbital.habitats.cargo }
fn eye(world: &World, p: &Player) -> Vec3 { p.eye(world.voxel_scale) * world.voxel_scale.meters() }
fn focus(world: &World, p: &Player, at: Vec3) -> bool {
    let d = at - eye(world, p);
    d.length_squared() < 12.25 && d.normalize_or_zero().dot(p.forward_dir()) > 0.55
        && crate::world::raycast_block(world, eye(world, p) / world.voxel_scale.meters(),
            d.normalize_or_zero(), (d.length() - 0.35).max(0.0) / world.voxel_scale.meters()).is_none()
}
fn station_at(world: &World, p: &Player) -> Option<usize> {
    state(world).stations.iter().position(|s| s.ready && focus(world, p, s.origin.as_vec3() + TERMINAL))
}
pub fn console_focused(world: &World, p: &Player) -> bool { station_at(world, p).is_some() }
fn collectable(world: &World, p: &Player, s: &Station) -> bool {
    s.shipment.as_ref().is_some_and(|q| q.ticks == ARRIVAL_TICKS
        && eye(world, p).distance_squared(q.destination + Vec3::Y) < 36.0)
}

pub fn view(world: &World, p: &Player) -> View {
    let mut v = View { can_collect: state(world).stations.iter().any(|s| collectable(world, p, s)),
        receiver_available: super::receiver::intact(world), ..View::default() };
    let Some(i) = station_at(world, p) else { return v };
    let s = &state(world).stations[i];
    v.at_station = true;
    v.guards = s.guards.len();
    v.paused_seconds = s.pause.div_ceil(120);
    if let Some(q) = &s.shipment {
        v.status = if q.ticks < LOAD_TICKS { "Loading" }
            else if q.ticks < ARRIVAL_TICKS { "In Transit" } else { "Delivered" };
        v.seconds = (if q.ticks < LOAD_TICKS { LOAD_TICKS } else { ARRIVAL_TICKS } - q.ticks).div_ceil(120);
        v.items = q.items.clone();
    } else {
        v.can_order_cell = Supply::ALL.map(|supply| {
            let cost = supply.cost();
            world.ca_view.is_none() && world.rules.mode.interacts() && !world.adventure.is_dead()
                && world.inventory.count_item(cost.item) >= cost.count
        });
        v.items = s.cache.and_then(|id| world.caches.get(id))
            .map(|c| c.slots.iter().flatten().copied().collect()).unwrap_or_default();
        let cell = locker_cell(world, s.origin);
        v.can_dispatch = !v.items.is_empty() && world.get(cell) == B::SalvageCrate
            && world.rules.mode.interacts() && !world.adventure.is_dead();
        v.status = if v.items.is_empty() { "Empty" } else { "Ready" };
    }
    v
}

pub fn dispatch(world: &mut World, p: &Player, receiver: bool) -> bool {
    if !world.rules.mode.interacts() || world.adventure.is_dead() { return false; }
    containers::flush(world);
    let Some(i) = station_at(world, p) else { return false };
    if !view(world, p).can_dispatch { return false; }
    let destination = if receiver {
        if !super::receiver::intact(world) { return false; }
        super::receiver::position(world).unwrap()
    } else { state(world).stations[i].berth };
    let Some(cache) = state(world).stations[i].cache else { return false };
    let items: Vec<_> = world.caches[cache].slots.iter().flatten().copied().collect();
    if items.is_empty() { return false; }
    // One synchronous custody transfer. Refresh the open chest's presentation
    // copy so a subsequent UI flush cannot put dispatched items back.
    world.caches[cache].slots.fill(None);
    containers::refresh(world);
    launch(world, i, Shipment { items, destination, ticks: 0, cell_order: false });
    true
}

pub fn order_cell(world: &mut World, p: &Player, supply: Supply, receiver: bool) -> bool {
    if world.ca_view.is_some() || !world.rules.mode.interacts() || world.adventure.is_dead() {
        return false;
    }
    let Some(i) = station_at(world, p) else { return false; };
    let s = &state(world).stations[i];
    if s.shipment.is_some() { return false; }
    let destination = if receiver {
        if !super::receiver::intact(world) { return false; }
        let Some(position) = super::receiver::position(world) else { return false; };
        position
    } else {
        let Some(h) = world.orbital.habitats.habitats.iter().find(|h| h.origin == s.origin)
            else { return false; };
        super::express::ground_berth(world, h.cell, h.berth.as_dvec3()).as_vec3()
    };
    let cost = supply.cost();
    if !world.inventory.remove_item(cost.item, cost.count) { return false; }
    launch(world, i, Shipment {
        items: vec![ItemStack::new(CELL, 1)], destination, ticks: 0, cell_order: true,
    });
    true
}

fn launch(world: &mut World, i: usize, shipment: Shipment) {
    let s = &mut world.orbital.habitats.cargo.stations[i];
    s.shipment = Some(shipment);
    s.produced = 0;
    s.spawn_clock = 0;
    world.expedition.say("Cargo · Loading");
    world.adventure.sounds.push(crate::audio::Event { cue: crate::audio::Cue::Warning,
        pos: Some(s.origin.as_vec3() + TERMINAL), gain: 0.9, pitch: 0.7 });
    world.sync_from_inventory();
}

pub fn cell_ready(world: &World) -> bool {
    world.ca_view.is_none() && world.inventory.count_item(CELL) > 0
        && state(world).stations.iter().any(|s| s.calibrations > 0)
}

pub fn cell_delivery(world: &World) -> Option<(u32, IVec3)> {
    let blocks_per_metre = world.sample_config().inv_voxel_meters();
    state(world).stations.iter().filter_map(|s| s.shipment.as_ref())
        .filter(|q| q.cell_order)
        .map(|q| {
            let seconds = ARRIVAL_TICKS.saturating_sub(q.ticks).div_ceil(120);
            let destination = (q.destination * blocks_per_metre).floor().as_ivec3();
            (seconds, destination)
        })
        .min_by_key(|&(seconds, _)| seconds)
}

/// Install one ground-collected cell. Ordinary mined mithril alone cannot
/// calibrate a bank, and each paid delivery supports one installation.
pub fn install_cell(world: &mut World) -> bool {
    let Some(s) = world.orbital.habitats.cargo.stations.iter_mut().find(|s| s.calibrations > 0)
        else { return false; };
    if !world.inventory.remove_item(CELL, 1) { return false; }
    s.calibrations -= 1;
    world.sync_from_inventory();
    true
}

pub fn collect(world: &mut World, p: &Player) -> bool {
    if !world.rules.mode.interacts() || world.adventure.is_dead() { return false; }
    let ids: Vec<_> = state(world).stations.iter().enumerate()
        .filter(|(_, s)| collectable(world, p, s)).map(|(i, _)| i).collect();
    if ids.is_empty() { return false; }
    let mut mithril_delivered = false;
    for i in ids {
        let s = &mut world.orbital.habitats.cargo.stations[i];
        let q = s.shipment.as_mut().unwrap();
        for stack in &mut q.items { stack.count = world.inventory.try_insert(*stack).1; }
        q.items.retain(|s| s.count != 0);
        if q.items.is_empty() {
            if q.cell_order {
                s.calibrations = s.calibrations.saturating_add(1);
                mithril_delivered = true;
            }
            s.shipment = None;
        }
    }
    world.sync_from_inventory();
    let remaining = state(world).stations.iter().any(|s| collectable(world, p, s));
    world.expedition.say(if remaining { "Cargo · Inventory Full" }
        else if mithril_delivered { "Mithril Delivered · Bronze Bank" }
        else { "Cargo · Collected" });
    true
}

pub fn prompt(world: &World, p: &Player) -> Option<Prompt> {
    if !world.rules.mode.interacts() || world.adventure.is_dead() { return None; }
    if state(world).stations.iter().any(|s| collectable(world, p, s)) {
        return Some(Prompt { title: "Ground Cargo", action: "[F] Collect" });
    }
    if console_focused(world, p) { return Some(Prompt { title: "Station Freight", action: "[F] Freight" }); }
    state(world).stations.iter().find(|s| s.ready && focus(world, p, s.origin.as_vec3() + CONTROL))
        .map(|s| Prompt { title: "Constructor Control", action: if s.cooldown == 0 { "[F] Interrupt" } else { "Recharging" } })
}

pub fn interact(world: &mut World, p: &Player) -> bool {
    if collect(world, p) { return true; }
    if !world.rules.mode.interacts() || world.adventure.is_dead() { return false; }
    let Some(i) = state(world).stations.iter().position(|s| s.ready && focus(world, p, s.origin.as_vec3() + CONTROL)) else { return false };
    let s = &mut world.orbital.habitats.cargo.stations[i];
    if s.cooldown == 0 {
        s.pause = 12 * 120;
        s.cooldown = 45 * 120;
        world.expedition.say("Constructors · Paused");
    } else { world.expedition.say("Control · Recharging"); }
    true
}

pub fn locker_cell(world: &World, origin: IVec3) -> IVec3 {
    (origin + IVec3::new(14, 0, 0)) * world.voxel_scale.meters().recip().round() as i32
}

/// Runs on the fixed world clock, including while the passenger is dead or away.
pub fn tick(world: &mut World, p: &Player) {
    if world.ca_view.is_some() { return; }
    let mut cargo = std::mem::take(&mut world.orbital.habitats.cargo);
    for h in &world.orbital.habitats.habitats {
        if h.ready && !cargo.stations.iter().any(|s| s.origin == h.origin) {
            cargo.stations.push(Station { origin: h.origin, berth: h.berth, fixtures: 0,
                ready: false, cache: None, shipment: None, guards: Vec::new(),
                pause: 0, cooldown: 0, spawn_clock: 0, produced: 0, calibrations: 0 });
        }
    }
    let mut build_budget = 1024;
    for s in &mut cargo.stations {
        if !s.ready { security::install(world, s, &mut build_budget); }
        s.pause = s.pause.saturating_sub(1);
        s.cooldown = s.cooldown.saturating_sub(1);
        if let Some(q) = &mut s.shipment {
            q.ticks = (q.ticks + 1).min(ARRIVAL_TICKS);
            if [LOAD_TICKS, ARRIVAL_TICKS].contains(&q.ticks) && eye(world, p).distance_squared(s.origin.as_vec3()) < 10000.0 {
                world.expedition.say(if q.ticks == LOAD_TICKS { "Cargo · Launched" } else { "Cargo · Delivered" });
            }
        }
        security::advance(world, p, s);
    }
    world.orbital.habitats.cargo = cargo;
}

/// Called before combat evicts distant actors, and before settling robot deaths.
pub fn remember(world: &mut World, enemies: &[crate::world::adventure::Enemy]) {
    habitat::residents::remember(world, enemies);
    for e in enemies.iter().filter(|e| e.kind == ROBOT) {
        if let Some(s) = world.orbital.habitats.cargo.stations.iter_mut().find(|s| s.origin == e.site) {
            if e.health <= 0.0 { s.guards.retain(|r| r.slot != e.slot); }
            else if let Some(r) = s.guards.iter_mut().find(|r| r.slot == e.slot) {
                r.pos = e.pos; r.health = e.health;
            }
        }
    }
}

pub fn validate(state: &habitat::State) -> anyhow::Result<()> {
    use anyhow::ensure;
    ensure!(state.cargo.stations.len() <= 16, "Too many station freight records");
    let mut seen = std::collections::HashSet::new();
    for s in &state.cargo.stations {
        ensure!(seen.insert(s.origin) && state.habitats.iter().any(|h| h.origin == s.origin && h.berth == s.berth), "Unknown freight station");
        ensure!(s.fixtures <= 20_000 && s.cache.is_none_or(|c| c < containers::LIMIT)
            && (!s.ready || s.cache.is_some()), "Invalid freight fixture record");
        ensure!(s.pause <= 12 * 120 && s.cooldown <= 45 * 120 && s.spawn_clock <= 4 * 120 && s.produced <= 12, "Invalid constructor clock");
        ensure!(s.guards.len() <= MAX_GUARDS, "Too many station guards");
        let mut slots = std::collections::HashSet::new();
        for g in &s.guards {
            ensure!(g.slot < MAX_GUARDS as u32 && slots.insert(g.slot) && g.pos.is_finite()
                && (g.pos - s.origin.as_vec3()).length() < 150.0
                && g.health.is_finite() && g.health > 0.0 && g.health <= 60.0, "Invalid station guard");
        }
        if let Some(q) = &s.shipment {
            ensure!(q.ticks <= ARRIVAL_TICKS && q.destination.is_finite()
                && q.destination.abs().max_element() < 900_000.0
                && !q.items.is_empty() && q.items.len() <= containers::SLOTS, "Invalid freight shipment");
            ensure!(!q.cell_order || (q.items.len() == 1 && q.items[0].item == CELL && q.items[0].count == 1),
                "Invalid mithril cell shipment");
            for stack in &q.items {
                let (k, p) = stack.item.encode_u32();
                ensure!(Item::decode_u32(k, p)? == stack.item && (1..=1_048_576).contains(&stack.count), "Invalid freight item");
            }
        }
    }
    Ok(())
}
