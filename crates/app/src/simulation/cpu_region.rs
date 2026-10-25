//! Opt-in headless region: exact cells, ordered ingress and a two-CA-tick mirror.
//! No conversion back into the eight-level legacy water map or ordinary saves.
mod backend;
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
    /// Last command incorporated, not merely admitted to the queue.
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

#[derive(Clone, Copy, Debug)]
pub struct Command {
    pub sequence: u64,
    pub submitted_player_tick: u64,
    pub earliest_ca_tick: u64,
    pub at: IVec3,
    pub cell: Cell,
}

/// A result is usable only under its actual tick. An asynchronous backend must
/// return unavailable before mutation when it cannot meet this exact request.
pub trait CaBackend {
    fn advance(&mut self, tick: u64, commands: &[Command]) -> Result<Snapshot, String>;
}

pub struct Region {
    backend: backend::Cpu,
    pending: VecDeque<Command>,
    results: VecDeque<Arc<Snapshot>>,
    pub published: Arc<Snapshot>,
    pub completed: u64,
    next_sequence: u64,
}

impl Region {
    pub fn import(world: &World, origin: IVec3) -> Result<Self, String> {
        let (backend, initial) = backend::Cpu::import(world, origin)?;
        let published = Arc::new(initial);
        Ok(Self {
            backend,
            pending: VecDeque::with_capacity(COMMAND_CAPACITY),
            results: VecDeque::from([published.clone()]),
            published,
            completed: 0,
            next_sequence: 1,
        })
    }

    /// Failure means *not accepted*: caller retains the action and its resources.
    /// Accepted commands beyond the per-tick quota stay FIFO, without coalescing.
    pub fn submit(&mut self, player_tick: u64, at: IVec3, cell: Cell) -> Result<Command, String> {
        if !self.published.contains(at) {
            return Err("edit outside CPU region (halo is immutable)".into());
        }
        backend::validate(cell)?;
        if self.pending.len() == COMMAND_CAPACITY {
            return Err("CPU command queue full; edit not accepted, retry after advancing".into());
        }
        let next = self
            .next_sequence
            .checked_add(1)
            .ok_or("command sequence exhausted")?;
        let earliest_ca_tick = self.completed.checked_add(1).ok_or("CA tick exhausted")?;
        let command = Command {
            sequence: self.next_sequence,
            submitted_player_tick: player_tick,
            earliest_ca_tick,
            at,
            cell,
        };
        self.pending.push_back(command);
        self.next_sequence = next;
        Ok(command)
    }

    pub fn ready(&self, tick: u64) -> Result<(), String> {
        if self.completed.checked_add(1) != Some(tick) {
            return Err("out-of-order CA tick".into());
        }
        u32::try_from(tick - 1).map_err(|_| "CPU kernel tick exhausted")?;
        let due = tick.saturating_sub(2);
        if !self.results.iter().any(|s| s.tick == due) {
            return Err(format!("authority stalled: missing CA snapshot {due}"));
        }
        Ok(())
    }

    pub fn advance(&mut self, tick: u64) -> Result<(), String> {
        self.ready(tick)?;
        let due = tick.saturating_sub(2);
        // Check the deadline BEFORE advancing authority or consuming commands.
        let published = self
            .results
            .iter()
            .find(|s| s.tick == due)
            .cloned()
            .ok_or_else(|| format!("authority stalled: missing CA snapshot {due}"))?;
        let commands: Vec<_> = self
            .pending
            .iter()
            .take(COMMANDS_PER_TICK)
            .take_while(|c| c.earliest_ca_tick <= tick)
            .copied()
            .collect();
        let result = self.backend.advance(tick, &commands)?;
        self.pending.drain(..commands.len());
        self.completed = tick;
        self.published = published;
        while self.results.front().is_some_and(|s| s.tick < due) {
            self.results.pop_front();
        }
        self.results.push_back(Arc::new(result));
        Ok(())
    }

    pub fn summary(&self) -> String {
        let latest = self.results.back().unwrap_or(&self.published);
        format!(
            "\"ca_completed\":{},\"ca_published\":{},\"ca_applied_sequence\":{},\"ca_visible_sequence\":{},\"ca_pending\":{},\"ca_hash\":\"{:016x}\",\"ca_stalled_blocks\":{}",
            self.completed,
            self.published.tick,
            latest.applied_sequence,
            self.published.applied_sequence,
            self.pending.len(),
            latest.hash,
            latest.stats.stalled_blocks
        )
    }
}
