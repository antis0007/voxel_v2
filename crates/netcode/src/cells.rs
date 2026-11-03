//! Tier A cellular replication: changed cells per client within a cell budget
//! (design section 7).
//!
//! Changes collect per 8-cell brick, newest value per cell, so a cell that flips
//! many times between flushes costs one entry. Each flush writes whole bricks
//! nearest the client first until the budget is spent. A brick that gathers too
//! many changes, or waits too long, turns stale: its cells are dropped and the
//! caller resends the whole brick once it has been quiet, or after a hard limit if
//! it never quiets. Sleeping bricks cost nothing.
//!
//! Bricks are keyed by frame (ADR 0040): frame 0 is the world grid, any other is an
//! assembly's local grid. A flush is a sequence of runs, one per frame; a run for an
//! assembly names the revision the client must hold and the one it reaches, so a
//! client missing a baseline or a flush can tell and ask for a resend.
//!
//! Flushes go on a reliable ordered channel; a lost flush would leave cells behind.
mod assembly;
mod brick;
mod read;

pub use assembly::AssemblyBaseline;
pub use brick::{read_brick, write_brick};
pub use read::{CellReader, Received, read_flush};

use crate::wire::{put_i64, put_u64};
use glam::IVec3;
use std::collections::HashMap;

pub const BRICK: i32 = 8;
pub const BRICK_CELLS: usize = (BRICK * BRICK * BRICK) as usize;
/// Pending cells past which a brick is cheaper resent whole.
pub const STALE_CELLS: usize = 192;
/// Flushes a brick may be passed over before it turns stale.
pub const STALE_SKIPS: u16 = 15;
/// Flushes without a change before a stale brick is resent.
pub const QUIET: u64 = 15;
/// Flushes after which a stale brick is resent even while active.
pub const MAX_STALE: u64 = 300;

pub fn brick_of(cell: IVec3) -> IVec3 {
    IVec3::new(cell.x.div_euclid(BRICK), cell.y.div_euclid(BRICK), cell.z.div_euclid(BRICK))
}

fn index_in_brick(cell: IVec3) -> u16 {
    let l = cell - brick_of(cell) * BRICK;
    (l.x + BRICK * (l.y + BRICK * l.z)) as u16
}

pub(crate) fn cell_at(brick: IVec3, index: u16) -> IVec3 {
    let i = i32::from(index);
    brick * BRICK + IVec3::new(i % BRICK, (i / BRICK) % BRICK, i / (BRICK * BRICK))
}

/// Newest value per cell of one brick, indexed directly so a repeated write is O(1)
/// and the flush walks cells in index order without sorting.
struct Cells {
    present: [u64; BRICK_CELLS / 64],
    values: [u32; BRICK_CELLS],
    len: usize,
}

impl Cells {
    fn set(&mut self, index: u16, value: u32) {
        let (word, bit) = (usize::from(index) / 64, 1u64 << (index % 64));
        if self.present[word] & bit == 0 {
            self.present[word] |= bit;
            self.len += 1;
        }
        self.values[usize::from(index)] = value;
    }

    fn iter(&self) -> impl Iterator<Item = (u16, u32)> + '_ {
        self.present.iter().enumerate().flat_map(move |(word, &bits)| {
            let mut rest = bits;
            std::iter::from_fn(move || {
                (rest != 0).then(|| {
                    let index = word * 64 + rest.trailing_zeros() as usize;
                    rest &= rest - 1;
                    (index as u16, self.values[index])
                })
            })
        })
    }
}

#[derive(Default)]
struct Pending {
    cells: Option<Box<Cells>>,
    skipped: u16,
    /// Flush the brick turned stale, while it is.
    stale_since: Option<u64>,
    last_change: u64,
}

/// One frame's part of a flush: the client's position in that frame's grid and,
/// for an assembly, the revision this flush brings the client to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct View {
    pub frame: u32,
    pub center: IVec3,
    pub revision: u32,
}

impl View {
    pub fn world(center: IVec3) -> Self {
        Self { frame: 0, center, revision: 0 }
    }
}

/// Server side, one per client.
#[derive(Default)]
pub struct CellStream {
    bricks: HashMap<(u32, IVec3), Pending>,
    /// Revision the client holds per assembly; runs are written only for these.
    sent: HashMap<u32, u32>,
    flushes: u64,
    /// Cleared cell sets kept for reuse, so steady traffic allocates nothing.
    #[allow(clippy::vec_box, reason = "boxes move between bricks and this pool without copying 2 KiB")]
    spare: Vec<Box<Cells>>,
    runs: Vec<(u64, Vec<u8>)>,
}

