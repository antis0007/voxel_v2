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

// Magic, count/flags and checksum, plus the largest legal payload. Use the
// encoded format rather than the filename so renamed archives keep their bounds.
fn file_limit(magic: &[u8]) -> Option<usize> {
    match magic {
        m if m == WAYPOINT_MAGIC => Some(20 + MAX_WAYPOINTS * (12 + 3 + MAX_NAME)),
        m if m == TILE_MAGIC => Some(20 + MAX_TILES * TILE_BYTES),
        m if m == PREFS_MAGIC => Some(20),
        _ => None,
    }
}

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
    ensure!(file_limit(magic).is_some_and(|limit| bytes.len() <= limit), "Map file exceeds limit");
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
        // abs(i32::MIN) can panic in debug builds or wrap in release builds.
        ensure!(pos.to_array().iter().all(|&v| v > -COORD_LIMIT && v < COORD_LIMIT), "Waypoint out of range");
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
        ensure!(key.to_array().iter().all(|&v| v > -COORD_LIMIT / 32 && v < COORD_LIMIT / 32), "Map tile out of range");
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

/// Reserve a sibling without truncating another writer's file or a stale temp.
fn create_temp(path: &Path) -> Result<(std::path::PathBuf, std::fs::File)> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let name = path.file_name().context("Map file has no name")?;
    for _ in 0..16 {
        let mut temp_name = name.to_os_string();
        temp_name.push(format!(".{}.{}.tmp", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        let tmp = path.with_file_name(temp_name);
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&tmp) {
            Ok(file) => return Ok((tmp, file)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.into()),
        }
    }
    bail!("Cannot reserve a map temporary file")
}

/// Publish a complete, synced sibling by rename. A failed write keeps the old
/// archive; concurrent writers cannot truncate or rename each other's temporary.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    let (tmp, mut file) = create_temp(path)?;
    let result = (|| -> Result<()> {
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&tmp, path)?;
        Ok(())
    })();
    // The closure has closed the file even on error, so Windows can remove it.
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

fn read_file(path: &Path) -> Result<Vec<u8>> {
    use std::io::Read;
    let mut file = std::fs::File::open(path)?;
    let mut magic = [0u8; 8];
    file.read_exact(&mut magic)?;
    let limit = file_limit(&magic).context("Unsupported map file")?;
    ensure!(file.metadata()?.len() <= limit as u64, "Map file exceeds limit");
    let mut bytes = magic.to_vec();
    // The sentinel byte catches growth after metadata inspection without an
    // unbounded allocation. Truncation and checksum errors remain decoder errors.
    file.take((limit - magic.len() + 1) as u64).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= limit, "Map file exceeds limit");
    Ok(bytes)
}

/// Missing files read as `None`; unreadable or corrupt ones are reported.
pub fn read<T>(path: &Path, decode: impl Fn(&[u8]) -> Result<T>) -> Option<T> {
    let result = read_file(path).and_then(|bytes| decode(&bytes));
    match result {
        Ok(v) => Some(v),
        Err(e) if e.downcast_ref::<std::io::Error>().is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound) => None,
        Err(e) => {
            eprintln!("[map] Ignoring {}: {e:#}", path.display());
            None
        }
    }
}

#[cfg(test)]
#[path = "sidecar_tests.rs"]
mod tests;
