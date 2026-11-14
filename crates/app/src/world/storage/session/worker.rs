use super::{World, writer::{Job, Completed}};
use anyhow::{Result, ensure};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError};

pub struct Writer {
    tx: SyncSender<Job>,
    rx: Receiver<Result<Completed>>,
    active: bool,
    applying: Option<Completed>,
}

impl Writer {
    pub fn new() -> Result<Self> {
        let (tx, jobs) = mpsc::sync_channel::<Job>(1);
        let (done, rx) = mpsc::sync_channel(1);
        std::thread::Builder::new().name("cellular-checkpoint".into()).spawn(move || {
            while let Ok(job) = jobs.recv() {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| job.persist()))
                    .unwrap_or_else(|_| Err(anyhow::anyhow!("Cellular save worker panicked")));
                if done.send(result).is_err() { break; }
            }
        })?;
        Ok(Self { tx, rx, active: false, applying: None })
    }

    pub fn busy(&self) -> bool { self.active }

    pub fn submit(&mut self, job: Job) -> Result<()> {
        ensure!(!self.active, "A cellular save is already in progress");
        self.tx.try_send(job).map_err(|_| anyhow::anyhow!("Cellular save worker unavailable"))?;
        self.active = true;
        Ok(())
    }

    /// Return true only after disk publication and live acknowledgement finish.
    pub fn poll(&mut self, world: &mut World) -> Result<bool> {
        if !self.active { return Ok(false); }
        if self.applying.is_none() {
            match self.rx.try_recv() {
                Ok(Ok(done)) => self.applying = Some(done),
                Ok(Err(error)) => { self.active = false; return Err(error); }
                Err(TryRecvError::Disconnected) => {
                    self.active = false;
                    anyhow::bail!("Cellular save worker stopped");
                }
                Err(TryRecvError::Empty) => return Ok(false),
            }
        }
        let result = match self.applying.as_mut() {
            Some(done) => done.apply(world),
            None => Ok(false),
        };
        if !matches!(result, Ok(false)) {
            self.applying = None;
            self.active = false;
        }
        result
    }
}
