//! Evolve the tributary network at the scale of whole hills and their divides.
use super::{Face, Grid, Mesh, Projection, SEA, World, WorldSampleConfig, atlas, inflow, move_talus, noise, smooth, transport};
use super::super::{EVOLVED_WATERSHEDS_GENERATOR, drift::history::RockColumn};
use glam::DVec2;
use std::{collections::{HashMap, VecDeque}, sync::{Arc, Mutex, OnceLock}};

const CORE_M: f64 = 2304.0;
const PAD_M: f64 = 768.0;
const BLEND_M: f64 = 384.0;
const CELL_M: f64 = 48.0;
const EPOCHS: usize = 22;
const CACHE_LIMIT: usize = 48;
// Uplift, incision, and contributing area at channel onset.
const PREVIEW_FACTORS: [f64; 3] = [1.5, 1.8, 0.5];

type Key = (Face, i32, i32);
type Slot = Arc<OnceLock<Region>>;

#[derive(Default)]
pub(super) struct Regions(Mutex<Cache>);

#[derive(Default)]
struct Cache {
    regions: HashMap<Key, Slot>,
    insertion: VecDeque<Key>,
}

pub(super) struct Window(Vec<(Key, Slot)>);

struct Region {
    grid: Grid,
    bed: Vec<f32>,
    soil: Vec<f32>,
    streams: Vec<Stream>,
}

struct Inputs {
    debris: bool,
    mesh: Mesh,
    base: Vec<f64>,
    bed: Vec<f64>,
    soil: Vec<f64>,
    rock: Vec<Option<RockColumn>>,
    fixed: Vec<bool>,
    outlets: Vec<bool>,
    wet: Vec<bool>,
    rain: Vec<f64>,
    local_runoff: Vec<f64>,
    uplift: Vec<f64>,
    erodibility: Vec<f64>,
    momentum: Option<Vec<DVec2>>,
}

struct Stream {
    source: DVec2,
    receiver: DVec2,
    discharge: f64,
    speed: f64,
}

impl Regions {
    pub(super) fn window(&self, world: &World, cfg: &WorldSampleConfig, face: Face, grid: Grid) -> Window {
        let origin = DVec2::from_array(grid.min_m);
        let margin = DVec2::splat(BLEND_M + CELL_M);
        let low = origin - margin;
        let high = origin + DVec2::splat(grid.n as f64 * grid.cell_m) + margin;
        let first = (low / CORE_M).floor().as_ivec2();
        let last = (high / CORE_M).floor().as_ivec2();
        let mut regions = Vec::new();
        for z in first.y..=last.y {
            for x in first.x..=last.x {
                let key = (face, x, z);
                let slot = {
                    let mut cache = self.0.lock().unwrap_or_else(|error| error.into_inner());
                    if let Some(slot) = cache.regions.get(&key).cloned() {
                        if let Some(i) = cache.insertion.iter().position(|&k| k == key) { cache.insertion.remove(i); }
                        cache.insertion.push_back(key);
                        slot
                    } else {
                        if cache.regions.len() >= CACHE_LIMIT && let Some(old) = cache.insertion.pop_front() {
                            cache.regions.remove(&old);
                        }
                        let slot = Slot::default();
                        cache.regions.insert(key, Arc::clone(&slot));
                        cache.insertion.push_back(key);
                        slot
                    }
                };
                slot.get_or_init(|| Region::solve(world, cfg, key));
                regions.push((key, slot));
            }
        }
        Window(regions)
    }
}

fn weight(index: i32, metres: f64) -> f64 {
    let distance = metres - (f64::from(index) + 0.5) * CORE_M;
    1.0 - smooth(CORE_M * 0.5 - BLEND_M, CORE_M * 0.5 + BLEND_M, distance.abs())
}

