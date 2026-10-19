//! Durable sparse cellular activity. Sleeping chunks allocate no work masks.
use super::{CHUNK_SIZE, ChunkData, chunk_coord_from_world_block};
use anyhow::{Result, ensure};
use glam::IVec3;
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};

pub const MAX_ACTIVE_CHUNKS: usize = 4096;
pub const WORDS: usize = 512;
// Cells whose rules can depend on an edit: face neighbours and grains one
// level above a diagonal landing cell. Admission and wakeup use the same set.
pub(crate) const WAKE_NEIGHBORS: [IVec3; 10] = [
    IVec3::X, IVec3::NEG_X, IVec3::Y, IVec3::NEG_Y, IVec3::Z, IVec3::NEG_Z,
    IVec3::new(1, 1, 0), IVec3::new(-1, 1, 0),
    IVec3::new(0, 1, 1), IVec3::new(0, 1, -1),
];
const MAX_DEFERRED_REGIONS: usize = 8192;
#[derive(Clone, Copy)]
struct Sweep {
    min: IVec3,
    max: IVec3,
    cursor: IVec3,
}
pub type Mask = [u64; WORDS];
pub struct Batch {
    cells: [IVec3; 256],
    len: usize,
}
impl Batch {
    fn new() -> Self {
        Self {
            cells: [IVec3::ZERO; 256],
            len: 0,
        }
    }
    fn push(&mut self, p: IVec3) {
        self.cells[self.len] = p;
        self.len += 1;
    }
}
impl std::ops::Deref for Batch {
    type Target = [IVec3];
    fn deref(&self) -> &[IVec3] {
        &self.cells[..self.len]
    }
}
struct Work {
    bits: Arc<Mask>,
    cursor: usize,
    count: usize,
}
#[derive(Default)]
pub struct Activity {
    work: HashMap<IVec3, Work>,
    fair: VecDeque<IVec3>,
    pending: usize,
    deferred: HashMap<IVec3, [u64; 8]>,
    sweep: Option<Sweep>,
    pub tick: u64,
    /// Full admission coalesces external edits into a durable rescan request.
    pub recovery: bool,
    pub recovery_generation: u64,
}
pub struct Snapshot {
    masks: Vec<(IVec3, Arc<Mask>, usize)>,
    tick: u64,
    recovery: bool,
    deferred: Vec<(IVec3, [u64; 8])>,
    sweep: Option<Sweep>,
}
impl Activity {
    pub fn chunks(&self) -> usize {
        self.work.len()
    }
    pub fn pending(&self) -> usize {
        self.pending
    }
    pub fn contains(&self, c: IVec3) -> bool {
        self.work.get(&c).is_some_and(|w| w.count > 0)
    }
    pub fn has_deferred(&self) -> bool {
        !self.deferred.is_empty() || self.sweep.is_some()
    }
    fn defer(&mut self, c: IVec3) {
        let region = IVec3::new(c.x.div_euclid(8), c.y.div_euclid(8), c.z.div_euclid(8));
        if self.deferred.contains_key(&region) || self.deferred.len() < MAX_DEFERRED_REGIONS {
            let i = (c.y.rem_euclid(8) * 64 + c.z.rem_euclid(8) * 8 + c.x.rem_euclid(8)) as usize;
            self.deferred.entry(region).or_insert([0; 8])[i / 64] |= 1u64 << (i % 64);
        } else {
            // Emergency coarsening stays bounded even after millions of sparse
            // deferred chunks. Revisit a superset, never discard a wakeup.
            self.sweep = Some(match self.sweep {
                Some(s) => {
                    let min = s.min.min(c);
                    Sweep {
                        min,
                        max: s.max.max(c),
                        cursor: min,
                    }
                }
                None => Sweep {
                    min: c,
                    max: c,
                    cursor: c,
                },
            });
        }
    }
    pub fn promote(&mut self) {
        if self.work.len() >= MAX_ACTIVE_CHUNKS - 16 {
            return;
        }
        let next = if let Some((&region, mask)) = self.deferred.iter().next() {
            let word = mask.iter().position(|&m| m != 0).unwrap();
            let bit = mask[word].trailing_zeros() as usize;
            let i = word * 64 + bit;
            let mask = self.deferred.get_mut(&region).unwrap();
            mask[word] &= mask[word] - 1;
            if mask.iter().all(|&m| m == 0) {
                self.deferred.remove(&region);
            }
            Some(region * 8 + IVec3::new((i % 8) as i32, (i / 64) as i32, ((i / 8) % 8) as i32))
        } else if let Some(s) = &mut self.sweep {
            let c = s.cursor;
            if s.cursor.x < s.max.x {
                s.cursor.x += 1;
            } else if s.cursor.z < s.max.z {
                s.cursor.x = s.min.x;
                s.cursor.z += 1;
            } else if s.cursor.y < s.max.y {
                s.cursor.x = s.min.x;
                s.cursor.z = s.min.z;
                s.cursor.y += 1;
            } else {
                self.sweep = None;
            }
            Some(c)
        } else {
            None
        };
        if let Some(c) = next {
            self.merge(c, &[u64::MAX; WORDS]);
        }
    }
    pub fn request_recovery(&mut self) {
        self.recovery = true;
        self.recovery_generation = self.recovery_generation.wrapping_add(1);
    }
    pub fn can_wake(&self, coords: &[IVec3]) -> bool {
        let extra = coords
            .iter()
            .enumerate()
            .filter(|(i, c)| !coords[..*i].contains(c) && !self.work.contains_key(c))
            .count();
        self.work.len() + extra <= MAX_ACTIVE_CHUNKS
    }
    pub fn wake(&mut self, p: IVec3) -> bool {
        let c = chunk_coord_from_world_block(p);
        if !self.work.contains_key(&c) {
            if self.work.len() >= MAX_ACTIVE_CHUNKS {
                self.defer(c);
                return false;
            }
            self.work.insert(
                c,
                Work {
                    bits: Arc::new([0; WORDS]),
                    cursor: 0,
                    count: 0,
                },
            );
            self.fair.push_back(c);
        }
        let i = ChunkData::local_index(p.x, p.y, p.z);
        let w = self.work.get_mut(&c).unwrap();
        let bit = 1u64 << (i % 64);
        if w.bits[i / 64] & bit == 0 {
            Arc::make_mut(&mut w.bits)[i / 64] |= bit;
            w.count += 1;
            self.pending += 1;
        }
        true
    }
    pub fn wake_neighbors(&mut self, p: IVec3) {
        self.wake(p);
        for d in WAKE_NEIGHBORS {
            self.wake(p + d);
        }
    }
    pub fn merge(&mut self, c: IVec3, bits: &Mask) -> bool {
        if bits.iter().all(|&b| b == 0) {
            return true;
        }
        if !self.work.contains_key(&c) {
            if self.work.len() >= MAX_ACTIVE_CHUNKS {
                self.defer(c);
                return false;
            }
            self.work.insert(
                c,
                Work {
                    bits: Arc::new([0; WORDS]),
                    cursor: 0,
                    count: 0,
                },
            );
            self.fair.push_back(c);
        }
        let w = self.work.get_mut(&c).unwrap();
        let target = Arc::make_mut(&mut w.bits);
        for (old, &new) in target.iter_mut().zip(bits) {
            let added = (new & !*old).count_ones() as usize;
            w.count += added;
            self.pending += added;
            *old |= new;
        }
        true
    }
    pub fn next_fair(&mut self) -> Option<IVec3> {
        // Empty preferred-lane entries are retired here, without stale queue duplicates.
        for _ in 0..256 {
            let c = self.fair.pop_front()?;
            if self.contains(c) {
                self.fair.push_back(c);
                return Some(c);
            }
            self.work.remove(&c);
        }
        None
    }
    pub fn take(&mut self, c: IVec3, limit: usize) -> Batch {
        let limit = limit.min(256);
        let Some(w) = self.work.get_mut(&c) else {
            return Batch::new();
        };
        let mut result = Batch::new();
        if w.count == 0 || limit == 0 {
            return result;
        }
        let bits = Arc::make_mut(&mut w.bits);
        // Advance past consumed bit positions, including bits reawakened by a
        // neighbour. Continuously moving low bits cannot starve the rest.
        for _ in 0..(WORDS + limit) {
            let word = w.cursor / 64;
            let offset = w.cursor % 64;
            let candidates = bits[word] & (u64::MAX << offset);
            if candidates == 0 {
                w.cursor = ((word + 1) % WORDS) * 64;
                continue;
            }
            let b = candidates.trailing_zeros() as usize;
            let i = word * 64 + b;
            bits[word] &= !(1u64 << b);
            w.cursor = (i + 1) % (WORDS * 64);
            result.push(
                c * CHUNK_SIZE
                    + IVec3::new((i % 32) as i32, (i / 1024) as i32, ((i / 32) % 32) as i32),
            );
            w.count -= 1;
            self.pending -= 1;
            if result.len() == limit || w.count == 0 {
                break;
            }
        }
        result
    }
    pub fn restrict(&mut self, c: IVec3, candidates: &Mask) {
        let Some(w) = self.work.get_mut(&c) else {
            return;
        };
        for (bits, &keep) in Arc::make_mut(&mut w.bits).iter_mut().zip(candidates) {
            let removed = (*bits & !keep).count_ones() as usize;
            *bits &= keep;
            w.count -= removed;
            self.pending -= removed;
        }
    }
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            masks: self
                .work
                .iter()
                .filter(|(_, w)| w.count > 0)
                .map(|(&c, w)| (c, Arc::clone(&w.bits), w.cursor))
                .collect(),
            tick: self.tick,
            recovery: self.recovery,
            deferred: self.deferred.iter().map(|(&c, &mask)| (c, mask)).collect(),
            sweep: self.sweep,
        }
    }
    pub fn decode(next: &mut impl FnMut() -> Result<u32>, extended: bool) -> Result<Self> {
        let mut state = Self::default();
        state.tick = next()? as u64 | ((next()? as u64) << 32);
        let recovery = next()?;
        ensure!(recovery <= 1, "Invalid simulation recovery flag");
        state.recovery = recovery != 0;
        let count = next()? as usize;
        ensure!(
            count <= MAX_ACTIVE_CHUNKS,
            "Too many active simulation chunks"
        );
        for _ in 0..count {
            let c = IVec3::new(next()? as i32, next()? as i32, next()? as i32);
            ensure!(
                c.to_array().iter().all(|v| v.unsigned_abs() <= 32_000),
                "Invalid simulation coordinate"
            );
            ensure!(!state.work.contains_key(&c), "Duplicate simulation chunk");
            let cursor = next()? as usize;
            ensure!(cursor < WORDS * 64, "Invalid simulation cursor");
            let n = next()? as usize;
            ensure!((1..=WORDS).contains(&n), "Invalid simulation mask size");
            let mut mask = [0; WORDS];
            for _ in 0..n {
                let i = next()? as usize;
                ensure!(i < WORDS && mask[i] == 0, "Invalid simulation mask index");
                mask[i] = next()? as u64 | ((next()? as u64) << 32);
                ensure!(mask[i] != 0, "Empty simulation mask word");
            }
            state.merge(c, &mask);
            state.work.get_mut(&c).unwrap().cursor = cursor;
        }
        if extended {
            let count = next()? as usize;
            ensure!(
                count <= MAX_DEFERRED_REGIONS,
                "Too many deferred simulation regions"
            );
            for _ in 0..count {
                let c = IVec3::new(next()? as i32, next()? as i32, next()? as i32);
                ensure!(
                    c.to_array().iter().all(|v| v.unsigned_abs() <= 4000),
                    "Invalid deferred simulation region"
                );
                ensure!(
                    !state.deferred.contains_key(&c),
                    "Duplicate deferred simulation region"
                );
                let mut mask = [0u64; 8];
                for word in &mut mask {
                    *word = next()? as u64 | ((next()? as u64) << 32);
                }
                ensure!(
                    mask.iter().any(|&m| m != 0),
                    "Empty deferred simulation region"
                );
                state.deferred.insert(c, mask);
            }
            let sweep = next()?;
            ensure!(sweep <= 1, "Invalid simulation sweep flag");
            if sweep != 0 {
                let mut point = || -> Result<IVec3> {
                    let p = IVec3::new(next()? as i32, next()? as i32, next()? as i32);
                    ensure!(
                        p.to_array().iter().all(|v| v.unsigned_abs() <= 32_000),
                        "Invalid simulation sweep coordinate"
                    );
                    Ok(p)
                };
                let min = point()?;
                let max = point()?;
                let cursor = point()?;
                ensure!(
                    min.cmple(max).all() && cursor.cmpge(min).all() && cursor.cmple(max).all(),
                    "Invalid simulation sweep bounds"
                );
                state.sweep = Some(Sweep { min, max, cursor });
            }
        }
        Ok(state)
    }
}
impl Snapshot {
    /// Encoded by the save worker; capture only pins immutable 4 KiB masks.
    pub fn encode(&self, bytes: &mut Vec<u8>) {
        let push = |b: &mut Vec<u8>, v: u32| b.extend_from_slice(&v.to_le_bytes());
        push(bytes, self.tick as u32);
        push(bytes, (self.tick >> 32) as u32);
        push(bytes, self.recovery as u32);
        push(bytes, self.masks.len() as u32);
        for (c, mask, cursor) in &self.masks {
            for v in c.to_array() {
                push(bytes, v as u32);
            }
            push(bytes, *cursor as u32);
            push(bytes, mask.iter().filter(|&&v| v != 0).count() as u32);
            for (i, &word) in mask.iter().enumerate().filter(|(_, v)| **v != 0) {
                push(bytes, i as u32);
                push(bytes, word as u32);
                push(bytes, (word >> 32) as u32);
            }
        }
        push(bytes, self.deferred.len() as u32);
        for (c, mask) in &self.deferred {
            for v in c.to_array() {
                push(bytes, v as u32);
            }
            for word in mask {
                push(bytes, *word as u32);
                push(bytes, (word >> 32) as u32);
            }
        }
        push(bytes, self.sweep.is_some() as u32);
        if let Some(s) = self.sweep {
            for p in [s.min, s.max, s.cursor] {
                for v in p.to_array() {
                    push(bytes, v as u32);
                }
            }
        }
    }
}
