//! Opt-in native freight exercise. The normal game tick owns every shipment transition.
use super::{
    BRIDGE, BlockId, Clearance, DETOUR, DETOUR_Z, IVec3, LANES, SECOND, Site, State, Wait, World,
    fork, site,
};
use crate::{
    player::Player,
    world::{DayNightCycle, storage},
};
use anyhow::{Result, bail, ensure};
use std::collections::HashSet;

enum Step {
    Start,
    OneLane {
        cargo: u32,
        market: u32,
    },
    Detour {
        cargo: u32,
        market: u32,
    },
    Blocked {
        cargo: u32,
        market: u32,
        since: Option<u64>,
    },
    Repaired {
        cargo: u32,
        market: u32,
    },
    Saving {
        cargo: u32,
        market: u32,
    },
    Resume,
    Deliver {
        cargo: u32,
        market: u32,
    },
}

pub(crate) struct Run {
    step: Step,
    started: std::time::Instant,
}

fn signature(s: State) -> String {
    format!(
        "stock={:?} market={} cargo={} progress={} holding={} lane={}",
        s.stock, s.market, s.cargo, s.progress, s.holding, s.lane
    )
}

fn deck_cell(world: &World, site: Site, lane: usize) -> IVec3 {
    let z = LANES.get(lane).copied().unwrap_or(DETOUR_Z);
    let p = site.origin + glam::Vec3::new((BRIDGE.start + BRIDGE.end) as f32 * 0.5, 0.0, z);
    let scale = world.voxel_scale.meters();
    (p / scale).floor().as_ivec3() - IVec3::Y
}

fn change(
    world: &mut World,
    p: IVec3,
    from: BlockId,
    to: BlockId,
    blocks: &mut HashSet<IVec3>,
    chunks: &mut HashSet<IVec3>,
) -> Result<()> {
    ensure!(world.get(p) == from, "Unexpected bridge block at {p:?}");
    world.set(p, to);
    ensure!(world.get(p) == to, "Bridge edit rejected at {p:?}");
    blocks.insert(p);
    crate::world::mark_chunk_and_neighbors_dirty(chunks, p);
    Ok(())
}

impl Run {
    pub(crate) fn from_env(benchmark: bool) -> Result<Option<Self>> {
        let Some(value) = std::env::var_os("VOXEL_FREIGHT_REVIEW") else {
            return Ok(None);
        };
        ensure!(
            benchmark && std::env::var_os("VOXEL_SAVE_DIR").is_some_and(|p| !p.is_empty()),
            "Freight review requires a benchmark and an explicit scratch save directory"
        );
        let step = match value.to_str() {
            Some("exercise") => {
                ensure!(
                    !storage::quicksave_path().exists(),
                    "Freight exercise needs a fresh scratch save directory"
                );
                Step::Start
            }
            Some("resume") => Step::Resume,
            _ => bail!("Unknown freight review phase"),
        };
        Ok(Some(Self {
            step,
            started: std::time::Instant::now(),
        }))
    }

