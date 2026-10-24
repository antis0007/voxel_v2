//! Finite brood, committed physical attacks and productive nursery work.
//! An owned pod is carried OR a source; every replenishment consumes biomass.
use super::*;
use action::{Phase, Run};

pub(super) const KIND: u32 = 15;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Action {
    Rake,
    Stamp,
    Pollen,
    Plant,
}
#[derive(Default)]
pub(super) struct State {
    pub run: Option<Run<Action>>,
    pub pods: u32,
    pub spores: u32,
    pub work: f64,
    pub plants: u32,
    pub interrupted: u32,
    pub pending_drop: bool,
    pub ready: bool,
}
pub(super) fn action_name(e: &Enemy) -> Option<&'static str> {
    e.matron
        .run
        .as_ref()
        .map(|r| match (r.kind, r.phase()) {
            (_, Phase::Recover) => "Recovering",
            (Action::Rake, _) => "Branch rake — step behind",
            (Action::Stamp, _) => "Root stamp — jump or retreat",
            (Action::Pollen, _) => "Pollen fan — use cover",
            (Action::Plant, _) => "Planting brood — interrupt or recover the pod",
        })
        .or(Some(if e.matron.pods == 0 && e.matron.spores < 6 {
            "Foraging for nursery supplies"
        } else {
            "Tending the nursery"
        }))
}
fn flat(v: Vec3) -> Vec3 {
    v * Vec3::new(1.0, 0.0, 1.0)
}
fn move_ground(e: &mut Enemy, world: &World, direction: Vec3, distance: f32) {
    let steps = (distance / 0.1).ceil().max(1.0) as u32;
    for _ in 0..steps {
        let q = e.pos + direction * distance / steps as f32;
        if free(world, q, KIND) && factory::solid_point(world, q - Vec3::Y * 0.15) {
            e.phase += q.distance(e.pos);
            e.pos = q;
        } else {
            break;
        }
    }
}
fn place(e: &mut Enemy, world: &mut World, target: Vec3) -> bool {
    if e.matron.pods == 0 {
        return false;
    }
    let Some(owner) = e.habitat else {
        return false;
    };
    if e.pos.distance(target) > 2.0 || !line(world, e.center(), target + Vec3::Y * 0.3) {
        return false;
    }
    if super::super::habitats::plant_brood(world, owner, target) {
        // Transfer only after the destination owns the viable source.
        e.matron.pods -= 1;
        e.matron.plants += 1;
        println!(
            "[matron] planted owner={owner} carried={} at={target:?}",
            e.matron.pods
        );
        true
    } else {
        false
    }
}
pub(super) fn hurt(e: &mut Enemy, damage: f32) {
    if damage < 18.0 {
        return;
    }
    if let Some(run) = &mut e.matron.run
        && run.kind == Action::Plant && run.phase() == Phase::Prepare {
            run.interrupt();
            e.matron.pending_drop = true;
            e.matron.interrupted += 1;
        }
}

