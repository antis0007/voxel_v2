//! Bounded, two-scale erosion of volcanic flanks in body-space metres.
use super::tectonics::smooth;
use super::volcanism::{Bins, CELL_M, bins, cell};
use super::SEA;
use crate::geo::{erosion, grid::Grid};
use crate::planet::noise_at;
use crate::volcano::{Kind, Vent};
use engine_core::cube_sphere::atlas::RADIUS_M;
use glam::{DVec2, DVec3};
mod sector;

struct Flank {
    centre: DVec3,
    summit_guard: f64,
    east: DVec3,
    north: DVec3,
    reach: f64,
    grid: Grid,
    change: Vec<f32>,
    exposed: Option<Vec<f32>>,
}

impl Flank {
    fn bake(
        vent: Vent,
        seed: u32,
        refined: bool,
        surface: &impl Fn(DVec3) -> (f64, f64, f64),
    ) -> Self {
        let east = DVec3::Y.cross(vent.dir).try_normalize().unwrap_or(DVec3::X);
        let north = vent.dir.cross(east);
        let reach = vent.radius_m * 1.55;
        let body = vent.dir * RADIUS_M;
        let mut previous: Option<(Grid, Vec<f32>)> = None;
        let mut change = Vec::new();
        let stages: &[(usize, usize, f64)] = if refined {
            &[(40, 8, 2.35), (80, 10, 1.7), (160, 8, 1.1)]
        } else {
            &[(40, 8, 1.65), (80, 10, 0.94)]
        };
        for (stage, &(n, steps, incision)) in stages.iter().enumerate() {
            let grid = Grid { n, cell_m: 2.0 * reach / n as f64, min_m: [-reach; 2] };
            let mut base = Vec::with_capacity(grid.len());
            let mut height = Vec::with_capacity(grid.len());
            let mut fixed = Vec::with_capacity(grid.len());
            let mut runoff = Vec::with_capacity(grid.len());
            let mut rock = Vec::with_capacity(grid.len());
            for i in 0..grid.len() {
                let [x, z] = grid.at(i);
                let dir = (body + x * east + z * north).normalize();
                let (h, rain, exposure) = surface(dir);
                let cover = noise_at(dir * RADIUS_M / 93.0, seed);
                let held = grid.on_border(i) || h < SEA - 1.0 || x.hypot(z) >= reach;
                base.push(h);
                height.push(previous.as_ref().filter(|_| !held).map_or(h, |(old, values)| {
                    let inherited = old.bilinear(values, x, z);
                    if refined { h + inherited } else { inherited }
                }));
                fixed.push(held);
                // Young porous flows infiltrate more of the same rainfall.
                let fabric = if refined {
                    // Porous younger flows absorb rainfall while older
                    // fractured flows and ash beds concentrate incision.
                    let body = dir * RADIUS_M;
                    let cross = east * body.dot(east);
                    noise_at(cross / 130.0 + (body - cross) / 340.0, seed.wrapping_add(43))
                } else { 0.5 };
                runoff.push((0.18 + rain * 1.15) * (0.42 + 0.58 * exposure)
                    * (1.0 + 0.55 * (fabric - 0.5)));
                rock.push((0.65 + 0.8 * cover) * (0.52 + 0.70 * exposure)
                    * (1.0 + 1.20 * (fabric - 0.5)));
            }
            let mut debris = vec![0.0; grid.len()];
            let mut area = vec![0.0; grid.len()];
            let mut rank = vec![0; grid.len()];
            for _ in 0..steps {
                let flow = erosion::route(&grid, &height, &fixed, &runoff);
                let cell_area = grid.cell_m.powi(2);
                for (a, r) in area.iter_mut().zip(&runoff) {
                    *a = r * cell_area;
                }
                for (order, &i) in flow.order.iter().enumerate() {
                    rank[i as usize] = order;
                }
                // Use the flood order for flats and distribute flow across
                // the descending neighbours on the exposed slopes.
                for &i in flow.order.iter().rev() {
                    let i = i as usize;
                    if fixed[i] { continue; }
                    let mut paths = [(0, 0.0); 8];
                    let mut count = 0;
                    let mut total = 0.0;
                    for (j, distance) in grid.neighbours(i) {
                        let slope = (height[i] - height[j]) / distance;
                        if slope > 0.0 && rank[j] < rank[i] {
                            let weight = slope.powf(1.3);
                            paths[count] = (j, weight);
                            count += 1;
                            total += weight;
                        }
                    }
                    if total > 0.0 {
                        for &(j, weight) in &paths[..count] {
                            area[j] += area[i] * weight / total;
                        }
                    } else {
                        let j = flow.receiver[i] as usize;
                        if i != j { area[j] += area[i]; }
                    }
                }
                for &i in &flow.order {
                    let i = i as usize;
                    if fixed[i] || flow.spill[i] > height[i] + 0.05 { continue; }
                    let j = flow.receiver[i] as usize;
                    let [x, z] = grid.at(i);
                    let [rx, rz] = grid.at(j);
                    let drop = (height[i] - height[j]).max(0.0);
                    let grade = drop / (x - rx).hypot(z - rz).max(grid.cell_m);
                    let drainage = ((area[i] - 2.0 * cell_area) / 1800.0)
                        .clamp(0.0, 30.0).powf(0.70);
                    let cut = (incision * rock[i] * drainage * grade.min(0.85).powf(1.3))
                        .min(drop * 0.55);
                    height[i] -= cut;
                    debris[i] += cut;
                }
            }
            let talus = if vent.kind == Kind::Shield { if refined { 0.63 } else { 0.43 } } else { 0.65 };
            erosion::relax_talus(&grid, &mut height, &fixed, talus, if n == 40 { 2 } else { 5 });
            let flow = erosion::route(&grid, &height, &fixed, &runoff);
            for &i in flow.order.iter().rev() {
                let i = i as usize;
                if fixed[i] { continue; }
                let j = flow.receiver[i] as usize;
                if j == i { continue; }
                let grade = (height[i] - height[j]).max(0.0) / grid.cell_m;
                let deposited = (debris[i] * 0.24 * (1.0 - smooth(0.04, 0.22, grade)))
                    .min(grid.cell_m * 0.04);
                height[i] += deposited;
                debris[j] += debris[i] - deposited;
            }
            if stage + 1 == stages.len() {
                change = height.iter().zip(&base).map(|(h, b)| (h - b) as f32).collect();
            }
            previous = Some((grid, height.into_iter().zip(base)
                .map(|(h, b)| if refined { (h - b) as f32 } else { h as f32 }).collect()));
        }
        let summit_guard = if vent.kind == Kind::Shield && vent.active { 330.0 } else { vent.radius_m * 0.16 };
        Self { centre: vent.dir, summit_guard, east, north, reach, grid: previous.unwrap().0, change, exposed: None }
    }

