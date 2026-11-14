//! Bounded coastal evolution on a tangent grid. The result is baked once per
//! world recipe. Voxel sampling only interpolates its bed, water and sediment.
pub(in crate::planetary) mod river;
pub(in crate::planetary) mod meander;
mod feedback;
mod bedload;
mod branch_audit;

use super::{SEA, delta_islands, estuary, geo::Ground, outlook, tectonics::smooth};
use engine_core::cube_sphere::atlas::RADIUS_M;
use glam::DVec3;

const NX: usize = 56;
const NY: usize = 40;
const DX: f64 = 16.0;
const INLAND: f64 = 208.0;
const STEPS: usize = 640;
const G: f64 = 9.81;
const DRY: f64 = 0.001;
const FINE_SETTLING: f64 = 0.035;
// Transport stores geomorphic-equivalent sediment volume. Both the incoming
// load and carrying capacity use this scale, while water runs at its CFL step.
const MOBILE_MORPH_FACTOR: f64 = 24.0;
pub(super) const RADIUS: f64 = 780.0;

#[derive(Clone, Copy)]
pub(super) struct Grid {
    nx: usize,
    ny: usize,
    dx: f64,
    inland: f64,
    steps: usize,
    mobile: bool,
    tidal: bool,
    evolving: bool,
    feedback: bool,
    distributary: bool,
    shore_history: Option<f64>,
}

impl Grid {
    pub const LEGACY: Self = Self {
        nx: NX, ny: NY, dx: DX, inland: INLAND, steps: STEPS, mobile: false, tidal: false, evolving: false, feedback: false, distributary: false, shore_history: None,
    };
    pub const MOBILE: Self = Self {
        nx: 145, ny: 111, dx: 4.0, inland: 220.0, steps: 1664, mobile: true, tidal: false, evolving: false, feedback: false, distributary: false, shore_history: None,
    };
    pub const MOBILE_COARSE: Self = Self {
        nx: 73, ny: 56, dx: 8.0, inland: 220.0, steps: 832, mobile: true, tidal: false, evolving: false, feedback: false, distributary: false, shore_history: None,
    };
    pub const TIDAL: Self = Self { tidal: true, ..Self::MOBILE };
    pub const TIDAL_COARSE: Self = Self { tidal: true, ..Self::MOBILE_COARSE };
    pub const EVOLVING: Self = Self { evolving: true, ..Self::TIDAL };
    pub const EVOLVING_COARSE: Self = Self { evolving: true, ..Self::TIDAL_COARSE };
    pub const FEEDBACK: Self = Self { feedback: true, ..Self::EVOLVING };
    pub const FEEDBACK_COARSE: Self = Self { feedback: true, ..Self::EVOLVING_COARSE };
    pub const DISTRIBUTARY: Self = Self { distributary: true, ..Self::FEEDBACK };
    pub const DISTRIBUTARY_COARSE: Self = Self { distributary: true, ..Self::FEEDBACK_COARSE };
    pub fn shore_history(fine: bool, exposure: f64) -> Self {
        let grid = if fine { Self::EVOLVING } else { Self::EVOLVING_COARSE };
        Self { shore_history: Some(exposure), ..grid }
    }
}

/// Relative Manning conveyance for a wide section at a common roughness and slope.
pub(super) fn conveyance(area: f64, width: f64) -> f64 {
    area * (area / width).powf(2.0 / 3.0)
}

#[derive(Clone, Copy, Default)]
struct Water {
    depth: f64,
    momentum: [f64; 2],
    suspended: [f64; 2],
    // Tagged subset of the fine grain. It carries no additional mass.
    river_fines: f64,
}

impl Water {
    fn velocity(self) -> [f64; 2] {
        if self.depth <= DRY {
            [0.0; 2]
        } else {
            self.momentum.map(|q| q / self.depth)
        }
    }
}

#[derive(Clone, Copy)]
struct Bed {
    rock: f64,
    sediment: [f64; 2],
}

impl Bed {
    fn height(self) -> f64 {
        self.rock + self.sediment[0] + self.sediment[1]
    }
}

#[derive(Clone, Copy, Default)]
pub(super) struct Budget {
    pub bed_change: f64,
    pub water_error: f64,
    pub sediment_error: f64,
    pub new_land_m2: f64,
}

pub(super) struct Tile {
    grid: Grid,
    pub outlet_connected: bool,
    origin: DVec3,
    forward: DVec3,
    across: DVec3,
    // Bed, depth, fine fraction and net deposition. Discard the working state.
    cells: Box<[[f32; 4]]>,
    // Suspended river fines, wet fraction and newly retained fine sediment.
    coastal_water: Option<Box<[[f32; 3]]>>,
    outlet: Option<(glam::DVec2, f64)>,
    estuary: Option<estuary::Estuary>,
    islands: Option<delta_islands::Islands>,
    outlook: Option<outlook::Outlook>,
    pub budget: Budget,
}

