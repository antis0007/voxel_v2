//! World-facing machines: exact volume accounting, bounded edits, and backpressure.
use super::*;
use crate::player::Player;
use crate::simulation::residency::Residency;
use crate::world::chunk_coord_from_world_block;
use std::time::Instant;

pub const WATER_CAPACITY: u32 = 8192;

pub(super) fn route_water(factory: &mut Factory) {
    // A source has one rate allowance across all of its attached networks.
    let mut moved = HashSet::new();
    for network in &factory.fluids {
        let mut flow = false;
        for &p in &network.ports {
            let source = &factory.nodes[&p];
            if !source.enabled
                || !matches!(source.kind, Kind::Pump | Kind::Tank)
                || moved.contains(&p)
            {
                continue;
            }
            let mut budget = source.water.min(128);
            let source_kind = source.kind;
            let mut targets = network.ports.clone();
            targets.sort_by_key(|q| {
                (
                    if factory.nodes[q].kind == Kind::Washer {
                        0
                    } else if factory.nodes[q].kind == Kind::Tank {
                        1
                    } else {
                        2
                    },
                    (q.x, q.y, q.z),
                )
            });
            if !targets.is_empty() {
                // Rotate within each class without moving tanks ahead of washers.
                let washer_count = targets
                    .iter()
                    .filter(|q| factory.nodes[q].kind == Kind::Washer)
                    .count();
                if washer_count > 0 {
                    targets[..washer_count].rotate_left(factory.cursor % washer_count);
                }
            }
            for q in targets {
                let dest = &factory.nodes[&q];
                if p == q
                    || !dest.enabled
                    || !(dest.kind == Kind::Washer
                        || (dest.kind == Kind::Tank && source_kind == Kind::Pump))
                {
                    continue;
                }
                let amount = budget.min(WATER_CAPACITY - dest.water);
                if amount == 0 {
                    continue;
                }
                factory.nodes.get_mut(&p).unwrap().water -= amount;
                factory.nodes.get_mut(&q).unwrap().water += amount;
                budget -= amount;
                flow = true;
                moved.insert(p);
                if budget == 0 {
                    break;
                }
            }
        }
        if flow {
            for p in &network.conduits {
                factory.nodes.get_mut(p).unwrap().activity = 1.0;
            }
        }
    }
}

