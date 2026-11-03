//! Client side of cell flushes: decodes runs and holds the revision per assembly.
use super::{AssemblyBaseline, BRICK, BRICK_CELLS, cell_at};
use crate::wire::{Reader, WireError};
use glam::IVec3;
use std::collections::HashMap;

/// Brick coordinates stay where `brick * BRICK + 7` fits in i32.
pub(crate) const LIMIT: i32 = i32::MAX / BRICK - 1;

pub(crate) fn fits(c: Option<i32>) -> Result<i32, WireError> {
    c.filter(|c| (-LIMIT..=LIMIT).contains(c)).ok_or(WireError("brick coordinate"))
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Received {
    /// (frame, cell in that frame's grid, value).
    pub cells: Vec<(u32, IVec3, u32)>,
    /// Assemblies whose run was refused; each needs a fresh baseline.
    pub resend: Vec<u32>,
}

/// Revision held per assembly; the world frame needs none.
#[derive(Default)]
pub struct CellReader {
    revisions: HashMap<u32, u32>,
}

impl CellReader {
    /// Installs a baseline; flushes for that assembly are accepted from its revision.
    pub fn install(&mut self, baseline: &AssemblyBaseline) {
        self.revisions.insert(baseline.id, baseline.revision);
    }

    pub fn forget(&mut self, frame: u32) {
        self.revisions.remove(&frame);
    }

    pub fn revision(&self, frame: u32) -> Option<u32> {
        self.revisions.get(&frame).copied()
    }

    /// Decodes one flush. A run for an assembly without a baseline, or whose base
    /// revision is not the one held, is dropped and the assembly listed for resend;
    /// its baseline is forgotten so later runs are refused until the resend lands.
    /// A malformed flush changes nothing.
    pub fn read(&mut self, bytes: &[u8]) -> Result<Received, WireError> {
        let mut r = Reader::new(bytes);
        let mut got = Received::default();
        let mut reached: Vec<(u32, Option<u32>)> = Vec::new();
        while !r.done() {
            let frame = r.u32()?;
            if reached.iter().any(|&(f, _)| f == frame) {
                return Err(WireError("repeated frame"));
            }
            let accept = if frame == 0 {
                true
            } else {
                let base = r.u32()?;
                let to = base.checked_add(r.u32()?).ok_or(WireError("revision"))?;
                let ok = self.revisions.get(&frame) == Some(&base);
                reached.push((frame, ok.then_some(to)));
                if !ok {
                    got.resend.push(frame);
                }
                ok
            };
            if frame == 0 {
                reached.push((0, None));
            }
            let start = got.cells.len();
            read_run(&mut r, frame, &mut got.cells)?;
            if !accept {
                got.cells.truncate(start);
            }
        }
        for (frame, to) in reached.into_iter().filter(|&(f, _)| f != 0) {
            match to {
                Some(to) => self.revisions.insert(frame, to),
                None => self.revisions.remove(&frame),
            };
        }
        Ok(got)
    }
}

fn read_run(r: &mut Reader, frame: u32, out: &mut Vec<(u32, IVec3, u32)>) -> Result<(), WireError> {
    let home = IVec3::new(fits(Some(r.i32()?))?, fits(Some(r.i32()?))?, fits(Some(r.i32()?))?);
    let bricks = r.u64()?;
    for _ in 0..bricks {
        let (x, y, z) = (r.i32()?, r.i32()?, r.i32()?);
        let brick = IVec3::new(fits(home.x.checked_add(x))?, fits(home.y.checked_add(y))?, fits(home.z.checked_add(z))?);
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
            index = index.checked_add(gap).ok_or(WireError("cell index"))?;
            let index = u16::try_from(index).ok().filter(|&i| usize::from(i) < BRICK_CELLS).ok_or(WireError("cell index"))?;
            out.push((frame, cell_at(brick, index), r.u32()?));
        }
    }
    Ok(())
}

/// Decodes a world-only flush into absolute cells; assembly runs are an error.
pub fn read_flush(bytes: &[u8]) -> Result<Vec<(IVec3, u32)>, WireError> {
    let got = CellReader::default().read(bytes)?;
    if !got.resend.is_empty() {
        return Err(WireError("assembly run without baseline"));
    }
    Ok(got.cells.into_iter().map(|(_, cell, value)| (cell, value)).collect())
}
