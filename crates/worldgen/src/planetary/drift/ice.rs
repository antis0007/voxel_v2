//! Persistent cold-epoch ice, abrasion and till in the surface-history loop.
//! Reduced shallow-ice transport: thickness/slope drive motion, bed strength
//! limits abrasion, and the river graph carries the released sediment.
use super::{Mesh, Crust, history};
use crate::planetary::{SEA, hydrology::Flow, smooth};
use engine_core::cube_sphere::atlas;
use glam::DVec3;

pub(super) struct Ice {
    pub excess: Vec<f64>,
    pub target: Vec<f64>,
    pub baseline: Vec<f64>,
    pub former: Vec<f64>,
    moved: Vec<f64>,
    cut: Vec<f64>,
    load: Vec<f64>,
    carbon: Vec<f64>,
}

impl Ice {
    pub fn new(n: usize) -> Self {
        Self { excess: vec![0.0; n], target: vec![0.0; n], baseline: vec![0.0; n],
            former: vec![0.0; n], moved: vec![0.0; n], cut: vec![0.0; n],
            load: vec![0.0; n], carbon: vec![0.0; n] }
    }

    pub fn sea(&self, mesh: &Mesh, height: &[f64]) -> f64 {
        let mut volume = 0.0;
        let mut ocean = 0.0;
        for i in 0..height.len() {
            if height[i] > SEA { volume += self.excess[i] * mesh.area[i]; }
            else { ocean += mesh.area[i]; }
        }
        SEA - volume / ocean.max(mesh.area.iter().sum::<f64>() * 0.05)
    }

    pub fn at(&self, mesh: &Mesh, dir: DVec3) -> f64 { mesh.sample(&self.former, dir) }

    pub fn advance(&mut self, mesh: &Mesh, flow: &Flow, records: &mut [history::Record],
        crust: &[Crust], height: &mut [f64], soil: &mut [f64], strength: f64, dt: f64, myr: f64) -> f64
    {
        self.moved.fill(0.0); self.cut.fill(0.0); self.load.fill(0.0); self.carbon.fill(0.0);
        // A finite relaxation time retains accumulation and thaw across
        // surface steps; it does not repaint ice from the current temperature.
        let balance = -(-dt / 0.12).exp_m1();
        for i in 0..height.len() {
            self.excess[i] += (self.target[i] - self.excess[i]) * balance;
            if height[i] <= SEA { self.excess[i] = 0.0; }
        }
        for i in 0..height.len() {
            if height[i] <= SEA { continue; }
            let thickness = self.baseline[i] + self.excess[i];
            if thickness <= 0.0 { continue; }
            let extent = smooth(1.0, 12.0, thickness);
            self.former[i] = self.former[i].max(extent);
            let surface = height[i] + thickness;
            let mut total = 0.0;
            for &(j, distance) in &mesh.neighbours[i] {
                let grade = ((surface - height[j] - self.baseline[j] - self.excess[j]) / distance).max(0.0);
                total += grade.powi(3);
            }
            if total <= 1e-12 { continue; }
            // A bounded finite-volume transfer preserves water volume when
            // neighbouring spherical cells have unequal areas.
            let volume = self.excess[i] * mesh.area[i] * (total * thickness * 0.02).min(0.20);
            self.moved[i] -= volume;
            for &(j, distance) in &mesh.neighbours[i] {
                let grade = ((surface - height[j] - self.baseline[j] - self.excess[j]) / distance).max(0.0);
                self.moved[j] += volume * grade.powi(3) / total;
            }
            let r = flow.receiver[i];
            let distance = (mesh.directions[i] - mesh.directions[r]).length() * atlas::RADIUS_M;
            let grade = (height[i] - height[r]).max(0.0) / distance.max(1.0);
            let resistance = records[i].resistance(myr, crust[i].carbonate);
            let abrasion = (thickness * grade * extent * strength * 0.07 / resistance).min(0.18);
            self.cut[i] = self.cut[i].max(abrasion);
            let direction = (mesh.directions[r] - mesh.directions[i]).normalize_or_zero();
            for &(j, _) in &mesh.neighbours[i] {
                let rise = height[j] - height[i];
                if !(0.0..40.0).contains(&rise) { continue; }
                let cross = 1.0 - (mesh.directions[j] - mesh.directions[i]).normalize_or_zero().dot(direction).abs();
                self.cut[j] = self.cut[j].max(abrasion * cross * 0.55 * (1.0 - smooth(8.0, 40.0, rise)));
            }
        }
        let mut fresh = 0.0;
        for i in 0..height.len() {
            self.excess[i] = (self.excess[i] + self.moved[i] / mesh.area[i]).max(0.0);
            let cut = self.cut[i].min((height[i] - SEA - 0.25).max(0.0));
            if cut <= 0.0 { continue; }
            let (rock, carbonate) = records[i].remove(cut, crust[i].carbonate);
            fresh += rock * mesh.area[i];
            height[i] -= cut;
            self.load[i] = cut * mesh.area[i];
            self.carbon[i] = carbonate * mesh.area[i];
        }
        for &i in flow.order.iter().rev() {
            if self.load[i] <= 0.0 { continue; }
            let r = flow.receiver[i];
            let retained = if r == i || height[i] <= SEA { 1.0 }
                else { 1.0 - 0.85 * smooth(0.5, 8.0, self.baseline[i] + self.excess[i]) };
            let deposit = self.load[i] * retained;
            records[i].deposit(myr, deposit / mesh.area[i], self.carbon[i] / self.load[i].max(1e-12), false);
            height[i] += deposit / mesh.area[i];
            if r != i {
                self.load[r] += self.load[i] - deposit;
                self.carbon[r] += self.carbon[i] * (1.0 - retained);
            }
            soil[i] = records[i].mobile(myr);
        }
        fresh
    }
}

mod snapshot;
