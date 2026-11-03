//! Background thread for shell face bakes and the night-light walk; separate from the tile
//! worker so a 262,144-tap bake never preempts tile bakes or their resumption state.

use super::shell::{Light, ShellConfig, anchorage_sites, bake_shell, night_lights_from};
use glam::{DVec3, IVec2};
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};

pub struct ShellBake {
    pub face: u8,
    pub resolution: u32,
    pub coverage: Vec<u8>,
}

enum Job {
    Face(ShellConfig, f32, u8),
    /// Factory clusters come from the render thread; the chart-wide site walk happens here.
    Lights { seed: u32, clusters: Vec<(DVec3, u32)>, eye_body: DVec3 },
}

enum Done {
    Face(ShellBake),
    Lights(Vec<Light>),
}

pub struct ShellWorker {
    requests: Sender<Job>,
    results: Receiver<Done>,
    alive: Arc<AtomicBool>,
}

impl ShellWorker {
    /// `None` when the thread cannot be spawned; the shell then stays clear.
    pub fn spawn() -> Option<ShellWorker> {
        let (requests, request_rx) = channel::<Job>();
        let (result_tx, results) = channel();
        let alive = Arc::new(AtomicBool::new(true));
        let flag = alive.clone();
        std::thread::Builder::new()
            .name("globe-shell".into())
            .spawn(move || {
                // A panicking bake leaves the shell at its last faces; the driver sees `alive` drop.
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    // Sites depend only on the seed, so one walk serves every refresh.
                    let mut sites: Option<(u32, Vec<(DVec3, IVec2)>)> = None;
                    while let Ok(job) = request_rx.recv() {
                        let done = match job {
                            Job::Face(cfg, time_s, face) => {
                                Done::Face(ShellBake { face, resolution: cfg.resolution, coverage: bake_shell(&cfg, time_s, face) })
                            }
                            Job::Lights { seed, clusters, eye_body } => {
                                let list = match &sites {
                                    Some((s, list)) if *s == seed => list,
                                    _ => &sites.insert((seed, anchorage_sites(seed))).1,
                                };
                                Done::Lights(night_lights_from(seed, list, &clusters, eye_body))
                            }
                        };
                        if result_tx.send(done).is_err() {
                            return;
                        }
                    }
                }));
                flag.store(false, Ordering::Release);
            })
            .ok()?;
        Some(ShellWorker { requests, results, alive })
    }

    pub fn request(&self, cfg: ShellConfig, time_s: f32, face: u8) {
        let _ = self.requests.send(Job::Face(cfg, time_s, face));
    }

    pub fn request_lights(&self, seed: u32, clusters: Vec<(DVec3, u32)>, eye_body: DVec3) {
        let _ = self.requests.send(Job::Lights { seed, clusters, eye_body });
    }

    pub fn alive(&self) -> bool {
        self.alive.load(Ordering::Acquire)
    }

    /// Appends finished faces to `faces` and returns the newest light list, if one arrived.
    pub fn drain_into(&self, faces: &mut VecDeque<ShellBake>) -> Option<Vec<Light>> {
        let mut lights = None;
        for done in self.results.try_iter() {
            match done {
                Done::Face(bake) => faces.push_back(bake),
                Done::Lights(list) => lights = Some(list),
            }
        }
        lights
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wait<T>(mut poll: impl FnMut() -> Option<T>) -> Option<T> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while std::time::Instant::now() < deadline {
            if let Some(v) = poll() {
                return Some(v);
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        None
    }

    #[test]
    fn a_requested_face_comes_back_at_its_resolution() {
        let Some(w) = ShellWorker::spawn() else { panic!("spawn failed") };
        let cfg = ShellConfig { resolution: 16, controls: Default::default(), weather_seed: 1 };
        w.request(cfg, 0.0, 3);
        let mut got = VecDeque::new();
        assert!(wait(|| { w.drain_into(&mut got); (!got.is_empty()).then_some(()) }).is_some());
        assert_eq!(got.len(), 1);
        assert_eq!((got[0].face, got[0].coverage.len()), (3, 256));
    }

    #[test]
    fn lights_come_back_from_the_worker_thread() {
        let Some(w) = ShellWorker::spawn() else { panic!("spawn failed") };
        let eye = DVec3::new(0.0, 0.0, 7.0e6);
        w.request_lights(74, vec![(DVec3::new(0.0, 0.0, 6.4e6), 64)], eye);
        let mut faces = VecDeque::new();
        let lights = wait(|| w.drain_into(&mut faces)).expect("lights");
        let sites = anchorage_sites(74);
        assert_eq!(lights, night_lights_from(74, &sites, &[(DVec3::new(0.0, 0.0, 6.4e6), 64)], eye));
        assert!(!lights.is_empty());
    }
}
