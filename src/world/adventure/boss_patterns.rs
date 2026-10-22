//! Composable committed attacks: planning, warnings and release share one geometry.
use super::*;

pub(super) struct Pattern {
    pub id: u32,
    pub name: &'static str,
    pub recovery: f32,
}
pub(super) const PATTERNS: &[Pattern] = &[
    Pattern {
        id: 20,
        name: "Fault line",
        recovery: 2.0,
    },
    Pattern {
        id: 21,
        name: "Closing script",
        recovery: 2.4,
    },
    Pattern {
        id: 22,
        name: "Cinder fan",
        recovery: 1.8,
    },
    Pattern {
        id: 23,
        name: "Crossed pages",
        recovery: 2.2,
    },
];

pub(super) fn plan(e: &mut Enemy, feet: Vec3, eye: Vec3, ward_visible: bool) {
    let distance = (eye - e.center()).length();
    let preferred = if ward_visible && e.kind == 7 && distance > 4.5 {
        21
    } else if ward_visible || e.encounter.stationary > 1.2 || distance < 4.5 {
        20
    } else if e.kind == 7 && e.attacks % 3 == 0 {
        21
    } else if e.encounter.motion.length() > 2.0 {
        23
    } else {
        22
    };
    e.attack = if preferred == e.encounter.last_pattern {
        if preferred == 22 { 20 } else if ward_visible { 23 } else { 22 }
    } else {
        preferred
    };
    e.encounter.last_pattern = e.attack;
    e.aim = if matches!(e.attack, 20 | 21) {
        feet
    } else {
        // Account for the committed windup as well as travel. A direction change
        // still defeats the prediction; the target is never updated after planning.
        let windup = if e.encounter.boss_phase > 0 { 0.8 } else { 1.1 };
        let speed = if e.encounter.boss_phase > 0 { 12.0 } else { 9.0 };
        eye + (e.encounter.motion * (windup + distance / speed * 0.6)).clamp_length_max(5.0)
    };
    e.lunge = ((feet - e.pos) * Vec3::new(1.0, 0.0, 1.0)).normalize_or_zero();
    if e.lunge.length_squared() < 0.1 {
        e.lunge = Vec3::Z;
    }
}

fn marks(e: &Enemy) -> Vec<(Vec3, f32, f32)> {
    let enraged = e.encounter.boss_phase > 0;
    match e.attack {
        20 => {
            let count = if enraged { 5 } else { 3 };
            (0..count)
                .map(|i| {
                    (
                        e.aim + e.lunge * (i as f32 - (count - 1) as f32 * 0.5) * 2.2,
                        1.45,
                        0.45 + i as f32 * 0.18,
                    )
                })
                .collect()
        }
        21 => (0..5)
            .map(|i| {
                // Five traps with a deliberate escape channel on the sixth side.
                let a = i as f32 * std::f32::consts::TAU / 6.0 + e.attacks as f32 * 0.65;
                (e.aim + Vec3::new(a.cos(), 0.0, a.sin()) * 3.0, 1.2, 0.65)
            })
            .collect(),
        _ => Vec::new(),
    }
}

pub(super) fn telegraph(e: &Enemy, m: &mut Model) {
    if e.tell <= 0.0 {
        return;
    }
    for (p, r, _) in marks(e) {
        tactics::ring(m, p, r, PALETTE_CORAL, 0.7);
    }
    if matches!(e.attack, 22 | 23) {
        // Low, short rails show the committed launch direction; the world stays visible.
        let dir = (e.aim - e.center()).normalize_or_zero();
        for angle in [-0.4_f32, 0.4] {
            let d = Quat::from_rotation_y(angle) * dir;
            m.beam(
                e.center() + d * 0.6,
                e.center() + d * 2.2,
                0.05,
                PALETTE_CORAL,
                0.6,
            );
        }
    }
}

pub(super) fn release(
    e: &mut Enemy,
    world: &World,
    shots: &mut Vec<Projectile>,
    hazards: &mut Vec<tactics::Hazard>,
) {
    let enraged = e.encounter.boss_phase > 0;
    for (pos, radius, delay) in marks(e) {
        if hazards.len() >= 16 {
            break;
        }
        hazards.push(tactics::Hazard {
            pos,
            radius,
            delay,
            life: 0.25,
            pulse: 0.0,
            damage: if enraged { 24.0 } else { 18.0 },
            spores: false,
        });
    }
    if matches!(e.attack, 22 | 23) {
        let side = Vec3::new(-e.lunge.z, 0.0, e.lunge.x);
        let count = if e.attack == 23 {
            2
        } else if enraged {
            7
        } else {
            5
        };
        for i in 0..count {
            if shots.len() >= 96 {
                break;
            }
            let offset = if e.attack == 23 {
                side * if i == 0 { -1.4 } else { 1.4 }
            } else {
                Vec3::ZERO
            };
            let proposed = e.center() + offset;
            let origin = if line(world, e.center(), proposed) {
                proposed
            } else {
                e.center()
            };
            let angle = if e.attack == 23 {
                0.0
            } else {
                (i as f32 - (count - 1) as f32 * 0.5) * 0.22
            };
            let direction = Quat::from_rotation_y(angle) * (e.aim - origin).normalize_or_zero();
            shots.push(Projectile {
                farm: None,
                pos: origin,
                velocity: direction * if enraged { 12.0 } else { 9.0 },
                hostile: true,
                payload: None,
                payload_mods: Default::default(),
                spell: 0,
                life: 4.0,
                mods: Default::default(),
                bore_budget: 0,
                trail_timer: 0.0,
                splashed: false,
                follow: None,
            });
        }
    }
    let recovery = PATTERNS
        .iter()
        .find(|p| p.id == e.attack)
        .map_or(2.0, |p| p.recovery);
    e.cooldown = recovery * if enraged { 0.8 } else { 1.0 };
    e.encounter.exposed = 0.7;
    e.recovery = 0.24;
}
