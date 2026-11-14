//! World-facing machines: exact volume accounting, bounded edits, and backpressure.
use super::*;
use crate::player::Player;
use crate::simulation::residency::Residency;
use crate::world::chunk_coord_from_world_block;
use crate::world::effects::{self, Failure, WaterDirection};
use crate::simulation::Budget;

pub const WATER_CAPACITY: u32 = 8192;

/// Machines that consume piped water: washers, and piles for coolant.
fn wants_coolant(n: &super::Node) -> bool {
    matches!(n.kind, Kind::Washer | Kind::WetScrubber)
        || (n.kind == Kind::Cultivator && super::biomatter::recipe(n.mode).is_some_and(|r| r.water > 0))
        || super::pile::is_pile(n) || super::industry::boiler::active(n)
}

pub(super) fn route_water(factory: &mut Factory) {
    // A source has one rate allowance across all of its attached networks.
    let mut moved = HashSet::new();
    for network in &factory.fluids {
        let mut flow = false;
        for &p in &network.ports {
            let source = &factory.nodes[&p];
            if !source.enabled
                || !matches!(source.kind, Kind::Pump | Kind::Tank | Kind::GasGenerator)
                || moved.contains(&p)
            {
                continue;
            }
            let mut budget = source.water.min(128);
            let source_kind = source.kind;
            let mut targets = network.ports.clone();
            targets.sort_by_key(|q| {
                (
                    if wants_coolant(&factory.nodes[q]) {
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
                    .filter(|q| wants_coolant(&factory.nodes[q]))
                    .count();
                if washer_count > 0 {
                    targets[..washer_count].rotate_left(factory.cursor % washer_count);
                }
            }
            for q in targets {
                let dest = &factory.nodes[&q];
                if p == q
                    || !dest.enabled
                    || !(wants_coolant(dest)
                        || (dest.kind == Kind::Tank && matches!(source_kind, Kind::Pump | Kind::GasGenerator)))
                {
                    continue;
                }
                let unit = u64::from(industry::WATER_UNIT_MG);
                let mass = u64::from(dest.water) * unit + u64::from(dest.separator_water_mg);
                let room = (u64::from(WATER_CAPACITY) * unit).saturating_sub(mass) / unit;
                let amount = budget.min(room as u32);
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

/// Battery banks in Mana font mode pour their charge into the wielder's mana
/// while they stand within six metres: 1 mana costs 1 kJ (4 units), at up to
/// 20 mana per second from the nearest font.
fn mana_fonts(world: &mut World, player_m: Vec3, dt: f32, size: i32) {
    let scale = world.voxel_scale.meters();
    let deficit = world.adventure.max_mana() - world.adventure.mana;
    if deficit <= 0.0 {
        return;
    }
    let nearest = world
        .factory
        .nodes
        .iter()
        .filter(|(_, n)| n.kind == Kind::Battery && n.enabled && n.mode == 3 && n.energy >= 4)
        .map(|(p, _)| {
            let centre = (p.as_vec3() + Vec3::splat(size as f32 * 0.5)) * scale;
            (*p, (centre - player_m).length())
        })
        .filter(|(_, d)| *d <= 6.0)
        .min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.to_array().cmp(&b.0.to_array())));
    let Some((p, _)) = nearest else {
        return;
    };
    let n = world.factory.nodes.get_mut(&p).unwrap();
    let budget = n.work_clock + 20.0 * dt * 4.0;
    let units = (budget.floor() as u32).min(n.energy).min((deficit * 4.0).floor() as u32);
    n.work_clock = budget.fract();
    n.energy -= units;
    world.factory.stats.power(0, units);
    if units > 0 {
        n.activity = 1.0;
    }
    world.adventure.mana = (world.adventure.mana + units as f32 / 4.0).min(world.adventure.max_mana());
}

pub(crate) fn tick_world(
    world: &mut World,
    player: &Player,
    dt: f32,
    sources: &mut Residency,
    cursor: &mut usize,
    deadline: Budget,
    blocks: &mut HashSet<IVec3>,
    chunks: &mut HashSet<IVec3>,
) {
    let size = step(world);

    if dt <= 0.0 {
        return;
    }
    let scale = world.voxel_scale.meters();
    mana_fonts(world, player.pos * scale, dt, size);
    let (lo, hi) =
        super::super::player_aabb(player.pos, world.voxel_scale, player.scale, player.crouch_t);
    // Reuses one buffer: this runs every fixed step.
    let mut machines = std::mem::take(&mut world.factory.machine_scratch);
    machines.clear();
    machines.extend(
        world
            .factory
            .nodes
            .iter()
            .filter(|(_, n)| {
                ((n.kind.excavates() || n.kind == Kind::Pump) && n.enabled)
                    || (n.kind.excavates() && n.blocks.output.iter().any(|&count| count > 0))
                    || (n.kind == Kind::Quarry && n.placer.has_feed())
                    || (n.kind == Kind::Washer && (n.output[GOLD_CONCENTRATE] > 0
                        || n.blocks.output[blocks::Material::Sand.index()] > 0))
                    || (n.kind.processor() && (n.enabled || super::chest_io::has_output(n)))
                    || (n.kind == Kind::Furnace && n.blocks.output[blocks::Material::GoldBlock.index()] > 0)
                    || (n.kind == Kind::Assembler && (n.enabled || super::assembler::has_output(n)) && n.target != 0)
                    || (n.draining && n.kind.water_port())
            })
            .map(|(&p, _)| p),
    );
    machines.sort_by_key(|p| (p.x, p.y, p.z));
    if !machines.is_empty() {
        let rotation = *cursor % machines.len();
        machines.rotate_left(rotation);
    }
    // Accumulate every enabled machine before the bounded service pass, so
    // distant or delayed machines retain time credit instead of losing it.
    for p in &machines {
        let n = world.factory.nodes.get_mut(p).unwrap();
        if n.enabled || n.draining || super::chest_io::has_output(n) {
            n.work_clock = (n.work_clock + dt).min(0.5);
        }
    }
    for &p in &machines {
        if !deadline.has_time() {
            break;
        }
        *cursor = cursor.wrapping_add(1);
        let mut n = world.factory.nodes.remove(&p).unwrap();
        if n.kind == Kind::Assembler {
            super::assembler::chests(world, p, &mut n, size, deadline);
            world.factory.nodes.insert(p, n);
            continue;
        }
        if n.kind.processor() {
            super::chest_io::transfer(world, p, &mut n, size, deadline);
        }
        if matches!(n.kind, Kind::Quarry | Kind::Washer) {
            super::placer::route(world, p, &mut n, size);
        }
        if n.draining {
            drain(world, p, &mut n, sources, deadline, blocks, chunks);
            world.factory.nodes.insert(p, n);
            continue;
        }
        if n.kind.excavates() {
            super::blocks::export(world, p, &mut n, size, deadline);
            if !n.enabled {
                world.factory.nodes.insert(p, n);
                continue;
            }
        }
        if n.kind.processor() {
            super::blocks::export(world, p, &mut n, size, deadline);
            world.factory.nodes.insert(p, n);
            continue;
        }
        if super::industry::blocked(&n) {
            n.status="Exhaust blocked";world.factory.nodes.insert(p,n);continue;
        }
        if !world.factory.chips.allows(p) {
            // The controller's output is off: no intake, and no time credit.
            n.work_clock = 0.0;
            n.status = "Held by controller";
            world.factory.nodes.insert(p, n);
            continue;
        }

        let interval = if n.kind == Kind::Pump {
            0.25 / (n.upgrade + 1) as f32
        } else {
            0.5 / (size * size * size) as f32 / (n.upgrade + 1) as f32
        };
        let mut edits = 0;
        while n.work_clock >= interval && edits < 16 && deadline.has_time() {
            if n.kind.excavates()
                && n.excavation >= super::quarry::limit(size, n.mode, n.target) as u32
            {
                n.status = if n.kind == Kind::MiningDrill && n.mode == 0 {
                    "Shaft complete"
                } else {
                    "Quarry complete"
                };
                break;
            }
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
                let exact = crate::world::environment::State::enabled(world.generator_version);
                let minimum = if exact { 1 } else { quantum };
                if n.water > WATER_CAPACITY - minimum {
                    n.status = "Tank full";
                    break;
                }
                let mut pumped = false;
                let mut waiting = false;
                let mut sediment_blocked = false;
                let mut polluted=false;
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
                        if world.industry.contamination(q,scale)>0 {polluted=true;continue;}
                        let old = sources.read(world, q).unwrap().1;
                        let available = if exact {
                            (world.environment.exact_water(q, old) / effects::WATER_UNIT_UL) as u32
                        } else { u32::from(old) * quantum };
                        let intake = crate::world::environment::inspect::intake_units(world, q);
                        if intake == 0 && old > 0 { sediment_blocked = true; }
                        let take = available.min(u32::from(intake) * quantum)
                            .min(WATER_CAPACITY - n.water).min(budget);
                        if take == 0 {
                            continue;
                        }
                        if !sources.prepare(world, &[q]) {
                            waiting = true;
                            continue;
                        }
                        let actual = match effects::exchange_water(world, q, &mut n.water,
                            WATER_CAPACITY, take, WaterDirection::IntoBuffer) {
                            Ok(commit) => commit.publish(world, blocks, chunks),
                            Err(Failure::Deferred) => { waiting = true; 0 }
                            Err(_) => 0,
                        };
                        if actual > 0 {
                            budget -= actual;
                            pumped = true;
                        }
                        if budget < minimum {
                            break;
                        }
                    }
                    if budget < minimum {
                        break;
                    }
                }
                if pumped {
                    n.energy -= cost;
                    world.factory.stats.power(0, cost);
                    n.status = "Processing";
                } else if waiting {
                    n.work_clock = (n.work_clock + interval).min(0.5);
                    n.status = "Waiting for world data";
                    break;
                } else {
                    n.status = if polluted { "Contaminated intake" } else if sediment_blocked { "Intake clogged: clear sediment or relocate pump" } else { "Intake dry" };
                }
            } else {
                let q = super::quarry::cell(p, &n, size);
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
                let Some((block, _)) = sources.read(world, q) else {
                    n.status = "Waiting for world data";
                    break;
                };
                let product = match super::quarry::product(world, &n, q, block, size) {
                    Ok(Some(product)) => product,
                    Ok(None) => {
                        n.excavation += 1;
                        continue;
                    }
                    Err(status) => {
                        n.status = status;
                        break;
                    }
                };
                if !product.fits(&n) {
                    n.status = "Output blocked";
                    break;
                }
                if !sources.prepare(world, &[q]) {
                    n.status = "Waiting for world data";
                    break;
                }
                match effects::replace(world, q, block, B::Air) {
                    Ok(commit) => { commit.publish(world, blocks, chunks); }
                    Err(Failure::Deferred) => {
                        n.work_clock = (n.work_clock + interval).min(0.5);
                        n.status = "Waiting for excavation";
                        break;
                    }
                    Err(_) => {
                        n.status = "Excavation unavailable";
                        break;
                    }
                }
                product.store(world, &mut n);
                charge_excavation(world, &mut n, size, cost);
            }
        }
        world.factory.nodes.insert(p, n);
    }
    world.factory.machine_scratch = machines;
}

fn charge_excavation(world: &mut World, n: &mut Node, size: i32, cost: u32) {
    n.energy -= cost;
    super::industry::produce(n, cost);
    world.factory.stats.power(0, cost);
    if n.burn == 0 {
        n.burn = (size * size * size) as u32;
    }
    n.burn -= 1;
    n.excavation += 1;
    n.status = "Processing";
}

fn drain(
    world: &mut World,
    p: IVec3,
    n: &mut Node,
    sources: &mut Residency,
    deadline: Budget,
    blocks: &mut HashSet<IVec3>,
    chunks: &mut HashSet<IVec3>,
) {
    let size = step(world);
    let quantum = 64 / (size * size * size) as u32;
    let exact = crate::world::environment::State::enabled(world.generator_version);
    let minimum = if exact { 1 } else { quantum };
    let mut changed = 0;
    let mut waiting = false;
    // Drain through every face into free neighbouring voxels, never replacing solids.
    for d in DIRS {
        for a in 0..size {
            for b in 0..size {
                if changed >= 16 || n.water < minimum || !deadline.has_time() {
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
                // Preview against the prepared source cache before promoting
                // a full destination into editable residency. Commit rechecks.
                let old = sources.read(world, q).unwrap().1;
                let room = if exact {
                    (world.environment.water_capacity_ul(q)
                        .saturating_sub(world.environment.exact_water(q, old)) / effects::WATER_UNIT_UL) as u32
                } else { u32::from(8 - old) * quantum };
                if room.min(n.water) < minimum { continue; }
                if !sources.prepare(world, &[q]) {
                    waiting = true;
                    continue;
                }
                let actual = match effects::exchange_water(world, q, &mut n.water,
                    WATER_CAPACITY, u32::MAX, WaterDirection::IntoWorld) {
                    Ok(commit) => commit.publish(world, blocks, chunks),
                    Err(Failure::Deferred) => { waiting = true; 0 }
                    Err(_) => 0,
                };
                if actual > 0 {
                    changed += 1;
                }
            }
        }
    }
    if n.water == 0 {
        n.draining = false;
        n.status = if n.separator_water_mg > 0 { "Process water remains" } else { "Drained" };
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
