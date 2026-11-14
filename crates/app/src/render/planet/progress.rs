//! Measured work from one planet cover job.
use std::sync::Mutex;

#[derive(Clone, Copy, Debug, Default)]
pub struct Snapshot {
    pub name: &'static str,
    pub done: u64,
    pub total: u64,
    pub regions: u32,
    pub region_total: u32,
}

pub struct Progress(Mutex<Snapshot>);

impl Progress {
    pub fn new(region_total: u32) -> Self {
        Self(Mutex::new(Snapshot { name: "Planet Cover", region_total, ..Snapshot::default() }))
    }

    pub fn snapshot(&self) -> Snapshot {
        self.0.lock().map_or(Snapshot::default(), |s| *s)
    }

    pub fn begin(&self, name: &'static str, total: u64) {
        if let Ok(mut s) = self.0.lock() {
            s.name = name;
            s.done = 0;
            s.total = total;
        }
    }

    pub fn advance(&self, count: u64) {
        if let Ok(mut s) = self.0.lock() {
            s.done = s.done.saturating_add(count);
            if s.total > 0 { s.done = s.done.min(s.total); }
        }
    }

    pub fn finish_region(&self) {
        if let Ok(mut s) = self.0.lock() {
            s.regions = (s.regions + 1).min(s.region_total);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_totals_keep_real_work_counts() {
        let p = Progress::new(7);
        p.begin("Coast Refinement", 0);
        p.advance(17);
        assert_eq!(p.snapshot().done, 17);
        assert_eq!(p.snapshot().total, 0);
    }

    #[test]
    fn new_phase_resets_counts_but_keeps_completed_regions() {
        let p = Progress::new(7);
        p.begin("Surface Samples", 100);
        p.advance(101);
        assert_eq!(p.snapshot().done, 100);
        p.finish_region();
        p.begin("Coast Refinement", 0);
        let s = p.snapshot();
        assert_eq!((s.done, s.total, s.regions, s.region_total), (0, 0, 1, 7));
    }

    #[test]
    fn parallel_rows_publish_exact_counts() {
        let p = Progress::new(6);
        p.begin("Surface Samples", 8000);
        std::thread::scope(|scope| {
            for _ in 0..8 { let p = &p; scope.spawn(move || { for _ in 0..100 { p.advance(10); } }); }
        });
        assert_eq!(p.snapshot().done, 8000);
    }

    #[test]
    fn each_job_has_independent_state() {
        let a = Progress::new(7);
        a.advance(10);
        a.finish_region();
        let b = Progress::new(6);
        assert_eq!((b.snapshot().done, b.snapshot().regions), (0, 0));
    }
}
