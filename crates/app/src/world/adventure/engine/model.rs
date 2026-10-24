//! Presentation reads the simulation's arm position and action clock; it never decides contact.
use super::*;
use boss_look::{pose, scaled, tint};

const PAINT: [[f32; 3]; 3] = [[0.34, 0.3, 0.28], [0.26, 0.32, 0.4], [0.38, 0.3, 0.2]];
const TRIM: [[f32; 3]; 3] = [[0.72, 0.52, 0.24], [0.62, 0.66, 0.7], [0.66, 0.24, 0.14]];

pub(in crate::world::adventure) fn model(e: &Enemy, m: &mut Model, time: f32) {
    let s = &e.engine;
    let v = boss_look::index(e);
    let run = s.run.as_ref();
    let p = pose(run);
    let kind = run.map(|r| r.kind);
    let iron = tint(e, PAINT[v]);
    let dark = tint(e, scaled(PAINT[v], 0.55));
    let brass = tint(e, TRIM[v]);
    let mark = m.mark();
    // Whole-body action offsets: the swing coils away then whips through,
    // the ram sits back on its haunches then lurches, the volley rears.
    let (twist, pitch, shove) = match kind {
        Some(Action::Swing) => (0.45 * p.wind - 0.55 * p.strike * p.hold, 0.0, 0.0),
        Some(Action::Shove) => (0.0, 0.12 * p.wind - 0.1 * p.hold * p.strike, 0.3 * p.wind - 0.45 * p.strike * p.hold),
        Some(Action::Volley) => (0.0, -0.22 * p.weight, 0.0),
        None => (0.0, 0.0, 0.0),
    };
    let idle = (time * 2.2).sin() * 0.025;
    let q = Quat::from_rotation_y(e.yaw);
    let body_q = q * Quat::from_rotation_y(twist) * Quat::from_rotation_x(pitch);
    let hull = |o: Vec3| e.pos + q * Vec3::new(0.0, 0.0, shove) + body_q * (o + Vec3::Y * idle);
    // Piston legs: hip under the hull, a rear-bent knee and a broad foot.
    for (i, (x, z)) in [(-0.62, -0.5), (0.62, -0.5), (-0.62, 0.55), (0.62, 0.55)].into_iter().enumerate() {
        let gait = (e.phase * 4.0 + i as f32 * 1.6).sin();
        let hip = hull(Vec3::new(x, 0.62, z));
        let foot = local(e, Vec3::new(x * 1.25, 0.06, z + gait * 0.16));
        let knee = (hip + foot) * 0.5 + q * Vec3::new(x.signum() * 0.18, 0.12 + gait.max(0.0) * 0.12, 0.22);
        m.beam(hip, knee, 0.15, dark, 0.0);
        m.beam(knee, foot, 0.11, iron, 0.0);
        m.beam(knee, knee + (foot - knee) * 0.5, 0.05, brass, 0.0);
        m.cuboid(foot, Vec3::new(0.36, 0.12, 0.44), q, dark, 0.0);
    }
    // Hull: base, a stepped deck, side armour and a front grille.
    m.cuboid(hull(Vec3::new(0.0, 0.95, 0.0)), Vec3::new(1.6, 0.9, 1.5), body_q, iron, 0.0);
    m.cuboid(hull(Vec3::new(0.0, 1.5, 0.15)), Vec3::new(1.25, 0.35, 1.1), body_q, dark, 0.0);
    for side in [-1.0f32, 1.0] {
        m.cuboid(hull(Vec3::new(side * 0.84, 1.0, 0.0)), Vec3::new(0.1, 0.7, 1.3), body_q * Quat::from_rotation_z(side * 0.1), dark, 0.0);
        for z in [-0.5, 0.0, 0.5] {
            m.cuboid(hull(Vec3::new(side * 0.9, 1.3, z)), Vec3::splat(0.07), body_q, brass, 0.0);
        }
    }
    let heat = if s.building.is_some() { 0.8 + (time * 6.0).sin() * 0.3 } else { 0.25 };
    for i in 0..4 {
        m.cuboid(hull(Vec3::new(-0.45 + i as f32 * 0.3, 0.85, -0.77)), Vec3::new(0.12, 0.45, 0.04), body_q, [1.0, 0.45, 0.12], heat + p.wind);
    }
    // Smokestack with a breathing ember mouth.
    let stack = hull(Vec3::new(-0.45, 1.95, 0.45));
    m.cuboid(stack, Vec3::new(0.28, 0.6, 0.28), body_q, dark, 0.0);
    m.cuboid(stack + Vec3::Y * 0.34, Vec3::new(0.34, 0.08, 0.34), body_q, brass, 0.0);
    m.orb(stack + Vec3::Y * (0.45 + (time * 1.7).fract() * 0.5), 0.09 + (time * 1.7).fract() * 0.1, [0.32, 0.3, 0.3], 0.0);
    m.orb(stack + Vec3::Y * 0.34, 0.08, [1.0, 0.5, 0.15], heat * 1.5);
    // Stock bins along the flanks: one ingot per unit still owned.
    for i in 0..s.stock.min(8) {
        let side = if i % 2 == 0 { -1.0 } else { 1.0 };
        let row = (i / 2) as f32;
        m.cuboid(hull(Vec3::new(side * 0.98, 0.62 + row * 0.17, 0.35)), Vec3::new(0.18, 0.13, 0.4), body_q, brass, 0.0);
    }
    // Cradle at the back glows while it works.
    m.cuboid(hull(CRADLE - Vec3::Y * 0.25), Vec3::new(0.9, 0.2, 0.7), body_q, [0.5, 0.26, 0.12], heat);
    for x in [-0.42, 0.42] {
        m.beam(hull(CRADLE + Vec3::new(x, -0.2, 0.3)), hull(CRADLE + Vec3::new(x, 0.35, 0.3)), 0.04, brass, 0.0);
    }
    if let Some((module, left)) = s.building {
        let grown = (1.0 - left / 5.0).clamp(0.15, 1.0);
        m.cuboid(hull(CRADLE), Vec3::splat(0.5 * grown), body_q * Quat::from_rotation_y(time), module.color(), heat * 0.5);
        m.beam(hull(CRADLE + Vec3::new(0.0, 0.5, 0.3)), hull(CRADLE), 0.02, [1.0, 0.7, 0.3], 2.0 * (time * 20.0).sin().max(0.0));
    }
    // Service arm: shoulder, upper arm, forearm and a three-finger clamp.
    let shoulder = hull(Vec3::new(0.5, 1.75, 0.4));
    let hand = s.transfer.map_or(hull(CRADLE + Vec3::Y * 0.4), |(_, t)| arm(e, t));
    let elbow = (shoulder + hand) * 0.5 + Vec3::Y * 0.55;
    m.orb(shoulder, 0.16, brass, 0.0);
    m.beam(shoulder, elbow, 0.12, brass, 0.0);
    m.orb(elbow, 0.11, dark, 0.0);
    m.beam(elbow, hand, 0.1, brass, 0.0);
    let grip = if s.transfer.is_some() { 0.18 } else { 0.3 };
    for i in 0..3 {
        let a = i as f32 * std::f32::consts::TAU / 3.0;
        m.beam(hand, hand + q * Vec3::new(a.cos() * grip, -0.25, a.sin() * grip), 0.035, dark, 0.0);
    }
    if let Some((module, _)) = s.transfer {
        m.cuboid(hand - Vec3::Y * 0.2, Vec3::splat(0.5), q, module.color(), 0.6);
    }
    tool(e, m, time, body_q, &hull, brass, dark);
    // Head: a visored sensor block; the eye flares through the wind-up.
    let head = hull(Vec3::new(0.0, 1.55, -0.72));
    m.cuboid(head, Vec3::new(0.62, 0.38, 0.32), body_q, dark, 0.0);
    m.cuboid(head + body_q * Vec3::new(0.0, 0.22, -0.05), Vec3::new(0.7, 0.07, 0.4), body_q, brass, 0.0);
    m.orb(head + body_q * Vec3::new(0.0, 0.0, -0.17), 0.12, [1.0, 0.45, 0.12], 1.0 + p.wind * 2.5);
    let k = boss_look::scale(e);
    m.scale_since(mark, e.pos, Vec3::splat(k));
    if let Some((module, at)) = s.loose {
        m.cuboid(at + Vec3::Y * 0.25, Vec3::splat(0.5), Quat::from_rotation_y(time), module.color(), 0.8);
        tactics::ring(m, at, PICKUP_RANGE, PALETTE_LEAF, 0.0);
    }
    if let Some((_, _)) = s.transfer {
        tactics::ring(m, Vec3::new(hand.x, e.pos.y + 0.02, hand.z), 0.5, PALETTE_CORAL, 0.1);
    }
    if let Some(r) = run
        && r.phase() == Phase::Prepare
        && r.kind == Action::Swing
    {
        tactics::ring(m, e.pos, 3.6, PALETTE_CORAL, 0.2);
    }
}

