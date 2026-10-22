//! Grounded duel and finite, editable cover. Gate voxels have exactly one owner.
use super::*;
use crate::world::BlockId as B;
use action::{Phase, Run};

pub(super) const KIND: u32 = 14;
const MAX_GATE: usize = 256;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Action {
    Shove,
    Thrust,
    Sweep,
    Plant,
    Retrieve,
}
#[derive(Clone, Default)]
pub(super) struct State {
    pub run: Option<Run<Action>>,
    /// Owned timber cells in the carried gate. Zero means planted or destroyed.
    stock: u32,
    initialized: bool,
    /// Only cells actually placed by this gate may be reclaimed.
    cells: Vec<IVec3>,
    anchor: Vec3,
    planted_for: f32,
    pressure: f32,
    enraged: bool,
    follow: bool,
    pub contacts: u32,
    pub plants: u32,
    pub retrieved: u32,
}
#[derive(Clone)]
pub(super) struct Saved {
    pos: Vec3,
    home: Vec3,
    health: f32,
    yaw: f32,
    state: State,
}

fn horizontal(v: Vec3) -> Vec3 {
    v * Vec3::new(1.0, 0.0, 1.0)
}
fn facing(e: &Enemy) -> Vec3 {
    Vec3::new(-e.yaw.sin(), 0.0, -e.yaw.cos())
}
fn gate_shape(scale: f32) -> Vec<IVec3> {
    let width = (3.0 / scale).ceil() as i32;
    let height = (2.5 / scale).ceil() as i32;
    let mut out = Vec::new();
    for y in 0..height {
        for x in 0..width {
            let px = (x as f32 + 0.5) * scale - 1.5;
            let py = (y as f32 + 0.5) * scale;
            // A genuine sight/shooting gap, broad enough to read at ordinary distance.
            if px.abs() < 0.55 && (1.0..2.0).contains(&py) {
                continue;
            }
            out.push(IVec3::new(x - width / 2, y, 0));
        }
    }
    out
}
fn initialize(e: &mut Enemy, scale: f32) {
    if !e.toll.initialized {
        e.toll.initialized = true;
        e.toll.stock = gate_shape(scale).len() as u32;
    }
}
pub(super) fn guarded(e: &Enemy, incoming: Vec3) -> bool {
    e.kind == KIND
        && e.toll.stock > 0
        && incoming.normalize_or_zero().dot(facing(e)) < -0.35
        && e.toll
            .run
            .as_ref()
            .is_none_or(|r| r.phase() == Phase::Prepare)
}
pub(super) fn pressure(e: &mut Enemy, amount: f32) {
    e.toll.pressure = (e.toll.pressure + amount).min(100.0);
}
fn move_ground(e: &mut Enemy, world: &World, direction: Vec3, distance: f32) {
    let steps = (distance / 0.1).ceil().max(1.0) as usize;
    for _ in 0..steps {
        let q = e.pos + direction * distance / steps as f32;
        let mut accepted = None;
        for rise in [0.0, 0.5] {
            let at = q + Vec3::Y * rise;
            if free(world, at, KIND) && factory::solid_point(world, at - Vec3::Y * 0.2) {
                accepted = Some(at);
                break;
            }
        }
        if let Some(at) = accepted {
            e.phase += horizontal(at - e.pos).length();
            e.pos = at;
        } else {
            break;
        }
    }
}
fn plant(e: &mut Enemy, world: &mut World, feet: Vec3, eye: Vec3, dirty: &mut Vec<IVec3>) -> bool {
    let scale = world.voxel_scale.meters();
    let forward = facing(e);
    let across = if forward.z.abs() > forward.x.abs() {
        IVec3::X
    } else {
        IVec3::Z
    };
    // Plant beside the body, so the keeper can leave by either side.
    let base = ((e.pos + forward * 1.3) / scale).floor().as_ivec3();
    let cells: Vec<_> = gate_shape(scale)
        .into_iter()
        .map(|p| base + across * p.x + IVec3::Y * p.y)
        .collect();
    if e.toll.stock == 0 || cells.len() > MAX_GATE || dirty.len() + cells.len() > 2048 {
        return false;
    }
    let cells: Vec<_> = cells.into_iter().take(e.toll.stock as usize).collect();
    for &q in &cells {
        let p = (q.as_vec3() + Vec3::splat(0.5)) * scale;
        if world.get(q) != B::Air
            || world
                .factory
                .nodes
                .contains_key(&factory::anchor(q, factory::step(world)))
            || (horizontal(p - feet).length() < 0.7 && p.y > feet.y - 0.2 && p.y < eye.y + 0.4)
            || (horizontal(p - e.pos).length() < 0.8 && p.y < e.pos.y + 2.6)
        {
            return false;
        }
        if q.y == base.y && !world.solid(q - IVec3::Y) {
            return false;
        }
    }
    world.set_many(&cells.iter().map(|&q| (q, B::Planks)).collect::<Vec<_>>());
    // Region admission can decline writes. Debit only published cells.
    let cells: Vec<_> = cells
        .into_iter()
        .filter(|&q| world.get(q) == B::Planks)
        .collect();
    if cells.is_empty() {
        return false;
    }
    e.toll.stock -= cells.len() as u32;
    e.toll.anchor = (base.as_vec3() + Vec3::splat(0.5)) * scale;
    dirty.extend(cells.iter().copied());
    e.toll.cells = cells;
    e.toll.planted_for = 0.0;
    e.toll.plants += 1;
    true
}
fn retrieve(e: &mut Enemy, world: &mut World, dirty: &mut Vec<IVec3>) {
    if horizontal(e.pos - e.toll.anchor).length() > 3.0 || dirty.len() + e.toll.cells.len() > 2048 {
        return;
    }
    // Missing/changed cells were already mined or destroyed; never replace that stock.
    let remaining: Vec<_> = e
        .toll
        .cells
        .iter()
        .copied()
        .filter(|&q| world.get(q) == B::Planks)
        .collect();
    world.set_many(&remaining.iter().map(|&q| (q, B::Air)).collect::<Vec<_>>());
    e.toll.cells.clear();
    for q in remaining {
        if world.get(q) == B::Air {
            e.toll.stock += 1;
            e.toll.retrieved += 1;
            dirty.push(q);
        } else {
            e.toll.cells.push(q);
        }
    }
}
fn begin(e: &mut Enemy, kind: Action, target: Vec3, sounds: &mut Vec<Sound>) {
    let (prepare, active, recover) = match kind {
        Action::Shove => (0.7, 0.2, 0.95),
        Action::Thrust => (1.0, 0.4, 1.25),
        Action::Sweep => (1.15, 0.45, 1.4),
        Action::Plant => (1.25, 0.1, 0.8),
        Action::Retrieve => (1.45, 0.1, 1.25),
    };
    e.aim = target;
    e.lunge = horizontal(target - e.pos).normalize_or_zero();
    if e.lunge != Vec3::ZERO {
        e.yaw = (-e.lunge.x).atan2(-e.lunge.z);
    }
    e.attacks = e.attacks.wrapping_add(1);
    e.toll.run = Some(Run::new(kind, e.attacks, prepare, active, recover));
    e.attack = 8; // Pose and contact own their danger shape; no generic radial marker.
    e.tell = prepare;
    sound(sounds, Cue::ForgeWindup, Some(e.center()), 0.7);
    println!(
        "[tollkeeper] action={kind:?} sequence={} target={target:?}",
        e.attacks
    );
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
    initialize(e, world.voxel_scale.meters());
    e.toll.planted_for += dt;
    // No healing, resetting cover, or teleporting on disengagement.
    if (feet - e.home).length() > 48.0 {
        e.toll.run = None;
        e.tell = 0.0;
        e.cooldown = 2.0;
        return (0.0, Vec3::ZERO);
    }
    let mut damage = 0.0;
    if let Some(mut run) = e.toll.run.take() {
        if e.stagger > 0.3 && matches!(run.phase(), Phase::Prepare | Phase::Active) {
            run.interrupt();
            e.toll.follow = false;
        }
        let release = run.advance(dt);
        e.tell = if run.phase() == Phase::Prepare {
            run.prepare - run.elapsed
        } else {
            0.0
        };
        if release {
            match run.kind {
                Action::Plant => {
                    plant(e, world, feet, eye, dirty);
                    e.toll.pressure = 0.0;
                }
                Action::Retrieve => retrieve(e, world, dirty),
                _ => {}
            }
            sound(sounds, Cue::Swing, Some(e.center()), 0.85);
        }
        if run.phase() == Phase::Active {
            if run.kind == Action::Thrust {
                move_ground(e, world, e.lunge, dt * 3.3);
            }
            let d = horizontal(feet - e.pos);
            let along = d.dot(e.lunge);
            let side = d.cross(e.lunge).y.abs();
            let contact = match run.kind {
                Action::Thrust => along > 0.0 && along < 4.5 && side < 0.65,
                Action::Sweep => {
                    let progress = ((run.elapsed - run.prepare) / run.active).clamp(0.0, 1.0);
                    let previous = ((run.elapsed - dt - run.prepare) / run.active).clamp(0.0, 1.0);
                    let angle = e
                        .lunge
                        .cross(d.normalize_or_zero())
                        .y
                        .atan2(e.lunge.dot(d.normalize_or_zero()));
                    d.length() < 3.5
                        && angle >= -1.65 + previous * 3.3 - 0.24
                        && angle <= -1.65 + progress * 3.3 + 0.24
                }
                Action::Shove => along > 0.0 && d.length() < 2.2 && side < 1.1,
                _ => false,
            };
            if !run.hit && contact && (feet.y - e.pos.y).abs() < 1.4 && line(world, e.center(), eye)
            {
                damage = match run.kind {
                    Action::Thrust => 23.0,
                    Action::Sweep => 20.0,
                    _ => 12.0,
                };
                run.hit = true;
                e.toll.contacts += 1;
                println!(
                    "[tollkeeper] contact sequence={} damage={damage}",
                    run.sequence
                );
            }
        }
        if run.phase() == Phase::Recover
            && e.visible
            && matches!(run.kind, Action::Thrust | Action::Shove | Action::Sweep)
            && horizontal(feet - e.pos).length() < 2.8
        {
            move_ground(
                e,
                world,
                horizontal(e.pos - feet).normalize_or_zero(),
                dt * 1.2,
            );
        }
        if run.phase() == Phase::Done {
            if e.toll.follow && run.kind == Action::Thrust && e.visible {
                e.toll.follow = false;
                begin(e, Action::Sweep, feet, sounds);
            } else {
                e.cooldown = 0.55;
            }
        } else {
            e.toll.run = Some(run);
        }
    } else {
        let distance = horizontal(feet - e.pos).length();
        if !e.toll.enraged && e.health < e.max_health() * 0.5 {
            e.toll.enraged = true;
            e.cooldown = e.cooldown.max(1.5);
            sound(sounds, Cue::Warning, Some(e.center()), 0.8);
        }
        let remembered = e.visible || e.alert > 0.0;
        let target = if e.visible { feet } else { e.last_seen };
        let retrieve_gate =
            !e.toll.cells.is_empty() && e.toll.planted_for > if e.toll.enraged { 5.0 } else { 9.0 };
        if e.cooldown <= 0.0 && (remembered || retrieve_gate) {
            let choice = if retrieve_gate && horizontal(e.pos - e.toll.anchor).length() < 2.8 {
                Some(Action::Retrieve)
            } else if e.toll.stock > 0
                && e.toll.cells.is_empty()
                && e.visible
                && (e.toll.pressure >= 12.0 || e.attacks % 4 == 2)
                && distance > 2.0
            {
                Some(Action::Plant)
            } else if e.visible && distance < 1.8 {
                Some(Action::Shove)
            } else if e.visible && distance < 3.4 && e.attacks % 2 == 1 {
                Some(Action::Sweep)
            } else if e.visible && distance < 6.0 {
                Some(Action::Thrust)
            } else {
                None
            };
            if let Some(kind) = choice {
                e.toll.follow = e.toll.enraged && kind == Action::Thrust && e.attacks.is_multiple_of(3);
                begin(
                    e,
                    kind,
                    if kind == Action::Retrieve {
                        e.toll.anchor
                    } else {
                        target
                    },
                    sounds,
                );
            }
        }
        if e.toll.run.is_none() && (remembered || retrieve_gate) {
            let goal = if retrieve_gate { e.toll.anchor } else { target };
            let desired = horizontal(goal - e.pos).normalize_or_zero();
            // Try distinct sides of real obstructions. No through-wall pursuit.
            if horizontal(goal - e.pos).length() > if retrieve_gate { 2.0 } else { 2.7 }
                || (!e.visible && e.alert > 0.0)
            {
                for angle in [0.0_f32, 0.85, -0.85, 1.55, -1.55] {
                    let direction = Quat::from_rotation_y(angle) * desired;
                    let before = e.pos;
                    move_ground(
                        e,
                        world,
                        direction,
                        dt * if e.toll.stock > 0 { 1.25 } else { 2.5 },
                    );
                    if e.pos != before {
                        break;
                    }
                }
            }
            let yaw = (-desired.x).atan2(-desired.z);
            let turn = (yaw - e.yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
                - std::f32::consts::PI;
            e.yaw += turn.clamp(-dt * 2.2, dt * 2.2);
        }
    }
    e.velocity.y = (e.velocity.y - 14.0 * dt).max(-12.0);
    let vertical = e.velocity.y * dt;
    let steps = (vertical.abs() / 0.1).ceil().max(1.0) as usize;
    for _ in 0..steps {
        let q = e.pos + Vec3::Y * vertical / steps as f32;
        if free(world, q, KIND) {
            e.pos = q;
        } else {
            e.velocity.y = 0.0;
            break;
        }
    }
    (damage, (e.center() - eye).normalize_or_zero() * damage)
}

pub(super) fn model(e: &Enemy, m: &mut Model, time: f32) {
    let mark = m.mark();
    m.set_surface_kind(58.0);
    let p = e.pos;
    let run = e.toll.run.as_ref();
    let wind = run.map_or(0.0, |r| {
        if r.phase() == Phase::Prepare {
            r.wind()
        } else {
            0.0
        }
    });
    let active = run.is_some_and(|r| r.phase() == Phase::Active);
    let iron = if e.flash > 0.0 {
        [0.9, 0.83, 0.66]
    } else {
        [0.25, 0.28, 0.29]
    };
    let timber = [0.32, 0.21, 0.12];
    let gait = (e.phase * 5.5).sin() * 0.14;
    for x in [-0.22, 0.22] {
        m.cuboid(
            p + Vec3::new(x, 0.53, gait * x.signum()),
            Vec3::new(0.27, 1.0, 0.32),
            Quat::IDENTITY,
            iron,
            0.0,
        );
        m.cuboid(
            p + Vec3::new(x, 0.12, -0.08 + gait * x.signum()),
            Vec3::new(0.32, 0.24, 0.46),
            Quat::IDENTITY,
            iron,
            0.0,
        );
    }
    m.cuboid(
        p + Vec3::Y * 1.4,
        Vec3::new(0.8, 0.8, 0.48),
        Quat::from_rotation_x(-wind * 0.18),
        iron,
        0.0,
    );
    // Overlapping plates, a narrow gorget and an inset visor break the mannequin outline.
    for x in [-0.5, 0.5] {
        m.cuboid(
            p + Vec3::new(x, 1.72, 0.0),
            Vec3::new(0.42, 0.25, 0.58),
            Quat::from_rotation_z(-x * 0.3),
            iron,
            0.0,
        );
        m.cuboid(
            p + Vec3::new(x * 0.45, 0.88, -0.06),
            Vec3::new(0.31, 0.32, 0.5),
            Quat::from_rotation_x(-0.12),
            [0.19, 0.21, 0.22],
            0.0,
        );
    }
    m.cuboid(
        p + Vec3::Y * 1.84,
        Vec3::new(0.52, 0.18, 0.48),
        Quat::IDENTITY,
        [0.15, 0.17, 0.18],
        0.0,
    );
    m.cuboid(
        p + Vec3::Y * 2.05,
        Vec3::new(0.46, 0.5, 0.42),
        Quat::IDENTITY,
        iron,
        0.0,
    );
    m.cuboid(
        p + Vec3::new(0.0, 2.16, -0.21),
        Vec3::new(0.54, 0.11, 0.13),
        Quat::IDENTITY,
        [0.13, 0.15, 0.16],
        0.0,
    );
    m.cuboid(
        p + Vec3::new(0.0, 2.1, -0.23),
        Vec3::new(0.3, 0.055, 0.035),
        Quat::IDENTITY,
        [0.86, 0.61, 0.28],
        0.6,
    );
    let sweep = run.is_some_and(|r| r.kind == Action::Sweep);
    let strike = if sweep {
        let t = run.map_or(0.0, |r| {
            ((r.elapsed - r.prepare) / r.active).clamp(0.0, 1.0)
        });
        Quat::from_rotation_y(if active {
            -1.65 + t * 3.3
        } else {
            -wind * 1.65
        }) * Vec3::new(0.0, 0.0, -2.9)
    } else if run.is_some_and(|r| r.kind == Action::Shove) {
        Vec3::new(0.0, 0.0, if active { -1.45 } else { -0.8 })
    } else {
        Vec3::new(
            0.0,
            wind * 0.25,
            if active { -3.8 } else { -1.5 + wind * 0.7 },
        )
    };
    let hand = p + Vec3::new(0.55, 1.35, -0.1);
    m.beam(p + Vec3::new(0.4, 1.65, 0.0), hand, 0.17, iron, 0.0);
    m.beam(hand - strike * 0.2, hand + strike, 0.07, timber, 0.0);
    m.beam(
        hand + strike,
        hand + strike + Vec3::new(-0.36, -0.25, 0.0),
        0.12,
        iron,
        0.0,
    );
    if e.toll.stock > 0 || !e.toll.initialized {
        let lowering = run
            .filter(|r| r.kind == Action::Plant)
            .map_or(0.0, |r| r.wind());
        let gate = p + Vec3::new(-0.95, 1.35 - lowering * 0.1, -0.45 - lowering * 0.9);
        for x in [-1.25, 1.25] {
            m.cuboid(
                gate + Vec3::X * x,
                Vec3::new(0.22, 2.5, 0.22),
                Quat::IDENTITY,
                iron,
                0.0,
            );
        }
        for y in [-1.05, -0.55, 0.8, 1.15] {
            m.cuboid(
                gate + Vec3::Y * y,
                Vec3::new(2.5, 0.3, 0.18),
                Quat::IDENTITY,
                timber,
                0.0,
            );
        }
        m.beam(p + Vec3::new(-0.4, 1.65, 0.0), gate, 0.18, iron, 0.0);
    } else {
        let reach = if run.is_some_and(|r| r.kind == Action::Retrieve) {
            wind * 1.15
        } else {
            0.0
        };
        m.beam(
            p + Vec3::new(-0.4, 1.65, 0.0),
            p + Vec3::new(-0.6, 1.0, -reach),
            0.17,
            iron,
            0.0,
        );
    }
    m.bake_model_uv_since(mark, p, 58.0);
    m.rotate_since(mark, p, Quat::from_rotation_y(e.yaw));
    m.set_surface_kind(41.0);
    let _ = time;
}

pub(super) fn snapshot(e: &Enemy) -> Saved {
    let mut state = e.toll.clone();
    // Reload and reactivation cancel attacks into a fresh recovery; resource events stay applied.
    state.run = None;
    state.follow = false;
    Saved {
        pos: e.pos,
        home: e.home,
        health: e.health,
        yaw: e.yaw,
        state,
    }
}
pub(super) fn release_stock(e: &mut Enemy, world: &mut World) {
    use crate::world::inventory::{DroppedStack, Item, ItemStack};
    initialize(e, world.voxel_scale.meters());
    let count = std::mem::take(&mut e.toll.stock);
    e.toll.cells.clear(); // Planted material is already owned by the world.
    if count == 0 {
        return;
    }
    let item = Item::Block(B::Planks);
    let (inserted, remainder) = world.inventory.try_insert(ItemStack::new(item, count));
    if inserted > 0 {
        world.inventory.notify_pickup(item, inserted);
    }
    if remainder > 0 && world.inventory.dropped.len() < 1024 {
        world.inventory.dropped.push(DroppedStack {
            pos: e.pos + Vec3::Y,
            stack: ItemStack::new(item, remainder),
        });
    }
}
pub(super) fn restore(e: &mut Enemy, saved: &Saved) {
    e.pos = saved.pos;
    e.home = saved.home;
    e.health = saved.health;
    e.yaw = saved.yaw;
    e.toll = saved.state.clone();
    e.cooldown = 2.0;
    e.tell = 0.0;
}
pub(super) fn encode(saved: &Saved, bytes: &mut Vec<u8>) {
    let mut put = |v: u32| bytes.extend_from_slice(&v.to_le_bytes());
    for p in [saved.pos, saved.home, saved.state.anchor] {
        for v in p.to_array() {
            put(v.to_bits());
        }
    }
    for v in [
        saved.health.to_bits(),
        saved.yaw.to_bits(),
        saved.state.stock,
        saved.state.initialized as u32,
        saved.state.enraged as u32,
        saved.state.cells.len() as u32,
    ] {
        put(v);
    }
    for p in &saved.state.cells {
        for v in p.to_array() {
            put(v as u32);
        }
    }
}
pub(super) fn decode(read: &mut impl FnMut() -> anyhow::Result<u32>) -> anyhow::Result<Saved> {
    use anyhow::ensure;
    let mut vector = || -> anyhow::Result<Vec3> {
        let p = Vec3::new(
            f32::from_bits(read()?),
            f32::from_bits(read()?),
            f32::from_bits(read()?),
        );
        ensure!(
            p.is_finite() && p.abs().max_element() < 1_000_000.0,
            "Invalid Tollkeeper position"
        );
        Ok(p)
    };
    let pos = vector()?;
    let home = vector()?;
    let anchor = vector()?;
    let health = f32::from_bits(read()?);
    let yaw = f32::from_bits(read()?);
    let stock = read()?;
    let initialized = read()?;
    let enraged = read()?;
    let count = read()? as usize;
    ensure!(
        health.is_finite()
            && (0.0..=600.0).contains(&health)
            && yaw.is_finite()
            && stock <= MAX_GATE as u32
            && count <= MAX_GATE
            && stock as usize + count <= MAX_GATE
            && initialized <= 1
            && enraged <= 1
            && (initialized != 0 || (stock == 0 && count == 0)),
        "Invalid Tollkeeper state"
    );
    let mut cells = Vec::with_capacity(count);
    for _ in 0..count {
        let p = IVec3::new(read()? as i32, read()? as i32, read()? as i32);
        ensure!(
            p.to_array().iter().all(|v| v.unsigned_abs() < 4_000_000) && !cells.contains(&p),
            "Invalid gate cell"
        );
        cells.push(p);
    }
    Ok(Saved {
        pos,
        home,
        health,
        yaw,
        state: State {
            stock,
            initialized: initialized != 0,
            cells,
            anchor,
            enraged: enraged != 0,
            ..Default::default()
        },
    })
}

impl Adventure {
    pub(crate) fn encode_boss_series(&self, bytes: &mut Vec<u8>) {
        let mut saved = self.tollkeepers.clone();
        for e in &self.enemies {
            if e.kind == KIND && e.health > 0.0 {
                saved.insert(e.site, snapshot(e));
            }
        }
        let mut entries: Vec<_> = saved.iter().collect();
        entries.sort_by_key(|(site, _)| (site.x, site.y, site.z));
        bytes.extend_from_slice(&(entries.len() as u32).to_le_bytes());
        for (site, state) in entries {
            for v in site.to_array() {
                bytes.extend_from_slice(&(v as u32).to_le_bytes());
            }
            encode(state, bytes);
        }
    }
    pub(crate) fn decode_boss_series(
        &mut self,
        read: &mut impl FnMut() -> anyhow::Result<u32>,
    ) -> anyhow::Result<()> {
        use anyhow::ensure;
        let count = read()? as usize;
        ensure!(count <= MAX_RECORDS, "Too many boss records");
        for _ in 0..count {
            let site = IVec3::new(read()? as i32, read()? as i32, read()? as i32);
            let state = decode(read)?;
            ensure!(
                self.records
                    .get(&site)
                    .is_some_and(|mask| mask & reliquaries::AWAKENED != 0 && mask & 8 == 0),
                "Boss without a live awakened encounter"
            );
            ensure!(
                self.tollkeepers.insert(site, state).is_none(),
                "Duplicate boss state"
            );
        }
        Ok(())
    }
}

/// Disposable arena using the production sampler, damage path and first-person camera.
pub fn install_review(world: &mut World, player: &mut Player) {
    let s = world.voxel_scale.meters();
    let n = s.recip() as i32;
    let base = Vec3::new(0.0, 80.0, 0.0);
    let mut edits = Vec::new();
    for y in -6 * n..=7 * n {
        for z in -16 * n..=16 * n {
            for x in -18 * n..=18 * n {
                let q = IVec3::new(x, 80 * n + y, z);
                let p = (Vec3::new(x as f32, y as f32, z as f32) + Vec3::splat(0.5)) * s;
                if let Some(block) = crate::world::toll_crossing::block(p, s) {
                    edits.push((q, block));
                }
            }
        }
    }
    world.set_many(&edits);
    let site = IVec3::new(999, 999, 0);
    let mut enemy = Enemy::new(site, 3, KIND, base + Vec3::new(0.0, 0.02, -1.0), 0);
    if std::env::var_os("VOXEL_TOLLKEEPER_ENRAGED").is_some() {
        enemy.health *= 0.45;
    }
    world.adventure.enemies.clear();
    world.adventure.tollkeepers.clear();
    world
        .adventure
        .records
        .insert(site, reliquaries::AWAKENED | 256 | reliquaries::PHYSICAL);
    world.adventure.enemies.push(enemy);
    if std::env::var_os("VOXEL_TOLLKEEPER_BASELINE").is_some() {
        world.adventure.enemies.clear();
    }
    world.adventure.wand = 2;
    world.adventure.hunger = 50.0;
    player.pos = (base + Vec3::new(0.0, 0.02, 5.5)) / s;
    player.precise_pos = player.pos.as_dvec3();
    player.vel = Vec3::ZERO;
    review_camera(player, s);
    println!(
        "[tollkeeper-review] installed initial_health={:.1} voxel_scale={s}",
        world.adventure.health
    );
}
pub fn review_camera(player: &mut Player, scale: f32) {
    // Preserve ordinary first-person projection; this only fixes orientation.
    let direction =
        (Vec3::new(0.0, 81.3, -1.0) - (player.pos * scale + Vec3::Y * 1.62)).normalize_or_zero();
    player.yaw = direction.x.atan2(-direction.z);
    player.pitch = direction.y.asin();
}
pub fn review_cast(world: &mut World, player: &mut Player) -> bool {
    if std::env::var_os("VOXEL_TOLLKEEPER_CAST").is_none() {
        return false;
    }
    if let Some(e) = world
        .adventure
        .enemies
        .iter()
        .find(|e| e.kind == KIND && e.health > 0.0)
    {
        let s = world.voxel_scale.meters();
        let d = (e.center() - player.eye(world.voxel_scale) * s).normalize_or_zero();
        player.yaw = d.x.atan2(-d.z);
        player.pitch = d.y.asin();
        return true;
    }
    false
}
pub fn review_input(world: &World, player: &mut Player, input: &mut crate::player::Input) {
    let Some(e) = world
        .adventure
        .enemies
        .iter()
        .find(|e| e.kind == KIND && e.health > 0.0)
    else {
        return;
    };
    let scale = world.voxel_scale.meters();
    let d = (e.center() - player.eye(world.voxel_scale) * scale).normalize_or_zero();
    player.yaw = d.x.atan2(-d.z);
    player.pitch = d.y.asin();
    if std::env::var_os("VOXEL_TOLLKEEPER_MOVE").is_none() {
        return;
    }
    // Script normal movement keys through update_player: gravity, steps and cover stay real.
    let t = world.adventure.time;
    let target = if ((t / 5.0) as u32).is_multiple_of(2) {
        Vec3::new(-2.5, 80.0, 6.5)
    } else {
        Vec3::new(2.5, 80.0, 8.5)
    };
    let delta = horizontal(target - player.pos * scale);
    let forward = horizontal(player.forward_dir()).normalize_or_zero();
    let right = Vec3::new(-forward.z, 0.0, forward.x);
    input.forward = delta.dot(forward) > 0.25;
    input.back = delta.dot(forward) < -0.25;
    input.right = delta.dot(right) > 0.25;
    input.left = delta.dot(right) < -0.25;
}
pub fn observe_review(world: &World) {
    use std::sync::atomic::{AtomicU32, Ordering};
    static LAST: AtomicU32 = AtomicU32::new(u32::MAX);
    let key = (world.adventure.time / 2.0) as u32;
    if LAST.swap(key, Ordering::Relaxed) == key {
        return;
    }
    if let Some(e) = world.adventure.enemies.iter().find(|e| e.kind == KIND) {
        println!(
            "[tollkeeper-review] t={:.2} player_health={:.1} boss_health={:.1} action={:?} phase={:?} pos={:?} stock={} world_gate={} plants={} reclaimed={} contacts={}",
            world.adventure.time,
            world.adventure.health,
            e.health,
            e.toll.run.as_ref().map(|r| r.kind),
            e.toll.run.as_ref().map(|r| r.phase()),
            e.pos,
            e.toll.stock,
            e.toll.cells.len(),
            e.toll.plants,
            e.toll.retrieved,
            e.toll.contacts
        );
    }
}

/// Explicit disposable play-review: use the real region checkpoint worker and decoder.
pub fn review_checkpoint(
    world: &mut World,
    player: &mut Player,
    day: &mut crate::world::DayNightCycle,
    saves: &mut crate::world::storage::SaveService,
) {
    use std::sync::atomic::{AtomicU8, Ordering};
    static STAGE: AtomicU8 = AtomicU8::new(0);
    if std::env::var_os("VOXEL_TOLLKEEPER_CHECKPOINT").is_none() {
        return;
    }
    let Some(root) = std::env::var_os("VOXEL_CAPTURE_DIR") else {
        return;
    };
    // This mode is restricted to its capture directory, never the player's save slot.
    let expected = std::path::PathBuf::from(root).join("disposable-saves");
    if std::env::var_os("VOXEL_SAVE_DIR").map(std::path::PathBuf::from) != Some(expected) {
        return;
    }
    match STAGE.load(Ordering::Relaxed) {
        0 => {
            if let Some(e) = world
                .adventure
                .enemies
                .iter()
                .find(|e| e.kind == KIND && e.toll.plants > 0 && !e.toll.cells.is_empty())
            {
                println!(
                    "[tollkeeper-checkpoint] request health={:.1} carried={} placed={}",
                    e.health,
                    e.toll.stock,
                    e.toll.cells.len()
                );
                crate::world::storage::set_autosave_enabled(false);
                saves.request(true);
                STAGE.store(1, Ordering::Relaxed);
            }
        }
        1 => {
            saves.tick(Some(world), player, day);
            if !saves.busy() {
                STAGE.store(2, Ordering::Relaxed);
                if saves.failed {
                    eprintln!("[tollkeeper-checkpoint] save failed: {:?}", saves.status);
                    return;
                }
                match crate::world::storage::load_review_checkpoint() {
                    Ok(mut session) => {
                        let site = IVec3::new(999, 999, 0);
                        if let Some(saved) = session.world.adventure.tollkeepers.get(&site).cloned()
                        {
                            let mut e = Enemy::new(site, 3, KIND, Vec3::ZERO, 0);
                            restore(&mut e, &saved);
                            let published = e
                                .toll
                                .cells
                                .iter()
                                .filter(|&&q| session.world.get(q) == B::Planks)
                                .count();
                            println!(
                                "[tollkeeper-checkpoint] loaded version={} health={:.1} carried={} placed={} world_cells={} cooldown={:.1} attack={:?}",
                                crate::world::storage::SAVE_VERSION,
                                e.health,
                                e.toll.stock,
                                e.toll.cells.len(),
                                published,
                                e.cooldown,
                                e.toll.run
                            );
                            session
                                .world
                                .adventure
                                .dirty_voxels
                                .extend(e.toll.cells.iter().copied());
                            session.world.adventure.enemies.push(e);
                            *world = session.world;
                            *player = session.player;
                            *day = session.day_night;
                        } else {
                            eprintln!("[tollkeeper-checkpoint] missing saved actor");
                        }
                    }
                    Err(error) => eprintln!("[tollkeeper-checkpoint] load failed: {error:#}"),
                }
            }
        }
        _ => {}
    }
}