    pub(crate) fn frame(
        &mut self,
        world: &mut World,
        player: &Player,
        day: &DayNightCycle,
        blocks: &mut HashSet<IVec3>,
        chunks: &mut HashSet<IVec3>,
        saves: &mut storage::SaveService,
    ) -> Result<bool> {
        ensure!(
            self.started.elapsed().as_secs() < 220,
            "Freight review timed out"
        );
        let site = site(&world.sample_config())
            .ok_or_else(|| anyhow::anyhow!("No freight site on this Planet"))?;
        let north = deck_cell(world, site, 0);
        let south = deck_cell(world, site, 1);
        let detour = deck_cell(world, site, DETOUR as usize);
        let has_detour = world.generator_version >= engine_voxel::PLANET_FREIGHT_DETOUR_GENERATOR;
        let state = world.farming.planet_freight.unwrap_or_default();
        let crossing = fork(state.holding);
        self.step = match self.step {
            Step::Start if state.cargo > 0 && state.lane == 0 && state.progress == crossing + 2 => {
                change(world, north, BlockId::Planks, BlockId::Air, blocks, chunks)?;
                println!("[freight-review] north broken {}", signature(state));
                Step::OneLane {
                    cargo: state.cargo,
                    market: state.market,
                }
            }
            Step::OneLane { cargo, market }
                if state.lanes[0] == Clearance::Blocked && state.lane == 1 =>
            {
                ensure!(
                    state.cargo == cargo
                        && state.market == market
                        && state.progress == crossing + 1,
                    "Reroute must keep the load and restart at the approach"
                );
                println!("[freight-review] rerouted south {}", signature(state));
                change(world, south, BlockId::Planks, BlockId::Air, blocks, chunks)?;
                if has_detour {
                    Step::Detour { cargo, market }
                } else {
                    Step::Blocked {
                        cargo,
                        market,
                        since: None,
                    }
                }
            }
            Step::Detour { cargo, market }
                if state.lane == DETOUR && state.progress == crossing + 1 =>
            {
                ensure!(
                    state.cargo == cargo && state.market == market,
                    "Detour changed cargo ownership"
                );
                println!("[freight-review] service crossing {}", signature(state));
                change(
                    world,
                    detour,
                    BlockId::StoneBricks,
                    BlockId::Air,
                    blocks,
                    chunks,
                )?;
                Step::Blocked {
                    cargo,
                    market,
                    since: None,
                }
            }
            Step::Blocked {
                cargo,
                market,
                since,
            } if matches!(state.wait, Wait::Crossing) => {
                ensure!(
                    state.lanes == [Clearance::Blocked; 2]
                        && state.cargo == cargo
                        && state.market == market
                        && state.progress == crossing,
                    "Blocked crossing changed the load or position"
                );
                let clock = world.farming.clock;
                let since = since.unwrap_or(clock);
                if clock - since < 2 * SECOND {
                    Step::Blocked {
                        cargo,
                        market,
                        since: Some(since),
                    }
                } else {
                    println!(
                        "[freight-review] both blocked, held for 2s {}",
                        signature(state)
                    );
                    change(world, north, BlockId::Air, BlockId::Planks, blocks, chunks)?;
                    change(world, south, BlockId::Air, BlockId::Planks, blocks, chunks)?;
                    if has_detour {
                        change(
                            world,
                            detour,
                            BlockId::Air,
                            BlockId::StoneBricks,
                            blocks,
                            chunks,
                        )?;
                    }
                    Step::Repaired { cargo, market }
                }
            }
            Step::Repaired { cargo, market }
                if state.progress > crossing && state.lanes == [Clearance::Open; 2] =>
            {
                ensure!(
                    state.cargo == cargo && state.market == market,
                    "Repair lost the active shipment"
                );
                println!("[freight-review] repaired, moving {}", signature(state));
                saves.request(true);
                saves.tick(Some(world), player, day);
                Step::Saving { cargo, market }
            }
            Step::Saving { cargo, market } => {
                saves.tick(Some(world), player, day);
                if saves.busy() {
                    return Ok(false);
                }
                ensure!(!saves.failed, "Checkpoint save failed");
                let loaded = storage::load_review_checkpoint()?;
                let saved = loaded
                    .world
                    .farming
                    .planet_freight
                    .ok_or_else(|| anyhow::anyhow!("Missing saved freight state"))?;
                ensure!(
                    saved.cargo == cargo && saved.market == market && saved.progress > crossing,
                    "Checkpoint did not preserve moving cargo"
                );
                std::fs::write(
                    storage::save_directory().join("freight-review-state.txt"),
                    signature(saved),
                )?;
                println!("[freight-review] saved {}", signature(saved));
                return Ok(true);
            }
            Step::Resume => {
                let expected = std::fs::read_to_string(
                    storage::save_directory().join("freight-review-state.txt"),
                )?;
                ensure!(
                    signature(state) == expected && state.cargo > 0,
                    "Native reload changed the saved shipment"
                );
                println!("[freight-review] loaded {}", signature(state));
                Step::Deliver {
                    cargo: state.cargo,
                    market: state.market,
                }
            }
            Step::Deliver { cargo, market } if state.market != market => {
                ensure!(
                    state.market == market + cargo && state.cargo == 0,
                    "Arrival credited an incorrect load"
                );
                let item = super::super::pantry::BIOMASS;
                let before = world.inventory.count_item(item);
                let target = (site.market() / world.voxel_scale.meters())
                    .floor()
                    .as_ivec3();
                ensure!(
                    super::collect(world, target),
                    "Market counter did not accept collection"
                );
                let collected = world.inventory.count_item(item) - before;
                let remaining = world.farming.planet_freight.unwrap().market;
                ensure!(
                    collected > 0 && remaining + collected == state.market,
                    "Collection did not conserve market stock"
                );
                println!(
                    "[freight-review] delivered {cargo}, collected {collected}, market remaining {remaining}"
                );
                return Ok(true);
            }
            ref current => {
                return match current {
                    Step::Deliver { cargo, .. } => {
                        ensure!(
                            state.cargo == *cargo,
                            "Resumed load vanished before arrival"
                        );
                        Ok(false)
                    }
                    _ => Ok(false),
                };
            }
        };
        Ok(false)
    }
}
