//! Survey immutable site geography away from the simulation thread.
//! Placement, saved rewards and mechanism changes stay with the live World.
use engine_worldgen::{WorldSampleConfig, sites};
use glam::{IVec3, Vec3};
use std::sync::{Arc, Mutex, OnceLock, atomic::{AtomicBool, Ordering}, mpsc};

const MAX_DRIFT_M: f32 = 24.0;

#[derive(Clone, Copy)]
struct Origin {
    cfg: WorldSampleConfig,
    feet: Vec3,
}

impl Origin {
    fn current(self, cfg: WorldSampleConfig, feet: Vec3) -> bool {
        self.cfg == cfg && self.feet.distance_squared(feet) <= MAX_DRIFT_M * MAX_DRIFT_M
    }
}

pub(super) struct Candidate {
    pub site: IVec3,
    pub centre: Vec3,
    pub kind: u32,
}

struct Pending {
    origin: Origin,
    reply: Mutex<mpsc::Receiver<Vec<Candidate>>>,
    cancelled: Arc<AtomicBool>,
}

impl Drop for Pending {
    fn drop(&mut self) { self.cancelled.store(true, Ordering::Relaxed); }
}

struct Job {
    origin: Origin,
    reply: mpsc::SyncSender<Vec<Candidate>>,
    cancelled: Arc<AtomicBool>,
}

#[derive(Default)]
pub(crate) struct Discovery {
    pending: Option<Pending>,
    published: Option<Origin>,
}

fn lane() -> Option<&'static mpsc::SyncSender<Job>> {
    static LANE: OnceLock<Option<mpsc::SyncSender<Job>>> = OnceLock::new();
    LANE.get_or_init(|| {
        // One running survey and at most two queued requests across all players.
        let (send, receive) = mpsc::sync_channel::<Job>(2);
        match std::thread::Builder::new().name("relic-discovery".into()).spawn(move || {
            while let Ok(job) = receive.recv() {
                if job.cancelled.load(Ordering::Relaxed) { continue; }
                let mut trace = crate::gameplay_trace::Trace::new();
                let encounters = sites::encounters(&job.origin.cfg, job.origin.feet);
                if job.cancelled.load(Ordering::Relaxed) { continue; }
                let candidates = encounters.into_iter().map(|(site, centre, _)| Candidate {
                    site, centre, kind: sites::reward_kind(&job.origin.cfg, site),
                }).collect();
                trace.mark("reliquary-site-worker");
                let _ = job.reply.try_send(candidates);
            }
        }) {
            Ok(_) => Some(send),
            Err(error) => {
                eprintln!("[reliquary] discovery worker unavailable: {error}");
                None
            }
        }
    }).as_ref()
}

impl Discovery {
    /// Drop old geography before interaction after a teleport or world change.
    pub(super) fn invalidate(&mut self, cfg: WorldSampleConfig, feet: Vec3) -> bool {
        if self.pending.as_ref().is_some_and(|p| !p.origin.current(cfg, feet)) {
            self.pending = None;
        }
        if self.published.is_some_and(|origin| !origin.current(cfg, feet)) {
            self.published = None;
            return true;
        }
        false
    }

    pub(super) fn request(&mut self, cfg: WorldSampleConfig, feet: Vec3) {
        if self.pending.is_some() || !feet.is_finite() { return; }
        let Some(lane) = lane() else { return; };
        let (reply, receive) = mpsc::sync_channel(1);
        let origin = Origin { cfg, feet };
        let cancelled = Arc::new(AtomicBool::new(false));
        if lane.try_send(Job { origin, reply, cancelled: Arc::clone(&cancelled) }).is_ok() {
            self.pending = Some(Pending { origin, reply: Mutex::new(receive), cancelled });
        }
    }

    /// Called each tick; never waits for the worker and publishes each reply once.
    pub(super) fn take(&mut self, cfg: WorldSampleConfig, feet: Vec3) -> Option<Vec<Candidate>> {
        let pending = self.pending.as_mut()?;
        let reply = pending.reply.get_mut().unwrap_or_else(|error| error.into_inner()).try_recv();
        match reply {
            Ok(candidates) => {
                let origin = pending.origin;
                self.pending = None;
                if !origin.current(cfg, feet) { return None; }
                self.published = Some(origin);
                Some(candidates)
            }
            Err(mpsc::TryRecvError::Disconnected) => { self.pending = None; None }
            Err(mpsc::TryRecvError::Empty) => None,
        }
    }
}
