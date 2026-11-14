//! Runs the player job actions and normal game ticks in a disposable native scene.
use super::{B, IVec3, ItemStack, Vec3, World, assign, designate, resident, vehicle};
use crate::{
    player::Player,
    world::{
        DayNightCycle, WorldType,
        inventory::{DroppedStack, Item},
        storage,
    },
};
use anyhow::{Result, bail, ensure};

#[derive(Clone, Copy)]
enum Phase {
    Setup,
    Pickup,
    Saving,
    Resume,
    Finish,
}

pub(crate) struct Run {
    phase: Phase,
    building: bool,
    started: std::time::Instant,
}

fn fixture(world: &World) -> (IVec3, [IVec3; 2], Vec3, Vec3) {
    let y = engine_worldgen::terrain_height(&world.sample_config(), 0, 0) + 1;
    (
        IVec3::new(8, y, 0),
        [IVec3::new(6, y, 4), IVec3::new(9, y, 4)],
        Vec3::new(0.5, y as f32, 0.5),
        Vec3::new(3.5, y as f32 + 0.1, 2.5),
    )
}

fn count(world: &World, chest: IVec3, item: Item) -> u32 {
    world
        .block_data
        .chest(chest)
        .iter()
        .flatten()
        .filter(|s| s.item == item)
        .map(|s| s.count)
        .sum()
}

fn build_origin(chest: IVec3) -> IVec3 {
    chest + IVec3::new(-3, 0, 2)
}

fn verify_saved(world: &World, chest: IVec3, mines: [IVec3; 2], building: bool) -> Result<()> {
    let jobs = &world.farming.jobs;
    ensure!(
        jobs.worker == Some(0) && jobs.stockpile == Some(chest.to_array()),
        "Assignment or chest did not persist"
    );
    ensure!(
        mines
            .iter()
            .all(|p| jobs.mine.contains(&(p.to_array(), B::Stone as u16))),
        "Mining orders did not persist"
    );
    let (block, quantity) = if building {
        (B::Planks, 2)
    } else {
        (B::Clay, 20)
    };
    ensure!(
        jobs.carry == Some(ItemStack::new(Item::Block(block), quantity)),
        "Carried stack did not persist"
    );
    if building {
        ensure!(world.travel.homes.get(&0) == Some(&fixture(world).2), "Resident home did not persist");
        ensure!(
            jobs.priorities() == [4, 3, 1] && jobs.building_material && jobs.build.len() == 2,
            "Build orders, materials or priorities did not persist"
        );
        ensure!(
            count(world, chest, Item::Block(B::Planks)) == 1,
            "Stockpile materials did not debit exactly twice"
        );
        ensure!(
            [build_origin(chest), build_origin(chest) + IVec3::X]
                .into_iter()
                .all(|p| world.get(p) == B::Air),
            "Blueprint cells were built before the material checkpoint"
        );
    }
    ensure!(
        count(world, chest, Item::Block(B::Clay)) == 0,
        "Saved stack was already in storage"
    );
    Ok(())
}

impl Run {
    pub(crate) fn from_env(benchmark: bool) -> Result<Option<Self>> {
        let Some(value) = std::env::var_os("VOXEL_JOBS_REVIEW") else {
            return Ok(None);
        };
        ensure!(
            benchmark && std::env::var_os("VOXEL_SAVE_DIR").is_some_and(|p| !p.is_empty()),
            "Jobs review requires a benchmark and an explicit scratch save directory"
        );
        let building = matches!(value.to_str(), Some("build-exercise" | "build-resume"));
        let phase = match value.to_str() {
            Some("exercise" | "build-exercise") => {
                ensure!(
                    !storage::quicksave_path().exists(),
                    "Jobs exercise needs a fresh scratch save directory"
                );
                Phase::Setup
            }
            Some("resume" | "build-resume") => Phase::Resume,
            _ => bail!("Unknown jobs review phase"),
        };
        Ok(Some(Self {
            phase,
            building,
            started: std::time::Instant::now(),
        }))
    }

