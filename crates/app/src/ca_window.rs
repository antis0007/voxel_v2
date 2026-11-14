//! Window lifecycle for an existing exact cellular session.
use crate::{player::{Input, Player}, simulation::WorldSimulation,
    world::{World, DayNightCycle, storage::{self, session::Restored}}};
use anyhow::{Context, Result, ensure};
use std::{path::PathBuf, time::{Duration, Instant}};

pub struct Session {
    pub loaded: Option<Restored>,
    directory: PathBuf,
    writer: storage::session::Writer,
    pending: bool,
    next_due: Instant,
    retry: Instant,
    pub failed: bool,
    pub status: Option<String>,
}

impl Session {
    pub fn parse() -> Result<Option<Self>> {
        let args: Vec<_> = std::env::args().collect();
        let paths: Vec<_> = args.iter().filter(|arg| arg.starts_with("--ca-window")).collect();
        if paths.is_empty() { return Ok(None); }
        ensure!(paths.len() == 1, "duplicate --ca-window option");
        let directory = paths[0].strip_prefix("--ca-window=").filter(|s| !s.is_empty())
            .context("expected --ca-window=<exact-session-directory>")?;
        ensure!(!args.iter().any(|a| a.starts_with("--benchmark") || a.starts_with("--multiplayer")
            || a.starts_with("--ca-session") || a.starts_with("--ca-checkpoint")
            || a.starts_with("--ca-cpu-region")), "cellular window requires its own exact session");
        let backends: Vec<_> = args.iter().filter(|arg| arg.starts_with("--ca-backend")).collect();
        ensure!(backends.len() <= 1, "duplicate --ca-backend option");
        let gpu = match backends.first().map(|s| s.as_str()) {
            None | Some("--ca-backend=cpu") => false,
            Some("--ca-backend=gpu") => true,
            _ => anyhow::bail!("expected --ca-backend=cpu|gpu"),
        };
        let directory = PathBuf::from(directory).canonicalize()?;
        let mut restored = storage::session::load(&directory)?;
        if gpu { restored.region.enable_gpu().map_err(anyhow::Error::msg)?; }
        let status = restored.saved.recovery_notice.clone();
        Ok(Some(Self { loaded: Some(restored), directory, writer: storage::session::Writer::new()?, pending: false,
            next_due: Instant::now() + Duration::from_secs(storage::autosave_seconds() as u64),
            retry: Instant::now(), failed: false, status }))
    }

    pub fn request(&mut self) {
        self.pending = true;
        self.failed = false;
        self.retry = Instant::now();
        self.status = Some("Saving session".into());
    }

    pub fn busy(&self) -> bool { self.pending || self.writer.busy() }

    fn fail(&mut self, error: anyhow::Error) {
        self.pending = false;
        self.failed = true;
        self.status = Some(format!("Session save failed: {error:#}"));
    }

    pub fn tick(&mut self, world: &mut World, player: &Player, day: &DayNightCycle,
        sim: &WorldSimulation, input: &Input) {
        if self.loaded.is_some() { return; }
        match self.writer.poll(world) {
            Ok(true) => {
                self.status = Some("Session saved".into());
                self.next_due = Instant::now() + Duration::from_secs(storage::autosave_seconds() as u64);
            }
            Ok(false) => {}
            Err(error) => { self.fail(error); return; }
        }
        if self.failed { return; }
        if !self.busy() && storage::autosave_enabled() && Instant::now() >= self.next_due {
            self.request();
        }
        if !self.pending || self.writer.busy() || Instant::now() < self.retry { return; }
        match storage::session::capture(&self.directory, world, player, day, sim, input) {
            Ok(None) => { self.retry = Instant::now() + Duration::from_millis(250); }
            Ok(Some(job)) => match self.writer.submit(job) {
                Ok(()) => { self.pending = false; self.status = Some("Saving session".into()); }
                Err(error) => self.fail(error),
            },
            Err(error) => self.fail(error),
        }
    }
}
