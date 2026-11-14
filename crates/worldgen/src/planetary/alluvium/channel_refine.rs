//! Nested channel routing inside a shared spherical drainage edge.
use super::{Flow, Mesh, Migration, Reach, SEA, SOURCE_AREA, Section, on_sphere, smooth};
use crate::planet::noise_at;
use glam::DVec3;

/// Give all the reaches at a confluence one low point in the same transect.
pub(in crate::planetary) fn channel_anchors(
    mesh: &Mesh, height: &[f64], flow: &Flow, erodibility: &[f64], seed: u32,
) -> Vec<DVec3> {
    let mut anchors: Vec<_> = mesh.directions.iter().copied().map(on_sphere).collect();
    let mut incoming = vec![None; anchors.len()];
    for (i, &r) in flow.receiver.iter().enumerate() {
        if i != r && height[i] > SEA && incoming[r].is_none_or(|j| flow.discharge[i] > flow.discharge[j]) {
            incoming[r] = Some(i);
        }
    }
    for (i, &r) in flow.receiver.iter().enumerate() {
        if i == r || height[r] <= SEA || flow.discharge[i] < SOURCE_AREA
            || flow.spill[i] > height[i] + 0.75 || flow.spill[r] > height[r] + 0.75
        {
            continue;
        }
        let a = mesh.directions[i];
        let b = mesh.directions[r];
        let start = anchors[i];
        let length = start.distance(on_sphere(b));
        let grade = (height[i] - height[r]).max(0.0) / length;
        let width = Section::from_runoff(flow.discharge[i], grade).width;
        let mobility = mobility(width, grade);
        let upstream = incoming[i].map_or(a, |j| mesh.directions[j]);
        let normal = a.cross(b - upstream).normalize_or_zero();
        let range = (4.0 * width + 12.0).min(0.20 * length) * mobility;
        let mut best = (0.0, f64::INFINITY);
        for step in -4..=4 {
            let fraction = f64::from(step) * 0.25;
            let candidate = start + normal * (range * fraction);
            let rise = mesh.sample(height, candidate.normalize()) - height[i];
            if rise > 1.5 + 2.0 * mobility { continue; }
            let weak = material(mesh, erodibility, candidate, seed);
            let cost = rise * 0.65 + fraction * fraction * 0.65 - weak * 2.5;
            if cost < best.1 { best = (range * fraction, cost); }
        }
        anchors[i] = on_sphere(start + normal * best.0);
    }
    anchors
}

pub(super) fn material(mesh: &Mesh, erodibility: &[f64], p: DVec3, seed: u32) -> f64 {
    mesh.sample(erodibility, p.normalize())
        + 0.54 * noise_at(p / 87.0, seed.wrapping_add(1203))
        + 0.21 * noise_at(p / 26.0, seed.wrapping_add(1207))
}

fn mobility(width: f64, grade: f64) -> f64 {
    (1.0 - smooth(0.009, 0.049, grade)) * smooth(1.7, 5.8, width)
}

