//! Bucketed agenda: one slot per due tick inside a sliding window.
//!
//! Keys below the window sit in `late`, keys beyond it in `far`; both are
//! ordered sets, so any insert is legal. Inside the window a slot is an
//! unsorted list until it is first drained, then it is sorted once and popped
//! from the back. The pop sequence equals [`super::Ordered`]'s for every input.

use super::{Key, Queue};
use std::{collections::{BTreeMap, BTreeSet, HashSet}, hash::{BuildHasherDefault, DefaultHasher}};

type Membership = HashSet<u64, BuildHasherDefault<DefaultHasher>>;
const DENSE_AFTER: usize = 64;

/// Sliding-window bucket queue with the reference total order.
#[derive(Clone, Debug)]
pub struct Wheel {
    mask: u64,
    /// Every slot key has `due` in `base..base + size`.
    base: u64,
    /// `site << 32 | actor`, descending once the slot's `sorted` bit is set.
    slots: Vec<Vec<u64>>,
    sorted: Vec<u64>,
    occupied: Vec<u64>,
    /// Bit `w` set when `occupied[w]` is nonzero.
    summary: Vec<u64>,
    in_slots: usize,
    // Membership only; iteration order never affects queue order.
    dense: BTreeMap<usize, Membership>,
    late: BTreeSet<Key>,
    far: BTreeSet<Key>,
}

impl Default for Wheel {
    fn default() -> Self { Self::with_bits(14) }
}

const fn pack(key: Key) -> u64 { (key.site as u64) << 32 | key.actor as u64 }
const fn unpack(due: u64, e: u64) -> Key { Key::new(due, (e >> 32) as u32, e as u32) }

impl Wheel {
    /// A window of `2^bits` ticks; `bits` is clamped to `6..=20`.
    pub fn with_bits(bits: u32) -> Self {
        let size = 1usize << bits.clamp(6, 20);
        let words = size / 64;
        Self {
            mask: size as u64 - 1,
            base: 0,
            slots: vec![Vec::new(); size],
            sorted: vec![0; words],
            occupied: vec![0; words],
            summary: vec![0; words.div_ceil(64)],
            in_slots: 0,
            dense: BTreeMap::new(),
            late: BTreeSet::new(),
            far: BTreeSet::new(),
        }
    }

    fn size(&self) -> u64 { self.mask + 1 }
    fn slot_of(&self, due: u64) -> usize { (due & self.mask) as usize }
    fn due_of(&self, slot: usize) -> u64 { self.base + ((slot as u64).wrapping_sub(self.base) & self.mask) }
    fn bit(words: &[u64], i: usize) -> bool { words[i >> 6] >> (i & 63) & 1 == 1 }

    fn mark(&mut self, i: usize) {
        self.occupied[i >> 6] |= 1 << (i & 63);
        self.summary[i >> 12] |= 1 << (i >> 6 & 63);
    }

    fn unmark(&mut self, i: usize) {
        self.dense.remove(&i);
        self.occupied[i >> 6] &= !(1 << (i & 63));
        self.sorted[i >> 6] &= !(1 << (i & 63));
        if self.occupied[i >> 6] == 0 { self.summary[i >> 12] &= !(1 << (i >> 6 & 63)); }
    }

    /// First occupied slot at index `from` or above.
    fn scan(&self, from: usize) -> Option<usize> {
        let mut w = from >> 6;
        let head = self.occupied.get(w)? & (!0u64 << (from & 63));
        if head != 0 { return Some(w << 6 | head.trailing_zeros() as usize); }
        w += 1;
        let mut s = w >> 6;
        let mut bits = self.summary.get(s)? & (!0u64 << (w & 63));
        loop {
            if bits != 0 {
                let w = s << 6 | bits.trailing_zeros() as usize;
                return Some(w << 6 | self.occupied[w].trailing_zeros() as usize);
            }
            s += 1;
            bits = *self.summary.get(s)?;
        }
    }

    /// Occupied slot with the lowest due tick.
    fn first_slot(&self) -> Option<usize> {
        if self.in_slots == 0 { return None; }
        self.scan(self.slot_of(self.base)).or_else(|| self.scan(0))
    }

