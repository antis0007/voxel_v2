//! Bounded local movement receipts, independent of transient sound/UI flags.
//! These describe experienced traversal, not permission to reveal a destination.
use glam::Vec3;
use std::{collections::VecDeque, path::PathBuf, sync::Arc};

#[derive(Clone)]
pub struct Crossing {
    pub sequence: u64,
    pub root: Arc<PathBuf>,
    pub opening: u64,
    pub revision: u64,
    pub entry: Vec3,
    pub exit: Vec3,
    pub source_up: Vec3,
    pub up: Vec3,
    pub forward: Vec3,
}

#[derive(Clone)]
pub struct Crossings {
    pub epoch: u64,
    sequence: u64,
    pub events: VecDeque<Crossing>,
}

impl Default for Crossings {
    fn default() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static EPOCH: AtomicU64 = AtomicU64::new(1);
        Self {
            epoch: EPOCH.fetch_add(1, Ordering::Relaxed),
            sequence: 0,
            events: VecDeque::new(),
        }
    }
}

impl Crossings {
    pub fn record(
        &mut self,
        root: Arc<PathBuf>,
        opening: u64,
        revision: u64,
        entry: Vec3,
        exit: Vec3,
        source_up: Vec3,
        up: Vec3,
        forward: Vec3,
    ) {
        if ![entry, exit, source_up, up, forward]
            .iter()
            .all(|v| v.is_finite())
        {
            return;
        }
        self.sequence = self.sequence.saturating_add(1);
        if self.events.len() == 64 {
            self.events.pop_front();
        }
        self.events.push_back(Crossing {
            sequence: self.sequence,
            root,
            opening,
            revision,
            entry,
            exit,
            source_up,
            up,
            forward,
        });
    }
}
