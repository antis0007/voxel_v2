//! Opt-in regional erosion review. Inputs come from the spherical solve;
//! the result is a metre-addressed heightfield for offline comparison.
pub mod braid;
mod explore;
mod measurements;
pub(in crate::planetary) mod structural;
mod valley_form;
pub use measurements::{Measurements, measure, measure_whole};

use super::super::{SEA, World, active, direction, get};
use crate::{BlockId, WorldSampleConfig};
use crate::geo::{erosion, grid::Grid};
use engine_core::cube_sphere::atlas;
use glam::{DVec2, DVec3};

pub const SPAN_M: f64 = 4_000.0;
const OUTLET_HALF_WIDTH_M: f64 = 160.0;
const CHANNEL_RUNOFF: f64 = 24_000.0;

#[derive(Clone, Copy)]
pub struct Frame {
    pub origin: DVec2,
    pub across: DVec2,
    pub downstream: DVec2,
}

impl Frame {
    pub fn chart(self, x: f64, z: f64) -> DVec2 {
        self.origin + x * self.across + z * self.downstream
    }

    pub fn local(self, chart: DVec2) -> DVec2 {
        let d = chart - self.origin;
        DVec2::new(d.dot(self.across), d.dot(self.downstream))
    }
}

#[derive(Clone, Copy)]
struct Material {
    soil_m: f64,
    rock_erodibility: f64,
    bed_contrast: f64,
    diffusivity: f64,
    critical_slope: f64,
    outlet_drop_m: f64,
    ravine_depth_m: f64,
    ravine_width_m: f64,
    spur_relief_m: f64,
    crest_lift_m: f64,
    summit_lift_m: f64,
    /// Share of incision concentrated in the inner gorge.
    inner_gorge: f64,
    varied_bedding: bool,
    upland_gain: f64,
    rill_relief_m: f64,
    rock_package: Option<structural::RockPackage>,
    weathering: bool,
    upland_runoff: bool,
    exploration: bool,
    valley_form: bool,
    sill_erosion: bool,
}

const LAYERED: Material = Material {
    soil_m: 0.65,
    rock_erodibility: 0.011,
    bed_contrast: 0.84,
    diffusivity: 0.16,
    critical_slope: 0.85,
    outlet_drop_m: 34.0,
    ravine_depth_m: 31.0,
    ravine_width_m: 41.0,
    spur_relief_m: 118.0,
    crest_lift_m: 16.0,
    summit_lift_m: 41.0,
    inner_gorge: 0.24,
    varied_bedding: true,
    upland_gain: -0.17,
    rill_relief_m: 25.0,
    rock_package: None,
    weathering: false,
    upland_runoff: false,
    exploration: false,
    valley_form: false,
    sill_erosion: false,
};
const SOIL_COVERED: Material = Material {
    soil_m: 4.2,
    rock_erodibility: 0.007,
    bed_contrast: 0.12,
    diffusivity: 0.8,
    critical_slope: 0.38,
    outlet_drop_m: 8.0,
    ravine_depth_m: 28.0,
    ravine_width_m: 54.0,
    spur_relief_m: 104.0,
    crest_lift_m: 12.0,
    summit_lift_m: 29.0,
    inner_gorge: 0.16,
    varied_bedding: false,
    upland_gain: 1.05,
    rill_relief_m: 17.0,
    rock_package: None,
    weathering: false,
    upland_runoff: false,
    exploration: false,
    valley_form: false,
    sill_erosion: false,
};

// Review profile only. Relief is established at the regional scale before
// the same erosion/transport/deposition sequence refines the catchment.
const GORGE: Material = Material {
    outlet_drop_m: 62.0,
    ravine_depth_m: 46.0,
    ravine_width_m: 42.0,
    spur_relief_m: 164.0,
    crest_lift_m: 19.0,
    inner_gorge: 0.63,
    upland_gain: 0.0,
    rill_relief_m: 18.0,
    varied_bedding: true,
    ..LAYERED
};

fn mapped_rock(site: Site) -> Material {
    Material {
        rock_package: Some(structural::RockPackage::new(site.outlet_m - 34.0)),
        soil_m: 0.28,
        rock_erodibility: 0.0045,
        diffusivity: 0.11,
        ..LAYERED
    }
}

fn jointed_rock(mut material: Material, seed: u32) -> Material {
    material.rock_package = material.rock_package.map(|rock| rock.with_joints(seed));
    material.weathering = true;
    material.upland_runoff = true;
    material
}

fn sediment_weathering(mut material: Material) -> Material {
    if let Some(rock) = &mut material.rock_package {
        rock.sediment_weathering = true;
    }
    material
}

#[derive(Clone, Copy)]
struct Site {
    frame: Frame,
    outlet_m: f64,
    parent_runoff_area_m2: f64,
    relief: f64,
    collision: f64,
}

/// Choose distinct reaches with room for their upstream landscape. Use the
/// receiver direction, rather than the chart axes, to set the window.
fn candidate_sites(cfg: &WorldSampleConfig, world: &World) -> Vec<Site> {
    let mut candidates = Vec::new();
    for (i, &r) in world.flow.receiver.iter().enumerate() {
        let q = world.flow.discharge[i];
        if i == r
            || !(350_000.0..=3_000_000.0).contains(&q)
            || world.height[i] < SEA + 46.0
            || world.flow.spill[i] > world.height[i] + 0.5
        {
            continue;
        }
        let chart = |i| {
            let p = atlas::from_body(world.river_nodes[i]);
            DVec2::new(p.x, p.z)
        };
        let outlet = chart(i);
        let downstream = (chart(r) - outlet).normalize_or_zero();
        if downstream == DVec2::ZERO || (chart(r) - outlet).length() > 650.0 {
            continue;
        }
        let across = downstream.perp();
        let frame = Frame {
            origin: outlet - downstream * SPAN_M - across * (SPAN_M * 0.5),
            across,
            downstream,
        };
        // Keep a window on one chart, with a margin around all four corners.
        let face = atlas::split(DVec3::new(outlet.x, 0.0, outlet.y)).0;
        if [(0.0, 0.0), (SPAN_M, 0.0), (0.0, SPAN_M), (SPAN_M, SPAN_M)]
            .into_iter()
            .any(|(x, z)| {
                let p = frame.chart(x, z);
                let (f, local) = atlas::split(DVec3::new(p.x, 0.0, p.y));
                f != face || local.x.abs().max(local.z.abs()) > atlas::HALF_M - 400.0
            })
        {
            continue;
        }
        let (mut low, mut high) = (f64::INFINITY, f64::NEG_INFINITY);
        let mut collision = 0.0;
        for z in 0..=6 {
            for x in 0..=6 {
                let p = frame.chart(f64::from(x) * SPAN_M / 6.0, f64::from(z) * SPAN_M / 6.0);
                let d = direction(cfg, p.x, p.y);
                let h = world.mesh.sample(&world.height, d);
                low = low.min(h);
                high = high.max(h);
                collision += world.mesh.sample(&world.collision, d).min(1.0);
            }
        }
        if low > SEA + 6.0 && high - world.height[i] >= 35.0 {
            candidates.push(Site {
                frame,
                outlet_m: world.height[i],
                parent_runoff_area_m2: q,
                relief: high - low,
                collision: collision / 49.0,
            });
        }
    }
    candidates
}

fn sites(cfg: &WorldSampleConfig, world: &World) -> Option<[Site; 2]> {
    let mut candidates = candidate_sites(cfg, world);
    let centre = |s: &Site| s.frame.chart(SPAN_M * 0.5, SPAN_M * 0.5);
    let canyon = choose_site(cfg, world, &mut candidates, LAYERED, 320.0, 1.0, 3.2)?;
    candidates.retain(|s| centre(s).distance(centre(&canyon)) > SPAN_M * 1.5);
    let valley = choose_site(cfg, world, &mut candidates, SOIL_COVERED, 145.0, 0.0, 2.7)?;
    Some([canyon, valley])
}

