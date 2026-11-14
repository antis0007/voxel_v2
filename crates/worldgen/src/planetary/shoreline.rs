//! Wave-driven sandy shorelines on the solve mesh (generator 289).
//!
//! A one-contour-line coastline model after Ashton, Murray and Arnoult
//! (2001, Nature 414) and Ashton and Murray (2006, JGR 111). Alongshore
//! sediment flux follows the CERC relation, Q ~ cos^(6/5)(phi) sin(phi), for
//! deep-water waves meeting the local shoreline at angle phi. Flux peaks
//! near 45 degrees, so waves arriving at a high angle make the shoreline
//! anti-diffusive: bumps grow into cuspate capes, flying spits and
//! recurved hooks, while low-angle waves smooth it. Neighbouring capes
//! shadow each other from oblique swell, which limits the growth and sets
//! their spacing.
//!
//! The shoreline moves by conserving volume over the active shoreface,
//! from the closure depth to the berm crest. Only mobile cover is
//! transported, so rocky coasts keep their solved planform. The wave
//! climate is a spread of directions about the planet's zonal winds.
use super::{SEA, map_history, topology::Mesh};
use engine_core::cube_sphere::atlas::RADIUS_M;
use glam::DVec3;

/// Shoreline update steps.
const STEPS: usize = 160;
/// Steps between wave-shadow refreshes; shadows move slowly.
const SHADOW_EVERY: usize = 16;
/// Seaward toe of the active profile (closure depth), metres.
const CLOSURE: f64 = SEA - 12.0;
/// Berm crest: deposition builds beach and spit tops no higher than this.
const BERM: f64 = SEA + 2.5;
/// Height band that takes part in alongshore exchange.
const BAND: std::ops::RangeInclusive<f64> = (SEA - 14.0)..=(SEA + 6.0);
/// Alongshore velocity scale, metres per step at unit flux.
const KAPPA: f64 = 6.0;
/// Angles of the deep-water wave climate about the zonal wind, degrees.
const SPREAD: [f64; 6] = [-70.0, -42.0, -14.0, 14.0, 42.0, 70.0];
/// Distances checked toward each wave source for land shadowing, metres.
const FETCH_M: [f64; 5] = [400.0, 900.0, 1_800.0, 3_500.0, 7_000.0];

/// Deep-water wave propagation directions (unit, tangent) and weights at `d`.
fn climate(d: DVec3) -> [(DVec3, f64); 6] {
    let east = DVec3::Y.cross(d).normalize_or_zero();
    // Same zonal belts as the solve's rain and abrasion winds.
    let wind = if d.y.abs() > 0.42 && d.y.abs() < 0.82 { east } else { -east };
    let mut waves = [(DVec3::ZERO, 0.0); 6];
    let mut total = 0.0;
    for (slot, angle) in waves.iter_mut().zip(SPREAD) {
        let a = angle.to_radians();
        let dir = (wind * a.cos() + d.cross(wind) * a.sin()).normalize_or_zero();
        let weight = a.cos().max(0.0);
        *slot = (dir, weight);
        total += weight;
    }
    for slot in &mut waves { slot.1 /= total.max(1e-9); }
    waves
}

/// Seaward unit normal of the local shoreline, or `None` on flat ground.
fn seaward(mesh: &Mesh, height: &[f64], i: usize) -> Option<DVec3> {
    let d = mesh.directions[i];
    let mut gradient = DVec3::ZERO;
    for &(j, distance) in &mesh.neighbours[i] {
        let edge = mesh.directions[j] - d;
        let tangent = (edge - d * edge.dot(d)).normalize_or_zero();
        gradient += tangent * ((height[j] - height[i]) / distance.max(1e-9));
    }
    (gradient.length() > 1e-5).then(|| -gradient.normalize())
}

/// True when land blocks waves arriving at node `i` from `-wave`.
fn shadowed(mesh: &Mesh, height: &[f64], i: usize, wave: DVec3) -> bool {
    let d = mesh.directions[i];
    FETCH_M.iter().any(|&m| mesh.sample(height, (d - wave * (m / RADIUS_M)).normalize()) > SEA)
}

