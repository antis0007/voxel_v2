//! One heap allowance covers construction scratch and every completed result.
//! Workers reserve BEFORE allocating output; reservations survive world resets.
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;
static LIMIT_MIB: AtomicUsize = AtomicUsize::new(256);
pub fn mesh_budget_mib() -> usize {
    LIMIT_MIB.load(Ordering::Relaxed)
}
pub fn set_mesh_budget_mib(value: usize) {
    LIMIT_MIB.store(value.clamp(160, 2048), Ordering::Relaxed);
}
#[derive(Default)]
struct State {
    used: usize,
    next_ticket: usize,
    waiters: VecDeque<usize>,
    planning: usize,
}
/// Exact-LOD jobs that may plan at once. Each holds `PLANNING_BYTES` while it
/// waits to grow, so the count is bounded: four hold 96 MiB, which leaves
/// 64 MiB for the largest single mesh under the smallest allowed budget
/// (160 MiB). One planner made every exact mesh wait on a single thread.
const PLANNERS: usize = 4;
const _: () = assert!(PLANNERS * crate::mesh::PLANNING_BYTES + 64 * 1024 * 1024 <= 160 * 1024 * 1024);
#[derive(Default)]
pub(super) struct MeshBudget {
    state: Mutex<State>,
    available: Condvar,
}
pub(super) struct MeshLease {
    budget: Arc<MeshBudget>,
    bytes: usize,
}
pub(super) struct PlanningPermit(Arc<MeshBudget>);
impl Drop for PlanningPermit {
    fn drop(&mut self) {
        self.0
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .planning -= 1;
        self.0.available.notify_all();
    }
}
impl MeshBudget {
    // A bounded planner count avoids every worker retaining scratch while all
    // wait for a large output reservation. Mesh construction stays parallel.
    pub fn begin_plan(self: &Arc<Self>, cancelled: &AtomicBool) -> Option<PlanningPermit> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        while state.planning >= PLANNERS {
            if cancelled.load(Ordering::Relaxed) {
                return None;
            }
            state = self
                .available
                .wait_timeout(state, Duration::from_millis(25))
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
        if cancelled.load(Ordering::Relaxed) {
            return None;
        }
        state.planning += 1;
        Some(PlanningPermit(self.clone()))
    }
    pub fn acquire(self: &Arc<Self>, bytes: usize, cancelled: &AtomicBool) -> Option<MeshLease> {
        self.acquire_retaining(bytes, 0, cancelled)
    }
    // `retained` is already charged to state.used, but cannot be reclaimed
    // while this request waits. Check the whole job against a changing cap.
    fn acquire_retaining(
        self: &Arc<Self>, bytes: usize, retained: usize, cancelled: &AtomicBool,
    ) -> Option<MeshLease> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let ticket = state.next_ticket;
        state.next_ticket = state.next_ticket.wrapping_add(1);
        state.waiters.push_back(ticket);
        loop {
            let limit = mesh_budget_mib() * 1024 * 1024;
            if cancelled.load(Ordering::Relaxed) || retained.saturating_add(bytes) > limit {
                state.waiters.retain(|&t| t != ticket);
                self.available.notify_all();
                return None;
            }
            // FIFO keeps a large near chunk from being starved by a continuous
            // supply of smaller far meshes. Impossible requests leave the FIFO
            // instead of holding the planner and starving every later job.
            if state.waiters.front() == Some(&ticket)
                && state.used.saturating_add(bytes) <= limit
            {
                state.waiters.pop_front();
                state.used += bytes;
                self.available.notify_all();
                return Some(MeshLease {
                    budget: Arc::clone(self),
                    bytes,
                });
            }
            state = self
                .available
                .wait_timeout(state, Duration::from_millis(25))
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
    }
    pub fn used(&self) -> usize {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).used
    }
}
impl MeshLease {
    pub fn grow_to(&mut self, bytes: usize, cancelled: &AtomicBool) -> Option<()> {
        if bytes <= self.bytes {
            return Some(());
        }
        let mut extra = self.budget.acquire_retaining(bytes - self.bytes, self.bytes, cancelled)?;
        self.bytes += extra.bytes;
        extra.bytes = 0;
        Some(())
    }
    pub fn finish(&mut self, bytes: usize) {
        assert!(
            bytes <= self.bytes,
            "Mesh exceeded its conservative allocation reservation"
        );
        self.budget
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .used -= self.bytes - bytes;
        self.bytes = bytes;
        self.budget.available.notify_all();
    }
}
impl Drop for MeshLease {
    fn drop(&mut self) {
        self.budget
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .used -= self.bytes;
        self.budget.available.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::mpsc, thread};
    static TEST_LIMIT: Mutex<()> = Mutex::new(());

