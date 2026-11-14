//! Partial water cells as the changes since the copy last sent to one joiner.
//!
//! Unchanged chunks share their cell maps with the previous snapshot, so the
//! host compares handles first and visits only chunks that changed or still
//! carry a wave. The section carries a digest of every level; the joiner
//! compares it with its own water and asks for a full list on a mismatch.
use crate::fast_hash::FastMap;
use crate::world::{
    self,
    water::{WaterLevels, WaterSnapshot},
};
use anyhow::{Result, ensure};
use engine_core::CHUNK_SIZE;
use engine_netcode::wire::{Reader, put_i64, put_u64};
use glam::{IVec3, Vec2};
use std::collections::HashSet;

pub(super) const MAX_CELLS: usize = 262_144;
const MAX_COORD: i64 = 1_000_000;
/// Level byte flag: two little-endian f32 velocity components follow.
const MOVING: u8 = 0x80;

/// What one joiner holds, as last sent.
#[derive(Default)]
pub struct Sent {
    /// `None` until the first full list.
    cells: Option<WaterSnapshot>,
    /// Nonzero velocities, by chunk.
    motion: FastMap<IVec3, FastMap<IVec3, Vec2>>,
    digest: u64,
}

struct Cell {
    p: IVec3,
    level: u8,
    velocity: Vec2,
}

impl Sent {
    /// Forgets the joiner's copy so the next section is a full list.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Appends the section body; returns true when it is a full list.
    pub fn write(&mut self, levels: &WaterLevels, out: &mut Vec<u8>) -> bool {
        let current = levels.snapshot_all();
        let full = self.cells.is_none();
        let (mut set, mut clear) = (Vec::new(), Vec::new());
        match self.cells.take() {
            None => {
                self.motion.clear();
                self.digest = 0;
                for (&p, &level) in current.iter() {
                    let velocity = levels.velocity(p);
                    self.note_motion(p, velocity);
                    self.digest = self.digest.wrapping_add(mix(p, level));
                    set.push(Cell { p, level, velocity });
                }
            }
            Some(old) => {
                let mut chunks: Vec<IVec3> = current.changed_chunks(&old).map(|(q, _, _)| q).collect();
                chunks.extend(self.motion.keys().copied());
                chunks.sort_unstable_by_key(|q| q.to_array());
                chunks.dedup();
                for q in chunks {
                    let min = q * CHUNK_SIZE;
                    let max = min + IVec3::splat(CHUNK_SIZE);
                    let waves = self.motion.remove(&q).unwrap_or_default();
                    for (&p, &level) in current.in_box(min, max) {
                        let velocity = levels.velocity(p);
                        self.note_motion(p, velocity);
                        let before = old.get(&p).copied();
                        if before != Some(level) || waves.get(&p).copied().unwrap_or(Vec2::ZERO) != velocity {
                            self.digest = self.digest.wrapping_sub(before.map_or(0, |b| mix(p, b))).wrapping_add(mix(p, level));
                            set.push(Cell { p, level, velocity });
                        }
                    }
                    for (&p, &level) in old.in_box(min, max) {
                        if current.get(&p).is_none() {
                            self.digest = self.digest.wrapping_sub(mix(p, level));
                            clear.push(p);
                        }
                    }
                }
            }
        }
        self.cells = Some(current);
        set.sort_unstable_by_key(|c| c.p.to_array());
        clear.sort_unstable_by_key(|p| p.to_array());
        out.extend_from_slice(&self.digest.to_le_bytes());
        put_u64(out, set.len() as u64);
        let mut previous = IVec3::ZERO;
        for c in &set {
            put_delta(out, &mut previous, c.p);
            if c.velocity == Vec2::ZERO {
                out.push(c.level);
            } else {
                out.push(c.level | MOVING);
                out.extend_from_slice(&c.velocity.x.to_le_bytes());
                out.extend_from_slice(&c.velocity.y.to_le_bytes());
            }
        }
        put_u64(out, clear.len() as u64);
        let mut previous = IVec3::ZERO;
        for &p in &clear {
            put_delta(out, &mut previous, p);
        }
        full
    }

    fn note_motion(&mut self, p: IVec3, velocity: Vec2) {
        if velocity != Vec2::ZERO {
            self.motion.entry(engine_core::chunk_coord_from_world_block(p)).or_default().insert(p, velocity);
        }
    }
}

