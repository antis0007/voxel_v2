//! Bounded external anatomy. Ice and consumed water use the ordinary persisted world.
//! The pulse is magic, not a thermal simulation. No terrain is removed on disengage.
use super::*;
use crate::world::BlockId as B;
use std::collections::{HashSet, VecDeque};

const MAX_GROWTH: usize = 384;
const MAX_PULSE: usize = 768;
const NEIGHBORS: [IVec3; 6] = [
    IVec3::X,
    IVec3::NEG_X,
    IVec3::Y,
    IVec3::NEG_Y,
    IVec3::Z,
    IVec3::NEG_Z,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(super) enum Phase {
    #[default]
    Seek,
    Plant,
    Grow,
    Warn,
    Pulse,
    Vent,
}

#[derive(Default)]
pub(super) struct Crown {
    pub phase: Phase,
    pub clock: f32,
    pub root: IVec3,
    pub target: Vec3,
    pub planted: Vec3,
    plan: VecDeque<IVec3>,
    sources: Vec<IVec3>,
    frontier: Vec<IVec3>,
    visited: HashSet<IVec3>,
    hit: bool,
    pub grown: usize,
    pub severed: u32,
    pub cycles: u32,
    scale: f32,
}

fn center(p: IVec3, scale: f32) -> Vec3 {
    (p.as_vec3() + Vec3::splat(0.5)) * scale
}

/// At most 29*29*3 probes at the smallest supported voxel scale, once per decision.
fn water(world: &World, feet: Vec3) -> Vec<IVec3> {
    let s = world.voxel_scale.meters();
    let p = (feet / s).floor().as_ivec3();
    let radius = (3.5 / s).ceil() as i32;
    let mut out = Vec::new();
    for y in -2..=0 {
        for z in -radius..=radius {
            for x in -radius..=radius {
                let q = p + IVec3::new(x, y, z);
                if world.water_volume(q) == 8 && world.get(q + IVec3::Y) == B::Air {
                    out.push(q);
                }
            }
        }
    }
    out.sort_by_key(|q| ((center(*q, s) - feet).length_squared() * 1000.0) as i32);
    out.truncate(MAX_GROWTH);
    out
}

/// Resident torch indices and the bounded shared thermal lattice supply real heat cues.
/// Lexicographic tie breaking makes simultaneous equal lures deterministic.
fn heat(world: &World, root: Vec3) -> Option<Vec3> {
    let s = world.voxel_scale.meters();
    let mut best: Option<(f32, IVec3)> = None;
    let mut consider = |q: IVec3, strength: f32| {
        let p = center(q, s);
        let d = p - root;
        let range = d.length();
        if !(2.0..=13.0).contains(&range) || d.y.abs() > 2.5 {
            return;
        }
        // Trace up to the source's near face, not through its own solid voxel.
        let from = root + Vec3::Y * 0.7;
        let near = p - (p - from).normalize_or_zero() * (s * 0.9);
        if !line(world, from, near) {
            return;
        }
        let score = strength / (4.0 + range * range);
        if best.is_none_or(|(old, old_q)| {
            score > old || (score == old && (q.x, q.y, q.z) < (old_q.x, old_q.y, old_q.z))
        }) {
            best = Some((score, q));
        }
    };
    let c = crate::world::chunk_coord_from_world_block((root / s).floor().as_ivec3());
    let r = (13.0 / (crate::world::CHUNK_SIZE as f32 * s)).ceil() as i32;
    for z in -r..=r {
        for y in -1..=1 {
            for x in -r..=r {
                if let Some(chunk) = world.chunks.get(&(c + IVec3::new(x, y, z))) {
                    for &q in &chunk.torches {
                        if world.get(q).is_torch() {
                            consider(q, 240.0);
                        }
                    }
                }
            }
        }
    }
    for (&(x, y, z), cell) in &world.sandbox.cells {
        if cell.temperature >= 100.0 {
            consider(IVec3::new(x, y, z), cell.temperature.min(600.0));
        }
    }
    best.map(|(_, q)| center(q, s))
}

fn route(root: IVec3, end: IVec3, width: i32, out: &mut VecDeque<IVec3>) {
    let mut p = root;
    let across = if (end.x - root.x).abs() >= (end.z - root.z).abs() {
        IVec3::Z
    } else {
        IVec3::X
    };
    let mut known: HashSet<_> = out.iter().copied().collect();
    for _ in 0..128 {
        for w in -(width / 2)..=(width - 1) / 2 {
            let q = p + across * w;
            if known.insert(q) && out.len() < MAX_GROWTH {
                out.push_back(q);
            }
        }
        if p == end || out.len() >= MAX_GROWTH {
            break;
        }
        let d = end - p;
        // Face-adjacent cells, including on rises: no diagonal gaps in the attack graph.
        let axis = if d.x.abs() >= d.z.abs() && d.x != 0 {
            0
        } else if d.z != 0 {
            2
        } else {
            1
        };
        p[axis] += d[axis].signum();
    }
}

fn begin(e: &mut Enemy, world: &World) -> bool {
    let s = world.voxel_scale.meters();
    let candidates = water(world, e.pos);
    let mut tried: Vec<Vec3> = Vec::new();
    let mut contact = None;
    for &root in &candidates {
        let p = center(root, s);
        let d = p - e.pos;
        let horizontal = d * Vec3::new(1.0, 0.0, 1.0);
        if !(1.5..=3.2).contains(&horizontal.length())
            || tried.iter().any(|q| (*q - p).length() < 0.8)
        {
            continue;
        }
        tried.push(p);
        if let Some(target) = heat(world, p) {
            contact = Some((root, target));
            break;
        }
        if tried.len() >= 8 {
            break;
        }
    }
    let Some((root, target)) = contact else {
        if std::env::var_os("VOXEL_HART_CROWN_REVIEW").is_some() {
            println!(
                "[hart-crown] no_contact pos={:?} water_candidates={} tried={}",
                e.pos,
                candidates.len(),
                tried.len()
            );
        }
        return false;
    };
    // Only draw from this connected, initially wet pool. A nearby sealed tank or
    // unrelated puddle does not become a remote source of free anatomy.
    let available: HashSet<_> = candidates.into_iter().collect();
    let mut seen = HashSet::from([root]);
    let mut queue = VecDeque::from([root]);
    let mut sources = Vec::new();
    while let Some(q) = queue.pop_front() {
        sources.push(q);
        for d in NEIGHBORS {
            let p = q + d;
            if available.contains(&p) && seen.insert(p) {
                queue.push_back(p);
            }
        }
    }
    let end = (target / s).floor().as_ivec3().with_y(root.y + 1);
    let mut plan = VecDeque::new();
    plan.push_back(root);
    route(root + IVec3::Y, end, (1.0 / s).ceil() as i32, &mut plan);
    // A short second tine shares a real junction, rather than an invisible graph edge.
    let fork = plan.get(plan.len() / 2).copied().unwrap_or(root + IVec3::Y);
    let side = if (end.x - root.x).abs() >= (end.z - root.z).abs() {
        IVec3::Z
    } else {
        IVec3::X
    };
    route(fork, fork + side * (2.0 / s) as i32, 1, &mut plan);
    let old = &e.hart.crown;
    e.hart.crown = Crown {
        phase: Phase::Plant,
        clock: 1.6,
        root,
        target,
        planted: e.pos,
        sources,
        plan,
        scale: s,
        grown: old.grown,
        severed: old.severed,
        cycles: old.cycles + 1,
        ..Default::default()
    };
    e.lunge = ((center(root, s) - e.pos) * Vec3::new(1.0, 0.0, 1.0)).normalize_or_zero();
    println!(
        "[hart-crown] commit root={root:?} target={target:?} water_cells={} cycle={}",
        e.hart.crown.sources.len(),
        e.hart.crown.cycles
    );
    true
}

/// Runs before the supporting melee AI. True holds the Hart in a planted, interruptible action.
pub(super) fn tick(
    e: &mut Enemy,
    world: &mut World,
    feet: Vec3,
    eye: Vec3,
    dt: f32,
    dirty: &mut Vec<IVec3>,
) -> (bool, f32) {
    let c = &mut e.hart.crown;
    if c.phase == Phase::Plant {
        let desired = (-e.lunge.x).atan2(-e.lunge.z);
        let turn = (desired - e.yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
        e.yaw += turn.clamp(-dt * 3.0, dt * 3.0);
    }
    c.clock -= dt;
    if c.phase != Phase::Seek && !factory::solid_point(world, e.pos - Vec3::Y * 0.18) {
        c.phase = Phase::Seek;
        c.clock = 3.0;
        c.frontier.clear();
        return (false, 0.0);
    }
    if c.phase == Phase::Seek {
        if c.clock <= 0.0
            && e.tell <= 0.0
            && e.hart.active <= 0.0
            // The shared melee tick decrements recovery AFTER this controller.
            // Admit its ending step here, or melee consumes every future action
            // boundary and the crown can only ever plant once after spawning.
            && e.hart.recovery <= dt
            && e.cooldown <= 0.0
            && e.visible
        {
            e.hart.crown.clock = 3.0;
            if begin(e, world) {
                e.hart.follow = None;
                e.hart.moving = 0.0;
                return (true, 0.0);
            }
        }
        return (false, 0.0);
    }
    let c = &mut e.hart.crown;
    if (e.stagger > 0.3 && e.poise > 2.65) || (e.pos - c.planted).length() > 0.35 {
        c.phase = Phase::Vent;
        c.clock = 2.5;
        c.frontier.clear();
    }
    match c.phase {
        Phase::Plant if c.clock <= 0.0 => {
            c.phase = Phase::Grow;
            c.clock = 0.0;
        }
        Phase::Grow if c.clock <= 0.0 => {
            c.clock = c.scale / 1.5;
            // One metre-wide row per work tick, advancing at 1.5 m/s. At most
            // four transfers at quarter scale; no catch-up burst after a stall.
            for _ in 0..(1.0 / c.scale).ceil() as usize {
                let Some(q) = c.plan.pop_front() else {
                    c.phase = Phase::Warn;
                    c.clock = 1.8;
                    break;
                };
                let block = world.get(q);
                if block == B::Ice {
                    continue;
                }
                // A partial destination cannot be overwritten with a full ice cell:
                // doing so would silently destroy its existing water on top of the debit.
                if block == B::Water && world.water_volume(q) != 8 {
                    c.plan.clear();
                    c.phase = Phase::Warn;
                    c.clock = 1.8;
                    break;
                }
                if block != B::Air && block != B::Water {
                    c.plan.clear();
                    c.phase = Phase::Warn;
                    c.clock = 1.8;
                    break;
                }
                // Never materialize geometry inside the player or a machine assembly.
                let p = center(q, c.scale);
                if ((p - feet) * Vec3::new(1.0, 0.0, 1.0)).length() < 0.65
                    && p.y > feet.y - 0.15
                    && p.y < eye.y + 0.3
                {
                    c.phase = Phase::Warn;
                    c.clock = 1.8;
                    c.plan.clear();
                    break;
                }
                if dirty.len() + 2 > 2048 {
                    break;
                }
                let source = if block == B::Water && world.water_volume(q) == 8 {
                    Some(q)
                } else {
                    c.sources
                        .iter()
                        .rev()
                        .copied()
                        .find(|p| world.water_volume(*p) == 8)
                };
                let Some(source) = source else {
                    c.plan.clear();
                    c.phase = Phase::Warn;
                    c.clock = 1.8;
                    break;
                };
                // Both addresses must be admitted before withdrawing material. Factory-owned
                // cells are excluded, and failed destination writes refund the exact full cell.
                let owns = |p| {
                    world
                        .factory
                        .nodes
                        .contains_key(&factory::anchor(p, factory::step(world)))
                };
                if owns(q) || owns(source) {
                    c.phase = Phase::Vent;
                    c.clock = 2.5;
                    break;
                }
                if source != q {
                    world.set_water_volume(source, 0);
                    if world.water_volume(source) != 0 {
                        break;
                    }
                }
                world.set(q, B::Ice);
                if world.get(q) != B::Ice {
                    if source != q {
                        world.set_water_volume(source, 8);
                    }
                    c.phase = Phase::Vent;
                    c.clock = 2.5;
                    break;
                }
                dirty.push(source);
                dirty.push(q);
                c.grown += 1;
                world.activity.wake(q);
                world.activity.wake(source);
            }
        }
        Phase::Warn if c.clock <= 0.0 => {
            // Only a stationary, visibly planted crown can inject a NEW pulse.
            if world.get(c.root) == B::Ice && (e.pos - c.planted).length() < 0.35 {
                c.frontier = vec![c.root];
                c.visited.insert(c.root);
                c.hit = false;
                c.phase = Phase::Pulse;
                c.clock = 0.0;
                println!("[hart-crown] pulse launch grown={}", c.grown);
            } else {
                c.phase = Phase::Vent;
                c.clock = 2.5;
            }
        }
        Phase::Pulse if c.clock <= 0.0 => {
            c.clock = 0.18;
            let mut next = Vec::new();
            for p in c.frontier.drain(..) {
                if world.get(p) != B::Ice {
                    c.severed += 1;
                    continue;
                }
                for d in NEIGHBORS {
                    let q = p + d;
                    if c.visited.len() < MAX_PULSE
                        && next.len() < 64
                        && world.get(q) == B::Ice
                        && c.visited.insert(q)
                    {
                        next.push(q);
                    }
                }
            }
            c.frontier = next;
            if c.frontier.is_empty() || c.visited.len() >= MAX_PULSE {
                println!(
                    "[hart-crown] pulse ended visited={} cut_fronts={}",
                    c.visited.len(),
                    c.severed
                );
                c.phase = Phase::Vent;
                c.clock = 2.5;
            }
        }
        Phase::Vent if c.clock <= 0.0 => {
            c.phase = Phase::Seek;
            c.clock = 5.0;
            c.frontier.clear();
            // Deliberate physical advance follows the interference; no instant replant.
            e.cooldown = 1.4;
            e.hart.recovery = 0.0;
        }
        _ => {}
    }
    let mut damage = 0.0;
    if c.phase == Phase::Pulse && !c.hit {
        for &q in &c.frontier {
            let p = center(q, c.scale) + Vec3::Y * (c.scale * 0.5);
            let d = feet - p;
            if world.get(q) == B::Ice
                && (d * Vec3::new(1.0, 0.0, 1.0)).length() < 0.75
                && d.y >= -0.3
                && d.y < 1.8
                && line(world, p + Vec3::Y * 0.15, eye)
            {
                damage = 18.0;
                c.hit = true;
                break;
            }
        }
    }
    (c.phase != Phase::Seek, damage)
}

pub(super) fn draw(e: &Enemy, m: &mut Model) {
    let c = &e.hart.crown;
    if c.phase == Phase::Seek {
        return;
    }
    let root = center(c.root, c.scale) + Vec3::Y * (c.scale * 0.5);
    if matches!(c.phase, Phase::Grow | Phase::Warn | Phase::Pulse)
        || (c.phase == Phase::Plant && c.clock < 0.4)
    {
        // Visible descending root of the planted crown, never a hidden remote tether.
        let crown = e.pos + e.lunge * 1.48 + Vec3::Y * 2.6;
        m.beam(crown, root, 0.10, [0.63, 0.67, 0.65], 0.0);
    }
    if matches!(c.phase, Phase::Plant | Phase::Grow | Phase::Warn) {
        let front = c
            .plan
            .front()
            .copied()
            .map(|p| center(p, c.scale))
            .unwrap_or(root);
        for d in [Vec3::X, Vec3::Z] {
            m.beam(
                front - d * 0.28 + Vec3::Y * 0.55,
                front + d * 0.28 + Vec3::Y * 0.55,
                0.025,
                [0.65, 0.82, 0.88],
                0.7,
            );
        }
        // A quiet amber bracket identifies the committed source, without a room-wide beam.
        m.beam(
            c.target + Vec3::Y * 0.35,
            c.target + Vec3::Y * 0.8,
            0.035,
            [0.85, 0.46, 0.12],
            0.8,
        );
    }
    for &q in &c.frontier {
        let p = center(q, c.scale) + Vec3::Y * (c.scale * 0.5 + 0.035);
        m.cuboid(
            p,
            Vec3::new(c.scale * 0.85, 0.07, c.scale * 0.85),
            Quat::IDENTITY,
            [0.67, 0.90, 1.0],
            1.0,
        );
    }
}
