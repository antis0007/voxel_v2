//! Portable water and bounded, live connected-pool inspection.
use super::{World, BlockId, effects, inventory::Item, chunk_coord_from_world_block};
use crate::player::Player;
use glam::IVec3;
use std::{collections::{HashSet, VecDeque}, time::{Duration, Instant}};

pub const CAPACITY_UL: u32 = 1_000_000_000;
/// Payload bit marking a bucket full of lava; volumes use the low 30 bits.
pub const LAVA_FLAG: u32 = 1 << 31;
pub const LAVA_BUCKET: u32 = LAVA_FLAG | CAPACITY_UL;
pub const ACID_BUCKET: u32 = (1 << 30) | CAPACITY_UL;
pub const FUEL_CELL: u32 = (1 << 31) | (1 << 30);

pub const fn valid_bucket_payload(amount: u32) -> bool {
    amount <= CAPACITY_UL || matches!(amount, LAVA_BUCKET | ACID_BUCKET | FUEL_CELL)
}

pub fn bucket_contents(amount: u32) -> String {
    match amount {
        LAVA_BUCKET => "Lava · 1 block".into(),
        ACID_BUCKET => "Sulfuric acid · 1 block".into(),
        FUEL_CELL => "Fuel gas · 1 charge".into(),
        _ => format!("{} / 1 m³", volume(u64::from(amount))),
    }
}
pub fn volume(ul: u64) -> String {
    if ul >= 1_000_000_000 { format!("{:.2} m³", ul as f64 / 1_000_000_000.0) }
    else if ul > 0 && ul < 1_000_000 { format!("{:.3} mL", ul as f64 / 1_000.0) }
    else { format!("{:.2} L", ul as f64 / 1_000_000.0) }
}

pub fn target(world: &World, player: &Player) -> Option<(IVec3, IVec3)> {
    super::collision::raycast_liquids_precise(world, player.precise_eye(world.voxel_scale),
        player.forward_dir(), 6.0 / world.voxel_scale.meters())
}

pub fn use_cell(world: &World, hit: IVec3, before: IVec3, pour: bool) -> IVec3 {
    match world.get(hit) {
        BlockId::Water if !pour || world.water_microlitres(hit) < world.environment.water_capacity_ul(hit) => hit,
        BlockId::Lava | BlockId::SulfuricAcid if !pour => hit,
        _ => before,
    }
}

/// Returns true whenever a bucket owns the use gesture, including refusal.
pub fn use_bucket(world: &mut World, player: &Player, slot: usize, pour: bool,
    blocks: &mut HashSet<IVec3>, chunks: &mut HashSet<IVec3>) -> bool {
    let Some(stack) = world.inventory.hotbar.get(slot).copied().flatten() else { return false };
    let Item::Bucket(mut amount) = stack.item else { return false };
    if world.adventure.is_dead() || stack.count != 1 || amount == FUEL_CELL { return true; }
    let Some((hit, before)) = target(world, player) else { return true };
    if use_full_cell_liquid(world, hit, before, slot, amount, pour, blocks, chunks) { return true; }
    let water = world.get(hit) == BlockId::Water;
    let scoop = water && !pour;
    let cell = use_cell(world, hit, before, pour);
    let direction = if scoop { effects::WaterDirection::IntoBuffer } else { effects::WaterDirection::IntoWorld };
    match effects::exchange_water_ul(world, cell, &mut amount, CAPACITY_UL, direction) {
        Ok(receipt) => {
            receipt.publish(world, blocks, chunks);
            world.inventory.hotbar[slot].as_mut().unwrap().item = Item::Bucket(amount);
            world.sync_from_inventory();
        }
        Err(effects::Failure::NoChange) => world.expedition.say(if scoop { "Bucket Full" } else if amount == 0 { "Bucket Empty" } else { "Destination Full" }),
        Err(effects::Failure::Deferred) => world.expedition.say("Water Unavailable"),
        Err(_) => world.expedition.say("Transfer Blocked"),
    }
    true
}