pub(super) fn evolve(
    reach: &mut Reach, mesh: &Mesh, height: &[f64], erodibility: &[f64],
    runoff: f64, grade: f64, seed: u32, branching: bool,
) {
    let width = reach.start.width.max(reach.end.width);
    if branching && (width < 5.5 || grade >= 0.023) { return; }
    let freedom = mobility(width, grade);
    if freedom <= 0.0 { return; }
    let [a, b] = [reach.curve[0], reach.curve[reach.curve.len() - 1]];
    let length = a.distance(b);
    if length < 4.0 * width { return; }
    let valley_axis = std::array::from_fn(|i| {
        let at = i as f64 * 0.25 * (reach.curve.len() - 1) as f64;
        let j = (at as usize).min(reach.curve.len() - 2);
        reach.curve[j].lerp(reach.curve[j + 1], at - j as f64)
    });
    let across = (a + b).normalize().cross((b - a).normalize());
    let bound = (8.0 * width + 13.0).min(length * 0.34);
    let cycles = 18;
    let mut former: Vec<[f64; 3]> = Vec::new();
    for (steps, refinements) in [(12, cycles), (28, 12)] {
        let mut axes = Vec::with_capacity(steps + 1);
        let mut banks = Vec::with_capacity(steps + 1);
        let mut profile = Vec::with_capacity(steps + 1);
        for i in 0..=steps {
            let t = i as f64 / steps as f64;
            let original = t * (reach.curve.len() - 1) as f64;
            let j = (original as usize).min(reach.curve.len() - 2);
            let axis = reach.curve[j].lerp(reach.curve[j + 1], original - j as f64);
            let low = mesh.sample(height, axis.normalize());
            // Test bank heights along each transect. A nearby higher
            // terrace reduces migration toward that side.
            let support = |side: f64| {
                let mut span = 0.0;
                let mut previous_head = low;
                let limit = low + 2.5 + 5.0 * freedom;
                for f in [0.24, 0.48, 0.72, 1.0] {
                    let head = mesh.sample(height, (axis + across * (side * bound * f)).normalize());
                    if head > limit {
                        if branching {
                            let cross = (limit - previous_head) / (head - previous_head);
                            span += (bound * f - span) * cross;
                        }
                        break;
                    }
                    span = bound * f;
                    previous_head = head;
                }
                span
            };
            let limit = [support(-1.0), support(1.0)];
            let gate = (4.0 * t * (1.0 - t)).powi(2);
            let material_offsets = [-bound, -0.5 * bound, 0.0, 0.5 * bound, bound];
            let weakness = material_offsets.map(|m| {
                let point = axis + across * m;
                if branching {
                    mesh.sample(erodibility, point.normalize())
                        + 0.57 * noise_at(point / (8.0 * width).max(108.0), seed.wrapping_add(1203))
                } else { material(mesh, erodibility, point, seed) }
            });
            let old = if former.is_empty() { [0.0; 3] } else {
                let at = t * (former.len() - 1) as f64;
                let j = (at as usize).min(former.len() - 2);
                std::array::from_fn(|k| former[j][k] + (at - j as f64) * (former[j + 1][k] - former[j][k]))
            };
            axes.push(axis);
            banks.push(([-limit[0] * gate, limit[1] * gate], weakness, gate));
            profile.push(old);
        }
        let mut next = profile.clone();
        let run = length / steps as f64;
        let speed = (runoff / 950_000.0).powf(0.25).clamp(0.55, 2.0);
        let adjustment = if branching { 1.0 - (-run / (2.6 * width)).exp() } else { 0.34 };
        for _ in 0..refinements {
            let mut upstream_turn = 0.0;
            for i in 1..steps {
                let [centre, low, high] = profile[i];
                let before = axes[i - 1] + across * profile[i - 1][0];
                let here = axes[i] + across * centre;
                let after = axes[i + 1] + across * profile[i + 1][0];
                let turn = (after + before - 2.0 * here).dot(across) / run.max(1.0);
                let (limit, weakness, gate) = banks[i];
                let strength = |side: f64| {
                    let at = ((centre + side * width) / bound * 2.0 + 2.0).clamp(0.0, 4.0);
                    let j = (at as usize).min(3);
                    weakness[j] + (weakness[j + 1] - weakness[j]) * (at - j as f64)
                };
                // Flow reacts to the upstream bend with a lag. A local
                // curvature penalty prevents single-station kinks.
                let pressure = 2.25 * upstream_turn - 0.32 * turn + 3.5 * (strength(1.0) - strength(-1.0));
                upstream_turn += (turn - upstream_turn) * adjustment;
                let retreat = pressure.tanh() * (width * 0.51 * speed * freedom * gate);
                let smoothing = (profile[i - 1][0] + profile[i + 1][0] - 2.0 * centre) * 0.24;
                let moved = (centre + retreat + smoothing).clamp(limit[0], limit[1]);
                next[i] = [moved, low.min(moved), high.max(moved)];
            }
            std::mem::swap(&mut profile, &mut next);
        }
        if steps == 28 {
            if branching {
                // Bank motion below one channel width is unresolved at this
                // scale. Relax those offsets inside the solved valley limits.
                let rounds = (width / run).ceil().clamp(1.0, 4.0) as usize;
                for _ in 0..rounds {
                    for i in 1..steps {
                        let (limit, _, _) = banks[i];
                        next[i] = profile[i];
                        next[i][0] = (0.5 * profile[i][0] + 0.25 * (profile[i - 1][0] + profile[i + 1][0]))
                            .clamp(limit[0], limit[1]);
                    }
                    std::mem::swap(&mut profile, &mut next);
                }
            }
            reach.curve = axes.into_iter().zip(&profile).map(|(p, m)| on_sphere(p + across * m[0])).collect();
        }
        former = profile;
    }
    let swept = former.iter().map(|m| m[2] - m[1]).fold(0.0, f64::max);
    reach.broad_plain = swept > 0.75 * width && grade < 0.020;
    reach.valley = reach.valley.max(swept * 1.35 + width * 1.4 + 12.0);
    let banks = former.iter().map(|m| {
        let span = m[2] - m[1];
        // Eroded cross-section spread across the swept belt. The inner
        // shelf is capped by the local floodplain grade at sampling time.
        let fill = 0.45 * span * reach.start.depth / (span + 2.0 * width);
        [(span + 0.4 * width).min(0.78 * reach.valley), fill]
    }).collect();
    reach.migration = Some(Migration { valley_axis, banks });
    reach.curvature = (0..reach.curve.len()).map(|i| {
        if i == 0 || i + 1 == reach.curve.len() { return 0.0; }
        let a = reach.curve[i] - reach.curve[i - 1];
        let b = reach.curve[i + 1] - reach.curve[i];
        let normal = reach.curve[i].normalize().cross((a + b).normalize_or_zero());
        2.0 * (b.normalize_or_zero() - a.normalize_or_zero()).dot(normal)
            / (a.length() + b.length()).max(1.0)
    }).collect();
}
