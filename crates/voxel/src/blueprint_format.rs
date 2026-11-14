//! Shared encoding for the game blueprint library and offline authoring tools.
use engine_content::BlockId;

pub const MAGIC: &[u8; 8] = b"VXBLUEPR";
pub const VERSION: u16 = 1;
pub const MAX_SIDE: i32 = 256;
pub const MAX_VOLUME: usize = 1 << 21;

pub fn fnv1a64(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// Cells are ordered x, then z, then y. The palette uses stable material names.
pub fn encode(size: [u16; 3], cells: &[BlockId]) -> Option<Vec<u8>> {
    if size
        .into_iter()
        .any(|axis| axis == 0 || i32::from(axis) > MAX_SIDE)
        || size.into_iter().map(usize::from).product::<usize>() != cells.len()
        || cells.len() > MAX_VOLUME
    {
        return None;
    }
    let mut palette = Vec::new();
    let mut indices = [None; BlockId::COUNT];
    let mut runs: Vec<(u8, u32)> = Vec::new();
    for &block in cells {
        let slot = indices.get_mut(block as usize)?;
        if slot.is_none() {
            // Run indices are one byte: a blueprint holds at most 256 materials.
            *slot = Some(u8::try_from(palette.len()).ok()?);
            palette.push(block);
        }
        let index = slot.as_ref()?;
        match runs.last_mut() {
            Some((previous, count)) if previous == index => *count += 1,
            _ => runs.push((*index, 1)),
        }
    }
    let mut out = Vec::with_capacity(64 + runs.len() * 2);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    for axis in size {
        out.extend_from_slice(&axis.to_le_bytes());
    }
    out.extend_from_slice(&(palette.len() as u16).to_le_bytes());
    for block in palette {
        let name = format!("{block:?}");
        out.push(name.len() as u8);
        out.extend_from_slice(name.as_bytes());
    }
    out.extend_from_slice(&(runs.len() as u32).to_le_bytes());
    for (index, mut count) in runs {
        out.push(index);
        while count >= 128 {
            out.push((count as u8 & 0x7f) | 0x80);
            count >>= 7;
        }
        out.push(count as u8);
    }
    out.extend_from_slice(&fnv1a64(&out).to_le_bytes());
    Some(out)
}