/// Lava and acid carry one whole cell per bucket.
fn use_full_cell_liquid(world: &mut World, hit: IVec3, before: IVec3, slot: usize, amount: u32, pour: bool,
    blocks: &mut HashSet<IVec3>, chunks: &mut HashSet<IVec3>) -> bool {
    let struck = world.get(hit);
    let (cell, expected, put, next) = match (amount, struck) {
        (0, BlockId::Lava) if !pour => (hit, struck, BlockId::Air, LAVA_BUCKET),
        (0, BlockId::SulfuricAcid) if !pour => (hit, struck, BlockId::Air, ACID_BUCKET),
        (LAVA_BUCKET, _) => (before, BlockId::Air, BlockId::Lava, 0),
        (ACID_BUCKET, _) => (before, BlockId::Air, BlockId::SulfuricAcid, 0),
        _ => return false,
    };
    if world.get(cell) != expected {
        world.expedition.say("Destination Full");
    } else if world.admit_edit_chunk(chunk_coord_from_world_block(cell))
        && let Ok(receipt) = effects::replace(world, cell, expected, put)
    {
        receipt.publish(world, blocks, chunks);
        if let Some(s) = world.inventory.hotbar[slot].as_mut() { s.item = Item::Bucket(next); }
        world.sync_from_inventory();
    } else {
        world.expedition.say("Transfer Blocked");
    }
    true
}

#[derive(Clone, Copy)]
pub struct Reading { pub ul: u64, pub partial: bool, pub scanning: bool }

#[derive(Default)]
pub struct Probe {
    start: Option<IVec3>,
    queue: VecDeque<IVec3>,
    seen: HashSet<IVec3>,
    ul: u64,
    partial: bool,
    refreshed: Option<Instant>,
}
impl Probe {
    pub fn update(&mut self, world: &World, at: Option<IVec3>) -> Option<Reading> {
        let Some(at) = at.filter(|&p| world.get(p) == BlockId::Water) else { *self = Self::default(); return None };
        if self.start.is_none() || !self.seen.contains(&at) || (self.queue.is_empty()
            && self.refreshed.is_none_or(|t| t.elapsed() >= Duration::from_millis(500))) {
            *self = Self { start: Some(at), refreshed: Some(Instant::now()), ..Self::default() };
            self.queue.push_back(at); self.seen.insert(at);
        }
        // Includes dry boundary cells in the budget; never generates unloaded terrain.
        for _ in 0..256 {
            let Some(p) = self.queue.pop_front() else { break };
            if !world.chunks.contains_key(&chunk_coord_from_world_block(p)) { self.partial = true; continue; }
            let amount = world.water_microlitres(p);
            if amount == 0 { continue; }
            self.ul = self.ul.saturating_add(amount);
            for d in [IVec3::X, IVec3::NEG_X, IVec3::Y, IVec3::NEG_Y, IVec3::Z, IVec3::NEG_Z] {
                let q = p + d;
                if self.seen.contains(&q) { continue; }
                if self.seen.len() >= 8192 { self.partial = true; continue; }
                self.seen.insert(q); self.queue.push_back(q);
            }
        }
        Some(Reading { ul: self.ul, partial: self.partial, scanning: !self.queue.is_empty() })
    }
}

#[cfg(test)]
mod volume_tests {
    #[test]
    fn turbine_bucket_reading_preserves_small_amounts_and_existing_large_units() {
        for (ul, expected) in [(1, "0.001 mL"), (96, "0.096 mL"), (192, "0.192 mL"),
            (999_999, "999.999 mL"), (0, "0.00 L"), (1_000_000, "1.00 L"),
            (999_999_999, "1000.00 L"), (1_000_000_000, "1.00 m³")] {
            assert_eq!(super::volume(ul), expected);
            assert_eq!(super::bucket_contents(ul as u32), format!("{expected} / 1 m³"));
        }
        assert_eq!(super::bucket_contents(super::LAVA_BUCKET), "Lava · 1 block");
        assert_eq!(super::bucket_contents(super::ACID_BUCKET), "Sulfuric acid · 1 block");
        assert_eq!(super::bucket_contents(super::FUEL_CELL), "Fuel gas · 1 charge");
    }
}