fn len(cells: &Option<Box<Cells>>) -> usize {
    cells.as_ref().map_or(0, |c| c.len)
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Flush {
    pub cells: usize,
    /// Bricks (frame, brick) the caller sends whole, on the same ordered channel after this flush.
    pub resend: Vec<(u32, IVec3)>,
}

impl CellStream {
    fn recycle(&mut self, cells: Option<Box<Cells>>) {
        if let Some(mut c) = cells {
            c.present = [0; BRICK_CELLS / 64];
            c.len = 0;
            self.spare.push(c);
        }
    }

    /// Records a world cell change inside this client's interest set.
    pub fn changed(&mut self, cell: IVec3, value: u32) {
        self.changed_in(0, cell, value);
    }

    /// Records a cell change in `frame`'s grid.
    pub fn changed_in(&mut self, frame: u32, cell: IVec3, value: u32) {
        let p = self.bricks.entry((frame, brick_of(cell))).or_default();
        p.last_change = self.flushes;
        if p.stale_since.is_some() {
            return;
        }
        let spare = &mut self.spare;
        let cells = p.cells.get_or_insert_with(|| {
            spare.pop().unwrap_or_else(|| Box::new(Cells { present: [0; BRICK_CELLS / 64], values: [0; BRICK_CELLS], len: 0 }))
        });
        cells.set(index_in_brick(cell), value);
        if cells.len > STALE_CELLS {
            let dropped = p.cells.take();
            p.stale_since = Some(self.flushes);
            self.recycle(dropped);
        }
    }

    /// Records that the client was sent `frame`'s baseline at `revision`; pending
    /// changes for it are covered by the baseline and dropped.
    pub fn baseline_sent(&mut self, frame: u32, revision: u32) {
        self.sent.insert(frame, revision);
        let covered: Vec<_> = self.bricks.keys().filter(|k| k.0 == frame).copied().collect();
        for key in covered {
            if let Some(p) = self.bricks.remove(&key) {
                self.recycle(p.cells);
            }
        }
    }

    /// Forgets an assembly that left the client's interest.
    pub fn forget(&mut self, frame: u32) {
        self.baseline_sent(frame, 0);
        self.sent.remove(&frame);
    }

    pub fn pending_bricks(&self) -> usize {
        self.bricks.len()
    }

    /// Writes up to `budget` world cells nearest `center` into `out`.
    pub fn flush(&mut self, center: IVec3, budget: usize, out: &mut Vec<u8>) -> Flush {
        self.flush_frames(&[View::world(center)], budget, out)
    }

    /// Writes up to `budget` cells over `views`, nearest each view's centre first.
    /// Frames without a view, and assemblies without a sent baseline, stay pending.
    pub fn flush_frames(&mut self, views: &[View], budget: usize, out: &mut Vec<u8>) -> Flush {
        self.flushes += 1;
        let now = self.flushes;
        let view_of = |frame: u32| views.iter().position(|v| v.frame == frame);
        let mut resend: Vec<(u32, IVec3)> = Vec::new();
        let mut order: Vec<(i64, usize, IVec3)> = Vec::new();
        for (&(frame, brick), p) in &mut self.bricks {
            let Some(at) = view_of(frame).filter(|_| frame == 0 || self.sent.contains_key(&frame)) else { continue };
            match p.stale_since {
                Some(since) if now - p.last_change >= QUIET || now - since >= MAX_STALE => resend.push((frame, brick)),
                Some(_) => {}
                None if len(&p.cells) > 0 => {
                    let d = (brick - brick_of(views[at].center)).as_i64vec3();
                    order.push((d.dot(d), at, brick));
                }
                None => {}
            }
        }
        for key in &resend {
            if let Some(p) = self.bricks.remove(key) {
                self.recycle(p.cells);
            }
        }
        resend.sort_by_key(|&(f, b)| (f, b.x, b.y, b.z));
        order.sort_by_key(|&(d, at, b)| (d, at, b.x, b.y, b.z));

        let mut runs = std::mem::take(&mut self.runs);
        runs.iter_mut().for_each(|(n, body)| {
            *n = 0;
            body.clear();
        });
        runs.resize_with(views.len(), Default::default);
        let mut cells = 0usize;
        for (_, at, brick) in order {
            let key = (views[at].frame, brick);
            let Some(p) = self.bricks.get_mut(&key) else { continue };
            let n = len(&p.cells);
            if cells + n > budget {
                p.skipped += 1;
                if p.skipped >= STALE_SKIPS {
                    let dropped = p.cells.take();
                    p.stale_since = Some(now);
                    self.recycle(dropped);
                }
                continue;
            }
            let (written, body) = &mut runs[at];
            let rel = brick - brick_of(views[at].center);
            [rel.x, rel.y, rel.z].iter().for_each(|&c| put_i64(body, i64::from(c)));
            put_u64(body, n as u64);
            let mut prev = 0u16;
            for (index, value) in p.cells.iter().flat_map(|c| c.iter()) {
                put_u64(body, u64::from(index - prev));
                put_u64(body, u64::from(value));
                prev = index;
            }
            cells += n;
            *written += 1;
            if let Some(p) = self.bricks.remove(&key) {
                self.recycle(p.cells);
            }
        }
        for (view, (written, body)) in views.iter().zip(&runs) {
            if view.frame != 0 {
                let Some(&base) = self.sent.get(&view.frame) else { continue };
                if *written == 0 && base == view.revision {
                    continue;
                }
                put_u64(out, u64::from(view.frame));
                put_u64(out, u64::from(base));
                put_u64(out, u64::from(view.revision.wrapping_sub(base)));
                self.sent.insert(view.frame, view.revision);
            } else {
                put_u64(out, 0);
            }
            let home = brick_of(view.center);
            [home.x, home.y, home.z].iter().for_each(|&c| put_i64(out, i64::from(c)));
            put_u64(out, *written);
            out.extend_from_slice(body);
        }
        self.runs = runs;
        Flush { cells, resend }
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod frame_tests;
