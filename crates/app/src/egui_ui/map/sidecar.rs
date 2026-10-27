//! Versioned, checksummed map files beside a world, decoded with hard bounds.
use super::columns::COLUMNS;
use super::tiles::MAX_TILES;
use super::waypoints::{Kind, MAX_NAME, MAX_WAYPOINTS, PALETTE, Waypoint, Waypoints};
use anyhow::{Context, Result, bail, ensure};
use egui::Color32;
use glam::{IVec2, IVec3};
use std::path::Path;

const WAYPOINT_MAGIC: &[u8; 8] = b"VXWPT001";
const TILE_MAGIC: &[u8; 8] = b"VXMAP001";
const PREFS_MAGIC: &[u8; 8] = b"VXMMP001";
pub const WAYPOINT_FILE: &str = "waypoints.vxm";
pub const TILE_FILE: &str = "map.vxm";
pub const PREFS_FILE: &str = "minimap.vxp";
/// Waypoints far outside any generated terrain are corrupt, not distant.
const COORD_LIMIT: i32 = 1 << 28;
const TILE_BYTES: usize = 8 + COLUMNS / 8 + COLUMNS * 3;

fn hash(b: &[u8]) -> u64 {
    b.iter().fold(0xcbf29ce484222325, |h, &v| (h ^ v as u64).wrapping_mul(0x100000001b3))
}

fn seal(mut body: Vec<u8>) -> Vec<u8> {
    let sum = hash(&body);
    body.extend_from_slice(&sum.to_le_bytes());
    body
}

/// The payload after `magic`, once length and checksum hold.
fn open<'a>(bytes: &'a [u8], magic: &[u8; 8]) -> Result<&'a [u8]> {
    ensure!(bytes.len() >= 16 && bytes.get(..8) == Some(magic.as_slice()), "Unsupported map file");
    let end = bytes.len() - 8;
    let sum = u64::from_le_bytes(bytes[end..].try_into()?);
    ensure!(hash(&bytes[..end]) == sum, "Map file checksum mismatch");
    Ok(&bytes[8..end])
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.at.checked_add(n).context("Map file offset overflow")?;
        let out = self.bytes.get(self.at..end).context("Truncated map file")?;
        self.at = end;
        Ok(out)
    }
    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into()?))
    }
    fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_le_bytes(self.take(4)?.try_into()?))
    }
    fn done(&self) -> Result<()> {
        ensure!(self.at == self.bytes.len(), "Trailing bytes in map file");
        Ok(())
    }
}

pub fn encode_waypoints(w: &Waypoints) -> Vec<u8> {
    let mut out = WAYPOINT_MAGIC.to_vec();
    let list = &w.list[..w.list.len().min(MAX_WAYPOINTS)];
    out.extend_from_slice(&(list.len() as u32).to_le_bytes());
    for p in list {
        for v in p.pos.to_array() {
            out.extend_from_slice(&v.to_le_bytes());
        }
        let name = super::waypoints::clip_name(&p.name);
        out.extend_from_slice(&[p.colour, matches!(p.kind, Kind::Death) as u8, name.len() as u8]);
        out.extend_from_slice(name.as_bytes());
    }
    seal(out)
}

pub fn decode_waypoints(bytes: &[u8]) -> Result<Waypoints> {
    let mut r = Reader { bytes: open(bytes, WAYPOINT_MAGIC)?, at: 0 };
    let count = r.u32()? as usize;
    ensure!(count <= MAX_WAYPOINTS, "Too many waypoints");
    let mut list = Vec::with_capacity(count);
    for _ in 0..count {
        let pos = IVec3::new(r.i32()?, r.i32()?, r.i32()?);
        ensure!(pos.abs().max_element() < COORD_LIMIT, "Waypoint out of range");
        let colour = r.u8()?;
        ensure!((colour as usize) < PALETTE.len(), "Invalid waypoint colour");
        let kind = match r.u8()? {
            0 => Kind::Marker,
            1 => Kind::Death,
            _ => bail!("Invalid waypoint kind"),
        };
        let len = r.u8()? as usize;
        ensure!(len <= MAX_NAME, "Waypoint name too long");
        let name = std::str::from_utf8(r.take(len)?)?.to_owned();
        list.push(Waypoint { name, pos, colour, kind });
    }
    r.done()?;
    Ok(Waypoints { list })
}

