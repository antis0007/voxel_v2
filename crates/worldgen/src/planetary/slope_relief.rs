//! Resolve tributaries below the planet mesh without rerouting the parent rivers.
//! Refine rock and talus together as the grid spacing halves.
use super::{SEA, World, WorldSampleConfig, geo, noise, smooth};
use super::review::catchment::structural::mesh::Mesh;
use crate::geo::grid::Grid;
use engine_core::cube_sphere::{Face, Projection, atlas};
use glam::{DVec2, DVec3};
use std::{cell::RefCell, collections::{HashMap, VecDeque}, sync::{Arc, Mutex, OnceLock}};
mod gradient;
pub(super) mod transport;
mod inflow;
pub(super) mod watershed;
mod debris;
mod persistence;
pub use persistence::ReliefCacheStats;

pub(super) fn configure_cache(directory: std::path::PathBuf) -> bool {
    persistence::configure(directory)
}
pub(super) fn cache_stats() -> ReliefCacheStats { persistence::statistics() }

const CORE_M: f64 = 448.0;
const CELL_M: f64 = 8.0;
const BLEND_M: f64 = 48.0;
const PAD_M: f64 = 128.0;
const CACHE_LIMIT: usize = 192;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Key { face: u8, x: i32, z: i32 }

struct Tile {
    grid: Grid,
    delta: Vec<f32>,
    talus: Vec<f32>,
    base: Option<Vec<f32>>,
}

#[derive(Default)]
struct Levels {
    coarse: OnceLock<Stage>,
    cover: OnceLock<Tile>,
    detail: OnceLock<Tile>,
    rift_detail: OnceLock<Tile>,
}

type Slot = Arc<Levels>;
type HotKey = (u32, u64, u64, u32, bool, Key);

thread_local! {
    static HOT: RefCell<VecDeque<(HotKey, Slot)>> = const { RefCell::new(VecDeque::new()) };
}

#[derive(Default)]
struct Cache {
    tiles: HashMap<Key, Slot>,
    insertion: VecDeque<Key>,
}

#[derive(Default)]
pub(super) struct Tiles {
    local: Mutex<Cache>,
    distant: Mutex<Cache>,
    regions: watershed::Regions,
    distant_regions: watershed::Regions,
    persistence: Option<persistence::Context>,
}

impl Tiles {
    // Called only on the completed local World immediately before Arc publication.
    pub(super) fn attach_cache(&mut self, cfg: &WorldSampleConfig, shelf: bool, checksum: u64) {
        self.persistence = persistence::sealed_context(cfg, shelf, checksum);
    }

    fn levels(&self, world: &World, cfg: &WorldSampleConfig, key: Key, distant: bool) -> Slot {
        let hot_key = (cfg.seed, cfg.planet_fingerprint(), world.checksum, cfg.generator_version, distant, key);
        if let Some(tile) = HOT.with(|hot| hot.borrow().iter().find(|(k, _)| *k == hot_key)
            .map(|(_, tile)| Arc::clone(tile)))
        {
            return tile;
        }
        let slot = {
            // A whole-globe survey visits many coarse regions. Its bounded
            // cache must not evict the expensive detail around the player.
            let lane = if distant { &self.distant } else { &self.local };
            let mut cache = lane.lock().unwrap_or_else(|error| error.into_inner());
            if let Some(slot) = cache.tiles.get(&key).cloned() {
                if let Some(i) = cache.insertion.iter().position(|&k| k == key) { cache.insertion.remove(i); }
                cache.insertion.push_back(key);
                slot
            } else {
                if cache.tiles.len() >= CACHE_LIMIT && let Some(old) = cache.insertion.pop_front() {
                    cache.tiles.remove(&old);
                }
                let slot = Slot::default();
                cache.tiles.insert(key, Arc::clone(&slot));
                cache.insertion.push_back(key);
                slot
            }
        };
        HOT.with(|hot| {
            let mut hot = hot.borrow_mut();
            if hot.len() >= 8 { hot.pop_front(); }
            hot.push_back((hot_key, Arc::clone(&slot)));
        });
        slot
    }