/// The perimeter pins elevation, but only downhill edges discharge runoff.
/// A flat closed patch still needs one spillway for priority-flood routing.
pub(super) fn open_perimeter(grid: Grid, bed: &[f64], soil: &[f64], outlets: &mut [bool]) {
    let n = grid.n;
    let surface = |i: usize| bed[i] + soil[i];
    let mut spillway = 0;
    for (i, outlet) in outlets.iter_mut().enumerate() {
        if !grid.on_border(i) { continue; }
        let (x, z) = (i % n, i / n);
        let h = surface(i);
        *outlet |= (x == 0 && surface(i + 1) > h + 0.03)
            || (x + 1 == n && surface(i - 1) > h + 0.03)
            || (z == 0 && surface(i + n) > h + 0.03)
            || (z + 1 == n && surface(i - n) > h + 0.03);
        if h < surface(spillway) { spillway = i; }
    }
    if !outlets.iter().any(|&outlet| outlet) { outlets[spillway] = true; }
}

impl Window {
    pub(super) fn surface(&self, p: DVec2) -> (f64, f64) {
        let mut bed = 0.0;
        let mut soil = 0.0;
        for &((_, x, z), ref slot) in &self.0 {
            let share = weight(x, p.x) * weight(z, p.y);
            if share == 0.0 { continue; }
            let region = slot.get().expect("solved regional terrain");
            bed += share * region.grid.bilinear(&region.bed, p.x, p.y);
            soil += share * region.grid.bilinear(&region.soil, p.x, p.y);
        }
        (bed, soil)
    }

    pub(super) fn supply(&self, mesh: &Mesh, fixed: &[bool], rain: &mut [f64], momentum: Option<&mut [DVec2]>) {
        let mut inlets = inflow::Inlets::new(mesh, fixed, rain, momentum);
        for &((_, x, z), ref slot) in &self.0 {
            let region = slot.get().expect("solved regional drainage");
            for stream in &region.streams {
                let midpoint = (stream.source + stream.receiver) * 0.5;
                let share = weight(x, midpoint.x) * weight(z, midpoint.y);
                if share > 0.0 {
                    inlets.add(stream.source, stream.receiver, stream.discharge * share, stream.speed);
                }
            }
        }
    }
}

impl Region {
    fn solve(world: &World, cfg: &WorldSampleConfig, (face, x, z): Key) -> Self {
        let inputs = Inputs::sample(world, cfg, (face, x, z));
        let factors = if cfg.generator_version == EVOLVED_WATERSHEDS_GENERATOR {
            PREVIEW_FACTORS
        } else {
            [1.0; 3]
        };
        let (bed, soil) = inputs.evolve(factors);
        let mesh = &inputs.mesh;
        let surface: Vec<_> = bed.iter().zip(&soil).map(|(bed, soil)| bed + soil).collect();
        let flow = inputs.route(&surface);
        let mut streams = Vec::new();
        for (i, links) in flow.links.iter().enumerate() {
            let share: f64 = links.as_slice().iter().map(|link| f64::from(link.share)).sum();
            for link in links.as_slice() {
                let discharge = flow.q[i] * f64::from(link.share) / share;
                // The fine grid computes its own local rainfall. Import only
                // established channels that carry runoff across its boundary.
                if discharge >= 12_000.0 {
                    streams.push(Stream { source: mesh.points[i], receiver: mesh.points[link.to as usize],
                        discharge, speed: flow.speed.get(i).copied().unwrap_or(0.0) });
                }
            }
        }
        Self { grid: mesh.grid, bed: mesh.regularize(|i| bed[i] - inputs.base[i]), soil: mesh.regularize(|i| soil[i]), streams }
    }
}

