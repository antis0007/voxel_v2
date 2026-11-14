//! Representative climate stages on the same mesh as the river solve.
use super::{hydrology::Flow, tectonics::smooth, topology::Mesh};

pub(super) const PASSES_PER_EPOCH: usize = 3;
const COOLING: [f64; 6] = [8.0, 0.0, 12.0, 2.0, 6.0, 0.0];
const FLOW_WEIGHT: [f64; 3] = [0.65, 1.75, 0.90];

pub(super) struct History<'a> {
    temperature: &'a [f64],
    rain: &'a [f64],
    pub runoff: Vec<f64>,
    pub cover: Vec<f64>,
    ice: Vec<f64>,
    load: Vec<f64>,
    slide: Vec<f64>,
    quarried: f64,
    till: f64,
    river_feed: f64,
    slipped: f64,
    bank_cut: f64,
}

impl<'a> History<'a> {
    pub fn new(temperature: &'a [f64], rain: &'a [f64]) -> Self {
        let n = temperature.len();
        Self {
            temperature,
            rain,
            runoff: vec![0.0; n],
            cover: vec![0.0; n],
            ice: vec![0.0; n],
            load: vec![0.0; n],
            slide: vec![0.0; n],
            quarried: 0.0,
            till: 0.0,
            river_feed: 0.0,
            slipped: 0.0,
            bank_cut: 0.0,
        }
    }

    pub fn set_runoff(&mut self, mesh: &Mesh, height: &[f64], sea: f64, pass: usize) {
        let epoch = pass / PASSES_PER_EPOCH;
        let thaw = COOLING[epoch.saturating_sub(1)] - COOLING[epoch];
        for (i, value) in self.runoff.iter_mut().enumerate() {
            let now = self.temperature[i] - 0.0065 * (height[i] - sea).max(0.0) - COOLING[epoch];
            let liquid = 0.25 + 0.75 * smooth(-6.0, 3.0, now);
            let melt = smooth(0.0, 8.0, thaw)
                * smooth(-1.0, 3.0, now)
                * (1.0 - smooth(-1.5, 1.0, now - thaw));
            *value = mesh.area[i]
                * self.rain[i]
                * (liquid + 0.45 * melt)
                * FLOW_WEIGHT[pass % PASSES_PER_EPOCH];
        }
    }

    pub fn reshape(
        &mut self,
        mesh: &Mesh,
        height: &mut [f64],
        flow: &Flow,
        rock: &[f64],
        sea: f64,
        epoch: usize,
        river_feed: &mut [f64],
    ) -> bool {
        let cooling = COOLING[epoch];
        let thaw = COOLING[epoch.saturating_sub(1)] - cooling;
        let previous = (self.quarried, self.slipped);
        self.ice.fill(0.0);
        self.load.fill(0.0);
        // Traverse from headwaters to sea, so ice and its entrained rock
        // reach the terminus along the valleys that exist in this epoch.
        for &i in flow.order.iter().rev() {
            if height[i] <= sea {
                continue;
            }
            let r = flow.receiver[i];
            let area = mesh.area[i];
            let now = self.temperature[i] - 0.0065 * (height[i] - sea) - cooling;
            let mut flux = self.ice[i] + area * self.rain[i] * (1.0 - smooth(-5.0, 0.0, now));
            let edge = mesh.neighbours[i].iter().find(|&&(j, _)| j == r);
            let slope = edge.map_or(0.0, |&(_, d)| ((height[i] - height[r]) / d).max(0.0));
            let motion = smooth(0.008, 0.12, slope) * smooth(0.9, 5.0, flux / area);
            let cut = (13.0 * motion * rock[i].min(1.6)).min((height[i] - sea - 0.15).max(0.0));
            height[i] -= cut;
            self.cover[i] = (self.cover[i] - cut).max(0.0);
            self.quarried += cut * area;
            let load = self.load[i] + cut * area;
            let terminal = r == i || height[r] <= sea;
            let melt = if terminal {
                1.0
            } else {
                smooth(-0.5, 5.0, now)
            };
            flux *= 1.0 - melt;
            let till = (load * melt).min((495.0 - height[i]).clamp(0.0, 5.0) * area);
            height[i] += till / area;
            self.cover[i] += till / area;
            self.till += till;
            if terminal {
                river_feed[i] += load - till;
                self.river_feed += load - till;
            } else {
                self.ice[r] += flux;
                self.load[r] += load - till;
            }
        }
        if thaw > 0.0 {
            self.thaw_slopes(mesh, height, sea, cooling, thaw);
        }
        previous != (self.quarried, self.slipped)
    }

