//! Byte encoding of a `ToolAssembly`, for a future save version to carry.
//!
//! Layout: format byte, schema string, part count then (kind, material) strings,
//! modifier count then strings, xp u32, level u8, durability_left u32. Strings are
//! u16 length prefixed UTF-8; integers are little endian.

use crate::assembly::ToolAssembly;
use crate::materials::MaterialId;
use crate::parts::PartKind;
use std::fmt;

pub const FORMAT: u8 = 1;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DecodeError {
    Truncated,
    BadFormat(u8),
    BadUtf8,
    StringTooLong(usize),
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated => write!(f, "tool bytes end early"),
            Self::BadFormat(v) => write!(f, "tool format {v} unknown"),
            Self::BadUtf8 => write!(f, "tool string is not UTF-8"),
            Self::StringTooLong(n) => write!(f, "tool string of {n} bytes exceeds u16"),
        }
    }
}

impl std::error::Error for DecodeError {}

fn put_str(out: &mut Vec<u8>, s: &str) -> Result<(), DecodeError> {
    let len = u16::try_from(s.len()).map_err(|_| DecodeError::StringTooLong(s.len()))?;
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(s.as_bytes());
    Ok(())
}

struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], DecodeError> {
        let end = self.pos.checked_add(n).ok_or(DecodeError::Truncated)?;
        let slice = self.bytes.get(self.pos..end).ok_or(DecodeError::Truncated)?;
        self.pos = end;
        Ok(slice)
    }

    fn u8(&mut self) -> Result<u8, DecodeError> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, DecodeError> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    fn u32(&mut self) -> Result<u32, DecodeError> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn string(&mut self) -> Result<String, DecodeError> {
        let len = usize::from(self.u16()?);
        std::str::from_utf8(self.take(len)?).map(str::to_owned).map_err(|_| DecodeError::BadUtf8)
    }
}

impl ToolAssembly {
    /// Appends the tool to `out`. Fails only on a string over 65535 bytes.
    pub fn encode(&self, out: &mut Vec<u8>) -> Result<(), DecodeError> {
        out.push(FORMAT);
        put_str(out, &self.schema)?;
        out.push(u8::try_from(self.parts.len()).map_err(|_| DecodeError::StringTooLong(self.parts.len()))?);
        for (kind, material) in &self.parts {
            put_str(out, &kind.0)?;
            put_str(out, &material.0)?;
        }
        out.push(u8::try_from(self.modifiers.len()).map_err(|_| DecodeError::StringTooLong(self.modifiers.len()))?);
        for modifier in &self.modifiers {
            put_str(out, modifier)?;
        }
        out.extend_from_slice(&self.xp.to_le_bytes());
        out.push(self.level);
        out.extend_from_slice(&self.durability_left.to_le_bytes());
        Ok(())
    }

    /// Reads one tool from the front of `bytes`; returns it and the bytes consumed.
    /// Content is not validated against the tables, so a tool from a removed
    /// schema still loads and reports its error on use.
    pub fn decode(bytes: &[u8]) -> Result<(Self, usize), DecodeError> {
        let mut r = Reader { bytes, pos: 0 };
        let format = r.u8()?;
        if format != FORMAT {
            return Err(DecodeError::BadFormat(format));
        }
        let schema = r.string()?;
        let part_count = usize::from(r.u8()?);
        let mut parts = Vec::with_capacity(part_count);
        for _ in 0..part_count {
            let kind = PartKind(r.string()?);
            let material = MaterialId(r.string()?);
            parts.push((kind, material));
        }
        let modifier_count = usize::from(r.u8()?);
        let mut modifiers = Vec::with_capacity(modifier_count);
        for _ in 0..modifier_count {
            modifiers.push(r.string()?);
        }
        let xp = r.u32()?;
        let level = r.u8()?;
        let durability_left = r.u32()?;
        Ok((Self { schema, parts, modifiers, xp, level, durability_left }, r.pos))
    }
}