/// A decoded water section, checked before anything is published.
pub struct Update {
    full: bool,
    pub digest: u64,
    set: Vec<Cell>,
    clear: Vec<IVec3>,
}

impl Update {
    pub fn read(body: &[u8], full: bool) -> Result<Self> {
        let mut r = Reader::new(body);
        let digest = u64::from_le_bytes(wire(r.take(8))?.try_into()?);
        let count = wire(r.u64())? as usize;
        ensure!(count <= MAX_CELLS && count <= r.remaining(), "water replica exceeds budget");
        let mut set = Vec::with_capacity(count);
        let mut previous = IVec3::ZERO;
        for _ in 0..count {
            let p = read_delta(&mut r, &mut previous)?;
            let byte = wire(r.u8())?;
            let level = byte & !MOVING;
            let velocity = if byte & MOVING != 0 {
                let mut f = || -> Result<f32> { Ok(f32::from_le_bytes(wire(r.take(4))?.try_into()?)) };
                Vec2::new(f()?, f()?)
            } else {
                Vec2::ZERO
            };
            ensure!(level > 0 && level <= 8 && velocity.is_finite(), "invalid water replica");
            set.push(Cell { p, level, velocity });
        }
        let count = wire(r.u64())? as usize;
        ensure!(count <= MAX_CELLS && count <= r.remaining() && !(full && count > 0), "invalid water clear list");
        let mut clear = Vec::with_capacity(count);
        let mut previous = IVec3::ZERO;
        for _ in 0..count {
            clear.push(read_delta(&mut r, &mut previous)?);
        }
        ensure!(r.done(), "trailing water replica data");
        Ok(Self { full, digest, set, clear })
    }

    pub fn publish(self, levels: &mut WaterLevels, blocks: &mut HashSet<IVec3>, chunks: &mut HashSet<IVec3>) {
        let mut touch = |p: IVec3| {
            blocks.insert(p);
            world::mark_chunk_and_neighbors_dirty(chunks, p);
        };
        if self.full {
            for (&p, _) in levels.snapshot_all().iter() {
                touch(p);
            }
            *levels = WaterLevels::default();
        }
        for p in self.clear {
            let changed = levels.get(&p).copied().unwrap_or(8) != 8;
            levels.remove(&p);
            levels.remove_velocity(&p);
            if changed { touch(p); }
        }
        for c in self.set {
            let changed = levels.get(&c.p).copied().unwrap_or(8) != c.level;
            levels.insert(c.p, c.level);
            levels.set_velocity(c.p, c.velocity);
            // Momentum alone does not change the water mesh. Rebuilding every
            // moving cell here made a current remesh whole shorelines at 10 Hz.
            if changed { touch(c.p); }
        }
    }
}

/// Order-independent digest of every partial level, velocity excluded.
pub fn digest(levels: &WaterLevels) -> u64 {
    levels.snapshot_all().iter().fold(0u64, |sum, (&p, &level)| sum.wrapping_add(mix(p, level)))
}

fn mix(p: IVec3, level: u8) -> u64 {
    let mut z = (p.x as u32 as u64) | (p.y as u32 as u64) << 32;
    z ^= (p.z as u32 as u64 | u64::from(level) << 32).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

fn put_delta(out: &mut Vec<u8>, previous: &mut IVec3, p: IVec3) {
    for (a, b) in p.to_array().into_iter().zip(previous.to_array()) {
        put_i64(out, i64::from(a) - i64::from(b));
    }
    *previous = p;
}

fn read_delta(r: &mut Reader, previous: &mut IVec3) -> Result<IVec3> {
    let mut p = [0i32; 3];
    for (axis, (out, base)) in p.iter_mut().zip(previous.to_array()).enumerate() {
        let v = i64::from(base).checked_add(wire(r.i64())?).filter(|v| v.abs() <= MAX_COORD);
        let Some(v) = v else { anyhow::bail!("water replica coordinate {axis} out of range") };
        *out = v as i32;
    }
    *previous = IVec3::from_array(p);
    Ok(*previous)
}

fn wire<T>(r: Result<T, engine_netcode::wire::WireError>) -> Result<T> {
    r.map_err(|e| anyhow::anyhow!(e.0))
}
