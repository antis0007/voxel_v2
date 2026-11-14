//! Cooperative cloud raster worker; slices yield to the renderer and observe cancellation.
use super::shell::{Light, ShellConfig, anchorage_sites, night_lights_from, shell_texel};
use super::shell_sim::{ShellField, ShellWeather};
use crate::world::WeatherClock;
use engine_core::work::{WorkCursor, WorkSlice};
use engine_gpu::image::{RgbaMip, rgba_mips};
use glam::{DVec3, IVec2};
use std::collections::VecDeque;
use std::sync::{Arc, atomic::{AtomicBool, AtomicU64, Ordering}, mpsc::{Receiver, Sender, channel}};
use std::time::{Duration, Instant};

pub struct ShellBake {
    pub face: u8,
    pub resolution: u32,
    pub coverage: Vec<u8>,
    pub mips: Vec<RgbaMip>,
    pub drift_m: [f32; 2],
    pub time_s: f32,
}

enum Job {
    Face(ShellConfig, f32, u8),
    Lights { seed: u32, clusters: Vec<(DVec3, u32)>, eye_body: DVec3 },
}
enum Done { Face(ShellBake), Lights(Vec<Light>) }

struct FaceWork {
    cfg: ShellConfig,
    clock: WeatherClock,
    face: u8,
    cursor: WorkCursor,
    coverage: Vec<u8>,
    flow: Vec<[u8; 3]>,
    time: f32,
    weather: Option<Arc<ShellField>>,
    field_selected: bool,
    hint: usize,
}
impl FaceWork {
    fn new(cfg: ShellConfig, time: f32, face: u8, planet: Option<engine_voxel::WorldSampleConfig>) -> Self {
        let count = (cfg.resolution * cfg.resolution) as usize;
        let clock = WeatherClock::new(&cfg.controls, time, cfg.days);
        let clock = planet.map_or(clock, |cfg| clock.with_planet(cfg));
        Self { cfg, clock, face,
            cursor: WorkCursor::new(count), coverage: Vec::with_capacity(count), flow: Vec::with_capacity(count), time,
            weather: None, field_selected: false, hint: 0 }
    }
    fn advance(&mut self, stop: &AtomicBool) -> bool {
        let slice = WorkSlice::new(Duration::from_millis(1));
        loop {
            for index in self.cursor.take(32) {
                let x = index as u32 % self.cfg.resolution;
                let y = index as u32 / self.cfg.resolution;
                let value = match &self.weather {
                    Some(field) => super::shell::shell_weather_texel(&self.cfg,self.clock,
                        self.face,x,y,field,&mut self.hint),
                    None => shell_texel(&self.cfg,self.clock,self.face,x,y),
                };
                self.coverage.push(value);
                let dir = super::chart::to_body(DVec3::new(
                    f64::from(self.face) * super::chart::PITCH_M + super::shell::texel_chart_m(x, self.cfg.resolution),
                    0.0, super::shell::texel_chart_m(y, self.cfg.resolution))).normalize();
                self.flow.push(self.weather.as_ref().map_or([128; 3], |field| field.flow_at(dir)));
            }
            if self.cursor.complete() { return true; }
            if stop.load(Ordering::Acquire) || slice.exhausted() { return false; }
        }
    }
    fn finish(self) -> ShellBake {
        let column_opacity = super::shell::column_opacity(&self.cfg.controls);
        ShellBake { face: self.face, resolution: self.cfg.resolution,
            // Filter opacity rather than thresholding an averaged density: distant
            // clouds retain their fractional coverage instead of disappearing.
            mips: rgba_mips(self.cfg.resolution, self.coverage.iter().zip(&self.flow).flat_map(|(c, flow)| {
                [(f32::from(*c) * column_opacity).round() as u8, flow[0], flow[1], flow[2]]
            }).collect()),
            time_s: self.time,
            coverage: self.coverage, drift_m: self.clock.drift_m.to_array() }
    }
}
enum Pending { Face(FaceWork), Lights(Job) }
fn enqueue(queue: &mut VecDeque<Pending>, job: Job, planet: Option<engine_voxel::WorldSampleConfig>) {
    match job {
        Job::Face(cfg, time, face) => {
            queue.retain(|p| !matches!(p, Pending::Face(work) if work.face == face));
            queue.push_back(Pending::Face(FaceWork::new(cfg, time, face, planet)));
        }
        job @ Job::Lights { .. } => {
            queue.retain(|p| !matches!(p, Pending::Lights(_)));
            queue.push_back(Pending::Lights(job));
        }
    }
}

