//! Short, material-valid excavation and conserved physical spoil.
//! Staged until the family habitat/save/arena integration is complete.
use super::*;
use crate::world::BlockId as B;
use action::{Phase, Run};
pub(super) const KIND: u32 = 17;
const MAX_SPOIL: usize = 512;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Action {
    Dig,
    Sweep,
    Spill,
}
#[derive(Default)]
pub(super) struct State {
    pub run: Option<Run<Action>>,
    pub spoil: Vec<B>,
    pub removed: u32,
    pub deposited: u32,
    pub work: f64,
    pub blocked: bool,
}
pub(super) fn soft(b: B) -> bool {
    matches!(b, B::Dirt | B::Sand | B::Clay | B::Mud | B::Gravel)
}
pub(super) fn action_name(e: &Enemy) -> Option<&'static str> {
    Some(if e.miller.blocked {
        "Resistant face — cutter stopped"
    } else {
        match e.miller.run.as_ref().map(|r| (r.kind, r.phase())) {
            Some((_, Phase::Recover)) => "Cutter exposed — recovering",
            Some((Action::Dig, _)) => "Short excavation — leave its line",
            Some((Action::Sweep, _)) => "Cutter sweep — retreat or flank",
            Some((Action::Spill, _)) => "Depositing carried spoil",
            None => "Surveying the face",
        }
    })
}
fn front(world: &World, at: Vec3) -> Option<Vec<(IVec3, B)>> {
    let scale = world.voxel_scale.meters();
    let lo = ((at + Vec3::new(-0.95, 0.05, -0.95)) / scale)
        .floor()
        .as_ivec3();
    let hi = ((at + Vec3::new(0.95, 1.4, 0.95)) / scale)
        .floor()
        .as_ivec3();
    let mut material = Vec::new();
    for y in lo.y..=hi.y {
        for z in lo.z..=hi.z {
            for x in lo.x..=hi.x {
                let p = IVec3::new(x, y, z);
                let b = world.get(p);
                if b == B::Air || !world.solid(p) {
                    continue;
                }
                if !soft(b) {
                    return None;
                }
                material.push((p, b));
            }
        }
    }
    Some(material)
}
fn cut(e: &mut Enemy, world: &mut World, dirty: &mut Vec<IVec3>) -> bool {
    let next = e.pos + e.lunge * 0.3;
    if !factory::solid_point(world, next - Vec3::Y * 0.15) {
        return false;
    }
    let Some(material) = front(world, next) else {
        return false;
    };
    let limit = (MAX_SPOIL - e.miller.spoil.len()).min(16);
    let edits: Vec<_> = material
        .iter()
        .take(limit)
        .map(|(p, _)| (*p, B::Air))
        .collect();
    world.set_many(&edits);
    // The removed cell is now carried, never also credited as inventory or ore.
    for &(p, b) in material.iter().take(limit) {
        if world.get(p) == B::Air {
            e.miller.spoil.push(b);
            e.miller.removed = e.miller.removed.saturating_add(1);
            dirty.push(p);
        }
    }
    if free(world, next, KIND) {
        e.phase += e.pos.distance(next);
        e.pos = next;
        true
    } else {
        !edits.is_empty()
    }
}
fn spill(e: &mut Enemy, world: &mut World, feet: Vec3, dirty: &mut Vec<IVec3>) {
    let scale = world.voxel_scale.meters();
    let base = ((e.pos - e.lunge * 2.5) / scale).floor().as_ivec3();
    let side = Vec3::new(-e.lunge.z, 0.0, e.lunge.x);
    for i in 0..16 {
        let Some(&block) = e.miller.spoil.last() else {
            break;
        };
        let offset =
            (side * ((i % 5) as f32 - 2.0)).round().as_ivec3() + IVec3::Y * ((i / 5) as i32);
        let p = base + offset;
        let centre = (p.as_vec3() + Vec3::splat(0.5)) * scale;
        if centre.distance(feet) < 1.4
            || centre.distance(e.pos) < 1.5
            || world.get(p) != B::Air
            || !world.solid(p - IVec3::Y)
        {
            continue;
        }
        world.set(p, block);
        if world.get(p) == block {
            e.miller.spoil.pop();
            e.miller.deposited = e.miller.deposited.saturating_add(1);
            dirty.push(p);
        }
    }
}
pub(super) fn tick(
    e: &mut Enemy,
    world: &mut World,
    feet: Vec3,
    eye: Vec3,
    dt: f32,
    sounds: &mut Vec<Sound>,
    dirty: &mut Vec<IVec3>,
    navigation_budget: &mut usize,
) -> (f32, Vec3) {
    navigation::gravity(e, world, dt);
    let service = family_service(world, e);
    let near = e.visible && feet.distance(e.pos) < 24.0;
    if let Some(mut run) = e.miller.run.take() {
        if e.stagger > 0.35 && run.phase() == Phase::Prepare {
            run.interrupt();
        }
        let release = run.advance(dt);
        e.tell = if run.phase() == Phase::Prepare {
            run.prepare - run.elapsed
        } else {
            0.0
        };
        if release && run.kind == Action::Dig && !family_feed(world, e, 64) {
            run.interrupt();
        }
        if (run.phase() == Phase::Active && run.kind != Action::Sweep)
            || run.phase() == Phase::Recover
        {
            e.miller.work += dt as f64;
        }
        let mut damage = 0.0;
        if run.phase() == Phase::Active && run.kind == Action::Dig && e.miller.work >= 0.1 {
            e.miller.work -= 0.1;
            if e.miller.spoil.len() >= MAX_SPOIL || !cut(e, world, dirty) {
                e.miller.blocked = true;
                run.interrupt();
            }
        }
        if (run.phase() == Phase::Recover || run.kind == Action::Spill) && e.miller.work >= 0.1 {
            e.miller.work -= 0.1;
            spill(e, world, feet, dirty);
        }
        if run.phase() == Phase::Active && !run.hit && run.kind != Action::Spill {
            let d = (feet - e.pos) * Vec3::new(1.0, 0.0, 1.0);
            let contact = if run.kind == Action::Dig {
                d.dot(e.lunge) > 0.0 && d.cross(e.lunge).length() < 0.85 && d.length() < 2.8
            } else {
                d.length() < 3.2 && d.normalize_or_zero().dot(e.lunge) > -0.1
            };
            if contact && (feet.y - e.pos.y).abs() < 1.8 && line(world, e.center(), eye) {
                damage = 25.0;
                run.hit = true;
            }
        }
        if run.phase() != Phase::Done {
            e.miller.run = Some(run);
        } else {
            e.cooldown = 0.5;
        }
        return (damage, (e.center() - eye).normalize_or_zero() * damage);
    }
    if (!near && !service) || e.cooldown > 0.0 {
        return (0.0, Vec3::ZERO);
    }
    let d = if near {
        (feet - e.pos) * Vec3::new(1.0, 0.0, 1.0)
    } else {
        family_heading(world, e) * 4.0
    };
    let direction = d.normalize_or_zero();
    let material = front(world, e.pos + direction * 0.8);
    let diggable = material.as_ref().is_some_and(|m| !m.is_empty());
    let kind = if e.miller.spoil.len() > MAX_SPOIL / 2 {
        Action::Spill
    } else if near && d.length() < 3.2 {
        Action::Sweep
    } else if diggable {
        Action::Dig
    } else if !e.miller.spoil.is_empty() {
        Action::Spill
    } else {
        e.miller.blocked = material.is_none();
        if near {
            let heading =
                e.navigation
                    .heading(world, e.pos, feet, KIND, dt, e.slot, navigation_budget);
            let q = e.pos + heading * dt * 1.1;
            if free(world, q, KIND) && factory::solid_point(world, q - Vec3::Y * 0.15) {
                e.pos = q;
            }
        }
        return (0.0, Vec3::ZERO);
    };
    e.miller.blocked = false;
    e.lunge = direction;
    e.yaw = (-direction.x).atan2(-direction.z);
    e.attacks = e.attacks.wrapping_add(1);
    e.attack = 8;
    let (prepare, active, recover) = match kind {
        Action::Dig => (1.1, 2.0, 1.4),
        Action::Sweep => (0.95, 0.4, 1.1),
        Action::Spill => (0.6, 1.2, 0.7),
    };
    e.tell = prepare;
    e.miller.run = Some(Run::new(kind, e.attacks, prepare, active, recover));
    sound(sounds, Cue::Warning, Some(e.center()), 0.75);
    println!(
        "[miller] action={kind:?} removed={} deposited={} carried={}",
        e.miller.removed,
        e.miller.deposited,
        e.miller.spoil.len()
    );
    (0.0, Vec3::ZERO)
}
pub(super) fn model(e: &Enemy, m: &mut Model, _time: f32) {
    let mark = m.mark();
    m.set_surface_kind(58.0);
    let local = |x, y, z| e.pos + Vec3::new(x, y, z);
    let wind = e.miller.run.as_ref().map_or(0.0, |r| r.wind());
    let body = if e.flash > 0.0 {
        PALETTE_PAPER
    } else {
        [0.42, 0.34, 0.23]
    };
    m.cuboid(
        local(0.0, 0.75, 0.2),
        Vec3::new(1.7, 1.1, 1.8),
        Quat::IDENTITY,
        body,
        0.0,
    );
    for i in 0..4 {
        m.cuboid(
            local(0.0, 1.25 - i as f32 * 0.08, 0.55 - i as f32 * 0.35),
            Vec3::new(1.65, 0.15, 0.25),
            Quat::IDENTITY,
            [0.3, 0.29, 0.23],
            0.0,
        );
    }
    for side in [-1.0, 1.0] {
        for z in [-0.5, 0.5] {
            let stride = (e.phase * 5.0 + z * 3.0).sin() * 0.15;
            m.beam(
                local(side * 0.65, 0.7, z),
                local(side * 1.05, 0.1, z + stride),
                0.15,
                body,
                0.0,
            );
        }
        m.cuboid(
            local(side * 0.7, 0.8, 0.8),
            Vec3::new(0.35, 0.35, 0.45),
            Quat::IDENTITY,
            [0.2, 0.18, 0.14],
            0.0,
        );
    }
    m.cuboid(
        local(0.0, 0.65, -1.0 - wind * 0.2),
        Vec3::new(1.6, 0.9, 0.5),
        Quat::IDENTITY,
        [0.6, 0.52, 0.34],
        0.0,
    );
    for i in 0..7 {
        m.cuboid(
            local(i as f32 * 0.25 - 0.75, 0.4, -1.4 - wind * 0.2),
            Vec3::new(0.15, 0.45, 0.2),
            Quat::from_rotation_x(-0.3),
            [0.72, 0.69, 0.57],
            0.0,
        );
    }
    for x in [-0.45, 0.45] {
        m.cuboid(
            local(x, 0.95, -1.27),
            Vec3::splat(0.1),
            Quat::IDENTITY,
            PALETTE_AMBER,
            0.2,
        );
    }
    m.bake_model_uv_since(mark, e.pos, 58.0);
    m.rotate_since(mark, e.pos, Quat::from_rotation_y(e.yaw));
    m.set_surface_kind(41.0);
}
impl State {
    pub(super) fn save(&self, out: &mut Vec<u32>) {
        out.extend([
            self.removed,
            self.deposited,
            self.work.to_bits() as u32,
            (self.work.to_bits() >> 32) as u32,
            self.spoil.len() as u32,
        ]);
        out.extend(self.spoil.iter().map(|b| b.as_u32()));
    }
    pub(super) fn restore(&mut self, words: &[u32]) -> anyhow::Result<()> {
        anyhow::ensure!(
            words.len() >= 5
                && words[4] as usize <= MAX_SPOIL
                && words.len() == 5 + words[4] as usize,
            "Invalid Miller spoil"
        );
        let work = f64::from_bits(words[2] as u64 | ((words[3] as u64) << 32));
        anyhow::ensure!(
            work.is_finite() && (0.0..=0.1).contains(&work),
            "Invalid Miller work clock"
        );
        let spoil: Vec<_> = words[5..].iter().map(|v| B::from_u32(*v)).collect();
        anyhow::ensure!(
            spoil.iter().all(|b| soft(*b))
                && words[0].saturating_sub(words[1]) == spoil.len() as u32,
            "Invalid Miller material accounting"
        );
        self.removed = words[0];
        self.deposited = words[1];
        self.work = work;
        self.spoil = spoil;
        Ok(())
    }
}
