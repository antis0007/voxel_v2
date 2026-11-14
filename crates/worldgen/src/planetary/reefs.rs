//! Finite-volume aging of the plate-carried hotspot platforms. Coral follows
//! light, connected shallow substrate and advected resources; no ring mask.
//! This bounded surrogate uses at most 256 growth steps and four resource
//! exchanges per step, not a fluid solver or a calibrated coral ecosystem.
use super::{SEA, hotspots::Tracks, smooth, topology::Mesh};
use crate::BlockId;
use glam::DVec3;
use std::sync::Arc;

const MAX_STEPS: usize = 256;
const MAX_STEP_MYR: f64 = 0.25;

pub(super) struct Field {
    mesh: Arc<Mesh>,
    top: Vec<f64>,
    reef: Vec<f64>,
    sand: Vec<f64>,
}

impl Field {
    pub(super) fn bake(mesh: Arc<Mesh>, height: &mut [f64], rain: &[f64], temperature: &[f64], tracks: &Tracks, fine: bool) -> Self {
        let n = height.len();
        let history: Vec<_> = mesh.directions.iter().map(|&dir| tracks.island_history(dir)).collect();
        let oldest = history.iter().map(|&(_, age)| age).fold(0.0, f64::max);
        let steps = ((oldest / MAX_STEP_MYR).ceil() as usize).clamp(1, MAX_STEPS);
        let mut active: Vec<_> = (0..n).filter(|&i| history[i].0 > 0.1 && history[i].1 > 0.1).collect();
        let mut used = vec![false; n];
        for &i in &active { used[i] = true; }
        // Two connected halo rings receive offshore volcanic sand. They
        // have no coral source unless the same shallow substrate permits it.
        for _ in 0..2 {
            let end = active.len();
            for k in 0..end {
                for &(j, _) in &mesh.neighbours[active[k]] {
                    if !used[j] { used[j] = true; active.push(j); }
                }
            }
        }
        let mut reef = vec![0.0; n];
        let mut sand = vec![0.0_f64; n];
        let mut living = vec![0.0_f64; n];
        let mut next_living = living.clone();
        let mut resource = vec![1.0_f64; n];
        let mut next_resource = resource.clone();
        let mut moved = vec![0.0; n];
        let mut runoff = vec![0.0; n];
        let mut receiver = vec![usize::MAX; n];
        let mut order = active.clone();
        let mut currents = vec![DVec3::ZERO; n];
        let mut age = vec![0.0; n];
        for &i in &active {
            height[i] += history[i].0;
            let dir = mesh.directions[i];
            let east = DVec3::Y.cross(dir).normalize_or_zero();
            currents[i] = if dir.y.abs() < 0.5 { -east } else { east };
            // Halo deposits age with their source island rather than an
            // unrelated global clock, keeping transport finite at its edge.
            let neighbour_age = mesh.neighbours[i].iter().map(|&(j, _)| history[j].1).fold(0.0, f64::max);
            age[i] = if fine { super::island_clock::elapsed(history[i].0, history[i].1, neighbour_age) }
                else { history[i].1.max(neighbour_age) };
        }
        for step in 0..steps {
            for &i in &active {
                let elapsed = history[i].1;
                let early = (1.0 + elapsed * step as f64 / steps as f64).sqrt();
                let late = (1.0 + elapsed * (step + 1) as f64 / steps as f64).sqrt();
                // Fast initial thermal subsidence slows continuously. The
                // telescoping increments still land on the mapped substrate.
                let fraction = (late - early) / ((1.0 + elapsed).sqrt() - 1.0).max(1e-12);
                height[i] -= history[i].0 * fraction;
            }
            // Upwind advection with lateral exchange; emergent land blocks
            // exchange and occupied reef consumes the arriving resources.
            for _ in 0..4 {
                for &i in &active {
                    if height[i] >= SEA { next_resource[i] = 0.0; continue; }
                    let mut supply = 0.0;
                    let mut weights = 0.0;
                    for &(j, distance) in &mesh.neighbours[i] {
                        if height[j] >= SEA { continue; }
                        let incoming = (mesh.directions[i] - mesh.directions[j]).normalize_or_zero().dot(currents[i]).max(0.0);
                        let weight = (0.15 + incoming) * (SEA - height[j]).min(12.0).sqrt() / distance;
                        supply += resource[j] * weight;
                        weights += weight;
                    }
                    let renewal = if height[i] < SEA - 28.0 { 0.22 } else { 0.012 };
                    let fed = if weights > 0.0 { supply / weights } else { 0.0 };
                    next_resource[i] = ((fed * 0.65 + resource[i] * 0.35) * (-living[i] * 0.25).exp()
                        + renewal).clamp(0.0, 1.0);
                }
                std::mem::swap(&mut resource, &mut next_resource);
            }
            for &i in &active {
                let dt = age[i] / steps as f64;
                if dt <= 0.0 { continue; }
                let depth = SEA - height[i];
                let light = if depth > 0.0 { (1.0 - smooth(1.0, 28.0, depth)).powi(2) } else { 0.0 };
                let warm = smooth(17.0, 24.0, temperature[i]);
                let neighbour = mesh.neighbours[i].iter().map(|&(j, _)| living[j]).fold(0.0, f64::max);
                let colonise = if depth > 0.5 && depth < 14.0 && reef[i] < 0.1 { 0.12 } else { 0.0 };
                let carrying = warm * light * resource[i].sqrt() * (neighbour + colonise).min(1.0) * (-sand[i] / 0.75).exp();
                let biological_fraction = if fine { super::island_clock::relaxation(dt) } else { 0.35 };
                next_living[i] = living[i] + (carrying - living[i]) * biological_fraction;
                let growth = (48.0 * dt * next_living[i]).min((depth - 0.55).max(0.0)).min(if fine { super::island_clock::growth_cap(dt) } else { 3.0 });
                reef[i] += growth;
                height[i] += growth;
                // Resource-starved and strongly exposed framework becomes
                // mobile carbonate sand, rather than disappearing.
                let wave = (1.0 - smooth(0.0, 16.0, depth.abs())) * resource[i];
                let breakdown = ((0.25 * (1.0 - resource[i]) + wave * wave * 0.7) * dt).min(reef[i]);
                reef[i] -= breakdown;
                sand[i] += breakdown;
                let weathered = ((0.55 + rain[i] * 1.5) * dt * (1.0 + age[i].sqrt() * 0.3))
                    .min((height[i] - reef[i] - sand[i] - SEA).max(0.0))
                    .min(if fine { super::island_clock::weathering_cap(dt) } else { 0.5 });
                // Weathering changes solid rock to loose grains in place.
                sand[i] += weathered;
            }
            std::mem::swap(&mut living, &mut next_living);
            // Resolve drainage on the actual changing platform, not radial
            // spokes. A downhill graph is acyclic by height and index order.
            order.sort_unstable_by(|&a, &b| height[b].total_cmp(&height[a]).then(a.cmp(&b)));
            for &i in &active {
                runoff[i] = rain[i].max(0.0) * mesh.area[i];
                receiver[i] = mesh.neighbours[i].iter().filter(|&&(j, _)| used[j] && height[j] < height[i])
                    .max_by(|&&(a, da), &&(b, db)| ((height[i] - height[a]) / da).total_cmp(&((height[i] - height[b]) / db)))
                    .map_or(i, |&(j, _)| j);
                moved[i] = 0.0;
            }
            for &i in &order {
                let j = receiver[i];
                let arriving = moved[i] / mesh.area[i];
                sand[i] += arriving;
                height[i] += arriving;
                if i == j || height[i] <= SEA - 4.0 { continue; }
                runoff[j] += runoff[i];
                let distance = mesh.directions[i].distance(mesh.directions[j]) * engine_core::cube_sphere::atlas::RADIUS_M;
                let grade = ((height[i] - height[j]) / distance).max(0.0);
                let river = (runoff[i] / 50_000.0).sqrt() * grade;
                let dt = age[i] / steps as f64;
                let bed_cut = (river * dt * 2.0 * (-(sand[i] + reef[i]) / 0.7).exp())
                    .min((height[i] - sand[i] - reef[i] - SEA).max(0.0)).min(grade * distance * 0.06);
                height[i] -= bed_cut;
                let scour = (sand[i] * -(-(0.6 + river) * dt).exp_m1()).min(grade * distance * 0.2);
                sand[i] -= scour; height[i] -= scour;
                moved[j] += (scour + bed_cut) * mesh.area[i];
            }
            // Wave/current reworking exchanges finite volumes along the
            // connected shore. Deposits are limited by wave run-up, not a ring.
            for &i in &active { moved[i] = 0.0; }
            for &i in &active {
                if sand[i] <= 0.0 || height[i] < SEA - 9.0 || height[i] > SEA + 1.8 { continue; }
                let coast = 1.0 - smooth(0.0, 9.0, (height[i] - SEA).abs());
                let volume = sand[i].min(age[i] / steps as f64 * 0.45) * coast * mesh.area[i];
                let weight = |j: usize| {
                    if !used[j] || height[j] > SEA + 1.8 { return 0.0; }
                    let along = (mesh.directions[j] - mesh.directions[i]).normalize_or_zero().dot(currents[i]);
                    (along + 0.35).max(0.0) * (1.0 - smooth(0.0, 12.0, (height[j] - SEA).abs()))
                };
                let total: f64 = mesh.neighbours[i].iter().map(|&(j, _)| weight(j)).sum();
                if total <= 0.0 { continue; }
                let mut exported = 0.0;
                for &(j, _) in &mesh.neighbours[i] {
                    let room = ((SEA + 1.8 - height[j]) * mesh.area[j] - moved[j]).max(0.0);
                    let moved_volume = (volume * weight(j) / total).min(room);
                    moved[j] += moved_volume;
                    exported += moved_volume;
                }
                moved[i] -= exported;
            }
            for &i in &active {
                let change = moved[i] / mesh.area[i];
                sand[i] += change; height[i] += change;
            }
        }
        Self { mesh, top: height.to_vec(), reef, sand }
    }

    pub(super) fn sediment(&self, dir: DVec3) -> f64 { self.mesh.sample(&self.sand, dir) }

    pub(super) fn material(&self, dir: DVec3, height: f64) -> Option<BlockId> {
        // One spherical lookup for all three column fields; most of the
        // world has no island deposit and can skip the top-height samples.
        let (ids, weights) = self.mesh.stencil(dir);
        let sample = |values: &[f64]| ids.iter().zip(weights).map(|(&i, w)| values[i] * w).sum::<f64>();
        let sand = sample(&self.sand);
        let reef = sample(&self.reef);
        if sand <= 0.15 && reef <= 0.15 { return None; }
        let top = sample(&self.top);
        let depth = top - height;
        if depth < -0.1 { return None; }
        if sand > 0.15 && depth < sand { Some(BlockId::Sand) }
        else if reef > 0.15 && depth < sand + reef { Some(BlockId::Limestone) }
        else { None }
    }
}
