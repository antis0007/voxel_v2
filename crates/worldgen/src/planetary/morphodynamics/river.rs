//! Floods advance in both horizontal directions over a finite gravel cover.
use super::{Bed, DRY, G, Water, boundary, exchange, smooth, volume};
use crate::geo::Ground;
use glam::DVec3;

const LENGTH: usize = 105;
const WIDTH: usize = 53;
const FLOODS: usize = 10;
const STEPS: usize = 420;
const RECESSION: usize = 720;
const WET: f64 = 0.18;
const ROCK_STOCK: f64 = 2.4;
const FLOOD_PEAKS: [f64; FLOODS] = [0.64, 0.82, 1.05, 0.72, 1.35, 0.85, 1.10, 0.64, 1.50, 0.93];

pub(in crate::planetary) struct Reach {
    pub origin: DVec3,
    pub downstream: DVec3,
    pub half_width: f64,
    pub length: f64,
    pub tailwater: f64,
    pub runoff: f64,
    pub supply: f64,
}

pub(in crate::planetary) struct Tile {
    pub origin: DVec3,
    downstream: DVec3,
    across: DVec3,
    spacing: f64,
    cells: Box<[[f32; 4]]>,
    rock_exposure: Option<Box<[f32]>>,
    pub bank_m3: f64,
    pub headcut_m3: f64,
    pub incision_m3: f64,
    pub sediment_error_m3: f64,
    pub water_error_m3: f64,
    pub split_sections: usize,
}

#[derive(Clone, Copy)]
pub(in crate::planetary) struct Surface {
    pub bed: f64,
    pub depth: f64,
    pub shift: f64,
    pub deposited: f64,
    pub blend: f64,
    pub rock_exposure: f64,
}

