//! A six-legged orchard tree-beast: root legs, a bark trunk, a swaying canopy,
//! two talon branches and the brood pods it carries. Presentation only.
use super::*;
use boss_look::{pose, scaled, tint};

struct Look {
    bark: [f32; 3],
    leaf: [f32; 3],
    bloom: [f32; 3],
    pod: [f32; 3],
}
const LOOKS: [Look; 3] = [
    Look { bark: [0.36, 0.27, 0.18], leaf: [0.32, 0.5, 0.24], bloom: [0.82, 0.3, 0.26], pod: [0.62, 0.74, 0.38] },
    Look { bark: [0.42, 0.34, 0.3], leaf: [0.46, 0.62, 0.36], bloom: [0.98, 0.72, 0.86], pod: [0.9, 0.78, 0.5] },
    Look { bark: [0.2, 0.22, 0.16], leaf: [0.24, 0.36, 0.2], bloom: [0.5, 0.72, 0.36], pod: [0.42, 0.55, 0.3] },
];

pub(in crate::world::adventure) fn model(e: &Enemy, m: &mut Model, time: f32) {
    let look = &LOOKS[boss_look::index(e)];
    let mark = m.mark();
    m.set_surface_kind(58.0);
    let run = e.matron.run.as_ref();
    let p = pose(run);
    let kind = run.map(|r| r.kind);
    let bark = tint(e, look.bark);
    let dark = tint(e, scaled(look.bark, 0.6));
    let leaf = tint(e, look.leaf);
    let o = e.pos;
    let breath = (time * 1.1).sin();
    // Stamp rears the front up then slams; Plant bows; Pollen swells the canopy.
    let (rear, drop) = match kind {
        Some(Action::Stamp) => (0.35 * p.wind, 0.25 * p.strike * p.hold),
        Some(Action::Plant) => (-0.25 * p.weight, 0.15 * p.weight),
        _ => (0.0, 0.0),
    };
    let twist = if kind == Some(Action::Rake) { 0.4 * p.wind - 0.6 * p.strike * p.hold } else { 0.0 };
    let bloom = if kind == Some(Action::Pollen) { p.weight } else { 0.0 };
    let body_q = Quat::from_rotation_y(twist) * Quat::from_rotation_x(rear);
    let jump = if kind == Some(Action::Stamp) { 0.4 * p.wind } else { 0.0 };
    let core = o + Vec3::Y * (0.95 + jump - drop + breath * 0.03);
    let at = |v: Vec3| core + body_q * v;
    // Root legs: three per side, knuckled high, with splayed root toes.
    for side in [-1.0f32, 1.0] {
        for i in 0..3 {
            let z = i as f32 * 0.55 - 0.55;
            let gait = (e.phase * 5.0 + i as f32 * 2.1 + side).sin();
            let hip = at(Vec3::new(side * 0.6, -0.15, z));
            let foot = o + Vec3::new(side * 1.25, 0.04, z * 1.1 + gait * 0.14);
            let knee = Vec3::new((hip.x + foot.x) * 0.5 + side * 0.15, hip.y + 0.15 + gait.max(0.0) * 0.12, (hip.z + foot.z) * 0.5);
            m.beam(hip, knee, 0.15, bark, 0.0);
            m.beam(knee, foot, 0.11, dark, 0.0);
            for t in [-0.2f32, 0.2] {
                m.beam(foot, foot + Vec3::new(side * 0.2, -0.03, t), 0.04, dark, 0.0);
            }
        }
    }
    // Trunk body with bark plates and a split belly where the pods hang.
    m.cuboid(core, Vec3::new(1.4, 1.1, 1.35), body_q, bark, 0.0);
    for (x, y, z, a) in [(-0.55, 0.2, -0.3, 0.2), (0.55, 0.1, 0.2, -0.25), (0.0, 0.45, 0.6, 0.1), (-0.4, -0.2, 0.55, -0.15)] {
        m.cuboid(at(Vec3::new(x, y, z)), Vec3::new(0.5, 0.42, 0.12), body_q * Quat::from_rotation_y(a + x), dark, 0.0);
    }
    let chest = at(Vec3::new(0.0, 0.85, -0.1));
    m.cuboid(chest, Vec3::new(0.95, 0.75, 0.85), body_q * Quat::from_rotation_x(0.15 * p.wind), bark, 0.0);
    // Face: a gnarled knot with two amber eyes that brighten into a tell.
    let face = at(Vec3::new(0.0, 0.95, -0.55));
    m.cuboid(face, Vec3::new(0.55, 0.42, 0.18), body_q, dark, 0.0);
    for x in [-0.16, 0.16] {
        m.cuboid(face + body_q * Vec3::new(x, 0.06, -0.1), Vec3::splat(0.1), body_q, [0.95, 0.72, 0.38], 0.3 + p.wind * 2.0);
    }
    // Canopy: leaf clusters on six bough stubs; it breathes, sways and flares for pollen.
    for i in 0..6 {
        let a = i as f32 * std::f32::consts::TAU / 6.0 + 0.3 * (time * 0.7 + i as f32).sin() * 0.2;
        let spread = 0.55 + 0.35 * bloom;
        let base = at(Vec3::new(0.0, 1.2, 0.0));
        let tip = at(Vec3::new(a.sin() * spread, 1.75 + 0.1 * breath + 0.2 * bloom, a.cos() * spread));
        m.beam(base, tip, 0.06, dark, 0.0);
        let r = body_q * Quat::from_rotation_y(a) * Quat::from_rotation_x(0.35 + p.wind * 0.3 + bloom * 0.4);
        m.cuboid(tip, Vec3::new(0.55, 0.32, 0.5), r, leaf, 0.0);
        m.cuboid(tip + Vec3::Y * 0.2, Vec3::new(0.32, 0.22, 0.3), r, scaled(leaf, 1.15), 0.0);
        m.cuboid(tip + r * Vec3::new(0.18, 0.18, -0.2), Vec3::splat(0.1), r, tint(e, look.bloom), 0.2 + bloom * 2.0);
    }
    if bloom > 0.0 {
        for i in 0..6 {
            let a = time * 2.0 + i as f32;
            m.orb(at(Vec3::new(a.cos() * 0.9, 1.9 + (a * 1.3).sin() * 0.2, a.sin() * 0.9)), 0.05, look.bloom, 1.5 * bloom);
        }
    }
    // Talon branches: Rake draws one back and drags it across; Plant lowers both.
    for side in [-1.0f32, 1.0] {
        let shoulder = at(Vec3::new(side * 0.55, 0.75, -0.1));
        let hand = match kind {
            Some(Action::Rake) => {
                let a = if p.active { -1.7 + p.strike * 3.4 } else if p.preparing { -1.9 * p.wind } else { 1.7 * p.hold };
                o + Quat::from_rotation_y(side * 0.2 + a * 0.5) * Vec3::new(side * 1.1, 1.0, -1.1)
            }
            Some(Action::Stamp) => at(Vec3::new(side * 1.0, 0.9 + 0.5 * p.wind, -0.6)),
            Some(Action::Plant) => at(Vec3::new(side * 0.5, 0.0 - 0.5 * p.wind, -0.9)),
            _ => at(Vec3::new(side * 1.05, 0.25 + 0.05 * (time * 1.3 + side).sin(), -0.55)),
        };
        let elbow = (shoulder + hand) * 0.5 + Vec3::new(side * 0.3, 0.25, 0.1);
        m.beam(shoulder, elbow, 0.15, bark, 0.0);
        m.beam(elbow, hand, 0.12, bark, 0.0);
        for f in 0..3 {
            let spread = (f as f32 - 1.0) * 0.22;
            let dir = (hand - elbow).normalize_or_zero();
            let tip = hand + dir * 0.45 + Vec3::new(side * spread, -0.12, spread * 0.5);
            m.beam(hand, tip, 0.045, dark, 0.0);
        }
    }
    // Carried brood pods hang under the belly.
    for i in 0..e.matron.pods.min(2) {
        let sway = (time * 1.6 + i as f32).sin() * 0.05;
        let lower = if kind == Some(Action::Plant) { p.wind * 0.6 } else { 0.0 };
        let c = at(Vec3::new((i as f32 - 0.5) * 0.6 + sway, -0.5 - lower, -0.55));
        m.beam(c + Vec3::Y * 0.4, c + Vec3::Y * 0.2, 0.03, dark, 0.0);
        m.cuboid(c, Vec3::new(0.42, 0.55, 0.42), Quat::from_rotation_y(0.78), tint(e, look.pod), 0.15);
        m.cuboid(c - Vec3::Y * 0.3, Vec3::new(0.2, 0.12, 0.2), Quat::IDENTITY, tint(e, look.bloom), 0.2);
    }
    m.bake_model_uv_since(mark, e.pos, 58.0);
    m.rotate_since(mark, e.pos, Quat::from_rotation_y(e.yaw));
    m.scale_since(mark, e.pos, Vec3::splat(boss_look::scale(e)));
    m.set_surface_kind(41.0);
    if let Some(r) = run {
        if r.kind == Action::Stamp && r.phase() == Phase::Prepare {
            tactics::ring(m, e.pos, 3.4, PALETTE_CORAL, 0.2);
        }
        if r.kind == Action::Plant && r.phase() == Phase::Prepare {
            tactics::ring(m, e.aim, 0.6, PALETTE_LEAF, 0.0);
        }
    }
}
