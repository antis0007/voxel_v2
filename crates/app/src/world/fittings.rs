//! Player-built metre-grid doors, hatches and local mechanical switches.
use super::inventory::{Item, ItemStack};
use super::{BlockId as B, World, effects, player_aabb};
use engine_content::fittings as shape;
use glam::IVec3;
use std::collections::HashSet;

fn body(world: &World, p: IVec3) -> Vec<(IVec3, B)> {
    let b = world.get(p);
    let Some(s) = shape::state(b) else {
        return Vec::new();
    };
    let n = shape::step(world.voxel_scale.meters());
    let mut a = shape::anchor(p, n);
    if s.door && s.upper {
        a.y -= n;
    }
    let mut out = Vec::new();
    for y in 0..if s.door { 2 * n } else { n } {
        for z in 0..n {
            for x in 0..n {
                let q = a + IVec3::new(x, y, z);
                let mut expected = s;
                if s.door {
                    expected.upper = y >= n;
                }
                if world.get(q) == shape::block(expected) {
                    out.push((q, world.get(q)));
                }
            }
        }
    }
    out
}

fn overlaps(world: &World, edits: &[(IVec3, B)], player: &crate::player::Player) -> bool {
    let (lo, hi) = player_aabb(player.pos, world.voxel_scale, player.scale, player.crouch_t);
    edits.iter().any(|&(p, b)| {
        shape::boxes(b, p, world.voxel_scale.meters())
            .iter()
            .any(|&(a, b)| lo.cmplt(b).all() && hi.cmpgt(a).all())
    })
}

pub fn place(
    world: &mut World,
    p: IVec3,
    b: B,
    player: &crate::player::Player,
    slot: Option<usize>,
    blocks: &mut HashSet<IVec3>,
    chunks: &mut HashSet<IVec3>,
) -> bool {
    let Some(mut s) = shape::state(b) else {
        return false;
    };
    let item = Item::Block(shape::canonical(b).unwrap());
    if slot
        .is_some_and(|i| !world.inventory.hotbar[i].is_some_and(|s| s.item == item && s.count > 0))
    {
        return true;
    }
    let n = shape::step(world.voxel_scale.meters());
    let a = shape::anchor(p, n);
    s.z = player.forward_dir().x.abs() > player.forward_dir().z.abs();
    s.open = false;
    s.upper = !s.door && !s.lever && player.eye(world.voxel_scale).y < a.y as f32 + n as f32 * 0.5;
    let mut edits = Vec::new();
    for y in 0..if s.door { 2 * n } else { n } {
        for z in 0..n {
            for x in 0..n {
                let q = a + IVec3::new(x, y, z);
                let mut part = s;
                if s.door {
                    part.upper = y >= n;
                }
                edits.push((q, shape::block(part)));
            }
        }
    }
    let support = (0..n).any(|x| (0..n).any(|z| world.solid(a + IVec3::new(x, -1, z))))
        || (!s.door
            && !s.lever
            && [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z]
                .iter()
                .any(|d| world.solid(a + *d * n)));
    if !support
        || edits.iter().any(|&(q, _)| world.get(q) != B::Air)
        || overlaps(world, &edits, player)
    {
        world.expedition.say("Needs support and a clear opening");
        return true;
    }
    // A metre assembly can cross the streamed air/floor chunk boundary.
    let owners: HashSet<_> = edits
        .iter()
        .map(|&(p, _)| super::chunk_coord_from_world_block(p))
        .collect();
    if owners.into_iter().any(|c| !world.admit_edit_chunk(c)) {
        world.expedition.say("Opening unavailable; item retained");
        return true;
    }
    if effects::fitting_batch(world, &edits, blocks, chunks).is_err() {
        world.expedition.say("Opening unavailable; item retained");
        return true;
    }
    if let Some(i) = slot {
        world.inventory.consume_from_hotbar(i, item);
        world.sync_from_inventory();
    }
    true
}

pub fn interact(
    world: &mut World,
    p: IVec3,
    player: &crate::player::Player,
    blocks: &mut HashSet<IVec3>,
    chunks: &mut HashSet<IVec3>,
) -> bool {
    let Some(s) = shape::state(world.get(p)) else {
        return false;
    };
    if (p.as_vec3() - player.eye(world.voxel_scale)).length() * world.voxel_scale.meters() > 6.0 {
        return false;
    }
    let mut targets = vec![p];
    if s.lever {
        let n = shape::step(world.voxel_scale.meters());
        let a = shape::anchor(p, n);
        for y in -3..=3 {
            for z in -3..=3 {
                for x in -3..=3 {
                    if x * x + y * y + z * z > 9 {
                        continue;
                    }
                    let q = a + IVec3::new(x, y, z) * n;
                    if shape::state(world.get(q)).is_some_and(|t| !t.lever && (!t.door || !t.upper))
                    {
                        targets.push(q);
                    }
                }
            }
        }
    }
    for q in targets {
        let mut edits = body(world, q);
        for (_, b) in &mut edits {
            let mut t = shape::state(*b).unwrap();
            t.open = if s.lever { !s.open } else { !t.open };
            *b = shape::block(t);
        }
        if overlaps(world, &edits, player) {
            world.expedition.say("Opening obstructed");
            continue;
        }
        if effects::fitting_batch(world, &edits, blocks, chunks).is_err() {
            world.expedition.say("Mechanism unavailable");
        }
    }
    true
}

pub fn harvest(
    world: &mut World,
    p: IVec3,
    blocks: &mut HashSet<IVec3>,
    chunks: &mut HashSet<IVec3>,
) -> bool {
    let Some(b) = shape::canonical(world.get(p)) else {
        return false;
    };
    let stack = ItemStack::new(Item::Block(b), 1);
    if world.inventory.remaining_capacity_for(stack.item) < 1 {
        world.expedition.say("Inventory full");
        return true;
    }
    let edits: Vec<_> = body(world, p)
        .into_iter()
        .map(|(q, _)| (q, B::Air))
        .collect();
    if effects::fitting_batch(world, &edits, blocks, chunks).is_ok() {
        world.inventory.try_insert(stack);
        world.sync_from_inventory();
    }
    true
}