    /// Returns the exposure weight and the body-space gradient of the refined
    /// surface where the composed tiles applied; generator 300 carves along it.
    pub fn apply(&self, world: &World, cfg: &WorldSampleConfig, dir: DVec3, footprint: f32, ground: &mut geo::Ground) -> Option<(f64, DVec3)> {
        let focused = super::watershed_erosion(cfg.generator_version);
        let rifted = super::rifted_margins(cfg.generator_version);
        let connected = super::connected_relief(cfg.generator_version);
        let canyons = cfg.generator_version == super::LAYERED_CANYON_GENERATOR;
        let evolved = super::evolved_watersheds(cfg.generator_version);
        let weight = exposure(ground, focused, rifted || connected, canyons, evolved)
            * (1.0 - smooth(36.0, if connected { 148.0 } else { 96.0 }, f64::from(footprint)));
        if weight <= 0.0 { return None; }
        let (face, s, t) = Projection::Tangent.from_sphere(dir);
        let (x, z) = (s * atlas::HALF_M, t * atlas::HALF_M);
        let edge = atlas::HALF_M - x.abs().max(z.abs());
        if connected && edge < 90.0 {
            let gradient = self.apply_across_faces(world, cfg, dir, footprint, ground, weight);
            return Some((weight, gradient));
        }
        // The globe's parent relief is continuous at a face edge. Taper the
        // chart-local correction to the same elevation and first derivative.
        let weight = if connected { weight } else { weight * smooth(0.0, 100.0, edge) };
        if weight <= 0.0 { return None; }
        let (height, talus, gradient, base_gradient) = self.chart_sample(world, cfg, face, x, z, footprint);
        let before_top = f64::from(ground.top_m);
        let headroom = smooth(1.0, 32.0, 504.0 - before_top);
        ground.top_m += (weight * (height.min(0.0) + height.max(0.0) * headroom)) as f32;
        if super::composed_drainage(cfg.generator_version) {
            let correction = if height > 0.0 {
                let t = ((504.0 - before_top - 1.0) / 31.0).clamp(0.0, 1.0);
                gradient * headroom - base_gradient * (height * 6.0 * t * (1.0 - t) / 31.0)
            } else { gradient };
            let gradient = gradient::blended_gradient(base_gradient, correction, weight);
            let (east, north) = Projection::Tangent.tangents(face, s, t);
            let metres = atlas::RADIUS_M / atlas::HALF_M;
            ground.slope = gradient::physical_slope(gradient, east * metres, north * metres) as f32;
            ground.scree_m += (weight * talus * smooth(0.12, 0.35, f64::from(ground.slope))) as f32;
            return Some((weight, gradient::body_gradient(gradient, east * metres, north * metres)));
        }
        ground.slope = ground.slope.hypot((weight * gradient.length()) as f32);
        ground.scree_m += (weight * talus * smooth(0.12, 0.35, f64::from(ground.slope))) as f32;
        None
    }

    fn chart_sample(&self, world: &World, cfg: &WorldSampleConfig, face: Face, x: f64, z: f64, footprint: f32)
        -> (f64, f64, DVec2, DVec2)
    {
        let mut height = 0.0;
        let mut talus = 0.0;
        let mut gradient = DVec2::ZERO;
        let mut base_gradient = DVec2::ZERO;
        let composed = super::composed_drainage(cfg.generator_version);
        let coarse_weight = smooth(16.0, 32.0, f64::from(footprint));
        for (tx, wx) in blend(x) {
            for (tz, wz) in blend(z) {
                let share = wx * wz;
                if share <= 0.0 { continue; }
                let key = Key { face: face as u8, x: tx, z: tz };
                let (delta, slope, soil, base) = self.levels(world, cfg, key, coarse_weight >= 1.0)
                    .sample(world, cfg, key, DVec2::new(x, z), coarse_weight, footprint);
                height += share * delta;
                gradient += share * slope;
                if composed {
                    gradient += delta * DVec2::new(blend_derivative(x, tx) * wz,
                        wx * blend_derivative(z, tz));
                    base_gradient += share * base;
                }
                talus += share * soil;
            }
        }
        (height, talus, gradient, base_gradient)
    }

