//! The Glassweaver tends a small lattice of prism nodes anchored around its
//! arena. Every bolt it threads travels the visible strands from the node at its
//! hands to the node nearest the committed aim, then discharges from there.
//! Breaking a node (shots that strike it) cuts its strands, so the next bolt
//! reroutes or fizzles; the weaver walks over to mend what the player broke.
use super::*;
use action::{Phase, Run};

pub(super) const KIND: u32 = 7;
pub(super) const MAX_NODES: usize = 6;
pub(super) const NODE_HEALTH: f32 = 34.0;
const NODE_RADIUS: f32 = 0.55;
const BOLT_SPEED: f32 = 7.5;
const MAX_MENDS: u32 = 8;
const LASH_RANGE: f32 = 2.8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Action {
    Weave,
    Lash,
    Mend,
    Shift,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(super) enum Payload {
    #[default]
    Shard,
    /// Unlocked at half health: three diverging shards.
    Fan,
}
#[derive(Clone, Copy, Default)]
pub(super) struct Node {
    pub pos: Vec3,
    /// Zero or less means shattered: its strands carry nothing.
    pub health: f32,
}
#[derive(Clone, Copy, Default)]
pub(super) struct Bolt {
    pub path: [u8; MAX_NODES],
    pub len: u8,
    /// Index of the strand currently travelled (path[at] -> path[at + 1]).
    pub at: u8,
    pub t: f32,
    pub payload: Payload,
    pub aim: Vec3,
}
#[derive(Clone, Default)]
pub(super) struct State {
    pub run: Option<Run<Action>>,
    pub nodes: [Node; MAX_NODES],
    pub count: u8,
    pub bolt: Option<Bolt>,
    pub mend: Option<u8>,
    pub shift_to: Vec3,
    initialized: bool,
    pub enraged: bool,
    pub breaks: u32,
    pub mends: u32,
    pub fired: u32,
}

impl State {
    pub(super) fn intact(&self, i: usize) -> bool {
        i < self.count as usize && self.nodes[i].health > 0.0
    }
    /// Ring strands plus one diagonal brace: a compact, readable lattice.
    pub(super) fn linked(&self, a: usize, b: usize) -> bool {
        let n = self.count as usize;
        if a == b || a >= n || b >= n {
            return false;
        }
        let ring = (a + 1) % n == b || (b + 1) % n == a;
        let brace = (a.min(b), a.max(b)) == (0, n / 2);
        ring || brace
    }
    pub(super) fn strand(&self, a: usize, b: usize) -> bool {
        self.linked(a, b) && self.intact(a) && self.intact(b)
    }
}

pub(super) fn action_name(e: &Enemy) -> Option<&'static str> {
    let s = &e.glass;
    if let Some(r) = &s.run {
        return Some(match (r.kind, r.phase()) {
            (_, Phase::Recover) => "Recovering",
            (Action::Weave, _) => "Threading a bolt — break the lit strand",
            (Action::Lash, _) => "Glass lash — back away",
            (Action::Mend, _) => "Mending a prism — interrupt it",
            (Action::Shift, _) => "Re-anchoring",
        });
    }
    Some(if s.bolt.is_some() {
        "Bolt in the lattice — shatter the node ahead of it"
    } else {
        "Tending the lattice"
    })
}

fn flat(v: Vec3) -> Vec3 {
    v * Vec3::new(1.0, 0.0, 1.0)
}
/// Where the weaver's needles meet: the bolt's entry into the lattice.
pub(super) fn hands(e: &Enemy) -> Vec3 {
    e.pos + Quat::from_rotation_y(e.yaw) * Vec3::new(0.0, 1.9, -0.55)
}