pub struct ShellWorker {
    requests: Sender<Job>,
    results: Receiver<Done>,
    alive: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    bake_nanos: Arc<AtomicU64>,
}
impl ShellWorker {
    #[cfg(test)]
    pub fn spawn() -> Option<ShellWorker> {
        Self::spawn_for(None)
    }

    pub fn spawn_for(planet: Option<engine_voxel::WorldSampleConfig>) -> Option<ShellWorker> {
        let planet = planet.filter(engine_worldgen::planetary::active);
        let (requests, request_rx) = channel::<Job>();
        let (result_tx, results) = channel();
        let alive = Arc::new(AtomicBool::new(true));
        let stop = Arc::new(AtomicBool::new(false));
        let bake_nanos = Arc::new(AtomicU64::new(0));
        let (flag, cancelled, nanos) = (alive.clone(), stop.clone(), bake_nanos.clone());
        std::thread::Builder::new().name("globe-shell".into()).spawn(move || {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut sites: Option<(u32, Vec<(DVec3, IVec2)>)> = None;
                let mut weather: Option<ShellWeather> = None;
                let mut pending = VecDeque::new();
                while !cancelled.load(Ordering::Acquire) {
                    if pending.is_empty() {
                        match request_rx.recv_timeout(Duration::from_millis(20)) {
                            Ok(job) => enqueue(&mut pending, job, planet),
                            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                            Err(_) => break,
                        }
                    }
                    for job in request_rx.try_iter() { enqueue(&mut pending, job, planet); }
                    let start = Instant::now();
                    let done = match pending.pop_front() {
                        Some(Pending::Face(mut work)) => {
                            if weather.as_ref().is_none_or(|w| w.seed != work.cfg.weather_seed) {
                                weather = ShellWeather::new(work.cfg.weather_seed, planet);
                            }
                            let field = weather.as_mut().and_then(|w| w.prepare(work.cfg.days));
                            if !work.field_selected {
                                // Show planetary fronts during spin-up, with one
                                // density field for the entire face bake.
                                work.weather = field.or_else(|| {
                                    weather.as_ref().map(|w| w.preview(work.cfg.days))
                                });
                                work.field_selected = true;
                            }
                            if work.advance(&cancelled) { Some(Done::Face(work.finish())) }
                            else { pending.push_back(Pending::Face(work)); None }
                        }
                        Some(Pending::Lights(Job::Lights { seed, clusters, eye_body })) => {
                            let list = match &sites {
                                Some((s, list)) if *s == seed => list,
                                _ => &sites.insert((seed, anchorage_sites(seed))).1,
                            };
                            Some(Done::Lights(night_lights_from(seed, list, &clusters, eye_body)))
                        }
                        _ => None,
                    };
                    nanos.fetch_add(start.elapsed().as_nanos() as u64, Ordering::Relaxed);
                    if let Some(done) = done {
                        if result_tx.send(done).is_err() { break; }
                    }
                    // Limit sustained CPU use as well as the length of each turn.
                    std::thread::sleep(Duration::from_millis(1));
                }
            }));
            flag.store(false, Ordering::Release);
        }).ok()?;
        Some(ShellWorker { requests, results, alive, stop, bake_nanos })
    }
    pub fn request(&self, cfg: ShellConfig, time_s: f32, face: u8) {
        let _ = self.requests.send(Job::Face(cfg, time_s, face));
    }
    pub fn request_lights(&self, seed: u32, clusters: Vec<(DVec3, u32)>, eye_body: DVec3) {
        let _ = self.requests.send(Job::Lights { seed, clusters, eye_body });
    }
    pub fn alive(&self) -> bool { self.alive.load(Ordering::Acquire) }
    pub fn take_bake_nanos(&self) -> u64 { self.bake_nanos.swap(0, Ordering::Relaxed) }
    pub fn drain_into(&self, faces: &mut VecDeque<ShellBake>) -> Option<Vec<Light>> {
        let mut lights = None;
        for done in self.results.try_iter() {
            match done { Done::Face(bake) => faces.push_back(bake), Done::Lights(list) => lights = Some(list) }
        }
        lights
    }
}
impl Drop for ShellWorker {
    fn drop(&mut self) { self.stop.store(true, Ordering::Release); }
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
        let cfg = ShellConfig { resolution: 16, controls: Default::default(), weather_seed: 1, days: 0.0 };
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
