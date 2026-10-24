//! An armoured warden with a pike and a portable timber gate. Presentation
//! reads the action clock and the owned gate stock; it never decides contact.
use super::*;
use boss_look::{pose, scaled, tint};

struct Look {
    iron: [f32; 3],
    timber: [f32; 3],
    visor: [f32; 3],
    ember: f32,
}
const LOOKS: [Look; 3] = [
    Look { iron: [0.25, 0.28, 0.29], timber: [0.32, 0.21, 0.12], visor: [0.86, 0.61, 0.28], ember: 0.0 },
    Look { iron: [0.24, 0.42, 0.36], timber: [0.5, 0.4, 0.26], visor: [0.5, 0.95, 0.75], ember: 0.0 },
    Look { iron: [0.13, 0.12, 0.12], timber: [0.18, 0.1, 0.06], visor: [1.0, 0.42, 0.12], ember: 0.6 },
];

pub(in crate::world::adventure) fn model(e: &Enemy, m: &mut Model, time: f32) {
    let look = &LOOKS[boss_look::index(e)];
    let mark = m.mark();
    m.set_surface_kind(58.0);
    let o = e.pos;
    let run = e.toll.run.as_ref();
    let p = pose(run);
    let kind = run.map(|r| r.kind);
    let iron = tint(e, look.iron);
    let dark = tint(e, scaled(look.iron, 0.65));
    let timber = tint(e, look.timber);
    let breath = (time * 1.4).sin();
    let gait = (e.phase * 5.5).sin();
    let bob = gait.abs() * 0.04;
    // Whole-body lean: back into the coil, forward through the strike.
    let lean = match kind {
        Some(Action::Thrust) => -0.18 * p.wind + 0.3 * p.strike * p.hold,
        Some(Action::Shove) => -0.12 * p.wind + 0.25 * p.strike * p.hold,
        Some(Action::Plant) | Some(Action::Retrieve) => 0.3 * p.weight,
        _ => 0.0,
    };
    let turn = match kind {
        Some(Action::Sweep) => 0.5 * p.wind - 0.8 * p.strike * p.hold,
        _ => 0.0,
    };
    let crouch = 0.12 * p.wind + 0.06 * p.hold;
    let pelvis = o + Vec3::Y * (1.0 - crouch + bob);
    let torso_q = Quat::from_rotation_y(turn) * Quat::from_rotation_x(-lean);
    let up = |v: Vec3| pelvis + torso_q * v;
    // Legs: thigh, knee guard, shin, boot. Stance widens through the wind-up.
    for side in [-1.0f32, 1.0] {
        let stride = gait * side * 0.22;
        let spread = side * (0.22 + 0.1 * p.wind);
        let hip = pelvis + Vec3::new(spread, -0.05, 0.0);
        let foot = o + Vec3::new(spread * 1.2, 0.1, stride - side * 0.15 * p.wind);
        let knee = (hip + foot) * 0.5 + Vec3::new(0.0, 0.05, -0.16 - crouch * 0.8);
        m.beam(hip, knee, 0.14, dark, 0.0);
        m.beam(knee, foot, 0.12, iron, 0.0);
        m.cuboid(knee + Vec3::new(0.0, 0.0, -0.08), Vec3::new(0.2, 0.2, 0.12), Quat::IDENTITY, iron, 0.0);
        m.cuboid(foot + Vec3::new(0.0, -0.02, -0.08), Vec3::new(0.26, 0.18, 0.44), Quat::IDENTITY, dark, 0.0);
    }
    // Tabard plates hang from the belt and trail the motion.
    for (i, x) in [-0.25f32, 0.0, 0.25].into_iter().enumerate() {
        let swing = Quat::from_rotation_x(0.12 * gait * if i == 1 { 1.0 } else { -1.0 } + 0.2 * lean);
        m.cuboid(pelvis + Vec3::new(x, -0.28, -0.24), Vec3::new(0.24, 0.5, 0.05), swing, timber, look.ember * 0.3);
    }
    m.cuboid(pelvis, Vec3::new(0.7, 0.22, 0.45), Quat::IDENTITY, dark, 0.0);
    // Breastplate, ribbed back, pauldrons, gorget.
    m.cuboid(up(Vec3::new(0.0, 0.45 + breath * 0.015, 0.0)), Vec3::new(0.82, 0.78, 0.5), torso_q, iron, 0.0);
    m.cuboid(up(Vec3::new(0.0, 0.5, -0.26)), Vec3::new(0.5, 0.5, 0.06), torso_q * Quat::from_rotation_x(0.12), dark, 0.0);
    for y in [0.25, 0.45, 0.65] {
        m.cuboid(up(Vec3::new(0.0, y, 0.26)), Vec3::new(0.7, 0.06, 0.05), torso_q, dark, 0.0);
    }
    for side in [-1.0f32, 1.0] {
        let lift = 0.08 * p.wind * if side > 0.0 { 1.0 } else { 0.4 };
        m.cuboid(up(Vec3::new(side * 0.52, 0.82 + lift, 0.0)), Vec3::new(0.42, 0.24, 0.6), torso_q * Quat::from_rotation_z(-side * 0.32), iron, 0.0);
        m.cuboid(up(Vec3::new(side * 0.6, 0.66 + lift, 0.0)), Vec3::new(0.32, 0.14, 0.5), torso_q * Quat::from_rotation_z(-side * 0.5), dark, 0.0);
    }
    m.cuboid(up(Vec3::new(0.0, 0.9, 0.0)), Vec3::new(0.5, 0.16, 0.46), torso_q, dark, 0.0);
    // Great helm: visor slit, cheek plates and a timber crest.
    let helm_q = torso_q * Quat::from_rotation_x(-0.15 * p.wind);
    let helm = up(Vec3::new(0.0, 1.15 + breath * 0.02, 0.0));
    m.cuboid(helm, Vec3::new(0.46, 0.5, 0.44), helm_q, iron, 0.0);
    m.cuboid(helm + helm_q * Vec3::new(0.0, 0.06, -0.22), Vec3::new(0.52, 0.12, 0.1), helm_q, dark, 0.0);
    m.cuboid(helm + helm_q * Vec3::new(0.0, 0.05, -0.24), Vec3::new(0.3, 0.05, 0.03), helm_q, look.visor, 0.6 + p.wind * 2.0);
    for side in [-1.0f32, 1.0] {
        m.cuboid(helm + helm_q * Vec3::new(side * 0.22, -0.15, -0.12), Vec3::new(0.06, 0.24, 0.24), helm_q, dark, 0.0);
    }
    m.cuboid(helm + helm_q * Vec3::new(0.0, 0.33, 0.05), Vec3::new(0.08, 0.2, 0.5), helm_q, timber, look.ember);
    // Pike arm (right). The strike vector is in body space, like the old pose,
    // but now coils back first and overshoots before it settles.
    let shoulder = up(Vec3::new(0.42, 0.78, 0.0));
    let (hand, shaft) = match kind {
        Some(Action::Thrust) => {
            let back = Vec3::new(0.5, 0.35, 0.45);
            let out = Vec3::new(0.35, 0.4, -0.85);
            (back.lerp(out, p.strike * p.hold), Vec3::new(0.0, 0.05 * p.wind, -1.0))
        }
        Some(Action::Sweep) => {
            let a = if p.active { -1.65 + p.strike * 3.3 } else if p.preparing { -p.wind * 1.9 } else { 1.65 * p.hold };
            let r = Quat::from_rotation_y(a);
            (Vec3::new(0.55, 0.45 + 0.35 * p.wind, -0.1), r * Vec3::new(0.0, -0.08, -1.0))
        }
        Some(Action::Shove) => (Vec3::new(0.5, 0.3, -0.2), Vec3::new(0.0, 0.7, -0.3).normalize()),
        _ => (Vec3::new(0.55, 0.3, -0.12), Vec3::new(0.0, 0.92, -0.38).normalize()),
    };
    let hand = up(hand);
    let elbow = (shoulder + hand) * 0.5 + torso_q * Vec3::new(0.18, -0.1, 0.15);
    m.beam(shoulder, elbow, 0.14, iron, 0.0);
    m.beam(elbow, hand, 0.12, dark, 0.0);
    m.cuboid(hand, Vec3::new(0.18, 0.18, 0.2), torso_q, iron, 0.0);
    let dir = torso_q * shaft;
    m.beam(hand - dir * 0.9, hand + dir * 2.4, 0.06, timber, look.ember * 0.2);
    let head = hand + dir * 2.4;
    m.beam(head, head + dir * 0.55, 0.08, iron, p.wind * 0.4);
    let side = dir.cross(Vec3::Y).normalize_or_zero();
    m.beam(head + dir * 0.1, head + dir * 0.1 + side * 0.35 - Vec3::Y * 0.12, 0.09, iron, 0.0);
    m.beam(head + dir * 0.15, head + dir * 0.15 - side * 0.18, 0.05, dark, 0.0);
    // Gate arm (left): a carried palisade of the stock it still owns.
    let gate_shoulder = up(Vec3::new(-0.42, 0.78, 0.0));
    if e.toll.stock > 0 || !e.toll.initialized {
        let lowering = if kind == Some(Action::Plant) { p.wind } else { 0.0 };
        let raise = if kind == Some(Action::Shove) { 0.25 * p.wind + 0.4 * p.strike * p.hold } else { 0.0 };
        let gate = o + Vec3::new(-0.95, 1.35 - lowering * 0.1 + raise * 0.2, -0.45 - lowering * 0.9 - raise);
        for x in [-1.25, 1.25] {
            m.cuboid(gate + Vec3::X * x, Vec3::new(0.22, 2.5, 0.22), Quat::IDENTITY, iron, 0.0);
            m.cuboid(gate + Vec3::new(x, 1.3, 0.0), Vec3::new(0.3, 0.12, 0.3), Quat::IDENTITY, dark, 0.0);
        }
        // The sight gap between the low and high boards matches the planted gate.
        for (i, y) in [-1.05f32, -0.65, -0.25, 0.45, 0.8, 1.15].into_iter().enumerate() {
            let warp = Quat::from_rotation_z(0.03 * (i as f32 * 1.7).sin());
            m.cuboid(gate + Vec3::Y * y, Vec3::new(2.5, 0.3, 0.16), warp, timber, look.ember * 0.25);
        }
        m.beam(gate + Vec3::new(-1.2, -1.05, -0.1), gate + Vec3::new(1.2, 1.15, -0.1), 0.05, dark, 0.0);
        m.beam(gate_shoulder, gate + Vec3::new(0.9, 0.1, 0.1), 0.15, iron, 0.0);
    } else {
        let reach = if kind == Some(Action::Retrieve) { p.wind * 1.15 } else { 0.0 };
        let hand = o + Vec3::new(-0.6, 1.0 + 0.1 * breath, -reach);
        m.beam(gate_shoulder, hand, 0.15, iron, 0.0);
        for f in [-0.08f32, 0.0, 0.08] {
            m.beam(hand, hand + Vec3::new(f, -0.12, -0.15), 0.03, dark, 0.0);
        }
    }
    m.bake_model_uv_since(mark, o, 58.0);
    m.rotate_since(mark, o, Quat::from_rotation_y(e.yaw));
    m.scale_since(mark, o, Vec3::splat(boss_look::scale(e)));
    m.set_surface_kind(41.0);
}
