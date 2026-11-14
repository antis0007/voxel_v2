//! Per-Stage diagnostics, never used to select terrain values.
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReliefCacheStats {
    pub attempts: u64,
    pub hits: u64,
    pub misses: u64,
    /// Valid-envelope payloads rejected by the Stage decoder.
    pub invalid: u64,
    pub stored: u64,
    pub write_skipped: u64,
    pub bytes_read: u64,
    pub bytes_written: u64,
    pub bypassed: u64,
    pub read_errors: u64,
    pub write_errors: u64,
}

pub(super) static ATTEMPTS: AtomicU64 = AtomicU64::new(0);
pub(super) static HITS: AtomicU64 = AtomicU64::new(0);
pub(super) static MISSES: AtomicU64 = AtomicU64::new(0);
pub(super) static INVALID: AtomicU64 = AtomicU64::new(0);
pub(super) static STORED: AtomicU64 = AtomicU64::new(0);
pub(super) static WRITE_SKIPPED: AtomicU64 = AtomicU64::new(0);
pub(super) static BYTES_READ: AtomicU64 = AtomicU64::new(0);
pub(super) static BYTES_WRITTEN: AtomicU64 = AtomicU64::new(0);
pub(super) static BYPASSED: AtomicU64 = AtomicU64::new(0);
pub(super) static READ_ERRORS: AtomicU64 = AtomicU64::new(0);
pub(super) static WRITE_ERRORS: AtomicU64 = AtomicU64::new(0);

pub(super) fn snapshot() -> ReliefCacheStats {
    ReliefCacheStats { attempts: ATTEMPTS.load(Relaxed), hits: HITS.load(Relaxed),
        misses: MISSES.load(Relaxed), invalid: INVALID.load(Relaxed),
        stored: STORED.load(Relaxed), write_skipped: WRITE_SKIPPED.load(Relaxed),
        bytes_read: BYTES_READ.load(Relaxed), bytes_written: BYTES_WRITTEN.load(Relaxed),
        bypassed: BYPASSED.load(Relaxed), read_errors: READ_ERRORS.load(Relaxed),
        write_errors: WRITE_ERRORS.load(Relaxed) }
}
