//! A hen, enlarged: heavy body over thick legs, a small head that stays level
//! while everything else moves, stepped copper plumage, a comb that brightens
//! with stored charge and hackles that rise before a discharge. Presentation only.
use super::*;
use boss_look::{pose, scaled, tint};

struct Look {
    plume: [f32; 3],
    rust: [f32; 3],
    comb: [f32; 3],
}
const LOOKS: [Look; 3] = [
    Look { plume: [0.72, 0.42, 0.2], rust: [0.48, 0.24, 0.12], comb: [0.95, 0.22, 0.14] },
    Look { plume: [0.55, 0.3, 0.16], rust: [0.36, 0.18, 0.1], comb: [0.9, 0.3, 0.1] },
    Look { plume: [0.46, 0.6, 0.5], rust: [0.3, 0.42, 0.36], comb: [0.85, 0.2, 0.2] },
];

pub(in crate::world::adventure) fn model(e: &Enemy, m: &mut Model, time: f32) {
    let mark = m.mark();
    let look = &LOOKS[boss_look::index(e)];
    m.set_surface_kind(58.0);
    let run = e.hen.run.as_ref();
    let p = pose(run);
    let kind = run.map(|r| r.kind);
    let plume = tint(e, look.plume);
    let rust = tint(e, look.rust);
    let dark = tint(e, scaled(look.rust, 0.55));
    let o = e.pos;
    let breath = (time * 0.9).sin();
    let charge = e.hen.charge as f32 / CHARGE_MAX as f32;
    // Peck lowers and extends the neck; Scratch rocks the body forward onto one
    // foot; Roost settles the body onto the ground; the Look holds dead still.
    let (crouch, lean) = match kind {
        Some(Action::Peck) => (0.3 * p.weight, 0.35 * p.wind - 0.5 * p.strike * p.hold),
        Some(Action::Scratch) => (0.15 * p.weight, 0.25 * p.weight),
        Some(Action::Roost) => (0.9 * p.weight, 0.0),
        _ => (0.0, 0.0),
    };
    let body_q = Quat::from_rotation_y(e.yaw) * Quat::from_rotation_x(lean);
    let core = o + Vec3::Y * (2.1 - crouch + breath * 0.03);
    let at = |v: Vec3| core + body_q * v;
    let roosting = kind == Some(Action::Roost);
    // Legs: scaled shanks, knuckled knees, three toes each.
    for side in [-1.0f32, 1.0] {
        let gait = if roosting { 0.0 } else { (e.phase * 3.2 + side * 1.6).sin() };
        let hip = at(Vec3::new(side * 0.6, -0.6, 0.1));
        let foot = o + Vec3::new(side * 0.75, 0.05, -0.2 + gait * 0.35);
        let knee = Vec3::new((hip.x + foot.x) * 0.5, hip.y - 0.55 + gait.max(0.0) * 0.2, (hip.z + foot.z) * 0.5 - 0.3);
        m.beam(hip, knee, 0.22, rust, 0.0);
        m.beam(knee, foot, 0.16, [0.82, 0.68, 0.3], 0.0);
        for toe in [-0.35f32, 0.0, 0.35] {
            m.beam(foot, foot + Vec3::new(toe * 0.8, -0.02, -0.55), 0.06, [0.72, 0.58, 0.26], 0.0);
        }
    }
    // Body: one heavy block, then stepped feather clusters down the flanks.
    m.cuboid(core, Vec3::new(2.0, 1.7, 2.6), body_q, plume, 0.0);
    for (i, (x, y, z)) in [(-1.0, 0.3, -0.4), (1.0, 0.3, -0.4), (-0.95, -0.2, 0.5), (0.95, -0.2, 0.5), (0.0, 0.75, 0.3)].into_iter().enumerate() {
        let r = body_q * Quat::from_rotation_y(0.25 * (i as f32 - 2.0));
        m.cuboid(at(Vec3::new(x, y, z)), Vec3::new(0.55, 0.5, 0.95), r, rust, 0.0);
    }
    // Tail: a raised fan of three plates.
    for (i, a) in [-0.5f32, 0.0, 0.5].into_iter().enumerate() {
        let r = body_q * Quat::from_rotation_y(a) * Quat::from_rotation_x(-0.9 - 0.1 * i as f32);
        m.cuboid(at(Vec3::new(a * 0.6, 0.9, 1.5)), Vec3::new(0.35, 1.3, 0.12), r, if i == 1 { dark } else { rust }, 0.0);
    }
    // Neck and head. The head keeps its own level; the Look turns it sideways.
    let neck_base = at(Vec3::new(0.0, 0.7, -1.2));
    let reach = if kind == Some(Action::Peck) { 0.9 * p.strike * p.hold } else { 0.0 };
    let head = neck_base + body_q * Vec3::new(0.0, 1.2 - reach * 1.1, -0.6 - reach);
    m.beam(neck_base, head, 0.32, plume, 0.0);
    // Hackles rise one by one through the Look's preparation.
    let rise = if kind == Some(Action::Look) { p.wind } else { 0.0 };
    for i in 0..5 {
        let lift = ((rise * 6.0 - i as f32).clamp(0.0, 1.0)) * 0.35;
        let a = i as f32 * 1.2 - 2.4;
        let base = neck_base.lerp(head, 0.25 + i as f32 * 0.12);
        let tip = base + body_q * (Vec3::new(a.sin() * 0.45, 0.2 + lift, a.cos() * 0.45 * 0.6));
        m.beam(base, tip, 0.07, rust, lift * 2.0);
    }
    let stare = if kind == Some(Action::Look) { 1.1 * p.wind } else { 0.0 };
    let head_q = Quat::from_rotation_y(e.yaw + stare);
    m.cuboid(head, Vec3::new(0.7, 0.62, 0.8), head_q, plume, 0.0);
    m.cuboid(head + head_q * Vec3::new(0.0, -0.08, -0.62), Vec3::new(0.22, 0.2, 0.5), head_q, [0.9, 0.72, 0.3], 0.0);
    m.cuboid(head + head_q * Vec3::new(0.0, -0.42, -0.25), Vec3::new(0.18, 0.4, 0.22), head_q, tint(e, look.comb), 0.1);
    for x in [-0.38, 0.38] {
        m.cuboid(head + head_q * Vec3::new(x, 0.08, -0.2), Vec3::splat(0.12), head_q, [0.08, 0.06, 0.04], 0.0);
    }
    // Comb glow reads the stored charge from half a kilometre away.
    let comb_glow = 0.2 + charge * 2.2 + rise * 1.5;
    for i in 0..3 {
        let c = head + head_q * Vec3::new(0.0, 0.5 - 0.12 * (i as f32 - 1.0).abs(), 0.25 - i as f32 * 0.25);
        m.cuboid(c, Vec3::new(0.16, 0.42, 0.2), head_q, tint(e, look.comb), comb_glow);
    }
    // Storm crystals caught in the plumage.
    for (x, y, z) in [(-0.8, 0.9, 0.6), (0.9, 0.6, -0.2), (0.3, 1.0, 1.1)] {
        m.cuboid(at(Vec3::new(x, y, z)), Vec3::new(0.14, 0.3, 0.14), body_q * Quat::from_rotation_z(0.4), [0.7, 0.85, 1.0], 0.6 + charge * 1.6);
    }
    crate::world::adventure::enemy_presentation::motion::pose_body(e, m, mark);
}
