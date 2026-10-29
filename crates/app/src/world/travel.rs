//! Villager trades, ridden mounts and boats, and fishing.
//! Stock, absences and boats persist in `sidecar::FILE`, not the save body.
use super::World;
use super::adventure::wildlife::vehicle;
use super::inventory::{DroppedStack, Item, ItemStack};
use crate::player::{Input, Player};
use glam::{IVec3, Vec3};
use std::collections::BTreeMap;

pub mod fishing;
pub(crate) mod sidecar;
pub mod trades;
#[cfg(test)]
mod tests;

pub const BOAT: Item = Item::Tool(9);
pub const ROD: Item = Item::Tool(10);
/// One default in-game day in fixed steps; trades restock when it turns over.
pub const DAY_TICKS: u64 = 480 * 120;
pub const MAX_BOATS: usize = 256;
const REACH_METRES: f32 = 4.0;
const PLACE_METRES: f32 = 6.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Ride {
    /// A grown herd member, by herd identity.
    Mount(u32),
    Boat(u32),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Boat {
    pub pos: Vec3,
    pub yaw: f32,
}

#[derive(Clone, Debug, Default)]
pub struct State {
    /// Day number the stock below belongs to.
    pub day: u64,
    /// Uses spent today per (villager, trade row).
    pub used: BTreeMap<(u32, u16), u16>,
    /// Villagers gone for the rest of the given day.
    pub away: BTreeMap<u32, u64>,
    pub boats: BTreeMap<u32, Boat>,
    pub next_boat: u32,
    pub riding: Option<Ride>,
    pub cast: Option<fishing::Cast>,
}

/// What a use action did.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Use {
    None,
    Taken,
    OpenTrade(u32),
}

pub fn day(world: &World) -> u64 {
    world.farming.clock / DAY_TICKS
}

impl State {
    /// Clears spent uses and absences once the day turns over.
    pub fn restock(&mut self, day: u64) {
        if day != self.day {
            self.day = day;
            self.used.clear();
            self.away.retain(|_, d| *d == day);
        }
    }

    pub fn uses_left(&self, villager: u32, row: usize, trade: &trades::Trade) -> u16 {
        trade.uses.saturating_sub(self.used.get(&(villager, row as u16)).copied().unwrap_or(0))
    }

    pub fn add_boat(&mut self, boat: Boat) -> Option<u32> {
        if self.boats.len() >= MAX_BOATS {
            return None;
        }
        while self.boats.contains_key(&self.next_boat) {
            self.next_boat = (self.next_boat + 1) % (1 << 24);
        }
        let id = self.next_boat;
        self.boats.insert(id, boat);
        self.next_boat = (self.next_boat + 1) % (1 << 24);
        Some(id)
    }
}

/// Why a trade did not happen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Refusal {
    Unknown,
    OutOfStock,
    Missing,
}

/// Swaps the row's inputs for its output and spends one use.
pub fn trade(world: &mut World, villager: u32, row: usize) -> Result<(), Refusal> {
    world.travel.restock(day(world));
    let t = trades::page(villager).get(row).ok_or(Refusal::Unknown)?;
    if world.travel.uses_left(villager, row, t) == 0 {
        return Err(Refusal::OutOfStock);
    }
    if t.give.iter().any(|s| world.inventory.count_item(s.item) < s.count) {
        return Err(Refusal::Missing);
    }
    for s in &t.give {
        world.inventory.remove_item(s.item, s.count);
    }
    let (_, left) = world.inventory.try_insert(ItemStack::new(t.get.item, t.get.count));
    if left > 0 {
        let pos = vehicle::villager_centre(world, villager).unwrap_or(Vec3::ZERO);
        world.inventory.dropped.push(DroppedStack { pos, stack: ItemStack::new(t.get.item, left) });
    }
    *world.travel.used.entry((villager, row as u16)).or_insert(0) += 1;
    world.sync_from_inventory();
    Ok(())
}