    fn push(&mut self, due: u64, e: u64) {
        let i = self.slot_of(due);
        let draining = Self::bit(&self.sorted, i);
        let slot = &mut self.slots[i];
        if draining {
            let at = slot.partition_point(|&other| other > e);
            if slot.get(at) == Some(&e) { return; }
            slot.insert(at, e);
        } else {
            if let Some(members) = self.dense.get_mut(&i) {
                if !members.insert(e) { return; }
            } else if slot.len() >= DENSE_AFTER {
                let mut members: Membership = slot.iter().copied().collect();
                let inserted = members.insert(e);
                self.dense.insert(i, members);
                if !inserted { return; }
            } else if slot.contains(&e) { return; }
            slot.push(e);
        }
        self.in_slots += 1;
        self.mark(i);
    }

    /// Moves the window start forward and pulls `far` keys that now fit.
    fn rebase(&mut self, base: u64) {
        self.base = base;
        while let Some(&k) = self.far.first() {
            if k.due - base >= self.size() { break; }
            self.far.pop_first();
            self.push(k.due, pack(k));
        }
    }

    fn slot_min(&self, i: usize) -> Option<u64> {
        if Self::bit(&self.sorted, i) { self.slots[i].last().copied() } else { self.slots[i].iter().copied().min() }
    }
}

impl Queue for Wheel {
    fn insert(&mut self, key: Key) {
        if key.due < self.base {
            self.late.insert(key);
        } else if key.due - self.base >= self.size() {
            self.far.insert(key);
        } else {
            self.push(key.due, pack(key));
        }
    }

    fn remove(&mut self, key: Key) -> bool {
        if key.due < self.base { return self.late.remove(&key); }
        if key.due - self.base >= self.size() { return self.far.remove(&key); }
        let (i, e) = (self.slot_of(key.due), pack(key));
        let draining = Self::bit(&self.sorted, i);
        let slot = &mut self.slots[i];
        if draining {
            let at = slot.partition_point(|&other| other > e);
            if slot.get(at) != Some(&e) { return false; }
            slot.remove(at);
        } else {
            let Some(at) = slot.iter().position(|&other| other == e) else { return false; };
            slot.swap_remove(at);
            if let Some(members) = self.dense.get_mut(&i) { members.remove(&e); }
        }
        self.in_slots -= 1;
        if self.slots[i].is_empty() { self.unmark(i); }
        true
    }

    fn first(&self) -> Option<Key> {
        if let Some(&k) = self.late.first() { return Some(k); }
        if let Some(i) = self.first_slot() { return self.slot_min(i).map(|e| unpack(self.due_of(i), e)); }
        self.far.first().copied()
    }

    fn pop_due(&mut self, now: u64) -> Option<Key> {
        if let Some(&k) = self.late.first() {
            return if k.due <= now { self.late.pop_first() } else { None };
        }
        if self.in_slots == 0 {
            let due = self.far.first()?.due;
            if due > now { return None; }
            self.rebase(due);
        }
        let i = self.first_slot()?;
        let due = self.due_of(i);
        if due > now { return None; }
        if due != self.base { self.rebase(due); }
        if !Self::bit(&self.sorted, i) {
            self.dense.remove(&i);
            self.slots[i].sort_unstable_by(|a, b| b.cmp(a));
            self.sorted[i >> 6] |= 1 << (i & 63);
        }
        let e = self.slots[i].pop()?;
        self.in_slots -= 1;
        if self.slots[i].is_empty() { self.unmark(i); }
        Some(unpack(due, e))
    }

    fn len(&self) -> usize { self.late.len() + self.in_slots + self.far.len() }

    fn clear(&mut self) {
        let bits = self.size().trailing_zeros();
        *self = Self::with_bits(bits);
    }

    fn due_count(&self, now: u64) -> usize {
        let limit = Key::new(now, u32::MAX, u32::MAX);
        let mut n = self.late.range(..=limit).count() + self.far.range(..=limit).count();
        if self.in_slots > 0 && now >= self.base {
            let span = (now - self.base).min(self.mask);
            for due in self.base..=self.base + span { n += self.slots[self.slot_of(due)].len(); }
        }
        n
    }
}