impl Tile {
    pub fn bake(
        reach: &Reach, terrain: impl Fn(DVec3) -> Ground,
        strength: Option<&impl Fn(DVec3, f64) -> f64>,
    ) -> Option<Self> {
        let &Reach { origin, downstream, half_width, length, tailwater, runoff, supply } = reach;
        let across = origin.normalize().cross(downstream).normalize();
        let spacing = length / (LENGTH - 1) as f64;
        let area = spacing * spacing;
        let mid = (WIDTH - 1) as f64 * 0.5;
        let mut water = Vec::with_capacity(LENGTH * WIDTH);
        let mut bed = Vec::with_capacity(water.capacity());
        let mut initial = Vec::with_capacity(water.capacity());
        let mut strata = strength.map(|_| Vec::with_capacity(water.capacity()));
        for y in 0..WIDTH {
            for x in 0..LENGTH {
                let p = origin + spacing * (downstream * x as f64 + across * (y as f64 - mid));
                let g = terrain(p.normalize());
                let z = f64::from(g.top_m);
                let depth = g.water_m.map_or(0.0, |h| (f64::from(h) - z).max(0.0));
                let loose = 0.45 + 3.8 * (1.0 - smooth(0.10, 0.40, f64::from(g.slope)))
                    * smooth(0.08, 0.68, f64::from(g.riparian));
                // The incoming network already occupies an incised valley.
                // Deep parts of its old bed carry a veneer, while bars and
                // the uncut banks retain thicker mobile alluvium.
                let exposure = if strength.is_some() { smooth(0.38, 1.15, depth) } else { 0.0 };
                let loose = loose * (1.0 - exposure) + 0.20 * exposure;
                // The regional section anchors the inlet and outlet. Inside
                // the tile the valley starts with mobile gravel over the old
                // deep template and only a shallow pilot channel.
                let margin = x.min(LENGTH - 1 - x).min(y).min(WIDTH - 1 - y) as f64;
                let fill = (depth - 0.42).clamp(0.0, 4.0) * smooth(0.0, 15.0, margin) * (1.0 - exposure);
                bed.push(Bed { rock: z - loose, sediment: [loose + fill, 0.0] });
                if let (Some(rock), Some(strata)) = (strength, &mut strata) {
                    let surface = z - loose;
                    strata.push((surface, [0.25, 1.05, 1.85].map(|depth| rock(p.normalize(), surface - depth))));
                }
                water.push(Water { depth: depth - fill, ..Water::default() });
                initial.push(z);
            }
        }
        // The forcing comes from the connected upstream channel, not a
        // prescribed route across the tile.
        let inlets: Vec<_> = (0..WIDTH).filter_map(|y| {
            let i = y * LENGTH;
            let w = water[i].depth.min(2.0);
            (w > WET).then_some((i, w))
        }).collect();
        let outlet_depth = (0..WIDTH).map(|y| water[(y + 1) * LENGTH - 1].depth)
            .fold(0.0, f64::max);
        if inlets.is_empty() || outlet_depth < WET {
            return None;
        }
        let inlet_sum: f64 = inlets.iter().map(|(_, w)| w).sum();
        let bankfull = (runoff * 1.3e-5).clamp(8.0, 520.0).min(half_width * 13.0);
        let supply_ratio = (0.001 + supply / (runoff * 12.0).max(1.0)).clamp(0.001, 0.024);
        let mut next = water.clone();
        let mut level = initial.clone();
        let starting_bed: Vec<_> = bed.iter().map(|b| b.height()).collect();
        let mut velocity = vec![[0.0; 2]; water.len()];
        let initial_budget = volume(&water, &bed, area);
        let mut incoming = [0.0; 2];
        let mut outgoing = [0.0; 2];
        let mut bank = 0.0;
        let mut headcut = 0.0;
        let mut incision = 0.0;
        for step in 0..FLOODS * STEPS + RECESSION {
            let phase = (step % STEPS) as f64 / STEPS as f64;
            let peak = if strength.is_some() {
                FLOOD_PEAKS.get(step / STEPS).copied().unwrap_or(0.0)
            } else { 1.0 };
            let flood = if step < FLOODS * STEPS {
                smooth(0.02, 0.25, phase) * (1.0 - smooth(0.59, 0.88, phase)) * peak
            } else { 0.0 };
            let discharge = 0.12 + 0.88 * flood;
            let q = bankfull * discharge;
            // Manning scaling for the downstream cross-section. A fixed
            // bankfull tailwater drowns gravel bars after every flood.
            let stage = tailwater - outlet_depth * (1.0 - discharge.powf(0.6)) + flood * 0.25;
            let mut speed: f64 = 1.0;
            for (i, w) in water.iter().enumerate() {
                level[i] = bed[i].height();
                velocity[i] = w.velocity();
                speed = speed.max((G * w.depth).sqrt() + velocity[i][0].abs() + velocity[i][1].abs());
            }
            let dt = 0.32 * spacing / speed;
            let ratio = dt / spacing;
            next.copy_from_slice(&water);
            for y in 0..WIDTH {
                for x in 0..LENGTH {
                    let i = y * LENGTH + x;
                    if x + 1 < LENGTH {
                        exchange(i, i + 1, 0, ratio, &water, &level, &velocity, &mut next);
                    } else {
                        boundary(0, 1.0, ratio, stage, water[i], level[i],
                            velocity[i], &mut next[i], &mut outgoing, area);
                    }
                    if y + 1 < WIDTH {
                        exchange(i, i + LENGTH, 1, ratio, &water, &level, &velocity, &mut next);
                    } else {
                        next[i].momentum[1] -= ratio * 0.5 * G * water[i].depth.powi(2);
                    }
                    if y == 0 {
                        next[i].momentum[1] += ratio * 0.5 * G * water[i].depth.powi(2);
                    }
                    if x == 0 {
                        next[i].momentum[0] += ratio * 0.5 * G * water[i].depth.powi(2);
                    }
                }
            }
            let feed = q * dt;
            // Bedload arriving from upstream can peak after local runoff.
            // On the falling limb the flow has less capacity to move it.
            let transport_pulse = if step < FLOODS * STEPS {
                smooth(0.13, 0.41, phase) * (1.0 - smooth(0.76, 0.99, phase))
            } else { 0.0 };
            let grains = (bankfull * dt * supply_ratio * (0.03 + 1.30 * transport_pulse * peak)).min(feed * 0.045);
            incoming[0] += feed;
            incoming[1] += grains;
            for &(i, w) in &inlets {
                let fraction = w / (inlet_sum * area);
                next[i].depth += feed * fraction;
                next[i].momentum[0] += feed * fraction * 1.1;
                next[i].suspended[0] += grains * fraction;
            }
            for (w, b) in next.iter_mut().zip(&mut bed) {
                w.depth = w.depth.max(0.0);
                let u = w.velocity();
                let speed2 = u[0] * u[0] + u[1] * u[1];
                let drag = 1.0 + dt * 0.043 * speed2.sqrt() / w.depth.max(0.12);
                w.momentum = if w.depth < DRY { [0.0; 2] } else { w.momentum.map(|v| v / drag) };
                let capacity = w.depth * (speed2 - 0.38).max(0.0).min(7.0) * 0.023;
                // Deposition and entrainment exchange the same gravel between
                // the bed and moving water. The bedrock floor is fixed.
                let exchange = ((capacity - w.suspended[0]) * (dt * 0.55).min(0.65))
                    .clamp(-w.suspended[0].max(0.0), b.sediment[0].max(0.0));
                b.sediment[0] -= exchange;
                w.suspended[0] += exchange;
            }
            if let Some(strata) = &strata {
                for ((w, b), &(surface, resistance)) in next.iter_mut().zip(&mut bed).zip(strata) {
                    if w.depth < 0.30 || b.sediment[0] > 0.85 { continue; }
                    let [u, v] = w.velocity();
                    let speed2 = u * u + v * v;
                    let cut = (surface - b.rock).clamp(0.0, ROCK_STOCK);
                    let threshold = resistance[((cut / 0.8) as usize).min(2)];
                    let capacity = w.depth * (speed2 - 0.38).clamp(0.0, 7.0) * 0.023;
                    let carried = w.suspended[0].max(0.0);
                    let tools = smooth(0.001, 0.025, carried);
                    let bare = 1.0 - smooth(0.10, 0.85, b.sediment[0]);
                    // A sediment-starved flow has few tools. Excess gravel
                    // settles and covers the bed. Broken rock joins the load.
                    let wear = (0.028 * dt * (speed2 - threshold).clamp(0.0, 4.0) * tools * bare)
                        .min((capacity - carried).max(0.0)).min(ROCK_STOCK - cut);
                    b.rock -= wear;
                    w.suspended[0] += wear;
                    incision += wear * area;
                }
            }
            // Scour at the toe destabilizes the bank above it. Fallen gravel
            // joins the wet cell and is carried by the next hydraulic step.
            for y in 1..WIDTH - 1 {
                for x in 1..LENGTH - 1 {
                    let i = y * LENGTH + x;
                    if next[i].depth < 0.10 { continue; }
                    let u = next[i].velocity();
                    let stress = (u[0] * u[0] + u[1] * u[1] - 0.55).clamp(0.0, 4.0);
                    for (j, normal_speed) in [
                        (i - 1, -u[0]), (i + 1, u[0]),
                        (i - LENGTH, -u[1]), (i + LENGTH, u[1]),
                    ] {
                        // Where shallow floodwater returns to a lower bed,
                        // scour the upstream lip. As the lip retreats, the
                        // shortcut can intercept more of the next flood.
                        let drop = level[i] - level[j];
                        if next[i].depth < 1.4 && next[j].depth > 0.12 && normal_speed > 0.3 && drop > 0.24 {
                            let loose = (0.025 * dt * (normal_speed - 0.3) * (drop - 0.24).min(2.5))
                                .min(bed[i].sediment[0].max(0.0));
                            bed[i].sediment[0] -= loose;
                            next[j].suspended[0] += loose;
                            headcut += loose * area;
                        }
                        let excess = (level[j] - level[i] - spacing * 0.18).clamp(0.0, 3.0);
                        if next[i].depth < 0.32 || excess == 0.0 || next[j].depth > next[i].depth * 0.65 { continue; }
                        let fallen = (0.024 * dt * stress * excess).min(bed[j].sediment[0].max(0.0));
                        bed[j].sediment[0] -= fallen;
                        next[i].suspended[0] += fallen;
                        bank += fallen * area;
                    }
                }
            }
            std::mem::swap(&mut water, &mut next);
        }
        let end = volume(&water, &bed, area);
        incoming[1] += incision;
        let errors = std::array::from_fn::<_, 2, _>(|i| initial_budget[i] + incoming[i] - outgoing[i] - end[i]);
        if !errors.iter().all(|v| v.is_finite() && v.abs() < 0.05) || bank < area {
            return None;
        }
        let split_sections = (LENGTH / 8..LENGTH * 7 / 8).filter(|&x| {
            let mut runs = 0;
            let mut last = false;
            for y in 1..WIDTH - 1 {
                let wet = water[y * LENGTH + x].depth > WET && water[y * LENGTH + x].velocity()[0] > 0.12;
                runs += usize::from(wet && !last);
                last = wet;
            }
            runs >= 2
        }).count();
        let cells = bed.iter().zip(&water).zip(initial.into_iter().zip(starting_bed)).map(|((b, w), (z, start))| {
            [b.height() as f32, w.depth as f32, (b.height() - z) as f32, (b.height() - start) as f32]
        }).collect();
        let rock_exposure = strata.as_ref().map(|_| bed.iter().map(|b| {
            (1.0 - smooth(0.10, 0.65, b.sediment[0])) as f32
        }).collect());
        Some(Self { origin, downstream, across, spacing, cells, rock_exposure, bank_m3: bank, headcut_m3: headcut,
            incision_m3: incision,
            sediment_error_m3: errors[1], water_error_m3: errors[0], split_sections })
    }