#[derive(Clone, Copy)]
pub(super) struct Surface {
    pub bed: f64,
    pub depth: f64,
    pub mud: f64,
    pub deposited: f64,
    pub blend: f64,
    pub outlet_bed: Option<f64>,
    pub estuary: Option<estuary::Section>,
    pub island: Option<delta_islands::Surface>,
    pub island_share: f64,
    pub outlook: Option<outlook::Grade>,
    pub mobile: bool,
    pub suspended: f64,
    pub inundation: f64,
    pub fine_accretion: f64,
}

fn volume(water: &[Water], bed: &[Bed], area: f64) -> [f64; 2] {
    let mut result = [0.0; 2];
    for (w, b) in water.iter().zip(bed) {
        result[0] += w.depth * area;
        result[1] += (w.suspended[0] + w.suspended[1] + b.sediment[0] + b.sediment[1]) * area;
    }
    result
}

impl Tile {
    pub fn bake(
        origin: DVec3,
        forward: DVec3,
        across: DVec3,
        runoff: f64,
        load: f64,
        width: f64,
        wave_drift: f64,
        grid: Grid,
        terrain: impl Fn(DVec3) -> (f64, f64),
    ) -> Self {
        let Grid { nx, ny, dx, inland, steps, mobile, tidal, evolving, feedback, distributary, shore_history } = grid;
        let area = dx * dx;
        let mut initial = Vec::with_capacity(nx * ny);
        let mut water = Vec::with_capacity(nx * ny);
        for y in 0..ny {
            for x in 0..nx {
                let a = x as f64 * dx - inland;
                let b = (y as f64 - (ny - 1) as f64 * 0.5) * dx;
                let point = (origin + forward * a + across * b).normalize() * RADIUS_M;
                let (h, head) = terrain(point);
                initial.push(h);
                water.push(Water { depth: (head - h).max(0.0), ..Water::default() });
            }
        }
        let mut bed: Vec<_> = initial
            .iter()
            .map(|&h| {
                // Shelf sediment can be remobilised. The underlying rock remains fixed.
                let loose = 0.35 + 0.75 * (1.0 - smooth(SEA - 8.0, SEA + 6.0, h));
                let loose = if mobile { loose * 2.4 } else { loose };
                Bed {
                    rock: h - loose,
                    sediment: [loose * 0.75, loose * 0.25],
                }
            })
            .collect();
        let mut next = water.clone();
        let mut levels = initial.clone();
        let mut velocity = vec![[0.0; 2]; water.len()];
        let start = volume(&water, &bed, area);
        let mut imported = [0.0; 2];
        let mut exported = [0.0; 2];

        // Transfer the upstream reach's discharge into the existing lower
        // valley. The short event needs to deliver its river water and sand
        // to the actual mouth before it recedes.
        let inlet_x = if distributary {
            ((inland - 68.0) / dx) as usize
        } else if mobile {
            (16.0 / dx) as usize
        } else {
            4
        };
        let span = if mobile { (80.0 / dx) as usize } else { 5 };
        let mid = (ny / 2 - span..=ny / 2 + span)
            .min_by(|&a, &b| initial[a * nx + inlet_x].total_cmp(&initial[b * nx + inlet_x]))
            .unwrap();
        let mut inlets: Vec<_> = (0..ny)
            .filter_map(|y| {
                let lateral = y.abs_diff(mid) as f64 * dx;
                let weight = 1.0 - smooth(0.0, width.max(dx), lateral);
                (weight > 0.0).then_some((y * nx + inlet_x, weight))
            })
            .collect();
        let inlet_sum: f64 = inlets.iter().map(|i| i.1).sum();
        let mut wet_inlets = vec![0.0; if evolving { inlets.len() } else { 0 }];
        // The short hydraulic run represents an event sequence. Its sediment
        // supply is a bounded fraction of the catchment's eroded volume.
        let sediment_supply = (load * 0.10).min(620_000.0);
        let sand_reserve = if distributary { sediment_supply * 0.02 } else { 0.0 };
        // The tidal bake compresses a flood into a short run. Scale the
        // runoff-derived discharge so the pulse can traverse the lower reach
        // and spill onto the shelf before the next tide reverses it.
        let bankfull = if tidal {
            (runoff * 20e-9 * 380.0).clamp(60.0, 1150.0)
        } else {
            (runoff * 20e-9 * 38.0).clamp(12.0, 1150.0)
        };
        let pulse = |step: usize| {
            let phase = step as f64 / steps as f64;
            if shore_history.is_some() {
                let freshet = smooth(0.0, 0.14, phase) * (1.0 - smooth(0.19, 0.50, phase));
                let storm = smooth(0.56, 0.62, phase) * (1.0 - smooth(0.68, 0.95, phase));
                0.62 * freshet + storm
            } else if tidal {
                // The second flood finds the bed reshaped by the first.
                let first = smooth(0.0, 0.09, phase) * (1.0 - smooth(0.32, 0.48, phase));
                let second = smooth(0.53, 0.63, phase) * (1.0 - smooth(0.88, 1.0, phase));
                first + (if distributary { 0.85 } else { 0.74 }) * second
            } else {
                smooth(0.0, 0.15, phase) * (1.0 - smooth(0.59, 0.84, phase))
            }
        };
        let pulse_sum: f64 = (0..steps).map(pulse).sum();
        let mut inundation = tidal.then(|| vec![0u16; water.len()]);
        let mut crest = feedback.then(|| vec![f32::NEG_INFINITY; water.len()]);
        let mut jet = distributary.then(|| bedload::Flood::new(water.len()));
        let mut audit = (tidal && std::env::var_os("VOXEL_DELTA_AUDIT").is_some())
            .then(|| branch_audit::Branches::new(water.len()));
        let audit_steps = [66, 76, 86].map(|phase| steps * phase / 100);
        let handoff = steps * 52 / 100;

        for step in 0..steps {
            if let Some(audit) = &mut audit && audit_steps.contains(&step) {
                audit.observe(grid, &bed, &water, &inlets);
            }
            // The first pulse has receded and the second has not started.
            if step == handoff && let (Some(crest), Some(wet_steps)) = (crest.take(), inundation.as_deref()) {
                if let Some(jet) = jet.take() {
                    // Advance the bed over additional floods using a small
                    // portion of the sediment available from the catchment.
                    let sand = sand_reserve.min(imported[1] * 12.0);
                    imported[1] += jet.advance(grid, &mut bed, &mut water, &crest, &inlets, sand);
                }
                feedback::handoff(grid, &mut bed, &mut water, &crest, (wet_steps, step.div_ceil(8)), &mut inlets);
            }
            // Two compressed tidal cycles, ending at the world sea datum.
            let sea_level = if tidal {
                let phase = step as f64 / (steps - 1) as f64;
                if let Some(exposure) = shore_history {
                    let angle = phase * std::f64::consts::TAU;
                    SEA + (2.0 * angle).sin() * (0.18 + 0.18 * exposure)
                        + 0.12 * angle.sin()
                } else {
                    SEA + (phase * 2.0 * std::f64::consts::TAU).sin() * (0.30 + 0.35 * wave_drift.abs())
                }
            } else { SEA };
            let mut speed: f64 = 1.0;
            for (i, (w, b)) in water.iter().zip(&bed).enumerate() {
                levels[i] = b.height();
                velocity[i] = w.velocity();
                speed =
                    speed.max((G * w.depth).sqrt() + velocity[i][0].abs() + velocity[i][1].abs());
            }
            let dt = 0.36 * dx / speed;
            let ratio = dt / dx;
            next.copy_from_slice(&water);
            for y in 0..ny {
                for x in 0..nx {
                    let i = y * nx + x;
                    if x + 1 < nx {
                        exchange(i, i + 1, 0, ratio, &water, &levels, &velocity, &mut next);
                    } else {
                        boundary(
                            0,
                            1.0,
                            ratio,
                            sea_level,
                            water[i],
                            levels[i],
                            velocity[i],
                            &mut next[i],
                            &mut exported,
                            area,
                        );
                    }
                    if y + 1 < ny {
                        exchange(i, i + nx, 1, ratio, &water, &levels, &velocity, &mut next);
                    } else {
                        boundary(
                            1,
                            1.0,
                            ratio,
                            sea_level,
                            water[i],
                            levels[i],
                            velocity[i],
                            &mut next[i],
                            &mut exported,
                            area,
                        );
                    }
                    if y == 0 {
                        boundary(
                            1,
                            -1.0,
                            ratio,
                            sea_level,
                            water[i],
                            levels[i],
                            velocity[i],
                            &mut next[i],
                            &mut exported,
                            area,
                        );
                    }
                    if x == 0 {
                        // Upstream wall. The river enters through the source strip.
                        next[i].momentum[0] += ratio * 0.5 * G * water[i].depth.powi(2);
                    }
                }
            }
            let flood = pulse(step);
            let supplied_water = bankfull * (0.10 + 0.90 * flood) * dt;
            let requested_sediment = (sediment_supply - sand_reserve) * flood / pulse_sum;
            let supplied_sediment = if tidal {
                // Accelerating bed evolution must not multiply the inlet's
                // suspended sediment concentration by the same factor.
                requested_sediment.min(supplied_water * 0.045)
            } else if mobile {
                // The inlet cannot carry an entire geomorphic episode in
                // one short flood. Preserve a dilute bed-load feed.
                requested_sediment.min(supplied_water * 0.045 * MOBILE_MORPH_FACTOR)
            } else {
                requested_sediment
            };
            imported[0] += supplied_water;
            imported[1] += supplied_sediment;
            let mut wet_sum = 0.0;
            for (value, &(i, taper)) in wet_inlets.iter_mut().zip(&inlets) {
                let depth = water[i].depth;
                *value = if depth > DRY { taper * conveyance(depth, 1.0) } else { 0.0 };
                wet_sum += *value;
            }
            for (slot, &(i, weight)) in inlets.iter().enumerate() {
                let share = if evolving {
                    // Let the inherited wetted cross-section set the inflow profile.
                    // A dry inlet first receives water in its lowest cell.
                    if wet_sum > 0.0 { wet_inlets[slot] / (wet_sum * area) }
                    else if i == mid * nx + inlet_x { 1.0 / area }
                    else { 0.0 }
                } else {
                    weight / (inlet_sum * area)
                };
                let depth = supplied_water * share;
                next[i].depth += depth;
                next[i].momentum[0] += depth * 1.6;
                next[i].suspended[0] += supplied_sediment * share * 0.64;
                let fines = supplied_sediment * share * 0.36;
                next[i].suspended[1] += fines;
                if tidal { next[i].river_fines += fines; }
            }
            for (w, b) in next.iter_mut().zip(&mut bed) {
                // Semi-implicit drag and grain-dependent exchange. Every bed
                // increment is removed from suspension in the same cell.
                w.depth = w.depth.max(0.0);
                let [u, v] = w.velocity();
                let speed = u.hypot(v);
                let drag = 1.0 + dt * 0.035 * speed / w.depth.max(0.08);
                w.momentum = if w.depth <= DRY {
                    [0.0; 2]
                } else {
                    w.momentum.map(|q| q / drag)
                };
                let cohesion = if tidal {
                    let fine = b.sediment[1] / (b.sediment[0] + b.sediment[1]).max(1e-9);
                    1.0 + 0.85 * smooth(0.2, 0.7, fine)
                } else { 1.0 };
                for (grain, (settling, critical)) in
                    [(0.16, 0.45), (FINE_SETTLING, 0.26)].into_iter().enumerate()
                {
                    let critical = critical * cohesion;
                    let available = w.suspended[grain].max(0.0);
                    let exchange = if w.depth <= DRY {
                        available
                    } else if mobile {
                        let power = (speed * speed - critical * critical).max(0.0);
                        let capacity = w.depth * (0.0008 + 0.016 * power).min(0.14)
                            * MOBILE_MORPH_FACTOR;
                        let excess = (available - capacity).max(0.0);
                        // Aggregated fines can settle out in sheltered overbank water
                        // even below the channel's bulk transport capacity.
                        let depositable = if evolving && grain == 1 {
                            let slack = 1.0 - smooth(0.10, critical, speed);
                            let shallows = 1.0 - smooth(0.30, 1.50, w.depth);
                            excess + (available - excess) * slack * shallows
                        } else {
                            excess
                        };
                        let fallout = depositable * (dt * settling * 0.65 / w.depth.max(0.12)).min(1.0);
                        let undersaturation = (capacity - available).max(0.0);
                        let erosion = (undersaturation * (0.80 * dt).min(1.0))
                            .min(dt * 0.016 * power * MOBILE_MORPH_FACTOR)
                            .min(0.016).min(b.sediment[grain]);
                        fallout.min(dt * 0.072 * MOBILE_MORPH_FACTOR).min(0.022) - erosion
                    } else {
                        let fallout = available * (dt * settling / w.depth.max(0.12)).min(1.0);
                        let erosion = (dt * 0.003 * (speed * speed - critical * critical).max(0.0))
                            .min(b.sediment[grain]);
                        fallout.min(dt * 0.055) - erosion
                    };
                    if grain == 1 && w.river_fines > 0.0 {
                        let retained = 1.0 - exchange.max(0.0) / available.max(1e-12);
                        w.river_fines = w.river_fines.min(available) * retained;
                    }
                    b.sediment[grain] += exchange;
                    w.suspended[grain] = available - exchange;
                }
            }
            if step % 8 == 0 {
                if let Some(wet_steps) = &mut inundation {
                    for (count, cell) in wet_steps.iter_mut().zip(&next) {
                        *count += u16::from(cell.depth > 0.10);
                    }
                }
                if let (Some(crest), Some(jet)) = (&mut crest, &mut jet) {
                    jet.observe(crest, &water, &levels);
                } else if let Some(crest) = &mut crest {
                    for ((high, cell), height) in crest.iter_mut().zip(&water).zip(&levels) {
                        if cell.depth > 0.12 { *high = high.max((height + cell.depth) as f32); }
                    }
                }
                if mobile {
                    relax_wet_banks(&mut bed, &next, &mut levels, grid, 8.0 * dt);
                }
                if wave_drift != 0.0 {
                    let waterline = if shore_history.is_some() { sea_level } else { SEA };
                    littoral_transport(&mut bed, &mut levels, wave_drift, grid, waterline);
                }
                if let Some(exposure) = shore_history.filter(|&value| value > 0.0) {
                    swash_transport(&mut bed, &mut levels, grid, sea_level, exposure);
                }
                if evolving {
                    transport_bed_sand(&mut bed, &next, &mut levels, grid, 8.0 * dt);
                }
            }
            std::mem::swap(&mut water, &mut next);
        }
        if let Some(audit) = audit {
            audit.report(grid, &bed, &water, &initial, |x, y| {
                let a = x as f64 * dx - inland;
                let b = (y as f64 - (ny - 1) as f64 * 0.5) * dx;
                engine_core::cube_sphere::atlas::from_body((origin + forward * a + across * b).normalize() * RADIUS_M)
            });
        }
        let end = volume(&water, &bed, area);
        let outlet_connected = !mobile || wet_route_reaches_sea(grid, &water, &bed, &inlets);
        if mobile {
            eprintln!("[worldgen] mobile_delta cell_m={dx:.0} connected={outlet_connected} water_in_m3={:.1} sediment_in_m3={:.1}",
                imported[0], imported[1]);
        }
        let mut budget = Budget {
            water_error: start[0] + imported[0] - exported[0] - end[0],
            sediment_error: start[1] + imported[1] - exported[1] - end[1],
            ..Budget::default()
        };
        let coastal_water = inundation.map(|wet_steps| {
            let samples = steps.div_ceil(8) as f32;
            water.iter().zip(wet_steps).zip(bed.iter().zip(&initial)).map(|((w, count), (b, before))| {
                let optical = (1.0 - (-18.0 * w.river_fines.max(0.0)).exp()) * smooth(0.04, 0.40, w.depth);
                let original_fines = (before - b.rock) * 0.25;
                let accretion = (b.sediment[1] - original_fines).max(0.0);
                [optical as f32, f32::from(count) / samples, accretion as f32]
            }).collect()
        });
        let cells = bed
            .iter()
            .zip(water)
            .zip(initial)
            .map(|((b, w), before)| {
                let h = b.height();
                budget.bed_change += (h - before).abs() * area;
                if before < SEA && h > SEA + 0.15 {
                    budget.new_land_m2 += area;
                }
                let mud = b.sediment[1] / (b.sediment[0] + b.sediment[1]).max(1e-9);
                [h as f32, w.depth as f32, mud as f32, (h - before) as f32]
            })
            .collect();
        Self {
            grid,
            outlet_connected,
            origin,
            forward,
            across,
            cells,
            coastal_water,
            outlet: None,
            estuary: None,
            islands: None,
            outlook: None,
            budget,
        }
    }