/// Runs the shoreline model in place. Volume above the berm is dropped, so a
/// full spit stops growing upward and extends alongshore instead. `cover` is mobile sediment thickness
/// in metres and is updated with the height. `strength` scales transport.
pub(super) fn evolve(mesh: &Mesh, height: &mut [f64], cover: &mut [f64], strength: f64, workers: usize) {
    let n = height.len();
    if cover.len() != n { return; }
    let mut open = vec![[true; 6]; n];
    let mut flux = vec![DVec3::ZERO; n];
    let mut change = vec![0.0; n];
    let mut outgoing = vec![0.0; n];
    let (mut moved, mut active_peak) = (0.0, 0usize);
    for step in 0..STEPS {
        let active: Vec<usize> = (0..n).filter(|&i| BAND.contains(&height[i])).collect();
        active_peak = active_peak.max(active.len());
        if step % SHADOW_EVERY == 0 {
            let h: &[f64] = height;
            let shade = map_history(active.len(), workers, |k| {
                let i = active[k];
                let mut lit = [true; 6];
                for (slot, (wave, _)) in lit.iter_mut().zip(climate(mesh.directions[i])) {
                    *slot = !shadowed(mesh, h, i, wave);
                }
                lit
            });
            for (&i, lit) in active.iter().zip(shade) { open[i] = lit; }
        }
        // Alongshore flux vector per node: CERC with the shoreline angle.
        flux.fill(DVec3::ZERO);
        let h: &[f64] = height;
        let q = map_history(active.len(), workers, |k| {
            let i = active[k];
            let Some(normal) = seaward(mesh, h, i) else { return DVec3::ZERO };
            let mut q = DVec3::ZERO;
            for (slot, (wave, weight)) in climate(mesh.directions[i]).into_iter().enumerate() {
                // Waves travel toward the shore: their reverse faces seaward.
                let cos = -wave.dot(normal);
                if cos <= 0.0 || !open[i][slot] { continue; }
                let along = wave - normal * wave.dot(normal);
                q += along * (weight * cos.powf(1.2));
            }
            q
        });
        for (&i, q) in active.iter().zip(q) { flux[i] = q; }
        // Edge exchange through the shoreface cross-section, donors limited by their cover.
        change.fill(0.0);
        outgoing.fill(0.0);
        let depth = BERM - CLOSURE;
        let edge_volume = |i: usize, j: usize| {
            let edge = (mesh.directions[j] - mesh.directions[i]).normalize_or_zero();
            let width = 0.5 * (mesh.area[i].sqrt() + mesh.area[j].sqrt());
            KAPPA * strength * 0.5 * (flux[i] + flux[j]).dot(edge) * width * depth
        };
        for &i in &active {
            for &(j, _) in &mesh.neighbours[i] {
                if j <= i || !BAND.contains(&height[j]) { continue; }
                let v = edge_volume(i, j);
                if v > 0.0 { outgoing[i] += v } else { outgoing[j] -= v }
            }
        }
        let scale = |i: usize, height: &[f64]| {
            let sand = cover[i].min(height[i] - CLOSURE).max(0.0) * mesh.area[i];
            if outgoing[i] > sand { sand / outgoing[i] } else { 1.0 }
        };
        for &i in &active {
            for &(j, _) in &mesh.neighbours[i] {
                if j <= i || !BAND.contains(&height[j]) { continue; }
                let v = edge_volume(i, j);
                let v = if v > 0.0 { v * scale(i, height) } else { v * scale(j, height) };
                change[i] -= v;
                change[j] += v;
                moved += v.abs();
            }
        }
        for &i in &active {
            let dh = change[i] / mesh.area[i];
            if dh == 0.0 { continue; }
            // Spits and beaches build to the berm; extra volume stays on the shoreface.
            let top = if dh > 0.0 { (height[i] + dh).min(BERM.max(height[i])) } else { height[i] + dh };
            let applied = top - height[i];
            height[i] = top;
            cover[i] = (cover[i] + applied).max(0.0);
        }
    }
    eprintln!("[worldgen] shoreline steps={STEPS} active_nodes={active_peak} moved_m3={moved:.3e} strength={strength:.2}");
}
