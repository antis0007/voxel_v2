//! Bounded local tactics; attacks commit to their telegraphed target.
use super::*;
pub(super) struct Hazard {
    pub pos: Vec3,
    pub radius: f32,
    pub delay: f32,
    pub life: f32,
    pub pulse: f32,
    pub damage: f32,
    pub spores: bool,
}
pub(super) fn heading(
    e: &Enemy,
    goal: Vec3,
    distance: f32,
    chasing: bool,
    neighbors: &[(Vec3, IVec3)],
    shots: &[Projectile],
    time: f32,
) -> Vec3 {
    let toward = ((goal - e.pos) * Vec3::new(1.0, 0.0, 1.0)).normalize_or_zero();
    let side = Vec3::new(-toward.z, 0.0, toward.x) * if e.slot % 2 == 0 { 1.0 } else { -1.0 };
    let mut direction = toward;
    if chasing && e.visible {
        if brawler(e.kind) {
            if distance > 3.0 {
                direction += side * 0.65;
            }
        } else {
            direction = toward
                * if distance < 6.0 {
                    -1.0
                } else if distance > 11.0 {
                    1.0
                } else {
                    0.0
                }
                + side * (0.6 + 0.2 * (time + e.slot as f32).sin());
        }
        if !brawler(e.kind)
            && shots.iter().any(|s| {
                !s.hostile
                    && (s.pos - e.center()).length_squared() < 25.0
                    && s.velocity
                        .normalize_or_zero()
                        .dot((e.center() - s.pos).normalize_or_zero())
                        > 0.85
            })
        {
            direction += side * 1.8;
        }
    }
    for &(pos, _) in neighbors {
        let away = (e.pos - pos) * Vec3::new(1.0, 0.0, 1.0);
        let d = away.length_squared();
        if d > 0.001 && d < 2.25 {
            direction += away.normalize_or_zero() * (1.0 - d / 2.25);
        }
    }
    direction.normalize_or_zero()
}
pub(super) fn release(e: &mut Enemy, shots: &mut Vec<Projectile>, hazards: &mut Vec<Hazard>) {
    if e.attack == 2 {
        if hazards.len() < 16 {
            hazards.push(Hazard {
                pos: e.aim,
                radius: if e.kind == 5 { 3.6 } else { 2.6 },
                delay: 0.35,
                life: if e.kind == 5 { 0.2 } else { 3.2 },
                pulse: 0.0,
                damage: if e.kind == 5 { 22.0 } else { 8.0 },
                spores: e.kind != 5,
            });
        }
        return;
    }
    let count = if e.kind == 5 {
        5
    } else if e.kind == 4 || e.kind == 1 && e.attacks % 2 == 0 {
        3
    } else if e.kind == 2 {
        2
    } else {
        1
    };
    let speed = if e.kind == 1 && count == 1 {
        16.0
    } else if tall(e.kind) {
        11.0
    } else {
        8.0
    };
    for j in 0..count {
        if shots.len() >= 96 {
            break;
        }
        let spread = (j as f32 - (count - 1) as f32 * 0.5) * if e.kind == 5 { 0.18 } else { 0.12 };
        shots.push(Projectile {
            pos: e.center(),
            velocity: Quat::from_rotation_y(spread)
                * (e.aim - e.center()).normalize_or_zero()
                * speed,
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
fn ring(m: &mut Model, pos: Vec3, radius: f32, color: [f32; 3], glow: f32) {
    for i in 0..24 {
        let a = i as f32 * std::f32::consts::TAU / 24.0;
        let b = (i + 1) as f32 * std::f32::consts::TAU / 24.0;
        m.beam(
            pos + Vec3::new(a.cos() * radius, 0.08, a.sin() * radius),
            pos + Vec3::new(b.cos() * radius, 0.08, b.sin() * radius),
            0.025,
            color,
            glow,
        );
    }
}
impl Adventure {
    pub(super) fn tick_hazards(&mut self, world: &World, feet: Vec3, eye: Vec3, dt: f32) {
        let mut incoming = 0.0;
        let mut direction = Vec3::ZERO;
        for h in &mut self.hazards {
            if h.delay > 0.0 {
                h.delay -= dt;
                continue;
            }
            h.life -= dt;
            h.pulse -= dt;
            if h.pulse <= 0.0 {
                h.pulse = 0.8;
                sound(
                    &mut self.sounds,
                    if h.spores { Cue::Bloom } else { Cue::Burst },
                    Some(h.pos),
                    0.45,
                );
                let d = feet - h.pos;
                if Vec3::new(d.x, 0.0, d.z).length() < h.radius
                    && d.y.abs() < 1.8
                    && line(world, h.pos + Vec3::Y * 0.3, eye)
                {
                    incoming += h.damage;
                    direction += h.pos - eye;
                }
            }
        }
        self.hazards.retain(|h| h.life > 0.0);
        if incoming > 0.0 {
            self.damage(incoming, direction);
        }
    }
    pub(super) fn danger_models(&self, m: &mut Model) {
        for e in &self.enemies {
            if e.tell > 0.0 && e.attack == 2 {
                ring(
                    m,
                    e.aim,
                    if e.kind == 5 { 3.6 } else { 2.6 },
                    [1.0, 0.44, 0.12],
                    0.9,
                );
            }
        }
        for h in &self.hazards {
            let color = if h.spores {
                [0.75, 0.32, 0.15]
            } else {
                [1.0, 0.52, 0.18]
            };
            ring(m, h.pos, h.radius, color, 1.0);
            if h.delay <= 0.0 {
                for i in 0..10 {
                    let a = self.time * 1.8 + i as f32 * 2.4;
                    let r = h.radius * (i as f32 + 1.0) / 11.0;
                    m.orb(
                        h.pos
                            + Vec3::new(
                                a.cos() * r,
                                0.2 + (a * 1.7).sin().abs() * 0.5,
                                a.sin() * r,
                            ),
                        0.07,
                        color,
                        1.0,
                    );
                }
            }
        }
    }
}