    /// Choose a low-flow exit across the final sediment surface. A route
    /// must reach deep shelf water and prefers the lowest cut through bars.
    pub fn connect_outlet(
        &mut self,
        half_width: f64,
        estuary_inlet: Option<glam::DVec2>,
        drift: f64,
        islands: bool,
    ) {
        let mut best: Option<(f64, glam::DVec2)> = None;
        let mut candidates = Vec::with_capacity(9);
        for angle in [-0.68f64, -0.44, -0.24, -0.11, 0.0, 0.11, 0.24, 0.44, 0.68] {
            let (s, c) = angle.sin_cos();
            let direction = glam::DVec2::new(c, s);
            let mut cut = 0.0;
            let mut deep = 0;
            for step in 1..=24 {
                let end = direction * (step as f64 * DX);
                let point = self.origin + self.forward * end.x + self.across * end.y;
                let Some(sample) = self.sample(point) else {
                    break;
                };
                cut += (sample.bed - (SEA - 1.0)).max(0.0).powi(2);
                deep = if sample.bed < SEA - 2.5 { deep + 1 } else { 0 };
                if deep >= 3 {
                    let score = cut + end.length() * 0.08 + angle.abs() * 6.0;
                    if estuary_inlet.is_some() {
                        candidates.push((score, end));
                    }
                    if best.is_none_or(|(low, _)| score < low) {
                        best = Some((score, end));
                    }
                    break;
                }
            }
        }
        self.outlet = best.map(|(_, end)| (end, (half_width * 0.65).clamp(5.0, 24.0)));
        if let (Some(inlet), Some((score, end))) = (estuary_inlet, best) {
            let retained = self
                .cells
                .iter()
                .map(|c| f64::from(c[3]).max(0.0))
                .sum::<f64>()
                * DX
                * DX;
            // A second channel needs its own shelf route and enough sediment
            // between the two routes to keep the bar above their beds.
            let branch = if retained >= 50_000.0 && drift.abs() < 0.72 {
                candidates
                    .into_iter()
                    .filter(|&(cost, p)| {
                        (p.y - end.y).abs() > (half_width * 1.55).max(31.0)
                            && cost < score * 2.6 + 48.0
                    })
                    .min_by(|a, b| a.0.total_cmp(&b.0))
                    .map(|(_, p)| p)
            } else {
                None
            };
            let estuary = estuary::Estuary::new(
                inlet, end, half_width, retained, drift, branch,
            );
            if islands {
                let salt = self.origin.x.to_bits().rotate_left(11)
                    ^ self.origin.y.to_bits().rotate_left(29) ^ self.origin.z.to_bits();
                self.islands = delta_islands::Islands::build(
                    end, estuary.channel_geometry(-45.0).0, half_width, retained, drift,
                    (salt ^ (salt >> 32)) as u32,
                    |point| self.sample(self.body_point(point)).map(|s| (s.bed, s.mud)),
                );
            }
            self.estuary = Some(estuary);
        }
    }

