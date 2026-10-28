//! Blueprint file encoding: a material-name palette and run-length cells.
//!
//! Layout: magic, version `u16`, size `3 × u16`, palette count `u16`, then
//! per entry a `u8` length and the `BlockId` name, run count `u32`, runs of
//! (`u8` palette index, LEB128 length), and an FNV-1a 64 checksum of all
//! preceding bytes. Integers are little-endian. Names, not ids, keep files
//! valid when the material table is reordered.
use super::{Blueprint, MAX_VOLUME, valid_size};
use crate::world::BlockId;
use anyhow::{Result, bail, ensure};
use engine_content::ALL_BLOCK_IDS;
use glam::IVec3;

pub const MAGIC: &[u8; 8] = b"VXBLUEPR";
pub const VERSION: u16 = 1;
/// Header bytes before the palette: magic, version and size.
pub const HEADER: usize = 8 + 2 + 6;
/// Worst case is one two-byte run per cell plus the palette.
pub const MAX_FILE_BYTES: usize = 2 * MAX_VOLUME + 64 * 1024;
const MAX_PALETTE: usize = 256;
const MAX_NAME_BYTES: usize = 64;

pub fn fnv1a64(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3))
}

fn block_name(b: BlockId) -> String {
    format!("{b:?}")
}

fn block_by_name(name: &str) -> Option<BlockId> {
    ALL_BLOCK_IDS.iter().copied().find(|b| block_name(*b) == name)
}

pub fn encode(bp: &Blueprint) -> Vec<u8> {
    let mut palette: Vec<BlockId> = Vec::new();
    let mut runs: Vec<(u8, u32)> = Vec::new();
    for &b in bp.cells() {
        let slot = match palette.iter().position(|p| *p == b) {
            Some(i) => i,
            None => {
                palette.push(b);
                palette.len() - 1
            }
        };
        match runs.last_mut() {
            Some((s, n)) if usize::from(*s) == slot => *n += 1,
            // BlockId is a u8 enum, so a palette never passes 256 entries.
            _ => runs.push((slot as u8, 1)),
        }
    }
    let mut out = Vec::with_capacity(64 + runs.len() * 2);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    for axis in bp.size().to_array() {
        out.extend_from_slice(&(axis as u16).to_le_bytes());
    }
    out.extend_from_slice(&(palette.len() as u16).to_le_bytes());
    for b in &palette {
        let name = block_name(*b);
        out.push(name.len() as u8);
        out.extend_from_slice(name.as_bytes());
    }
    out.extend_from_slice(&(runs.len() as u32).to_le_bytes());
    for (slot, mut n) in runs {
        out.push(slot);
        loop {
            let byte = (n & 0x7f) as u8;
            n >>= 7;
            if n == 0 {
                out.push(byte);
                break;
            }
            out.push(byte | 0x80);
        }
    }
    let sum = fnv1a64(&out);
    out.extend_from_slice(&sum.to_le_bytes());
    out
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8]> {
        ensure!(self.bytes.len() - self.at >= n, "Blueprint File Truncated");
        let s = &self.bytes[self.at..self.at + n];
        self.at += n;
        Ok(s)
    }

    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    fn u32(&mut self) -> Result<u32> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn varint(&mut self) -> Result<u32> {
        let mut value = 0u32;
        for shift in (0..35).step_by(7) {
            let byte = self.u8()?;
            ensure!(shift < 28 || byte < 0x10, "Blueprint Run Too Long");
            value |= u32::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return Ok(value);
            }
        }
        bail!("Blueprint Run Too Long")
    }
}

/// Dimensions from the header alone, for listing files without decoding them.
pub fn peek_size(bytes: &[u8]) -> Option<IVec3> {
    let mut r = Reader { bytes, at: 0 };
    (r.take(8).ok()? == MAGIC && r.u16().ok()? == VERSION).then_some(())?;
    let size = IVec3::new(i32::from(r.u16().ok()?), i32::from(r.u16().ok()?), i32::from(r.u16().ok()?));
    valid_size(size).then_some(size)
}

pub struct Decoded {
    pub blueprint: Blueprint,
    /// Cells whose material this build does not know; they load as air.
    pub unknown: usize,
}

/// Every length is checked against the remaining bytes and the declared volume
/// before anything is allocated beyond the header's bounded size.
pub fn decode(bytes: &[u8]) -> Result<Decoded> {
    ensure!(bytes.len() <= MAX_FILE_BYTES, "Blueprint File Too Large");
    ensure!(bytes.len() >= HEADER + 8, "Blueprint File Truncated");
    let (body, tail) = bytes.split_at(bytes.len() - 8);
    let mut sum = [0u8; 8];
    sum.copy_from_slice(tail);
    let mut r = Reader { bytes: body, at: 0 };
    ensure!(r.take(8)? == MAGIC, "Not a Blueprint File");
    let version = r.u16()?;
    ensure!(version == VERSION, "Blueprint Version {version} Unsupported");
    ensure!(u64::from_le_bytes(sum) == fnv1a64(body), "Blueprint Checksum Mismatch");
    let size = IVec3::new(i32::from(r.u16()?), i32::from(r.u16()?), i32::from(r.u16()?));
    ensure!(valid_size(size), "Blueprint Size Invalid");
    let volume = size.as_i64vec3().element_product() as usize;
    let count = usize::from(r.u16()?);
    ensure!((1..=MAX_PALETTE).contains(&count), "Blueprint Palette Invalid");
    let mut palette = Vec::with_capacity(count);
    for _ in 0..count {
        let len = usize::from(r.u8()?);
        ensure!((1..=MAX_NAME_BYTES).contains(&len), "Blueprint Palette Invalid");
        let name = std::str::from_utf8(r.take(len)?).map_err(|_| anyhow::anyhow!("Blueprint Palette Invalid"))?;
        palette.push(block_by_name(name));
    }
    let runs = r.u32()? as usize;
    ensure!(runs <= volume && runs <= r.bytes.len() - r.at, "Blueprint Runs Invalid");
    let mut cells = Vec::with_capacity(volume);
    let mut unknown = 0;
    for _ in 0..runs {
        let slot = usize::from(r.u8()?);
        let n = r.varint()? as usize;
        ensure!(slot < palette.len() && n >= 1 && n <= volume - cells.len(), "Blueprint Runs Invalid");
        let block = palette[slot].unwrap_or_else(|| {
            unknown += n;
            BlockId::Air
        });
        cells.resize(cells.len() + n, block);
    }
    ensure!(cells.len() == volume, "Blueprint Runs Invalid");
    ensure!(r.at == r.bytes.len(), "Blueprint Trailing Bytes");
    let blueprint = Blueprint::new(size, cells).ok_or_else(|| anyhow::anyhow!("Blueprint Size Invalid"))?;
    Ok(Decoded { blueprint, unknown })
}
