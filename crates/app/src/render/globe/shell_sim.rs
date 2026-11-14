//! Claude's recovered pressure/moisture solver, advanced cooperatively off-thread.
//!
//! Solver time is a pure function of the synced weather day, never of wall
//! time: the field published for `(seed, days)` is the same on every machine.
use engine_core::planet_frame::PlanetAxis;
use engine_worldgen::sphere::{CloudCover, CloudSky, SphereGrid, Surface, WeatherParams, WeatherSim};
use glam::DVec3;
use std::sync::Arc;
use std::time::Instant;

use crate::world::{WEATHER_SPIN_UP as SPIN_UP, WEATHER_UNITS_PER_DAY as UNITS_PER_DAY, weather_model_time as model_time};
const REFRESH_UNITS: f64 = 1.0;
/// The solver is autonomous, so a run restarts from rest at each epoch and
/// spins up again; this bounds catch-up after a long skip or a late load.
const EPOCH_UNITS: f64 = 2.0 * UNITS_PER_DAY;
/// Solver steps per `prepare` call, the worker's per-turn work budget.
const STEPS_PER_CALL: u64 = 24;

/// The snapshot target for `days`: refresh-quantised model time and its epoch.
fn snapshot_target(days: f64) -> (f64, f64) {
    let target = (model_time(days) / REFRESH_UNITS).floor() * REFRESH_UNITS;
    (target, (target / EPOCH_UNITS).floor() * EPOCH_UNITS)
}

/// Fixed solver steps from rest that reach `target` within `epoch`.
fn solver_steps(target: f64, epoch: f64) -> u64 {
    ((SPIN_UP + target - epoch) / engine_worldgen::sphere::WEATHER_DT).round() as u64
}

pub(super) struct ShellField {
    time: f64,
    cover: Option<CloudCover>,
    sky: CloudSky,
}
impl ShellField {
    /// Body-space angular flow; the same systems move both density and fine detail.
    pub fn flow_at(&self, dir: DVec3) -> [u8; 3] {
        self.sky.advection(dir).to_array().map(|v| (128.0 + v.clamp(-0.16, 0.16) * (127.0 / 0.16)).round() as u8)
    }
    pub fn cover_at(&self, dir: DVec3, hint: &mut usize) -> f32 {
        let circulation = self.cover.as_ref()
            .map_or(0.0, |cover| cover.sample(dir, hint) - cover.mean());
        (self.sky.density(dir) + circulation * 0.25).clamp(0.0,1.0)
    }
}

pub(super) struct ShellWeather {
    grid: SphereGrid,
    surface: Surface,
    params: WeatherParams,
    sim: WeatherSim,
    /// Epoch the running solver started from rest at.
    epoch: f64,
    steps: u64,
    field: Option<Arc<ShellField>>,
    pub seed: u32,
    axis: PlanetAxis,
    year_fraction: f64,
}
impl ShellWeather {
    pub fn new(seed: u32, planet: Option<engine_voxel::WorldSampleConfig>) -> Option<Self> {
        let grid = SphereGrid::new(64, 1.0)?;
        let params = crate::world::weather_params(seed, planet);
        let surface = planet.as_ref().map_or_else(|| Surface::ocean(grid.len()),
            |cfg| engine_worldgen::planetary::weather_surface(cfg, &grid));
        let sim = WeatherSim::new(&grid, surface.clone(), params)?;
        Some(Self { grid, surface, params, sim, epoch: 0.0, steps: 0, field: None,
            seed, axis: params.axis, year_fraction: params.year_fraction })
    }

    pub fn preview(&self, days: f64) -> Arc<ShellField> {
        let time = SPIN_UP + model_time(days);
        Arc::new(ShellField {
            time,
            cover: None,
            sky: CloudSky::at(self.seed, self.axis, self.year_fraction, time),
        })
    }