    fn thaw_slopes(&mut self, mesh: &Mesh, height: &mut [f64], sea: f64, cooling: f64, thaw: f64) {
        self.slide.fill(0.0);
        for (i, &top) in height.iter().enumerate() {
            if top <= sea + 0.15 {
                continue;
            }
            let now = self.temperature[i] - 0.0065 * (top - sea) - cooling;
            let thawed = (1.0 - smooth(-3.0, -1.5, now - thaw)) * smooth(-1.5, 2.0, now);
            let wet = smooth(0.40, 0.82, self.rain[i]);
            if thawed * wet <= 0.0 {
                continue;
            }
            let excess = |j: usize, d| top - height[j] - 0.12 * d;
            let target = mesh.neighbours[i]
                .iter()
                .copied()
                .filter(|&(j, d)| height[j] > sea && excess(j, d) > 0.0)
                .max_by(|&(a, da), &(b, db)| {
                    excess(a, da).total_cmp(&excess(b, db)).then(b.cmp(&a))
                });
            let Some((j, distance)) = target else {
                continue;
            };
            let cut = (0.13 * excess(j, distance)).min(3.5).min(top - sea - 0.15) * thawed * wet;
            let space = (495.0 - height[j] - self.slide[j].max(0.0)).max(0.0);
            let volume = (cut * mesh.area[i]).min(space * mesh.area[j]);
            self.slide[i] -= volume / mesh.area[i];
            self.slide[j] += volume / mesh.area[j];
            self.slipped += volume;
        }
        for ((top, cover), &delta) in height.iter_mut().zip(&mut self.cover).zip(&self.slide) {
            *top += delta;
            *cover = (*cover + delta).max(0.0);
        }
    }

    pub fn flood_cut(
        &mut self,
        mesh: &Mesh,
        height: &mut [f64],
        flow: &Flow,
        rock: &[f64],
        sea: f64,
        river_feed: &mut [f64],
        sediment: &mut [f64],
    ) {
        self.slide.fill(0.0);
        for (i, &r) in flow.receiver.iter().enumerate() {
            let out = flow.receiver[r];
            if i == r
                || r == out
                || height[r] <= sea
                || height[r] < flow.spill[r] - 0.5
                || flow.discharge[r] < super::SOURCE_AREA
                || flow.discharge[i] >= 0.25 * flow.discharge[r]
            {
                continue;
            }
            let Some(&(_, distance)) = mesh.neighbours[i].iter().find(|&&(j, _)| j == r) else {
                continue;
            };
            let (bank, next) = (
                mesh.directions[i] - mesh.directions[r],
                mesh.directions[out] - mesh.directions[r],
            );
            let (bank, next) = (bank.normalize_or_zero(), next.normalize_or_zero());
            let across = 1.0 - bank.dot(next).abs();
            let main = mesh.neighbours[r]
                .iter()
                .map(|&(j, _)| j)
                .filter(|&j| j != i && flow.receiver[j] == r)
                .max_by(|&a, &b| {
                    flow.discharge[a]
                        .total_cmp(&flow.discharge[b])
                        .then(b.cmp(&a))
                });
            let outside = main.map_or(0.0, |up| {
                let incoming = (mesh.directions[r] - mesh.directions[up]).normalize_or_zero();
                bank.dot(incoming - next).clamp(0.0, 1.0)
            });
            let relief = height[i] - height[r];
            let susceptible =
                smooth(0.010, 0.075, relief / distance) * (1.0 - smooth(35.0, 85.0, relief));
            let power = smooth(1.0, 6.0, flow.discharge[r] / super::SOURCE_AREA);
            let cut =
                (0.8 * rock[i] * power * susceptible * (0.25 + 0.35 * across + 0.40 * outside))
                    .min((height[i] - sea - 0.15).max(0.0));
            self.slide[i] = -cut;
            let volume = cut * mesh.area[i];
            river_feed[r] += volume;
            if let Some(load) = sediment.get_mut(r) {
                *load += volume;
            }
            self.bank_cut += volume;
        }
        for ((top, cover), &delta) in height.iter_mut().zip(&mut self.cover).zip(&self.slide) {
            *top += delta;
            *cover = (*cover + delta).max(0.0);
        }
    }

    pub fn report(&self) {
        let residual = self.quarried - self.till - self.river_feed;
        eprintln!(
            "[worldgen] climate_history epochs={} ice_cut_m3={:.3} till_m3={:.3} river_feed_m3={:.3} thaw_slide_m3={:.3} flood_bank_m3={:.3} ice_residual_m3={residual:.6}",
            COOLING.len(),
            self.quarried,
            self.till,
            self.river_feed,
            self.slipped,
            self.bank_cut
        );
    }
}