    pub(crate) fn frame(
        &mut self,
        world: &mut World,
        player: &Player,
        day: &DayNightCycle,
        saves: &mut storage::SaveService,
    ) -> Result<bool> {
        ensure!(
            self.started.elapsed().as_secs() < 160,
            "Jobs review timed out: {}",
            super::status(world)
        );
        ensure!(
            world.world_type == WorldType::Superflat && world.voxel_scale.meters() == 1.0,
            "Jobs review uses a one-metre Superflat fixture"
        );
        let (chest, mines, settler, source) = fixture(world);
        self.phase = match self.phase {
            Phase::Setup => {
                for p in [settler.floor().as_ivec3(), chest] {
                    if !resident(world, p) {
                        ensure!(
                            world.admit_edit_chunk(crate::world::chunk_coord_from_world_block(p)),
                            "Fixture terrain could not be loaded"
                        );
                    }
                }
                world.set(chest, B::Chest);
                for p in mines {
                    world.set(p, B::Stone);
                }
                vehicle::review_scene(world, settler, settler + Vec3::Z * 1000.0);
                if self.building {
                    let invite = crate::world::travel::services::invite_to_base;
                    let eye = settler + Vec3::Y;
                    invite(world, 0, eye).map_err(anyhow::Error::msg)?;
                    ensure!(world.travel.following == Some(0), "Resident did not follow");
                    invite(world, 0, eye).map_err(anyhow::Error::msg)?;
                    ensure!(world.travel.following.is_none(), "Resident kept following after move-in");
                    println!("[jobs-review] invited resident and set home");
                }
                assign(world, 0);
                ensure!(designate(world, chest), "Could not mark stockpile");
                for p in mines {
                    ensure!(designate(world, p), "Could not mark mining order");
                }
                if self.building {
                    let mut planks = ItemStack::new(Item::Block(B::Planks), 3);
                    crate::world::block_data::store_hauled(world, chest, &mut planks);
                    ensure!(planks.count == 0, "Fixture stockpile rejected planks");
                    let blueprint =
                        crate::world::blueprint::Blueprint::filled(IVec3::new(2, 1, 1), B::Planks)
                            .unwrap();
                    let orders = super::queue_blueprint(world, &blueprint, build_origin(chest))
                        .map_err(anyhow::Error::msg)?;
                    ensure!(orders == 2, "Could not queue both blueprint cells");
                    super::set_priorities(world, 0, [4, 3, 1]);
                    println!("[jobs-review] queued 2 blueprint cells and set build > mine > haul");
                }
                world.inventory.dropped.push(DroppedStack {
                    pos: source,
                    stack: ItemStack::new(Item::Block(B::Clay), 20),
                });
                println!(
                    "[jobs-review] assigned villager 0, marked chest and 2 blocks, dropped 20 clay"
                );
                Phase::Pickup
            }
            Phase::Pickup if world.farming.jobs.carry.is_some() => {
                verify_saved(world, chest, mines, self.building)?;
                let feet = vehicle::villager_feet(world, 0)
                    .ok_or_else(|| anyhow::anyhow!("Worker disappeared"))?;
                ensure!(
                    feet.distance(settler) > 1.0,
                    "Worker did not walk to the load"
                );
                if self.building {
                    println!("[jobs-review] fetched 2 planks before mining or hauling at {feet:?}");
                } else {
                    println!("[jobs-review] walked to and picked up 20 clay at {feet:?}");
                }
                saves.request(true);
                saves.tick(Some(world), player, day);
                Phase::Saving
            }
            Phase::Saving => {
                saves.tick(Some(world), player, day);
                if saves.busy() {
                    return Ok(false);
                }
                ensure!(!saves.failed, "Checkpoint save failed");
                let loaded = storage::load_review_checkpoint()?;
                verify_saved(&loaded.world, chest, mines, self.building)?;
                ensure!(
                    self.building
                        || loaded
                            .world
                            .inventory
                            .dropped
                            .iter()
                            .all(|d| d.stack.item != Item::Block(B::Clay)),
                    "Carried stack also remained on the ground"
                );
                println!("[jobs-review] saved assignment, carried materials and pending orders");
                return Ok(true);
            }
            Phase::Resume => {
                verify_saved(world, chest, mines, self.building)?;
                println!("[jobs-review] reloaded assignment, carried materials and pending orders");
                Phase::Finish
            }
            Phase::Finish
                if count(world, chest, Item::Block(B::Stone)) == 2
                    && (!self.building || count(world, chest, Item::Block(B::Clay)) == 20) =>
            {
                ensure!(
                    count(world, chest, Item::Block(B::Clay)) == 20,
                    "Haul did not conserve the original stack"
                );
                ensure!(
                    mines.iter().all(|&p| world.get(p) == B::Air),
                    "A mining order left terrain behind"
                );
                ensure!(
                    world.farming.jobs.mine.is_empty() && world.farming.jobs.carry.is_none(),
                    "Finished orders remain active"
                );
                if self.building {
                    ensure!(
                        world.farming.jobs.build.is_empty()
                            && count(world, chest, Item::Block(B::Planks)) == 1
                            && [build_origin(chest), build_origin(chest) + IVec3::X]
                                .into_iter()
                                .all(|p| world.get(p) == B::Planks),
                        "Blueprint did not build exactly two cells from stockpile planks"
                    );
                    println!(
                        "[jobs-review] finished: 2 blueprint blocks built, 1 plank left, 2 stone mined and 20 clay hauled"
                    );
                } else {
                    println!(
                        "[jobs-review] finished: 20 clay and 2 mined stone in the marked chest"
                    );
                }
                return Ok(true);
            }
            Phase::Finish if self.building => {
                if world.farming.clock % 120 == 0 {
                    let j = &world.farming.jobs;
                    eprintln!("[jobs-review] clock={} feet={:?} status={} build={} mines={} carry={:?} waypoint={:?}", world.farming.clock, j.worker_pos, j.status, j.build.len(), j.mine.len(), j.carry, j.path.front());
                }
                let jobs = &world.farming.jobs;
                if jobs.carry.is_some_and(|s| s.item == Item::Block(B::Stone)) {
                    ensure!(
                        jobs.build.is_empty(),
                        "Mining overtook higher-priority construction"
                    );
                }
                if jobs.carry.is_some_and(|s| s.item == Item::Block(B::Clay)) {
                    ensure!(
                        count(world, chest, Item::Block(B::Stone)) == 2,
                        "Hauling overtook higher-priority mining"
                    );
                }
                Phase::Finish
            }
            current => current,
        };
        Ok(false)
    }
}
