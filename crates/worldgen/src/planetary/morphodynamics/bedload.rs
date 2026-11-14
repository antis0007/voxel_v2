//! Event-scale bedload redistribution between the two hydraulic floods.
use super::{Bed, Grid, SEA, Water};

pub(super) struct Flood {
    transport: Vec<[f32; 3]>,
}

impl Flood {
    pub(super) fn new(cells: usize) -> Self {
        Self {
            transport: vec![[0.0; 3]; cells],
        }
    }

    pub(super) fn observe(&mut self, crest: &mut [f32], water: &[Water], levels: &[f64]) {
        for (((peak, high), cell), &height) in
            self.transport.iter_mut().zip(crest).zip(water).zip(levels)
        {
            if cell.depth > 0.12 {
                *high = high.max((height + cell.depth) as f32);
                // Bedload is dominated by the peak of the sediment-fed flood.
                // Use the tracer to exclude currents belonging only to the tide.
                let [qx, qy] = cell.momentum;
                if qx > 0.0 && cell.river_fines > 1e-7 {
                    let power = qx * (qx * qx + qy * qy) / (cell.depth * cell.depth);
                    if power > f64::from(peak[2]) {
                        *peak = [qx as f32, qy as f32, power as f32];
                    }
                }
            }
        }
    }

    pub(super) fn advance(
        self,
        grid: Grid,
        bed: &mut [Bed],
        water: &mut [Water],
        crest: &[f32],
        inlets: &[(usize, f64)],
        sand_m3: f64,
    ) -> f64 {
        let started = std::time::Instant::now();
        let Grid { nx, ny, dx, .. } = grid;
        let mut travelling = vec![0.0_f64; bed.len()];
        let mut moved = 0.0;
        let sum: f64 = inlets
            .iter()
            .map(|&(i, taper)| taper * f64::from(self.transport[i][0]))
            .sum();
        let imported = if sum > 0.0 { sand_m3 } else { 0.0 };
        let bar_sand = self.settle_bars(grid, bed, crest, imported * 0.55);
        let inlet_x = inlets[0].0 % nx;
        // Extend the observed event over a few sediment-only passes. The
        // updated bed affects each pass, including which sills accept sand.
        const SWEEPS: usize = 6;
        for _ in 0..SWEEPS {
            if imported > 0.0 {
                let share = (imported - bar_sand) / (SWEEPS as f64 * dx * dx * sum);
                for &(i, taper) in inlets {
                    travelling[i] = share * taper * f64::from(self.transport[i][0]);
                }
            }
            for x in inlet_x..nx {
                for y in 0..ny {
                    let i = y * nx + x;
                    let incoming = std::mem::take(&mut travelling[i]);
                    let h = bed[i].height();
                    let head = f64::from(crest[i]);
                    let [seaward, across, power] = self.transport[i].map(f64::from);
                    let capacity = 2.1 * (power - 0.002).clamp(0.0, 2.0);
                    let mut exits = [None; 3];
                    let mut total = 0.0;
                    if x + 1 < nx && (incoming > 0.0 || capacity > 0.0) {
                        let drift = (across / seaward.max(0.06)).clamp(-0.85, 0.85);
                        for (slot, dy) in [-1, 0, 1].into_iter().enumerate() {
                            let Some(row) = y.checked_add_signed(dy).filter(|&j| j < ny) else {
                                continue;
                            };
                            let j = row * nx + x + 1;
                            let floor = bed[j].height();
                            let clearance = (head.max(SEA) - floor).clamp(0.0, 2.5);
                            let alignment = 1.15 - (dy as f64 - drift).abs().min(1.1);
                            let weight = alignment.powi(2) * clearance;
                            exits[slot] = Some((j, weight));
                            total += weight;
                        }
                    }
                    if total == 0.0 {
                        // A blocked cell cannot forward sand through dry
                        // ground. Keep the rest mobile for the next flood.
                        let settled = incoming.min((SEA + 0.85 - h).clamp(0.0, 0.60));
                        bed[i].sediment[0] += settled;
                        water[i].suspended[0] += incoming - settled;
                        continue;
                    }
                    // Flow scours the mobile skin where its carrying capacity
                    // exceeds supply. On expansion the surplus builds a bar.
                    let exchange = if capacity > incoming
                        && (SEA - 3.5..=SEA + 2.5).contains(&h)
                        && head > h + 0.20
                    {
                        -(capacity - incoming).min(0.45).min(bed[i].sediment[0])
                    } else {
                        (incoming - capacity)
                            .max(0.0)
                            .min((SEA + 0.85 - h).clamp(0.0, 0.60))
                    };
                    bed[i].sediment[0] += exchange;
                    moved += (-exchange).max(0.0);
                    let outgoing = (incoming - exchange) / total;
                    for (j, weight) in exits.into_iter().flatten() {
                        travelling[j] += outgoing * weight;
                    }
                }
            }
        }
        eprintln!(
            "[worldgen] delta_bedload imported_m3={imported:.2} bar_sand_m3={bar_sand:.2} remobilized_m3={:.2} pass_ms={:.3}",
            moved * dx * dx,
            started.elapsed().as_secs_f64() * 1000.0
        );
        imported
    }

    fn settle_bars(&self, grid: Grid, bed: &mut [Bed], crest: &[f32], available: f64) -> f64 {
        if available <= 0.0 {
            return 0.0;
        }
        let Grid {
            nx, ny, dx, inland, ..
        } = grid;
        let flank = (12.0 / dx).ceil() as usize;
        let mut shoals = Vec::new();
        let mut total = 0.0;
        for y in flank..ny - flank {
            for x in ((inland - 45.0) / dx) as usize..((inland + 82.0) / dx) as usize {
                let i = y * nx + x;
                let h = bed[i].height();
                let limit = (SEA + 0.78).min(f64::from(crest[i]) - 0.04);
                if h < SEA - 5.0 || h > SEA - 0.18 || limit <= SEA + 0.2 {
                    continue;
                }
                let left = i - flank * nx;
                let right = i + flank * nx;
                let relief = h - bed[left].height().max(bed[right].height());
                let bypass = f64::from(self.transport[left][0].min(self.transport[right][0]));
                let shear = f64::from(self.transport[i][2]);
                if relief < 0.16 || bypass < 0.045 || shear <= 0.0 {
                    continue;
                }
                // A submerged rise sheltered between two faster lanes
                // captures bedload as flow is deflected into those lanes.
                let flank_shear = f64::from(self.transport[left][2].min(self.transport[right][2]));
                let shelter = (flank_shear / (shear + flank_shear + 0.001)).clamp(0.0, 1.0);
                let weight = relief.min(2.0).powi(2) * bypass.min(1.0) * shelter;
                total += weight;
                shoals.push((i, weight, limit - h));
            }
        }
        if total <= 0.0 {
            return 0.0;
        }
        let unit = available / (total * dx * dx);
        let mut placed = 0.0;
        for (i, weight, clearance) in shoals {
            let settled = (unit * weight).min(clearance);
            bed[i].sediment[0] += settled;
            placed += settled;
        }
        placed * dx * dx
    }
}
