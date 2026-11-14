//! One owned lanterncap. Growth, shrink, root, and conjure share this record.
use super::*;
use crate::mesh::models::Model;
use crate::world::BlockId as B;
use crate::world::factory::Kind;
use anyhow::ensure;
use glam::{IVec3, Quat, Vec3};

pub(super) mod lanterncap;

pub use engine_content::SizeClass;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "multiplayer", derive(serde::Serialize, serde::Deserialize))]
pub enum Age {
    Juvenile = 0,
    Mature = 1,
}

impl Age {
    fn from_u32(v: u32) -> Option<Self> {
        match v {
            0 => Some(Self::Juvenile),
            1 => Some(Self::Mature),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "multiplayer", derive(serde::Serialize, serde::Deserialize))]
pub struct Companion {
    pub id: u32,
    pub age: Age,
    pub size: SizeClass,
    pub rooted: bool,
    pub health: f32,
    pub pos: Vec3,
    pub shown: f32,
    pub crate_mark: Option<IVec3>,
    pub pad_mark: Option<IVec3>,
    pub root_cells: Vec<(IVec3, B)>,
    pub idle_motes: u8,
    pub wait_played: bool,
}

impl Companion {
    pub(crate) fn is_rescued(&self) -> bool {
        self.id == lanterncap::ID
    }

    pub fn juvenile(id: u32, pos: Vec3) -> Self {
        Self {
            id,
            age: Age::Juvenile,
            size: SizeClass::Small,
            rooted: false,
            health: 40.0,
            pos,
            shown: SizeClass::Small.height(),
            crate_mark: None,
            pad_mark: None,
            root_cells: Vec::new(),
            idle_motes: 0,
            wait_played: false,
        }
    }