    fn apply_across_faces(&self, world: &World, cfg: &WorldSampleConfig, dir: DVec3,
        footprint: f32, ground: &mut geo::Ground, exposure: f64) -> DVec3
    {
        let half = atlas::HALF_M;
        let mut mass = 0.0;
        let mut height = 0.0;
        let mut talus = 0.0;
        let mut change = DVec3::ZERO;
        let mut base = DVec3::ZERO;
        let mut weight_gradient = DVec3::ZERO;
        let remaining = 504.0 - f64::from(ground.top_m);
        let headroom = smooth(1.0, 32.0, remaining);
        let t = ((remaining - 1.0) / 31.0).clamp(0.0, 1.0);
        let cap_gradient = 6.0 * t * (1.0 - t) / 31.0;
        let taper = |v: f64| {
            let t = ((v.abs() - half + 90.0) / 180.0).clamp(0.0, 1.0);
            (1.0 - t * t * (3.0 - 2.0 * t), -6.0 * t * (1.0 - t) * v.signum() / 180.0)
        };
        for face in Face::ALL {
            let axes = face.basis();
            let n = dir.dot(axes.n.as_dvec3());
            if n <= 0.0 { continue; }
            let x = (dir.dot(axes.u.as_dvec3()) / n).atan() * half / std::f64::consts::FRAC_PI_4;
            let z = (dir.dot(axes.v.as_dvec3()) / n).atan() * half / std::f64::consts::FRAC_PI_4;
            let (wx, dx) = taper(x);
            let (wz, dz) = taper(z);
            let weight = wx * wz;
            if weight <= 0.0 { continue; }
            let (h, soil, g, b) = self.chart_sample(world, cfg, face, x, z, footprint);
            let (east, north) = Projection::Tangent.tangents(face, x / half, z / half);
            let metres = atlas::RADIUS_M / half;
            let lift = |d| gradient::body_gradient(d, east * metres, north * metres);
            let dw = lift(DVec2::new(dx * wz, wx * dz));
            let cap = if h > 0.0 { headroom } else { 1.0 };
            mass += weight;
            height += weight * h * cap;
            talus += weight * soil;
            change += weight * (lift(g) * cap - lift(b) * (h.max(0.0) * cap_gradient))
                + h * cap * dw;
            base += weight * lift(b);
            weight_gradient += dw;
        }
        let delta = height / mass;
        ground.top_m += (exposure * delta) as f32;
        let gradient = (base + exposure * (change - delta * weight_gradient)) / mass;
        ground.slope = gradient.length() as f32;
        ground.scree_m += (exposure * talus / mass * smooth(0.12, 0.35, f64::from(ground.slope))) as f32;
        gradient
    }
}

fn exposure(ground: &geo::Ground, focused: bool, rifted: bool, canyons: bool, evolved: bool) -> f64 {
    if ground.water_m.is_some() { return 0.0; }
    let height = f64::from(ground.top_m);
    if evolved {
        // Drainage divides and headwaters may form on initially gentle ground.
        return smooth(SEA + 3.0, SEA + 14.0, height)
            * (1.0 - smooth(0.08, 0.30, f64::from(ground.delta)));
    }
    let grade = f64::from(ground.slope);
    let upland = if canyons { smooth(0.035, 0.125, grade) }
        else if focused { smooth(0.055, 0.17, grade) } else { 1.0 };
    let above_shore = if rifted { smooth(SEA + 3.0, SEA + 18.0, height) }
        else { smooth(SEA + 7.0, SEA + 25.0, height) };
    let bank = 1.0 - smooth(0.06, 0.38, f64::from(ground.riparian));
    let bank = if canyons { bank.max(0.8 * smooth(0.13, 0.35, grade)) } else { bank };
    above_shore
        * (0.43 + 0.57 * smooth(0.045, 0.17, grade))
        * bank
        * (1.0 - smooth(0.0, 0.15, f64::from(ground.delta))) * upland
}

fn blend(metres: f64) -> [(i32, f64); 2] {
    let tile = (metres / CORE_M).floor() as i32;
    let within = metres.rem_euclid(CORE_M);
    let (other, distance) = if within < CORE_M * 0.5 { (tile - 1, within) }
        else { (tile + 1, CORE_M - within) };
    let weight = 0.5 * (1.0 - smooth(0.0, BLEND_M, distance));
    [(tile, 1.0 - weight), (other, weight)]
}

fn blend_derivative(metres: f64, member: i32) -> f64 {
    let tile = (metres / CORE_M).floor() as i32;
    let within = metres.rem_euclid(CORE_M);
    let (distance, sign) = if within < CORE_M * 0.5 { (within, 1.0) }
        else { (CORE_M - within, -1.0) };
    let t = (distance / BLEND_M).clamp(0.0, 1.0);
    let other = -3.0 * t * (1.0 - t) * sign / BLEND_M;
    if member == tile { -other } else { other }
}

