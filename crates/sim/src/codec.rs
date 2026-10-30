//! Little-endian byte reading and writing shared by the store checkpoint, the action log and the
//! run checkpoint. Every count is checked against the bytes left before anything is allocated.
use crate::cell::Cell;
use glam::IVec3;

/// Why saved bytes were refused. A refused input never yields a partial value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckpointError {
    BadMagic,
    Truncated,
    TrailingBytes,
    /// The bytes decode but describe a value the rules forbid.
    Inconsistent(&'static str),
}

pub(crate) struct Reader<'a> {
    bytes: &'a [u8],
}

impl<'a> Reader<'a> {
    /// A reader past `magic`, or `BadMagic` / `Truncated`.
    pub(crate) fn open(bytes: &'a [u8], magic: &[u8; 8]) -> Result<Self, CheckpointError> {
        let mut r = Self { bytes };
        if r.take(magic.len())? != magic {
            return Err(CheckpointError::BadMagic);
        }
        Ok(r)
    }

    pub(crate) fn take(&mut self, n: usize) -> Result<&'a [u8], CheckpointError> {
        if self.bytes.len() < n {
            return Err(CheckpointError::Truncated);
        }
        let (head, rest) = self.bytes.split_at(n);
        self.bytes = rest;
        Ok(head)
    }

    pub(crate) fn u8(&mut self) -> Result<u8, CheckpointError> {
        Ok(self.take(1)?[0])
    }

    pub(crate) fn u32(&mut self) -> Result<u32, CheckpointError> {
        let mut b = [0; 4];
        b.copy_from_slice(self.take(4)?);
        Ok(u32::from_le_bytes(b))
    }

    pub(crate) fn i32(&mut self) -> Result<i32, CheckpointError> {
        Ok(self.u32()? as i32)
    }

    pub(crate) fn u64(&mut self) -> Result<u64, CheckpointError> {
        let mut b = [0; 8];
        b.copy_from_slice(self.take(8)?);
        Ok(u64::from_le_bytes(b))
    }

    pub(crate) fn coord(&mut self) -> Result<IVec3, CheckpointError> {
        Ok(IVec3::new(self.i32()?, self.i32()?, self.i32()?))
    }

    pub(crate) fn cell(&mut self) -> Result<Cell, CheckpointError> {
        // Unaligned: a cell can start at any byte of a log.
        Ok(bytemuck::pod_read_unaligned(self.take(size_of::<Cell>())?))
    }

    /// A count that must fit in what is left, `unit` bytes per item.
    pub(crate) fn count(&mut self, unit: usize) -> Result<usize, CheckpointError> {
        let n = self.u32()? as usize;
        if n.checked_mul(unit).is_none_or(|bytes| bytes > self.bytes.len()) {
            return Err(CheckpointError::Truncated);
        }
        Ok(n)
    }

    /// The rest of the input, which must be empty.
    pub(crate) fn finish(self) -> Result<(), CheckpointError> {
        if self.bytes.is_empty() { Ok(()) } else { Err(CheckpointError::TrailingBytes) }
    }
}

pub(crate) fn put_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

pub(crate) fn put_coord(out: &mut Vec<u8>, c: IVec3) {
    for v in [c.x, c.y, c.z] {
        out.extend_from_slice(&v.to_le_bytes());
    }
}

pub(crate) fn put_cell(out: &mut Vec<u8>, c: Cell) {
    out.extend_from_slice(bytemuck::bytes_of(&c));
}
