//! Bounded asynchronous column planning. Erosion never runs on the frame thread.
use super::visibility::{ColumnRange, column_y_chunk_range};
use crate::world::WorldSampleConfig;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
const CAPACITY: usize = 64;
const BACKLOG: usize = 32768;
#[derive(Clone, Copy, PartialEq, Eq)]
struct Interest {
    center: (i32, i32),
    radius: i32,
}
impl Interest {
    fn contains(self, c: (i32, i32)) -> bool {
        c.0.abs_diff(self.center.0).max(c.1.abs_diff(self.center.1)) <= self.radius as u32
    }
}
struct Job {
    epoch: u64,
    c: (i32, i32),
    cfg: WorldSampleConfig,
}
struct Done {
    epoch: u64,
    c: (i32, i32),
    range: Option<ColumnRange>,
    failed: bool,
}
pub(super) struct ColumnPlans {
    ranges: HashMap<(i32, i32), ColumnRange>,
    pending: HashSet<(i32, i32)>,
    backlog: VecDeque<(i32, i32)>,
    waiting: HashSet<(i32, i32)>,
    ready: Vec<(i32, i32)>,
    cfg: Option<WorldSampleConfig>,
    epoch: Arc<AtomicU64>,
    interest: Arc<Mutex<Interest>>,
    tx: Option<mpsc::SyncSender<Job>>,
    rx: mpsc::Receiver<Done>,
    retry: Instant,
}
impl ColumnPlans {
    pub fn new() -> Self {
        let (_, rx) = mpsc::sync_channel(1);
        Self {
            ranges: HashMap::new(),
            pending: HashSet::new(),
            backlog: VecDeque::new(),
            waiting: HashSet::new(),
            ready: Vec::new(),
            cfg: None,
            epoch: Arc::new(AtomicU64::new(0)),
            interest: Arc::new(Mutex::new(Interest {
                center: (0, 0),
                radius: 0,
            })),
            tx: None,
            rx,
            retry: Instant::now(),
        }
    }
    pub fn clear(&mut self) {
        self.epoch.fetch_add(1, Ordering::Relaxed);
        self.ranges.clear();
        self.pending.clear();
        self.backlog.clear();
        self.waiting.clear();
        self.ready.clear();
    }
    pub fn set_interest(&mut self, center: (i32, i32), radius: i32) {
        let next = Interest {
            center,
            radius: radius.max(0),
        };
        let mut current = self.interest.lock().unwrap_or_else(|e| e.into_inner());
        if *current == next {
            return;
        }
        *current = next;
        drop(current);
        self.retain(|&c, _| next.contains(c));
    }
    pub fn begin(&mut self, cfg: WorldSampleConfig) {
        if self.cfg != Some(cfg) {
            self.clear();
            self.cfg = Some(cfg);
        }
        if self.tx.is_none() && Instant::now() >= self.retry {
            self.start();
        }
        if self.tx.is_none() {
            return;
        }
        let start = Instant::now();
        for _ in 0..CAPACITY {
            match self.rx.try_recv() {
                Ok(done) if done.epoch == self.epoch.load(Ordering::Relaxed) => {
                    self.pending.remove(&done.c);
                    let interest = *self.interest.lock().unwrap_or_else(|e| e.into_inner());
                    if !interest.contains(done.c) {
                        continue;
                    }
                    if let Some(range) = done.range {
                        self.ranges.insert(done.c, range);
                        self.ready.push(done.c);
                    } else {
                        if self.waiting.insert(done.c) {
                            self.backlog.push_back(done.c);
                        }
                        if done.failed {
                            self.retry = Instant::now() + Duration::from_secs(1);
                        }
                    }
                }
                Ok(_) => {}
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.tx = None;
                    for c in self.pending.drain() {
                        if self.waiting.insert(c) {
                            self.backlog.push_front(c);
                        }
                    }
                    self.retry = Instant::now() + Duration::from_secs(2);
                    break;
                }
            }
            if start.elapsed() >= Duration::from_millis(1) {
                break;
            }
        }
        self.pump();
    }
    fn start(&mut self) {
        let (tx, jobs) = mpsc::sync_channel::<Job>(CAPACITY);
        let (done, rx) = mpsc::sync_channel(CAPACITY);
        let jobs = Arc::new(Mutex::new(jobs));
        let count = std::thread::available_parallelism().map_or(1, |n| (n.get() / 4).clamp(1, 2));
        let mut started = 0;
        for i in 0..count {
            let jobs = Arc::clone(&jobs);
            let done = done.clone();
            let epoch = Arc::clone(&self.epoch);
            let interest = Arc::clone(&self.interest);
            match std::thread::Builder::new()
                .name(format!("column-plan-{i}"))
                .spawn(move || {
                    loop {
                        let job = jobs.lock().unwrap_or_else(|e| e.into_inner()).recv();
                        let Ok(job) = job else { break };
                        if job.epoch != epoch.load(Ordering::Relaxed) {
                            continue;
                        }
                        // A teleport can leave a queue full of obsolete work. Skip it
                        // before erosion; at most the currently running workers finish.
                        let relevant = || {
                            interest
                                .lock()
                                .unwrap_or_else(|e| e.into_inner())
                                .contains(job.c)
                        };
                        let result = relevant().then(|| {
                            std::panic::catch_unwind(|| {
                                column_y_chunk_range(job.c.0, job.c.1, &job.cfg)
                            })
                        });
                        let failed = matches!(result, Some(Err(_)));
                        let range = result.and_then(Result::ok).filter(|_| relevant());
                        if done
                            .send(Done {
                                epoch: job.epoch,
                                c: job.c,
                                range,
                                failed,
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                }) {
                Ok(_) => started += 1,
                Err(e) => eprintln!("Column planner startup failed: {e}"),
            }
        }
        if started > 0 {
            self.tx = Some(tx);
            self.rx = rx;
        } else {
            self.retry = Instant::now() + Duration::from_secs(2);
        }
    }
    fn pump(&mut self) {
        if Instant::now() < self.retry {
            return;
        }
        let Some(tx) = &self.tx else { return };
        let Some(cfg) = self.cfg else { return };
        while self.pending.len() < CAPACITY {
            let Some(c) = self.backlog.pop_front() else {
                break;
            };
            match tx.try_send(Job {
                epoch: self.epoch.load(Ordering::Relaxed),
                c,
                cfg,
            }) {
                Ok(()) => {
                    self.waiting.remove(&c);
                    self.pending.insert(c);
                }
                Err(mpsc::TrySendError::Full(_)) => {
                    self.backlog.push_front(c);
                    break;
                }
                Err(mpsc::TrySendError::Disconnected(_)) => {
                    self.tx = None;
                    self.retry = Instant::now() + Duration::from_secs(2);
                    break;
                }
            }
        }
    }
    pub fn range(&mut self, c: (i32, i32)) -> Option<ColumnRange> {
        if let Some(r) = self.ranges.get(&c) {
            return Some(*r);
        }
        if !self.pending.contains(&c) && !self.waiting.contains(&c) && self.backlog.len() < BACKLOG
        {
            self.waiting.insert(c);
            self.backlog.push_back(c);
        }
        None
    }
    pub fn completed(&mut self) -> Vec<((i32, i32), ColumnRange)> {
        let ready = std::mem::take(&mut self.ready);
        ready
            .into_iter()
            .filter_map(|c| self.ranges.get(&c).copied().map(|r| (c, r)))
            .collect()
    }
    pub fn retain(&mut self, mut keep: impl FnMut(&(i32, i32), &mut ColumnRange) -> bool) {
        self.ranges.retain(|c, r| keep(c, r));
        self.backlog.retain(|c| keep(c, &mut (0, 0, 0)));
        self.waiting.retain(|c| keep(c, &mut (0, 0, 0)));
        self.ready.retain(|c| self.ranges.contains_key(c));
    }
}
