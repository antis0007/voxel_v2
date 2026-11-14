//! Exact terrain attachment heights, prepared before drawing a corridor.
//! Cold 3x3 clearance surveys must not block a frame at a planet-face crossing.
use crate::world::{WorldSampleConfig, orbital};
use glam::IVec2;
use std::{cell::RefCell, collections::{HashMap, VecDeque}, sync::{Arc, OnceLock, mpsc,
    atomic::{AtomicBool, Ordering}}};

#[derive(Clone, Copy)]
pub(super) struct Heights {
    pub hold: f32,
    pub footing: f32,
    pub roots: [f32; 2],
}

#[derive(Default)]
struct Cache {
    cfg: Option<WorldSampleConfig>,
    ready: HashMap<IVec2, Heights>,
    order: VecDeque<IVec2>,
    pending: HashMap<IVec2, mpsc::Receiver<Heights>>,
    cancelled: Arc<AtomicBool>,
}
thread_local! { static CACHE: RefCell<Cache> = RefCell::new(Cache::default()); }

struct Job {
    cfg: WorldSampleConfig,
    site: orbital::Site,
    cancelled: Arc<AtomicBool>,
    reply: mpsc::SyncSender<Heights>,
}

fn lane() -> Option<&'static mpsc::SyncSender<Job>> {
    static LANE: OnceLock<Option<mpsc::SyncSender<Job>>> = OnceLock::new();
    LANE.get_or_init(|| {
        let (send, receive) = mpsc::sync_channel::<Job>(4);
        std::thread::Builder::new().name("orbital-terrain".into()).spawn(move || {
            while let Ok(job) = receive.recv() {
                if job.cancelled.load(Ordering::Relaxed) { continue; }
                let started = std::time::Instant::now();
                let state = orbital::State::default();
                let hold = orbital::hold_height(&job.cfg, &state, job.site);
                if job.cancelled.load(Ordering::Relaxed) { continue; }
                let socket = orbital::Site { z: job.site.z + super::FREIGHT_SOCKET_Z_M, ..job.site };
                let footing = orbital::footing_height(&job.cfg, &state, socket);
                let roots = [-1.0, 1.0].map(|side| orbital::footing_height(&job.cfg, &state,
                    orbital::Site { x: socket.x + side * 6.0, ..socket }));
                if !job.cancelled.load(Ordering::Relaxed)
                    && job.reply.try_send(Heights { hold, footing, roots }).is_ok()
                    && std::env::var_os("VOXEL_GAMEPLAY_TRACE").is_some()
                {
                    eprintln!("[orbital-terrain] ready cell={},{} ms={:.2}",
                        job.site.cell.x, job.site.cell.y, started.elapsed().as_secs_f64() * 1000.0);
                }
            }
        }).ok()?;
        Some(send)
    }).as_ref()
}

pub(super) fn get(cfg: WorldSampleConfig, site: orbital::Site) -> Option<Heights> {
    CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.cfg != Some(cfg) {
            cache.cancelled.store(true, Ordering::Relaxed);
            *cache = Cache { cfg: Some(cfg), ..Cache::default() };
        }
        // Drain completed sites even after the camera has moved away. Otherwise
        // unread replies would accumulate along a long flight or fill the queue.
        let completed: Vec<_> = cache.pending.iter().filter_map(|(&cell, reply)| {
            match reply.try_recv() {
                Ok(heights) => Some((cell, Some(heights))),
                Err(mpsc::TryRecvError::Disconnected) => Some((cell, None)),
                Err(mpsc::TryRecvError::Empty) => None,
            }
        }).collect();
        for (cell, heights) in completed {
            cache.pending.remove(&cell);
            if let Some(heights) = heights {
                if cache.ready.len() >= 64 && let Some(old) = cache.order.pop_front() {
                    cache.ready.remove(&old);
                }
                cache.ready.insert(cell, heights);
                cache.order.push_back(cell);
            }
        }
        if let Some(&heights) = cache.ready.get(&site.cell) { return Some(heights); }
        if cache.pending.contains_key(&site.cell) || cache.pending.len() >= 8 { return None; }
        let lane = lane()?;
        let (reply, receive) = mpsc::sync_channel(1);
        if lane.try_send(Job { cfg, site, cancelled: cache.cancelled.clone(), reply }).is_ok() {
            cache.pending.insert(site.cell, receive);
        }
        None
    })
}
