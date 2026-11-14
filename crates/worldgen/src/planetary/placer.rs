use super::{SEA, hydrology::Flow, noise, smooth, topology::Mesh};
use engine_core::cube_sphere::atlas::RADIUS_M;

/// Relative enrichment at the end of each receiver link. Gold enters the
/// network with eroded rock and settles faster where transport power drops.
pub(super) fn route(
    seed: u32,
    mesh: &Mesh,
    height: &[f64],
    flow: &Flow,
    erosion: &[f64],
    collision: &[f64],
    rift: &[f64],
) -> Vec<f64> {
    let mut gold = vec![0.0; height.len()];
    let mut gravel = vec![0.0; height.len()];
    let mut grade = vec![0.0; height.len()];
    let power = |i: usize, r: usize| {
        let run = (mesh.directions[i].distance(mesh.directions[r]) * RADIUS_M).max(1.0);
        flow.discharge[i].sqrt() * (height[i] - height[r]).max(0.0) / run
    };
    for &i in flow.order.iter().rev() {
        let r = flow.receiver[i];
        if r == i || height[i] <= SEA {
            continue;
        }
        let p = mesh.directions[i] * RADIUS_M;
        let belt = 1.0 - smooth(0.018, 0.105, (noise(seed, p / 6300.0, 661) - 0.5).abs());
        let host = smooth(0.10, 0.65, collision[i].max(rift[i]));
        gold[i] += erosion[i] * belt * host;
        gravel[i] += erosion[i];
        let here = power(i, r);
        let downstream = power(r, flow.receiver[r]);
        let slowdown = (1.0 - downstream / (here + 0.02)).clamp(0.0, 1.0);
        let lake = smooth(0.75, 8.0, flow.spill[r] - height[r]);
        let gold_settles = (0.04 + 0.45 * slowdown).max(0.82 * lake);
        let gravel_settles = (0.02 + 0.22 * slowdown).max(0.70 * lake);
        let retained = gold[i] * gold_settles;
        // Small feeder gullies contribute their sediment, but a trace in an
        // otherwise empty reach is insufficient to create mineable gravel.
        grade[i] = (retained * 6.0 / (gravel[i] * gravel_settles + mesh.area[i] * 0.003)).min(1.0);
        gold[r] += gold[i] - retained;
        gravel[r] += gravel[i] * (1.0 - gravel_settles);
    }
    grade
}
