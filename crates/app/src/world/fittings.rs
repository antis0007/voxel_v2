//! Player-built metre-grid doors, hatches and local mechanical switches.
use super::inventory::{Item, ItemStack};
use super::{BlockId as B, World, effects, player_aabb};
use engine_content::fittings::{self as shape, Switch};
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
    s.upper = !s.door && s.switch.is_none() && player.eye(world.voxel_scale).y < a.y as f32 + n as f32 * 0.5;
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
            && s.switch.is_none()
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
    match s.switch {
        Some(Switch::Plate) if !s.open => return true,
        Some(_) => signal(world, p, !s.open, player, blocks, chunks),
        None => {
            let mut edits = body(world, p);
            set_open(&mut edits, !s.open);
            commit(world, &edits, player, blocks, chunks);
        }
    }
    true
}

fn set_open(edits: &mut [(IVec3, B)], open: bool) {
    for (_, block) in edits {
        let state = shape::state(*block).unwrap();
        *block = shape::block(shape::State { open, ..state });
    }
}

/// The same three-metre linkage serves every mechanical input. Commit the
/// switch and all outputs together so their saved states agree after a retry.
fn signal(
    world: &mut World,
    p: IVec3,
    active: bool,
    player: &crate::player::Player,
    blocks: &mut HashSet<IVec3>,
    chunks: &mut HashSet<IVec3>,
) {
    let n = shape::step(world.voxel_scale.meters());
    let anchor = shape::anchor(p, n);
    let mut edits = body(world, p);
    for y in -3..=3 {
        for z in -3..=3 {
            for x in -3..=3 {
                if x * x + y * y + z * z > 9 {
                    continue;
                }
                let target = anchor + IVec3::new(x, y, z) * n;
                if shape::state(world.get(target)).is_some_and(|s| s.switch.is_none() && (!s.door || !s.upper) && s.open != active) {
                    edits.extend(body(world, target));
                }
            }
        }
    }
    set_open(&mut edits, active);
    if commit(world, &edits, player, blocks, chunks) {
        world.adventure.sounds.push(crate::audio::Event {
            cue: crate::audio::Cue::Clang,
            pos: Some((anchor.as_vec3() + glam::Vec3::splat(n as f32 * 0.5)) * world.voxel_scale.meters()),
            gain: 0.55,
            pitch: if active { 1.15 } else { 0.85 },
        });
    }
}

fn commit(
    world: &mut World,
    edits: &[(IVec3, B)],
    player: &crate::player::Player,
    blocks: &mut HashSet<IVec3>,
    chunks: &mut HashSet<IVec3>,
) -> bool {
    if overlaps(world, edits, player) {
        world.expedition.say("Opening obstructed");
        return false;
    }
    if effects::fitting_batch(world, edits, blocks, chunks).is_err() {
        world.expedition.say("Mechanism unavailable");
        return false;
    }
    true
}

pub fn step(
    world: &mut World,
    player: &crate::player::Player,
    blocks: &mut HashSet<IVec3>,
    chunks: &mut HashSet<IVec3>,
) {
    if !player.on_ground || player.flying || player.anchor.is_some() || player.up_dir().y < 0.99
        || world.adventure.is_dead() || !world.rules.mode.interacts() || world.ca_view.is_some()
    {
        return;
    }
    let v = world.voxel_scale.meters();
    let p = (player.pos - glam::Vec3::Y * (0.06 / v)).floor().as_ivec3();
    if world.get(p) != B::PressurePlate {
        return;
    }
    let n = shape::step(v);
    let local = (player.pos - shape::anchor(p, n).as_vec3()) / n as f32;
    if (0.04..=0.96).contains(&local.x) && (0.04..=0.96).contains(&local.z)
        && (0.02..=0.15).contains(&local.y)
    {
        signal(world, p, true, player, blocks, chunks);
    }
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