pub(super) fn tick(
    e: &mut Enemy,
    world: &mut World,
    feet: Vec3,
    eye: Vec3,
    dt: f32,
    shots: &mut Vec<Projectile>,
    sounds: &mut Vec<Sound>,
    navigation_budget: &mut usize,
) -> (f32, Vec3) {
    navigation::gravity(e, world, dt);
    if !e.matron.ready {
        e.matron.ready = true;
        // Variants differ in how far their nursery work has progressed, never in
        // free stock: every pod and spore charge is still paid for in biomass.
        let variant = boss_look::index(e);
        e.matron.work = e.matron.work.max([0.0, 6.0, 10.0][variant]);
    }
    e.matron.work += dt as f64;
    // A source's real stock replenishes one resource, never both from one payment.
    if e.matron.work >= 12.0 {
        e.matron.work -= 12.0;
        if let Some(source) = e
            .habitat
            .and_then(|id| world.habitats.sources.iter_mut().find(|s| s.id == id))
            && source.feed >= 64 {
                if e.matron.pods < 2 {
                    source.feed -= 64;
                    source.feed_used = source.feed_used.saturating_add(64);
                    world.factory.stats.used(13, 64);
                    e.matron.pods += 1;
                } else if e.matron.spores < 6 {
                    source.feed -= 64;
                    source.feed_used = source.feed_used.saturating_add(64);
                    world.factory.stats.used(13, 64);
                    e.matron.spores += 6;
                }
            }
    }
    if e.matron.pending_drop
        && place(e, world, e.pos + Vec3::new(1.6, 0.0, 0.0)) {
            e.matron.pending_drop = false;
        }
    let mut damage = 0.0;
    if let Some(mut run) = e.matron.run.take() {
        if e.stagger > 0.3 && run.phase() == Phase::Prepare {
            run.interrupt();
        }
        let release = run.advance(dt);
        e.tell = if run.phase() == Phase::Prepare {
            run.prepare - run.elapsed
        } else {
            0.0
        };
        if release {
            if run.kind == Action::Stamp && super::super::habitats::review::enabled() {
                println!(
                    "[matron] stamp-release sequence={} feet_height={:.3} range={:.3}",
                    run.sequence,
                    feet.y - e.pos.y,
                    flat(feet - e.pos).length()
                );
            }
            match run.kind {
                Action::Plant => {
                    place(e, world, e.aim);
                }
                Action::Pollen if e.matron.spores >= 6 && shots.len() <= 90 => {
                    let aim = (e.aim - e.center()).normalize_or_zero();
                    for i in 0..6 {
                        shots.push(Projectile {
                            farm: None,
                            pos: e.center(),
                            velocity: Quat::from_rotation_y((i as f32 - 2.5) * 0.16) * aim * 9.0,
                            hostile: true,
                            payload: None,
                            payload_mods: Default::default(),
                            spell: 0,
                            life: 3.0,
                            mods: wands::ShotMods {
                                damage_mul: 1.25,
                                ..Default::default()
                            },
                            bore_budget: 0,
                            trail_timer: 0.0,
                            splashed: false,
                            follow: None,
                        });
                    }
                    e.matron.spores -= 6;
                }
                _ => {}
            }
            sound(sounds, Cue::Swing, Some(e.center()), 0.7);
        }
        if run.phase() == Phase::Active {
            if run.kind == Action::Rake {
                move_ground(e, world, e.lunge, dt * 1.5);
            }
            let d = flat(feet - e.pos);
            let contact = match run.kind {
                Action::Rake => {
                    let progress = ((run.elapsed - run.prepare) / run.active).clamp(0.0, 1.0);
                    let previous = ((run.elapsed - dt - run.prepare) / run.active).clamp(0.0, 1.0);
                    let angle = e
                        .lunge
                        .cross(d.normalize_or_zero())
                        .y
                        .atan2(e.lunge.dot(d.normalize_or_zero()));
                    d.length() < 3.6
                        && angle >= -1.7 + previous * 3.4 - 0.18
                        && angle <= -1.7 + progress * 3.4 + 0.18
                }
                Action::Stamp => d.length() < 3.4 && feet.y - e.pos.y < 0.65,
                _ => false,
            };
            if !run.hit && contact && (feet.y - e.pos.y).abs() < 2.1 && line(world, e.center(), eye)
            {
                damage = if run.kind == Action::Stamp {
                    24.0
                } else {
                    19.0
                };
                run.hit = true;
                println!(
                    "[matron] contact action={:?} sequence={} damage={damage}",
                    run.kind, run.sequence
                );
            }
        }
        if run.phase() != Phase::Done {
            e.matron.run = Some(run);
        } else {
            if run.kind == Action::Stamp && super::super::habitats::review::enabled() {
                println!(
                    "[matron] stamp-complete sequence={} hit={}",
                    run.sequence, run.hit
                );
            }
            e.cooldown = 0.4;
        }
        return (damage, (e.center() - eye).normalize_or_zero() * damage);
    }
    e.tell = 0.0;
    if e.cooldown > 0.0 {
        return (0.0, Vec3::ZERO);
    }
    let distance = flat(feet - e.pos).length();
    let active = distance < 24.0 && e.visible;
    let service = e
        .habitat
        .and_then(|id| world.habitats.sources.iter().find(|s| s.id == id))
        .is_some_and(|s| s.automatic);
    if !active && !service {
        return (0.0, Vec3::ZERO);
    }
    let direction = flat(if active { feet - e.pos } else { Vec3::NEG_Z }).normalize_or_zero();
    let bed = e
        .habitat
        .and_then(|id| super::super::habitats::brood_target(world, id, e.home));
    let kind = if active && distance < 3.5 {
        if e.attacks.is_multiple_of(2) {
            Action::Stamp
        } else {
            Action::Rake
        }
    } else if e.matron.pods > 0 && bed.is_some() && (e.attacks.is_multiple_of(3) || !active) {
        Action::Plant
    } else if active && e.matron.spores >= 6 {
        Action::Pollen
    } else {
        if active {
            let goal = flat(feet - e.pos).normalize_or_zero();
            if e.pos.distance(e.home) < 24.0 {
                move_ground(e, world, goal, dt * 1.0);
            }
        } else if e.pos.distance(e.home) > 0.3 {
            move_ground(e, world, flat(e.home - e.pos).normalize_or_zero(), dt);
        }
        return (0.0, Vec3::ZERO);
    };
    e.lunge = direction;
    e.yaw = (-direction.x).atan2(-direction.z);
    e.aim = if kind == Action::Plant {
        let target = bed.unwrap();
        if e.pos.distance(target) > 1.8 {
            let before = e.pos;
            let direction =
                e.navigation
                    .heading(world, e.pos, target, KIND, dt, e.slot, navigation_budget);
            move_ground(e, world, direction, dt);
            if e.pos.distance_squared(before) < 0.000001 {
                // Containment is valid; blocked work cannot suppress combat.
                e.attacks = e.attacks.wrapping_add(1);
                e.cooldown = 0.3;
            }
            return (0.0, Vec3::ZERO);
        }
        target
    } else {
        eye
    };
    let (prepare, active, recover) = match kind {
        Action::Rake => (0.85, 0.4, 1.05),
        Action::Stamp => (1.15, 0.2, 1.1),
        Action::Pollen => (1.2, 0.1, 1.0),
        Action::Plant => (1.5, 0.15, 1.1),
    };
    e.attacks = e.attacks.wrapping_add(1);
    e.matron.run = Some(Run::new(kind, e.attacks, prepare, active, recover));
    e.tell = prepare;
    e.attack = 8;
    sound(sounds, Cue::Warning, Some(e.center()), 0.7);
    println!(
        "[matron] action={kind:?} sequence={} pods={} spores={}",
        e.attacks, e.matron.pods, e.matron.spores
    );
    (0.0, Vec3::ZERO)
}

mod model;
pub(super) use model::model;
