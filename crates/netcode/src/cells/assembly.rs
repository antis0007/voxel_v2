//! Reliable `AssemblyBaseline` message: every brick of an assembly at one revision.
use super::read::fits;
use super::{BRICK_CELLS, read_brick, write_brick};
use crate::wire::{Reader, WireError, put_i64, put_u64};
use glam::IVec3;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssemblyBaseline {
    /// Never 0: frame 0 is the world.
    pub id: u32,
    pub revision: u32,
    /// Inclusive local brick range, min then max.
    pub bounds: [IVec3; 2],
    /// Bricks inside `bounds`; absent bricks are air (all zero).
    pub bricks: Vec<(IVec3, Box<[u32; BRICK_CELLS]>)>,
}

fn key(b: IVec3) -> (i32, i32, i32) {
    (b.z, b.y, b.x)
}

impl AssemblyBaseline {
    /// Writes bricks in (z, y, x) order relative to the minimum bound; bricks
    /// outside `bounds` are skipped.
    pub fn encode(&self, out: &mut Vec<u8>) {
        let [min, max] = self.bounds;
        put_u64(out, u64::from(self.id));
        put_u64(out, u64::from(self.revision));
        [min.x, min.y, min.z].iter().for_each(|&c| put_i64(out, i64::from(c)));
        let extent = (max - min).max(IVec3::ZERO);
        [extent.x, extent.y, extent.z].iter().for_each(|&c| put_u64(out, c as u64));
        let mut order: Vec<&(IVec3, Box<[u32; BRICK_CELLS]>)> =
            self.bricks.iter().filter(|(b, _)| b.cmpge(min).all() && b.cmple(max).all()).collect();
        order.sort_by_key(|(b, _)| key(*b));
        order.dedup_by_key(|(b, _)| key(*b));
        put_u64(out, order.len() as u64);
        let mut bytes = Vec::new();
        for (brick, cells) in order {
            let rel = *brick - min;
            [rel.x, rel.y, rel.z].iter().for_each(|&c| put_u64(out, c as u64));
            bytes.clear();
            write_brick(cells, &mut bytes);
            put_u64(out, bytes.len() as u64);
            out.extend_from_slice(&bytes);
        }
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, WireError> {
        let mut r = Reader::new(bytes);
        let id = r.u32()?;
        if id == 0 {
            return Err(WireError("assembly id 0"));
        }
        let revision = r.u32()?;
        let min = IVec3::new(fits(Some(r.i32()?))?, fits(Some(r.i32()?))?, fits(Some(r.i32()?))?);
        let mut along = |m: i32| -> Result<i32, WireError> { fits(i32::try_from(r.u64()?).ok().and_then(|e| m.checked_add(e))) };
        let max = IVec3::new(along(min.x)?, along(min.y)?, along(min.z)?);
        let count = r.u64()?;
        // Each brick takes at least 7 bytes, so a hostile count cannot reserve memory.
        if count > r.remaining() as u64 / 7 {
            return Err(WireError("brick count"));
        }
        let mut bricks = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let rel = [r.u64()?, r.u64()?, r.u64()?];
            let span = (max - min).to_array();
            if rel.iter().zip(span).any(|(&c, s)| c > s as u64) {
                return Err(WireError("brick outside bounds"));
            }
            let brick = min + IVec3::new(rel[0] as i32, rel[1] as i32, rel[2] as i32);
            if bricks.last().is_some_and(|(b, _): &(IVec3, _)| key(*b) >= key(brick)) {
                return Err(WireError("brick order"));
            }
            let n = usize::try_from(r.u64()?).map_err(|_| WireError("brick length"))?;
            bricks.push((brick, Box::new(read_brick(r.take(n)?)?)));
        }
        if !r.done() {
            return Err(WireError("trailing bytes"));
        }
        Ok(Self { id, revision, bounds: [min, max], bricks })
    }
}
