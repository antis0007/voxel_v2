//! A bounded, fallible listener: real stimuli and editable air connections.
//! Staged until the family habitat/save/stimulus/arena integration is complete.
use super::*;
use action::{Phase, Run};
use std::collections::{HashSet, VecDeque};
pub(super) const KIND: u32 = 18;
#[derive(Clone, Copy)]
pub(super) struct Stimulus {
    pub pos: Vec3,
    pub strength: f32,
    pub sequence: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Action {
    Pressure,
    Shove,
}
#[derive(Default)]
pub(super) struct State {
    pub run: Option<Run<Action>>,
    pub inferred: Vec3,
    pub confidence: f32,
    pub last_stimulus: u32,
    pub mouth: usize,
    pub origin: Vec3,
    pub tissue: f64,
    pub responses: u32,
}
fn organs(home: Vec3) -> [Vec3; 3] {
    [
        home + Vec3::new(-4.0, 1.2, 0.0),
        home + Vec3::Y * 1.2,
        home + Vec3::new(4.0, 1.2, 0.0),
    ]
}
fn connected(world: &World, a: Vec3, b: Vec3) -> bool {
    if a.distance(b) > 18.0 {
        return false;
    }
    if line(world, a, b) {
        return true;
    }
    let s = world.voxel_scale.meters();
    let step = factory::step(world);
    let start = factory::anchor((a / s).floor().as_ivec3(), step);
    let goal = factory::anchor((b / s).floor().as_ivec3(), step);
    let mut queue = VecDeque::from([start]);
    let mut visited = HashSet::from([start]);
    while let Some(p) = queue.pop_front() {
        if (p - goal).abs().max_element() <= step {
            return true;
        }
        if visited.len() >= 192 {
            break;
        }
        for d in [
            IVec3::X,
            IVec3::NEG_X,
            IVec3::Y,
            IVec3::NEG_Y,
            IVec3::Z,
            IVec3::NEG_Z,
        ] {
            let q = p + d * step;
            if (q - start).abs().max_element() > 18 * step || visited.contains(&q) || world.solid(q)
            {
                continue;
            }
            visited.insert(q);
            queue.push_back(q);
        }
    }
    false
}
pub(super) fn hit(e: &Enemy, p: Vec3, extra: f32) -> bool {
    organs(e.home).iter().any(|o| o.distance(p) < 0.8 + extra)
}
pub(super) fn action_name(e: &Enemy) -> Option<&'static str> {
    Some(match e.choir.run.as_ref().map(|r| (r.kind, r.phase())) {
        Some((_, Phase::Recover)) => "Diaphragm relaxed — recovering",
        Some((Action::Pressure, _)) => "Inhaling toward the last disturbance",
        Some((Action::Shove, _)) => "Defensive breath — step back",
        None if e.choir.confidence > 0.0 => "Following a disturbance",
        None => "Listening — noise reveals a route",
    })
}
pub(super) fn tick(
    e: &mut Enemy,
    world: &mut World,
    feet: Vec3,
    eye: Vec3,
    dt: f32,
    stimuli: &[Stimulus],
    sounds: &mut Vec<Sound>,
) -> (f32, Vec3) {
    e.choir.confidence = (e.choir.confidence - dt * 0.18).max(0.0);
    let ports = organs(e.home);
    if e.choir.run.is_none() {
        // Own warning/release sounds are excluded by the shared event collector.
        for signal in stimuli {
            if signal.sequence == e.choir.last_stimulus {
                continue;
            }
            let mut heard = None;
            for (i, p) in ports.iter().enumerate() {
                let strength = signal.strength / (1.0 + p.distance(signal.pos) * 0.25);
                if strength > 0.1
                    && connected(world, *p, signal.pos)
                    && heard.is_none_or(|(_, score)| strength > score)
                {
                    heard = Some((i, strength));
                }
            }
            if let Some((mouth, strength)) = heard {
                if strength >= e.choir.confidence * 0.6 {
                    e.choir.inferred = signal.pos;
                    e.choir.confidence = strength;
                    e.choir.last_stimulus = signal.sequence;
                    e.choir.mouth = mouth;
                }
            }
        }
        // A useful renewable organ product consumes its own food event.
        if family_can_feed(world, e, 64) {
            e.choir.tissue = (e.choir.tissue + dt as f64).min(30.0);
        }
        if e.choir.tissue >= 30.0 && family_product_room(world, e, 64) && family_feed(world, e, 64)
        {
            family_product(world, e, 64);
            e.choir.tissue = 0.0;
        }
        if e.cooldown > 0.0 {
            return (0.0, Vec3::ZERO);
        }
        let close = ports
            .iter()
            .position(|p| p.distance(eye) < 2.3 && line(world, *p, eye));
        let action = if let Some(i) = close {
            e.choir.mouth = i;
            e.choir.inferred = eye;
            Action::Shove
        } else if e.choir.confidence > 0.15 {
            Action::Pressure
        } else {
            return (0.0, Vec3::ZERO);
        };
        e.choir.origin = ports[e.choir.mouth];
        e.aim = e.choir.inferred;
        e.lunge = (e.aim - e.choir.origin).normalize_or_zero();
        e.attacks = e.attacks.wrapping_add(1);
        e.attack = 8;
        let prepare = if action == Action::Pressure { 1.3 } else { 0.7 };
        e.tell = prepare;
        e.choir.run = Some(Run::new(action, e.attacks, prepare, 0.25, 1.2));
        sound(sounds, Cue::ArchiveWindup, Some(e.choir.origin), 0.7);
        println!(
            "[choir] action={action:?} mouth={} inferred={:?} confidence={:.2}",
            e.choir.mouth, e.aim, e.choir.confidence
        );
        return (0.0, Vec3::ZERO);
    }
    let mut run = e.choir.run.take().unwrap();
    if e.stagger > 0.35 && run.phase() == Phase::Prepare {
        run.interrupt();
    }
    let release = run.advance(dt);
    e.tell = if run.phase() == Phase::Prepare {
        run.prepare - run.elapsed
    } else {
        0.0
    };
    if release {
        sound(sounds, Cue::ArchiveRelease, Some(e.choir.origin), 0.8);
        e.choir.responses = e.choir.responses.saturating_add(1);
        world.sandbox.apply_radial_impulse(
            e.choir.origin + e.lunge * 2.0,
            2.5,
            4.0,
            world.voxel_scale.meters(),
        );
    }
    let mut damage = 0.0;
    if run.phase() == Phase::Active && !run.hit {
        let d = eye - e.choir.origin;
        let range = if run.kind == Action::Shove { 2.7 } else { 10.0 };
        if d.length() < range
            && d.normalize_or_zero().dot(e.lunge) > 0.88
            && line(world, e.choir.origin, eye)
        {
            damage = if run.kind == Action::Shove {
                13.0
            } else {
                26.0
            };
            run.hit = true;
        }
    }
    if run.phase() != Phase::Done {
        e.choir.run = Some(run);
    } else {
        e.cooldown = 0.8;
        e.choir.confidence *= 0.3;
    }
    (damage, -e.lunge * damage)
}
pub(super) fn model(e: &Enemy, m: &mut Model, _time: f32) {
    let mark = m.mark();
    m.set_surface_kind(58.0);
    let ports = organs(e.home);
    let wind = e.choir.run.as_ref().map_or(0.0, |r| {
        if r.phase() == Phase::Prepare {
            r.wind()
        } else {
            0.0
        }
    });
    for (i, p) in ports.iter().enumerate() {
        let selected = i == e.choir.mouth && e.choir.run.is_some();
        let color = if e.flash > 0.0 {
            PALETTE_PAPER
        } else {
            [0.56, 0.34, 0.28]
        };
        m.beam(
            e.home + Vec3::new(0.0, 0.15, 1.0),
            *p + Vec3::Z * 0.6,
            0.15,
            [0.3, 0.33, 0.23],
            0.0,
        );
        m.cuboid(
            *p + Vec3::Z * 0.3,
            Vec3::new(1.0, 1.1, 0.7),
            Quat::IDENTITY,
            color,
            0.0,
        );
        for j in 0..4 {
            let a = j as f32 * std::f32::consts::FRAC_PI_2;
            let r = 0.5 + if selected { wind * 0.25 } else { 0.0 };
            m.cuboid(
                *p + Vec3::new(a.sin() * r, a.cos() * r, -0.25),
                Vec3::new(0.3, 0.3, 0.25),
                Quat::from_rotation_z(-a),
                [0.72, 0.58, 0.4],
                0.0,
            );
        }
        m.cuboid(
            *p - Vec3::Z * 0.1,
            Vec3::new(0.45, 0.5, 0.1),
            Quat::IDENTITY,
            [0.15, 0.12, 0.14],
            0.0,
        );
    }
    m.bake_model_uv_since(mark, e.home, 58.0);
    m.set_surface_kind(41.0);
    if e.choir
        .run
        .as_ref()
        .is_some_and(|r| r.phase() == Phase::Prepare)
    {
        m.beam(e.choir.origin, e.aim, 0.05, PALETTE_CORAL, 0.1);
    }
}
impl State {
    pub(super) fn save(&self, out: &mut Vec<u32>) {
        out.extend(self.inferred.to_array().map(f32::to_bits));
        out.extend([
            self.confidence.to_bits(),
            self.last_stimulus,
            self.mouth as u32,
            self.tissue.to_bits() as u32,
            (self.tissue.to_bits() >> 32) as u32,
            self.responses,
        ]);
    }
    pub(super) fn restore(&mut self, words: &[u32]) -> anyhow::Result<()> {
        anyhow::ensure!(words.len() == 9 && words[5] < 3, "Invalid Choir state");
        let inferred = Vec3::new(
            f32::from_bits(words[0]),
            f32::from_bits(words[1]),
            f32::from_bits(words[2]),
        );
        let confidence = f32::from_bits(words[3]);
        let tissue = f64::from_bits(words[6] as u64 | ((words[7] as u64) << 32));
        anyhow::ensure!(
            inferred.is_finite()
                && inferred.abs().max_element() <= 1_000_000.0
                && confidence.is_finite()
                && (0.0..=16.0).contains(&confidence)
                && tissue.is_finite()
                && (0.0..=30.0).contains(&tissue),
            "Invalid Choir perception"
        );
        self.inferred = inferred;
        self.confidence = confidence;
        self.last_stimulus = words[4];
        self.mouth = words[5] as usize;
        self.tissue = tissue;
        self.responses = words[8];
        Ok(())
    }
}