    fn bake_group(group: &[Vent], seed: u32, surface: &impl Fn(DVec3) -> (f64, f64, f64)) -> Self {
        let centre = group.iter().map(|vent| vent.dir).sum::<DVec3>().normalize();
        let east = DVec3::Y.cross(centre).try_normalize().unwrap_or(DVec3::X);
        let north = centre.cross(east);
        let sources: Vec<_> = group.iter().map(|&vent| {
            let point = DVec2::new(vent.dir.dot(east), vent.dir.dot(north)) * (RADIUS_M / vent.dir.dot(centre));
            (vent, point)
        }).collect();
        let reach = sources.iter().map(|(vent, p)| p.length() + sector::SUPPORT * vent.radius_m)
            .fold(0.0, f64::max) + 64.0;
        let at = |p: DVec2| surface((centre * RADIUS_M + p.x * east + p.y * north).normalize());
        let (grid, change, exposed) = sector::evolve(&sources, seed, reach, at);
        Self { centre, summit_guard: 0.0, east, north, reach, grid, change, exposed: Some(exposed) }
    }

    fn sample(&self, dir: DVec3) -> (f64, bool) {
        let offset = (dir - self.centre) * RADIUS_M;
        let distance = offset.length();
        if distance >= self.reach { return (0.0, false); }
        let (x,z) = (offset.dot(self.east), offset.dot(self.north));
        if let Some(exposed) = &self.exposed {
            let cos = dir.dot(self.centre);
            if cos <= 0.0 { return (0.0, false); }
            let (x,z) = (x / cos, z / cos);
            return (self.grid.bilinear(&self.change, x, z), self.grid.bilinear(exposed, x, z) > 0.55);
        }
        let summit = smooth(self.summit_guard, self.summit_guard + 120.0, distance);
        let edge = 1.0 - smooth(self.reach * 0.78, self.reach, distance);
        (self.grid.bilinear(&self.change, x, z) * summit * edge, false)
    }
}

pub(super) struct Field {
    flanks: Vec<Flank>,
    bins: Bins,
    grouped: bool,
}

impl Field {
    pub fn bake(
        vents: impl Iterator<Item = Vent>,
        seed: u32,
        workers: usize,
        refined: bool,
        collapsed: bool,
        surface: impl Fn(DVec3) -> (f64, f64, f64) + Sync,
    ) -> Self {
        let mut vents: Vec<_> = vents.collect();
        let flanks = if collapsed {
            let mut groups = Vec::new();
            while let Some(first) = vents.pop() {
                let mut group = vec![first];
                let mut i = 0;
                while i < group.len() {
                    let vent = group[i];
                    vents.retain(|&other| {
                        let overlap = (vent.radius_m + other.radius_m) * sector::SUPPORT;
                        if vent.dir.distance(other.dir) * RADIUS_M >= overlap { return true; }
                        group.push(other);
                        false
                    });
                    i += 1;
                }
                group.sort_by_key(|v| v.active);
                groups.push(group);
            }
            super::map_ordered(groups.len(), workers, |i| Flank::bake_group(&groups[i], seed, &surface))
        } else {
            super::map_ordered(vents.len(), workers, |i| Flank::bake(vents[i], seed, refined, &surface))
        };
        let bins = bins(flanks.iter().map(|f| (f.centre, f.reach)), CELL_M);
        Self { flanks, bins, grouped: collapsed }
    }

    pub fn sample(&self, dir: DVec3) -> (f64, bool) {
        let samples = self.bins.get(&cell(dir, CELL_M)).into_iter().flatten()
            .map(|&i| self.flanks[i].sample(dir));
        if self.grouped {
            samples.fold((0.0, false), |(height, rock), (change, exposed)| (height + change, rock || exposed))
        } else {
            samples.max_by(|a, b| a.0.abs().total_cmp(&b.0.abs())).unwrap_or((0.0, false))
        }
    }
}
