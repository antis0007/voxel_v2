//! Signature guardians use the same collision, damage and bounded effect budgets.
use super::*;
pub(super) fn kind(site: u32) -> u32 {
    match site {
        3 => 6,
        5 => 7,
        8 => 8,
        _ => 0,
    }
}
pub fn name(kind: u32) -> &'static str {
    match kind {
        6 => "The Cinder Castellan",
        7 => "The Unwritten Keeper",
        _ => "The Prism Hart",
    }
}
pub(super) fn health(kind: u32) -> f32 {
    match kind {
        6 => 360.0,
        7 => 280.0,
        _ => 320.0,
    }
}
pub(super) fn color(kind: u32) -> [f32; 3] {
    match kind {
        6 => [1.0, 0.36, 0.08],
        7 => [0.72, 0.42, 1.0],
        _ => [0.25, 0.85, 1.0],
    }
}
fn cue(kind: u32, release: bool) -> Cue {
    match (kind, release) {
        (6, false) => Cue::ForgeWindup,
        (6, true) => Cue::ForgeSlam,
        (7, false) => Cue::ArchiveWindup,
        (7, true) => Cue::ArchiveRelease,
        (_, false) => Cue::PrismWindup,
        (_, true) => Cue::PrismRelease,
    }
}
pub(super) fn model(e: &Enemy, m: &mut Model, time: f32) {
    let c = if e.flash > 0.0 {
        [1.0, 0.96, 0.83]
    } else {
        color(e.kind)
    };
    let enraged = e.health < health(e.kind) * 0.5;
    let mark = m.mark();
    let wind = if e.tell > 0.0 {
        (1.0 - e.tell / if enraged { 0.8 } else { 1.1 }).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let wind = wind * wind * (3.0 - 2.0 * wind);
    let recovery = if e.tell <= 0.0 {
        ((e.cooldown - if enraged { 1.3 } else { 2.1 } + 0.38) / 0.38).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let stride = (e.phase * 5.0).sin();
    let p = e.pos
        + Vec3::Y
            * if e.kind == 7 {
                0.12 * (time * 1.8).sin() + wind * 0.2
            } else {
                -wind * 0.12 - recovery * 0.08
            };
    if e.kind == 6 {
        m.cuboid(
            p + Vec3::Y * 1.2,
            Vec3::new(0.88, 1.25, 0.62),
            Quat::IDENTITY,
            [0.20, 0.22, 0.23],
            0.0,
        );
        for x in [-0.6, 0.6] {
            m.cuboid(
                p + Vec3::new(x, 1.3 + wind * 0.35, -wind * 0.18),
                Vec3::new(0.30, 0.95, 0.42),
                Quat::from_rotation_x(-wind * 2.2 + recovery * 0.9),
                [0.38, 0.24, 0.12],
                0.0,
            );
            m.cuboid(
                p + Vec3::new(x * 0.48, 0.34, stride * x * 0.15),
                Vec3::new(0.28, 0.68, 0.35),
                Quat::IDENTITY,
                [0.18, 0.20, 0.22],
                0.0,
            );
        }
        for y in [0.9, 1.1, 1.3] {
            m.cuboid(
                p + Vec3::new(0.0, y, -0.33),
                Vec3::new(0.5, 0.075, 0.035),
                Quat::IDENTITY,
                c,
                2.0,
            );
        }
        m.cuboid(
            p + Vec3::Y * 2.02,
            Vec3::new(0.55, 0.40, 0.48),
            Quat::IDENTITY,
            [0.3, 0.28, 0.23],
            0.0,
        );
    } else if e.kind == 7 {
        m.cuboid(
            p + Vec3::Y * 1.1,
            Vec3::new(0.5, 1.35, 0.45),
            Quat::from_rotation_y(time * 0.3),
            [0.16, 0.11, 0.24],
            0.0,
        );
        for i in 0..5 {
            let a = i as f32 * 1.256 + time * 0.8 + wind * 2.5 - recovery;
            let radius = 0.62 + wind * 0.55 + recovery * 0.35;
            let q = p + Vec3::new(
                a.cos() * radius,
                1.4 + a.sin() * 0.22 + wind * 0.35,
                a.sin() * radius,
            );
            m.cuboid(
                q,
                Vec3::new(0.22, 0.31, 0.08),
                Quat::from_rotation_y(-a),
                [0.55, 0.42, 0.23],
                0.2,
            );
            m.cuboid(
                q + Vec3::Y * 0.02,
                Vec3::new(0.17, 0.22, 0.085),
                Quat::from_rotation_y(-a),
                [0.78, 0.75, 0.63],
                0.0,
            );
        }
        m.orb(p + Vec3::Y * 2.1, 0.20, c, 1.8);
    } else {
        m.cuboid(
            p + Vec3::Y * 1.1,
            Vec3::new(0.75, 0.6, 0.85),
            Quat::IDENTITY,
            [0.15, 0.32, 0.39],
            0.2,
        );
        for x in [-0.3, 0.3] {
            for z in [-0.3, 0.3] {
                m.beam(
                    p + Vec3::new(x, 0.1, z + stride * (x * z).signum() * 0.12),
                    p + Vec3::new(x, 0.95, z),
                    0.09,
                    [0.22, 0.46, 0.55],
                    0.0,
                );
            }
        }
        let antler_mark = m.mark();
        m.orb(p + Vec3::new(0.0, 1.75, -0.28), 0.25, c, 1.1);
        for x in [-1.0, 1.0] {
            let base = p + Vec3::new(x * 0.15, 1.9, -0.25);
            let top = p + Vec3::new(x * 0.5, 2.5, -0.12);
            m.beam(base, top, 0.055, c, 1.0);
            m.beam(
                top - Vec3::Y * 0.18,
                top + Vec3::new(x * 0.22, -0.05, 0.0),
                0.035,
                c,
                1.0,
            );
        }
        m.rotate_since(
            antler_mark,
            p + Vec3::new(0.0, 1.55, -0.28),
            Quat::from_rotation_x(wind * 0.65 - recovery * 0.4),
        );
    }
    m.orb(
        p + Vec3::new(0.0, 1.5, -0.38),
        0.11,
        c,
        if enraged { 3.0 } else { 1.5 },
    );
    m.rotate_since(mark, e.pos, Quat::from_rotation_y(e.yaw));
}
pub(super) fn tick(
    e: &mut Enemy,
    world: &World,
    feet: Vec3,
    eye: Vec3,
    dt: f32,
    shots: &mut Vec<Projectile>,
    hazards: &mut Vec<tactics::Hazard>,
    sounds: &mut Vec<Sound>,
) {
    let enraged = e.health < health(e.kind) * 0.5;
    let d = eye - e.center();
    if (feet - e.home).length() > 48.0 {
        e.pos = e.home;
        e.health = health(e.kind);
        e.tell = 0.0;
        e.cooldown = 2.0;
        return;
    }
    // Commit the facing with the attack target, leaving a readable dodge window.
    if e.tell <= 0.0 {
        e.yaw = (-d.x).atan2(-d.z);
    }
    if e.stagger > 0.0 {
        e.stagger = e.stagger.min(0.07);
    }
    // Interruptible, but not permanently stun-locked.
    else if e.tell > 0.0 {
        e.tell -= dt;
        if e.tell <= 0.0 {
            let area = e.attacks % 2 == 0;
            if area {
                let count = if enraged { 5 } else { 3 };
                for i in 0..count {
                    if hazards.len() >= 16 {
                        break;
                    }
                    let offset = if e.kind == 6 {
                        (e.aim - e.pos).normalize_or_zero() * ((i as f32 - 1.0) * 2.2)
                    } else {
                        let a = i as f32 * std::f32::consts::TAU / count as f32;
                        Vec3::new(a.cos(), 0.0, a.sin()) * 2.5
                    };
                    hazards.push(tactics::Hazard {
                        pos: e.aim + offset,
                        radius: if e.kind == 6 { 1.8 } else { 1.5 },
                        delay: 0.55 + i as f32 * 0.16,
                        life: 0.22,
                        pulse: 0.0,
                        damage: if enraged { 24.0 } else { 18.0 },
                        spores: false,
                    });
                }
            } else {
                let count = if e.kind == 7 {
                    if enraged { 16 } else { 10 }
                } else if enraged {
                    9
                } else {
                    5
                };
                for i in 0..count {
                    if shots.len() >= 96 {
                        break;
                    }
                    let direction = if e.kind == 7 {
                        let a = i as f32 * std::f32::consts::TAU / count as f32
                            + e.attacks as f32 * 0.4;
                        Vec3::new(a.cos(), 0.0, a.sin())
                    } else {
                        Quat::from_rotation_y((i as f32 - (count - 1) as f32 * 0.5) * 0.16)
                            * (e.aim - e.center()).normalize_or_zero()
                    };
                    shots.push(Projectile {
                        pos: e.center(),
                        velocity: direction * if enraged { 12.0 } else { 9.0 },
                        hostile: true,
                        payload: None,
                        spell: 0,
                        life: 4.0,
                        mods: Default::default(),
                        bore_budget: 0,
                        trail_timer: 0.0,
                    });
                }
            }
            sound(sounds, cue(e.kind, true), Some(e.center()), 0.85);
            e.cooldown = if enraged { 1.3 } else { 2.1 };
        }
    } else if e.visible && e.cooldown <= 0.0 {
        e.attacks = e.attacks.wrapping_add(1);
        e.attack = if e.attacks % 2 == 0 { 2 } else { 0 };
        e.aim = if e.attack == 2 { feet } else { eye };
        e.tell = if enraged { 0.8 } else { 1.1 };
        sound(sounds, cue(e.kind, false), Some(e.center()), 0.8);
    } else if e.visible && d.length() > 7.0 {
        let movement = (d * Vec3::new(1.0, 0.0, 1.0)).normalize_or_zero() * dt * 1.2;
        let q = e.pos + movement;
        if free(world, q, e.kind) && factory::solid_point(world, q - Vec3::Y * 0.6) {
            e.phase += movement.length();
            e.pos = q;
        }
    }
    // Ground following is swept just like ordinary guardians.
    e.velocity.y = (e.velocity.y - dt * 14.0).max(-12.0);
    let dy = e.velocity.y * dt;
    let steps = (dy.abs() / 0.16).ceil().max(1.0) as usize;
    for _ in 0..steps {
        let q = e.pos + Vec3::Y * dy / steps as f32;
        if free(world, q, e.kind) {
            e.pos = q;
        } else {
            e.velocity.y = 0.0;
            break;
        }
    }
}