fn tool(e: &Enemy, m: &mut Model, time: f32, body_q: Quat, hull: &dyn Fn(Vec3) -> Vec3, brass: [f32; 3], dark: [f32; 3]) {
    let s = &e.engine;
    let run = s.run.as_ref();
    let p = pose(run);
    let mount = hull(MOUNT);
    match s.installed {
        Some(Module::Brace) => {
            let c = hull(Vec3::new(0.0, 1.0, -0.88));
            m.cuboid(c, Vec3::new(1.7, 1.4, 0.15), body_q, tint(e, Module::Brace.color()), 0.15);
            for x in [-0.55, 0.0, 0.55] {
                m.cuboid(c + body_q * Vec3::new(x, 0.0, -0.1), Vec3::new(0.12, 1.3, 0.08), body_q, dark, 0.0);
            }
            m.cuboid(c + body_q * Vec3::new(0.0, 0.0, -0.12), Vec3::new(1.5, 0.12, 0.06), body_q, brass, 0.0);
        }
        Some(Module::Cutter) => {
            let sweep = run.map_or(-0.6, |r| {
                if r.kind == Action::Swing && r.phase() == Phase::Active {
                    ((r.elapsed - r.prepare) / r.active).clamp(0.0, 1.0) * 3.2 - 1.6
                } else if r.kind == Action::Swing && r.phase() == Phase::Recover {
                    1.6 - 0.8 * (1.0 - p.hold)
                } else {
                    -0.6 - p.wind * 1.0
                }
            });
            let dir = Quat::from_rotation_y(e.yaw + sweep) * Vec3::NEG_Z;
            let tip = mount + dir * 1.6;
            m.beam(mount, tip, 0.12, dark, 0.0);
            // A spinning saw disk: eight teeth, faster as it winds up.
            let spin = time * (6.0 + p.wind * 18.0 + p.hold * 10.0);
            for i in 0..8 {
                let a = spin + i as f32 * std::f32::consts::TAU / 8.0;
                let r = Quat::from_rotation_y(a);
                m.cuboid(tip + r * Vec3::new(0.0, 0.0, 0.42), Vec3::new(0.14, 0.05, 0.22), r, tint(e, Module::Cutter.color()), 0.2 + p.wind);
            }
            m.cuboid(tip, Vec3::new(0.5, 0.06, 0.5), Quat::from_rotation_y(spin), brass, 0.0);
        }
        Some(Module::Launcher) => {
            let lift = Quat::from_rotation_x(0.35 * p.weight);
            for x in [-0.15, 0.15] {
                let base = mount + body_q * Vec3::new(x, 0.0, 0.0);
                m.beam(base, base + body_q * lift * Vec3::new(0.0, 0.0, -0.9), 0.1, dark, 0.0);
                m.orb(base + body_q * lift * Vec3::new(0.0, 0.0, -0.92), 0.08, tint(e, Module::Launcher.color()), p.wind * 3.0);
            }
            m.cuboid(mount + body_q * Vec3::new(0.0, 0.0, 0.2), Vec3::new(0.55, 0.4, 0.5), body_q, tint(e, Module::Launcher.color()), 0.2);
        }
        None => {
            m.cuboid(mount, Vec3::new(0.3, 0.3, 0.3), body_q, dark, 0.0);
        }
    }
}
