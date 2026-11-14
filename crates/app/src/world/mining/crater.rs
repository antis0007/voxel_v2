//! Rock stress follows connected terrain; only exposed pieces become bodies.
use super::{Tool, ToolClass, harvest_cell, survival_break};
use crate::world::{BlockId as B, World, effects};
use glam::{IVec3, Vec3};
use std::collections::{HashSet, VecDeque};

const FACES: [IVec3; 6] = [IVec3::Y, IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z, IVec3::NEG_Y];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cracked {
    pub mined: usize,
    pub debris: usize,
}

pub(super) fn rock(b: B) -> bool {
    matches!(b, B::Stone | B::Sandstone | B::Basalt | B::Limestone)
}

fn eligible(world: &World, p: IVec3, tool: Tool) -> Option<B> {
    effects::admit(world, p).ok()?;
    let b = world.get(p);
    let matches_tool = match tool.class {
        Some(ToolClass::Dig) => rock(b),
        Some(ToolClass::Scoop) => matches!(b, B::Grass | B::Dirt | B::Sand | B::Gravel | B::Clay | B::Mud | B::SnowGrass),
        _ => false,
    };
    (matches_tool && survival_break(b, tool).drops).then_some(b)
}

fn opening(world: &World, p: IVec3) -> Option<IVec3> {
    FACES.into_iter().find(|&d| effects::admit(world, p + d).is_ok() && world.get(p + d) == B::Air)
}

fn rubble(world: &mut World, p: IVec3, material: B, tool: Tool, speed: f32) -> bool {
    if world.sandbox.bodies.len() >= crate::simulation::sandbox::MAX_BODIES - 16 { return false; }
    let Some(exit) = opening(world, p) else { return false };
    let mut cells = vec![(p, material)];
    if speed > 2.0 {
        for d in FACES {
            let q = p + d;
            if d.dot(exit) == 0 && eligible(world, q, tool) == Some(material)
                && effects::admit(world, q + exit).is_ok() && world.get(q + exit) == B::Air {
                cells.push((q, material));
                break;
            }
        }
    }
    let direction = (exit.as_vec3() + Vec3::Y * 0.18).normalize_or_zero();
    let density = (material.material().physics.density / 1000.0).max(0.5);
    let velocity = direction * (speed / density.sqrt() / world.voxel_scale.meters());
    if !world.sandbox.add_debris(p, &cells, velocity) { return false; }
    if let Some(body) = world.sandbox.bodies.last_mut() {
        body.angular = Vec3::Y.cross(direction) * 1.1;
    }
    true
}

fn burst(world: &mut World, p: IVec3, b: B, count: usize, speed: f32) {
    let scale = world.voxel_scale.meters();
    world.adventure.spawn_voxel_burst(
        (p.as_vec3() + Vec3::splat(0.5)) * scale, b.color(p), count, speed,
        (scale * 0.15).clamp(0.018, 0.075),
    );
}

pub(super) fn chip(world: &mut World, at: IVec3, material: B, tool: Tool,
    blocks: &mut HashSet<IVec3>, chunks: &mut HashSet<IVec3>) {
    burst(world, at, material, 8, 2.1);
    let mut released = false;
    for d in FACES {
        let p = at + d;
        let Some(b) = eligible(world, p, tool) else { continue };
        let hardness = b.material().physics.hardness.max(0.25);
        let stress = (0.45 + 0.07 * tool.tier as f32) / (0.55 + hardness * 0.65);
        if world.mining_fractures.stress(p, b, stress.clamp(0.18, 0.84)) && !released {
            released = rubble(world, p, b, tool, 1.8) || harvest_cell(world, p, Some(tool), blocks, chunks, Vec3::ZERO);
            if released {
                world.mining_fractures.0.retain(|(cell, ..)| *cell != p);
                burst(world, p, b, 5, 1.6);
            }
        }
    }
}

pub fn can_crack(world: &World, at: IVec3, tool: Tool) -> bool {
    eligible(world, at, tool).is_some()
}

/// A charged hit cuts into the face, with material hardness and an irregular
/// seam deciding which connected cells give way. Work is bounded per hit.
pub fn crack(world: &mut World, at: IVec3, radius_m: f32, tool: Tool,
    blocks: &mut HashSet<IVec3>, chunks: &mut HashSet<IVec3>) -> Cracked {
    let Some(material) = eligible(world, at, tool) else { return Cracked::default() };
    if !radius_m.is_finite() || radius_m <= 0.0 { return Cracked::default(); }
    let radius = (radius_m / world.voxel_scale.meters()).clamp(1.0, 6.0);
    let budget = (radius_m * 8.0).ceil().clamp(4.0, 22.0) as usize;
    let mut queue = VecDeque::from([at]);
    let mut seen = HashSet::from([at]);
    let mut core = Vec::new();
    let mut rim = Vec::new();
    while let Some(p) = queue.pop_front() {
        let Some(b) = eligible(world, p, tool) else { continue };
        let d = p - at;
        let seam = engine_core::hash::hash_u32((p.x as u32).wrapping_mul(73856093)
            ^ (p.y as u32).wrapping_mul(19349663) ^ (p.z as u32).wrapping_mul(83492791));
        let resistance = 0.84 + b.material().physics.hardness * 0.16 + (seam & 15) as f32 * 0.022;
        if p != at && (d.as_vec3().length() * resistance > radius * 0.72 || core.len() >= budget) {
            rim.push(p);
            continue;
        }
        core.push(p);
        for face in FACES {
            let q = p + face;
            if (q - at).length_squared() as f32 <= radius * radius && seen.insert(q) {
                queue.push_back(q);
            }
        }
    }
    let mut out = Cracked::default();
    for p in core {
        if harvest_cell(world, p, Some(tool), blocks, chunks, Vec3::ZERO) { out.mined += 1; }
        else if p == at { return out; }
    }
    for p in rim {
        let Some(b) = eligible(world, p, tool) else { continue };
        if opening(world, p).is_none() { continue; }
        if out.debris < 4 && rubble(world, p, b, tool, 2.7) {
            out.debris += 1;
        } else {
            world.mining_fractures.stress(p, b, 0.58);
        }
    }
    burst(world, at, material, 22, 3.8);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fluid::VoxelScalePreset;
    use crate::world::WorldType;

    #[test]
    fn crater_mines_core_and_detaches_rim() {
        let mut world = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 3);
        let at = IVec3::new(0, 10, 0);
        let slab: Vec<_> = (-4..=4)
            .flat_map(|x| (6..=12).flat_map(move |y| (-4..=4).map(move |z| (IVec3::new(x, y, z), B::Stone))))
            .collect();
        world.set_many(&slab);
        let (mut blocks, mut chunks) = (HashSet::new(), HashSet::new());
        let cracked = crack(&mut world, at, 3.0, Tool::of(crate::world::inventory::Item::Tool(8)).unwrap(), &mut blocks, &mut chunks);
        assert!(cracked.mined > 0, "core is mined");
        assert!(cracked.debris > 0, "rim detaches");
        assert!(world.sandbox.bodies.iter().all(|b| b.voxels.len() <= 8));
    }
}