fn initialize(e: &mut Enemy) {
    if e.glass.initialized {
        return;
    }
    e.glass.initialized = true;
    let variant = boss_look::index(e);
    e.glass.count = [5u8, 4, 6][variant];
    let radius = [5.0, 4.4, 5.6][variant];
    let turn = engine_core::hash::hash01(e.site.x as u32 ^ (e.site.z as u32).rotate_left(11)) * std::f32::consts::TAU;
    for i in 0..e.glass.count as usize {
        let a = turn + i as f32 * std::f32::consts::TAU / e.glass.count as f32;
        e.glass.nodes[i] = Node {
            pos: e.home + Vec3::new(a.cos() * radius, 1.5 + 0.35 * (i as f32 * 2.1).sin(), a.sin() * radius),
            health: node_health(e),
        };
    }
    e.glass.shift_to = e.home;
}
fn node_health(e: &Enemy) -> f32 {
    // Smoked glass is tougher; it has fewer nodes to lose.
    NODE_HEALTH * [1.0, 1.4, 0.85][boss_look::index(e)]
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

/// Half health: one anchor tears loose and re-seats elsewhere, whole again,
/// and the weaver starts threading fan payloads.
fn escalate(e: &mut Enemy, world: &mut World, sounds: &mut Vec<Sound>) {
    if e.glass.enraged || e.health >= e.max_health() * 0.5 {
        return;
    }
    e.glass.enraged = true;
    let n = e.glass.count as usize;
    let moved = (boss_look::index(e) + 1) % n;
    let offset = e.glass.nodes[moved].pos - e.home;
    e.glass.nodes[moved].pos = e.home + Quat::from_rotation_y(0.9) * offset + Vec3::Y * 0.6;
    e.glass.nodes[moved].health = node_health(e);
    e.cooldown = e.cooldown.max(1.2);
    sound(sounds, Cue::Warning, Some(e.center()), 0.9);
    world.expedition.say("The Glassweaver tears an anchor free and re-seats it.");
}

fn begin(e: &mut Enemy, kind: Action, sounds: &mut Vec<Sound>) {
    let (prepare, active, recover) = match kind {
        Action::Weave => (if e.glass.enraged { 0.8 } else { 1.05 }, 0.12, 0.85),
        Action::Lash => (0.55, 0.25, 0.8),
        Action::Mend => (1.6, 0.1, 0.8),
        Action::Shift => (0.35, 1.0, 0.35),
    };
    e.attacks = e.attacks.wrapping_add(1);
    e.glass.run = Some(Run::new(kind, e.attacks, prepare, active, recover));
    e.attack = 8;
    e.tell = prepare;
    sound(sounds, if kind == Action::Weave { Cue::ArchiveWindup } else { Cue::Warning }, Some(e.center()), 0.7);
}

#[allow(clippy::too_many_arguments)]
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
    initialize(e);
    navigation::gravity(e, world, dt);
    if (feet - e.home).length() > 48.0 {
        e.glass.run = None;
        e.glass.bolt = None;
        e.tell = 0.0;
        return (0.0, Vec3::ZERO);
    }
    strike_nodes(e, world, shots, dt, sounds);
    carry(e, dt, shots, sounds);
    escalate(e, world, sounds);
    let mut damage = 0.0;
    if let Some(mut run) = e.glass.run.take() {
        if e.stagger > 0.3 && run.phase() == Phase::Prepare {
            run.interrupt();
        }
        let release = run.advance(dt);
        e.tell = if run.phase() == Phase::Prepare { run.prepare - run.elapsed } else { 0.0 };
        if release {
            match run.kind {
                Action::Weave => {
                    if let Some((path, len)) = route(&e.glass, hands(e), e.aim) {
                        let payload = if e.glass.enraged && run.sequence.is_multiple_of(2) { Payload::Fan } else { Payload::Shard };
                        e.glass.bolt = Some(Bolt { path, len, at: 0, t: 0.0, payload, aim: e.aim });
                    }
                }
                Action::Mend => {
                    if let Some(i) = e.glass.mend.take()
                        && !e.glass.intact(i as usize)
                    {
                        e.glass.nodes[i as usize].health = node_health(e) * 0.6;
                        e.glass.mends += 1;
                        sound(sounds, Cue::Mend, Some(e.glass.nodes[i as usize].pos), 0.8);
                    }
                }
                _ => {}
            }
        }
        if run.phase() == Phase::Active {
            match run.kind {
                Action::Shift => {
                    let to = flat(e.glass.shift_to - e.pos);
                    if to.length() > 0.2 {
                        move_ground(e, world, to.normalize_or_zero(), dt * 3.2);
                    }
                }
                Action::Lash => {
                    let d = flat(feet - e.pos);
                    let along = d.dot(e.lunge);
                    if !run.hit
                        && along > 0.0
                        && d.length() < LASH_RANGE
                        && d.cross(e.lunge).y.abs() < 1.2
                        && (feet.y - e.pos.y).abs() < 2.0
                        && line(world, e.center(), eye)
                    {
                        damage = 14.0;
                        run.hit = true;
                    }
                }
                _ => {}
            }
        }
        if run.phase() == Phase::Done {
            e.cooldown = 0.45;
            e.glass.mend = None;
        } else {
            e.glass.run = Some(run);
        }
        return (damage, (e.center() - eye).normalize_or_zero() * damage);
    }
    e.tell = 0.0;
    if e.cooldown > 0.0 || !e.visible {
        return (0.0, Vec3::ZERO);
    }
    let distance = flat(feet - e.pos).length();
    let toward = flat(feet - e.pos).normalize_or_zero();
    let broken = (0..e.glass.count as usize).find(|&i| !e.glass.intact(i));
    let intact = (0..e.glass.count as usize).filter(|&i| e.glass.intact(i)).count();
    // Commit direction and aim at the start of every action; they never track.
    e.lunge = toward;
    e.yaw = (-toward.x).atan2(-toward.z);
    if distance < LASH_RANGE - 0.3 {
        begin(e, Action::Lash, sounds);
    } else if let Some(i) = broken
        && e.glass.mends < MAX_MENDS
        && (intact * 2 <= e.glass.count as usize || e.attacks % 3 == 2)
    {
        let node = e.glass.nodes[i].pos;
        let ground = Vec3::new(node.x, e.pos.y, node.z);
        if flat(ground - e.pos).length() > 2.2 {
            let heading = e.navigation.heading(world, e.pos, ground, KIND, dt, e.slot, navigation_budget);
            move_ground(e, world, heading, dt * 1.6);
            e.yaw = (-heading.x).atan2(-heading.z);
        } else {
            let at = flat(node - e.pos).normalize_or_zero();
            e.yaw = (-at.x).atan2(-at.z);
            e.glass.mend = Some(i as u8);
            begin(e, Action::Mend, sounds);
        }
    } else if e.glass.bolt.is_none() && intact > 0 && (distance < 14.0 || e.attacks % 4 != 3) {
        e.aim = eye;
        begin(e, Action::Weave, sounds);
    } else if e.glass.bolt.is_none() {
        // Re-anchor between the lattice centre and the far side from the player.
        e.glass.shift_to = e.home - toward * 2.5;
        let to = flat(e.glass.shift_to - e.pos).normalize_or_zero();
        e.lunge = to;
        begin(e, Action::Shift, sounds);
    }
    (0.0, Vec3::ZERO)
}

mod lattice;
mod model;
pub(super) use lattice::{carry, route, strike_nodes};
pub(super) use model::model;

#[cfg(test)]
mod tests;