impl Levels {
    fn sample(&self, world: &World, cfg: &WorldSampleConfig, key: Key, p: DVec2, coarse_weight: f64, footprint: f32) -> (f64, DVec2, f64, DVec2) {
        let connected = super::connected_relief(cfg.generator_version);
        let evolved = super::evolved_watersheds(cfg.generator_version);
        let sample = |tile: &Tile| {
            if connected { tile.sample_smooth(p) } else { tile.sample(p.x, p.y) }
        };
        let distant = coarse_weight >= 1.0;
        let coarse = self.coarse.get_or_init(|| persistence::solve_cached(world, cfg, key, 32.0, 8, None, distant));
        let cover = || {
            let tile = self.cover.get_or_init(|| {
                Tile::new(coarse.grid, coarse.rock_delta.clone(), coarse.soil.clone(), coarse.base.clone(), evolved)
            });
            let (height, gradient, talus, base) = sample(tile);
            let height = if cfg.generator_version == super::WATERSHED_EROSION_GENERATOR {
                tile.filtered_height(p.x, p.y).unwrap_or(height)
            } else {
                height
            };
            (height, gradient, talus, base)
        };
        if coarse_weight >= 1.0 {
            return cover();
        }
        let solve_detail = || {
            let middle = persistence::solve_cached(world, cfg, key, 16.0, 7, Some(coarse), false);
            persistence::solve_cached(world, cfg, key, CELL_M, 8, Some(&middle), false)
        };
        let fine_weight = if connected || super::rifted_margins(cfg.generator_version) {
            1.0 - smooth(3.0, 8.0, f64::from(footprint))
        } else { 0.0 };
        if fine_weight > 0.0 {
            let near = sample(self.rift_detail.get_or_init(|| {
                let parent = solve_detail();
                let fine = persistence::solve_cached(world, cfg, key, 4.0, 5, Some(&parent), false);
                let _ = self.detail.set(Tile::new(parent.grid, parent.rock_delta, parent.soil, parent.base, evolved));
                Tile::new(fine.grid, fine.rock_delta, fine.soil, fine.base, evolved)
            }));
            if fine_weight >= 1.0 { return near; }
            let mid = sample(self.detail.get().expect("fine detail publishes its parent"));
            return (mid.0 + (near.0 - mid.0) * fine_weight, mid.1.lerp(near.1, fine_weight),
                mid.2 + (near.2 - mid.2) * fine_weight, mid.3.lerp(near.3, fine_weight));
        }
        let (height, gradient, talus, base) = sample(self.detail.get_or_init(|| {
            let fine = solve_detail();
            Tile::new(fine.grid, fine.rock_delta, fine.soil, fine.base, evolved)
        }));
        if coarse_weight <= 0.0 {
            return (height, gradient, talus, base);
        }
        let (coarse_height, coarse_gradient, coarse_talus, coarse_base) = cover();
        (height + coarse_weight * (coarse_height - height),
            gradient.lerp(coarse_gradient, coarse_weight),
            talus + coarse_weight * (coarse_talus - talus),
            base.lerp(coarse_base, coarse_weight))
    }
}

impl Tile {
    fn new(grid: Grid, mut rock_delta: Vec<f32>, soil: Vec<f32>, base: Option<Vec<f32>>, evolved: bool) -> Self {
        let (low, high) = if evolved { (-128.0, 80.0) } else { (-42.0, 26.0) };
        for (rock, &soil) in rock_delta.iter_mut().zip(&soil) {
            *rock = (*rock + soil).clamp(low, high);
        }
        Self { grid, delta: rock_delta, talus: soil, base }
    }

    // Round the joins of the coarse erosion grid without overshooting the
    // surrounding bed heights. The fine grid still supplies playable terrain.
    fn filtered_height(&self, x: f64, z: f64) -> Option<f64> {
        let (ix, iz, u, v) = self.grid.cell(x, z);
        let n = self.grid.n;
        if ix == 0 || iz == 0 || ix + 2 >= n || iz + 2 >= n {
            return None;
        }
        let rows = [iz - 1, iz, iz + 1, iz + 2].map(|row| {
            let i = row * n + ix;
            bounded_cubic([i - 1, i, i + 1, i + 2].map(|j| f64::from(self.delta[j])), u)
        });
        Some(bounded_cubic(rows, v))
    }

