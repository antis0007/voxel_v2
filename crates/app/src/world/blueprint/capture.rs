//! Budgeted copy of a world region into a blueprint.
use super::{Blueprint, Region, valid_size};
use crate::world::{BlockId, World, WorldSampleConfig, factory};
use engine_core::Budget;

/// Cells read between deadline checks.
const BUDGET_STRIDE: usize = 64;

pub struct Capture {
    cfg: WorldSampleConfig,
    region: Region,
    blueprint: Blueprint,
    index: usize,
    /// Cells left as air because a machine node or protected block owns them.
    pub skipped: usize,
}

impl Capture {
    /// `None` when the region is larger than a blueprint can hold.
    pub fn new(world: &World, region: Region) -> Option<Self> {
        let size = region.size();
        if !valid_size(size) {
            return None;
        }
        Some(Self {
            cfg: world.sample_config(),
            region,
            blueprint: Blueprint::filled(size, BlockId::Air)?,
            index: 0,
            skipped: 0,
        })
    }

    pub fn region(&self) -> Region {
        self.region
    }

    pub fn progress(&self) -> (usize, usize) {
        (self.index, self.blueprint.volume())
    }

    /// Whether the capture was taken from the world `world` still is.
    pub fn same_world(&self, world: &World) -> bool {
        self.cfg == world.sample_config()
    }

    /// Reads up to `max_cells` cells within `budget`; true once every cell is read.
    pub fn step(&mut self, world: &World, budget: Budget, max_cells: usize) -> bool {
        let machine = factory::step(world);
        let total = self.blueprint.volume();
        let mut done = 0;
        while self.index < total && done < max_cells {
            if done % BUDGET_STRIDE == 0 && done > 0 && !budget.has_time() {
                break;
            }
            let local = self.blueprint.local(self.index);
            let p = self.region.min + local;
            let b = world.get(p);
            // A machine's node state cannot travel in a blueprint, and bedrock
            // or tombstones would paste as cells a player cannot remove.
            let owned = world.factory.nodes.contains_key(&factory::anchor(p, machine));
            if owned || matches!(b, BlockId::Bedrock | BlockId::TombStone) {
                self.skipped += usize::from(b != BlockId::Air);
            } else {
                self.blueprint.cells[self.index] = b;
            }
            self.index += 1;
            done += 1;
        }
        self.index >= total
    }

    pub fn finish(self) -> Blueprint {
        self.blueprint
    }
}
