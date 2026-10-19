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
    planning: bool,
}
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
            .planning = false;
        self.0.available.notify_all();
    }
}
impl MeshBudget {
    // One bounded planner avoids N workers retaining scratch while all wait
    // for a large output reservation. Actual mesh construction stays parallel.
    pub fn begin_plan(self: &Arc<Self>, cancelled: &AtomicBool) -> Option<PlanningPermit> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        while state.planning {
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
        state.planning = true;
        Some(PlanningPermit(self.clone()))
    }
    pub fn acquire(self: &Arc<Self>, bytes: usize, cancelled: &AtomicBool) -> Option<MeshLease> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let ticket = state.next_ticket;
        state.next_ticket = state.next_ticket.wrapping_add(1);
        state.waiters.push_back(ticket);
        loop {
            if cancelled.load(Ordering::Relaxed) {
                state.waiters.retain(|&t| t != ticket);
                self.available.notify_all();
                return None;
            }
            // FIFO keeps a large near chunk from being starved by a continuous
            // supply of smaller far meshes. The minimum fits any supported chunk.
            if state.waiters.front() == Some(&ticket)
                && state.used.saturating_add(bytes) <= mesh_budget_mib() * 1024 * 1024
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
        let mut extra = self.budget.acquire(bytes - self.bytes, cancelled)?;
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
