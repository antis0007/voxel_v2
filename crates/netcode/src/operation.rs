//! Server-side receipts that make client operations apply at most once.
//!
//! A client numbers each consequential request (a transfer, a craft, a trade)
//! with an increasing sequence and resends it until it sees the result. The
//! server keeps the results of the most recent operations per client, so a
//! resend after a lost reply returns the stored result instead of applying the
//! operation again. Sequences older than the kept window are reported as
//! expired: the server can no longer tell whether they applied, so the caller
//! must answer with a full state sync rather than guess.
//!
//! The sequence namespace outlives connections and server restarts. A
//! reconnecting client resumes from [`Receipts::next_seq`] instead of zero, a
//! reused sequence with different contents is a [`Seen::Conflict`], and the
//! persisted [`Floor`] keeps every sequence issued before a restart from
//! becoming executable again once its receipt is gone.
//!
//! Revision checks (`container`) reject actions made against stale contents;
//! receipts reject the same action arriving twice against current contents.

use std::collections::BTreeMap;

/// One client operation: who sent it and its sequence on that client.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OperationId {
    pub client: u32,
    pub seq: u32,
}

/// What the server should do with an incoming operation.
#[derive(Clone, Debug, PartialEq)]
pub enum Seen<R> {
    /// Not applied yet: apply it, then call [`Receipts::record`].
    New,
    /// Already applied: send this result again.
    Done(R),
    /// Sequence already applied with different contents; reject it.
    Conflict,
    /// Older than the kept window; send a full sync.
    Expired,
}

/// Persisted part of a client's receipts: nothing below `next` may apply.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Floor {
    pub next: u32,
}

struct Entry<R> {
    digest: u64,
    result: R,
}

/// Bounded per-client results of applied operations.
pub struct Receipts<R> {
    capacity: usize,
    results: BTreeMap<u32, Entry<R>>,
    /// Lowest sequence that can still be classified.
    floor: u32,
    /// One past the highest sequence recorded or restored.
    next: u32,
}

impl<R: Clone> Receipts<R> {
    pub fn new(capacity: usize) -> Self {
        Self::restore(capacity, Floor::default())
    }

    /// Rebuilds receipts after a restart; earlier sequences are expired.
    ///
    /// Results are not persisted, so a sequence below the floor that never
    /// applied (it arrived out of order) also expires; the client resyncs.
    pub fn restore(capacity: usize, floor: Floor) -> Self {
        Self { capacity: capacity.max(1), results: BTreeMap::new(), floor: floor.next, next: floor.next }
    }

    /// `digest` hashes the operation's canonical contents.
    pub fn check(&self, seq: u32, digest: u64) -> Seen<R> {
        if seq < self.floor {
            return Seen::Expired;
        }
        match self.results.get(&seq) {
            Some(e) if e.digest == digest => Seen::Done(e.result.clone()),
            Some(_) => Seen::Conflict,
            None => Seen::New,
        }
    }

    /// Stores the result of an applied operation; returns false if `seq` is
    /// expired or already recorded, in which case nothing changes.
    pub fn record(&mut self, seq: u32, digest: u64, result: R) -> bool {
        if seq < self.floor || self.results.contains_key(&seq) {
            return false;
        }
        self.results.insert(seq, Entry { digest, result });
        self.next = self.next.max(seq.saturating_add(1));
        while self.results.len() > self.capacity {
            if let Some((oldest, _)) = self.results.pop_first() {
                self.floor = oldest + 1;
            }
        }
        true
    }

    /// Sequence a reconnecting client continues from.
    pub fn next_seq(&self) -> u32 {
        self.next
    }

    /// State to persist with the world so a restart cannot reapply.
    pub fn floor(&self) -> Floor {
        Floor { next: self.next }
    }

    pub fn len(&self) -> usize {
        self.results.len()
    }

    pub fn is_empty(&self) -> bool {
        self.results.is_empty()
    }
}

/// Client-side sequence source for one player key.
#[derive(Default)]
pub struct Sequencer {
    client: u32,
    next: u32,
}

impl Sequencer {
    pub fn new(client: u32) -> Self {
        Self { client, next: 0 }
    }

    /// Continues from the server's [`Receipts::next_seq`] after a reconnect.
    pub fn resume(client: u32, next: u32) -> Self {
        Self { client, next }
    }

    /// Returns `None` once the sequence space is used up; the key needs a
    /// new client id, because wrapping would collide with old receipts.
    pub fn issue(&mut self) -> Option<OperationId> {
        let seq = self.next;
        self.next = seq.checked_add(1)?;
        Some(OperationId { client: self.client, seq })
    }
}

#[cfg(test)]
mod tests;