impl Inputs {
    fn sample(world: &World, cfg: &WorldSampleConfig, (face, x, z): Key) -> Self {
        let grid = Grid {
            n: ((CORE_M + 2.0 * PAD_M) / CELL_M) as usize,
            cell_m: CELL_M,
            min_m: [f64::from(x) * CORE_M - PAD_M, f64::from(z) * CORE_M - PAD_M],
        };
        let mesh = Mesh::with_alignment(grid, true);
        let len = grid.len();
        let mut base = Vec::with_capacity(len);
        let mut bed = Vec::with_capacity(len);
        let mut soil = Vec::with_capacity(len);
        let mut rock = Vec::with_capacity(len);
        let mut fixed = Vec::with_capacity(len);
        let mut outlets = Vec::with_capacity(len);
        let mut wet = Vec::with_capacity(len);
        let mut rain = Vec::with_capacity(len);
        let mut uplift = Vec::with_capacity(len);
        let mut erodibility = Vec::with_capacity(len);
        for (i, p) in mesh.points.iter().enumerate() {
            let dir = Projection::Tangent.to_sphere(face, p.x / atlas::HALF_M, p.y / atlas::HALF_M);
            let volcano = world.volcano_shape(dir);
            let ground = world.ground_base(cfg, dir, 8.0, volcano);
            let h = f64::from(ground.top_m);
            let cover = world.mesh.sample(&world.sediment_cover, dir).clamp(0.0, 12.0);
            let outlet = ground.water_m.is_some() || h < SEA + 3.0;
            let anchored = outlet || grid.on_border(i)
                || volcano.active_caldera || world.edifice_lift(dir) > 12.0;
            let tectonics = world.mesh.sample(&world.collision, dir).min(1.0);
            let fault = world.mesh.sample(&world.fault_damage, dir);
            let body = dir * atlas::RADIUS_M;
            let warped = noise(cfg.seed, body / 1160.0, 281_013);
            let block = smooth(0.22, 0.78, warped);
            // Raised blocks keep headwater slopes active as erosion lowers the channels.
            let rate = 3.4 * block * (0.55 + 0.45 * tectonics) * smooth(SEA + 14.0, SEA + 100.0, h);
            let fracture = noise(cfg.seed, body / 240.0, 281_019);
            base.push(h);
            bed.push(h - cover);
            soil.push(cover);
            rock.push(world.history_strata(cfg, dir));
            fixed.push(anchored);
            outlets.push(outlet);
            wet.push(ground.water_m.is_some() || h < SEA);
            rain.push(0.23 + 0.77 * f64::from(ground.moisture));
            uplift.push(if anchored { 0.0 } else { rate });
            erodibility.push(0.65 + 0.52 * fracture + 0.58 * fault);
        }
        open_perimeter(grid, &bed, &soil, &mut outlets);
        let local_runoff: Vec<_> = rain.iter().zip(&mesh.area).map(|(rain, area)| rain * area).collect();
        let moving = super::super::branching_watersheds(cfg.generator_version);
        let mut momentum = moving.then(|| vec![DVec2::ZERO; len]);
        inflow::add(world, face, &mesh, &fixed, &mut rain, momentum.as_deref_mut());
        Self { debris: super::super::landscape_cycles(cfg.generator_version), mesh, base, bed, soil, rock, fixed, outlets, wet, rain, local_runoff, uplift, erodibility, momentum }
    }

    fn route(&self, surface: &[f64]) -> super::super::review::catchment::structural::mesh::Routing {
        if let Some(momentum) = &self.momentum { self.mesh.route_momentum(surface, &self.outlets, &self.rain, momentum) }
        else { self.mesh.route(surface, &self.outlets, &self.rain) }
    }