    pub fn model(&self, m: &mut Model, time: f32, viewer: Vec3) {
        let h = self.shown.max(0.4);
        let w = self.size.half_width();
        let rescued = self.is_rescued();
        let bob = if self.rooted {
            0.0
        } else if rescued {
            (time * 4.4).sin() * 0.085
        } else {
            (time * 2.2).sin() * 0.04
        };
        let foot = self.pos + Vec3::Y * bob;
        m.cuboid(
            foot + Vec3::Y * (h * 0.28),
            Vec3::new(w * 0.55, h * 0.28, w * 0.55),
            Quat::IDENTITY,
            [0.42, 0.28, 0.16],
            0.0,
        );
        m.cuboid(
            foot + Vec3::Y * (h * 0.72),
            Vec3::new(w * 1.7, h * 0.22, w * 1.7),
            Quat::IDENTITY,
            if rescued { [0.39, 0.73, 0.56] } else { super::PALETTE_LEAF },
            if rescued { 0.45 } else { 0.2 },
        );
        m.orb(
            foot + Vec3::Y * (h * 0.78),
            w * 0.18,
            super::PALETTE_AMBER,
            0.7,
        );
        if rescued {
            let facing = (viewer - foot).with_y(0.0).try_normalize().unwrap_or(Vec3::Z);
            let across = Vec3::Y.cross(facing);
            for side in [-1.0, 1.0] {
                m.orb(foot + Vec3::Y * (h * 0.43) + facing * (w * 0.56) + across * (side * w * 0.30),
                    w * 0.11, [1.0, 0.87, 0.48], 1.7);
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GrowthTarget {
    Companion(u32),
    SporeCell(IVec3),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GrowthReject {
    Clearance,
    NotATarget,
    OwnerFull,
    NoStep,
}

pub fn clearance_ok(world: &World, feet: Vec3, size: SizeClass) -> bool {
    let scale = world.voxel_scale.meters().max(1e-3);
    let steps = (size.height() / scale).ceil() as i32;
    let span = (size.half_width() / scale).floor() as i32;
    let base = (feet / scale).floor().as_ivec3();
    for y in 0..=steps {
        for x in -span..=span {
            for z in -span..=span {
                if world.solid(base + IVec3::new(x, y, z)) {
                    return false;
                }
            }
        }
    }
    true
}

fn step_size(size: SizeClass, steps: i32) -> Result<SizeClass, GrowthReject> {
    if steps == 1 {
        size.next().ok_or(GrowthReject::NoStep)
    } else if steps == -1 {
        size.prev().ok_or(GrowthReject::NoStep)
    } else {
        Err(GrowthReject::NoStep)
    }
}

pub fn apply_growth(
    world: &mut World,
    target: GrowthTarget,
    steps: i32,
) -> Result<SizeClass, GrowthReject> {
    match target {
        GrowthTarget::SporeCell(cell) => {
            if steps != 1 {
                return Err(GrowthReject::NoStep);
            }
            if world.adventure.companion.is_some() {
                return Err(GrowthReject::OwnerFull);
            }
            let scale = world.voxel_scale.meters();
            let feet = (cell.as_vec3() + Vec3::new(0.5, 1.0, 0.5)) * scale;
            if !matches!(
                world.get(cell),
                B::FungalSoil | B::GlowCap | B::Dirt | B::Grass
            ) {
                return Err(GrowthReject::NotATarget);
            }
            if !clearance_ok(world, feet, SizeClass::Small) {
                emit_world(&mut world.adventure, GrowthEvent::Reject, feet);
                return Err(GrowthReject::Clearance);
            }
            world.adventure.companion = Some(Companion::juvenile(1, feet));
            emit_world(&mut world.adventure, GrowthEvent::Conjure, feet);
            Ok(SizeClass::Small)
        }
        GrowthTarget::Companion(id) => {
            let Some(current) = world
                .adventure
                .companion
                .as_ref()
                .filter(|c| c.id == id)
                .map(|c| (c.size, c.pos, c.rooted, c.age))
            else {
                return Err(GrowthReject::NotATarget);
            };
            let next = step_size(current.0, steps)?;
            if !clearance_ok(world, current.1, next) {
                emit_world(&mut world.adventure, GrowthEvent::Reject, current.1);
                return Err(GrowthReject::Clearance);
            }
            let rooted = current.2;
            if let Some(c) = world.adventure.companion.as_mut() {
                c.size = next;
                c.age = current.3;
            }
            if rooted {
                rebuild_root(world, id);
            }
            emit_world(
                &mut world.adventure,
                GrowthEvent::Success,
                current.1 + Vec3::Y * next.height(),
            );
            Ok(next)
        }
    }
}

pub fn set_rooted(world: &mut World, id: u32, rooted: bool) {
    let Some(c) = world.adventure.companion.as_ref() else {
        return;
    };
    if c.id != id {
        return;
    }
    if !rooted {
        let cells = world
            .adventure
            .companion
            .as_ref()
            .unwrap()
            .root_cells
            .clone();
        for (cell, block) in cells {
            if world.get(cell) == block {
                world.set(cell, B::Air);
            }
        }
        if let Some(c) = world.adventure.companion.as_mut() {
            c.rooted = false;
            c.root_cells.clear();
        }
        return;
    }
    if let Some(c) = world.adventure.companion.as_mut() {
        c.rooted = true;
    }
    rebuild_root(world, id);
    if let Some(pos) = world.adventure.companion.as_ref().map(|c| c.pos) {
        emit_world(&mut world.adventure, GrowthEvent::Root, pos);
    }
}

fn rebuild_root(world: &mut World, id: u32) {
    let Some(c) = world.adventure.companion.as_ref() else {
        return;
    };
    if c.id != id {
        return;
    }
    let old = c.root_cells.clone();
    let size = c.size;
    let pos = c.pos;
    for (cell, block) in &old {
        if world.get(*cell) == *block {
            world.set(*cell, B::Air);
        }
    }
    let scale = world.voxel_scale.meters().max(1e-3);
    let base = (pos / scale).floor().as_ivec3();
    let top = (size.height() / scale).floor() as i32;
    let span = (size.half_width() / scale).floor() as i32;
    let mut cells = Vec::new();
    for y in 0..=top.max(1) {
        let cap = y >= top.max(1);
        for x in -span..=span {
            for z in -span..=span {
                if !cap && (x != 0 || z != 0) {
                    continue;
                }
                let cell = base + IVec3::new(x, y, z);
                if world.solid(cell) {
                    continue;
                }
                let block = if cap { B::GlowCap } else { B::FungalSoil };
                world.set(cell, block);
                cells.push((cell, block));
                if cells.len() >= 48 {
                    break;
                }
            }
        }
    }
    if let Some(c) = world.adventure.companion.as_mut() {
        c.root_cells = cells;
        c.rooted = true;
    }
}

#[derive(Clone, Copy)]
pub struct ParticleRecipe {
    pub color: [f32; 3],
    pub glow: f32,
    pub gravity: f32,
    pub life: f32,
    pub size: f32,
    pub burst: u8,
}

#[derive(Clone, Copy)]
pub enum GrowthEvent {
    Success,
    Reject,
    Root,
    Idle,
    Conjure,
}

pub fn recipe(spell: u32) -> ParticleRecipe {
    let color = spell_color(spell);
    ParticleRecipe {
        color,
        glow: 1.4,
        gravity: if matches!(spell, 35 | 36 | 44) {
            0.2
        } else {
            1.2
        },
        life: 0.6,
        size: 0.04,
        burst: 8,
    }
}

fn event_recipe(event: GrowthEvent) -> ParticleRecipe {
    match event {
        GrowthEvent::Success | GrowthEvent::Conjure | GrowthEvent::Root => ParticleRecipe {
            color: super::PALETTE_OLIVE,
            glow: 1.15,
            gravity: 0.2,
            life: 0.8,
            size: 0.05,
            burst: 16,
        },
        GrowthEvent::Reject => ParticleRecipe {
            color: [0.48, 0.36, 0.26],
            glow: 0.4,
            gravity: 2.0,
            life: 0.4,
            size: 0.035,
            burst: 6,
        },
        GrowthEvent::Idle => ParticleRecipe {
            color: super::PALETTE_AMBER,
            glow: 1.2,
            gravity: 0.2,
            life: 0.7,
            size: 0.03,
            burst: 1,
        },
    }
}

pub fn emit_world(adventure: &mut Adventure, event: GrowthEvent, pos: Vec3) {
    if matches!(event, GrowthEvent::Idle) {
        let Some(c) = adventure.companion.as_mut() else {
            return;
        };
        if c.idle_motes >= 8 {
            return;
        }
        c.idle_motes = c.idle_motes.saturating_add(1);
    }
    let recipe = event_recipe(event);
    let ring = matches!(
        event,
        GrowthEvent::Success | GrowthEvent::Conjure | GrowthEvent::Root
    );
    let mut spawned = 0u8;
    if ring {
        for i in 0..12 {
            if adventure.particles.len() >= 480 {
                break;
            }
            let a = i as f32 * std::f32::consts::TAU / 12.0;
            adventure.spawn_voxel_particle(
                pos + Vec3::new(a.cos() * 0.45, 0.0, a.sin() * 0.45),
                Vec3::new(a.cos() * 0.4, 0.6, a.sin() * 0.4),
                recipe.color,
                recipe.size,
                recipe.glow,
                0.5,
                recipe.gravity,
                recipe.life,
            );
            spawned += 1;
        }
    }
    let burst = recipe.burst;
    for i in 0..burst {
        if adventure.particles.len() >= 480 {
            break;
        }
        let a = i as f32 * 2.4 + pos.x;
        adventure.spawn_voxel_particle(
            pos,
            Vec3::new(
                a.cos() * 0.8,
                if recipe.gravity < 0.5 { 1.2 } else { 0.2 },
                a.sin() * 0.8,
            ),
            recipe.color,
            recipe.size,
            recipe.glow,
            0.85,
            recipe.gravity,
            recipe.life,
        );
        spawned = spawned.saturating_add(1);
        let _ = spawned;
    }
}

pub fn resolve_effect(
    world: &mut World,
    effect: u32,
    at: Vec3,
    created: Option<u32>,
) -> Option<u32> {
    match effect {
        36 => {
            if world.adventure.companion.is_some() {
                emit_world(&mut world.adventure, GrowthEvent::Reject, at);
                return None;
            }
            if !clearance_ok(world, at, SizeClass::Small) {
                emit_world(&mut world.adventure, GrowthEvent::Reject, at);
                return None;
            }
            world.adventure.companion = Some(Companion::juvenile(1, at));
            emit_world(&mut world.adventure, GrowthEvent::Conjure, at);
            Some(1)
        }
        35 | 39 => {
            let steps = if effect == 39 { -1 } else { 1 };
            let target = if let Some(id) = created {
                GrowthTarget::Companion(id)
            } else if world
                .adventure
                .companion
                .as_ref()
                .is_some_and(|c| (c.pos - at).length() < 3.0)
            {
                let id = world.adventure.companion.as_ref().unwrap().id;
                GrowthTarget::Companion(id)
            } else {
                let scale = world.voxel_scale.meters();
                let cell = (at / scale).floor().as_ivec3();
                GrowthTarget::SporeCell(cell)
            };
            let id = match &target {
                GrowthTarget::Companion(id) => Some(*id),
                GrowthTarget::SporeCell(_) => Some(1),
            };
            apply_growth(world, target, steps).ok()?;
            id
        }
        _ => None,
    }
}

pub fn tick(adventure: &mut Adventure, dt: f32) {
    let Some(c) = adventure.companion.as_mut() else {
        return;
    };
    let target = c.size.height();
    let rate = (dt / 0.35).clamp(0.0, 1.0);
    c.shown += (target - c.shown) * rate;
    if c.idle_motes > 0 {
        c.idle_motes = c.idle_motes.saturating_sub(1);
    }
}

pub fn cultivate(world: &mut World) {
    let scale = world.voxel_scale.meters();
    let companion_cell = world
        .adventure
        .companion
        .as_ref()
        .map(|c| ((c.pos / scale).floor().as_ivec3(), c.id));
    let machines: Vec<IVec3> = world
        .factory
        .nodes
        .iter()
        .filter(|(_, n)| n.kind == Kind::Cultivator && n.enabled && n.input[13] >= 1)
        .map(|(p, _)| *p)
        .collect();
    for p in machines {
        let Some((cell, id)) = companion_cell else {
            continue;
        };
        let d = cell - p;
        if d.x.abs() + d.y.abs() + d.z.abs() != 1 {
            continue;
        }
        if apply_growth(world, GrowthTarget::Companion(id), 1).is_ok()
            && let Some(n) = world.factory.nodes.get_mut(&p) {
                n.input[13] = n.input[13].saturating_sub(1);
            }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CarryState {
    Idle,
    Rooted,
    Waiting,
    Moved,
}

pub fn carry_step(world: &mut World) -> CarryState {
    let Some(c) = world.adventure.companion.as_ref() else {
        return CarryState::Idle;
    };
    if c.rooted {
        return CarryState::Rooted;
    }
    let (Some(from), Some(to)) = (c.crate_mark, c.pad_mark) else {
        return CarryState::Idle;
    };
    let carry = c.size.carry() as u32;
    let scale = world.voxel_scale.meters();
    let from_pos = (from.as_vec3() + Vec3::splat(0.5)) * scale;
    let idx = world
        .inventory
        .dropped
        .iter()
        .position(|d| (d.pos - from_pos).length() < 1.6);
    let Some(idx) = idx else {
        return CarryState::Idle;
    };
    if world.inventory.dropped[idx].stack.count > carry {
        if let Some(c) = world.adventure.companion.as_mut()
            && !c.wait_played {
                c.wait_played = true;
                let pos = c.pos;
                emit_world(&mut world.adventure, GrowthEvent::Reject, pos);
            }
        return CarryState::Waiting;
    }
    let pad = (to.as_vec3() + Vec3::splat(0.5)) * scale;
    world.inventory.dropped[idx].pos = pad;
    if let Some(c) = world.adventure.companion.as_mut() {
        c.pos = pad;
        c.wait_played = false;
    }
    CarryState::Moved
}

impl Adventure {
    pub(crate) fn encode_companion(&self, bytes: &mut Vec<u8>) {
        let mut put = |n: u32| bytes.extend_from_slice(&n.to_le_bytes());
        let Some(c) = &self.companion else {
            put(0);
            return;
        };
        put(1);
        put(c.id);
        put(c.age as u32);
        put(c.size as u32);
        put(u32::from(c.rooted));
        put(c.health.to_bits());
        put(c.pos.x.to_bits());
        put(c.pos.y.to_bits());
        put(c.pos.z.to_bits());
        put(c.root_cells.len() as u32);
        for (cell, block) in c.root_cells.iter().take(48) {
            put(cell.x as u32);
            put(cell.y as u32);
            put(cell.z as u32);
            put(*block as u32);
        }
    }

    pub(crate) fn decode_companion(
        read: &mut impl FnMut() -> anyhow::Result<u32>,
    ) -> anyhow::Result<Option<Companion>> {
        let flag = read()?;
        if flag == 0 {
            return Ok(None);
        }
        ensure!(flag == 1, "Invalid companion flag");
        let id = read()?;
        let age = Age::from_u32(read()?).ok_or_else(|| anyhow::anyhow!("Invalid companion age"))?;
        let size = SizeClass::from_u32(read()?)
            .ok_or_else(|| anyhow::anyhow!("Invalid companion size"))?;
        let rooted = read()? != 0;
        let health = f32::from_bits(read()?);
        let pos = Vec3::new(
            f32::from_bits(read()?),
            f32::from_bits(read()?),
            f32::from_bits(read()?),
        );
        ensure!(
            health.is_finite() && (0.0..=200.0).contains(&health) && pos.is_finite(),
            "Invalid companion body"
        );
        let count = read()? as usize;
        ensure!(count <= 48, "Invalid companion roots");
        let mut root_cells = Vec::with_capacity(count);
        for _ in 0..count {
            let cell = IVec3::new(read()? as i32, read()? as i32, read()? as i32);
            let block = B::from_u32(read()?);
            root_cells.push((cell, block));
        }
        Ok(Some(Companion {
            id,
            age,
            size,
            rooted,
            health,
            pos,
            shown: size.height(),
            crate_mark: None,
            pad_mark: None,
            root_cells,
            idle_motes: 0,
            wait_played: false,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fluid::VoxelScalePreset;
    use crate::world::{BlockId, World, WorldType};

    fn world() -> World {
        World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 3)
    }

    #[test]
    fn growth_steps_age_and_rejects_a_low_ceiling() {
        let mut world = world();
        world.adventure.companion = Some(Companion::juvenile(1, Vec3::new(4.0, 80.0, 4.0)));
        assert_eq!(
            apply_growth(&mut world, GrowthTarget::Companion(1), 1).unwrap(),
            SizeClass::Normal
        );
        assert_eq!(
            world.adventure.companion.as_ref().unwrap().age,
            Age::Juvenile
        );
        apply_growth(&mut world, GrowthTarget::Companion(1), 1).unwrap();
        assert_eq!(
            world.adventure.companion.as_ref().unwrap().size,
            SizeClass::Large
        );
        let head = (world.adventure.companion.as_ref().unwrap().pos + Vec3::Y * 2.2)
            .floor()
            .as_ivec3();
        world.set(head, BlockId::Stone);
        world.adventure.companion.as_mut().unwrap().size = SizeClass::Normal;
        assert_eq!(
            apply_growth(&mut world, GrowthTarget::Companion(1), 1),
            Err(GrowthReject::Clearance)
        );
        assert_eq!(
            world.adventure.companion.as_ref().unwrap().size,
            SizeClass::Normal
        );
    }

    #[test]
    fn root_is_standable_and_unroot_spares_foreign_blocks() {
        let mut world = world();
        world.adventure.companion = Some(Companion::juvenile(1, Vec3::new(8.0, 80.0, 8.0)));
        set_rooted(&mut world, 1, true);
        let cells = world
            .adventure
            .companion
            .as_ref()
            .unwrap()
            .root_cells
            .clone();
        assert!(!cells.is_empty());
        assert!(world.solid(cells[0].0));
        let foreign = cells[0].0;
        world.set(foreign, BlockId::Stone);
        set_rooted(&mut world, 1, false);
        assert_eq!(world.get(foreign), BlockId::Stone);
        for (cell, _) in cells.iter().skip(1) {
            assert_eq!(world.get(*cell), BlockId::Air);
        }
    }

    #[test]
    fn success_keeps_a_ring_when_the_pool_is_nearly_full() {
        let mut world = world();
        for i in 0..470 {
            world.adventure.spawn_voxel_particle(
                Vec3::ZERO,
                Vec3::Y,
                [1.0, 1.0, 1.0],
                0.02,
                0.2,
                1.0,
                1.0,
                2.0 + (i % 3) as f32,
            );
        }
        emit_world(
            &mut world.adventure,
            GrowthEvent::Success,
            Vec3::new(0.0, 2.0, 0.0),
        );
        // The pool caps at 480. The ring is written first, so the ten free
        // slots are ring motes and the burst is dropped.
        assert_eq!(world.adventure.particles.len(), 480);
        let ring = &world.adventure.particles[470..];
        assert_eq!(ring.len(), 10);
        assert!(ring.iter().all(|p| {
            (p.pos.y - 2.0).abs() < 0.05
                && (p.pos.x * p.pos.x + p.pos.z * p.pos.z - 0.45 * 0.45).abs() < 0.02
        }));
    }

    #[test]
    fn conjure_then_growth_enlarges_only_the_created_id() {
        let mut world = world();
        let id = resolve_effect(&mut world, 36, Vec3::new(3.0, 80.0, 3.0), None).unwrap();
        resolve_effect(&mut world, 35, Vec3::new(3.0, 80.0, 3.0), Some(id)).unwrap();
        let c = world.adventure.companion.as_ref().unwrap();
        assert_eq!(c.id, id);
        assert_eq!(c.size, SizeClass::Normal);
        assert_eq!(c.age, Age::Juvenile);
    }
}
