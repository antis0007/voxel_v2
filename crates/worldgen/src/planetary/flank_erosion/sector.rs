//! Sector failure, fluvial incision and talus on volcanic bedrock.
use super::{SEA, smooth};
use crate::geo::grid::Grid;
use crate::planet::noise_at;
use crate::planetary::review::catchment::structural::mesh::Mesh;
use crate::planetary::slope_relief::{move_talus, transport::abrasion};
use crate::volcano::{Kind, Vent};
use glam::{DVec2, DVec3};

pub(super) const SUPPORT: f64 = 1.85;

struct Sector {
    origin: DVec2,
    downhill: DVec2,
    radius: f64,
    depth: f64,
}

impl Sector {
    fn choose(vent: Vent, origin: DVec2, key: u32, surface: &impl Fn(DVec2) -> (f64, f64, f64)) -> Self {
        let radius = vent.radius_m;
        let roll = f64::from(crate::hash_u32(key)) / f64::from(u32::MAX);
        // Score unsupported flanks from upper slope to toe. Prefer a single
        // coherent collapse facing a low valley or the open sea.
        let (downhill, fall) = (0..16).map(|i| {
            let (sin, cos) = ((f64::from(i) / 16.0 + roll) * std::f64::consts::TAU).sin_cos();
            let axis = DVec2::new(cos, sin);
            let upper = surface(origin + axis * radius * 0.52).0;
            let toe = surface(origin + axis * radius * 1.28).0;
            (axis, upper - toe)
        }).max_by(|a,b| a.1.total_cmp(&b.1)).unwrap();
        let mobility = if vent.kind == Kind::Shield { 0.72 + 0.28 * roll } else { 0.30 + 0.32 * roll };
        Self { origin, downhill, radius, depth: fall.clamp(0.0, 150.0) * mobility }
    }

    fn slide(&self, p: DVec2, height: f64) -> (f64, f64) {
        let p = (p - self.origin) / self.radius;
        let distance = p.dot(self.downhill);
        let across = self.downhill.perp_dot(p);
        let head = 0.37 + 2.15 * across * across;
        let wall = smooth(head - 0.02, head + 0.053, distance);
        let edge = 1.0 - smooth(0.32, 0.47 + 0.08 * distance.max(0.0), across.abs());
        let toe = 1.0 - smooth(1.05, 1.60, distance);
        let cut = self.depth.min((height - (SEA - 10.0)).max(0.0)) * wall * edge * toe;
        let deposit = smooth(1.01, 1.27, distance)
            * (1.0 - smooth(1.47, 1.75, distance))
            * (1.0 - smooth(0.27, 0.64, across.abs()))
            * (1.0 - smooth(SEA - 8.0, SEA + 33.0, height));
        (cut, deposit)
    }
}

struct Level {
    grid: Grid,
    incision: Vec<f32>,
    soil: Vec<f32>,
}

