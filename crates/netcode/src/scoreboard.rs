//! Scoreboard: objectives, the three display slots, and per-player scores,
//! replicated as ordered changes on the reliable channel.
//!
//! The server coalesces between flushes, so a counter that ticks every frame sends
//! its latest value once. A joining client gets `Board::snapshot` and then the
//! same change stream. Decoding validates every change, so a hostile stream
//! cannot grow the board past its limits.
use crate::wire::{Reader, WireError, put_i64, put_u64};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_OBJECTIVES: usize = 16;
pub const MAX_LABEL_BYTES: usize = 24;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    /// A column in the player list.
    List,
    Sidebar,
    BelowName,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    Count,
    Seconds,
    Metres,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Order {
    HighFirst,
    LowFirst,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Objective {
    pub label: String,
    pub unit: Unit,
    pub order: Order,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change {
    Define { id: u8, objective: Objective },
    Remove { id: u8 },
    Show { slot: Slot, id: Option<u8> },
    Score { id: u8, player: u32, value: i64 },
    /// The player left; drops all their scores.
    Clear { player: u32 },
}

/// A score as shown: plain count, `m:ss` or `h:mm:ss`, or metres.
pub fn format_value(value: i64, unit: Unit) -> String {
    match unit {
        Unit::Count => value.to_string(),
        Unit::Metres => format!("{value} m"),
        Unit::Seconds => {
            let (sign, s) = if value < 0 { ("-", value.unsigned_abs()) } else { ("", value.unsigned_abs()) };
            match s {
                s if s >= 3_600 => format!("{sign}{}:{:02}:{:02}", s / 3_600, s / 60 % 60, s % 60),
                s => format!("{sign}{}:{:02}", s / 60, s % 60),
            }
        }
    }
}

fn valid_label(label: &str) -> bool {
    !label.trim().is_empty() && label.len() <= MAX_LABEL_BYTES && !label.chars().any(char::is_control)
}

/// Board state, on the server and on each client.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Board {
    objectives: BTreeMap<u8, Objective>,
    slots: [Option<u8>; 3],
    scores: BTreeMap<(u8, u32), i64>,
}

impl Board {
    pub fn apply(&mut self, change: &Change) -> Result<(), WireError> {
        match change {
            Change::Define { id, objective } => {
                // 255 is reserved: `Show` writes `id + 1` so that 0 means an empty slot.
                if *id == u8::MAX || !valid_label(&objective.label) {
                    return Err(WireError("objective label"));
                }
                if !self.objectives.contains_key(id) && self.objectives.len() >= MAX_OBJECTIVES {
                    return Err(WireError("too many objectives"));
                }
                self.objectives.insert(*id, objective.clone());
            }
            Change::Remove { id } => {
                self.objectives.remove(id);
                self.scores.retain(|(o, _), _| o != id);
                self.slots.iter_mut().filter(|s| **s == Some(*id)).for_each(|s| *s = None);
            }
            Change::Show { slot, id } => {
                if id.is_some_and(|id| !self.objectives.contains_key(&id)) {
                    return Err(WireError("unknown objective"));
                }
                self.slots[*slot as usize] = *id;
            }
            Change::Score { id, player, value } => {
                if !self.objectives.contains_key(id) {
                    return Err(WireError("unknown objective"));
                }
                self.scores.insert((*id, *player), *value);
            }
            Change::Clear { player } => self.scores.retain(|(_, p), _| p != player),
        }
        Ok(())
    }

    /// Changes that rebuild this board from empty, for a joining client.
    pub fn snapshot(&self) -> Vec<Change> {
        let defines = self.objectives.iter().map(|(&id, o)| Change::Define { id, objective: o.clone() });
        let shows = [Slot::List, Slot::Sidebar, Slot::BelowName]
            .into_iter()
            .filter_map(|slot| self.slots[slot as usize].map(|id| Change::Show { slot, id: Some(id) }));
        let scores = self.scores.iter().map(|(&(id, player), &value)| Change::Score { id, player, value });
        defines.chain(shows).chain(scores).collect()
    }

    pub fn score(&self, id: u8, player: u32) -> Option<i64> {
        self.scores.get(&(id, player)).copied()
    }

    pub fn shown(&self, slot: Slot) -> Option<(u8, &Objective)> {
        let id = self.slots[slot as usize]?;
        self.objectives.get(&id).map(|o| (id, o))
    }

    /// The top `rows` players for an objective in its order, ties by player id.
    pub fn ranking(&self, id: u8, rows: usize) -> Vec<(u32, i64)> {
        let Some(objective) = self.objectives.get(&id) else { return Vec::new() };
        let mut all: Vec<(u32, i64)> = self.scores.range((id, 0)..=(id, u32::MAX)).map(|(&(_, p), &v)| (p, v)).collect();
        all.sort_by(|a, b| match objective.order {
            Order::HighFirst => b.1.cmp(&a.1),
            Order::LowFirst => a.1.cmp(&b.1),
        }
        .then(a.0.cmp(&b.0)));
        all.truncate(rows);
        all
    }
}

/// Server side: the board plus changes not yet flushed.
#[derive(Default)]
pub struct BoardWriter {
    board: Board,
    structure: Vec<Change>,
    dirty: BTreeSet<(u8, u32)>,
}

impl BoardWriter {
    pub fn board(&self) -> &Board {
        &self.board
    }

    /// Applies a structural change (define, remove, show, clear) and queues it.
    pub fn change(&mut self, change: Change) -> Result<(), WireError> {
        self.board.apply(&change)?;
        match change {
            Change::Score { id, player, .. } => {
                self.dirty.insert((id, player));
            }
            other => self.structure.push(other),
        }
        Ok(())
    }

    pub fn set(&mut self, id: u8, player: u32, value: i64) -> Result<(), WireError> {
        if self.board.score(id, player) == Some(value) {
            return Ok(());
        }
        self.change(Change::Score { id, player, value })
    }

    pub fn add(&mut self, id: u8, player: u32, delta: i64) -> Result<(), WireError> {
        let value = self.board.score(id, player).unwrap_or(0).saturating_add(delta);
        self.set(id, player, value)
    }

    /// Encodes everything since the last flush; empty when nothing changed.
    pub fn flush(&mut self) -> Vec<u8> {
        let mut changes = std::mem::take(&mut self.structure);
        for (id, player) in std::mem::take(&mut self.dirty) {
            if let Some(value) = self.board.score(id, player) {
                changes.push(Change::Score { id, player, value });
            }
        }
        if changes.is_empty() {
            return Vec::new();
        }
        encode(&changes)
    }
}

const DEFINE: u8 = 0;
const REMOVE: u8 = 1;
const SHOW: u8 = 2;
const SCORE: u8 = 3;
const CLEAR: u8 = 4;

pub fn encode(changes: &[Change]) -> Vec<u8> {
    let mut out = Vec::new();
    put_u64(&mut out, changes.len() as u64);
    for c in changes {
        match c {
            Change::Define { id, objective } => {
                out.extend([DEFINE, *id, objective.unit as u8, objective.order as u8]);
                put_u64(&mut out, objective.label.len() as u64);
                out.extend(objective.label.as_bytes());
            }
            Change::Remove { id } => out.extend([REMOVE, *id]),
            Change::Show { slot, id } => out.extend([SHOW, *slot as u8, id.map_or(0, |i| i.wrapping_add(1))]),
            Change::Score { id, player, value } => {
                out.extend([SCORE, *id]);
                put_u64(&mut out, u64::from(*player));
                put_i64(&mut out, *value);
            }
            Change::Clear { player } => {
                out.push(CLEAR);
                put_u64(&mut out, u64::from(*player));
            }
        }
    }
    out
}

/// Decodes and applies a flush to `board`; on error the board is left unchanged.
pub fn apply_flush(board: &mut Board, bytes: &[u8]) -> Result<Vec<Change>, WireError> {
    let mut r = Reader::new(bytes);
    let count = r.u64()?;
    let mut next = board.clone();
    let mut changes = Vec::new();
    for _ in 0..count {
        let change = match r.u8()? {
            DEFINE => {
                let id = r.u8()?;
                let unit = [Unit::Count, Unit::Seconds, Unit::Metres].get(usize::from(r.u8()?)).copied();
                let order = [Order::HighFirst, Order::LowFirst].get(usize::from(r.u8()?)).copied();
                let len = r.u64()?;
                if len > MAX_LABEL_BYTES as u64 {
                    return Err(WireError("objective label"));
                }
                let bytes: Vec<u8> = (0..len).map(|_| r.u8()).collect::<Result<_, _>>()?;
                let label = String::from_utf8(bytes).map_err(|_| WireError("objective label"))?;
                let (Some(unit), Some(order)) = (unit, order) else { return Err(WireError("objective kind")) };
                Change::Define { id, objective: Objective { label, unit, order } }
            }
            REMOVE => Change::Remove { id: r.u8()? },
            SHOW => {
                let slot = [Slot::List, Slot::Sidebar, Slot::BelowName].get(usize::from(r.u8()?)).copied();
                let id = r.u8()?.checked_sub(1);
                Change::Show { slot: slot.ok_or(WireError("slot"))?, id }
            }
            SCORE => Change::Score { id: r.u8()?, player: r.u32()?, value: r.i64()? },
            CLEAR => Change::Clear { player: r.u32()? },
            _ => return Err(WireError("unknown change")),
        };
        next.apply(&change)?;
        changes.push(change);
    }
    if !r.done() {
        return Err(WireError("trailing bytes"));
    }
    *board = next;
    Ok(changes)
}

#[cfg(test)]
mod tests;