    fn sample(&self, x: f64, z: f64) -> (f64, DVec2, f64, DVec2) {
        let (ix, iz, u, v) = self.grid.cell(x, z);
        let i = iz * self.grid.n + ix;
        let [a, b, c, d] = [i, i + 1, i + self.grid.n, i + self.grid.n + 1]
            .map(|j| f64::from(self.delta[j]));
        let low = a + (b - a) * u;
        let high = c + (d - c) * u;
        let height = low + (high - low) * v;
        let dx = (b - a) * (1.0 - v) + (d - c) * v;
        let base = self.base.as_ref().map_or(DVec2::ZERO, |base| {
            let [a,b,c,d] = [i, i+1, i+self.grid.n, i+self.grid.n+1].map(|j| f64::from(base[j]));
            DVec2::new((b-a)*(1.0-v)+(d-c)*v, (c-a)*(1.0-u)+(d-b)*u) / self.grid.cell_m
        });
        (height, DVec2::new(dx, high - low) / self.grid.cell_m, self.grid.bilinear(&self.talus, x, z), base)
    }

    fn sample_smooth(&self, p: DVec2) -> (f64, DVec2, f64, DVec2) {
        let (x, z, u, v) = self.grid.cell(p.x, p.y);
        let (wx, dx) = spline(u);
        let (wz, dz) = spline(v);
        let index = |i: usize, t: usize| (i + t).saturating_sub(1).min(self.grid.n - 1);
        let mut height = 0.0;
        let mut talus = 0.0;
        let mut slope = DVec2::ZERO;
        let mut base_slope = DVec2::ZERO;
        for j in 0..4 {
            for i in 0..4 {
                let k = index(z, j) * self.grid.n + index(x, i);
                let weight = wx[i] * wz[j];
                let derivative = DVec2::new(dx[i] * wz[j], wx[i] * dz[j]);
                let delta = f64::from(self.delta[k]);
                height += weight * delta;
                talus += weight * f64::from(self.talus[k]);
                slope += derivative * delta;
                if let Some(base) = &self.base { base_slope += derivative * f64::from(base[k]); }
            }
        }
        (height, slope / self.grid.cell_m, talus, base_slope / self.grid.cell_m)
    }
}

fn spline(t: f64) -> ([f64; 4], [f64; 4]) {
    let s = 1.0 - t;
    let tt = t * t;
    (
        [s * s * s / 6.0, (4.0 - 6.0 * tt + 3.0 * tt * t) / 6.0,
            (4.0 - 6.0 * s * s + 3.0 * s * s * s) / 6.0, tt * t / 6.0],
        [-0.5 * s * s, -2.0 * t + 1.5 * tt, 2.0 * s - 1.5 * s * s, 0.5 * tt],
    )
}

fn bounded_cubic([a, b, c, d]: [f64; 4], t: f64) -> f64 {
    let slope = |left: f64, right: f64| {
        if left * right > 0.0 { 2.0 * left * right / (left + right) } else { 0.0 }
    };
    let step = c - b;
    let start = slope(b - a, step);
    let end = slope(step, d - c);
    b + t * (start + t * (3.0 * step - 2.0 * start - end + t * (start + end - 2.0 * step)))
}

struct Stage {
    grid: Grid,
    rock_delta: Vec<f32>,
    soil: Vec<f32>,
    base: Option<Vec<f32>>,
}

