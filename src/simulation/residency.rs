//! Simulation source residency is independent of camera/mesh residency. No frame-side I/O.
use crate::world::{
    BlockId, ChunkData, World, WorldSampleConfig, chunk_coord_from_world_block,
    regions::{DiskChunk, RegionView},
};
use glam::IVec3;
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
const CAPACITY: usize = 8;
const LIMIT: usize = 32 * 1024 * 1024;
// Worst decoded chunk, partial-water hash table, read buffer and generation scratch.
const RESERVATION: usize = 4 * 1024 * 1024;
struct Job {
    coord: IVec3,
    version: Option<u64>,
    cfg: WorldSampleConfig,
    view: RegionView,
    epoch: u64,
}
struct Done {
    coord: IVec3,
    version: Option<u64>,
    epoch: u64,
    data: Result<DiskChunk, String>,
    candidates: Option<Box<crate::world::activity::Mask>>,
}
struct Entry {
    version: Option<u64>,
    data: DiskChunk,
    bytes: usize,
    touched: u64,
}
pub(crate) struct Residency {
    entries: HashMap<IVec3, Entry>,
    pending: HashMap<IVec3, Option<u64>>,
    bytes: usize,
    clock: u64,
    tx: Option<mpsc::SyncSender<Job>>,
    rx: mpsc::Receiver<Done>,
    epoch: Arc<AtomicU64>,
    retry: Instant,
    waiters: HashMap<IVec3, Vec<IVec3>>,
    ready: Vec<IVec3>,
    edited_bytes: usize,
    edited_limit: usize,
    pub memory_pressure: bool,
    pub error: Option<String>,
}
impl Default for Residency {
    fn default() -> Self {
        let (_, rx) = mpsc::sync_channel(1);
        Self {
            entries: HashMap::new(),
            pending: HashMap::new(),
            bytes: 0,
            clock: 0,
            tx: None,
            rx,
            epoch: Arc::new(AtomicU64::new(0)),
            retry: Instant::now(),
            error: None,
            edited_bytes: 0,
            edited_limit: 0,
            memory_pressure: false,
            waiters: HashMap::new(),
            ready: Vec::new(),
        }
    }
}
impl Residency {
    pub fn reset(&mut self) {
        self.epoch.fetch_add(1, Ordering::Relaxed);
        self.entries.clear();
        self.bytes = 0;
        self.error = None;
        self.waiters.clear();
        self.ready.clear();
    }
    fn version(world: &World, c: IVec3) -> Option<u64> {
        world.regions.view.records.get(&c).map(|r| r.version)
    }
    fn start(&mut self) {
        let (tx, jobs) = mpsc::sync_channel::<Job>(CAPACITY);
        let (completed, rx) = mpsc::sync_channel(CAPACITY);
        let jobs = Arc::new(Mutex::new(jobs));
        let mut started = 0;
        // Keep the simulation pool small; it shares CPU with streaming/meshing.
        for i in 0..std::thread::available_parallelism().map_or(1, |n| (n.get() / 6).clamp(1, 2)) {
            let jobs = jobs.clone();
            let completed = completed.clone();
            let epoch = self.epoch.clone();
            match std::thread::Builder::new()
                .name(format!("simulation-source-{i}"))
                .spawn(move || {
                    loop {
                        let Ok(job) = jobs.lock().unwrap_or_else(|e| e.into_inner()).recv() else {
                            break;
                        };
                        if job.epoch != epoch.load(Ordering::Relaxed) {
                            if completed
                                .send(Done {
                                    coord: job.coord,
                                    version: job.version,
                                    epoch: job.epoch,
                                    data: Err("Cancelled".into()),
                                    candidates: None,
                                })
                                .is_err()
                            {
                                break;
                            }
                            continue;
                        }
                        let data = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                            || -> anyhow::Result<DiskChunk> {
                                if let Some(data) = job.view.load(job.coord)? {
                                    Ok(data)
                                } else {
                                    Ok(DiskChunk {
                                        chunk: Arc::new(ChunkData::generate(&job.cfg, job.coord)),
                                        water: Arc::default(),
                                    })
                                }
                            },
                        ))
                        .map_err(|_| "Simulation source job panicked".to_owned())
                        .and_then(|r| r.map_err(|e| format!("{e:#}")));
                        let candidates = data.as_ref().ok().map(|d| candidate_mask(d, job.coord));
                        if completed
                            .send(Done {
                                coord: job.coord,
                                version: job.version,
                                epoch: job.epoch,
                                data,
                                candidates,
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                }) {
                Ok(_) => started += 1,
                Err(e) => self.error = Some(format!("Simulation worker startup: {e}")),
            }
        }
        if started > 0 {
            self.tx = Some(tx);
            self.rx = rx;
        } else {
            self.retry = Instant::now() + Duration::from_secs(2);
        }
    }
    pub fn poll(&mut self, world: &mut World) {
        self.edited_bytes = world.edited_resident_bytes();
        self.edited_limit = crate::world::regions::edited_budget_mib() * 1024 * 1024;
        self.memory_pressure = false;
        if self.tx.is_none() && Instant::now() >= self.retry {
            self.start();
        }
        let started = Instant::now();
        for _ in 0..CAPACITY {
            match self.rx.try_recv() {
                Ok(done) => {
                    self.pending.remove(&done.coord);
                    let waiters = self.waiters.remove(&done.coord).unwrap_or_default();
                    if done.epoch != self.epoch.load(Ordering::Relaxed) {
                        continue;
                    }
                    if world.chunks.contains_key(&done.coord)
                        || Self::version(world, done.coord) != done.version
                    {
                        self.ready.extend(waiters);
                        continue;
                    }
                    match done.data {
                        Ok(data) => {
                            let bytes = data.chunk.blocks.len() * 2
                                + data.chunk.torches.capacity() * 32
                                + data.water.capacity() * 32;
                            // Reservation is held through the result channel, not released at worker completion.
                            if bytes <= RESERVATION {
                                self.clock = self.clock.wrapping_add(1);
                                if let Some(old) = self.entries.insert(
                                    done.coord,
                                    Entry {
                                        version: done.version,
                                        data,
                                        bytes,
                                        touched: self.clock,
                                    },
                                ) {
                                    self.bytes -= old.bytes;
                                }
                                self.bytes += bytes;
                                if let Some(mut mask) = done.candidates {
                                    for (&(x,y,z), _) in world.sandbox.cells.range((done.coord.x*32,i32::MIN,i32::MIN)..=(done.coord.x*32+31,i32::MAX,i32::MAX)) {
                                        let p = IVec3::new(x,y,z);
                                        if chunk_coord_from_world_block(p) == done.coord { let i=ChunkData::local_index(x,y,z); mask[i/64] |= 1u64 << (i%64); }
                                    }
                                    world.activity.restrict(done.coord, &mask);
                                }
                                self.error = None;
                                self.ready.extend(waiters);
                            } else {
                                self.error =
                                    Some("Decoded simulation source exceeds reservation".into());
                                self.retry = Instant::now() + Duration::from_secs(2);
                            }
                        }
                        Err(e) => {
                            self.error = Some(e);
                            self.retry = Instant::now() + Duration::from_secs(2);
                        }
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.tx = None;
                    self.pending.clear();
                    self.waiters.clear();
                    self.retry = Instant::now() + Duration::from_secs(2);
                    self.error = Some("Simulation source worker stopped; activity retained".into());
                    break;
                }
            }
            if started.elapsed() >= Duration::from_micros(500) {
                break;
            }
        }
    }
    pub fn request(&mut self, world: &World, c: IVec3) -> bool {
        if world.chunks.contains_key(&c) {
            return true;
        }
        let version = Self::version(world, c);
        if let Some(entry) = self.entries.get_mut(&c) {
            if entry.version == version {
                self.clock = self.clock.wrapping_add(1);
                entry.touched = self.clock;
                return true;
            }
            let old = self.entries.remove(&c).unwrap();
            self.bytes -= old.bytes;
        }
        if self.pending.contains_key(&c)
            || self.pending.len() >= CAPACITY
            || Instant::now() < self.retry
        {
            return false;
        }
        // Leave room for every queued/running/completed result, including worker scratch.
        while self.bytes + (self.pending.len() + 1) * RESERVATION > LIMIT {
            let Some(victim) = self
                .entries
                .iter()
                .min_by_key(|(_, e)| e.touched)
                .map(|(&c, _)| c)
            else {
                return false;
            };
            self.bytes -= self.entries.remove(&victim).unwrap().bytes;
        }
        if let Some(tx) = &self.tx {
            match tx.try_send(Job {
                coord: c,
                version,
                cfg: world.sample_config(),
                view: world.regions.view.clone(),
                epoch: self.epoch.load(Ordering::Relaxed),
            }) {
                Ok(()) => {
                    self.pending.insert(c, version);
                }
                Err(mpsc::TrySendError::Full(_)) => {}
                Err(mpsc::TrySendError::Disconnected(_)) => {
                    self.tx = None;
                    self.retry = Instant::now() + Duration::from_secs(2);
                }
            }
        }
        false
    }
    pub fn request_background(&mut self, world: &World, c: IVec3) -> bool {
        // World-facing machines leave two admission slots for cellular fronts.
        if self.pending.len() >= CAPACITY - 2
            && !world.chunks.contains_key(&c)
            && !self.entries.contains_key(&c)
        {
            return false;
        }
        self.request(world, c)
    }
    pub fn request_for(&mut self, world: &World, c: IVec3, owner: IVec3) -> bool {
        let ready = self.request(world, c);
        if !ready && self.pending.contains_key(&c) {
            let waiters = self.waiters.entry(c).or_default();
            if !waiters.contains(&owner) {
                waiters.push(owner);
            }
        }
        ready
    }
    pub fn completed(&mut self) -> Vec<IVec3> {
        std::mem::take(&mut self.ready)
    }
    pub fn read(&self, world: &World, p: IVec3) -> Option<(BlockId, u8)> {
        let c = chunk_coord_from_world_block(p);
        let i = ChunkData::local_index(p.x, p.y, p.z);
        if let Some(chunk) = world.chunks.get(&c) {
            let b = chunk.blocks[i];
            return Some((
                b,
                if b == BlockId::Water {
                    world.water_levels.get(&p).copied().unwrap_or(8)
                } else {
                    0
                },
            ));
        }
        self.entries
            .get(&c)
            .filter(|e| e.version == Self::version(world, c))
            .map(|e| {
                let b = e.data.chunk.blocks[i];
                (
                    b,
                    if b == BlockId::Water {
                        e.data.water.get(&p).copied().unwrap_or(8)
                    } else {
                        0
                    },
                )
            })
    }
    pub fn prepare(&mut self, world: &mut World, positions: &[IVec3]) -> bool {
        let mut incoming = 0;
        for (i, &p) in positions.iter().enumerate() {
            let c = chunk_coord_from_world_block(p);
            if !world.chunks.contains_key(&c)
                && !positions[..i]
                    .iter()
                    .any(|&p| chunk_coord_from_world_block(p) == c)
            {
                incoming += self.entries.get(&c).map_or(RESERVATION, |e| e.bytes);
            }
        }
        if self.edited_bytes.saturating_add(incoming) > self.edited_limit {
            self.memory_pressure = true;
            world.request_simulation_writeback();
            return false;
        }
        if positions.iter().any(|&p| self.read(world, p).is_none()) {
            return false;
        }
        for &p in positions {
            let c = chunk_coord_from_world_block(p);
            if !world.chunks.contains_key(&c) {
                let entry = self.entries.remove(&c).unwrap();
                self.bytes -= entry.bytes;
                self.edited_bytes += entry.bytes;
                world.install_simulation_chunk(c, entry.data);
            }
        }
        true
    }
    pub fn pending(&self) -> usize {
        self.pending.len()
    }
    pub fn bytes(&self) -> usize {
        self.bytes + self.pending.len() * RESERVATION
    }
}

// Recovery and coarse overflow may cover whole chunks. Filter sleeping solids
// and balanced water off-thread, before the frame-side rule scheduler sees them.
fn candidate_mask(data: &DiskChunk, c: IVec3) -> Box<crate::world::activity::Mask> {
    let mut mask = Box::new([0; 512]);
    let base = c * crate::world::CHUNK_SIZE;
    for (i, &b) in data.chunk.blocks.iter().enumerate() {
        if !matches!(b, BlockId::Water | BlockId::Sand | BlockId::Gravel | BlockId::Ash | BlockId::RigidCore) && !super::thermal::active(b) {
            continue;
        }
        let p = base + IVec3::new((i % 32) as i32, (i / 1024) as i32, ((i / 32) % 32) as i32);
        let read = |q: IVec3| {
            if chunk_coord_from_world_block(q) != c {
                return None;
            }
            let b = data.chunk.blocks[ChunkData::local_index(q.x, q.y, q.z)];
            Some((
                b,
                if b == BlockId::Water {
                    data.water.get(&q).copied().unwrap_or(8)
                } else {
                    0
                },
            ))
        };
        let down = read(p - IVec3::Y);
        let active = if super::thermal::active(b) || b == BlockId::RigidCore { true } else if b == BlockId::Water {
            let volume = data.water.get(&p).copied().unwrap_or(8);
            down.is_none_or(|(b, v)| b == BlockId::Air || (b == BlockId::Water && v < 8))
                || [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z]
                    .iter()
                    .any(|&d| {
                        read(p + d).is_none_or(|(b, v)| {
                            matches!(b, BlockId::Air | BlockId::Water)
                                && (volume.saturating_sub(v) > 1
                                    || (volume == 1 && v == 0 && read(p + d - IVec3::Y)
                                        .is_none_or(|(id, amount)| id == BlockId::Air
                                            || (id == BlockId::Water && amount < 8))))
                        })
                    })
        } else {
            down.is_none_or(|(b, _)| matches!(b, BlockId::Air | BlockId::Water))
                || [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z].iter().any(|&d| {
                    read(p + d).is_none_or(|(b, _)| matches!(b, BlockId::Air | BlockId::Water))
                        && read(p + d - IVec3::Y)
                            .is_none_or(|(b, _)| matches!(b, BlockId::Air | BlockId::Water))
                })
        };
        if active {
            mask[i / 64] |= 1u64 << (i % 64);
        }
    }
    mask
}