    fn body_point(&self, p: glam::DVec2) -> DVec3 {
        (self.origin + self.forward * p.x + self.across * p.y).normalize() * RADIUS_M
    }

    pub fn survey_outlook(&self, terrain: impl Fn(DVec3) -> Ground) -> Option<outlook::Outlook> {
        let (outlet, _) = self.outlet?;
        outlook::Outlook::survey(outlet, |p| terrain(self.body_point(p).normalize()))
    }

    pub fn set_outlook(&mut self, outlook: Option<outlook::Outlook>, hash: u64) -> u64 {
        self.outlook = outlook;
        self.outlook.as_ref().map_or(hash, |outlook| outlook.checksum(hash))
    }

    pub fn outlook_points(&self) -> Option<[DVec3; 3]> {
        Some(self.outlook.as_ref()?.points().map(|p| self.body_point(p)))
    }

    pub fn sample(&self, p: DVec3) -> Option<Surface> {
        let d = p - self.origin;
        let local = glam::DVec2::new(d.dot(self.forward), d.dot(self.across));
        let Grid { nx, ny, dx, inland, mobile, .. } = self.grid;
        let x = (local.x + inland) / dx;
        let y = local.y / dx + (ny - 1) as f64 * 0.5;
        let edge = x.min(y).min((nx - 1) as f64 - x).min((ny - 1) as f64 - y);
        if edge <= 0.0 {
            return None;
        }
        let (ix, iy) = (x.floor() as usize, y.floor() as usize);
        let (u, v) = (x - ix as f64, y - iy as f64);
        let i = iy * nx + ix;
        let corners = [i, i + 1, i + nx, i + nx + 1];
        let weights = [(1.0 - u) * (1.0 - v), u * (1.0 - v), (1.0 - u) * v, u * v];
        let read = |field: usize| {
            corners
                .into_iter()
                .zip(weights)
                .map(|(j, w)| w * f64::from(self.cells[j][field]))
                .sum()
        };
        let outlet_bed = self.outlet.and_then(|(end, width)| {
            let t = local.dot(end) / end.length_squared();
            if !(0.0..=1.0).contains(&t) {
                return None;
            }
            let section = (local - end * t).length() / width;
            (section < 1.0).then(|| SEA - 1.65 * (1.0 - section * section))
        });
        let coastal = |field: usize| self.coastal_water.as_ref().map_or(0.0, |cells| {
            corners.into_iter().zip(weights).map(|(j, w)| w * f64::from(cells[j][field])).sum()
        });
        Some(Surface {
            bed: read(0),
            depth: read(1),
            mud: read(2),
            deposited: read(3),
            blend: if mobile { smooth(0.0, 43.0, edge * dx) } else { smooth(0.0, 6.0, edge) },
            outlet_bed,
            estuary: self.estuary.as_ref().and_then(|form| form.sample(local)),
            island: self.islands.as_ref().and_then(|islands| islands.sample(local)),
            island_share: self.islands.as_ref().map_or(0.0, |islands| islands.share),
            outlook: self.outlook.as_ref().and_then(|outlook| outlook.sample(local)),
            mobile,
            suspended: coastal(0),
            inundation: coastal(1),
            fine_accretion: coastal(2),
        })
    }