    pub fn radius(&self) -> f64 { self.spacing * (LENGTH as f64 + WIDTH as f64 * 0.5) }

    pub fn checksum(&self, mut hash: u64) -> u64 {
        for cell in &self.cells {
            for &value in cell {
                hash = (hash ^ u64::from(value.to_bits())).wrapping_mul(0x100000001b3);
            }
        }
        if let Some(exposure) = &self.rock_exposure {
            for &value in exposure {
                hash = (hash ^ u64::from(value.to_bits())).wrapping_mul(0x100000001b3);
            }
        }
        hash
    }

    pub fn sample(&self, p: DVec3, footprint: f32) -> Option<Surface> {
        let relative = p - self.origin;
        let x = relative.dot(self.downstream) / self.spacing;
        let y = relative.dot(self.across) / self.spacing + (WIDTH - 1) as f64 * 0.5;
        let margin = x.min((LENGTH - 1) as f64 - x).min(y).min((WIDTH - 1) as f64 - y);
        if margin <= 0.0 { return None; }
        let blend = smooth(1.0, 12.0, margin) * (1.0 - smooth(9.0, 48.0, f64::from(footprint)));
        if blend == 0.0 { return None; }
        let i = y as usize * LENGTH + x as usize;
        let u = x.fract();
        let v = y.fract();
        let [bed, depth, shift, deposited] = std::array::from_fn(|k| {
            let a = f64::from(self.cells[i][k]) * (1.0 - u) + f64::from(self.cells[i + 1][k]) * u;
            let b = f64::from(self.cells[i + LENGTH][k]) * (1.0 - u) + f64::from(self.cells[i + LENGTH + 1][k]) * u;
            a * (1.0 - v) + b * v
        });
        let rock_exposure = self.rock_exposure.as_ref().map_or(0.0, |a| {
            let north = f64::from(a[i]) * (1.0 - u) + f64::from(a[i + 1]) * u;
            let south = f64::from(a[i + LENGTH]) * (1.0 - u) + f64::from(a[i + LENGTH + 1]) * u;
            north * (1.0 - v) + south * v
        });
        Some(Surface { bed, depth, shift, deposited, blend, rock_exposure })
    }
}
