//! Shared worker pool for horizon sampling.
//!
//! Sample cost varies about 7x between lattice regions, so splitting the work
//! into one equal slice per worker left most workers idle while the slowest
//! slice finished. Here workers claim small blocks from a shared counter
//! instead, and the results come back in item order, so the output is the same
//! as a serial pass.
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// Worker count for one horizon job: the available cores, at most `cap`, or one
/// when `VOXEL_HORIZON_SERIAL=1`.
pub(in crate::render) fn workers(cap: usize) -> usize {
    if std::env::var("VOXEL_HORIZON_SERIAL").as_deref() == Ok("1") {
        return 1;
    }
    std::thread::available_parallelism()
        .map_or(1, |count| count.get())
        .min(cap)
        .max(1)
}

/// Reads a worker cap override such as `VOXEL_HORIZON_WORKERS`, for timing sweeps.
pub(in crate::render) fn cap_from_env(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|&count| count >= 1)
        .unwrap_or(default)
}

/// Applies `job` to every item on up to `workers` threads and returns the
/// results in item order.
///
/// Workers claim `block` consecutive items at a time. Keep `items` spatially
/// sorted so each block reuses the same erosion tiles. Returns `None` when
/// `cancel` is set, a job returns `None`, or a worker panics.
pub(in crate::render) fn map_blocks<T, R, F>(
    items: &[T],
    block: usize,
    workers: usize,
    cancel: &AtomicBool,
    job: F,
) -> Option<Vec<R>>
where
    T: Sync,
    R: Send,
    F: Fn(&T) -> Option<R> + Sync,
{
    let block = block.max(1);
    let block_count = items.len().div_ceil(block);
    let next_block = AtomicUsize::new(0);
    let failed = AtomicBool::new(false);
    let stop = || failed.load(Ordering::Relaxed) || cancel.load(Ordering::Relaxed);

    let worker = || {
        let mut finished = Vec::new();
        loop {
            let index = next_block.fetch_add(1, Ordering::Relaxed);
            if index >= block_count || stop() {
                return finished;
            }
            let start = index * block;
            let part = &items[start..(start + block).min(items.len())];
            let results: Option<Vec<R>> = part
                .iter()
                .map(|item| if stop() { None } else { job(item) })
                .collect();
            match results {
                Some(results) => finished.push((index, results)),
                None => {
                    failed.store(true, Ordering::Relaxed);
                    return finished;
                }
            }
        }
    };

    let workers = workers.clamp(1, block_count.max(1));
    let mut finished: Vec<(usize, Vec<R>)> = if workers == 1 {
        worker()
    } else {
        std::thread::scope(|scope| {
            let handles: Vec<_> = (0..workers).map(|_| scope.spawn(worker)).collect();
            // Join every worker, so none outlives an early failure.
            let mut all = Vec::with_capacity(block_count);
            for handle in handles {
                match handle.join() {
                    Ok(part) => all.extend(part),
                    Err(_) => failed.store(true, Ordering::Relaxed),
                }
            }
            all
        })
    };
    if stop() {
        return None;
    }
    finished.sort_unstable_by_key(|&(index, _)| index);
    Some(finished.into_iter().flat_map(|(_, results)| results).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn results_keep_item_order_for_any_worker_count() {
        let items: Vec<u32> = (0..1000).collect();
        let cancel = AtomicBool::new(false);
        for workers in [1, 3, 8] {
            for block in [1, 7, 512, 5000] {
                let doubled = map_blocks(&items, block, workers, &cancel, |&n| Some(n * 2)).unwrap();
                assert_eq!(doubled, items.iter().map(|n| n * 2).collect::<Vec<_>>());
            }
        }
    }

    #[test]
    fn a_failed_job_or_cancel_returns_none() {
        let items: Vec<u32> = (0..100).collect();
        let cancel = AtomicBool::new(false);
        assert!(map_blocks(&items, 8, 4, &cancel, |&n| (n != 50).then_some(n)).is_none());
        cancel.store(true, Ordering::Relaxed);
        assert!(map_blocks(&items, 8, 4, &cancel, |&n| Some(n)).is_none());
    }

    #[test]
    fn empty_input_is_empty_output() {
        let cancel = AtomicBool::new(false);
        assert_eq!(map_blocks(&[] as &[u32], 8, 4, &cancel, |&n| Some(n)), Some(Vec::new()));
    }
}
