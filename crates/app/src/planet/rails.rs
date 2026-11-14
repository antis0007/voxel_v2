//! Cached star-system snapshots shared by flight, sky and lighting.
//!
//! One fixed step and one frame ask for the same few ticks many times; each
//! evaluation solves Kepler for every body and allocates two vectors.
use engine_space::{Snapshot, System};
use std::cell::RefCell;

/// Distinct ticks kept: the current step, the one before it, and the frame's.
const SLOTS: usize = 4;

thread_local! {
    static CACHE: RefCell<Vec<(u64, Snapshot)>> = const { RefCell::new(Vec::new()) };
}

/// Runs `f` on the system snapshot at `tick`, evaluating it at most once per tick.
pub fn with_snapshot<R>(system: &System, tick: u64, f: impl FnOnce(&Snapshot) -> R) -> R {
    CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(i) = cache.iter().position(|(t, _)| *t == tick) {
            return f(&cache[i].1);
        }
        let mut snapshot = if cache.len() >= SLOTS { cache.remove(0).1 } else { Snapshot::default() };
        system.evaluate(tick, &mut snapshot);
        cache.push((tick, snapshot));
        f(&cache[cache.len() - 1].1)
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn cached_snapshots_match_fresh_evaluation() {
        let system = super::super::influence::system().unwrap();
        for tick in [0, 1, 0, 9_000, 1, 2, 3, 4, 0] {
            let mut fresh = engine_space::Snapshot::default();
            system.evaluate(tick, &mut fresh);
            super::with_snapshot(system, tick, |s| assert_eq!(s.position, fresh.position));
        }
    }
}