    pub fn checksum(&self, mut hash: u64) -> u64 {
        for value in [self.origin, self.forward, self.across]
            .into_iter()
            .flat_map(|v| v.to_array())
        {
            hash = (hash ^ value.to_bits()).wrapping_mul(0x100000001b3);
        }
        for v in self.cells.iter().flatten() {
            hash = (hash ^ u64::from(v.to_bits())).wrapping_mul(0x100000001b3);
        }
        if let Some(field) = &self.coastal_water {
            for v in field.iter().flatten() { hash = (hash ^ u64::from(v.to_bits())).wrapping_mul(0x100000001b3); }
        }
        if let Some((end, width)) = self.outlet {
            for value in [end.x, end.y, width] {
                hash = (hash ^ value.to_bits()).wrapping_mul(0x100000001b3);
            }
        }
        let hash = self.estuary
            .as_ref()
            .map_or(hash, |form| form.checksum(hash));
        self.islands.as_ref().map_or(hash, |islands| islands.checksum(hash))
    }

    pub fn origin(&self) -> DVec3 {
        self.origin
    }
}

/// Move the mobile sand layer along the current. A cell loses only its own
/// available sand, and flux that slows down in the next cell builds the bed.
fn transport_bed_sand(bed: &mut [Bed], water: &[Water], change: &mut [f64], grid: Grid, dt: f64) {
    let Grid { nx, ny, dx, .. } = grid;
    change.fill(0.0);
    for (i, w) in water.iter().enumerate() {
        if w.depth < 0.08 || bed[i].sediment[0] <= 0.0 {
            continue;
        }
        let [u, v] = w.velocity();
        let speed_sq = u * u + v * v;
        let excess = (speed_sq - 0.45f64.powi(2)).max(0.0);
        if excess == 0.0 {
            continue;
        }
        // A bounded bedload closure proportional to excess stress to the 3/2 power.
        let moved = (0.003 * excess * excess.sqrt() * dt * MOBILE_MORPH_FACTOR / dx)
            .min(0.08)
            .min(bed[i].sediment[0]);
        let (x, y) = (i % nx, i / nx);
        let magnitude = u.abs() + v.abs();
        let east = if u >= 0.0 { (x + 1 < nx).then(|| i + 1) } else { (x > 0).then(|| i - 1) };
        let north = if v >= 0.0 { (y + 1 < ny).then(|| i + nx) } else { (y > 0).then(|| i - nx) };
        for (target, weight) in [(east, u.abs()), (north, v.abs())] {
            if let Some(j) = target {
                let head = w.depth + bed[i].height();
                let accessible = 1.0 - smooth(head - 0.05, head, bed[j].height());
                let transfer = moved * (weight / magnitude) * accessible;
                change[i] -= transfer;
                change[j] += transfer;
            }
        }
    }
    for (cell, &sand) in bed.iter_mut().zip(change.iter()) {
        cell.sediment[0] += sand;
    }
}

