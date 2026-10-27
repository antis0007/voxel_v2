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
//! Flushes go on a reliable ordered channel; a lost flush would leave cells behind.
use crate::wire::{Reader, WireError, put_i64, put_u64};
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

fn cell_at(brick: IVec3, index: u16) -> IVec3 {
    let i = i32::from(index);
    brick * BRICK + IVec3::new(i % BRICK, (i / BRICK) % BRICK, i / (BRICK * BRICK))
}

#[derive(Default)]
struct Pending {
    cells: Vec<(u16, u32)>,
    skipped: u16,
    /// Flush the brick turned stale, while it is.
    stale_since: Option<u64>,
    last_change: u64,
}

/// Server side, one per client.
#[derive(Default)]
pub struct CellStream {
    bricks: HashMap<IVec3, Pending>,
    flushes: u64,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Flush {
    pub cells: usize,
    /// Bricks the caller sends whole, on the same ordered channel after this flush.
    pub resend: Vec<IVec3>,
}

impl CellStream {
    /// Records a cell change inside this client's interest set.
    pub fn changed(&mut self, cell: IVec3, value: u32) {
        let p = self.bricks.entry(brick_of(cell)).or_default();
        p.last_change = self.flushes;
        if p.stale_since.is_some() {
            return;
        }
        let index = index_in_brick(cell);
        match p.cells.iter_mut().find(|(i, _)| *i == index) {
            Some(entry) => entry.1 = value,
            None => p.cells.push((index, value)),
        }
        if p.cells.len() > STALE_CELLS {
            p.cells = Vec::new();
            p.stale_since = Some(self.flushes);
        }
    }

    pub fn pending_bricks(&self) -> usize {
        self.bricks.len()
    }

    /// Writes up to `budget` cells nearest `center` into `out`.
    pub fn flush(&mut self, center: IVec3, budget: usize, out: &mut Vec<u8>) -> Flush {
        self.flushes += 1;
        let now = self.flushes;
        let home = brick_of(center);
        let mut resend: Vec<IVec3> = Vec::new();
        let mut order: Vec<(i64, IVec3)> = Vec::new();
        for (&brick, p) in &mut self.bricks {
            match p.stale_since {
                Some(since) if now - p.last_change >= QUIET || now - since >= MAX_STALE => resend.push(brick),
                Some(_) => {}
                None if !p.cells.is_empty() => {
                    let d = (brick - home).as_i64vec3();
                    order.push((d.dot(d), brick));
                }
                None => {}
            }
        }
        for brick in &resend {
            self.bricks.remove(brick);
        }
        resend.sort_by_key(|b| (b.x, b.y, b.z));
        order.sort_by_key(|&(d, b)| (d, b.x, b.y, b.z));

        let mut body = Vec::new();
        let (mut written, mut cells) = (0u64, 0usize);
        for (_, brick) in order {
            let Some(p) = self.bricks.get_mut(&brick) else { continue };
            if cells + p.cells.len() > budget {
                p.skipped += 1;
                if p.skipped >= STALE_SKIPS {
                    p.cells = Vec::new();
                    p.stale_since = Some(now);
                }
                continue;
            }
            let rel = brick - home;
            [rel.x, rel.y, rel.z].iter().for_each(|&c| put_i64(&mut body, i64::from(c)));
            put_u64(&mut body, p.cells.len() as u64);
            p.cells.sort_unstable_by_key(|(i, _)| *i);
            let mut prev = 0u16;
            for &(index, value) in &p.cells {
                put_u64(&mut body, u64::from(index - prev));
                put_u64(&mut body, u64::from(value));
                prev = index;
            }
            cells += p.cells.len();
            written += 1;
            self.bricks.remove(&brick);
        }
        [home.x, home.y, home.z].iter().for_each(|&c| put_i64(out, i64::from(c)));
        put_u64(out, written);
        out.extend_from_slice(&body);
        Flush { cells, resend }
    }
}

/// Client side: decodes one flush into absolute cells.
pub fn read_flush(bytes: &[u8]) -> Result<Vec<(IVec3, u32)>, WireError> {
    let mut r = Reader::new(bytes);
    let home = IVec3::new(r.i32()?, r.i32()?, r.i32()?);
    let bricks = r.u64()?;
    let mut out = Vec::new();
    for _ in 0..bricks {
        let brick = home + IVec3::new(r.i32()?, r.i32()?, r.i32()?);
        let count = r.u64()?;
        if count > BRICK_CELLS as u64 {
            return Err(WireError("brick cell count"));
        }
        let mut index = 0u64;
        for n in 0..count {
            let gap = r.u64()?;
            if n > 0 && gap == 0 {
                return Err(WireError("repeated cell"));
            }
            index += gap;
            let index = u16::try_from(index).ok().filter(|&i| usize::from(i) < BRICK_CELLS).ok_or(WireError("cell index"))?;
            out.push((cell_at(brick, index), r.u32()?));
        }
    }
    if !r.done() {
        return Err(WireError("trailing bytes"));
    }
    Ok(out)
}

/// Whole-brick form for resends: a palette, then (palette index, run length) pairs.
pub fn write_brick(cells: &[u32; BRICK_CELLS], out: &mut Vec<u8>) {
    let mut palette: Vec<u32> = Vec::new();
    let mut runs: Vec<(usize, usize)> = Vec::new();
    for &value in cells {
        let slot = palette.iter().position(|&p| p == value).unwrap_or_else(|| {
            palette.push(value);
            palette.len() - 1
        });
        match runs.last_mut() {
            Some((s, n)) if *s == slot => *n += 1,
            _ => runs.push((slot, 1)),
        }
    }
    put_u64(out, palette.len() as u64);
    palette.iter().for_each(|&p| put_u64(out, u64::from(p)));
    put_u64(out, runs.len() as u64);
    for (slot, n) in runs {
        put_u64(out, slot as u64);
        put_u64(out, n as u64);
    }
}

pub fn read_brick(bytes: &[u8]) -> Result<[u32; BRICK_CELLS], WireError> {
    let mut r = Reader::new(bytes);
    let size = r.u64()?;
    if size == 0 || size > BRICK_CELLS as u64 {
        return Err(WireError("palette size"));
    }
    let palette = (0..size).map(|_| r.u32()).collect::<Result<Vec<_>, _>>()?;
    let mut cells = [0u32; BRICK_CELLS];
    let mut at = 0usize;
    for _ in 0..r.u64()? {
        let value = *palette.get(r.u64()? as usize).ok_or(WireError("palette index"))?;
        let n = r.u64()? as usize;
        let end = at.checked_add(n).filter(|&e| e <= BRICK_CELLS && n > 0).ok_or(WireError("run length"))?;
        cells[at..end].fill(value);
        at = end;
    }
    if at != BRICK_CELLS || !r.done() {
        return Err(WireError("brick length"));
    }
    Ok(cells)
}

#[cfg(test)]
mod tests;
