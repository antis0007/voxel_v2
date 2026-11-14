//! Bounded mantle provinces and a weathering/outgassing carbon reservoir.
//! Rates are scaled for the compact world; this is not an Earth reconstruction.
use super::{Drift, Plates, HistoryBudget, climate_cycle::Climate};
use crate::planetary::{smooth, tectonics::Edge};
use glam::DVec3;

struct Province {
    centre: DVec3,
    vents: [DVec3; 7],
    peak: f64,
    lifetime: f64,
}

pub(super) struct Evolution {
    provinces: Vec<Province>,
    carbon: f64,
    pub climate: Option<Climate>,
    pub melt: Vec<f64>,
}

impl Evolution {
    pub fn new(seed: u32) -> Self {
        let unit = |key: u32| f64::from(crate::hash_u32(seed ^ key.wrapping_mul(0x9e3779b9))) / f64::from(u32::MAX);
        let provinces = (0..12).map(|id| {
            let y = unit(id * 31 + 1) * 2.0 - 1.0;
            let angle = unit(id * 31 + 2) * std::f64::consts::TAU;
            let r = (1.0 - y * y).sqrt();
            let centre = DVec3::new(r * angle.cos(), y, r * angle.sin());
            let x = centre.any_orthonormal_vector();
            let z = centre.cross(x);
            let vents = std::array::from_fn(|v| {
                let key = id * 31 + v as u32 * 3 + 3;
                let radius = 0.035 + unit(key) * 0.11;
                let a = unit(key + 1) * std::f64::consts::TAU;
                (centre + radius * (x * a.cos() + z * a.sin())).normalize()
            });
            Province { centre, vents, peak: 45.0 + unit(id * 31 + 26) * 640.0,
                lifetime: 35.0 + unit(id * 31 + 27) * 45.0 }
        }).collect();
        Self { provinces, carbon: 1.0, climate: None, melt: Vec::new() }
    }

    pub fn forcing(&self) -> f64 { 5.35 * self.carbon.ln() }
}

pub(super) fn advance(field: &mut Drift, plates: &Plates, edges: &[Edge], myr: u16, dt: f64,
    budget: &mut HistoryBudget) {
    let Some(state) = &mut field.deep else { return; };
    let inherited = field.crust.clone();
    let records = &mut field.history.as_mut().expect("deep history").records;
    state.melt.resize(field.crust.len(), 0.0);
    let mut volcanic = 0.0;
    let mut weathering = 0.0;
    let mut area = 0.0;
    for (i, crust) in field.crust.iter_mut().enumerate() {
        let dir = field.mesh.directions[i];
        let a = field.mesh.area[i];
        // Juvenile arc crust is an explicit mantle source. Its addition must
        // appear in the continental budget rather than masquerade as conserved
        // transport. Mature cratons receive very little further differentiation.
        let edge = &edges[i];
        let compression = edge.stresses()[0].min(1.0);
        let across = (dir + edge.normal * (edge.boundary + 0.065)).normalize();
        let (ids, weights) = field.mesh.stencil(across);
        let other_age: f64 = ids.iter().zip(weights).map(|(&j, w)| inherited[j].ocean_age_myr * w).sum();
        let other_continent: f64 = ids.iter().zip(weights).map(|(&j, w)| inherited[j].continental * w).sum();
        let upper = crust.continental > other_continent
            || ((crust.continental - other_continent).abs() < 0.05 && crust.ocean_age_myr < other_age);
        let juvenile = if upper { 0.045 * dt * compression * (1.0 - crust.continental)
            * smooth(15.0, 100.0, other_age) } else { 0.0 };
        let old_thickness = crust.continental_km;
        crust.continental_km += juvenile;
        crust.continental = (crust.continental_km / 28.0).clamp(0.0, 1.0);
        if juvenile > 0.0 {
            crust.carbonate *= old_thickness / crust.continental_km;
            if crust.lineage == 0 { crust.lineage = plates.source_lineage(edge.owner); }
            budget.continental_created_km3 += juvenile * a / 1e6;
        }
        let mut supply = 0.0;
        for province in &state.provinces {
            let t = (f64::from(myr) - province.peak) / province.lifetime;
            if t.abs() >= 1.0 { continue; }
            let distance = dir.distance_squared(province.centre);
            if distance > 0.065 { continue; }
            let pulse = (1.0 - t * t).powi(2);
            // A shared basaltic platform plus several separate centres makes
            // two-dimensional island groups, not one curve of hotspot cones.
            let platform = 0.65 * (-distance / 0.018).exp();
            let vents: f64 = province.vents.iter().map(|v| {
                5.5 * (-dir.distance_squared(*v) / 0.00065).exp()
            }).sum();
            supply += pulse * (platform + vents);
        }
        // Extension lets melt reach the surface through thinner lithosphere.
        let permeability = 0.25 + 0.75 * (1.0 - crust.continental)
            + 0.25 * smooth(0.3, 2.0, crust.strain);
        let lava = supply.min(8.0) * permeability * dt;
        // The existing reservoir/eruption solver consumes this supply, stamps
        // actual threshold-crossing dates, and adds the resulting basalt beds.
        state.melt[i] = lava / (3.0 * dt);
        volcanic += lava / dt * a;
        let record = &records[i];
        let tropical_reaction = ((record.temperature - 15.0) * 0.045).clamp(-2.0, 2.0).exp();
        let fresh_rock = 0.5 + 0.5 * (-record.weathered_myr / 60.0).exp();
        weathering += crust.continental * record.wet * tropical_reaction * fresh_rock * a;
        area += a;
    }
    // Analytic relaxation avoids negative carbon or timestep-sized jumps.
    // Fresh wet rock consumes CO2; basalt production and background ridges add it.
    let source = 0.06 + 50.0 * volcanic / area.max(1.0);
    let sink = 0.04 + weathering / area.max(1.0);
    let target = (source / sink).clamp(0.20, 14.0);
    state.carbon += (target - state.carbon) * -(-dt / 22.0).exp_m1();
    if myr % 30 == 0 {
        eprintln!("[deep-time] myr={myr} plates={} co2_ratio={:.3} forcing_wm2={:.3} basalt_supply_m3={:.3}",
            plates.count(), state.carbon, state.forcing(), volcanic * dt);
    }
}

mod snapshot;