/// Only the final terrain fields are retained by a playable planet.
pub(in crate::planetary) struct PlayableRegion {
    frame: Frame,
    centre: DVec3,
    grid: Grid,
    height: Vec<f32>,
    soil: Vec<f32>,
    runoff: Vec<f32>,
    rock_package: Option<structural::RockPackage>,
    elevation_offset: f64,
    entry: Option<DVec2>,
    trails: Option<explore::Trails>,
    valley_deposits: Option<Vec<u8>>,
}

pub(in crate::planetary) struct RegionSurface {
    pub weight: f64,
    pub height: f64,
    pub slope: f64,
    pub soil: f64,
    pub drainage: f64,
}

/// Chart transform of the baked region sampled by the normal Planet path.
pub fn playable_frame(cfg: &WorldSampleConfig) -> Option<Frame> {
    if !active(cfg) {
        return None;
    }
    get(cfg).eroded_region.as_ref().map(|region| region.frame)
}

/// The paths used by the surface column sampler, in chart metres.
pub fn playable_trails(cfg: &WorldSampleConfig) -> Option<[Vec<DVec2>; 2]> {
    if !super::super::uses_catchment_trails(cfg.generator_version) || !active(cfg) { return None; }
    let world = get(cfg);
    let region = world.eroded_region.as_ref()?;
    Some(region.trails.as_ref()?.chart(region.frame))
}

impl PlayableRegion {
    pub(in crate::planetary) fn bake(cfg: &WorldSampleConfig, world: &World) -> Option<Self> {
        let started = std::time::Instant::now();
        let mut candidates = candidate_sites(cfg, world);
        let site = choose_site(cfg, world, &mut candidates, LAYERED, 320.0, 1.0, 3.2)?;
        let material = if super::super::uses_mapped_canyon(cfg.generator_version) {
            Material {
                weathering: matches!(cfg.generator_version,
                    super::super::WEATHERING_HISTORY_GENERATOR | super::super::UPLAND_RUNOFF_GENERATOR),
                upland_runoff: cfg.generator_version == super::super::UPLAND_RUNOFF_GENERATOR,
                ..mapped_rock(site) }
        } else {
            LAYERED
        };
        let exploration = super::super::uses_catchment_trails(cfg.generator_version);
        let sill_erosion = cfg.generator_version == super::super::SILL_EROSION_GENERATOR;
        let valley_form = sill_erosion || cfg.generator_version == super::super::VALLEY_FORM_GENERATOR;
        let bedrock_scarp = cfg.generator_version == super::super::BEDROCK_SCARP_GENERATOR;
        let folded = bedrock_scarp || cfg.generator_version == super::super::FOLDED_SCARP_GENERATOR;
        let coupled = cfg.generator_version == super::super::SEDIMENT_WEATHERING_GENERATOR;
        let material = if cfg.generator_version == super::super::JOINTED_CANYON_GENERATOR || exploration || coupled || folded {
            Material { exploration, valley_form, sill_erosion, ..jointed_rock(material, cfg.seed) }
        } else {
            material
        };
        let mut material = if coupled || folded { sediment_weathering(material) } else { material };
        if folded {
            material.rock_package = material.rock_package.map(|rock| rock.with_folds(bedrock_scarp));
        }
        let solved = solve(cfg, world, site, material, false, false);
        let frame = solved.frame;
        let centre = frame.chart(SPAN_M * 0.5, SPAN_M * 0.5);
        let grid = Grid { n: solved.side, cell_m: solved.spacing_m, min_m: [0.0; 2] };
        let mut height: Vec<_> = solved.bedrock_m.into_iter().zip(&solved.sediment_m)
            .map(|(rock, soil)| rock + soil).collect();
        let elevation_offset = if folded || cfg.generator_version == super::super::UPLAND_RUNOFF_GENERATOR {
            // Preserve the evolved relief when fitting the canyon into the
            // playable height range. Clamping each column would plane off
            // the high ridges. Move the rock package by the same datum.
            let (low, high) = height.iter().fold((f32::INFINITY, f32::NEG_INFINITY),
                |(low, high), &h| (low.min(h), high.max(h)));
            let offset = (high - (engine_voxel::WORLD_MAX_Y_METERS - 7.0)).max(0.0)
                .min((low - SEA as f32 - 6.0).max(0.0));
            for h in &mut height {
                *h -= offset;
            }
            f64::from(offset)
        } else {
            0.0
        };
        let mut region = Self {
            frame,
            centre: direction(cfg, centre.x, centre.y),
            grid,
            height,
            soil: solved.sediment_m,
            runoff: solved.runoff_m3_year.into_iter().map(|volume| volume as f32).collect(),
            rock_package: solved.rock_package,
            elevation_offset,
            entry: None,
            trails: solved.trails,
            valley_deposits: solved.valley_deposits,
        };
        if region.rock_package.is_some() {
            region.entry = region.trails.as_ref().and_then(|trails| trails.paths[0].first())
                .map(|p| region.frame.chart(p.x, p.y)).or_else(|| region.find_entry());
        }
        eprintln!("[worldgen] regional_erosion seed={} centre_x={:.2} centre_z={:.2} cell_m={} bake_ms={:.0}",
            cfg.seed, centre.x, centre.y, grid.cell_m, started.elapsed().as_secs_f64() * 1000.0);
        region.log_review_cameras();
        if material.upland_runoff {
            eprintln!("[worldgen] regional_frame origin={:?} across={:?} downstream={:?} upland_rock_m3={:.1} datum_shift_m={elevation_offset:.1}",
                frame.origin, frame.across, frame.downstream, solved.upland_rock_m3);
        }
        Some(region)
    }

    fn find_entry(&self) -> Option<DVec2> {
        let height = |x, z| self.grid.bilinear(&self.height, x, z);
        let mut best: Option<(f64, DVec2)> = None;
        for z in (920..3080).step_by(32) {
            for x in (920..3080).step_by(32) {
                let (x, z) = (f64::from(x), f64::from(z));
                let h = height(x, z);
                if h < SEA + 5.0 { continue; }
                let footing = [height(x - 6.0, z), height(x + 6.0, z),
                    height(x, z - 6.0), height(x, z + 6.0)];
                if footing.into_iter().any(|y| (y - h).abs() > 1.5) { continue; }
                let grade = (height(x, z - 24.0) - h).abs().min((height(x, z + 24.0) - h).abs());
                if grade > 5.0 { continue; }
                let relief = height(x - 120.0, z).max(height(x + 120.0, z)) - h;
                let score = relief - 4.0 * grade;
                if best.is_none_or(|(high, _)| score > high) {
                    best = Some((score, self.frame.chart(x, z)));
                }
            }
        }
        best.map(|(_, at)| at)
    }

    pub(in crate::planetary) fn entry(&self) -> Option<DVec2> { self.entry }

    pub(in crate::planetary) fn on_trail(&self, dir: DVec3) -> bool {
        self.trails.as_ref().is_some_and(|trails| self.local(dir).is_some_and(|(p, _)| trails.covers(p)))
    }

    pub(in crate::planetary) fn valley_cover(&self, dir: DVec3) -> Option<BlockId> {
        let deposits = self.valley_deposits.as_ref()?;
        let (p, _) = self.local(dir)?;
        let x = (p.x / self.grid.cell_m) as usize;
        let z = (p.y / self.grid.cell_m) as usize;
        match deposits[z * self.grid.n + x] {
            1 => Some(BlockId::Sand),
            2 => Some(BlockId::Gravel),
            _ => None,
        }
    }

    pub(in crate::planetary) fn trail_overlaps(&self, dir: DVec3, radius: f64) -> bool {
        self.trails.as_ref().is_some_and(|trails| self.local(dir).is_some_and(|(p, _)| trails.overlaps(p, radius)))
    }

