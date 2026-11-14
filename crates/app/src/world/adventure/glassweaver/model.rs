//! Presentation only: reads the lattice, bolt and action clock; never decides contact.
use super::*;
use boss_look::{ease, pose, scaled, tint};

struct Palette {
    glass: [f32; 3],
    shell: [f32; 3],
    core: [f32; 3],
}
const PALETTES: [Palette; 3] = [
    Palette { glass: [0.72, 0.52, 1.0], shell: [0.2, 0.15, 0.3], core: [0.95, 0.8, 1.0] },
    Palette { glass: [0.46, 0.44, 0.5], shell: [0.12, 0.11, 0.13], core: [1.0, 0.62, 0.3] },
    Palette { glass: [0.4, 0.95, 0.78], shell: [0.1, 0.22, 0.26], core: [0.75, 1.0, 0.95] },
];

pub(in crate::world::adventure) fn model(e: &Enemy, m: &mut Model, time: f32) {
    let s = &e.glass;
    let pal = &PALETTES[boss_look::index(e)];
    let run = s.run.as_ref();
    let p = pose(run);
    let kind = run.map(|r| r.kind);
    let glass = tint(e, pal.glass);
    let shell = tint(e, pal.shell);
    let breath = (time * 1.6).sin();
    let mark = m.mark();
    m.set_surface_kind(58.0);
    let o = e.pos;
    let at = |x: f32, y: f32, z: f32| o + Vec3::new(x, y, z);
    // Weave coils the whole body back then lunges the needles forward; mend kneels.
    let (lean, crouch) = match kind {
        Some(Action::Weave) => (-0.25 * p.wind + 0.35 * p.strike * p.hold, 0.12 * p.wind),
        Some(Action::Lash) => (-0.2 * p.wind + 0.5 * p.strike * p.hold, 0.2 * p.wind),
        Some(Action::Mend) => (0.25 * p.weight, 0.45 * p.weight),
        _ => (0.0, 0.0),
    };
    let hover = 0.05 * breath - crouch;
    // Four jointed legs: hip on the pelvis ring, a high knee, a needle foot.
    for (i, (x, z)) in [(-0.55, -0.45), (0.55, -0.45), (-0.6, 0.5), (0.6, 0.5)].into_iter().enumerate() {
        let hip = at(x * 0.45, 1.05 + hover, z * 0.45);
        // Convert persistent world contacts into the unscaled body frame. The
        // final body transform below restores the exact planted position.
        let local = if s.motion.ready {
            Quat::from_rotation_y(-e.yaw) * (s.motion.feet[i].at - e.pos) / boss_look::scale(e)
        } else { motion::REST[i] };
        let foot = e.pos + local;
        let delta = foot - hip;
        let length = delta.length().max(0.001);
        let axis = delta / length;
        let outward = Vec3::new(x, 0.65, z).normalize();
        let bend = (outward - axis * outward.dot(axis)).normalize_or_zero();
        let upper = 0.9_f32;
        let lower = 1.15_f32;
        let reach = length.clamp((lower-upper).abs()+0.001, upper+lower-0.001);
        let along = (upper*upper - lower*lower + reach*reach) / (2.0*reach);
        let knee = hip + axis * along + bend * (upper*upper-along*along).max(0.0).sqrt();
        m.beam(hip, knee, 0.075, shell, 0.0);
        m.beam(knee, foot, 0.05, shell, 0.0);
        m.cuboid(foot + Vec3::Y * 0.05, Vec3::new(0.09, 0.1, 0.09), Quat::from_rotation_y(0.78), glass, 0.4);
    }
    // Pelvis ring and the loom frame behind it.
    m.cuboid(at(0.0, 1.05 + hover, 0.0), Vec3::new(0.55, 0.22, 0.5), Quat::from_rotation_y(0.78), shell, 0.0);
    for x in [-0.32, 0.32] {
        m.beam(at(x, 1.1 + hover, 0.25), at(x * 1.4, 2.3 + hover, 0.75), 0.04, shell, 0.0);
    }
    m.beam(at(-0.45, 2.3 + hover, 0.75), at(0.45, 2.3 + hover, 0.75), 0.04, shell, 0.0);
    for i in 0..5 {
        let x = -0.36 + i as f32 * 0.18;
        let shimmer = 0.4 + 0.4 * (time * 3.0 + i as f32).sin().max(0.0);
        m.beam(at(x, 2.25 + hover, 0.74), at(x * 0.7, 1.2 + hover, 0.3), 0.012, glass, shimmer + p.wind);
    }
    // Torso: a tapering faceted column that leans with the action.
    let tilt = Quat::from_rotation_x(-lean);
    let chest = at(0.0, 1.05 + hover, 0.0);
    let up = |y: f32, z: f32| chest + tilt * Vec3::new(0.0, y, z);
    m.cuboid(up(0.35, 0.0), Vec3::new(0.38, 0.5, 0.3), tilt, shell, 0.0);
    m.material_box(up(0.8, -0.02), Vec3::new(0.5, 0.42, 0.34), tilt * Quat::from_rotation_y(0.78), glass, 56.0, 0.25 + p.wind * 0.6);
    m.orb(up(0.8, -0.2), 0.1, pal.core, 1.2 + p.wind * 2.0 + breath * 0.2);
    // Collar spines and the head: a tilted prism with a slit eye.
    for i in 0..5 {
        let a = -1.0 + i as f32 * 0.5;
        m.beam(up(1.05, 0.05), up(1.35 + a.cos() * 0.2, 0.25 + a.sin().abs() * 0.1) + tilt * Vec3::X * a * 0.35, 0.025, glass, 0.3);
    }
    let head = up(1.35 + 0.03 * breath, -0.12);
    let nod = tilt * Quat::from_rotation_x(0.2 * p.wind) * Quat::from_rotation_z(0.78);
    m.material_box(head, Vec3::new(0.26, 0.26, 0.34), nod, glass, 56.0, 0.35);
    m.cuboid(head + tilt * Vec3::new(0.0, 0.0, -0.17), Vec3::new(0.2, 0.035, 0.03), tilt, pal.core, 1.8 + p.wind * 2.0);
    // Arms: shoulder, elbow, a long glass needle per hand.
    for side in [-1.0f32, 1.0] {
        let shoulder = up(1.0, 0.0) + tilt * Vec3::X * side * 0.32;
        let (elbow, hand) = match kind {
            Some(Action::Weave) => {
                // Draw the thread back to the ear, then thrust both needles together.
                let back = Vec3::new(side * 0.6, 1.25 + 0.2 * p.wind, 0.35);
                let front = Vec3::new(side * 0.12, 0.9, -1.0);
                let h = back.lerp(front, p.strike * p.hold);
                (Vec3::new(side * 0.55, 0.85, 0.1).lerp(Vec3::new(side * 0.35, 0.85, -0.5), p.strike * p.hold), h)
            }
            Some(Action::Lash) => {
                let sweep = -1.2 + 2.4 * p.strike;
                let r = Quat::from_rotation_y(side * (0.8 * p.wind) + sweep * p.hold * side);
                (Vec3::new(side * 0.55, 0.85, -0.1), r * Vec3::new(side * 0.5, 0.75, -1.1))
            }
            Some(Action::Mend) => (Vec3::new(side * 0.45, 0.7, -0.45), Vec3::new(side * 0.15, 0.55 + 0.05 * (time * 9.0).sin(), -1.15)),
            _ => {
                let sway = (time * 1.3 + side).sin() * 0.08;
                (Vec3::new(side * 0.55, 0.65, 0.0), Vec3::new(side * 0.4, 0.45 + sway, -0.45))
            }
        };
        let elbow = chest + tilt * elbow;
        let hand = chest + tilt * hand;
        m.beam(shoulder, elbow, 0.055, shell, 0.0);
        m.beam(elbow, hand, 0.045, shell, 0.0);
        let needle = (hand - elbow).normalize_or_zero();
        m.beam(hand, hand + needle * 0.7, 0.02, glass, 0.8 + p.wind * 1.5);
    }
    // Three orbiting shards spin faster while it threads a bolt.
    for i in 0..3 {
        let a = time * (1.1 + p.wind * 4.0) + i as f32 * std::f32::consts::TAU / 3.0;
        let c = up(1.45, 0.0) + Vec3::new(a.cos() * 0.6, 0.15 * (a * 2.0).sin(), a.sin() * 0.6);
        m.cuboid(c, Vec3::new(0.07, 0.18, 0.07), Quat::from_rotation_y(a) * Quat::from_rotation_z(0.6), glass, 1.0);
    }
    m.bake_model_uv_since(mark, e.pos, 58.0);
    m.rotate_since(mark, e.pos, Quat::from_rotation_y(e.yaw));
    let k = boss_look::scale(e);
    m.scale_since(mark, e.pos, Vec3::splat(k));
    m.set_surface_kind(41.0);
    lattice(e, m, time, pal);
    for shard in &s.motion.shards {
        let fade = (shard.life / 0.6).clamp(0.0, 1.0);
        m.material_box(shard.at, Vec3::new(0.10, 0.16, 0.07) * fade,
            shard.rotation, pal.glass, 56.0, 0.0);
    }
}