pub(crate) fn tick_world(
    world: &mut World,
    player: &Player,
    dt: f32,
    sources: &mut Residency,
    cursor: &mut usize,
    deadline: Instant,
    blocks: &mut HashSet<IVec3>,
    chunks: &mut HashSet<IVec3>,
) {
    let size = step(world);

    if dt <= 0.0 {
        return;
    }
    let scale = world.voxel_scale.meters();
    let (lo, hi) =
        super::super::player_aabb(player.pos, world.voxel_scale, player.scale, player.crouch_t);
    let mut machines: Vec<_> = world
        .factory
        .nodes
        .iter()
        .filter(|(_, n)| {
            (matches!(n.kind, Kind::MiningDrill | Kind::Pump) && n.enabled) || n.draining
        })
        .map(|(&p, _)| p)
        .collect();
    machines.sort_by_key(|p| (p.x, p.y, p.z));
    if !machines.is_empty() {
        let rotation = *cursor % machines.len();
        machines.rotate_left(rotation);
    }
    // Accumulate every enabled machine before the bounded service pass, so
    // distant or delayed machines retain time credit instead of losing it.
    for p in &machines {
        let n = world.factory.nodes.get_mut(p).unwrap();
        n.work_clock = (n.work_clock + dt).min(0.5);
    }
    for p in machines {
        if Instant::now() >= deadline {
            break;
        }
        *cursor = cursor.wrapping_add(1);
        let mut n = world.factory.nodes.remove(&p).unwrap();
        if n.draining {
            drain(world, p, &mut n, sources, deadline, blocks, chunks);
            world.factory.nodes.insert(p, n);
            continue;
        }

        let interval = if n.kind == Kind::Pump {
            0.25 / (n.upgrade + 1) as f32
        } else {
            0.5 / (size * size * size) as f32 / (n.upgrade + 1) as f32
        };
        let mut edits = 0;
        while n.work_clock >= interval && edits < 16 && Instant::now() < deadline {
            n.work_clock -= interval;
            edits += 1;
            let cost = if n.kind == Kind::Pump {
                4
            } else if n.burn == 0 {
                16
            } else {
                0
            };
            if n.energy < cost {
                n.status = "Needs cable / power";
                break;
            }
            if n.kind == Kind::Pump {
                // Intake is the one-metre footprint immediately underneath.
                let quantum = 64 / (size * size * size) as u32;
                if n.water > WATER_CAPACITY - quantum {
                    n.status = "Tank full";
                    break;
                }
                let mut pumped = false;
                let mut waiting = false;
                let mut budget = 128;
                for z in 0..size {
                    for x in 0..size {
                        let q = p + IVec3::new(x, -1, z);
                        if q.y < world.sample_config().min_world_y() {
                            continue;
                        }
                        if !sources.request_background(world, chunk_coord_from_world_block(q)) {
                            waiting = true;
                            continue;
                        }
                        let old = sources.read(world, q).unwrap().1;
                        let take = (old as u32)
                            .min((WATER_CAPACITY - n.water) / quantum)
                            .min(budget / quantum) as u8;
                        if take == 0 {
                            continue;
                        }
                        if !sources.prepare(world, &[q]) {
                            waiting = true;
                            continue;
                        }
                        world.set_water_volume(q, old - take);
                        let actual = old.saturating_sub(world.water_volume(q));
                        if actual > 0 {
                            n.water += actual as u32 * quantum;
                            budget -= actual as u32 * quantum;
                            pumped = true;
                            blocks.insert(q);
                            mark_chunk_and_neighbors_dirty(chunks, q);
                        }
                        if budget < quantum {
                            break;
                        }
                    }
                    if budget < quantum {
                        break;
                    }
                }
                if pumped {
                    n.energy -= cost;
                    n.status = "Processing";
                } else if waiting {
                    n.work_clock = (n.work_clock + interval).min(0.5);
                    n.status = "Waiting for world data";
                    break;
                } else {
                    n.status = "Intake dry";
                }
            } else {
                let width = 3 * size;
                let layer = width * width;
                let index = n.excavation as i32;
                if index >= layer * 24 * size {
                    n.status = "Shaft complete";
                    break;
                }
                let q = p + IVec3::new(
                    index % width - size,
                    -1 - index / layer,
                    (index / width) % width - size,
                );
                if q.y < world.sample_config().min_world_y() {
                    n.status = "Bedrock reached";
                    break;
                }
                // Do not excavate player footing or any registered assembly.
                if (q.as_vec3().cmplt(hi + Vec3::splat(0.3 / scale))
                    & (q.as_vec3() + Vec3::ONE).cmpgt(lo - Vec3::new(0.3, 1.0, 0.3) / scale))
                .all()
                {
                    n.status = "Stand clear of shaft";
                    break;
                }
                if world.factory.nodes.contains_key(&anchor(q, size)) {
                    n.status = "Assembly in shaft";
                    break;
                }
                if !sources.request_background(world, chunk_coord_from_world_block(q)) {
                    n.work_clock = (n.work_clock + interval).min(0.5);
                    n.status = "Waiting for world data";
                    break;
                }
                let b = sources.read(world, q).unwrap().0;
                if b == B::Air {
                    n.excavation += 1;
                    continue;
                }
                let resource = match b {
                    B::CopperOre => Some(0),
                    B::TinOre => Some(1),
                    B::IronOre => Some(2),
                    B::CoalOre => Some(3),
                    B::Crystal => Some(5),
                    _ => None,
                };
                if resource.is_none()
                    && !matches!(
                        b,
                        B::Stone
                            | B::Dirt
                            | B::Grass
                            | B::Sand
                            | B::Basalt
                            | B::Gravel
                            | B::Limestone
                    )
                {
                    n.status = "Obstruction in shaft";
                    break;
                }
                let yield_n = 64 / (size * size * size) as u32;
                if resource.is_some_and(|i| n.output[i] > CAPACITY - yield_n) {
                    n.status = "Output blocked";
                    break;
                }
                if !sources.prepare(world, &[q]) {
                    n.status = "Waiting for world data";
                    break;
                }
                world.set(q, B::Air);
                if world.get(q) != B::Air {
                    n.status = "Excavation unavailable";
                    break;
                }
                if let Some(i) = resource {
                    n.output[i] += yield_n;
                }
                n.energy -= cost;
                if n.burn == 0 {
                    n.burn = (size * size * size) as u32;
                }
                n.burn -= 1;
                n.excavation += 1;
                n.status = "Processing";
                blocks.insert(q);
                mark_chunk_and_neighbors_dirty(chunks, q);
            }
        }
        world.factory.nodes.insert(p, n);
    }
}

fn drain(
    world: &mut World,
    p: IVec3,
    n: &mut Node,
    sources: &mut Residency,
    deadline: Instant,
    blocks: &mut HashSet<IVec3>,
    chunks: &mut HashSet<IVec3>,
) {
    let size = step(world);
    let quantum = 64 / (size * size * size) as u32;
    let mut changed = 0;
    let mut waiting = false;
    // Drain through every face into free neighbouring voxels, never replacing solids.
    for d in DIRS {
        for a in 0..size {
            for b in 0..size {
                if changed >= 16 || n.water < quantum || Instant::now() >= deadline {
                    break;
                }
                let q = if d.x != 0 {
                    p + IVec3::new(if d.x > 0 { size } else { -1 }, a, b)
                } else if d.y != 0 {
                    p + IVec3::new(a, if d.y > 0 { size } else { -1 }, b)
                } else {
                    p + IVec3::new(a, b, if d.z > 0 { size } else { -1 })
                };
                if q.y < world.sample_config().min_world_y()
                    || q.y >= world.sample_config().max_world_y()
                {
                    continue;
                }
                if !sources.request_background(world, chunk_coord_from_world_block(q)) {
                    waiting = true;
                    continue;
                }
                if world.factory.nodes.contains_key(&anchor(q, size))
                    || !matches!(sources.read(world, q).unwrap().0, B::Air | B::Water)
                {
                    continue;
                }
                let old = sources.read(world, q).unwrap().1;
                let amount = (n.water / quantum).min((8 - old) as u32) as u8;
                if amount == 0 {
                    continue;
                }
                if !sources.prepare(world, &[q]) {
                    waiting = true;
                    continue;
                }
                world.set_water_volume(q, old + amount);
                let actual = world.water_volume(q).saturating_sub(old);
                n.water -= actual as u32 * quantum;
                if actual > 0 {
                    changed += 1;
                    blocks.insert(q);
                    mark_chunk_and_neighbors_dirty(chunks, q);
                }
            }
        }
    }
    if n.water == 0 {
        n.draining = false;
        n.status = "Drained";
    } else {
        n.status = if changed > 0 {
            "Draining"
        } else if waiting {
            "Waiting for world data"
        } else {
            "Drain obstructed"
        };
    }
}
