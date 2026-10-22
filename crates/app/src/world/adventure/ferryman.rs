//! Connected-water movement, bounded real surges and a conserved cargo service.
//! Staged until the family/save/cargo/arena integration is complete.
use super::*;
use crate::world::BlockId as B;
use action::{Phase, Run};
use std::collections::{HashMap, VecDeque};
pub(super) const KIND: u32 = 20;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Action {
    Oar,
    Surge,
}
#[derive(Default)]
pub(super) struct State {
    pub run: Option<Run<Action>>,
    pub stranded: bool,
    pub carried: u32,
    pub destination: IVec3,
    pub volume_moved: u32,
    pub work: f64,
    pub cargo_resource: u32,
    pub cargo: u32,
    pub outbound: bool,
    pub trips: u32,
}
fn water_cell(world: &World, p: Vec3) -> Option<IVec3> {
    let s = world.voxel_scale.meters();
    let below = ((p - Vec3::Y * 0.1) / s).floor().as_ivec3();
    [below, below - IVec3::Y, below + IVec3::Y]
        .into_iter()
        .find(|q| world.water_volume(*q) >= 2)
}
fn surface(world: &World, p: IVec3) -> Vec3 {
    let s = world.voxel_scale.meters();
    (p.as_vec3() + Vec3::new(0.5, world.water_volume(p) as f32 / 8.0, 0.5)) * s
}
fn connected_step(world: &World, start: IVec3, goal: Vec3) -> Option<IVec3> {
    let mut queue = VecDeque::from([start]);
    let mut parents = HashMap::from([(start, start)]);
    let mut best = start;
    let mut score = surface(world, start).distance_squared(goal);
    while let Some(p) = queue.pop_front() {
        let next_score = surface(world, p).distance_squared(goal);
        if next_score < score {
            score = next_score;
            best = p;
        }
        if parents.len() >= 256 {
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
            let q = p + d;
            if parents.contains_key(&q) || world.water_volume(q) < 2 {
                continue;
            }
            let point = surface(world, q);
            if point.distance(surface(world, start)) > 12.0 || !free(world, point, KIND) {
                continue;
            }
            parents.insert(q, p);
            queue.push_back(q);
        }
    }
    if best == start {
        return None;
    }
    while parents[&best] != start {
        best = parents[&best];
    }
    Some(best)
}
fn discharge(e: &mut Enemy, world: &mut World, dirty: &mut Vec<IVec3>) {
    if e.ferryman.carried == 0 {
        return;
    }
    let q = e.ferryman.destination;
    if !matches!(world.get(q), B::Air | B::Water) {
        return;
    }
    let old = world.water_volume(q);
    let put = e.ferryman.carried.min((8 - old) as u32) as u8;
    world.set_water_volume(q, old + put);
    let actual = world.water_volume(q).saturating_sub(old) as u32;
    e.ferryman.carried -= actual.min(e.ferryman.carried);
    e.ferryman.volume_moved = e.ferryman.volume_moved.saturating_add(actual);
    dirty.push(q);
}
fn surge(e: &mut Enemy, world: &mut World, dirty: &mut Vec<IVec3>) {
    discharge(e, world, dirty);
    if e.ferryman.carried > 0 || !family_water_buffer_room(world, e) {
        return;
    }
    let Some(from) = water_cell(world, e.pos) else {
        return;
    };
    // A direction selects an adjacent voxel, independently of voxel size.
    let delta = if e.lunge.x.abs() > e.lunge.z.abs() {
        IVec3::X * e.lunge.x.signum() as i32
    } else {
        IVec3::Z * e.lunge.z.signum() as i32
    };
    if delta == IVec3::ZERO {
        return;
    }
    let to = from + delta;
    if !matches!(world.get(to), B::Air | B::Water) || !world.solid(to - IVec3::Y) {
        return;
    }
    let available = world.water_volume(from);
    let room = 8 - world.water_volume(to);
    let take = available.min(room).min(2);
    if take == 0 {
        return;
    }
    // Debit into the saved hull reservoir before attempting the destination.
    // A failed destination write leaves real water carried, never lost/refunded twice.
    world.set_water_volume(from, available - take);
    let removed = available.saturating_sub(world.water_volume(from)) as u32;
    e.ferryman.carried += removed;
    e.ferryman.destination = to;
    dirty.push(from);
    discharge(e, world, dirty);
}
pub(super) fn action_name(e: &Enemy) -> Option<&'static str> {
    Some(if e.ferryman.stranded {
        "Stranded — usable water removed"
    } else if e.ferryman.carried > 0 {
        "Holding water — outlet blocked"
    } else {
        match e.ferryman.run.as_ref().map(|r| (r.kind, r.phase())) {
            Some((_, Phase::Recover)) => "Hull exposed — recovering",
            Some((Action::Oar, _)) => "Oar sweep — retreat or flank",
            Some((Action::Surge, _)) => "Channel surge — leave the announced line",
            None if e.ferryman.cargo > 0 => "Carrying real cargo through the basin",
            None => "Following connected water",
        }
    })
}
pub(super) fn tick(
    e: &mut Enemy,
    world: &mut World,
    feet: Vec3,
    eye: Vec3,
    dt: f32,
    sounds: &mut Vec<Sound>,
    dirty: &mut Vec<IVec3>,
) -> (f32, Vec3) {
    discharge(e, world, dirty);
    let start = water_cell(world, e.pos);
    e.ferryman.stranded = start.is_none();
    if start.is_none() {
        navigation::gravity(e, world, dt);
    }
    let service = family_service(world, e);
    let near = e.visible && feet.distance(e.pos) < 22.0;
    if let Some(mut run) = e.ferryman.run.take() {
        if e.stagger > 0.35 && run.phase() == Phase::Prepare {
            run.interrupt();
        }
        run.advance(dt);
        e.tell = if run.phase() == Phase::Prepare {
            run.prepare - run.elapsed
        } else {
            0.0
        };
        let mut damage = 0.0;
        if run.phase() == Phase::Active {
            if run.kind == Action::Surge {
                e.ferryman.work += dt as f64;
                if e.ferryman.work >= 0.1 {
                    e.ferryman.work -= 0.1;
                    surge(e, world, dirty);
                }
            }
            let d = (feet - e.pos) * Vec3::new(1.0, 0.0, 1.0);
            let reach = if run.kind == Action::Oar { 3.2 } else { 4.5 };
            let narrow = if run.kind == Action::Oar { -0.1 } else { 0.8 };
            if !run.hit
                && d.length() < reach
                && d.normalize_or_zero().dot(e.lunge) > narrow
                && (feet.y - e.pos.y).abs() < 1.8
                && line(world, e.center(), eye)
            {
                damage = 22.0;
                run.hit = true;
            }
        }
        if run.phase() != Phase::Done {
            e.ferryman.run = Some(run);
        } else {
            e.cooldown = 0.6;
        }
        return (damage, (e.center() - eye).normalize_or_zero() * damage);
    }
    if service {
        e.ferryman.work += dt as f64;
        if e.ferryman.work >= 0.25 {
            e.ferryman.work -= 0.25;
            if let Some(owner) = e.habitat {
                if family_ferry_exchange(
                    world,
                    owner,
                    e.pos,
                    e.ferryman.outbound,
                    &mut e.ferryman.cargo_resource,
                    &mut e.ferryman.cargo,
                ) {
                    e.ferryman.outbound = !e.ferryman.outbound;
                    e.ferryman.trips = e.ferryman.trips.saturating_add(1);
                }
            }
        }
    }
    let close = near && feet.distance(e.pos) < 3.0;
    if e.cooldown <= 0.0 && (close || (near && !e.ferryman.stranded)) {
        let action = if close || e.ferryman.stranded {
            Action::Oar
        } else {
            Action::Surge
        };
        e.lunge = ((feet - e.pos) * Vec3::new(1.0, 0.0, 1.0)).normalize_or_zero();
        e.yaw = (-e.lunge.x).atan2(-e.lunge.z);
        e.attacks = e.attacks.wrapping_add(1);
        e.attack = 8;
        let prepare = if action == Action::Oar { 0.9 } else { 1.2 };
        e.tell = prepare;
        e.ferryman.run = Some(Run::new(
            action,
            e.attacks,
            prepare,
            if action == Action::Oar { 0.4 } else { 1.1 },
            1.1,
        ));
        sound(sounds, Cue::Warning, Some(e.center()), 0.7);
        return (0.0, Vec3::ZERO);
    }
    if let Some(start) = start {
        let goal = if near {
            feet
        } else if service {
            family_ferry_destination(world, e, e.ferryman.outbound)
        } else {
            e.home
        };
        if let Some(next) = connected_step(world, start, goal) {
            let target = surface(world, next);
            let delta = target - e.pos;
            let q = e.pos + delta.clamp_length_max(dt * 1.4);
            if free(world, q, KIND) && water_cell(world, q).is_some() {
                e.phase += e.pos.distance(q);
                e.pos = q;
            }
        }
    }
    (0.0, Vec3::ZERO)
}
pub(super) fn model(e: &Enemy, m: &mut Model, _time: f32) {
    let mark = m.mark();
    m.set_surface_kind(58.0);
    let local = |x, y, z| e.pos + Vec3::new(x, y, z);
    let hull = if e.flash > 0.0 {
        PALETTE_PAPER
    } else {
        [0.22, 0.3, 0.32]
    };
    m.cuboid(
        local(0.0, 0.3, 0.0),
        Vec3::new(1.5, 0.5, 2.3),
        Quat::IDENTITY,
        hull,
        0.0,
    );
    for side in [-1.0, 1.0] {
        for i in 0..5 {
            m.beam(
                local(side * 0.6, 0.4, i as f32 * 0.4 - 0.8),
                local(side * 0.45, 1.0, i as f32 * 0.4 - 0.8),
                0.1,
                [0.58, 0.57, 0.46],
                0.0,
            );
        }
    }
    m.cuboid(
        local(0.0, 1.2, -0.65),
        Vec3::new(0.6, 0.85, 0.2),
        Quat::IDENTITY,
        [0.72, 0.69, 0.57],
        0.0,
    );
    for x in [-0.15, 0.15] {
        m.cuboid(
            local(x, 1.35, -0.78),
            Vec3::new(0.1, 0.15, 0.05),
            Quat::IDENTITY,
            [0.18, 0.3, 0.36],
            0.15,
        );
    }
    let wind = e.ferryman.run.as_ref().map_or(0.0, |r| r.wind());
    m.beam(
        local(0.5, 1.1, 0.0),
        local(1.1, 0.2, -1.0 - wind * 0.6),
        0.1,
        [0.44, 0.3, 0.19],
        0.0,
    );
    m.cuboid(
        local(1.1, 0.2, -1.1 - wind * 0.6),
        Vec3::new(0.4, 0.1, 0.65),
        Quat::IDENTITY,
        [0.44, 0.3, 0.19],
        0.0,
    );
    if e.ferryman.cargo > 0 {
        m.cuboid(
            local(0.0, 0.7, 0.25),
            Vec3::splat(0.5),
            Quat::IDENTITY,
            [0.5, 0.36, 0.2],
            0.0,
        );
    }
    if e.ferryman.carried > 0 {
        m.cuboid(
            local(0.0, 0.65, -0.1),
            Vec3::splat(0.5),
            Quat::IDENTITY,
            PALETTE_WATER,
            0.0,
        );
    }
    m.bake_model_uv_since(mark, e.pos, 58.0);
    m.rotate_since(mark, e.pos, Quat::from_rotation_y(e.yaw));
    m.set_surface_kind(41.0);
    if e.ferryman
        .run
        .as_ref()
        .is_some_and(|r| r.kind == Action::Surge && r.phase() == Phase::Prepare)
    {
        m.beam(
            e.pos + Vec3::Y * 0.1,
            e.pos + e.lunge * 4.5 + Vec3::Y * 0.1,
            0.05,
            PALETTE_CORAL,
            0.1,
        );
    }
}
impl State {
    pub(super) fn save(&self, out: &mut Vec<u32>) {
        out.extend([
            self.carried,
            self.volume_moved,
            self.work.to_bits() as u32,
            (self.work.to_bits() >> 32) as u32,
            self.cargo_resource,
            self.cargo,
            self.outbound as u32,
            self.trips,
        ]);
        out.extend(self.destination.to_array().map(|v| v as u32));
    }
    pub(super) fn restore(&mut self, words: &[u32]) -> anyhow::Result<()> {
        anyhow::ensure!(
            words.len() == 11
                && words[0] <= 8
                && words[4] < (factory::RESOURCE_COUNT as u32)
                && words[5] <= factory::CAPACITY
                && words[6] <= 1,
            "Invalid Ferryman cargo"
        );
        let work = f64::from_bits(words[2] as u64 | ((words[3] as u64) << 32));
        anyhow::ensure!(
            work.is_finite() && (0.0..0.25).contains(&work),
            "Invalid Ferryman work clock"
        );
        self.carried = words[0];
        self.volume_moved = words[1];
        self.work = work;
        self.cargo_resource = words[4];
        self.cargo = words[5];
        self.outbound = words[6] != 0;
        self.trips = words[7];
        self.destination = IVec3::new(words[8] as i32, words[9] as i32, words[10] as i32);
        anyhow::ensure!(
            self.destination.as_vec3().abs().max_element() <= 4_000_000.0,
            "Invalid Ferryman destination"
        );
        Ok(())
    }
}
