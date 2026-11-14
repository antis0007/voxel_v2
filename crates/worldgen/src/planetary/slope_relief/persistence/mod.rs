//! Opt-in disposable Stage reuse. Authority exists only on published Worlds.
mod codec;
mod key;
mod stats;
mod storage;
use std::{path::PathBuf, sync::{Arc, OnceLock, atomic::Ordering::Relaxed}};
use super::{Key, Stage, World, WorldSampleConfig};
pub use stats::ReliefCacheStats;

static STORE: OnceLock<Arc<storage::Store>> = OnceLock::new();

pub(super) fn configure(directory: PathBuf) -> bool {
    if directory.as_os_str().is_empty() || !directory.is_absolute() { return false; }
    STORE.set(Arc::new(storage::Store::new(directory))).is_ok()
}

pub(super) fn statistics() -> ReliefCacheStats { stats::snapshot() }

pub(super) struct Context {
    store: Arc<storage::Store>,
    construction: Vec<u8>,
    checksum: u64,
    shelf: bool,
}

pub(super) fn sealed_context(cfg: &WorldSampleConfig, shelf: bool, checksum: u64) -> Option<Context> {
    let store = Arc::clone(STORE.get()?);
    let construction = crate::planetary::geology_cache::identity(cfg, shelf)?;
    Some(Context { store, construction, checksum, shelf })
}

pub(super) fn solve_cached(world: &World, cfg: &WorldSampleConfig, tile: Key,
    spacing: f64, passes: usize, parent: Option<&Stage>, distant: bool) -> Stage
{
    let solve = || Stage::solve(world, cfg, tile, spacing, passes, parent, distant);
    let Some(context) = &world.slope_tiles.persistence else {
        stats::BYPASSED.fetch_add(1, Relaxed);
        return solve();
    };
    let Some(request) = key::request(&context.construction, context.checksum, context.shelf,
        cfg, tile, spacing, passes, distant, parent) else {
        stats::BYPASSED.fetch_add(1, Relaxed);
        return solve();
    };
    stats::ATTEMPTS.fetch_add(1, Relaxed);
    let mut replace_invalid = false;
    match context.store.read(&request.bytes) {
        Ok(Some(bytes)) => {
            stats::BYTES_READ.fetch_add(bytes.len() as u64, Relaxed);
            match codec::decode(&bytes, &request.grid,
                crate::planetary::composed_drainage(cfg.generator_version)) {
                Ok(stage) => {
                    stats::HITS.fetch_add(1, Relaxed);
                    return stage;
                }
                Err(_) => {
                    stats::INVALID.fetch_add(1, Relaxed);
                    replace_invalid = true;
                }
            }
        }
        Ok(None) => {}
        Err(_) => { stats::READ_ERRORS.fetch_add(1, Relaxed); }
    }
    stats::MISSES.fetch_add(1, Relaxed);
    let stage = solve();
    match codec::encode(&stage) {
        Ok(bytes) => match context.store.try_write(&request.bytes, &bytes, replace_invalid) {
            Ok(storage::WriteOutcome::Stored) => {
                stats::STORED.fetch_add(1, Relaxed);
                stats::BYTES_WRITTEN.fetch_add(bytes.len() as u64, Relaxed);
            }
            Ok(_) => { stats::WRITE_SKIPPED.fetch_add(1, Relaxed); }
            Err(_) => {
                stats::WRITE_ERRORS.fetch_add(1, Relaxed);
                stats::WRITE_SKIPPED.fetch_add(1, Relaxed);
            }
        },
        Err(_) => { stats::WRITE_SKIPPED.fetch_add(1, Relaxed); }
    }
    stage
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod authority_tests;

#[cfg(test)]
mod authority_strengthening;
