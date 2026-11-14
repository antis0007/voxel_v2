//! A coherent application checkpoint with exact isolated cellular authority.
mod actors;
mod format;
mod writer;
mod worker;
pub use writer::capture;
pub use worker::Writer;
use super::{DayNightCycle, Player, SavedSession, World, background, load_bytes,
    manifest, session_footer, session_identity, SAVE_VERSION};
use crate::{player::Input, simulation::{WorldSimulation, cpu_region::Region}};
use anyhow::{Context, Result, ensure};
use std::path::{Path, PathBuf};

pub struct Restored {
    pub saved: SavedSession,
    pub region: Region,
    pub player_tick: u64,
    pub input: Input,
}

pub(super) fn world_bytes(bytes: &[u8]) -> Result<&[u8]> {
    if bytes.starts_with(format::MAGIC) { Ok(format::parse(bytes)?.world) } else { Ok(bytes) }
}

pub(super) fn slots(directory: &Path) -> [PathBuf; 2] {
    [directory.join("simulation.vxs"), directory.join("simulation.prev.vxs")]
}

pub fn save(directory: &Path, world: &mut World, player: &Player, day: &DayNightCycle,
    sim: &WorldSimulation, input: &Input) -> Result<bool> {
    let Some(job) = capture(directory, world, player, day, sim, input)? else { return Ok(false) };
    let mut done = job.persist()?;
    while !done.apply(world)? {}
    Ok(true)
}

fn load_one(path: &Path) -> Result<Restored> {
    let bytes = background::read_manifest(path)?;
    let parts = format::parse(&bytes)?;
    let mut saved = load_bytes(parts.world, path)?;
    ensure!(parts.model == actors::model(&saved.world), "Exact session player model changed");
    let actors = actors::decode(parts.actors)?;
    ensure!(saved.player.pos == actors.player.pos && saved.player.scale == actors.player.scale,
        "Exact session actor and world identity disagree");
    let (region, player_tick) = Region::decode_checkpoint(&saved.world, parts.origin, parts.region)
        .map_err(anyhow::Error::msg)?;
    saved.player = actors.player;
    saved.day_night = actors.day;
    Ok(Restored { saved, region, player_tick, input: actors.input })
}

pub fn load(directory: &Path) -> Result<Restored> {
    let [slot, previous] = slots(directory);
    match load_one(&slot) {
        Ok(saved) => Ok(saved),
        Err(primary) => {
            let mut recovered = load_one(&previous)
                .with_context(|| format!("Exact session failed ({primary:#}); previous checkpoint also failed"))?;
            recovered.saved.recovery_notice = Some("Recovered the previous exact session".into());
            Ok(recovered)
        }
    }
}
