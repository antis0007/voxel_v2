//! Reservation and interruption checks through the production transfer functions.
use super::*;
use anyhow::{Result, ensure};
use crate::world::{block_data, inventory::{Item, ItemStack}};
use super::super::{State as Jobs, construction::{self, Order}, crew};

pub(super) fn run(root: &std::path::Path) -> Result<()> {
    let mut w = World::with_seed_in(crate::world::WorldType::Superflat,
        crate::fluid::VoxelScalePreset::OneMeter, 8, &root.join("crew-reservations"));
    let y = engine_worldgen::terrain_height(&w.sample_config(), 0, 0) + 3;
    let pile = IVec3::new(1, y, 1);
    w.set(pile, B::Chest);
    let item = Item::Block(B::Log);
    let mut supplied = ItemStack::new(item, 3);
    block_data::store_hauled(&mut w, pile, &mut supplied);
    ensure!(supplied.count == 0, "Crew fixture storage");
    let orders: Vec<_> = (4..7).map(|x| Order::new(IVec3::new(x, y, 4), B::Log)).collect();
    for o in &orders { w.set(o.cell() - IVec3::Y, B::Stone); w.set(o.cell(), B::Air); }
    let mut j = Jobs { worker: Some(0), stockpile: Some(pile.to_array()), build: orders.clone(), ..Jobs::default() };
    construction::fetch(&mut w, &mut j, pile, orders[0]);
    ensure!(j.carry.is_some_and(|s| s.count == 3) && j.claims.len() == 3, "Paid claims missing");
    let cargo = j.carry;
    construction::fetch(&mut w, &mut j, pile, orders[0]);
    ensure!(j.carry == cargo && j.claims.len() == 3, "Repeated fetch changed cargo");
    // A competing worker cannot spend the first worker's cargo or claim a cell.
    let mut owner = crew::Worker::default();
    owner.swap(&mut j);
    j.worker = Some(1);
    construction::fetch(&mut w, &mut j, pile, orders[0]);
    ensure!(j.carry.is_none() && j.claims.len() == 3, "Competing fetch stole claims");
    owner.swap(&mut j);
    j.helpers.push(owner);
    ensure!(j.valid(), "Claimed jobs invalid");
    w.farming.jobs = j;
    let bytes = crate::world::farming::sidecar::encode(&w);
    let (loaded, _) = crate::world::farming::sidecar::decode(&bytes)?;
    w.farming = loaded;
    let mut j = std::mem::take(&mut w.farming.jobs);
    ensure!(j.claims.len() == 3 && j.carry == cargo && j.valid(), "Claims lost on reload");
    // A player edit wins; rejected placement consumes nothing and preserves it.
    w.set(orders[0].cell(), B::DressedStone);
    construction::place(&mut w, &mut j, orders[0], Vec3::splat(-100.0));
    ensure!(w.get(orders[0].cell()) == B::DressedStone && j.carry == cargo,
        "Player edit overwritten or rejected placement charged");
    w.set(orders[0].cell(), B::Air);
    construction::place(&mut w, &mut j, orders[0], Vec3::splat(-100.0));
    ensure!(w.get(orders[0].cell()) == B::Log && j.carry.is_some_and(|s| s.count == 2)
        && j.claims.len() == 2, "Claim commit did not debit once");
    let after = j.carry;
    construction::place(&mut w, &mut j, orders[0], Vec3::splat(-100.0));
    ensure!(j.carry == after, "Repeated placement charged twice");
    j.build.clear();
    j.reconcile_claims();
    ensure!(j.claims.is_empty() && j.carry == after, "Cancellation lost physical cargo");
    let mut returned = j.carry.take().ok_or_else(|| anyhow::anyhow!("Missing returned cargo"))?;
    block_data::store_hauled(&mut w, pile, &mut returned);
    ensure!(returned.count == 0 && w.block_data.chest(pile).iter().flatten().map(|s| s.count).sum::<u32>() == 2,
        "Cancellation return did not conserve three paid blocks");
    println!("[city-check] PASS exclusive claims, repeated fetch/commit, player edit, reload and cancellation conservation");
    full_roster(root)
}

