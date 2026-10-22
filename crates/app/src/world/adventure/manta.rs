//! Committed low flight, reachable landings and geometry-dependent containment.
//! This module is staged until its habitat/save/arena integration is complete.
use super::*;
use action::{Phase, Run};
pub(super) const KIND: u32 = 16;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Action {
    Pass,
    Claw,
    Volley,
}
#[derive(Default)]
pub(super) struct State {
    pub run: Option<Run<Action>>,
    pub start: Vec3,
    pub end: Vec3,
    pub fuel: u32,
    pub passes: u32,
    pub grounded: bool,
    pub molt: f64,
}
pub(super) fn action_name(e: &Enemy) -> Option<&'static str> {
    Some(if e.manta.fuel == 0 {
        "Perch depleted — needs feed"
    } else if e.manta.grounded {
        "Grounded — flight lane blocked"
    } else {
        match e.manta.run.as_ref().map(|r| (r.kind, r.phase())) {
            Some((_, Phase::Recover)) => "Landed recovery",
            Some((Action::Pass, _)) => "Low pass — intercept or take cover",
            Some((Action::Claw, _)) => "Wing rake — step aside",
            Some((Action::Volley, _)) => "Perched volley",
            None => "At perch",
        }
    })
}
fn supported(world: &World, p: Vec3) -> bool {
    factory::solid_point(world, p - Vec3::Y * 0.15) && free(world, p, KIND)
}
fn lane(world: &World, a: Vec3, b: Vec3) -> bool {
    let steps = (a.distance(b) / 0.25).ceil().clamp(1.0, 96.0) as usize;
    (1..=steps).all(|i| free(world, a.lerp(b, i as f32 / steps as f32), KIND))
}
fn perch(world: &World, home: Vec3, sequence: u32, current: Vec3) -> Option<Vec3> {
    let offsets = [
        Vec3::new(6.0, 0.0, 0.0),
        Vec3::new(-6.0, 0.0, 0.0),
        Vec3::new(0.0, 0.0, 6.0),
        Vec3::ZERO,
    ];
    (0..4)
        .filter_map(|i| {
            let at = home + offsets[(i + sequence as usize) % 4];
            (current.distance(at) > 3.0 && supported(world, at)).then_some(at)
        })
        .next()
}
pub(super) fn tick(
    e: &mut Enemy,
    world: &mut World,
    feet: Vec3,
    eye: Vec3,
    dt: f32,
    shots: &mut Vec<Projectile>,
    sounds: &mut Vec<Sound>,
) -> (f32, Vec3) {
    let mut hit = 0.0;
    let service = family_service(world, e);
    let near = e.visible && feet.distance(e.pos) < 25.0;
    if e.manta.run.is_none() {
        navigation::gravity(e, world, dt);
        // Nutrition pays for flight. Nothing is replenished by retreat or reload.
        if e.manta.fuel == 0 && family_feed(world, e, 64) {
            e.manta.fuel = 3;
        }
        if supported(world, e.pos) && family_can_feed(world, e, 64) {
            e.manta.molt = (e.manta.molt + dt as f64).min(24.0);
            if e.manta.molt >= 24.0
                && family_product_room(world, e, 64)
                && family_feed(world, e, 64)
            {
                family_product(world, e, 64);
                e.manta.molt = 0.0;
            }
        }
        if (!near && !service) || e.cooldown > 0.0 {
            return (0.0, Vec3::ZERO);
        }
        let target = perch(world, e.home, e.manta.passes + 1, e.pos);
        let reachable = target.filter(|p| lane(world, e.pos + Vec3::Y * 0.3, *p + Vec3::Y * 0.3));
        e.manta.grounded = reachable.is_none() || e.manta.fuel == 0;
        let action = if near && feet.distance(e.pos) < 3.2 {
            Action::Claw
        } else if !e.manta.grounded && (service || e.attacks % 2 == 0) {
            Action::Pass
        } else if near {
            Action::Volley
        } else {
            return (0.0, Vec3::ZERO);
        };
        e.manta.start = e.pos;
        e.manta.end = reachable.unwrap_or(e.pos);
        e.aim = eye;
        e.lunge = ((feet - e.pos) * Vec3::new(1.0, 0.0, 1.0)).normalize_or_zero();
        e.yaw = (-e.lunge.x).atan2(-e.lunge.z);
        let (prepare, active, recover) = match action {
            Action::Pass => (1.2, 2.0, 1.5),
            Action::Claw => (0.8, 0.35, 1.0),
            Action::Volley => (1.0, 0.1, 1.2),
        };
        e.attacks = e.attacks.wrapping_add(1);
        e.attack = 8;
        e.tell = prepare;
        e.manta.run = Some(Run::new(action, e.attacks, prepare, active, recover));
        sound(sounds, Cue::Warning, Some(e.center()), 0.7);
        return (0.0, Vec3::ZERO);
    }
    let mut run = e.manta.run.take().unwrap();
    let release = run.advance(dt);
    e.tell = if run.phase() == Phase::Prepare {
        run.prepare - run.elapsed
    } else {
        0.0
    };
    if release {
        match run.kind {
            Action::Pass => {
                e.manta.fuel = e.manta.fuel.saturating_sub(1);
            }
            Action::Volley if e.manta.fuel > 0 && shots.len() <= 93 => {
                e.manta.fuel -= 1;
                for i in -1..=1 {
                    family_bolt(shots, e.center(), e.aim, i as f32 * 0.14, 9.0, 1.0);
                }
            }
            _ => {}
        }
        sound(sounds, Cue::Swing, Some(e.center()), 0.7);
    }
    if run.phase() == Phase::Active {
        if run.kind == Action::Pass {
            let t = ((run.elapsed - run.prepare) / run.active).clamp(0.0, 1.0);
            let q = e.manta.start.lerp(e.manta.end, t)
                + Vec3::Y * (std::f32::consts::PI * t).sin() * 0.55;
            if lane(world, e.pos, q) {
                let old = e.pos;
                e.pos = q;
                e.phase += old.distance(q);
                let flow = (e.manta.end - e.manta.start).normalize_or_zero() * 2.0;
                // The local wake influences real packets, including player utility casts.
                for shot in shots.iter_mut().filter(|s| s.pos.distance(q) < 2.0) {
                    shot.velocity += flow * dt;
                }
            } else {
                e.manta.grounded = true;
                run.interrupt();
                e.cooldown = 0.8;
            }
        }
        if matches!(run.kind, Action::Pass | Action::Claw)
            && !run.hit
            && (feet - e.pos).length() < if run.kind == Action::Claw { 3.0 } else { 1.8 }
            && (run.kind == Action::Pass || e.lunge.dot((feet - e.pos).normalize_or_zero()) > 0.0)
            && line(world, e.center(), eye)
        {
            hit = if run.kind == Action::Claw { 18.0 } else { 24.0 };
            run.hit = true;
        }
    }
    if run.phase() == Phase::Recover {
        navigation::gravity(e, world, dt);
    }
    if run.phase() != Phase::Done {
        e.manta.run = Some(run);
    } else {
        e.manta.passes = e.manta.passes.wrapping_add(1);
        e.cooldown = 0.6;
    }
    (hit, (e.center() - eye).normalize_or_zero() * hit)
}
pub(super) fn model(e: &Enemy, m: &mut Model, _time: f32) {
    let mark = m.mark();
    m.set_surface_kind(58.0);
    let flying = e
        .manta
        .run
        .as_ref()
        .is_some_and(|r| r.kind == Action::Pass && r.phase() == Phase::Active);
    let folded = if flying { 0.0 } else { 0.9 };
    let local = |x, y, z| e.pos + Vec3::new(x, y, z);
    let body = if e.flash > 0.0 {
        PALETTE_PAPER
    } else {
        [0.28, 0.42, 0.48]
    };
    m.cuboid(
        local(0.0, 0.65, 0.0),
        Vec3::new(1.25, 0.7, 1.65),
        Quat::IDENTITY,
        body,
        0.0,
    );
    for side in [-1.0, 1.0] {
        for i in 0..5 {
            let spread = i as f32 * 0.4 + 0.5;
            let z = 0.5 - i as f32 * 0.3;
            let tip = local(
                side * (spread * (1.0 - folded * 0.55)),
                0.55 + folded * spread * 0.45,
                z,
            );
            m.beam(
                local(side * 0.4, 0.8, z - 0.2),
                tip,
                0.1,
                [0.62, 0.74, 0.72],
                0.0,
            );
            m.cuboid(
                (local(side * 0.4, 0.8, z - 0.2) + tip) * 0.5,
                Vec3::new((spread * 0.6).max(0.3), 0.1, 0.35),
                Quat::from_rotation_z(side * folded * 0.4),
                [0.35, 0.5, 0.56],
                0.0,
            );
        }
    }
    for x in [-0.35, 0.35] {
        m.cuboid(
            local(x, 0.8, -0.8),
            Vec3::splat(0.1),
            Quat::IDENTITY,
            PALETTE_AMBER,
            0.25,
        );
    }
    m.beam(local(0.0, 0.6, 0.6), local(0.0, 0.35, 2.0), 0.15, body, 0.0);
    m.bake_model_uv_since(mark, e.pos, 58.0);
    m.rotate_since(mark, e.pos, Quat::from_rotation_y(e.yaw));
    m.set_surface_kind(41.0);
    if let Some(run) = &e.manta.run {
        if run.kind == Action::Pass && run.phase() == Phase::Prepare {
            m.beam(
                e.manta.start + Vec3::Y * 0.1,
                e.manta.end + Vec3::Y * 0.1,
                0.05,
                PALETTE_CORAL,
                0.1,
            );
        }
    }
}
impl State {
    pub(super) fn save(&self, out: &mut Vec<u32>) {
        out.extend([
            self.fuel,
            self.passes,
            self.grounded as u32,
            self.molt.to_bits() as u32,
            (self.molt.to_bits() >> 32) as u32,
        ]);
    }
    pub(super) fn restore(&mut self, words: &[u32]) -> anyhow::Result<()> {
        anyhow::ensure!(
            words.len() == 5 && words[0] <= 3 && words[2] <= 1,
            "Invalid Manta state"
        );
        let molt = f64::from_bits(words[3] as u64 | ((words[4] as u64) << 32));
        anyhow::ensure!(
            molt.is_finite() && molt >= 0.0,
            "Invalid Manta molting clock"
        );
        self.fuel = words[0];
        self.passes = words[1];
        self.grounded = words[2] != 0;
        self.molt = molt;
        Ok(())
    }
}
