//! Broad ceramic masses over an iron carriage. Every active pose reads the
//! authoritative action clock; drives stay visibly broken after failure.
use super::mechanisms::{CONTROL, LOADER, TRACTION};
use super::*;
use boss_look::{pose, tint};
pub(in crate::world::adventure) fn model(e: &Enemy, m: &mut Model, time: f32) {
    let mark = m.mark();
    let s = &e.engine;
    let p = pose(s.run.as_ref());
    let kind = s.run.as_ref().map(|r| r.kind);
    let q = Quat::from_rotation_y(e.yaw);
    let ceramic = tint(e, [0.72, 0.69, 0.59]);
    let iron = tint(e, [0.12, 0.14, 0.16]);
    let stone = tint(e, [0.36, 0.35, 0.32]);
    let silver = tint(e, [0.54, 0.59, 0.61]);
    let gold = tint(e, [0.65, 0.47, 0.19]);
    let live = s.jam <= 0.0;
    let cut = kind == Some(Action::Shove);
    let sweep = kind == Some(Action::Swing);
    let settle = if cut { p.weight * 0.09 } else { 0.0 };
    let at = |v: Vec3| local(e, v);
    let body = |v: Vec3| at(v - Vec3::Y * settle);
    // Low cutting carriage and two broad tracks.
    for side in [-1.0f32, 1.0] {
        m.cuboid(
            at(Vec3::new(side * 0.72, 0.26, 0.0)),
            Vec3::new(0.42, 0.42, 1.85),
            q,
            iron,
            0.0,
        );
        for z in [-0.65, -0.25, 0.15, 0.55] {
            m.cuboid(
                at(Vec3::new(side * 0.94, 0.26, z)),
                Vec3::new(0.07, 0.25, 0.18),
                q,
                stone,
                0.0,
            );
        }
    }
    m.cuboid(
        body(Vec3::new(0.0, 0.78, 0.05)),
        Vec3::new(1.65, 0.82, 1.85),
        q,
        stone,
        0.0,
    );
    for x in [-0.44, 0.44] {
        m.cuboid(
            body(Vec3::new(x, 1.20, -0.12)),
            Vec3::new(0.80, 0.18, 1.50),
            q,
            ceramic,
            0.0,
        );
        m.cuboid(
            body(Vec3::new(x, 0.86, -0.96)),
            Vec3::new(0.76, 0.61, 0.14),
            q,
            ceramic,
            0.0,
        );
        // One narrow gold inscription and silver fasteners per armor panel.
        m.cuboid(
            body(Vec3::new(x, 1.301, 0.12)),
            Vec3::new(0.035, 0.012, 0.64),
            q,
            gold,
            0.0,
        );
        for z in [-0.65, 0.48] {
            m.cuboid(
                body(Vec3::new(x, 1.304, z)),
                Vec3::new(0.09, 0.018, 0.09),
                q,
                silver,
                0.0,
            );
        }
    }
    // The broad cutter spans the same frontal lane used by cut_advance.
    let cutter = body(Vec3::new(0.0, 0.66, -1.15));
    m.beam(
        cutter + q * Vec3::X * -1.15,
        cutter + q * Vec3::X * 1.15,
        0.30,
        iron,
        0.0,
    );
    for i in 0..9 {
        let x = -1.05 + i as f32 * 0.26;
        let spin = if cut && p.active && live {
            time * 12.0
        } else {
            0.2
        };
        m.cuboid(
            cutter + q * Vec3::X * x,
            Vec3::new(0.11, 0.48, 0.48),
            q * Quat::from_rotation_x(spin),
            silver,
            0.0,
        );
    }
    if cut && p.preparing {
        // Physical brake shoes settle before the cutter moves.
        for x in [-0.84, 0.84] {
            m.beam(
                body(Vec3::new(x, 0.72, 0.45)),
                at(Vec3::new(x * 1.2, 0.05, 0.6)),
                0.12,
                silver,
                0.0,
            );
        }
    }
    // Installed defenses and emitters remain physically readable.
    if s.installed == Some(Module::Brace) {
        let open = if s
            .run
            .as_ref()
            .is_some_and(|r| matches!(r.phase(), Phase::Active | Phase::Recover))
        {
            0.85
        } else {
            0.0
        };
        for side in [-1.0f32, 1.0] {
            m.cuboid(
                body(Vec3::new(side * (0.42 + open * 0.55), 0.95, -1.03)),
                Vec3::new(0.82, 1.05, 0.10),
                q * Quat::from_rotation_y(side * open),
                iron,
                0.0,
            );
        }
    }
    if s.installed == Some(Module::Launcher) {
        let mount = body(MOUNT);
        m.cuboid(mount, Vec3::new(0.45, 0.42, 0.65), q, iron, 0.0);
        m.beam(mount, mount + facing(e) * 0.8, 0.16, silver, 0.0);
    }
    if let Some((module, left)) = s.building {
        let grown = (1.0 - left / 5.0).clamp(0.1, 1.0);
        m.cuboid(
            body(CRADLE),
            Vec3::splat(0.38 * grown),
            q,
            module.color(),
            0.1,
        );
    }
    // Raised rear counterweight takes load before the offset arm accelerates.
    let tension = if sweep {
        p.wind * 0.32 - p.strike * p.hold * 0.18
    } else {
        0.0
    };
    for x in [-0.50, 0.50] {
        m.beam(
            body(Vec3::new(x, 1.1, 0.65)),
            at(Vec3::new(x, 2.15 + tension, 0.8)),
            0.10,
            iron,
            0.0,
        );
    }
    m.cuboid(
        at(Vec3::new(0.0, 2.12 + tension, 0.92)),
        Vec3::new(1.22, 0.54, 0.62),
        q,
        stone,
        0.0,
    );
    m.cuboid(
        at(Vec3::new(0.0, 2.12 + tension, 1.25)),
        Vec3::new(1.05, 0.40, 0.06),
        q,
        ceramic,
        0.0,
    );
    // Offset loading arm, with the scoop in the authoritative annular arc.
    let broken = s.loading_damage >= DRIVE_HEALTH;
    let angle = if let Some(r) = s.run.as_ref().filter(|r| r.kind == Action::Swing) {
        match r.phase() {
            Phase::Prepare => -0.6 - r.wind(),
            Phase::Active => -1.6 + ((r.elapsed - r.prepare) / r.active).clamp(0.0, 1.0) * 3.2,
            _ => 1.6,
        }
    } else {
        -0.6
    };
    let shoulder = at(Vec3::new(0.95, 1.40, 0.25));
    let hand = if broken {
        at(Vec3::new(1.65, 0.20, 0.65))
    } else if let Some((_, t)) = s.transfer {
        arm(e, t)
    } else if sweep {
        at(Vec3::new(-angle.sin() * 2.7, 0.9, -angle.cos() * 2.7))
    } else {
        at(Vec3::new(1.8, 1.7, -0.7))
    };
    let elbow = (shoulder + hand) * 0.5 + Vec3::Y * if broken { 0.0 } else { 0.65 };
    m.beam(shoulder, elbow, 0.19, iron, 0.0);
    m.beam(elbow, hand, 0.14, silver, 0.0);
    m.beam(shoulder + Vec3::Y * 0.15, elbow, 0.06, gold, 0.0);
    m.cuboid(hand, Vec3::new(0.70, 0.35, 0.55), q, ceramic, 0.0);
    // Stationary drive housings match the exact target shapes, without random scaling.
    for (offset, damage) in [(LOADER, s.loading_damage), (TRACTION, s.traction_damage)] {
        let failed = damage >= DRIVE_HEALTH;
        m.cuboid(at(offset), Vec3::splat(0.62), q, iron, 0.0);
        m.cuboid(
            at(offset),
            Vec3::new(0.66, 0.18, 0.42),
            q * Quat::from_rotation_z(if failed { 0.65 } else { 0.0 }),
            if failed { stone } else { gold },
            0.0,
        );
        let color = if s.jam > 0.0 {
            [0.35, 0.65, 0.80]
        } else {
            [0.85, 0.58, 0.22]
        };
        m.orb(
            at(offset) + q * offset.normalize_or_zero() * 0.30,
            0.09,
            color,
            if failed { 0.0 } else { 0.35 },
        );
    }
    m.cuboid(at(CONTROL), Vec3::new(0.55, 0.38, 0.25), q, iron, 0.0);
    m.cuboid(
        at(CONTROL + Vec3::Z * 0.14),
        Vec3::new(0.36, 0.16, 0.02),
        q,
        gold,
        0.25,
    );
    for i in 0..s.stock.min(8) {
        m.cuboid(
            body(Vec3::new(
                -0.55 + (i % 4) as f32 * 0.32,
                1.42,
                0.55 + (i / 4) as f32 * 0.23,
            )),
            Vec3::new(0.23, 0.12, 0.17),
            q,
            silver,
            0.0,
        );
    }
    if let Some((module, _)) = s.transfer {
        m.cuboid(
            hand - Vec3::Y * 0.2,
            Vec3::splat(0.4),
            q,
            module.color(),
            0.15,
        );
    }
    // Body only: a dropped module and the floor marks stay on the ground.
    crate::world::adventure::enemy_presentation::motion::pose_body(e, m, mark);
    if let Some((module, at)) = s.loose {
        m.cuboid(
            at + Vec3::Y * 0.25,
            Vec3::splat(0.5),
            q,
            module.color(),
            0.2,
        );
    }
    // Restrained floor marks retain first-person escape-space visibility.
    if p.preparing && cut {
        let side = Vec3::new(-e.lunge.z, 0.0, e.lunge.x) * 1.15;
        for sign in [-1.0, 1.0] {
            let a = e.pos + side * sign + Vec3::Y * 0.04;
            m.beam(a, a + e.lunge * 6.0, 0.025, PALETTE_CORAL, 0.1);
        }
    }
    if p.preparing && sweep {
        tactics::ring(m, e.pos, 1.5, PALETTE_CORAL, 0.1);
        tactics::ring(m, e.pos, 3.6, PALETTE_CORAL, 0.1);
    }
}