    fn log_review_cameras(&self) {
        let height = |x, z| self.grid.bilinear(&self.height, x, z);
        let mut best = (f64::NEG_INFINITY, 2000.0, 2000.0);
        for z in (1000..3000).step_by(24) {
            for x in (650..3350).step_by(24) {
                let (x, z) = (f64::from(x), f64::from(z));
                let h = height(x, z);
                let relief = height(x - 180.0, z).min(height(x + 180.0, z)) - h;
                let grade = (height(x, z + 12.0) - h).abs()
                    .max((height(x + 12.0, z) - h).abs());
                let score = relief - grade * 4.0;
                if score > best.0 {
                    best = (score, x, z);
                }
            }
        }
        let (_, x, z) = best;
        let ground = self.frame.chart(x, z);
        let air = self.frame.chart(x - 480.0, z - 620.0);
        let pose = |label, at: DVec2, forward: DVec2, clearance, pitch| {
            let yaw = forward.x.atan2(-forward.y);
            eprintln!("[worldgen] regional_erosion {label}_camera={:.2},{:.2},{clearance},{yaw:.6},{pitch}", at.x, at.y);
        };
        pose("ground", ground, self.frame.downstream, 0.1, -0.04);
        pose("air", air, ground - air, 610.0, -0.63);
    }

    pub(in crate::planetary) fn checksum(&self, base: u64) -> u64 {
        [self.frame.origin, self.frame.across, self.frame.downstream].into_iter()
            .flat_map(|v| [v.x.to_bits(), v.y.to_bits()])
            .chain(self.height.iter().chain(&self.soil).chain(&self.runoff).map(|v| u64::from(v.to_bits())))
            .chain(self.trails.iter().flat_map(|trails| trails.paths.iter().flatten())
                .flat_map(|v| [v.x.to_bits(), v.y.to_bits()]))
            .fold(base, |hash, bits| (hash ^ bits).wrapping_mul(0x100000001b3))
    }

    fn local(&self, dir: DVec3) -> Option<(DVec2, f64)> {
        if dir.distance_squared(self.centre) > (5_000.0 / atlas::RADIUS_M).powi(2) {
            return None;
        }
        let chart = atlas::from_body(dir * atlas::RADIUS_M);
        let p = self.frame.local(DVec2::new(chart.x, chart.z));
        let margin = p.min(DVec2::splat(SPAN_M) - p).min_element();
        (margin > 0.0).then_some((p, margin))
    }

    pub(in crate::planetary) fn rock_at(&self, dir: DVec3, elevation: f64) -> Option<BlockId> {
        let package = self.rock_package?;
        let (p, margin) = self.local(dir)?;
        // The bedrock contacts are mapped in the core, where the solved
        // surface has taken over completely from the parent planet.
        (margin >= 420.0).then(|| package.block(p.x, elevation + self.elevation_offset, p.y))
    }

    pub(in crate::planetary) fn outcrop(&self, dir: DVec3, elevation: f64) -> Option<BlockId> {
        let package = self.rock_package?;
        let (p, margin) = self.local(dir)?;
        (margin >= 420.0 && self.grid.bilinear(&self.soil, p.x, p.y) < 0.22)
            .then(|| package.block(p.x, elevation + self.elevation_offset, p.y))
    }

    pub(in crate::planetary) fn sample(&self, dir: DVec3) -> Option<RegionSurface> {
        let (p, margin) = self.local(dir)?;
        let (x, z, tx, tz) = self.grid.cell(p.x, p.y);
        let i = z * self.grid.n + x;
        let [a, b, c, d] = [i, i + 1, i + self.grid.n, i + self.grid.n + 1]
            .map(|i| f64::from(self.height[i]));
        let dx = (b - a) * (1.0 - tz) + (d - c) * tz;
        let dz = (c - a) * (1.0 - tx) + (d - b) * tx;
        Some(RegionSurface {
            weight: super::super::smooth(0.0, 420.0, margin),
            height: a + (b - a) * tx + dz * tz,
            slope: dx.hypot(dz) / self.grid.cell_m,
            soil: self.grid.bilinear(&self.soil, p.x, p.y),
            drainage: super::super::smooth(8_000.0, 160_000.0, self.grid.bilinear(&self.runoff, p.x, p.y)),
        })
    }
}

