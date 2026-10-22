//! A small editable routing instrument. Each pulse owns one paid Spark.
//! Staged until the family/save/interaction/arena integration is complete.
use super::*;
use crate::world::BlockId as B;
use action::{Phase, Run};
pub(super) const KIND: u32 = 21;
const MAX_PACKETS: usize = 8;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Action {
    Scissor,
    Inject,
}
pub(super) struct Packet {
    pub pos: Vec3,
    pub node: usize,
    pub hops: u32,
    pub life: f32,
    pub hostile: bool,
}
#[derive(Default)]
pub(super) struct State {
    pub run: Option<Run<Action>>,
    pub routes: [u32; 4],
    pub packets: Vec<Packet>,
    pub silk: f64,
    pub repair: f64,
    pub routed: u32,
    pub cut: u32,
    pub intact: u32,
}
fn nodes(home: Vec3) -> [Vec3; 4] {
    [
        home + Vec3::new(-3.0, 1.2, -2.0),
        home + Vec3::new(3.0, 1.2, -2.0),
        home + Vec3::new(3.0, 1.2, 3.0),
        home + Vec3::new(-3.0, 1.2, 3.0),
    ]
}
fn anchor(world: &World, p: Vec3) -> IVec3 {
    (p / world.voxel_scale.meters()).floor().as_ivec3()
}
fn alive(world: &World, p: Vec3) -> bool {
    world.get(anchor(world, p)) == B::Glass
}
fn segment(world: &World, a: Vec3, b: Vec3, lens: Vec3) -> bool {
    let old = anchor(world, a);
    let end = anchor(world, lens);
    let mut start = a;
    let step = (b - a).normalize_or_zero() * world.voxel_scale.meters() * 0.25;
    for _ in 0..8 {
        let Some((_, cell)) = trace::solid(world, start, b) else {
            return true;
        };
        if cell == end {
            return true;
        }
        if cell != old || world.get(cell) != B::Glass {
            return false;
        }
        if start.distance_squared(b) <= step.length_squared() {
            return true;
        }
        start += step;
    }
    false
}
fn advance_packets(e: &mut Enemy, world: &World, dt: f32, shots: &mut Vec<Projectile>) {
    let nodes = nodes(e.home);
    let mut packets = std::mem::take(&mut e.glassweaver.packets);
    packets.retain_mut(|p| {
        p.life -= dt;
        if p.life <= 0.0 || !alive(world, nodes[p.node]) {
            e.glassweaver.cut = e.glassweaver.cut.saturating_add(1);
            return false;
        }
        let target = nodes[p.node];
        let q = p.pos + (target - p.pos).clamp_length_max(dt * 8.0);
        // The lens occupies its physical Glass voxel; every other obstruction
        // absorbs the single packet. Cutting an anchor never recreates it.
        if !segment(world, p.pos, q, target) {
            e.glassweaver.cut = e.glassweaver.cut.saturating_add(1);
            return false;
        }
        p.pos = q;
        if q.distance_squared(target) > 0.01 {
            return true;
        }
        e.glassweaver.routed = e.glassweaver.routed.saturating_add(1);
        if p.hops == 0 || e.glassweaver.routes[p.node] == 4 {
            if shots.len() >= 96 {
                return true;
            }
            // Exit through the outward-facing lens. Release consumes this
            // packet; a bounded loop cannot create another paid payload.
            let direction = ((target - e.home) * Vec3::new(1.0, 0.0, 1.0)).normalize_or_zero();
            family_bolt(
                shots,
                target + direction * 0.5,
                target + direction * 10.0,
                0.0,
                12.0,
                1.0,
            );
            if let Some(shot) = shots.last_mut() {
                shot.hostile = p.hostile;
            }
            return false;
        }
        p.hops -= 1;
        p.node = e.glassweaver.routes[p.node] as usize;
        true
    });
    e.glassweaver.packets = packets;
}
/// A local player action rotates the physical next connection. A packet already
/// crossing a segment keeps its destination until it reaches that lens.
pub(super) fn turn(e: &mut Enemy, world: &World, player: Vec3, node: usize) -> bool {
    if node >= 4 {
        return false;
    }
    let p = nodes(e.home)[node];
    if player.distance(p) > 3.5 || !alive(world, p) {
        return false;
    }
    e.glassweaver.routes[node] = (e.glassweaver.routes[node] + 1) % 5;
    true
}
pub(super) fn inject(e: &mut Enemy, world: &World, from: Vec3, hostile: bool) -> bool {
    if e.glassweaver.packets.len() >= MAX_PACKETS {
        return false;
    }
    let ns = nodes(e.home);
    let next = (0..4)
        .filter(|&i| alive(world, ns[i]) && line(world, from, ns[i] - Vec3::Y * 0.25))
        .min_by(|&a, &b| {
            from.distance_squared(ns[a])
                .total_cmp(&from.distance_squared(ns[b]))
        });
    let Some(node) = next else {
        return false;
    };
    e.glassweaver.packets.push(Packet {
        pos: from,
        node,
        hops: 6,
        life: 8.0,
        hostile,
    });
    true
}
pub(super) fn action_name(e: &Enemy) -> Option<&'static str> {
    Some(
        match e.glassweaver.run.as_ref().map(|r| (r.kind, r.phase())) {
            Some((_, Phase::Recover)) => "Forelegs exposed — recovering",
            Some((Action::Scissor, _)) => "Scissor sweep — flank or retreat",
            Some((Action::Inject, _)) => "Charging the first lens — cut or redirect the route",
            None if !e.glassweaver.packets.is_empty() => "One paid pulse crossing the instrument",
            None => "Maintaining the lenses",
        },
    )
}
pub(super) fn tick(
    e: &mut Enemy,
    world: &mut World,
    feet: Vec3,
    eye: Vec3,
    dt: f32,
    shots: &mut Vec<Projectile>,
    sounds: &mut Vec<Sound>,
    dirty: &mut Vec<IVec3>,
) -> (f32, Vec3) {
    navigation::gravity(e, world, dt);
    e.glassweaver.intact = nodes(e.home)
        .into_iter()
        .enumerate()
        .fold(0, |mask, (i, p)| mask | ((alive(world, p) as u32) << i));
    advance_packets(e, world, dt, shots);
    let service = family_service(world, e);
    let near = e.visible && feet.distance(e.pos) < 24.0;
    if near || service {
        e.glassweaver.silk = (e.glassweaver.silk + dt as f64).min(24.0);
        if e.glassweaver.silk >= 24.0
            && family_product_room(world, e, 64)
            && family_feed(world, e, 64)
        {
            family_product(world, e, 64);
            e.glassweaver.silk = 0.0;
        }
        e.glassweaver.repair = (e.glassweaver.repair + dt as f64).min(8.0);
        if e.glassweaver.repair >= 8.0 {
            // Replacement requires supplied crystal, an empty voxel and support.
            // Never overwrite a player's wall to restore a prescribed fight.
            for p in nodes(e.home) {
                let q = anchor(world, p);
                if world.get(q) == B::Air
                    && world.solid(q - IVec3::Y)
                    && family_weaver_repair(world, e, q)
                {
                    dirty.push(q);
                    e.glassweaver.repair = 0.0;
                    break;
                }
            }
        }
    }
    if let Some(mut run) = e.glassweaver.run.take() {
        if e.stagger > 0.35 && run.phase() == Phase::Prepare {
            run.interrupt();
        }
        let release = run.advance(dt);
        e.tell = if run.phase() == Phase::Prepare {
            run.prepare - run.elapsed
        } else {
            0.0
        };
        if release && run.kind == Action::Inject && e.glassweaver.packets.len() < MAX_PACKETS {
            // Reserve mana only after a usable physical input lens was found.
            if family_can_mana(world, e, 12) && inject(e, world, e.center(), true) {
                family_mana(world, e, 12);
            }
        }
        let mut damage = 0.0;
        let d = (feet - e.pos) * Vec3::new(1.0, 0.0, 1.0);
        if run.kind == Action::Scissor
            && run.phase() == Phase::Active
            && !run.hit
            && d.length() < 3.2
            && e.lunge.dot(d.normalize_or_zero()) > -0.25
            && (feet.y - e.pos.y).abs() < 1.8
            && line(world, e.center(), eye)
        {
            damage = 21.0;
            run.hit = true;
        }
        if run.phase() != Phase::Done {
            e.glassweaver.run = Some(run);
        } else {
            e.cooldown = 0.5;
        }
        return (damage, (e.center() - eye).normalize_or_zero() * damage);
    }
    e.tell = 0.0;
    if e.cooldown > 0.0 || (!near && !service) {
        return (0.0, Vec3::ZERO);
    }
    let kind = if near && feet.distance(e.pos) < 3.4 {
        Action::Scissor
    } else {
        Action::Inject
    };
    if kind == Action::Inject && !family_can_mana(world, e, 12) {
        return (0.0, Vec3::ZERO);
    }
    e.lunge = ((feet - e.pos) * Vec3::new(1.0, 0.0, 1.0)).normalize_or_zero();
    e.attacks = e.attacks.wrapping_add(1);
    e.attack = 8;
    let prepare = if kind == Action::Scissor { 0.85 } else { 1.2 };
    e.glassweaver.run = Some(Run::new(kind, e.attacks, prepare, 0.35, 1.0));
    e.tell = prepare;
    sound(sounds, Cue::Warning, Some(e.center()), 0.7);
    (0.0, Vec3::ZERO)
}
pub(super) fn model(e: &Enemy, m: &mut Model, _time: f32) {
    let mark = m.mark();
    m.set_surface_kind(58.0);
    let color = if e.flash > 0.0 {
        PALETTE_PAPER
    } else {
        [0.38, 0.31, 0.47]
    };
    m.cuboid(
        e.pos + Vec3::Y * 0.8,
        Vec3::new(1.2, 0.9, 1.5),
        Quat::IDENTITY,
        color,
        0.0,
    );
    for side in [-1.0, 1.0] {
        for i in 0..3 {
            let z = i as f32 * 0.65 - 0.7;
            let a = e.pos + Vec3::new(side * 0.5, 0.85, z);
            let b = e.pos + Vec3::new(side * 1.7, 1.05, z + 0.15);
            m.beam(a, b, 0.13, color, 0.0);
            m.beam(
                b,
                e.pos + Vec3::new(side * 1.6, 0.1, z + 0.4),
                0.1,
                color,
                0.0,
            );
        }
    }
    let ns = nodes(e.home);
    for (i, p) in ns.into_iter().enumerate() {
        if e.glassweaver.intact & (1 << i) == 0 {
            continue;
        }
        m.cuboid(p, Vec3::splat(0.14), Quat::IDENTITY, PALETTE_MANA, 0.4);
        let next = e.glassweaver.routes[i] as usize;
        if next < 4 && next != i && e.glassweaver.intact & (1 << next) != 0 {
            m.beam(p, ns[next], 0.025, PALETTE_MANA, 0.1);
        }
    }
    for p in &e.glassweaver.packets {
        m.cuboid(p.pos, Vec3::splat(0.12), Quat::IDENTITY, PALETTE_CORAL, 0.8);
    }
    m.bake_model_uv_since(mark, e.pos, 58.0);
    m.set_surface_kind(41.0);
}
impl State {
    pub(super) fn save(&self, out: &mut Vec<u32>) {
        out.extend(self.routes);
        out.extend([
            self.silk.to_bits() as u32,
            (self.silk.to_bits() >> 32) as u32,
            self.repair.to_bits() as u32,
            (self.repair.to_bits() >> 32) as u32,
            self.routed,
            self.cut,
            self.packets.len() as u32,
        ]);
        for p in &self.packets {
            out.extend(p.pos.to_array().map(f32::to_bits));
            out.extend([p.node as u32, p.hops, p.life.to_bits(), p.hostile as u32]);
        }
    }
    pub(super) fn restore(&mut self, words: &[u32]) -> anyhow::Result<()> {
        anyhow::ensure!(
            words.len() >= 11 && words[..4].iter().all(|r| *r <= 4),
            "Invalid Glassweaver routes"
        );
        let count = words[10] as usize;
        anyhow::ensure!(
            count <= MAX_PACKETS && words.len() == 11 + count * 7,
            "Invalid Glassweaver packets"
        );
        let clock = |i: usize| f64::from_bits(words[i] as u64 | ((words[i + 1] as u64) << 32));
        let silk = clock(4);
        let repair = clock(6);
        anyhow::ensure!(
            silk.is_finite()
                && (0.0..=24.0).contains(&silk)
                && repair.is_finite()
                && (0.0..=8.0).contains(&repair),
            "Invalid Glassweaver clocks"
        );
        let mut packets = Vec::with_capacity(count);
        for p in words[11..].chunks_exact(7) {
            let pos = Vec3::new(
                f32::from_bits(p[0]),
                f32::from_bits(p[1]),
                f32::from_bits(p[2]),
            );
            let life = f32::from_bits(p[5]);
            anyhow::ensure!(
                pos.is_finite()
                    && pos.abs().max_element() <= 1_000_000.0
                    && p[3] < 4
                    && p[4] <= 6
                    && life.is_finite()
                    && life > 0.0
                    && life <= 8.0
                    && p[6] <= 1,
                "Invalid paid Glassweaver packet"
            );
            packets.push(Packet {
                pos,
                node: p[3] as usize,
                hops: p[4],
                life,
                hostile: p[6] != 0,
            });
        }
        self.routes.copy_from_slice(&words[..4]);
        self.silk = silk;
        self.repair = repair;
        self.routed = words[8];
        self.cut = words[9];
        self.packets = packets;
        Ok(())
    }
}