    // Cancel and join even on a regression: a failing fault test must not leave
    // a worker holding a lease or waiting forever.
    fn bounded_attempt(grow: bool) {
        let budget = Arc::new(MeshBudget::default());
        let cancelled = Arc::new(AtomicBool::new(false));
        let worker_budget = budget.clone();
        let worker_cancelled = cancelled.clone();
        let (tx, rx) = mpsc::channel();
        let cap = mesh_budget_mib() * 1024 * 1024;
        let worker = thread::spawn(move || {
            let rejected = if grow {
                let mut lease = worker_budget.acquire(cap / 2 + 1, &worker_cancelled).unwrap();
                let rejected = lease.grow_to(cap + 1, &worker_cancelled).is_none();
                // A rejected growth must not release its still-live input.
                assert_eq!(worker_budget.used(), cap / 2 + 1);
                rejected
            } else {
                worker_budget.acquire(cap + 1, &worker_cancelled).is_none()
            };
            tx.send(rejected).unwrap();
        });
        let result = rx.recv_timeout(Duration::from_secs(1));
        cancelled.store(true, Ordering::Relaxed);
        worker.join().unwrap();
        assert_eq!(budget.used(), 0);
        assert!(budget.state.lock().unwrap().waiters.is_empty());
        assert_eq!(result.ok(), Some(true), "impossible reservation waited for cancellation");
        let next = budget.acquire(cap, &AtomicBool::new(false)).unwrap();
        assert_eq!(budget.used(), cap);
        drop(next);
        assert_eq!(budget.used(), 0);
    }

    #[test]
    fn oversized_reservation_releases_fifo_head() {
        let _guard = TEST_LIMIT.lock().unwrap();
        bounded_attempt(false);
    }

    #[test]
    fn oversized_growth_counts_retained_lease() {
        let _guard = TEST_LIMIT.lock().unwrap();
        bounded_attempt(true);
    }

    #[test]
    fn budget_contraction_rejects_waiting_growth_without_releasing_live_bytes() {
        let _guard = TEST_LIMIT.lock().unwrap();
        struct RestoreLimit(usize);
        impl Drop for RestoreLimit {
            fn drop(&mut self) { set_mesh_budget_mib(self.0); }
        }
        let _restore = RestoreLimit(mesh_budget_mib());
        set_mesh_budget_mib(256);
        let mib = 1024 * 1024;
        let budget = Arc::new(MeshBudget::default());
        let cancelled = Arc::new(AtomicBool::new(false));
        let mut lease = budget.acquire(24 * mib, &cancelled).unwrap();
        let occupied = budget.acquire(232 * mib, &cancelled).unwrap();
        let worker_cancelled = cancelled.clone();
        let (tx, rx) = mpsc::channel();
        let worker = thread::spawn(move || {
            let result = lease.grow_to(200 * mib, &worker_cancelled).is_none();
            tx.send(result).unwrap();
        });
        let deadline = std::time::Instant::now() + Duration::from_secs(1);
        while budget.state.lock().unwrap().waiters.is_empty()
            && std::time::Instant::now() < deadline {
            thread::sleep(Duration::from_millis(1));
        }
        let queued = !budget.state.lock().unwrap().waiters.is_empty();
        set_mesh_budget_mib(160);
        let result = rx.recv_timeout(Duration::from_secs(1));
        cancelled.store(true, Ordering::Relaxed);
        worker.join().unwrap();
        assert!(queued, "growth must start waiting before the cap contracts");
        assert_eq!(result.ok(), Some(true));
        assert_eq!(budget.used(), 232 * mib, "existing backing remains charged above the new cap");
        assert!(budget.state.lock().unwrap().waiters.is_empty());
        drop(occupied);
        assert_eq!(budget.used(), 0);
    }
}