fn choose_site(
    cfg: &WorldSampleConfig,
    world: &World,
    candidates: &mut [Site],
    material: Material,
    target_relief: f64,
    prefer_collision: f64,
    target_area: f64,
) -> Option<Site> {
    let score = |s: &Site, relief: f64| {
        (relief - target_relief).abs() / 90.0 + (s.collision - prefer_collision).abs() * 0.6
    };
    candidates.sort_by(|a, b| score(a, a.relief).total_cmp(&score(b, b.relief)));
    candidates
        .iter()
        .take(96)
        .filter_map(|&s| {
            let (area, relief) = preview(cfg, world, s, material);
            (area >= 0.8).then_some((s, score(&s, relief) + (area - target_area).abs()))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(s, _)| s)
}

fn preview(cfg: &WorldSampleConfig, world: &World, site: Site, material: Material) -> (f64, f64) {
    let grid = Grid {
        n: 64,
        cell_m: SPAN_M / 64.0,
        min_m: [0.0; 2],
    };
    let h: Vec<_> = (0..grid.len())
        .map(|i| {
            let [x, z] = grid.at(i);
            base_height(cfg, world, site, material, x, z).0
        })
        .collect();
    let fixed: Vec<_> = (0..grid.len()).map(|i| grid.on_border(i)).collect();
    let flow = erosion::route(&grid, &h, &fixed, &vec![1.0; grid.len()]);
    let mask = outlet_mask(grid, &flow);
    let mut count = 0;
    let (mut low, mut high) = (f64::INFINITY, f64::NEG_INFINITY);
    for (&h, selected) in h.iter().zip(mask) {
        if selected {
            count += 1;
            low = low.min(h);
            high = high.max(h);
        }
    }
    (f64::from(count) * grid.cell_m.powi(2) / 1e6, high - low)
}

fn outlet_mask(grid: Grid, flow: &erosion::Flow) -> Vec<bool> {
    let mut selected = vec![false; grid.len()];
    for &i in &flow.order {
        let i = i as usize;
        let r = flow.receiver[i] as usize;
        selected[i] = if i == r {
            let [x, z] = grid.at(i);
            z >= SPAN_M - grid.cell_m && (x - SPAN_M * 0.5).abs() <= OUTLET_HALF_WIDTH_M
        } else {
            selected[r]
        };
    }
    selected
}

fn base_height(
    cfg: &WorldSampleConfig,
    world: &World,
    site: Site,
    material: Material,
    x: f64,
    z: f64,
) -> (f64, DVec3) {
    let p = site.frame.chart(x, z);
    let d = direction(cfg, p.x, p.y);
    let coarse = world
        .mesh
        .reconstruct_height(&world.height, &world.gradient, d);
    let body = d * atlas::RADIUS_M;
    let detail = |wavelength: f64, salt: u32| {
        crate::planet::noise_at(body / wavelength, cfg.seed.wrapping_add(salt)) - 0.5
    };
    let relief = coarse + 27.0 * detail(410.0, 3901) + 8.0 * detail(155.0, 3907);
    // Fix the downstream control and blend it into the starting surface.
    let (width, reach) = if material.rock_package.is_some_and(|rock| rock.jointed()) {
        (88.0, 150.0)
    } else {
        (OUTLET_HALF_WIDTH_M, 190.0)
    };
    let across = (x - SPAN_M * 0.5) / width;
    let outlet_weight = (-across.powi(2) - ((SPAN_M - z) / reach).powi(2)).exp();
    let target = (site.outlet_m - material.outlet_drop_m).max(SEA + 0.5);
    (relief + outlet_weight * (target - relief), d)
}

/// Inherited landforms are part of the initial bed, before the sediment
/// budget starts. Widen the drainage skeleton in physical space so a
/// tributary has a head, opposing slopes and room for an interfluve.
struct InheritedRelief {
    grid: Grid,
    offset: Vec<f32>,
    steepness: Vec<f32>,
    rills: Option<(Grid, Vec<f32>)>,
}

enum Landforms {
    Inherited(InheritedRelief),
    Rock(structural::Terrain),
}

impl Landforms {
    fn build(cfg: &WorldSampleConfig, world: &World, site: Site, material: Material) -> Self {
        if let Some(package) = material.rock_package {
            return Self::Rock(structural::Terrain::build(
                cfg, world, site, material, package,
            ));
        }
        Self::Inherited(InheritedRelief::build(cfg, world, site, material))
    }

    fn height(&self, material: Material, base: f64, x: f64, z: f64) -> f64 {
        match self {
            Self::Inherited(relief) => relief.height(material, base, x, z),
            Self::Rock(terrain) => terrain.height(x, z),
        }
    }

    fn cover(&self, material: Material, x: f64, z: f64) -> f64 {
        match self {
            Self::Inherited(_) => material.soil_m,
            Self::Rock(terrain) => terrain.cover(x, z),
        }
    }
}

impl InheritedRelief {
    fn build(cfg: &WorldSampleConfig, world: &World, site: Site, material: Material) -> Self {
        let grid = Grid {
            n: 500,
            cell_m: 8.0,
            min_m: [0.0; 2],
        };
        let anchor: Vec<_> = (0..grid.len())
            .map(|i| {
                let [x, z] = grid.at(i);
                base_height(cfg, world, site, material, x, z).0 as f32
            })
            .collect();
        let parent: Vec<_> = anchor
            .iter()
            .enumerate()
            .map(|(i, &h)| {
                if material.upland_gain == 0.0 {
                    return h;
                }
                let p = DVec2::from_array(grid.at(i));
                let shoulder = SHOULDER_AXES
                    .into_iter()
                    .flat_map(|axis| [axis, -axis])
                    .map(|axis| {
                        let at = p + 128.0 * axis;
                        grid.bilinear(&anchor, at.x, at.y)
                    })
                    .sum::<f64>()
                    / 8.0;
                let above_floor = f64::from(h) - site.outlet_m;
                let relief = (shoulder - site.outlet_m - 10.0).max(0.0);
                let gate = smooth_ramp((above_floor - 15.0) / 44.0);
                let edge = smooth_ramp(inset_m(p.x, p.y) / 480.0);
                h + (material.upland_gain * relief * gate * edge) as f32
            })
            .collect();
        let terrain: Vec<_> = (0..grid.len())
            .map(|i| slope_features(grid, &parent, i, cfg.seed, material))
            .collect();
        let height: Vec<_> = parent
            .iter()
            .zip(&terrain)
            .map(|(&h, &d)| f64::from(h) + d)
            .collect();
        let fixed: Vec<_> = (0..grid.len()).map(|i| grid.on_border(i)).collect();
        let flow = erosion::route(&grid, &height, &fixed, &vec![1.0; grid.len()]);
        let mut cut = vec![0.0_f32; grid.len()];
        for (i, &area) in flow.area.iter().enumerate() {
            let r = flow.receiver[i] as usize;
            let minimum_area = if material.varied_bedding {
                1050.0
            } else {
                2000.0
            };
            if i == r || area < minimum_area || flow.spill[i] > height[i] + 0.15 {
                continue;
            }
            let p = DVec2::from_array(grid.at(i));
            let mut downstream = r;
            let mut length = p.distance(DVec2::from_array(grid.at(r)));
            while length < 55.0 {
                let next = flow.receiver[downstream] as usize;
                if next == downstream {
                    break;
                }
                length += DVec2::from_array(grid.at(downstream))
                    .distance(DVec2::from_array(grid.at(next)));
                downstream = next;
            }
            let slope = ((height[i] - height[downstream]) / length).max(0.0);
            let grade = (slope / 0.15).clamp(0.0, 1.0).sqrt();
            // Fractured zones dissect earlier; coherent rock needs more
            // contributing area. Leave established trunk channels alone.
            let weakness = if material.varied_bedding {
                fracture_weakness(p.x, p.y) * (1.0 - area / 75_000.0).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let threshold = 2000.0 - 950.0 * weakness;
            let head = ((area - threshold) / 6000.0).clamp(0.0, 1.0);
            let size = (area / 45_000.0).powf(0.24).clamp(0.30, 1.65);
            let depth = material.ravine_depth_m * size * grade * head * (1.0 + 0.42 * weakness);
            let width = material.ravine_width_m * size.sqrt() * (1.0 - 0.20 * weakness);
            if depth < 0.2 {
                continue;
            }
            let radius = (width / grid.cell_m).ceil() as usize + 1;
            let (x, z) = (i % grid.n, i / grid.n);
            let end = DVec2::from_array(grid.at(r));
            let segment = end - p;
            for v in z.saturating_sub(radius)..=(z + radius).min(grid.n - 1) {
                for u in x.saturating_sub(radius)..=(x + radius).min(grid.n - 1) {
                    let j = v * grid.n + u;
                    let q = DVec2::from_array(grid.at(j));
                    let t = ((q - p).dot(segment) / segment.length_squared()).clamp(0.0, 1.0);
                    let radial = q.distance_squared(p + t * segment) / width.powi(2);
                    if radial >= 1.0 {
                        continue;
                    }
                    let shoulder = (1.0 - radial).powi(2);
                    // A narrow inner cut and a broad shoulder follow the
                    // same receiver segment. Both widths are in metres.
                    let inner = (1.0 - radial / 0.38_f64.powi(2)).max(0.0).powi(2);
                    let cross_section = shoulder + material.inner_gorge * (inner - shoulder);
                    cut[j] = cut[j].max((depth * cross_section) as f32);
                }
            }
        }
        let dissected: Vec<_> = height
            .iter()
            .zip(&cut)
            .map(|(&h, &cut)| (h - f64::from(cut)) as f32)
            .collect();
        let summits = summit_lift(grid, &dissected, site.outlet_m, material.summit_lift_m);
        let rills = (material.rill_relief_m > 0.0)
            .then(|| fine_rills(grid, &dissected, &cut, cfg.seed, material.rill_relief_m));
        let mut offset = cut;
        for (i, value) in offset.iter_mut().enumerate() {
            let [x, z] = grid.at(i);
            let crest = crest_lift(grid, &dissected, i) * material.crest_lift_m;
            let shaped = terrain[i] - f64::from(*value) + crest + f64::from(summits[i]);
            *value = parent[i] - anchor[i] + (shaped * smooth_ramp(inset_m(x, z) / 180.0)) as f32;
        }
        let steepness = (0..grid.len())
            .map(|i| {
                let elevation = |j: usize| f64::from(anchor[j]) + f64::from(offset[j]);
                grid.neighbours(i)
                    .map(|(j, d)| ((elevation(i) - elevation(j)) / d).abs())
                    .fold(0.0, f64::max) as f32
            })
            .collect();
        Self {
            grid,
            offset,
            steepness,
            rills,
        }
    }

    fn height(&self, material: Material, base: f64, x: f64, z: f64) -> f64 {
        let rills = self
            .rills
            .as_ref()
            .map_or(0.0, |(grid, field)| grid.bilinear(field, x, z));
        let relief = base + self.grid.bilinear(&self.offset, x, z) + rills;
        let steepness = self.grid.bilinear(&self.steepness, x, z);
        let ledge = ((steepness - 0.17) / 0.55).clamp(0.0, 1.0);
        let phase = bedding_phase(material, relief, x, z);
        // Flatten the resistant layer and steepen the softer interval.
        let intact = if material.varied_bedding {
            1.0 - 0.68 * fracture_weakness(x, z)
        } else {
            1.0
        };
        let relief_shift = -2.75 * intact * ((phase - 0.45) * std::f64::consts::TAU).sin();
        let edge = inset_m(x, z);
        relief + relief_shift * material.bed_contrast * ledge * (edge / 180.0).clamp(0.0, 1.0)
    }
}

fn inset_m(x: f64, z: f64) -> f64 {
    x.min(z).min(SPAN_M - x).min(SPAN_M - z)
}

fn smooth_ramp(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Sample narrow runnels along the local fall line on a common 4 m grid.
/// Fade across valley bottoms and into the open edges of the review window.
fn fine_rills(
    parent: Grid,
    height: &[f32],
    cut: &[f32],
    seed: u32,
    amplitude: f64,
) -> (Grid, Vec<f32>) {
    let grid = Grid {
        n: 1000,
        cell_m: 4.0,
        min_m: [0.0; 2],
    };
    let sample = |p: DVec2| parent.bilinear(height, p.x, p.y);
    let noise = |p: DVec2, wavelength: f64, salt: u32| {
        let rotated = DVec3::new(0.92 * p.x - 0.39 * p.y, 43.0, 0.39 * p.x + 0.92 * p.y);
        crate::planet::noise_at(rotated / wavelength, seed.wrapping_add(salt)) - 0.5
    };
    let field = (0..grid.len())
        .map(|i| {
            let [x, z] = grid.at(i);
            let p = DVec2::new(x, z);
            let gradient = DVec2::new(
                sample(p + 16.0 * DVec2::X) - sample(p - 16.0 * DVec2::X),
                sample(p + 16.0 * DVec2::Y) - sample(p - 16.0 * DVec2::Y),
            ) / 32.0;
            let grade = gradient.length();
            let slope = smooth_ramp((grade - 0.09) / 0.36);
            let floor = 1.0 - 0.83 * smooth_ramp(parent.bilinear(cut, x, z) / 11.0);
            if slope == 0.0 {
                return 0.0;
            }
            let along = gradient / grade * 17.0;
            let rill = 0.5 * noise(p, 22.0, 5113)
                + 0.25 * (noise(p + along, 22.0, 5113) + noise(p - along, 22.0, 5113));
            let shoulder = (noise(p + along, 49.0, 5107) + noise(p - along, 49.0, 5107)) * 0.5;
            (amplitude
                * slope
                * floor
                * smooth_ramp(inset_m(x, z) / 160.0)
                * (0.65 * rill + 0.35 * shoulder)) as f32
        })
        .collect();
    (grid, field)
}

const SHOULDER_AXES: [DVec2; 4] = [
    DVec2::X,
    DVec2::Y,
    DVec2::new(
        std::f64::consts::FRAC_1_SQRT_2,
        std::f64::consts::FRAC_1_SQRT_2,
    ),
    DVec2::new(
        std::f64::consts::FRAC_1_SQRT_2,
        -std::f64::consts::FRAC_1_SQRT_2,
    ),
];

/// Both sides of a crest fall away. Measure that relief across the ridge
/// at a fixed distance so planar slopes and channel bottoms stay quiet.
fn crest_lift(grid: Grid, height: &[f32], i: usize) -> f64 {
    let p = DVec2::from_array(grid.at(i));
    let h = f64::from(height[i]);
    let shoulder = |offset: DVec2| grid.bilinear(height, p.x + offset.x, p.y + offset.y);
    let drop = SHOULDER_AXES
        .into_iter()
        .map(|axis| h - shoulder(40.0 * axis).max(shoulder(-40.0 * axis)))
        .fold(0.0, f64::max);
    let strength = ((drop - 2.5) / 15.0).clamp(0.0, 1.0);
    strength * strength * (3.0 - 2.0 * strength)
}

/// Raise existing local summits over their shoulders with compact caps.
fn summit_lift(grid: Grid, height: &[f32], outlet_m: f64, lift_m: f64) -> Vec<f32> {
    const SEARCH_M: f64 = 56.0;
    const SUPPORT_M: f64 = 112.0;
    const TIP_M: f64 = 12.0;
    let search = (SEARCH_M / grid.cell_m).ceil() as usize;
    let support = (SUPPORT_M / grid.cell_m).ceil() as usize;
    let mut peaks = Vec::new();
    for z in support..grid.n - support {
        for x in support..grid.n - support {
            let i = z * grid.n + x;
            let h = height[i];
            if f64::from(h) < outlet_m + 28.0 || grid.neighbours(i).any(|(j, _)| height[j] >= h) {
                continue;
            }
            let higher = (z - search..=z + search).any(|v| {
                (x - search..=x + search).any(|u| {
                    u.abs_diff(x).pow(2) + v.abs_diff(z).pow(2) <= search.pow(2)
                        && height[v * grid.n + u] > h
                })
            });
            if higher {
                continue;
            }
            let p = DVec2::from_array(grid.at(i));
            let shoulder_mean = SHOULDER_AXES
                .into_iter()
                .flat_map(|axis| [axis, -axis])
                .map(|axis| {
                    let at = p + SUPPORT_M * axis;
                    grid.bilinear(height, at.x, at.y)
                })
                .sum::<f64>()
                / 8.0;
            let prominence = f64::from(h) - shoulder_mean;
            if prominence >= 11.0 {
                peaks.push((i, prominence));
            }
        }
    }
    peaks.sort_unstable_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    let mut accepted = Vec::<DVec2>::new();
    let mut lift = vec![0.0_f32; grid.len()];
    for (i, prominence) in peaks {
        let p = DVec2::from_array(grid.at(i));
        if accepted
            .iter()
            .any(|&q| p.distance_squared(q) < (SUPPORT_M * 1.45).powi(2))
        {
            continue;
        }
        accepted.push(p);
        let strength = (prominence / 37.0).clamp(0.30, 1.0) * lift_m;
        let (x, z) = (i % grid.n, i / grid.n);
        for v in z - support..=z + support {
            for u in x - support..=x + support {
                let j = v * grid.n + u;
                let distance =
                    (p.distance_squared(DVec2::from_array(grid.at(j))) + TIP_M.powi(2)).sqrt();
                let weight = ((SUPPORT_M - distance) / (SUPPORT_M - TIP_M)).max(0.0);
                lift[j] = lift[j].max((strength * weight.powi(2)) as f32);
            }
        }
    }
    lift
}

/// Elongate small variations along continuous fall lines. Adjacent
/// streamlines then carry coherent spurs instead of tracing the eight grid
/// directions. Slope gating leaves flats and floodplains relatively quiet.
fn slope_features(grid: Grid, height: &[f32], i: usize, seed: u32, material: Material) -> f64 {
    let sample = |p: DVec2| grid.bilinear(height, p.x, p.y);
    let gradient = |p: DVec2| {
        DVec2::new(
            sample(p + 24.0 * DVec2::X) - sample(p - 24.0 * DVec2::X),
            sample(p + 24.0 * DVec2::Y) - sample(p - 24.0 * DVec2::Y),
        ) / 48.0
    };
    let p = DVec2::from_array(grid.at(i));
    let slope = gradient(p).length();
    let weight = ((slope - 0.025) / 0.24).clamp(0.0, 1.0);
    if weight == 0.0 {
        return 0.0;
    }
    let noise = |p: DVec2, scale: f64, salt: u32| {
        let pos = DVec3::new(0.82 * p.x - 0.57 * p.y, 31.0, 0.57 * p.x + 0.82 * p.y);
        crate::planet::noise_at(pos / scale, seed.wrapping_add(salt)) - 0.5
    };
    let mut broad = 2.0 * noise(p, 130.0, 4973);
    let mut fine = 2.0 * noise(p, 47.0, 4987);
    for sign in [-1.0, 1.0] {
        let mut at = p;
        for step in 0..4 {
            let g = gradient(at);
            let midpoint = at + sign * 17.0 * g / g.length().max(0.055);
            let g = gradient(midpoint);
            at += sign * 34.0 * g / g.length().max(0.055);
            broad += noise(at, 130.0, 4973);
            if step == 0 {
                fine += noise(at, 47.0, 4987);
            }
        }
    }
    let broad = broad / 10.0;
    let fine = fine / 4.0;
    let fine_weight = 0.27 + 0.014 * material.rill_relief_m;
    material.spur_relief_m * weight * (broad + fine_weight * fine).clamp(-0.31, 0.31)
}

#[derive(Default)]
pub struct Budget {
    pub initial_soil_m3: f64,
    pub refinement_soil_m3: f64,
    pub eroded_rock_m3: f64,
    pub exported_sediment_m3: f64,
    pub final_soil_m3: f64,
    pub rainfall_m3_year: f64,
    pub exported_runoff_m3_year: f64,
}

pub struct Catchment {
    pub frame: Frame,
    pub spacing_m: f64,
    pub side: usize,
    pub outlet_m: f64,
    pub parent_runoff_area_m2: f64,
    pub initial_m: Vec<f32>,
    pub bedrock_m: Vec<f32>,
    pub sediment_m: Vec<f32>,
    pub runoff_m3_year: Vec<f64>,
    pub outlet_catchment: Vec<bool>,
    trails: Option<explore::Trails>,
    valley_deposits: Option<Vec<u8>>,
    pub budget: Budget,
    pub history_water_m3_year: Option<[f64; 3]>,
    pub history_sediment_m3: Option<[f64; 5]>,
    pub history_wind_m3: Option<[f64; 3]>,
    pub history_process_m3: Option<[f64; 3]>,
    upland_rock_m3: f64,
    rock_package: Option<structural::RockPackage>,
}

#[derive(Debug)]
pub struct Column {
    pub height_m: f64,
    pub sediment_m: f64,
    pub runoff_m3_year: f64,
}

impl Catchment {
    pub fn trail_points(&self) -> Option<[Vec<DVec2>; 2]> {
        Some(self.trails.as_ref()?.chart(self.frame))
    }
    pub fn rock_contacts(&self, x: f64, z: f64) -> Option<[f64; 5]> {
        self.rock_package.map(|p| p.contacts(x, z))
    }

    pub fn rock_unit_at(&self, x: f64, y: f64, z: f64) -> Option<&'static str> {
        self.rock_package.map(|p| p.rock(x, y, z).name)
    }

    /// Sample the same surface at any voxel spacing. Discharge belongs to a
    /// discrete receiver, so use the containing cell for that field.
    pub fn sample(&self, chart: DVec2) -> Option<Column> {
        let p = self.frame.local(chart);
        let grid = Grid {
            n: self.side,
            cell_m: self.spacing_m,
            min_m: [0.0; 2],
        };
        if !grid.contains(p.x, p.y) {
            return None;
        }
        let sediment_m = grid.bilinear(&self.sediment_m, p.x, p.y);
        let i = (p.y / self.spacing_m) as usize * self.side + (p.x / self.spacing_m) as usize;
        Some(Column {
            height_m: grid.bilinear(&self.bedrock_m, p.x, p.y) + sediment_m,
            sediment_m,
            runoff_m3_year: self.runoff_m3_year[i],
        })
    }
}

/// Both sites use the same solver and time schedule. The 2 m / half-step
/// rerun adds a spatial refinement level and halves each evolution step.
pub fn solve_pair(cfg: &WorldSampleConfig, finer: bool) -> Option<[Catchment; 2]> {
    solve_pair_with(cfg, finer, false, false)
}

/// One offline canyon cut into a tilted sequence of mapped rock units.
pub fn solve_rock_canyon(
    cfg: &WorldSampleConfig,
    finer: bool,
    half_step: bool,
) -> Option<Catchment> {
    solve_rock_canyon_with_weathering(cfg, finer, half_step, false)
}

pub fn solve_rock_canyon_with_weathering(
    cfg: &WorldSampleConfig,
    finer: bool,
    half_step: bool,
    weathering: bool,
) -> Option<Catchment> {
    if !active(cfg) {
        return None;
    }
    let world = get(cfg);
    let [site, _] = sites(cfg, &world)?;
    Some(solve(cfg, &world, site, Material { weathering, ..mapped_rock(site) }, finer, half_step))
}

pub fn solve_jointed_canyon(cfg: &WorldSampleConfig, finer: bool, half_step: bool) -> Option<Catchment> {
    solve_jointed_canyon_with_routes(cfg, finer, half_step, false)
}

pub fn solve_jointed_canyon_with_routes(cfg: &WorldSampleConfig, finer: bool, half_step: bool, exploration: bool) -> Option<Catchment> {
    solve_jointed_profile(cfg, finer, half_step, exploration, false, None)
}

pub fn solve_sediment_weathering_canyon(cfg: &WorldSampleConfig, finer: bool, half_step: bool) -> Option<Catchment> {
    solve_jointed_profile(cfg, finer, half_step, false, true, None)
}

pub fn solve_valley_form(cfg: &WorldSampleConfig, finer: bool, half_step: bool) -> Option<Catchment> {
    solve_jointed_profile(cfg, finer, half_step, true, false, Some(ValleyForm::Shoulders))
}

pub fn solve_sill_erosion(cfg: &WorldSampleConfig, finer: bool, half_step: bool) -> Option<Catchment> {
    solve_jointed_profile(cfg, finer, half_step, true, false, Some(ValleyForm::Sills))
}

enum ValleyForm { Shoulders, Sills }

fn solve_jointed_profile(cfg: &WorldSampleConfig, finer: bool, half_step: bool, exploration: bool, coupled: bool, valley: Option<ValleyForm>) -> Option<Catchment> {
    if !active(cfg) {
        return None;
    }
    let world = get(cfg);
    let [site, _] = sites(cfg, &world)?;
    let material = Material {
        exploration,
        valley_form: valley.is_some(),
        sill_erosion: matches!(valley, Some(ValleyForm::Sills)),
        ..jointed_rock(mapped_rock(site), cfg.seed)
    };
    let material = if coupled { sediment_weathering(material) } else { material };
    Some(solve(cfg, &world, site, material, finer, half_step))
}

/// Replay the final valley pass on the exported bed and sediment from `--explore`.
/// This pass uses physical area and the routed bed, so rainfall need not be replayed.
pub fn replay_valley(rock: Vec<f32>, soil: Vec<f32>, spacing: f64, datum: f64, seed: u32, sills: bool) -> Option<Vec<f32>> {
    if ![2.0, 4.0].contains(&spacing) || !datum.is_finite() { return None; }
    let grid = Grid { n: (SPAN_M / spacing) as usize, cell_m: spacing, min_m: [0.0; 2] };
    let len = grid.len();
    if rock.len() != len || soil.len() != len || rock.iter().chain(&soil).any(|v| !v.is_finite()) { return None; }
    let mut stage = Stage {
        grid, rock, soil, base: Vec::new(), runoff: Vec::new(), rain: vec![1.0; len],
        fixed: (0..len).map(|i| grid.on_border(i)).collect(),
        height: vec![0.0; len], flux: vec![0.0; len], flood_rank: vec![0; len],
    };
    let flow = stage.route();
    let material = jointed_rock(Material {
        sill_erosion: sills, rock_package: Some(structural::RockPackage::new(datum)), ..LAYERED
    }, seed);
    valley_form::evolve(&mut stage, &flow, material, seed, &mut Budget::default());
    Some(stage.rock.into_iter().zip(stage.soil).map(|(r, s)| r + s).collect())
}

pub fn solve_pair_with(
    cfg: &WorldSampleConfig,
    finer: bool,
    extreme: bool,
    half_step: bool,
) -> Option<[Catchment; 2]> {
    if !active(cfg) {
        return None;
    }
    let world = get(cfg);
    let [canyon, valley] = sites(cfg, &world)?;
    Some([
        solve(
            cfg,
            &world,
            canyon,
            if extreme { GORGE } else { LAYERED },
            finer,
            half_step,
        ),
        solve(cfg, &world, valley, SOIL_COVERED, finer, half_step),
    ])
}

struct Stage {
    grid: Grid,
    base: Vec<f32>,
    rock: Vec<f32>,
    soil: Vec<f32>,
    rain: Vec<f64>,
    fixed: Vec<bool>,
    height: Vec<f64>,
    flux: Vec<f64>,
    runoff: Vec<f64>,
    flood_rank: Vec<u32>,
}

struct Descent {
    targets: [(usize, f64); 8],
    count: usize,
    slope: f64,
    fall: f64,
}

impl Descent {
    fn shares(&self) -> &[(usize, f64)] {
        &self.targets[..self.count]
    }
}

fn descent(
    grid: Grid,
    height: &[f64],
    flow: &erosion::Flow,
    rank: &[u32],
    i: usize,
    q: f64,
) -> Descent {
    let mut result = Descent {
        targets: [(0, 0.0); 8],
        count: 0,
        slope: 0.0,
        fall: 0.0,
    };
    let mut total = 0.0;
    // Split shallow flow among downhill neighbours. Stronger flow favours
    // the steepest path as it collects in a channel.
    let concentration = (q / 45_000.0).min(1.0);
    if flow.spill[i] <= height[i] + 0.001 {
        for (j, distance) in grid.neighbours(i) {
            let drop = height[i] - height[j];
            // Filled flats have a routing gradient that the physical spill
            // height does not record. Use the actual flood order.
            if drop <= 0.0 || rank[j] >= rank[i] {
                continue;
            }
            let slope = drop / distance;
            let squared = slope * slope;
            let weight = squared * (1.0 - concentration + concentration * squared);
            result.targets[result.count] = (j, weight);
            result.count += 1;
            total += weight;
            result.slope += slope * weight;
            result.fall = result.fall.max(drop);
        }
    }
    if result.count == 0 {
        // The flood receiver carries water across flats and depressions.
        result.targets[0] = (flow.receiver[i] as usize, 1.0);
        result.count = 1;
    } else {
        result.slope /= total;
        for (_, weight) in &mut result.targets[..result.count] {
            *weight /= total;
        }
    }
    result
}

impl Stage {
    fn new(
        cfg: &WorldSampleConfig,
        world: &World,
        site: Site,
        material: Material,
        landforms: &Landforms,
        spacing: f64,
        previous: Option<&Self>,
        budget: &mut Budget,
    ) -> Self {
        let grid = Grid {
            n: (SPAN_M / spacing) as usize,
            cell_m: spacing,
            min_m: [0.0; 2],
        };
        let len = grid.len();
        let mut stage = Self {
            grid,
            base: Vec::with_capacity(len),
            rock: Vec::with_capacity(len),
            soil: Vec::with_capacity(len),
            rain: Vec::with_capacity(len),
            fixed: Vec::with_capacity(len),
            height: vec![0.0; len],
            flux: vec![0.0; len],
            runoff: vec![0.0; len],
            flood_rank: vec![0; len],
        };
        for i in 0..len {
            let [x, z] = grid.at(i);
            let (base, d) = base_height(cfg, world, site, material, x, z);
            let base = landforms.height(material, base, x, z);
            // Small perturbations break ties after refinement. The amplitude
            // drops with cell spacing, and drainage chooses the channels.
            let perturbation = (crate::planet::noise_at(
                DVec3::new(x, 0.0, z) / (spacing * 5.0),
                cfg.seed.wrapping_add(spacing as u32 * 37),
            ) - 0.5)
                * spacing
                * 0.035;
            let base = (base + perturbation) as f32;
            let fixed = grid.on_border(i);
            let (rock, soil) = if fixed {
                (base, 0.0)
            } else if let Some(old) = previous {
                let residual =
                    old.grid.bilinear(&old.rock, x, z) - old.grid.bilinear(&old.base, x, z);
                (
                    (f64::from(base) + residual) as f32,
                    old.grid.bilinear(&old.soil, x, z) as f32,
                )
            } else {
                let cover = landforms.cover(material, x, z) as f32;
                (base - cover, cover)
            };
            let wetness = world.mesh.sample(&world.rain, d);
            stage.base.push(base);
            stage.rock.push(rock);
            stage.soil.push(soil);
            stage.rain.push(0.18 + 1.2 * wetness);
            stage.fixed.push(fixed);
        }
        let volume = stage.soil_volume();
        if let Some(old) = previous {
            budget.refinement_soil_m3 += volume - old.soil_volume();
        } else {
            budget.initial_soil_m3 = volume;
        }
        stage
    }

    fn soil_volume(&self) -> f64 {
        self.soil.iter().map(|&v| f64::from(v)).sum::<f64>() * self.grid.cell_m.powi(2)
    }

    fn route(&mut self) -> erosion::Flow {
        for (i, h) in self.height.iter_mut().enumerate() {
            *h = f64::from(self.rock[i]) + f64::from(self.soil[i]);
        }
        let flow = erosion::route(&self.grid, &self.height, &self.fixed, &self.rain);
        for (rank, &i) in flow.order.iter().enumerate() {
            self.flood_rank[i as usize] = rank as u32;
        }
        flow
    }

    fn seed_runoff(&mut self) {
        let area = self.grid.cell_m.powi(2);
        for (q, rain) in self.runoff.iter_mut().zip(&self.rain) {
            *q = rain * area;
        }
    }

    fn final_runoff(&mut self, flow: &erosion::Flow) {
        self.seed_runoff();
        for &i in flow.order.iter().rev() {
            let i = i as usize;
            if self.fixed[i] {
                continue;
            }
            let q = self.runoff[i];
            for &(j, share) in
                descent(self.grid, &self.height, flow, &self.flood_rank, i, q).shares()
            {
                self.runoff[j] += q * share;
            }
        }
    }

    fn step(&mut self, material: Material, dt: f64, budget: &mut Budget) {
        let flow = self.route();
        let spacing = self.grid.cell_m;
        let cell_area = spacing.powi(2);
        let transport_scale = (spacing / 8.0).sqrt();
        let settle = 1.0 - (-spacing / 70.0).exp();
        self.flux.fill(0.0);
        self.seed_runoff();
        for &i in flow.order.iter().rev() {
            let i = i as usize;
            let r = flow.receiver[i] as usize;
            let arriving = self.flux[i];
            if i == r {
                budget.exported_sediment_m3 += arriving;
                continue;
            }
            let q = self.runoff[i];
            let path = descent(self.grid, &self.height, &flow, &self.flood_rank, i, q);
            let slope = path.slope.min(0.65);
            let capacity = q.sqrt() * slope * dt * transport_scale;
            let available = (capacity - arriving).max(0.0);
            let old_soil = f64::from(self.soil[i]);
            let soil_cut = old_soil.min(available / cell_area).min(path.fall * 0.45);
            let cover = (-(old_soil - soil_cut) / 1.8).exp();
            let [x, z] = self.grid.at(i);
            let old_rock = f64::from(self.rock[i]);
            let rock_rate = material.rock_erodibility
                * bedding(material, old_rock, x, z)
                * cover
                * q.min(90_000.0).sqrt()
                * slope;
            let cut = (rock_rate * dt)
                .min((available / cell_area - soil_cut).max(0.0))
                .min((path.fall * 0.60 - soil_cut).max(0.0));
            self.rock[i] = (old_rock - cut) as f32;
            let rock_cut = old_rock - f64::from(self.rock[i]);
            budget.eroded_rock_m3 += rock_cut * cell_area;
            // Settle over a physical reach length, so a single flat cell
            // cannot instantly trap the load of the entire upstream basin.
            let deposit = (arriving - capacity).max(0.0) * settle;
            self.soil[i] = (old_soil - soil_cut + deposit / cell_area) as f32;
            // Use the stored difference so routing and the audit agree after rounding.
            let outgoing =
                (arriving + (rock_cut + old_soil - f64::from(self.soil[i])) * cell_area).max(0.0);
            for &(j, share) in path.shares() {
                self.runoff[j] += q * share;
                self.flux[j] += outgoing * share;
            }
        }
        self.hillslopes(material, dt, budget);
    }

    fn hillslopes(&mut self, material: Material, dt: f64, budget: &mut Budget) {
        self.flux.fill(0.0);
        for (i, h) in self.height.iter_mut().enumerate() {
            *h = f64::from(self.rock[i]) + f64::from(self.soil[i]);
        }
        let cell_area = self.grid.cell_m.powi(2);
        for i in 0..self.grid.len() {
            if self.fixed[i] {
                continue;
            }
            let mut low = (i, 0.0);
            let mut bank_slope: f64 = 0.0;
            for (j, d) in self.grid.neighbours(i) {
                let slope = (self.height[i] - self.height[j]) / d;
                if slope > low.1 {
                    low = (j, slope);
                }
                if self.runoff[j] >= CHANNEL_RUNOFF && self.runoff[i] < self.runoff[j] * 0.2 {
                    bank_slope = bank_slope.max(slope);
                }
            }
            let (r, slope) = low;
            if r == i {
                continue;
            }
            let rock = f64::from(self.rock[i]);
            let soil = f64::from(self.soil[i]);
            let [x, z] = self.grid.at(i);
            // Channel undercutting weathers exposed banks separately from
            // bed incision. The resulting debris joins hillslope transport.
            let bank_limit = material
                .rock_package
                .map_or(material.critical_slope * 0.7, |p| {
                    (p.rock(x, rock, z).wall_grade * 0.63).clamp(0.35, 2.25)
                });
            let weathered = ((bank_slope - bank_limit).max(0.0)
                * bedding(material, rock, x, z)
                * (-soil / 0.9).exp()
                * 0.075
                * dt)
                .min((self.height[i] - self.height[r]) * 0.18);
            self.rock[i] = (rock - weathered) as f32;
            let produced = rock - f64::from(self.rock[i]);
            budget.eroded_rock_m3 += produced * cell_area;
            let mobility = (1.0 - (slope / material.critical_slope).powi(2)).max(0.16);
            let fraction = (material.diffusivity * dt / (cell_area * mobility)).min(0.24);
            let moved = (fraction * (self.height[i] - self.height[r])).min(soil + produced);
            self.flux[i] += produced - moved;
            if self.fixed[r] {
                budget.exported_sediment_m3 += moved * cell_area;
            } else {
                self.flux[r] += moved;
            }
        }
        for (soil, delta) in self.soil.iter_mut().zip(&self.flux) {
            *soil = (f64::from(*soil) + delta).max(0.0) as f32;
        }
    }
}

fn bedding(material: Material, rock: f64, x: f64, z: f64) -> f64 {
    if let Some(package) = material.rock_package {
        return package.rock(x, rock, z).erodibility;
    }
    let phase = bedding_phase(material, rock, x, z);
    let resistant = (1.0 - (phase - 0.45).abs() / 0.22).clamp(0.0, 1.0);
    1.0 - material.bed_contrast * resistant
}

/// Intersecting joint sets with slowly changing separation and strike.
fn fracture_weakness(x: f64, z: f64) -> f64 {
    let a = ((x + 0.43 * z + 34.0 * (z / 143.0).sin()) / 83.0)
        .sin()
        .abs();
    let b = ((z - 0.58 * x + 49.0 * (x / 192.0).sin()) / 117.0)
        .sin()
        .abs();
    (1.0 - 2.1 * a.min(b)).clamp(0.0, 1.0).powi(2)
}

fn bedding_phase(material: Material, rock: f64, x: f64, z: f64) -> f64 {
    let fold = (x / 410.0 + z / 680.0).sin() * 8.0;
    let tilted_height = rock - 0.031 * x + 0.017 * z - fold;
    // A monotone warp changes bed thickness without reversing the order
    // of the layers. The long lateral fold carries the same beds across
    // exposed walls and the incision calculation.
    let height = if material.varied_bedding {
        tilted_height
            + 9.0 * (tilted_height / 22.0).sin()
            + 3.0 * (tilted_height / 12.0 + x / 460.0 - z / 670.0).sin()
    } else {
        tilted_height
    };
    height.rem_euclid(19.0) / 19.0
}

fn solve(
    cfg: &WorldSampleConfig,
    world: &World,
    site: Site,
    material: Material,
    finer: bool,
    half_step: bool,
) -> Catchment {
    let landforms = Landforms::build(cfg, world, site, material);
    let mut budget = Budget::default();
    let mut previous = None;
    // The base schedule has 444 years. In the finer run the last stage's
    // duration is split between the 4 m and 2 m grids.
    let schedule = [
        (32.0, 12.0, 20),
        (16.0, 6.0, 20),
        (8.0, 3.0, 20),
        (4.0, 1.0, 24),
    ];
    for (spacing, dt, steps) in schedule {
        let mut stage = Stage::new(
            cfg,
            world,
            site,
            material,
            &landforms,
            spacing,
            previous.as_ref(),
            &mut budget,
        );
        let (dt, steps) = if finer && spacing == 4.0 {
            (dt * 0.5, steps)
        } else if finer {
            (dt * 0.5, steps * 2)
        } else {
            (dt, steps)
        };
        let (dt, steps) = if half_step {
            (dt * 0.5, steps * 2)
        } else {
            (dt, steps)
        };
        for _ in 0..steps {
            stage.step(material, dt, &mut budget);
        }
        previous = Some(stage);
    }
    let mut stage = previous.unwrap();
    if finer {
        stage = Stage::new(
            cfg,
            world,
            site,
            material,
            &landforms,
            2.0,
            Some(&stage),
            &mut budget,
        );
        let (dt, steps) = if half_step { (0.25, 48) } else { (0.5, 24) };
        for _ in 0..steps {
            stage.step(material, dt, &mut budget);
        }
    }
    let mut flow = stage.route();
    let mut valley_deposits = None;
    let trails = if material.exploration {
        let started = std::time::Instant::now();
        explore::dissect(&mut stage, &flow, &mut budget);
        flow = stage.route();
        if material.valley_form {
            valley_deposits = Some(valley_form::evolve(&mut stage, &flow, material, cfg.seed, &mut budget));
            flow = stage.route();
        }
        let trails = measurements::enclosed_outlet(stage.grid, &flow, 1.3, None).and_then(|outlet| {
            let terrain: Vec<_> = stage.height.iter().map(|&h| h as f32).collect();
            let centre = DVec2::from_array(stage.grid.at(outlet));
            let chart = site.frame.chart(centre.x, centre.y).as_vec2();
            let keepouts = crate::ruins::trail_keepouts(cfg, chart).into_iter().map(|p| (p, 300.0))
                .chain(crate::sites::trail_keepouts(cfg, chart).into_iter().map(|p| (p, 105.0)))
                .map(|(p, r)| (site.frame.local(p.as_dvec2()), r)).collect::<Vec<_>>();
            [DVec2::ZERO, DVec2::new(0.0, -192.0), DVec2::new(-160.0, -128.0), DVec2::new(160.0, -128.0)]
                .into_iter().find_map(|offset| explore::Trails::survey(stage.grid, &terrain, centre + offset, &keepouts))
        });
        eprintln!("[worldgen] connected_headwaters solve_ms={:.1} trails={}", started.elapsed().as_secs_f64() * 1000.0, trails.is_some());
        trails
    } else { None };
    let selected = outlet_mask(stage.grid, &flow);
    stage.final_runoff(&flow);
    budget.exported_runoff_m3_year = stage
        .runoff
        .iter()
        .zip(&stage.fixed)
        .filter_map(|(&q, &outlet)| outlet.then_some(q))
        .sum();
    budget.rainfall_m3_year = stage.rain.iter().sum::<f64>() * stage.grid.cell_m.powi(2);
    budget.final_soil_m3 = stage.soil_volume();
    Catchment {
        frame: site.frame,
        spacing_m: stage.grid.cell_m,
        side: stage.grid.n,
        outlet_m: (site.outlet_m - material.outlet_drop_m).max(SEA + 0.5),
        parent_runoff_area_m2: site.parent_runoff_area_m2,
        initial_m: stage.base,
        bedrock_m: stage.rock,
        sediment_m: stage.soil,
        runoff_m3_year: stage.runoff,
        outlet_catchment: selected,
        trails,
        valley_deposits,
        budget,
        history_water_m3_year: match &landforms {
            Landforms::Rock(terrain) => Some(terrain.water_balance()),
            Landforms::Inherited(_) => None,
        },
        history_sediment_m3: match &landforms {
            Landforms::Rock(terrain) => Some(terrain.sediment_balance()),
            Landforms::Inherited(_) => None,
        },
        history_wind_m3: match &landforms {
            Landforms::Rock(terrain) if material.weathering => Some(terrain.wind_balance()),
            _ => None,
        },
        history_process_m3: match &landforms {
            Landforms::Rock(terrain) if material.rock_package.is_some_and(|rock| rock.sediment_weathering) => Some(terrain.process_balance()),
            _ => None,
        },
        upland_rock_m3: match &landforms {
            Landforms::Rock(terrain) => terrain.upland_rock_m3(),
            Landforms::Inherited(_) => 0.0,
        },
        rock_package: material.rock_package,
    }
}