fn full_roster(root: &std::path::Path) -> Result<()> {
    let mut world = World::with_seed_in(crate::world::WorldType::Superflat,
        crate::fluid::VoxelScalePreset::OneMeter, 42, &root.join("full-roster"));
    let y = engine_worldgen::terrain_height(&world.sample_config(), 0, 0) + 2;
    let pile = IVec3::new(4, y, 4);
    let feet = Vec3::new(1.5, y as f32, 1.5);
    let mut player = crate::player::Player::new();
    player.pos = feet;
    player.precise_pos = feet.as_dvec3();
    let floor: Vec<_> = (-2..=20).flat_map(|x| (-2..=14).flat_map(move |z| [
        (IVec3::new(x, y - 1, z), B::Stone),
        (IVec3::new(x, y, z), B::Air),
        (IVec3::new(x, y + 1, z), B::Air),
    ])).collect();
    world.set_many(&floor);
    world.set(pile, B::Chest);
    crate::world::adventure::wildlife::vehicle::spawn_resident(&mut world, 0, feet + Vec3::X);
    super::super::assign(&mut world, 0);
    super::super::set_priorities(&mut world, 0, [0; 3]);
    ensure!(super::super::designate(&mut world, pile), "Crew stockpile designation");
    let recruits = crew::recruit(&mut world, feet).map_err(anyhow::Error::msg)?;
    ensure!(recruits == 10 && crew::count(&world) == 11, "Ten constructor recruits required");
    let orders: Vec<_> = (9..17).flat_map(|x| (1..11).map(move |z|
        Order::new(IVec3::new(x, y, z), B::Planks))).collect();
    ensure!(orders.iter().all(|order| order.ready(&world)), "Roster blueprint requires supported air cells");
    let mut material = ItemStack::new(Item::Block(B::Planks), orders.len() as u32);
    block_data::store_hauled(&mut world, pile, &mut material);
    ensure!(material.count == 0, "Construction bill storage");
    world.farming.jobs.build = orders.clone();
    let mut input = crate::player::Input::default();
    let mut checkpointed = false;
    let mut builders = std::collections::BTreeSet::new();
    let started = std::time::Instant::now();
    for _ in 0..12_000 {
        crate::simulation::fixed::gameplay(&mut world, &mut player, &mut input, false, |_, _, _, _|
            crate::simulation::fixed::Actions::default());
        world.adventure.dirty_voxels.clear();
        let jobs = &world.farming.jobs;
        let workers = crew::saved_workers(&world);
        builders.extend(workers.iter().map(|&(id, _)| id).filter(|&id|
            crew::view(jobs, id).is_some_and(|worker| worker.last_placement.is_some())));
        if !checkpointed && jobs.claims.len() >= 12 {
            let (restored, _) = crate::world::farming::sidecar::decode(&crate::world::farming::sidecar::encode(&world))?;
            ensure!(ron::to_string(&restored.jobs)? == ron::to_string(&jobs.checkpoint(&world.adventure.enemies, world.farming.clock))?, "Crew cargo and orders changed on reload");
            world.farming = restored;
            checkpointed = true;
        }
        if world.farming.jobs.build.is_empty() { break; }
    }
    let remaining = world.block_data.chest(pile).into_iter().flatten()
        .filter(|stack| stack.item == Item::Block(B::Planks)).map(|stack| stack.count).sum::<u32>();
    ensure!(checkpointed && builders.len() == recruits && world.farming.jobs.valid()
        && orders.iter().all(|order| world.get(order.cell()) == B::Planks)
        && crew::cargo(&world.farming.jobs) == 0 && remaining == 0,
        "Full roster: {} workers built, {} orders left, stock={remaining}, cargo={}, checkpoint={checkpointed}, workers={:?}",
        builders.len(), world.farming.jobs.build.len(), crew::cargo(&world.farming.jobs),
        crew::saved_workers(&world).iter().map(|&(id, at)| (at, super::super::status_for(&world, Some(id)))).collect::<Vec<_>>());
    println!("[city-check] PASS constructors={recruits} builders={} blocks={} ticks={} wall_ms={:.1}",
        builders.len(), orders.len(), world.farming.clock, started.elapsed().as_secs_f64() * 1000.0);
    Ok(())
}