    fn evolve(&self, [uplift_scale, incision_scale, drainage_area_scale]: [f64; 3]) -> (Vec<f64>, Vec<f64>) {
        let Self { mesh, base, rock, fixed, local_runoff, uplift, erodibility, .. } = self;
        let mut bed = self.bed.clone();
        let mut soil = self.soil.clone();
        let len = bed.len();
        let mut surface = vec![0.0; len];
        let mut carved = vec![0.0; len];
        let mut mass = vec![0.0; len];
        let mut repose = vec![0.0; len];
        let mut resistance = vec![0.0; len];
        for epoch in 0..EPOCHS {
            for i in 0..len {
                bed[i] += uplift_scale * uplift[i] * smooth(0.0, 50.0, 504.0 - bed[i]);
                surface[i] = bed[i] + soil[i];
                resistance[i] = rock[i].map_or(1.0, |column| column.resistance(bed[i]));
                repose[i] = (0.86 * resistance[i] / erodibility[i]).clamp(0.38, 1.85);
            }
            let flow = self.route(&surface);
            // Solve stream power from base level uphill. Each tributary sees
            // the already-incised elevation of its receivers in this epoch.
            for &node in &flow.order {
                let i = node as usize;
                if fixed[i] { carved[i] = 0.0; continue; }
                let discharge = flow.q[i];
                let contributing_area = (discharge - local_runoff[i]) / drainage_area_scale;
                let concentration = smooth(6_000.0, 42_000.0, contributing_area);
                let q = (discharge / 16_000.0).sqrt().min(6.0);
                let threshold = 0.045 / q.max(0.04);
                let power = 54.0 * incision_scale * q * erodibility[i] / resistance[i];
                let mut receiver_pull = 0.0;
                let mut coefficient = 1.0;
                for link in flow.links[i].as_slice() {
                    let j = link.to as usize;
                    let run = f64::from(link.distance);
                    let target = surface[j] - carved[j] + threshold * run;
                    if target < surface[i] {
                        let strength = power * f64::from(link.share) / run;
                        receiver_pull += strength * (surface[i] - target);
                        coefficient += strength;
                    }
                }
                // Only the occupied part of a regional cell contributes to
                // its mean erosion. Fine tiles resolve the narrower beds.
                let width = (5.0 + 4.0 * q).min(28.0);
                let cut = (receiver_pull / coefficient).min(6.0) * width / CELL_M * concentration;
                let floor = (base[i] - 76.0).max(SEA + 0.5);
                carved[i] = cut.min((surface[i] - floor).max(0.0));
            }

            // Carry the displaced bed and loose cover downstream in the same
            // network. Capacity falls on gentle reaches, so they retain fill.
            mass.fill(0.0);
            for &node in flow.order.iter().rev() {
                let i = node as usize;
                let routes = flow.links[i].as_slice();
                if fixed[i] {
                    transport::forward(routes, mass[i], &mut mass);
                    continue;
                }
                let soil_cut = carved[i].min(soil[i]);
                soil[i] -= soil_cut;
                bed[i] -= carved[i] - soil_cut;
                let load = mass[i] + carved[i] * mesh.area[i];
                let capacity = mesh.area[i] * 7.0 * (flow.q[i] / 12_000.0).sqrt() * flow.grade[i];
                let deposit = ((load - capacity).max(0.0) * 0.32).min(1.1 * mesh.area[i]);
                soil[i] += deposit / mesh.area[i];
                transport::forward(routes, load - deposit, &mut mass);
            }
            move_talus(mesh, fixed, &repose, &mut bed, &mut soil, &mut surface, &mut mass);
            if self.debris && epoch % 4 == 3 {
                // Deposits change the next epoch's river instead of drawing a
                // fan over it. Re-route after the incision and slope movement.
                for i in 0..len { surface[i] = bed[i] + soil[i]; }
                let debris_flow = self.route(&surface);
                super::debris::advance(mesh, &debris_flow, fixed, &self.rain, &resistance,
                    &mut bed, &mut soil, &mut mass);
            }
        }
        (bed, soil)
    }
}

pub(in crate::planetary) fn sweep(world: &World, cfg: &WorldSampleConfig, dir: glam::DVec3,
    parameters: &[[f64; 3]]) -> super::super::review::WatershedSweep
{
    let (face, s, t) = Projection::Tangent.from_sphere(dir);
    let region = |coord: f64| (coord * atlas::HALF_M / CORE_M).floor() as i32;
    let inputs = Inputs::sample(world, cfg, (face, region(s), region(t)));
    let side = (CORE_M / CELL_M) as usize;
    let pad = (PAD_M / CELL_M) as usize;
    let grid = inputs.mesh.grid;
    let crop = |data: &[f32]| (pad..pad + side).flat_map(|z| {
        data[z * grid.n + pad..z * grid.n + pad + side].iter().copied()
    }).collect();
    let wet = crop(&inputs.mesh.regularize(|i| if inputs.wet[i] { 1.0 } else { 0.0 }));
    let heights = parameters.iter().map(|&params| {
        let (bed, soil) = inputs.evolve(params);
        crop(&inputs.mesh.regularize(|i| bed[i] + soil[i]))
    }).collect();
    super::super::review::WatershedSweep { side, cell_m: CELL_M, heights, wet }
}
