//! Bind both World construction and query configuration, plus the entire parent.
use super::super::{Grid, Key, Stage, CORE_M, PAD_M};
use super::codec;
use sha2::{Digest, Sha256};
use crate::WorldSampleConfig;

pub(super) struct Request { pub bytes: Vec<u8>, pub grid: Grid }

pub(super) fn request(construction: &[u8], checksum: u64, shelf: bool,
    cfg: &WorldSampleConfig, key: Key, spacing: f64, passes: usize,
    distant: bool, parent: Option<&Stage>) -> Option<Request>
{
    let query = crate::planetary::geology_cache::identity(cfg, shelf)?;
    from_identities(construction, &query, checksum, key, spacing, passes, distant, parent)
}

pub(super) fn from_identities(construction: &[u8], query: &[u8], checksum: u64,
    key: Key, spacing: f64, passes: usize, distant: bool, parent: Option<&Stage>) -> Option<Request>
{
    // Only canonical production stage shapes. Other direct solves remain pure.
    if key.face >= 6 || ![(32.0_f64, 8), (16.0, 7), (8.0, 8), (4.0, 5)]
        .iter().any(|&(s, p)| spacing.to_bits() == s.to_bits() && passes == p)
        || construction.len() > 1024 || query.len() > 1024 { return None; }
    let grid = Grid { n: ((CORE_M + 2.0 * PAD_M) / spacing) as usize,
        cell_m: spacing,
        min_m: [f64::from(key.x) * CORE_M - PAD_M, f64::from(key.z) * CORE_M - PAD_M] };
    let mut bytes = b"AXIOM-REGIONAL-STAGE".to_vec();
    bytes.extend_from_slice(&1u32.to_le_bytes());
    for identity in [construction, query] {
        bytes.extend_from_slice(&(identity.len() as u32).to_le_bytes());
        bytes.extend_from_slice(identity);
    }
    bytes.extend_from_slice(&checksum.to_le_bytes());
    bytes.push(key.face);
    bytes.extend_from_slice(&key.x.to_le_bytes());
    bytes.extend_from_slice(&key.z.to_le_bytes());
    bytes.extend_from_slice(&spacing.to_bits().to_le_bytes());
    bytes.extend_from_slice(&(passes as u32).to_le_bytes());
    bytes.push(u8::from(distant));
    bytes.push(u8::from(parent.is_some()));
    if let Some(parent) = parent {
        bytes.extend_from_slice(&Sha256::digest(codec::encode(parent).ok()?));
    }
    Some(Request { bytes, grid })
}
