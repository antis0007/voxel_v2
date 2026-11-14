//! Optional disposable reuse of complete, version-bound geological solves.
mod storage;
mod key;
#[cfg(test)] mod tests;
use std::path::PathBuf;
use std::sync::OnceLock;
use super::{tectonics::Plates, geo};
static DIRECTORY: OnceLock<PathBuf> = OnceLock::new();

/// Configure derived geology storage before starting world-generation workers.
/// This directory must never be a player-save directory.
pub(crate) fn configure(directory: PathBuf) -> bool {
    if directory.as_os_str().is_empty() { return false; }
    DIRECTORY.set(directory).is_ok()
}

pub(super) fn restore(plates: &mut Plates, progress: &geo::Progress) -> bool {
    let Some(root) = DIRECTORY.get() else { return false; };
    let cfg = plates.config();
    let Some(key) = key::for_config(&cfg, plates.shape_shelf) else { return false; };
    let store = storage::Store::new(root.clone());
    let bytes = match store.read(&key) {
        Ok(Some(bytes)) => bytes,
        Ok(None) => { eprintln!("[geology-cache] miss seed={} generator={}", cfg.seed, cfg.generator_version); return false; }
        Err(error) => { eprintln!("[geology-cache] read unavailable: {error}"); return false; }
    };
    match super::snapshot::decode(&bytes, &cfg, plates.shape_shelf) {
        Ok((restored, frames)) => {
            *plates = restored;
            let count = frames.len();
            for frame in frames { progress.publish_drift(frame); }
            eprintln!("[geology-cache] hit seed={} generator={} bytes={} frames={count}", cfg.seed, cfg.generator_version, bytes.len());
            true
        }
        Err(error) => { eprintln!("[geology-cache] invalid snapshot: {error}"); false }
    }
}

pub(super) fn store(plates: &Plates, progress: &geo::Progress) {
    let Some(root) = DIRECTORY.get() else { return; };
    let Some(key) = key::for_config(&plates.config(), plates.shape_shelf) else { return; };
    match super::snapshot::encode(plates, progress) {
        Ok(bytes) => match storage::Store::new(root.clone()).write(&key, &bytes) {
            Ok(()) => eprintln!("[geology-cache] stored bytes={}", bytes.len()),
            Err(error) => eprintln!("[geology-cache] write skipped: {error}"),
        },
        Err(error) => eprintln!("[geology-cache] encoding skipped: {error}"),
    }
}

// Complete canonical identity reused by independent derived caches.
pub(super) fn identity(cfg: &crate::WorldSampleConfig, shelf: bool) -> Option<Vec<u8>> {
    key::for_config(cfg, shelf)
}
