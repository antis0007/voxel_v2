//! Per-client entity snapshots inside a byte budget (design section 5).
//!
//! Each entity in the client's interest set gains its priority every snapshot and
//! the highest are written first until the budget is spent, so near and fast
//! bodies update every snapshot and far ones less often, and none starves. Each is
//! written as a delta against its baseline: the newest state the client has
//! acknowledged. A body whose state equals that baseline, with nothing newer in
//! flight, costs nothing, so sleeping rigid bodies are free.
//!
//! Snapshots are unreliable. The client acknowledges the newest sequence plus a
//! mask of the 32 before it, and the server keeps 32 snapshots of history, so a
//! baseline is never older than the client's own history.
use crate::body::BodyState;
use crate::wire::{Reader, WireError, put_u64};
use std::collections::{HashMap, VecDeque};

/// Snapshots of history on both sides; also the oldest usable baseline.
pub const HISTORY: u32 = 32;

struct Track {
    /// Sequence the entity entered this client's set; older acks belong to an earlier incarnation.
    born: u32,
    accumulated: f32,
    baseline: Option<(u32, BodyState)>,
    /// Start sequence and state of the current run of identical sends.
    run: Option<(u32, BodyState)>,
}

fn newer(a: u32, b: u32) -> bool {
    a != b && a.wrapping_sub(b) < 1 << 31
}

fn varint_len(v: u64) -> usize {
    (64 - v.leading_zeros()).max(1).div_ceil(7) as usize
}

/// Server side, one per client.
#[derive(Default)]
pub struct SnapshotWriter {
    sequence: u32,
    tracks: HashMap<u32, Track>,
    history: VecDeque<(u32, Vec<(u32, BodyState)>)>,
    entry: Vec<u8>,
    body: Vec<u8>,
}

impl SnapshotWriter {
    /// Writes one snapshot of `bodies` (id, state, priority per snapshot) into `out`
    /// within `budget` bytes; returns the entity count written.
    pub fn write(&mut self, bodies: &[(u32, BodyState, f32)], budget: usize, out: &mut Vec<u8>) -> usize {
        self.sequence = self.sequence.wrapping_add(1);
        let seq = self.sequence;
        let mut due: Vec<(f32, u32, BodyState)> = Vec::new();
        for &(id, state, priority) in bodies {
            let track = self.tracks.entry(id).or_insert(Track { born: seq, accumulated: 0.0, baseline: None, run: None });
            let settled = match track.baseline {
                // The client shows the newest state it received, so every send after the
                // baseline must also carry this state.
                Some((b, base)) => base == state && track.run.is_none_or(|(start, s)| s == state && !newer(start, b)),
                None => false,
            };
            if settled {
                track.accumulated = 0.0;
            } else {
                track.accumulated += priority.max(0.0);
                due.push((track.accumulated, id, state));
            }
        }
        due.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));

        self.body.clear();
        let mut sent = Vec::new();
        for (_, id, state) in due {
            let Some(track) = self.tracks.get_mut(&id) else { continue };
            // A baseline older than the client's history goes as a full state.
            let (back, base) = match track.baseline {
                Some((b, base)) if seq.wrapping_sub(b) <= HISTORY => (seq.wrapping_sub(b) as u8, base),
                _ => (0, BodyState::default()),
            };
            self.entry.clear();
            put_u64(&mut self.entry, u64::from(id));
            self.entry.push(back);
            state.write_delta(&base, &mut self.entry);
            let header = varint_len(u64::from(seq)) + varint_len(sent.len() as u64 + 1);
            if header + self.body.len() + self.entry.len() > budget {
                continue;
            }
            self.body.extend_from_slice(&self.entry);
            track.accumulated = 0.0;
            if track.run.is_none_or(|(_, s)| s != state) {
                track.run = Some((seq, state));
            }
            sent.push((id, state));
        }
        let count = sent.len();
        put_u64(out, u64::from(seq));
        put_u64(out, count as u64);
        out.extend_from_slice(&self.body);
        self.history.push_back((seq, sent));
        while self.history.len() > HISTORY as usize {
            self.history.pop_front();
        }
        count
    }

    /// The client received `newest` and each `newest - 1 - i` whose bit `i` is set.
    pub fn acknowledge(&mut self, newest: u32, earlier: u32) {
        for (seq, states) in &self.history {
            let back = newest.wrapping_sub(*seq);
            let received = back == 0 || (1..=32).contains(&back) && earlier & (1 << (back - 1)) != 0;
            if !received {
                continue;
            }
            for (id, state) in states {
                if let Some(track) = self.tracks.get_mut(id)
                    && !newer(track.born, *seq)
                    && track.baseline.is_none_or(|(b, _)| newer(*seq, b))
                {
                    track.baseline = Some((*seq, *state));
                }
            }
        }
    }

    /// Drops an entity that left the client's interest set or despawned.
    pub fn forget(&mut self, id: u32) {
        self.tracks.remove(&id);
    }
}

/// Client side.
#[derive(Default)]
pub struct SnapshotReader {
    states: HashMap<u32, VecDeque<(u32, BodyState)>>,
    newest: Option<u32>,
    earlier: u32,
}

impl SnapshotReader {
    /// Decodes one snapshot; returns its sequence and the states it carried.
    /// A snapshot that cannot be fully decoded changes nothing and is not acknowledged.
    pub fn read(&mut self, bytes: &[u8]) -> Result<(u32, Vec<(u32, BodyState)>), WireError> {
        let mut r = Reader::new(bytes);
        let seq = r.u32()?;
        let count = r.u64()?;
        let mut got = Vec::new();
        for _ in 0..count {
            let id = r.u32()?;
            let back = r.u8()?;
            let base = if back == 0 {
                BodyState::default()
            } else {
                let want = seq.wrapping_sub(u32::from(back));
                let ring = self.states.get(&id).ok_or(WireError("baseline missing"))?;
                ring.iter().find(|(s, _)| *s == want).ok_or(WireError("baseline missing"))?.1
            };
            got.push((id, BodyState::read_delta(&base, &mut r)?));
        }
        if !r.done() {
            return Err(WireError("trailing bytes"));
        }
        for &(id, state) in &got {
            let ring = self.states.entry(id).or_default();
            ring.push_back((seq, state));
            ring.retain(|(s, _)| seq.wrapping_sub(*s) <= HISTORY || newer(*s, seq));
        }
        self.mark(seq);
        Ok((seq, got))
    }

    fn mark(&mut self, seq: u32) {
        match self.newest {
            None => self.newest = Some(seq),
            Some(n) if newer(seq, n) => {
                let shift = seq.wrapping_sub(n);
                self.earlier = if shift > 32 { 0 } else { ((u64::from(self.earlier) << shift) | 1 << (shift - 1)) as u32 };
                self.newest = Some(seq);
            }
            Some(n) => {
                let back = n.wrapping_sub(seq);
                if (1..=32).contains(&back) {
                    self.earlier |= 1 << (back - 1);
                }
            }
        }
    }

    /// Newest sequence received and the mask of the 32 before it.
    pub fn ack(&self) -> Option<(u32, u32)> {
        self.newest.map(|n| (n, self.earlier))
    }

    pub fn forget(&mut self, id: u32) {
        self.states.remove(&id);
    }
}

#[cfg(test)]
mod tests;
