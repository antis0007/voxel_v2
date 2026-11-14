//! A cold interval on the same sphere later drained by liquid water.
use super::{SEA, hydrology, noise, smooth, topology::Mesh};
use engine_core::cube_sphere::{Projection, atlas};
use glam::DVec3;

pub(super) fn carve(mesh: &Mesh, height: &mut [f64], rain: &[f64], temperature: &[f64]) -> Vec<f64> {
    let mut former = vec![0.0_f64; height.len()];
    let mut cold = vec![0.0; height.len()];
    let mut snow = vec![0.0; height.len()];
    let mut ice = vec![0.0_f64; height.len()];
    let mut cut = vec![0.0_f64; height.len()];
    let mut till = vec![0.0_f64; height.len()];
    let mut carried = vec![0.0_f64; height.len()];
    let mut eroded = 0.0;
    for _ in 0..5 {
        for i in 0..height.len() {
            // The game's vertical scale compresses the snowline with the
            // relief. Temperature and precipitation still set accumulation.
            cold[i] = 7.5 + (height[i] - SEA).max(0.0) * 0.032 - temperature[i];
            snow[i] = rain[i] * smooth(1.5, 9.0, cold[i])
                * smooth(SEA + 55.0, SEA + 155.0, height[i]);
            ice[i] = snow[i] * mesh.area[i];
        }
        let flow = hydrology::route(mesh, height, &snow, SEA);
        cut.fill(0.0);
        till.fill(0.0);
        carried.fill(0.0);
        for &i in flow.order.iter().rev() {
            let r = flow.receiver[i];
            if r == i || height[i] <= SEA { continue; }
            let melt = smooth(-1.5, 9.0, -cold[i]);
            let flux = ice[i] * (1.0 - 0.69 * melt);
            ice[r] += flux;
            let glacier = smooth(170_000.0, 1_600_000.0, flux);
            let fall = (height[i] - height[r]).max(0.0);
            let abrasion = 4.5 * glacier * smooth(1.5, 18.0, fall);
            former[i] = former[i].max(glacier);
            cut[i] = cut[i].max(abrasion);
            if glacier > 0.01 {
                let direction = (mesh.directions[r] - mesh.directions[i]).normalize();
                for &(j, _) in &mesh.neighbours[i] {
                    let side = (mesh.directions[j] - mesh.directions[i]).normalize();
                    let cross = 1.0 - side.dot(direction).abs();
                    let rise = height[j] - height[i];
                    if rise < 0.0 || rise > 52.0 { continue; }
                    let widening = abrasion * cross * (1.0 - smooth(12.0, 52.0, rise)) * 0.53;
                    cut[j] = cut[j].max(widening);
                    former[j] = former[j].max(glacier * cross * 0.53);
                }
            }
        }
        for &i in flow.order.iter().rev() {
            let r = flow.receiver[i];
            if height[i] <= SEA { continue; }
            let amount = cut[i].min((height[i] - SEA - 0.25).max(0.0));
            height[i] -= amount;
            let fresh = amount * mesh.area[i];
            eroded += fresh;
            let debris = carried[i] + fresh;
            let thaw = smooth(-2.0, 9.0, -cold[i]);
            let deposit = (debris * thaw * former[i] * 0.38).min(2.4 * mesh.area[i]);
            till[i] += deposit / mesh.area[i];
            if r != i { carried[r] += debris - deposit; }
        }
        for (h, s) in height.iter_mut().zip(&till) { *h += s; }
    }
    let marked = former.iter().filter(|&&v| v > 0.1).count();
    eprintln!("[worldgen] former_glaciers marked_nodes={marked} rock_removed_m3={eroded:.1}");
    former
}

pub(super) fn erratic(mesh: &Mesh, ice: &[f64], dir: DVec3, seed: u32, footprint: f32) -> f64 {
    if ice.is_empty() || footprint >= 9.0 || mesh.sample(ice, dir) < 0.08 { return 0.0; }
    let (face, s, t) = Projection::Tangent.from_sphere(dir);
    let x = s * atlas::HALF_M;
    let z = t * atlas::HALF_M;
    let pitch = 43.0;
    let cell = DVec3::new((x / pitch).floor(), face as u8 as f64 * 197.0, (z / pitch).floor());
    let unit = |salt| noise(seed, cell * 17.11, salt);
    if unit(9401) > 0.28 * mesh.sample(ice, dir) { return 0.0; }
    let dx = x.rem_euclid(pitch) - pitch * (0.21 + 0.58 * unit(9409));
    let dz = z.rem_euclid(pitch) - pitch * (0.21 + 0.58 * unit(9413));
    let (sin, cos) = (unit(9417) * std::f64::consts::TAU).sin_cos();
    let radius = 1.25 + 3.0 * unit(9421).powi(2);
    let square = ((cos * dx + sin * dz) / radius).powi(2)
        + ((cos * dz - sin * dx) / (0.64 * radius)).powi(2);
    0.82 * radius * (1.0 - square * square).max(0.0).sqrt()
        * (1.0 - smooth(3.0, 9.0, f64::from(footprint)))
}