/// Colours plus a known-column bitmask; unknown columns stay transparent.
pub fn encode_tiles<'a>(tiles: impl Iterator<Item = (IVec2, &'a [Color32])>) -> Vec<u8> {
    let mut body = Vec::new();
    let mut count = 0u32;
    for (key, colour) in tiles.take(MAX_TILES) {
        if colour.len() != COLUMNS {
            continue;
        }
        count += 1;
        body.extend_from_slice(&key.x.to_le_bytes());
        body.extend_from_slice(&key.y.to_le_bytes());
        let mut mask = [0u8; COLUMNS / 8];
        for (i, c) in colour.iter().enumerate() {
            if c.a() > 0 {
                mask[i / 8] |= 1 << (i % 8);
            }
        }
        body.extend_from_slice(&mask);
        for c in colour {
            body.extend_from_slice(&[c.r(), c.g(), c.b()]);
        }
    }
    let mut out = TILE_MAGIC.to_vec();
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&body);
    seal(out)
}

pub fn decode_tiles(bytes: &[u8]) -> Result<Vec<(IVec2, Vec<Color32>)>> {
    let mut r = Reader { bytes: open(bytes, TILE_MAGIC)?, at: 0 };
    let count = r.u32()? as usize;
    ensure!(count <= MAX_TILES, "Too many map tiles");
    ensure!(r.bytes.len() == 4 + count * TILE_BYTES, "Map tile file length mismatch");
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        let key = IVec2::new(r.i32()?, r.i32()?);
        ensure!(key.abs().max_element() < COORD_LIMIT / 32, "Map tile out of range");
        let mask = r.take(COLUMNS / 8)?;
        let rgb = r.take(COLUMNS * 3)?;
        let colour = (0..COLUMNS)
            .map(|i| match mask[i / 8] & (1 << (i % 8)) {
                0 => Color32::TRANSPARENT,
                _ => Color32::from_rgb(rgb[i * 3], rgb[i * 3 + 1], rgb[i * 3 + 2]),
            })
            .collect();
        out.push((key, colour));
    }
    r.done()?;
    Ok(out)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Prefs {
    pub minimap: bool,
    pub coordinates: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        Self { minimap: true, coordinates: true }
    }
}

pub fn encode_prefs(p: Prefs) -> Vec<u8> {
    let flags = u32::from(!p.minimap) | u32::from(!p.coordinates) << 1;
    let mut out = PREFS_MAGIC.to_vec();
    out.extend_from_slice(&flags.to_le_bytes());
    seal(out)
}

pub fn decode_prefs(bytes: &[u8]) -> Result<Prefs> {
    let mut r = Reader { bytes: open(bytes, PREFS_MAGIC)?, at: 0 };
    let flags = r.u32()?;
    r.done()?;
    Ok(Prefs { minimap: flags & 1 == 0, coordinates: flags & 2 == 0 })
}

/// Replace `path` through a synced temporary so a crash keeps the old file.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    let tmp = path.with_extension("tmp");
    let mut file = std::fs::File::create(&tmp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// Missing files read as `None`; unreadable or corrupt ones are reported.
pub fn read<T>(path: &Path, decode: impl Fn(&[u8]) -> Result<T>) -> Option<T> {
    let bytes = std::fs::read(path).ok()?;
    match decode(&bytes) {
        Ok(v) => Some(v),
        Err(e) => {
            eprintln!("[map] Ignoring {}: {e:#}", path.display());
            None
        }
    }
}

#[cfg(test)]
#[path = "sidecar_tests.rs"]
mod tests;
