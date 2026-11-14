//! Material-weighted bond cuts. Fracture changes ownership, never voxel mass.
use super::contact::Impact;
use crate::simulation::sandbox::{MAX_BODIES, MAX_BODY_VOXELS, RigidBody};
use crate::world::BlockId;
use glam::{IVec3, Quat, Vec3};

pub(super) fn split(body: &mut RigidBody, others: &mut Vec<RigidBody>, impact: Impact) -> bool {
    let limit = if body.felled_tree { super::super::sandbox::MAX_TREE_BODY_VOXELS } else { MAX_BODY_VOXELS };
    if body.voxels.len() < 4
        || body.voxels.len() > limit
        || impact.speed < 3.0
        || !impact.energy.is_finite()
        || others.len() + 1 >= MAX_BODIES
    {
        return false;
    }
    let Some(hit) = body.occupied.iter().position(|(p, _)| *p == impact.cell) else {
        return false;
    };
    let indices: crate::fast_hash::FastMap<_, _> = body
        .voxels
        .iter()
        .enumerate()
        .map(|(i, (p, _))| (*p, i))
        .collect();
    let mut lo = body.voxels[0].0;
    let mut hi = lo;
    for &(p, _) in &body.voxels {
        lo = lo.min(p);
        hi = hi.max(p);
    }
    let mut best: Option<(f32, usize, i32)> = None;
    for axis in 0..3 {
        let hit_coord = body.voxels[hit].0[axis];
        let candidates = [
            lo[axis] + (hi[axis] - lo[axis]) / 2,
            hit_coord,
            hit_coord - 1,
        ];
        for (index, &cut) in candidates.iter().enumerate() {
            if cut < lo[axis] || cut >= hi[axis] || candidates[..index].contains(&cut) {
                continue;
            }
            let mut cost = 0.0;
            let mut bonds = 0;
            for &(p, a) in &body.voxels {
                if p[axis] != cut {
                    continue;
                }
                let mut q = p;
                q[axis] += 1;
                if let Some(&i) = indices.get(&q) {
                    let b = body.voxels[i].1;
                    let hardness = a
                        .material()
                        .physics
                        .hardness
                        .min(b.material().physics.hardness)
                        .max(0.05);
                    cost += 2.0 + hardness * hardness * 10.0;
                    bonds += 1;
                }
            }
            // Prefer weak necks near the strike, rather than slicing through a
            // wide, distant section simply because its bounding box is longer.
            let score = cost * (1.0 + (cut - hit_coord).unsigned_abs() as f32 * 0.12);
            if bonds > 0 && impact.energy > score.max(6.0) && best.is_none_or(|b| score < b.0) {
                best = Some((score, axis, cut));
            }
        }
    }
    let Some((_, axis, cut)) = best else {
        return false;
    };
    let mut visited = vec![false; body.voxels.len()];
    let mut groups: Vec<Vec<(IVec3, BlockId)>> = Vec::new();
    for seed in 0..body.voxels.len() {
        if visited[seed] {
            continue;
        }
        let side = body.voxels[seed].0[axis] <= cut;
        let mut queue = vec![seed];
        let mut cells = Vec::new();
        visited[seed] = true;
        let mut cursor = 0;
        while cursor < queue.len() {
            let index = queue[cursor];
            cursor += 1;
            cells.push(body.occupied[index]);
            let p = body.voxels[index].0;
            for direction in super::DIRS {
                if let Some(&index) = indices.get(&(p + direction)) {
                    if !visited[index] && (body.voxels[index].0[axis] <= cut) == side {
                        visited[index] = true;
                        queue.push(index);
                    }
                }
            }
        }
        groups.push(cells);
    }
    if groups.len() < 2 || others.len() + groups.len() > MAX_BODIES {
        return false;
    }
    // Stable largest-first admission retains the main assembly as the parent.
    groups.sort_by_key(|g| std::cmp::Reverse(g.len()));
    let old_center = body.occupied_center();
    let mut fragments = groups
        .into_iter()
        .map(|cells| {
            let origin = cells[0].0;
            let mut fragment = RigidBody {
                felled_tree: body.felled_tree,
                origin,
                position: Vec3::ZERO,
                velocity: body.velocity,
                rotation: Quat::IDENTITY,
                angular: body.angular,
                debt: body.debt,
                settle_time: 0.0,
                was_submerged: body.was_submerged,
                voxels: cells.iter().map(|&(p, b)| (p - origin, b)).collect(),
                occupied: cells,
            };
            let (_, center, _) = fragment.mass_properties();
            fragment.velocity += body.angular.cross(origin.as_vec3() + center - old_center);
            fragment
        })
        .collect::<Vec<_>>();
    *body = fragments.remove(0);
    others.extend(fragments);
    true
}
