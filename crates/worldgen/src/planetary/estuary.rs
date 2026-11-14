//! Subcell landforms driven by the solved approach, outlet and retained sediment.
use super::{SEA, tectonics::smooth};
use glam::DVec2;

pub(super) const INLET: f64 = -176.0;

pub(super) struct Estuary {
    inlet: DVec2, // lateral offset and water level
    outlet: DVec2,
    width: f64,
    side: f64,
    bar_width: f64,
    bar_rise: f64,
    branch: Option<DVec2>,
}

#[derive(Clone, Copy)]
pub(super) struct Section {
    pub bed: f64,
    pub weight: f64,
    pub water: f64,
    pub bar: f64,
    pub sediment: f64,
    pub fine: f64,
    pub inlet_tie: f64,
}

impl Estuary {
    pub fn new(
        inlet: DVec2,
        outlet: DVec2,
        width: f64,
        sediment: f64,
        drift: f64,
        branch: Option<DVec2>,
    ) -> Self {
        let length = outlet.x.max(3.4 * width).clamp(116.0, 285.0);
        let side = branch.map_or_else(
            || {
                if drift.abs() > 0.08 {
                    drift.signum()
                } else if outlet.y > 0.0 {
                    -1.0
                } else {
                    1.0
                }
            },
            |end| (end.y - outlet.y).signum(),
        );
        let bar_width = (width * 0.70 + sediment.sqrt() * 0.028).clamp(17.0, 57.0);
        Self {
            inlet,
            outlet: DVec2::new(length, outlet.y),
            width: width.clamp(10.0, 50.0),
            side,
            bar_width,
            bar_rise: (sediment / (length * bar_width) * 0.42).clamp(0.0, 6.0),
            branch,
        }
    }

    pub(super) fn channel_geometry(&self, x: f64) -> (f64, f64, f64) {
        let length = self.outlet.x;
        let approach = smooth(INLET, 0.0, x);
        let jet = smooth(-52.0, length, x);
        let bend = smooth(INLET, -30.0, x) * (1.0 - smooth(-30.0, length * 0.8, x));
        let centre = self.inlet.x * (1.0 - approach) + self.outlet.y * jet
            - self.side * self.width * 0.96 * bend;
        (centre, bend, jet)
    }

    pub fn sample(&self, p: DVec2) -> Option<Section> {
        let x = p.x;
        let length = self.outlet.x;
        if x <= INLET || x >= length + 46.0 {
            return None;
        }
        let (centre, bend, jet) = self.channel_geometry(x);
        let width = self.width * (0.96 + 0.47 * jet + 0.24 * bend);
        let cross = (p.y - centre) * self.side / width + 0.20 * bend;
        let reach = smooth(INLET, -100.0, x) * (1.0 - smooth(length * 0.74, length + 46.0, x));
        let water = SEA + (self.inlet.y - SEA).max(0.0) * (1.0 - smooth(INLET, 24.0, x));
        // Shallower inner bench, narrow cut bank and a deeper outer thalweg.
        let offset = if cross < 0.0 { -cross } else { cross / 1.18 };
        let depth = (1.6 + self.width * 0.055) * (1.0 - 0.48 * jet);
        let profile = |d: f64, depth: f64, bank_slope: f64| {
            water - depth * (1.0 - d.min(1.0).powi(2)) + bank_slope * (d - 1.0).max(0.0)
        };
        let mut bed = profile(offset, depth, if cross < 0.0 { 5.5 } else { 2.1 });
        let mut near = 1.0 - smooth(1.14, 2.48, offset);
        if let Some(end) = self.branch {
            let split = smooth(-82.0, end.x * 0.7, x);
            let branch_y = centre + (end.y - centre) * split;
            let d = (p.y - branch_y).abs() / (width * (0.43 - 0.09 * split)).max(6.0);
            let active = smooth(-88.0, -20.0, x) * (1.0 - smooth(end.x, end.x + 54.0, x));
            bed += (profile(d, depth * 0.61, 2.4) - bed).min(0.0) * active;
            near = near.max(active * (1.0 - smooth(1.0, 2.0, d)));
        }
        let bar_x = (x - length * 0.24) / (length * 0.66 + 27.0);
        let bar_y = ((p.y - centre) * self.side - width * (1.49 + 0.26 * bar_x)) / self.bar_width;
        let sediment = 1.0 - smooth(0.26, 1.04, bar_x.hypot(bar_y));
        let bar = SEA - 3.3 + self.bar_rise * smooth(0.03, 0.92, sediment);
        let weight = reach * near;
        if weight <= 0.0 && sediment <= 0.0 {
            return None;
        }
        Some(Section {
            bed,
            weight,
            water,
            bar,
            sediment,
            fine: smooth(-0.5, 0.68, bar_y) * (1.0 - smooth(0.10, 1.0, bar_x)),
            inlet_tie: 1.0 - smooth(-137.0, -83.0, x),
        })
    }

    pub fn checksum(&self, mut hash: u64) -> u64 {
        let branch = self.branch.unwrap_or_default();
        for value in [
            self.inlet.x,
            self.inlet.y,
            self.outlet.x,
            self.outlet.y,
            self.width,
            self.side,
            self.bar_width,
            self.bar_rise,
            branch.x,
            branch.y,
        ] {
            hash = (hash ^ value.to_bits()).wrapping_mul(0x100000001b3);
        }
        hash
    }
}
