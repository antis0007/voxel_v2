//! Terrain transfer credits belong to the device, not the current world/cache.
//! Publication cancellation cannot release bytes still owned by queue staging.
use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};

// Fits one maximum arena mesh, while bounding small-upload callback backlog.
pub(super) const BYTE_LIMIT: usize = 192 << 20;
const JOB_LIMIT: usize = 16;

#[derive(Default)]
pub(super) struct UploadStaging {
    bytes: AtomicUsize,
    jobs: AtomicUsize,
}

impl UploadStaging {
    pub(super) fn reserve(self: &Arc<Self>, bytes: usize) -> Option<Credit> {
        self.jobs.fetch_update(Ordering::AcqRel, Ordering::Acquire,
            |n| (n < JOB_LIMIT).then_some(n + 1)).ok()?;
        if self.bytes.fetch_update(Ordering::AcqRel, Ordering::Acquire,
            |n| n.checked_add(bytes).filter(|&sum| sum <= BYTE_LIMIT)).is_err() {
            self.jobs.fetch_sub(1, Ordering::AcqRel);
            return None;
        }
        Some(Credit { ledger: Arc::clone(self), bytes })
    }

    pub(super) fn bytes(&self) -> usize { self.bytes.load(Ordering::Acquire) }
    pub(super) fn jobs(&self) -> usize { self.jobs.load(Ordering::Acquire) }

}

pub(super) struct Credit {
    ledger: Arc<UploadStaging>,
    bytes: usize,
}

impl Drop for Credit {
    fn drop(&mut self) {
        self.ledger.bytes.fetch_sub(self.bytes, Ordering::AcqRel);
        self.ledger.jobs.fetch_sub(1, Ordering::AcqRel);
    }
}

/// Called after the frame or fallback submit which flushed these writes.
pub(super) fn release_after_submit(queue: &wgpu::Queue, credits: Vec<Credit>) {
    if credits.is_empty() { return; }
    queue.on_submitted_work_done(move || drop(credits));
}
