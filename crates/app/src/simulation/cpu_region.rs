//! Isolated cellular region: exact cells, ordered ingress and delayed publication.
//! No conversion back into the eight-level legacy water map or ordinary saves.
mod backend;
mod checkpoint;
mod ingress;
mod ownership;
pub use ingress::{EditOutcome, EditStatus, StepResult};
use crate::world::{BlockId, World};
use engine_sim::{Cell, StepStats};
use glam::IVec3;
use std::{collections::VecDeque, sync::Arc};

pub const EDGE: i32 = 8;
pub const CELLS: usize = 512;
pub const COMMAND_CAPACITY: usize = 128;
pub const COMMANDS_PER_TICK: usize = 32;

pub fn local(index: usize) -> IVec3 {
    IVec3::new(
        (index % 8) as i32,
        (index / 64) as i32,
        ((index / 8) % 8) as i32,
    )
}

#[derive(Clone)]
pub struct Snapshot {
    pub origin: IVec3,
    /// S[0] is the imported state; S[n] is after cellular tick n.
    pub tick: u64,
    /// Last command resolved, including conflicts and no-op writes.
    pub applied_sequence: u64,
    pub cells: Box<[Cell; CELLS]>,
    pub hash: u64,
    pub stats: StepStats,
}

impl Snapshot {
    pub fn contains(&self, p: IVec3) -> bool {
        p.cmpge(self.origin).all() && p.cmplt(self.origin + IVec3::splat(EDGE)).all()
    }
    pub fn cell(&self, p: IVec3) -> Option<Cell> {
        if self.contains(p) {
            let q = p - self.origin;
            Some(self.cells[(q.x + 8 * q.z + 64 * q.y) as usize])
        } else if p.cmpge(self.origin - IVec3::ONE).all()
            && p.cmple(self.origin + IVec3::splat(EDGE)).all()
        {
            Some(Cell::filled(BlockId::Bedrock as u8))
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Command {
    pub sequence: u64,
    pub submitted_player_tick: u64,
    pub earliest_ca_tick: u64,
    pub at: IVec3,
    pub cell: Cell,
    pub expected: Option<Cell>,
}

/// A result is usable only under its actual tick. An asynchronous backend must
/// return unavailable before mutation when it cannot meet this exact request.
pub trait CaBackend {
    fn advance(&mut self, tick: u64, commands: &[Command]) -> Result<StepResult, String>;
}

pub struct Region {
    backend: backend::Cpu,
    gpu: Option<backend::gpu::Gpu>,
    pending: VecDeque<Command>,
    results: VecDeque<Arc<Snapshot>>,
    pub published: Arc<Snapshot>,
    pub completed: u64,
    next_sequence: u64,
    outcomes: VecDeque<EditOutcome>,
    owned: std::collections::BTreeMap<u64, ownership::Claim>,
    inventory_bound: bool,
    pub(crate) tool_slot: usize,
    pub(crate) tool_after: u64,
}

impl Region {
    pub fn import(world: &World, origin: IVec3) -> Result<Self, String> {
        let (backend, initial) = backend::Cpu::import(world, origin)?;
        let published = Arc::new(initial);
        Ok(Self {
            backend,
            gpu: None,
            pending: VecDeque::with_capacity(COMMAND_CAPACITY),
            results: VecDeque::from([published.clone()]),
            published,
            completed: 0,
            next_sequence: 1,
            outcomes: VecDeque::with_capacity(COMMAND_CAPACITY),
            owned: Default::default(),
            inventory_bound: false,
            tool_slot: 0,
            tool_after: 0,
        })
    }

    /// Failure means *not accepted*: caller retains the action and its resources.
    /// Accepted commands beyond the per-tick quota stay FIFO, without coalescing.
    pub fn submit(&mut self, player_tick: u64, at: IVec3, cell: Cell) -> Result<Command, String> {
        self.enqueue(player_tick, at, cell, None)
    }

    fn enqueue(&mut self, player_tick: u64, at: IVec3, cell: Cell, expected: Option<Cell>) -> Result<Command, String> {
        if player_tick / engine_core::time::PLAYER_STEPS_PER_CA != self.completed {
            return Err("edit submitted under a different CPU clock".into());
        }
        if !self.published.contains(at) {
            return Err("edit outside CPU region (halo is immutable)".into());
        }
        backend::validate(cell)?;
        if self.pending.len() + self.outcomes.len() == COMMAND_CAPACITY {
            return Err("CPU command/outcome capacity full; consume results or advance before retrying".into());
        }
        let next = self
            .next_sequence
            .checked_add(1)
            .ok_or("command sequence exhausted")?;
        // Both backends seal the due CA batch before its player step. GPU
        // readiness can span frames; submissions must not gain a CPU-only slot.
        let next_player = player_tick.checked_add(1).ok_or("player tick exhausted")?;
        let sealed = next_player % engine_core::time::PLAYER_STEPS_PER_CA == 0
            || self.gpu.as_ref().is_some_and(|gpu| gpu.is_staging());
        let offset = if sealed { 2 } else { 1 };
        let earliest_ca_tick = self.completed.checked_add(offset).ok_or("CA tick exhausted")?;
        let command = Command {
            sequence: self.next_sequence,
            submitted_player_tick: player_tick,
            earliest_ca_tick,
            at,
            cell,
            expected,
        };
        self.pending.push_back(command);
        self.next_sequence = next;
        Ok(command)
    }

    pub fn ready(&mut self, tick: u64) -> Result<bool, String> {
        if self.completed.checked_add(1) != Some(tick) {
            return Err("out-of-order CA tick".into());
        }
        u32::try_from(tick - 1).map_err(|_| "CPU kernel tick exhausted")?;
        let due = tick.saturating_sub(2);
        if !self.results.iter().any(|s| s.tick == due) {
            return Err(format!("authority stalled: missing CA snapshot {due}"));
        }
        match self.gpu.as_mut() {
            Some(gpu) => gpu.ready(&self.backend, tick, self.pending.iter().take(COMMANDS_PER_TICK)
                .take_while(|command| command.earliest_ca_tick <= tick).copied()),
            None => Ok(true),
        }
    }

    fn commands_for(&self, tick: u64) -> Vec<Command> {
        self.pending.iter().take(COMMANDS_PER_TICK)
            .take_while(|c| c.earliest_ca_tick <= tick).copied().collect()
    }

    pub fn enable_gpu(&mut self) -> Result<(), String> {
        if self.gpu.is_some() { return Err("GPU backend already active".into()); }
        self.gpu = Some(pollster::block_on(backend::gpu::Gpu::new(&self.backend))?);
        Ok(())
    }

    pub fn advance(&mut self, tick: u64) -> Result<(), String> {
        if !self.ready(tick)? { return Err("GPU result is still pending".into()); }
        let due = tick.saturating_sub(2);
        // Check the deadline BEFORE advancing authority or consuming commands.
        let published = self
            .results
            .iter()
            .find(|s| s.tick == due)
            .cloned()
            .ok_or_else(|| format!("authority stalled: missing CA snapshot {due}"))?;
        let result = if let Some(gpu) = self.gpu.as_mut() {
            let (backend, result) = gpu.take()?;
            self.backend = backend;
            result
        } else {
            let commands = self.commands_for(tick);
            self.backend.advance(tick, &commands)?
        };
        self.pending.drain(..result.outcomes.len());
        self.completed = tick;
        self.published = published;
        while self.results.front().is_some_and(|s| s.tick < due) {
            self.results.pop_front();
        }
        self.outcomes.extend(result.outcomes);
        self.results.push_back(Arc::new(result.snapshot));
        Ok(())
    }

    pub fn summary(&self) -> String {
        let latest = self.results.back().unwrap_or(&self.published);
        format!(
            "\"ca_completed\":{},\"ca_published\":{},\"ca_applied_sequence\":{},\"ca_visible_sequence\":{},\"ca_pending\":{},\"ca_hash\":\"{:016x}\",\"ca_stalled_blocks\":{},\"ca_outcomes\":{},\"ca_owned\":{},\"ca_inventory_bound\":{}",
            self.completed,
            self.published.tick,
            latest.applied_sequence,
            self.published.applied_sequence,
            self.pending.len(),
            latest.hash,
            latest.stats.stalled_blocks,
            self.outcomes.len(),
            self.owned.len(),
            self.inventory_bound
        )
    }
}