impl Stage {
    fn solve(world: &World, cfg: &WorldSampleConfig, key: Key, spacing: f64, passes: usize, parent: Option<&Self>, distant: bool) -> Self {
        let focused = super::watershed_erosion(cfg.generator_version);
        let composed = super::composed_drainage(cfg.generator_version);
        let connected = super::connected_relief(cfg.generator_version);
        let grid = Grid {
            n: ((CORE_M + 2.0 * PAD_M) / spacing) as usize,
            cell_m: spacing,
            min_m: [f64::from(key.x) * CORE_M - PAD_M, f64::from(key.z) * CORE_M - PAD_M],
        };
        let mesh = Mesh::with_alignment(grid, connected);
        let face = Face::ALL[key.face as usize];
        let mut base = Vec::with_capacity(grid.len());
        let mut rock = Vec::with_capacity(grid.len());
        let mut rain = Vec::with_capacity(grid.len());
        let mut fixed = Vec::with_capacity(grid.len());
        let mut strength = Vec::with_capacity(grid.len());
        let mut rock_limit = Vec::with_capacity(grid.len());
        let mut soil = Vec::with_capacity(grid.len());
        let mut uplift = Vec::with_capacity(grid.len());
        let strike = DVec3::new(0.35, -0.69, 0.63).normalize();
        let rifted = super::rifted_margins(cfg.generator_version);
        let coupled = super::coupled_history(cfg.generator_version);
        let canyons = cfg.generator_version == super::LAYERED_CANYON_GENERATOR;
        let mut beds = canyons.then(|| Vec::with_capacity(grid.len()));
        let branching = super::branching_watersheds(cfg.generator_version);
        let historical = super::geological_history(cfg.generator_version);
        let evolved = super::evolved_watersheds(cfg.generator_version);
        let regional = evolved.then(|| {
            let regions = if distant { &world.slope_tiles.distant_regions } else { &world.slope_tiles.regions };
            regions.window(world, cfg, face, grid)
        });
        let mut outlets = evolved.then(|| Vec::with_capacity(grid.len()));
        let mut history_beds = historical.then(|| Vec::with_capacity(grid.len()));
        let mut repose = vec![0.0; if canyons || historical { grid.len() } else { 0 }];
        for (i, point) in mesh.points.iter().enumerate() {
            let dir = Projection::Tangent.to_sphere(face, point.x / atlas::HALF_M, point.y / atlas::HALF_M);
            let surface = world.surface_query(dir);
            let volcanic = surface.volcano_shape();
            let intact = volcanic.active_caldera || surface.edifice_lift() > 12.0;
            // Every stage measures residual height against the same parent
            // surface. Coarse sampling must not reintroduce filtered river beds.
            let ground = surface.ground_base(cfg, CELL_M as f32, volcanic);
            let openness = exposure(&ground, focused, rifted || connected, canyons, evolved);
            let boundary = grid.on_border(i) || openness == 0.0 || intact;
            let body = dir * atlas::RADIUS_M;
            let along = strike * body.dot(strike);
            let across = body - along;
            let joint = if branching { noise(cfg.seed, body / 240.0, 14383) }
                else { noise(cfg.seed, along / 168.0 + across / 61.0, 14383) };
            let lens = noise(cfg.seed, body / 186.0, 14387);
            let erodible = smooth(0.23, 0.76, 0.64 * joint + 0.36 * lens);
            let h = f64::from(ground.top_m);
            if let Some(outlets) = &mut outlets {
                outlets.push(ground.water_m.is_some() || h < SEA + 3.0);
            }
            let collision = surface.collision();
            if let Some(beds) = &mut beds { beds.push(world.strata(cfg, dir)); }
            if let Some(beds) = &mut history_beds { beds.push(surface.history_strata(cfg)); }
            let bedding = super::bedding_height(cfg, body, collision, h);
            let weak_bed = if canyons || historical { 0.0 } else {
                1.0 - smooth(8.0, 20.0, (bedding - 6.0).abs().min(super::BED_INTERVAL - (bedding - 6.0).abs()))
            };
            let history_soil = if coupled { surface.sediment_cover() } else { 0.0 };
            let (relief, cover) = if let Some(regional) = &regional {
                if openness == 0.0 || intact {
                    (-history_soil, history_soil)
                } else if let Some(old) = parent {
                    (old.grid.bilinear(&old.rock_delta, point.x, point.y),
                        old.grid.bilinear(&old.soil, point.x, point.y))
                } else {
                    regional.surface(*point)
                }
            } else if boundary {
                (-history_soil, history_soil)
            } else if let Some(old) = parent {
                let inherited = old.grid.bilinear(&old.rock_delta, point.x, point.y);
                let inherited_soil = old.grid.bilinear(&old.soil, point.x, point.y);
                let local = if rifted || connected { 0.0 } else { (noise(cfg.seed, body / (spacing * 4.0), 14393 + spacing as u32) - 0.5)
                    * spacing * 0.15 * openness };
                (inherited + local, inherited_soil)
            } else {
                (if rifted || connected { -history_soil } else { (8.0 * (joint - 0.5) + 18.0 * (lens - 0.5)) * openness }, history_soil)
            };
            base.push(h);
            rock.push(h + relief);
            soil.push(cover);
            rain.push(0.25 + 0.75 * f64::from(ground.moisture));
            fixed.push(boundary);
            let damage = if focused { surface.fault_damage() } else { 0.0 };
            strength.push(openness * (0.35 + 0.85 * erodible + 0.45 * weak_bed + 0.60 * damage));
            rock_limit.push(0.57 + 1.45 * (1.0 - erodible) * (1.0 - 0.73 * weak_bed) * (1.0 - 0.6 * damage));
            // Differential uplift follows broad rock blocks. The fine stage
            // inherits the new divides and carves them without further uplift.
            let fold = noise(cfg.seed, along / 285.0 + across / 112.0, 14389);
            let uplift_rate = if !rifted && parent.is_none() && !boundary {
                4.0 * openness.sqrt() * (0.53 + 0.47 * collision.clamp(0.0, 1.0))
                    * smooth(0.37, 0.74, fold) * (0.48 + 0.52 * (1.0 - weak_bed))
                    * smooth(24.0, 120.0, h)
            } else {
                0.0
            };
            uplift.push(uplift_rate);
        }
        let mut height = vec![0.0; grid.len()];
        let mut load = vec![0.0; grid.len()];
        let local_runoff: Vec<_> = if branching {
            rain.iter().zip(&mesh.area).map(|(rain, area)| rain * area).collect()
        } else { Vec::new() };
        let mut bank_support = if branching { vec![1.0_f64; grid.len()] } else { Vec::new() };
        if let Some(outlets) = &mut outlets {
            watershed::open_perimeter(grid, &rock, &soil, outlets);
        }
        let drainage_outlets = outlets.as_deref().unwrap_or(&fixed);
        let mut momentum = branching.then(|| vec![DVec2::ZERO; grid.len()]);
        if let Some(regional) = &regional {
            regional.supply(&mesh, &fixed, &mut rain, momentum.as_deref_mut());
        } else if coupled {
            inflow::add(world, face, &mesh, &fixed, &mut rain, None);
        }
        for _ in 0..passes {
            for i in 0..height.len() {
                rock[i] += uplift[i];
                height[i] = rock[i] + soil[i];
            }
            let flow = if let Some(momentum) = &momentum { mesh.route_momentum(&height, drainage_outlets, &rain, momentum) }
                else { mesh.route(&height, drainage_outlets, &rain) };
            load.fill(0.0);
            bank_support.fill(1.0);
            for &i in flow.order.iter().rev() {
                let i = i as usize;
                let routes = flow.links[i].as_slice();
                if fixed[i] {
                    if evolved { transport::forward(routes, load[i], &mut load); }
                    continue;
                }
                let grade = flow.grade[i];
                let runoff = (flow.q[i] / 900.0).sqrt();
                let resistance = history_beds.as_ref().map_or_else(
                    || beds.as_ref().map_or(1.0, |beds| beds[i].resistance(rock[i])),
                    |beds| beds[i].as_ref().map_or(1.0, |b| b.resistance(rock[i])));
                // Routing and other nodes have not changed this rock height.
                // Keep repose at the same pre-incision height; fixed-node limits
                // are never read by move_talus, including its neighbour loop.
                if canyons || historical {
                    repose[i] = (rock_limit[i] * resistance).clamp(0.42, 2.9);
                }
                let channel = if branching {
                    let area = flow.q[i] - local_runoff[i];
                    let onset = (520.0 + 650.0 * resistance) / (5.0 * (grade + 0.05)).clamp(0.55, 2.8);
                    smooth(0.32 * onset, 1.45 * onset, area)
                } else { 1.0 };
                let threshold = if connected { 0.043 } else { 0.055 };
                let shear = (runoff * grade - threshold).max(0.0);
                let scour = (0.68 * shear * (0.30 + 0.70 * channel)).min(soil[i]);
                soil[i] -= scour;
                let transport_speed = if branching { (0.60 + 0.36 * flow.speed[i]).clamp(0.6, 1.55) } else { 1.0 };
                let capacity = mesh.area[i] * 2.8 * runoff * grade * transport_speed;
                let abrasion = if super::resolved_history(cfg.generator_version) {
                    transport::exposed_abrasion(load[i], capacity, mesh.area[i], soil[i], strength[i], grade, spacing)
                } else if composed {
                    transport::abrasion(load[i], capacity, mesh.area[i], soil[i], strength[i], grade, spacing)
                } else { 0.0 };
                let mut power = if focused { 3.8 + 1.8 * smooth(1.0, 5.0, runoff) } else { 3.0 };
                if connected {
                    power *= 1.0 + 0.30 * smooth(2.0, 6.0, runoff) * smooth(0.045, 0.16, grade);
                }
                let detachment = power * strength[i] * shear * (-soil[i] / 1.3).exp();
                let incision = if canyons || evolved { 0.74 } else { 0.45 };
                let limit = if evolved { 120.0 } else { 42.0 };
                let rock_cut = ((detachment + abrasion) * channel / resistance)
                    .min(grade * incision * spacing)
                    .min((rock[i] - base[i] + limit).max(0.0));
                rock[i] -= rock_cut;
                if branching && channel > 0.65 && rock_cut > 0.08 {
                    // A cut can undermine a steep adjacent bank. The talus
                    // step below transfers the failed rock and soil downhill.
                    let support = 1.0 - 0.40 * channel * smooth(0.08, 0.65, rock_cut);
                    for edge in mesh.links[i].as_slice() {
                        let j = edge.to as usize;
                        if height[j] - height[i] > 0.48 * f64::from(edge.distance) {
                            bank_support[j] = bank_support[j].min(support);
                        }
                    }
                }
                let sediment = load[i] + (scour + rock_cut) * mesh.area[i];
                let deposit = ((sediment - capacity).max(0.0) * 0.27).min(mesh.area[i] * 0.45);
                soil[i] += deposit / mesh.area[i];
                transport::forward(routes, sediment - deposit, &mut load);
            }
            for (repose, support) in repose.iter_mut().zip(&bank_support) { *repose *= support; }
            let limits = if canyons || historical { &repose } else { &rock_limit };
            move_talus(&mesh, &fixed, limits, &mut rock, &mut soil, &mut height, &mut load);
        }
        Self {
            grid,
            base: composed.then(|| mesh.regularize(|i| base[i])),
            rock_delta: mesh.regularize(|i| rock[i] - base[i]),
            soil: mesh.regularize(|i| soil[i]),
        }
    }
}

