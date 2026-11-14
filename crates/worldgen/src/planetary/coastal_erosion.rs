//! Wave abrasion and shelf deposition on the shared spherical terrain.
use super::{SEA, smooth, topology::Mesh};
use engine_core::cube_sphere::atlas::RADIUS_M;
use glam::DVec3;

#[derive(Default)]
pub(super) struct Budget {
    pub eroded_m3: f64,
    pub alongshore_m3: f64,
}

fn open_water(mesh: &Mesh, height: &[f64], origin: DVec3, offshore: DVec3) -> f64 {
    let mut fetch = 0.0;
    for reach in [300.0, 650.0, 1_500.0, 3_000.0, 6_000.0] {
        let ray = (origin + offshore * (reach / RADIUS_M)).normalize();
        if mesh.sample(height, ray) >= SEA { break; }
        fetch += 0.2;
    }
    fetch
}

/// Soil is stored as thickness and transfers use the actual spherical cell
/// area. Bedrock abrasion enters the same mobile sediment account.
pub(super) fn transport(mesh: &Mesh, height: &mut [f64], soil: &mut [f64],
    erodibility: &[f64], strength: f64, passes: usize) -> Budget
{
    transport_recorded(mesh, height, soil, erodibility, strength, passes, |_, _, _| {})
}

pub(super) fn transport_recorded(mesh: &Mesh, height: &mut [f64], soil: &mut [f64],
    erodibility: &[f64], strength: f64, passes: usize, mut record: impl FnMut(usize, f64, f64)) -> Budget
{
    let mut budget = Budget::default();
    let mut change = vec![0.0; height.len()];
    let mut rock_cut = vec![0.0; height.len()];
    let mut exported = vec![0.0; height.len()];
    for _ in 0..passes {
        change.fill(0.0);
        rock_cut.fill(0.0);
        exported.fill(0.0);
        for (i, &top) in height.iter().enumerate() {
            if !(SEA - 24.0..SEA + 26.0).contains(&top) { continue; }
            let origin = mesh.directions[i];
            let east = DVec3::Y.cross(origin).normalize_or_zero();
            let wind = if origin.y.abs() > 0.42 && origin.y.abs() < 0.82 { east } else { -east };
            let mut gradient = DVec3::ZERO;
            for &(j, distance) in &mesh.neighbours[i] {
                let edge = mesh.directions[j] - origin;
                let tangent = (edge - origin * edge.dot(origin)).normalize_or_zero();
                gradient += tangent * ((height[j] - top) / distance);
                if top <= SEA || !(SEA - 32.0..SEA).contains(&height[j]) { continue; }
                let fetch = open_water(mesh, height, mesh.directions[j], tangent);
                let wave = (0.25 + 0.75 * (-tangent).dot(wind).max(0.0)) * fetch;
                let cut = (2.5 * strength * erodibility[i] * wave
                    * smooth(0.008, 0.08, (top - height[j]) / distance)
                    * (1.0 - smooth(8.0, 26.0, top - SEA))).min(2.5)
                    / mesh.neighbours[i].len() as f64;
                rock_cut[i] += cut;
                change[j] += cut * mesh.area[i];
            }
            let inland = gradient.normalize_or_zero();
            let shore = origin.cross(inland).normalize_or_zero();
            let fetch = open_water(mesh, height, origin, -inland);
            // The signed oblique-wave term drives sediment along the shore.
            // A smaller counter-swell keeps sheltered coasts less active.
            let flux = 2.0 * shore.dot(wind)
                * (wind.dot(inland).max(0.0) - 0.25 * (-wind).dot(inland).max(0.0));
            let along = shore * flux.signum();
            let recipient_weight = |j: usize| {
                if !(SEA - 24.0..SEA + 2.0).contains(&height[j]) { return 0.0; }
                let edge = (mesh.directions[j] - origin).normalize_or_zero();
                edge.dot(along).max(0.0).powi(2)
                    * (1.0 - smooth(8.0, 24.0, (height[j] - SEA).abs()))
            };
            let total: f64 = mesh.neighbours[i].iter().map(|&(j, _)| recipient_weight(j)).sum();
            if total > 0.0 {
                let available = (soil[i] - rock_cut[i]).max(0.0) * mesh.area[i];
                let volume = available * (0.30 * strength * flux.abs() * fetch).min(0.45)
                    * (1.0 - smooth(3.0, 24.0, (top - SEA).abs()));
                change[i] -= volume;
                exported[i] += volume;
                for &(j, _) in &mesh.neighbours[i] {
                    change[j] += volume * recipient_weight(j) / total;
                }
                budget.alongshore_m3 += volume;
            }
        }
        for i in 0..height.len() {
            record(i, rock_cut[i] + exported[i] / mesh.area[i],
                ((change[i] + exported[i]) / mesh.area[i]).max(0.0));
            let scour = rock_cut[i].min(soil[i]);
            budget.eroded_m3 += (rock_cut[i] - scour) * mesh.area[i];
            soil[i] += change[i] / mesh.area[i] - scour;
            height[i] += change[i] / mesh.area[i] - rock_cut[i];
        }
    }
    budget
}

pub(super) fn evolve(mesh: &Mesh, height: &mut [f64], erodibility: &[f64], strength: f64) {
    let mut change = vec![0.0; height.len()];
    let mut moved = 0.0;
    for _ in 0..6 {
        change.fill(0.0);
        for (i, &top) in height.iter().enumerate() {
            let rise = top - SEA;
            if !(0.0..26.0).contains(&rise) { continue; }
            let origin = mesh.directions[i];
            for &(j, distance) in &mesh.neighbours[i] {
                let depth = SEA - height[j];
                if !(0.0..32.0).contains(&depth) { continue; }
                let offshore = mesh.directions[j];
                let tangent = (offshore - origin * offshore.dot(origin)).normalize();
                // A headland sees more open water than a neighbouring cove.
                // Stop at the first land obstruction along each offshore ray.
                let mut fetch = 0.0;
                for reach in [650.0, 1_500.0, 3_000.0, 6_000.0] {
                    let ray = (offshore + tangent * (reach / RADIUS_M)).normalize();
                    if mesh.sample(height, ray) >= SEA { break; }
                    fetch += 0.25;
                }
                let grade = (top - height[j]) / distance;
                let attack = (0.08 + 0.92 * fetch) * smooth(0.008, 0.08, grade)
                    * (1.0 - smooth(8.0, 26.0, rise)) * (1.0 - smooth(8.0, 32.0, depth));
                let cut = (3.0 * strength * erodibility[i] * attack).min(3.0)
                    / mesh.neighbours[i].len() as f64;
                let volume = cut * mesh.area[i];
                change[i] -= volume;
                change[j] += volume;
                moved += volume;
            }
        }
        for (i, h) in height.iter_mut().enumerate() {
            *h += change[i] / mesh.area[i];
        }
    }
    eprintln!("[worldgen] wave_abraded_and_redeposited_m3={moved:.1}");
}