/// WASD as a flat heading in camera space: forward is -z at yaw 0.
pub fn heading(input: &Input, yaw: f32) -> Vec3 {
    let (s, c) = yaw.sin_cos();
    let forward = Vec3::new(s, 0.0, -c);
    let right = Vec3::new(c, 0.0, s);
    let axis = |a: bool, b: bool| a as i32 as f32 - b as i32 as f32;
    (forward * axis(input.forward, input.back) + right * axis(input.right, input.left)).normalize_or_zero()
}

/// First water cell along the ray within `range` metres, stopping at solid blocks.
pub fn water_along(world: &World, eye_m: Vec3, forward: Vec3, range: f32) -> Option<IVec3> {
    let s = world.voxel_scale.meters();
    let step = (s * 0.25).min(0.25);
    let mut t = 0.0;
    while t <= range {
        let cell = ((eye_m + forward * t) / s).floor().as_ivec3();
        let block = world.get(cell);
        if block == super::BlockId::Water {
            return Some(cell);
        }
        if block.is_solid() {
            return None;
        }
        t += step;
    }
    None
}

/// Use action: rod casts or reels, a villager opens trade, an empty hand mounts, a boat is placed.
pub fn interact(world: &mut World, eye_m: Vec3, forward: Vec3, slot: usize) -> Use {
    let held = world.inventory.hotbar.get(slot).copied().flatten().map(|s| s.item);
    if held == Some(ROD) {
        fishing::use_rod(world, eye_m, forward, slot);
        return Use::Taken;
    }
    match vehicle::target(world, eye_m, forward, REACH_METRES) {
        Some(vehicle::Target::Villager(id)) => return Use::OpenTrade(id),
        Some(vehicle::Target::Mount(id)) if held.is_none() && world.travel.riding.is_none() => {
            world.travel.riding = Some(Ride::Mount(id));
            world.expedition.say("Riding");
            return Use::Taken;
        }
        Some(vehicle::Target::Boat(id)) if held.is_none() && world.travel.riding.is_none() => {
            world.travel.riding = Some(Ride::Boat(id));
            world.expedition.say("Boat · Riding");
            return Use::Taken;
        }
        _ => {}
    }
    if held == Some(BOAT)
        && let Some(cell) = water_along(world, eye_m, forward, PLACE_METRES)
        && let Some(y) = vehicle::float_y(world, (cell.as_vec3() + Vec3::splat(0.5)) * world.voxel_scale.meters())
    {
        let s = world.voxel_scale.meters();
        let pos = Vec3::new((cell.x as f32 + 0.5) * s, y, (cell.z as f32 + 0.5) * s);
        let yaw = (-forward.x).atan2(-forward.z);
        if world.travel.add_boat(Boat { pos, yaw }).is_none() {
            world.expedition.say(format!("Boats: {MAX_BOATS}/{MAX_BOATS}"));
        } else if world.inventory.consume_from_hotbar(slot, BOAT) {
            world.sync_from_inventory();
        }
        return Use::Taken;
    }
    Use::None
}

/// Per-step travel upkeep. True while riding, when the vehicle carries the player instead of walking.
pub fn step(world: &mut World, player: &mut Player, input: &Input, steer: bool, dt: f32) -> bool {
    let scale = world.voxel_scale.meters();
    let feet = player.pos * scale;
    world.travel.restock(day(world));
    vehicle::service(world, feet);
    fishing::tick(world, feet);
    let Some(ride) = world.travel.riding else { return false };
    if steer && input.sneak {
        world.travel.riding = None;
        player.pos += Vec3::Y * (0.6 / scale);
        player.precise_pos = player.pos.as_dvec3();
        return false;
    }
    let dir = if steer { heading(input, player.yaw) } else { Vec3::ZERO };
    let Some(seat) = vehicle::drive(world, ride, dir, steer && input.jump, dt) else {
        world.travel.riding = None;
        return false;
    };
    player.pos = seat / scale;
    player.precise_pos = player.pos.as_dvec3();
    player.vel = Vec3::ZERO;
    player.on_ground = true;
    player.flying = false;
    true
}
