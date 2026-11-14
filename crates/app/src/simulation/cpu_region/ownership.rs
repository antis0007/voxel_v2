//! Inventory escrow for conditional terrain commands.
use super::{Command, EditStatus, Region};
use crate::world::{BlockId, World, inventory::{Item, ItemStack}};
use engine_sim::Cell;
use glam::IVec3;
use std::sync::Arc;

#[derive(Clone, Copy)]
pub(super) struct Claim {
    pub block: BlockId,
    pub placement: bool,
}

impl Claim {
    pub fn cells(self) -> (Cell, Cell) {
        let filled = Cell::filled(self.block.sim_byte());
        if self.placement { (Cell::EMPTY, filled) } else { (filled, Cell::EMPTY) }
    }
}

impl Region {
    fn observed(&self, world: &World, at: IVec3) -> Result<Cell, String> {
        if !world.ca_view.as_ref().is_some_and(|view| Arc::ptr_eq(view, &self.published)) {
            return Err("terrain edit requires the current cellular publication".into());
        }
        if !self.published.contains(at) { return Err("terrain edit outside cellular interior".into()); }
        self.published.cell(at).ok_or("missing observed cell".into())
    }

    /// Reserve a held block until its conditional placement resolves.
    pub fn place_owned(&mut self, world: &mut World, player_tick: u64, at: IVec3,
        slot: usize) -> Result<Command, String> {
        let stack = world.inventory.hotbar.get(slot).copied().flatten()
            .filter(|stack| stack.count > 0).ok_or("selected slot is empty")?;
        let Item::Block(block @ (BlockId::Stone | BlockId::Sand)) = stack.item else {
            return Err("cellular placement supports held stone or sand".into());
        };
        let expected = self.observed(world, at)?;
        if expected != Cell::EMPTY { return Err("cellular placement requires empty space".into()); }
        let command = self.submit_replace(player_tick, at, expected, Cell::filled(block.sim_byte()))?;
        // No fallible work follows admission. The selected stack was checked above.
        world.inventory.hotbar[slot] = (stack.count > 1).then_some(ItemStack {
            item: stack.item, count: stack.count - 1,
        });
        self.owned.insert(command.sequence, Claim { block, placement: true });
        self.inventory_bound = true;
        world.ca_reservations.insert(at);
        world.sync_from_inventory();
        Ok(command)
    }

    /// Retain the mined block until its visible result can be collected.
    pub fn mine_owned(&mut self, world: &World, player_tick: u64, at: IVec3) -> Result<Command, String> {
        let expected = self.observed(world, at)?;
        let block = BlockId::from_u32(expected.material as u32);
        if !matches!(block, BlockId::Stone | BlockId::Sand) || expected != Cell::filled(block.sim_byte()) {
            return Err("cellular mining requires a full stone or sand cell".into());
        }
        let command = self.submit_replace(player_tick, at, expected, Cell::EMPTY)?;
        self.owned.insert(command.sequence, Claim { block, placement: false });
        self.inventory_bound = true;
        Ok(command)
    }

    /// Settle owned FIFO results after their publication deadline.
    pub fn settle_owned(&mut self, world: &mut World) {
        while let Some(outcome) = self.outcomes.front().copied() {
            let Some(claim) = self.owned.get(&outcome.sequence).copied() else { break };
            if outcome.tick > self.published.tick { break; }
            let applied = outcome.status == EditStatus::Applied;
            // A failed placement returns escrow; successful mining yields its block.
            if claim.placement != applied {
                let stack = ItemStack::new(Item::Block(claim.block), 1);
                if !world.inventory.can_insert(stack) { break; }
                let _ = world.inventory.try_insert(stack);
                world.sync_from_inventory();
            }
            self.owned.remove(&outcome.sequence);
            self.outcomes.pop_front();
            eprintln!("[ca-settled] sequence={} tick={} status={:?}",
                outcome.sequence, outcome.tick, outcome.status);
        }
        self.publish_reservations(world);
    }

    pub fn publish_reservations(&self, world: &mut World) {
        world.ca_reservations.clear();
        for (&sequence, claim) in &self.owned {
            if !claim.placement { continue; }
            if let Some(at) = self.pending.iter().find(|c| c.sequence == sequence).map(|c| c.at)
                .or_else(|| self.outcomes.iter().find(|o|
                    o.sequence == sequence && o.tick > self.published.tick).map(|o| o.at)) {
                world.ca_reservations.insert(at);
            }
        }
    }

    pub fn settle_results(&mut self, world: &mut World) {
        self.settle_owned(world);
        while let Some(outcome) = self.take_outcome() {
            eprintln!("[ca-resolved] sequence={} tick={} at={} status={:?}",
                outcome.sequence, outcome.tick, outcome.at, outcome.status);
            self.settle_owned(world);
        }
    }
}