pub(super) fn move_talus(mesh: &Mesh, fixed: &[bool], limit: &[f64], rock: &mut [f64], soil: &mut [f64], height: &mut [f64], mass: &mut [f64]) {
    let spacing = mesh.grid.cell_m;
    for i in 0..height.len() { height[i] = rock[i] + soil[i]; }
    mass.fill(0.0);
    for i in 0..height.len() {
        if fixed[i] { continue; }
        let links = mesh.links[i].as_slice();
        let mut weights = [0.0; 8];
        let mut total = 0.0;
        let mut excess = 0.0_f64;
        let mut steepest = 0.0_f64;
        for (slot, link) in links.iter().enumerate() {
            let d = f64::from(link.distance);
            let fall = (height[i] - height[link.to as usize]).max(0.0);
            let grade = fall / d;
            // Roering et al. 1999: soil transport accelerates as slope
            // approaches repose. Bound the denominator for a finite step.
            let flux = grade / (1.0 - (grade / 0.68).powi(2)).max(0.12);
            weights[slot] = flux;
            total += flux;
            steepest = steepest.max(flux);
            excess = excess.max(fall - limit[i] * d);
        }
        if total == 0.0 { continue; }
        let released = (0.18 * excess).min(spacing * 0.075) * (-soil[i] / 1.6).exp();
        rock[i] -= released;
        let moved = (0.025 * spacing * steepest).min(soil[i] + released);
        mass[i] += (released - moved) * mesh.area[i];
        for (slot, link) in links.iter().enumerate() {
            let j = link.to as usize;
            // Fixed outlets export talus; protected water beds stay fixed.
            if !fixed[j] { mass[j] += moved * mesh.area[i] * weights[slot] / total; }
        }
    }
    for i in 0..soil.len() { soil[i] = (soil[i] + mass[i] / mesh.area[i]).max(0.0); }
}

#[cfg(test)]
#[path = "slope_relief/resistance_tests.rs"]
mod resistance_tests;

#[cfg(test)]
#[path = "slope_relief/surface_query_tests.rs"]
mod surface_query_tests;