/// The bake may move a channel, but a puddle behind an inland sill is not
/// a valid river outlet. Check the wet cells all the way to the open sea.
fn wet_route_reaches_sea(grid: Grid, water: &[Water], bed: &[Bed], inlets: &[(usize, f64)]) -> bool {
    let mut seen = vec![false; water.len()];
    let mut queue = std::collections::VecDeque::new();
    let mut visit = |i: usize, queue: &mut std::collections::VecDeque<usize>| {
        if !seen[i] && water[i].depth >= 0.28 {
            seen[i] = true;
            queue.push_back(i);
        }
    };
    for &(i, _) in inlets {
        visit(i, &mut queue);
    }
    while let Some(i) = queue.pop_front() {
        let x = i % grid.nx;
        let y = i / grid.nx;
        let open_edge = x + 1 == grid.nx || ((y == 0 || y + 1 == grid.ny)
            && x as f64 * grid.dx > grid.inland + 24.0);
        if open_edge && bed[i].height() < SEA - 0.75 {
            return true;
        }
        if x > 0 { visit(i - 1, &mut queue); }
        if x + 1 < grid.nx { visit(i + 1, &mut queue); }
        if y > 0 { visit(i - grid.nx, &mut queue); }
        if y + 1 < grid.ny { visit(i + grid.nx, &mut queue); }
    }
    false
}

