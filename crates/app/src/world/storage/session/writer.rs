use super::{actors, background, format, manifest, session_footer, session_identity,
    DayNightCycle, Input, Player, World, WorldSimulation, SAVE_VERSION};
use crate::world::regions::{CheckpointEdits, Record, RegionView};
use anyhow::{Context, Result, ensure};
use glam::IVec3;
use std::{collections::VecDeque, fs, path::{Path, PathBuf}, sync::Arc, time::{Duration, Instant}};

pub struct Job {
    directory: PathBuf,
    edits: CheckpointEdits,
    header: Vec<u8>,
    footer: Vec<u8>,
    region: Vec<u8>,
    actors: Vec<u8>,
    origin: IVec3,
    model: u64,
    commit: u64,
}

pub(super) struct Completed {
    instance_id: u64,
    view: RegionView,
    changed: VecDeque<(IVec3, Record)>,
}

pub fn capture(directory: &Path, world: &mut World, player: &Player, day: &DayNightCycle,
    sim: &WorldSimulation, input: &Input) -> Result<Option<Job>> {
    let region = sim.cpu_region.as_ref().context("Exact session requires cellular authority")?;
    ensure!(sim.authority_error.is_none(), "Cannot checkpoint failed authority");
    ensure!(world.ca_view.as_ref().is_some_and(|view| Arc::ptr_eq(view, &region.published)),
        "World and cellular publication do not belong to one cut");
    let Some(edits) = world.capture_edits()? else { return Ok(None) };
    let mut footer = session_footer(world, SAVE_VERSION);
    world.activity.snapshot().encode(&mut footer);
    Ok(Some(Job {
        directory: directory.to_path_buf(), edits,
        header: session_identity(world, player, day, SAVE_VERSION)?, footer,
        region: region.encode_checkpoint(world, sim.player_tick.0).map_err(anyhow::Error::msg)?,
        actors: actors::encode(player, input, day)?, origin: region.published.origin,
        model: actors::model(world), commit: crate::world::regions::checkpoint_id(),
    }))
}

impl Job {
    pub(super) fn persist(self) -> Result<Completed> {
        let directory = self.directory.canonicalize().context("Exact session directory must exist")?;
        let base = self.edits.view.root.parent().and_then(Path::parent)
            .context("Invalid world storage root")?.canonicalize()?;
        ensure!(base == directory, "Exact session must stay beside its region store");
        let instance_id = self.edits.instance_id;
        let (view, changed) = self.edits.persist()?;
        background::validate_extents(&view)?;
        let inner = manifest(self.header, &view, &self.footer, self.commit)?;
        let bytes = format::encode(self.model, self.origin, &inner, &self.region, &self.actors, self.commit)?;
        let commits = view.root.join("checkpoints");
        fs::create_dir_all(&commits)?;
        let immutable = commits.join(format!("commit-{:020}.vxs", self.commit));
        background::write_atomic(&immutable, &bytes, self.commit)?;
        let lock = fs::OpenOptions::new().create(true).truncate(false).read(true).write(true)
            .open(directory.join(".checkpoint-lock"))?;
        lock.lock()?;
        let [slot, previous] = super::slots(&directory);
        if let Ok(old) = background::read_manifest(&slot) {
            ensure!(background::commit_id(&old).unwrap_or(0) < self.commit,
                "A newer exact session was published");
            background::write_atomic(&previous, &old, crate::world::regions::checkpoint_id())?;
        }
        background::write_atomic(&slot, &bytes, crate::world::regions::checkpoint_id())?;
        let mut retained = background::checkpoint_paths(&commits);
        retained.sort_by(|a, b| b.cmp(a));
        for path in retained.into_iter().skip(3) { let _ = fs::remove_file(path); }
        drop(lock);
        Ok(Completed { instance_id, view, changed: changed.into() })
    }
}

impl Completed {
    /// Merge saved records without replacing newer live state.
    pub(super) fn apply(&mut self, world: &mut World) -> Result<bool> {
        ensure!(world.regions.instance_id == self.instance_id && world.regions.view.root == self.view.root,
            "Saved session no longer matches the live world");
        let started = Instant::now();
        for _ in 0..64 {
            let Some((coord, record)) = self.changed.pop_front() else { break };
            world.apply_checkpoint(coord, record);
            if started.elapsed() >= Duration::from_millis(2) { break; }
        }
        Ok(self.changed.is_empty())
    }
}