pub(super) fn evolve(
    sources: &[(Vent, DVec2)], seed: u32, reach: f64, surface: impl Fn(DVec2) -> (f64, f64, f64),
) -> (Grid, Vec<f32>, Vec<f32>) {
    let sectors: Vec<_> = sources.iter().map(|&(vent, origin)| {
        let key = seed ^ vent.dir.x.to_bits() as u32 ^ (vent.dir.z.to_bits() >> 32) as u32;
        Sector::choose(vent, origin, key, &surface)
    }).collect();
    let mut previous: Option<Level> = None;
    for (stage, (spacing, max_side, cycles, time)) in [
        (72.0, 64, 8, 11.5), (36.0, 128, 8, 7.0), (18.0, 256, 6, 4.5),
    ].into_iter().enumerate() {
        let side = ((2.0 * reach / spacing).ceil() as usize).clamp(32, max_side);
        let grid = Grid { n: side, cell_m: 2.0 * reach / side as f64, min_m: [-reach; 2] };
        let mesh = Mesh::new(grid);
        let mut base = Vec::with_capacity(grid.len());
        let mut weight = Vec::with_capacity(grid.len());
        let mut rain = Vec::with_capacity(grid.len());
        let mut resistance = Vec::with_capacity(grid.len());
        let mut limit = Vec::with_capacity(grid.len());
        for (i, &p) in mesh.points.iter().enumerate() {
            let (h, precipitation, age) = surface(p);
            base.push(h);
            let nearest = sources.iter().map(|(v, origin)| p.distance(*origin) / v.radius_m)
                .fold(f64::INFINITY, f64::min);
            weight.push(if grid.on_border(i) { 0.0 } else { 1.0 - smooth(1.58, SUPPORT, nearest) });
            let fabric = noise_at(DVec3::new(p.x / 290.0, h / 34.0, p.y / 108.0), seed);
            let weak = smooth(0.30, 0.71, fabric);
            rain.push((0.28 + 1.65 * precipitation) * (0.38 + 0.32 * age + 0.44 * weak));
            resistance.push(0.32 + 1.18 * weak + 0.30 * age);
            limit.push(0.75 + 1.18 * (1.0 - weak));
        }
        let mut initial = base.clone();
        let mut apron = vec![0.0; grid.len()];
        for sector in &sectors {
            let (mut volume, mut apron_area) = (0.0, 0.0);
            for (i, &p) in mesh.points.iter().enumerate() {
                let (cut, deposit) = sector.slide(p, initial[i]);
                let cut = cut * weight[i];
                apron[i] = deposit * weight[i];
                initial[i] -= cut;
                volume += cut * mesh.area[i];
                apron_area += apron[i] * mesh.area[i];
            }
            // Excess debris exits onto the shelf.
            let thickness = (volume / apron_area.max(1.0)).min(28.0);
            for (h, w) in initial.iter_mut().zip(&apron) { *h += thickness * w; }
        }
        let fixed: Vec<_> = initial.iter().zip(&weight).map(|(&h, &w)| w == 0.0 || h < SEA - 0.5).collect();
        let mut rock = initial.clone();
        let mut soil = vec![0.0; grid.len()];
        if let Some(old) = &previous {
            for (i, p) in mesh.points.iter().enumerate() {
                if !fixed[i] {
                    rock[i] += old.grid.bilinear(&old.incision, p.x, p.y);
                    soil[i] = old.grid.bilinear(&old.soil, p.x, p.y);
                }
            }
        }
        let mut height = vec![0.0; grid.len()];
        let mut load = vec![0.0; grid.len()];
        for _ in 0..cycles {
            for i in 0..height.len() { height[i] = rock[i] + soil[i]; }
            let flow = mesh.route(&height, &fixed, &rain);
            load.fill(0.0);
            for &node in flow.order.iter().rev() {
                let i = node as usize;
                if fixed[i] { continue; }
                let grade = flow.grade[i];
                let runoff = (flow.q[i] / 1100.0).sqrt();
                let shear = (runoff * grade - 0.105).max(0.0);
                let scoured = (0.70 * shear).min(soil[i]);
                soil[i] -= scoured;
                let area = mesh.area[i];
                let capacity = area * 3.4 * runoff * grade;
                let tool_cut = abrasion(load[i], capacity, area, soil[i], resistance[i], grade, grid.cell_m);
                let weathered = time * shear * resistance[i] * (-soil[i] / 1.15).exp();
                let incision = (weathered + tool_cut).min(grade * grid.cell_m * 0.58)
                    .min((rock[i] - (SEA - 3.0)).max(0.0));
                rock[i] -= incision;
                let sediment = load[i] + (scoured + incision) * area;
                let deposit = ((sediment - capacity).max(0.0) * 0.34).min(area * 1.1);
                soil[i] += deposit / area;
                let links = flow.links[i].as_slice();
                let total: f64 = links.iter().map(|link| f64::from(link.share)).sum();
                for link in links {
                    load[link.to as usize] += (sediment - deposit) * f64::from(link.share) / total;
                }
            }
            move_talus(&mesh, &fixed, &limit, &mut rock, &mut soil, &mut height, &mut load);
        }
        if stage == 2 {
            let change = mesh.regularize(|i| weight[i] * (rock[i] + soil[i] - base[i]));
            let exposed = mesh.regularize(|i| {
                weight[i] * smooth(3.0, 15.0, base[i] - rock[i]) * (1.0 - smooth(0.2, 1.65, soil[i]))
            });
            return (grid, change, exposed);
        }
        previous = Some(Level {
            grid,
            incision: mesh.regularize(|i| rock[i] - initial[i]),
            soil: mesh.regularize(|i| soil[i]),
        });
    }
    unreachable!()
}
