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
    /// Older than the kept window; send a full sync.
    Expired,
}

/// Bounded per-client results of applied operations.
pub struct Receipts<R> {
    capacity: usize,
    results: BTreeMap<u32, R>,
    /// Lowest sequence that can still be classified.
    floor: u32,
}

impl<R: Clone> Receipts<R> {
    pub fn new(capacity: usize) -> Self {
        Self { capacity: capacity.max(1), results: BTreeMap::new(), floor: 0 }
    }

    pub fn check(&self, seq: u32) -> Seen<R> {
        if seq < self.floor {
            return Seen::Expired;
        }
        match self.results.get(&seq) {
            Some(r) => Seen::Done(r.clone()),
            None => Seen::New,
        }
    }

    /// Stores the result of an applied operation; returns false if `seq` is
    /// expired or already recorded, in which case nothing changes.
    pub fn record(&mut self, seq: u32, result: R) -> bool {
        if seq < self.floor || self.results.contains_key(&seq) {
            return false;
        }
        self.results.insert(seq, result);
        while self.results.len() > self.capacity {
            if let Some((oldest, _)) = self.results.pop_first() {
                self.floor = oldest + 1;
            }
        }
        true
    }

    pub fn len(&self) -> usize {
        self.results.len()
    }

    pub fn is_empty(&self) -> bool {
        self.results.is_empty()
    }
}

/// Client-side sequence source; one per connection lifetime of a player key.
#[derive(Default)]
pub struct Sequencer {
    client: u32,
    next: u32,
}

impl Sequencer {
    pub fn new(client: u32) -> Self {
        Self { client, next: 0 }
    }

    pub fn issue(&mut self) -> OperationId {
        let id = OperationId { client: self.client, seq: self.next };
        self.next = self.next.wrapping_add(1);
        id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_returns_stored_result() {
        let mut r = Receipts::new(8);
        assert_eq!(r.check(0), Seen::<u8>::New);
        assert!(r.record(0, 7));
        assert_eq!(r.check(0), Seen::Done(7));
        assert!(!r.record(0, 9));
        assert_eq!(r.check(0), Seen::Done(7));
    }

    #[test]
    fn out_of_order_sequences_are_independent() {
        let mut r = Receipts::new(8);
        assert!(r.record(3, 'c'));
        assert_eq!(r.check(1), Seen::New);
        assert!(r.record(1, 'a'));
        assert_eq!(r.check(3), Seen::Done('c'));
    }

    #[test]
    fn evicted_sequences_expire_instead_of_reapplying() {
        let mut r = Receipts::new(2);
        for seq in 0..3 {
            assert!(r.record(seq, seq));
        }
        assert_eq!(r.len(), 2);
        assert_eq!(r.check(0), Seen::Expired);
        assert!(!r.record(0, 0));
        assert_eq!(r.check(1), Seen::Done(1));
    }

    #[test]
    fn last_shared_item_moves_once_under_resends() {
        // Two clients each resend a take of the last item three times; the
        // server applies each id once, so exactly one take succeeds.
        let mut stock = 1u32;
        let mut held = [0u32; 2];
        let mut receipts = [Receipts::new(16), Receipts::new(16)];
        let mut seqs = [Sequencer::new(0), Sequencer::new(1)];
        let ids = [seqs[0].issue(), seqs[1].issue()];
        for _ in 0..3 {
            for id in ids {
                let table = &mut receipts[id.client as usize];
                match table.check(id.seq) {
                    Seen::New => {
                        let ok = stock > 0;
                        if ok {
                            stock -= 1;
                            held[id.client as usize] += 1;
                        }
                        table.record(id.seq, ok);
                    }
                    Seen::Done(_) | Seen::Expired => {}
                }
            }
        }
        assert_eq!(stock, 0);
        assert_eq!(held.iter().sum::<u32>(), 1);
        assert_eq!(receipts[0].check(ids[0].seq), Seen::Done(true));
        assert_eq!(receipts[1].check(ids[1].seq), Seen::Done(false));
    }

    #[test]
    fn sequencer_numbers_per_client() {
        let mut s = Sequencer::new(5);
        assert_eq!(s.issue(), OperationId { client: 5, seq: 0 });
        assert_eq!(s.issue(), OperationId { client: 5, seq: 1 });
    }
}