/// Move loose sand off a failing bank into adjacent flowing water.
/// Every transfer changes the two beds by the same volume.
fn relax_wet_banks(bed: &mut [Bed], water: &[Water], moved: &mut [f64], grid: Grid, dt: f64) {
    moved.fill(0.0);
    let mut exchange = |i: usize, j: usize| {
        let (a, b) = if bed[i].height() > bed[j].height() { (i, j) } else { (j, i) };
        if water[b].depth < 0.16 || water[a].depth > water[b].depth * 0.65 {
            return;
        }
        let [u, v] = water[b].velocity();
        let shear = smooth(0.18, 0.85, u * u + v * v);
        let fall = bed[a].height() - bed[b].height() - 0.24 * grid.dx;
        let available = bed[a].sediment[0] + moved[a].min(0.0);
        let sand = (0.13 * dt * fall.max(0.0) / grid.dx * shear)
            .min(available.max(0.0) * 0.40);
        moved[a] -= sand;
        moved[b] += sand;
    };
    for y in 0..grid.ny {
        for x in 0..grid.nx {
            let i = y * grid.nx + x;
            if x + 1 < grid.nx { exchange(i, i + 1); }
            if y + 1 < grid.ny { exchange(i, i + grid.nx); }
        }
    }
    for (b, &sand) in bed.iter_mut().zip(moved.iter()) {
        b.sediment[0] += sand;
    }
}

/// A bounded alongshore sand transport approximation in the breaking-wave
/// zone. The two cells share the same exchange. Mud stays in the fluvial solve.
fn littoral_transport(bed: &mut [Bed], moved: &mut [f64], drift: f64, grid: Grid, waterline: f64) {
    moved.fill(0.0);
    let breaker =
        |h: f64| smooth(waterline - 8.0, waterline - 1.2, h) * (1.0 - smooth(waterline - 0.1, waterline + 1.0, h));
    for i in 0..grid.nx * (grid.ny - 1) {
        let j = i + grid.nx;
        let (a, b) = if drift > 0.0 { (i, j) } else { (j, i) };
        let wave = breaker(bed[a].height()).min(breaker(bed[b].height()));
        let sand = (0.028 * drift.abs() * wave).min(bed[a].sediment[0]);
        moved[a] -= sand;
        moved[b] += sand;
    }
    for (b, &sand) in bed.iter_mut().zip(moved.iter()) {
        b.sediment[0] += sand;
    }
}