    /// A face keeps its immutable snapshot even if a newer bake advances time.
    /// Each call takes at most `STEPS_PER_CALL` solver steps; `None` until the
    /// snapshot for `days` is reached, so the result depends only on `(seed, days)`.
    pub fn prepare(&mut self, days: f64) -> Option<Arc<ShellField>> {
        let (target, epoch) = snapshot_target(days);
        let want = solver_steps(target, epoch);
        if self.field.as_ref().is_some_and(|f| f.time == SPIN_UP + target) {
            return self.field.clone();
        }
        if epoch != self.epoch || self.steps > want {
            self.sim = WeatherSim::new(&self.grid, self.surface.clone(), self.params)?;
            self.epoch = epoch;
            self.steps = 0;
        }
        let sim = &mut self.sim;
        let mut budget = STEPS_PER_CALL;
        while self.steps < want {
            if budget == 0 { return None; }
            sim.step();
            self.steps += 1;
            budget -= 1;
        }
        let time = SPIN_UP + target;
        let cover = sim.cover_snapshot();
        let sky = CloudSky::at(self.seed, self.axis, self.year_fraction, time);
        if self.field.is_none() {
            let (low, high) = sim.pressure().iter().fold((f64::INFINITY,f64::NEG_INFINITY),
                |(lo,hi), &p| (lo.min(p),hi.max(p)));
            let mean = cover.mean();
            println!("[orbital-weather] pressure_solver=true cells={} model_time={:.2} anomaly_min={low:.4} anomaly_max={high:.4} mean_cover={mean:.4}",sim.len(),time);
        }
        let field = Arc::new(ShellField { time, cover: Some(cover), sky });
        let published = field.clone();
        // The march's weather rows are re-sampled in a rolling sweep. A cover
        // that jumped to each new snapshot left a seam sweeping across every
        // clipmap level (strobing cloud-shadow bands), so fronts fade from the
        // previous snapshot over one refresh period instead.
        let previous = self.field.clone();
        // Presentation-only cross-fade; the snapshots themselves are tick-pure.
        let fade_s = (REFRESH_UNITS / 0.04) as f32;
        let born = Instant::now();
        crate::world::publish_fronts(self.seed, Arc::new(move |dir| {
            thread_local! { static HINT: std::cell::Cell<[usize; 2]> = const { std::cell::Cell::new([0; 2]) }; }
            HINT.with(|hint| {
                let mut h = hint.get();
                let new = published.cover_at(dir, &mut h[0]);
                let cover = match &previous {
                    Some(old) => {
                        let t = (born.elapsed().as_secs_f32() / fade_s).min(1.0);
                        let s = t * t * (3.0 - 2.0 * t);
                        old.cover_at(dir, &mut h[1]) * (1.0 - s) + new * s
                    }
                    None => new,
                };
                hint.set(h);
                cover
            })
        }));
        self.field = Some(field);
        self.field.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cover_hash(weather: &mut ShellWeather, days: f64) -> u64 {
        let field = loop {
            if let Some(field) = weather.prepare(days) { break field; }
        };
        let mut hint = 0;
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        for i in 0..512u32 {
            let (u, v) = (f64::from(i % 32) / 31.0, f64::from(i / 32) / 15.0);
            let (theta, z) = (u * std::f64::consts::TAU, v * 2.0 - 1.0);
            let r = (1.0 - z * z).max(0.0).sqrt();
            let c = field.cover_at(DVec3::new(r * theta.cos(), r * theta.sin(), z), &mut hint);
            hash = (hash ^ u64::from(c.to_bits())).wrapping_mul(0x0100_0000_01b3);
        }
        hash
    }

    #[test]
    fn cover_is_a_pure_function_of_seed_and_day() {
        let days = 0.45;
        let mut direct = ShellWeather::new(7, None).expect("weather");
        let mut stepped = ShellWeather::new(7, None).expect("weather");
        // A different request history must land on the same snapshot.
        let _ = cover_hash(&mut stepped, 0.2);
        let _ = cover_hash(&mut stepped, 0.9);
        assert_eq!(cover_hash(&mut direct, days), cover_hash(&mut stepped, days));
        let mut other = ShellWeather::new(8, None).expect("weather");
        assert_ne!(cover_hash(&mut direct, days), cover_hash(&mut other, days));
    }
}