/// The lattice lives in the arena, not on the body: nodes, strands, the bolt
/// in flight and, while it winds up, the strand route its bolt will take.
fn lattice(e: &Enemy, m: &mut Model, time: f32, pal: &Palette) {
    let s = &e.glass;
    let n = s.count as usize;
    let preview = s.run.as_ref().filter(|r| r.kind == Action::Weave && r.phase() == Phase::Prepare).and_then(|_| route(s, hands(e), e.aim));
    let live = s.bolt.map(|b| (b.path, b.len)).or(preview);
    let on_route = |a: usize, b: usize| {
        live.is_some_and(|(path, len)| (1..len as usize).any(|i| {
            let (x, y) = (path[i - 1] as usize, path[i] as usize);
            (x, y) == (a, b) || (x, y) == (b, a)
        }))
    };
    for a in 0..n {
        for b in a + 1..n {
            if s.strand(a, b) {
                let hot = on_route(a, b);
                let pulse = 0.5 + 0.3 * (time * 4.0 + a as f32).sin();
                m.beam(s.nodes[a].pos, s.nodes[b].pos, if hot { 0.04 } else { 0.018 }, if hot { pal.core } else { pal.glass }, if hot { 2.2 } else { pulse });
            }
        }
    }
    let full = NODE_HEALTH * 1.4;
    for (i, node) in s.nodes[..n].iter().enumerate() {
        let ground = Vec3::new(node.pos.x, e.home.y, node.pos.z);
        m.beam(ground, node.pos - Vec3::Y * 0.4, 0.035, pal.shell, 0.0);
        m.cuboid(ground + Vec3::Y * 0.08, Vec3::new(0.5, 0.16, 0.5), Quat::from_rotation_y(0.78), pal.shell, 0.0);
        if s.intact(i) {
            let spin = Quat::from_rotation_y(time * 0.7 + i as f32);
            let life = (node.health / full).clamp(0.25, 1.0);
            let mending = s.mend == Some(i as u8);
            let target = live.is_some_and(|(path, len)| path[len as usize - 1] as usize == i);
            let glow = 0.6 + life + if target { 1.5 + (time * 10.0).sin() } else { 0.0 };
            m.material_box(node.pos, Vec3::new(0.32, 0.62, 0.32) * (0.75 + 0.25 * life), spin * Quat::from_rotation_x(0.3), pal.glass, 56.0, glow * 0.18);
            m.cuboid(node.pos, Vec3::new(0.22, 0.22, 0.22), spin * Quat::from_rotation_z(0.78), pal.core, glow + 0.5);
            if mending {
                m.beam(hands(e), node.pos, 0.02, pal.core, 2.0);
            }
        } else {
            // Shattered: shards lie on the plinth; a mend shows the prism regrowing.
            for j in 0..3 {
                let a = j as f32 * 2.1 + i as f32;
                m.cuboid(ground + Vec3::new(a.cos() * 0.3, 0.2, a.sin() * 0.3), Vec3::new(0.12, 0.2, 0.08), Quat::from_rotation_y(a) * Quat::from_rotation_z(1.1), scaled(pal.glass, 0.6), 0.1);
            }
            if s.mend == Some(i as u8)
                && let Some(r) = &s.run
            {
                let g = ease(r.wind());
                m.cuboid(node.pos, Vec3::new(0.32, 0.62, 0.32) * g, Quat::from_rotation_y(time * 3.0), pal.glass, 1.5 * g);
                m.beam(hands(e), node.pos, 0.02, pal.core, 2.0);
            }
        }
    }
    if let Some(b) = s.bolt {
        let a = s.nodes[b.path[b.at as usize] as usize].pos;
        let c = if b.at + 1 < b.len { s.nodes[b.path[b.at as usize + 1] as usize].pos } else { a };
        let at = a.lerp(c, b.t);
        let r = if b.payload == Payload::Fan { 0.24 } else { 0.17 };
        m.orb(at, r, pal.core, 3.0);
        m.orb(a.lerp(c, (b.t - 0.12).max(0.0)), r * 0.6, pal.glass, 1.6);
    }
}