/// Small landward transfers on gently sloping shorefaces. The active band
/// follows the tide, so it cannot cut a fixed shelf at the mean sea datum.
fn swash_transport(bed: &mut [Bed], moved: &mut [f64], grid: Grid, waterline: f64, waves: f64) {
    moved.fill(0.0);
    let Grid { nx, ny, dx, .. } = grid;
    let runup = 0.18 + 0.20 * waves;
    for i in 0..bed.len() {
        let top = bed[i].height();
        let wet = smooth(waterline - 0.75, waterline - 0.15, top)
            * (1.0 - smooth(waterline - 0.08, waterline, top));
        if wet <= 0.0 { continue; }
        let (x, y) = (i % nx, i / nx);
        let neighbours = [(x > 0).then(|| i - 1), (x + 1 < nx).then(|| i + 1),
            (y > 0).then(|| i - nx), (y + 1 < ny).then(|| i + nx)];
        let mut target = None;
        let mut best = 0.0;
        for j in neighbours.into_iter().flatten() {
            let rise = bed[j].height() - top;
            let score = smooth(0.01, 0.08, rise)
                * (1.0 - smooth(dx * 0.08, dx * 0.18, rise))
                * (1.0 - smooth(waterline + 0.05, waterline + runup, bed[j].height()));
            if score > best { best = score; target = Some(j); }
        }
        if let Some(j) = target {
            let sand = (0.003 * waves * wet * best).min(bed[i].sediment[0]);
            moved[i] -= sand;
            moved[j] += sand;
        }
    }
    for (cell, &sand) in bed.iter_mut().zip(moved.iter()) { cell.sediment[0] += sand; }
}

// Hydrostatic reconstruction keeps a still lake at rest over varying bed
// elevations. The Rusanov flux uses one interface value for both neighbours.
fn flux(
    a: Water,
    b: Water,
    za: f64,
    zb: f64,
    va: [f64; 2],
    vb: [f64; 2],
    axis: usize,
) -> ([f64; 6], [f64; 2]) {
    let sill = za.max(zb);
    let ha = (za + a.depth - sill).max(0.0);
    let hb = (zb + b.depth - sill).max(0.0);
    let pressure = |h: f64| 0.5 * G * h * h;
    let qa = ha * va[axis];
    let qb = hb * vb[axis];
    let wave = (va[axis].abs() + (G * ha).sqrt()).max(vb[axis].abs() + (G * hb).sqrt());
    let q = 0.5 * (qa + qb - wave * (hb - ha));
    let mut flux = [q, 0.0, 0.0, 0.0, 0.0, 0.0];
    for j in 0..2 {
        flux[j + 1] = 0.5 * (qa * va[j] + qb * vb[j] - wave * (hb * vb[j] - ha * va[j]));
    }
    flux[axis + 1] += 0.5 * (pressure(ha) + pressure(hb));
    let donor = if q >= 0.0 { a } else { b };
    if donor.depth > DRY {
        flux[3] = q * donor.suspended[0] / donor.depth;
        flux[4] = q * donor.suspended[1] / donor.depth;
        flux[5] = q * donor.river_fines / donor.depth;
    }
    (
        flux,
        [
            pressure(a.depth) - pressure(ha),
            pressure(b.depth) - pressure(hb),
        ],
    )
}

fn apply(w: &mut Water, flux: [f64; 6], correction: f64, axis: usize, ratio: f64) {
    w.depth += ratio * flux[0];
    w.momentum[0] += ratio * flux[1];
    w.momentum[1] += ratio * flux[2];
    w.momentum[axis] += ratio * correction;
    w.suspended[0] += ratio * flux[3];
    w.suspended[1] += ratio * flux[4];
    w.river_fines += ratio * flux[5];
}

fn exchange(
    i: usize,
    j: usize,
    axis: usize,
    ratio: f64,
    water: &[Water],
    z: &[f64],
    v: &[[f64; 2]],
    next: &mut [Water],
) {
    if water[i].depth == 0.0 && water[j].depth == 0.0 {
        return;
    }
    let (f, [a, b]) = flux(water[i], water[j], z[i], z[j], v[i], v[j], axis);
    apply(&mut next[i], f, a, axis, -ratio);
    apply(&mut next[j], f, b, axis, ratio);
}

fn boundary(
    axis: usize,
    side: f64,
    ratio: f64,
    sea_level: f64,
    water: Water,
    z: f64,
    mut v: [f64; 2],
    next: &mut Water,
    exported: &mut [f64; 2],
    area: f64,
) {
    let sea = Water {
        depth: (sea_level - z).max(0.0),
        ..Water::default()
    };
    // Compute in the outward normal and convert the normal momentum back.
    v[axis] *= side;
    let (mut f, [correction, _]) = flux(water, sea, z, z, v, [0.0; 2], axis);
    f[axis + 1] *= side;
    apply(next, f, correction * side, axis, -ratio);
    exported[0] += ratio * area * f[0];
    exported[1] += ratio * area * (f[3] + f[4]);
}
