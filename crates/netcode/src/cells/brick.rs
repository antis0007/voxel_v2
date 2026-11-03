//! Whole-brick palette encoding, used for resends and assembly baselines.
use super::BRICK_CELLS;
use crate::wire::{Reader, WireError, put_u64};

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
