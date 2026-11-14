//! Simulation source residency is independent of camera/mesh residency. No frame-side I/O.
use crate::fast_hash::{FastMap, FastSet};
use crate::world::{
    BlockId, ChunkData, World, WorldSampleConfig, chunk_coord_from_world_block,
    regions::{DiskChunk, RegionView},
};
use glam::IVec3;
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
const CAPACITY: usize = 8;
// Let a dependency event cross the soft edit budget without allowing an
// unbounded burst of newly resident chunks before writeback can run.
const PRESSURE_CHUNKS_PER_POLL: usize = CAPACITY;
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
    entries: FastMap<IVec3, Entry>,
    pending: FastMap<IVec3, Option<u64>>,
    bytes: usize,
    clock: u64,
    tx: Option<mpsc::SyncSender<Job>>,
    rx: mpsc::Receiver<Done>,
    epoch: Arc<AtomicU64>,
    retry: Instant,
    waiters: FastMap<IVec3, Vec<IVec3>>,
    ready: Vec<IVec3>,
    edited_bytes: usize,
    edited_limit: usize,
    pressure_chunks_left: usize,
    pub memory_pressure: bool,
    pub error: Option<String>,
}
impl Default for Residency {
    fn default() -> Self {
        let (_, rx) = mpsc::sync_channel(1);
        Self {
            entries: FastMap::default(),
            pending: FastMap::default(),
            bytes: 0,
            clock: 0,
            tx: None,
            rx,
            epoch: Arc::new(AtomicU64::new(0)),
            retry: Instant::now(),
            error: None,
            edited_bytes: 0,
            edited_limit: 0,
            pressure_chunks_left: PRESSURE_CHUNKS_PER_POLL,
            memory_pressure: false,
            waiters: FastMap::default(),
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
                                        chunk: Arc::new(engine_worldgen::generate(&job.cfg, job.coord)),
                                        water: Arc::default(),
                                    })
                                }
                            },
                        ))
                        .map_err(|_| "Simulation source job panicked".to_owned())
                        .and_then(|r| r.map_err(|e| format!("{e:#}")));
                        let candidates = data.as_ref().ok().map(|d| candidate_mask(d, job.coord, &job.cfg));
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
    /// No chunk generation is in flight.
    pub fn is_idle(&self) -> bool {
        self.pending.is_empty()
    }
    pub fn poll(&mut self, world: &mut World) { self.poll_with(world, true); }
    #[cfg(feature = "multiplayer")]
    pub(crate) fn poll_replica(&mut self, world: &mut World) { self.poll_with(world, false); }
    fn poll_with(&mut self, world: &mut World, activity: bool) {
        self.edited_bytes = world.edited_resident_bytes();
        self.edited_limit = crate::world::regions::edited_budget_mib() * 1024 * 1024;
        self.memory_pressure = false;
        self.pressure_chunks_left = PRESSURE_CHUNKS_PER_POLL;
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
                                    for &(x, y, z) in world.sandbox.cells.keys() {
                                        let p = IVec3::new(x, y, z);
                                        if chunk_coord_from_world_block(p) == done.coord {
                                            let i = ChunkData::local_index(x, y, z);
                                            mask[i / 64] |= 1u64 << (i % 64);
                                        }
                                    }
                                    if activity { world.activity.restrict(done.coord, &mask); }
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
    /// Returns a prepared base without installing terrain or water.
    #[cfg(feature = "multiplayer")]
    pub(crate) fn replica_source(&self, world: &World, c: IVec3) -> Option<Arc<ChunkData>> {
        world.chunks.get(&c).cloned().or_else(|| self.entries.get(&c)
            .filter(|entry| entry.version == Self::version(world, c))
            .map(|entry| entry.data.chunk.clone()))
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
    /// [`Self::request_for`] for fair-lane work: it may not take the last half of
    /// the slots, so sources near the player never queue behind distant backlog.
    pub fn request_distant(&mut self, world: &World, c: IVec3, owner: IVec3) -> bool {
        if self.pending.len() >= CAPACITY / 2
            && !world.chunks.contains_key(&c)
            && !self.entries.contains_key(&c)
        {
            return false;
        }
        self.request_for(world, c, owner)
    }
    fn request_tracked(&mut self, world: &World, c: IVec3, owner: IVec3, limit: usize) -> bool {
        if self.pending.len() >= limit
            && !world.chunks.contains_key(&c)
            && !self.entries.contains_key(&c)
        {
            return false;
        }
        let ready = self.request(world, c);
        if !ready && self.pending.contains_key(&c) {
            let waiters = self.waiters.entry(c).or_default();
            if !waiters.contains(&owner) {
                waiters.push(owner);
            }
        }
        ready
    }
    /// Near activity leaves two source slots for environmental dependencies.
    pub fn request_for(&mut self, world: &World, c: IVec3, owner: IVec3) -> bool {
        self.request_tracked(world, c, owner, CAPACITY - 2)
    }
    /// Environmental events may use the two slots reserved from other activity.
    pub fn request_environment(&mut self, world: &World, c: IVec3, owner: IVec3) -> bool {
        self.request_tracked(world, c, owner, CAPACITY)
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
    /// Initial river stock must agree before and after source admission.
    pub fn water_amount(&self, world: &World, p: IVec3) -> Option<u64> {
        self.water_cell(world, p).map(|(_, amount)| amount)
    }
    pub fn water_cell(&self, world: &World, p: IVec3) -> Option<(BlockId, u64)> {
        let (b, coarse) = self.read(world, p)?;
        if b != BlockId::Water { return Some((b, 0)); }
        let c=chunk_coord_from_world_block(p);
        if !world.chunks.contains_key(&c) && !world.regions.view.records.contains_key(&c)
            && self.entries.get(&c).is_some_and(|e|e.data.chunk.modified_bounds.is_none()) {
            if let Some(amount)=engine_worldgen::river_water_amount(&world.sample_config(),p) { return Some((b, amount)); }
        }
        Some((b, world.environment.exact_water(p, coarse)))
    }
    /// Immutable authoritative source for a bounded derived geometry query.
    pub fn chunk_data<'a>(&'a self, world: &'a World, c: IVec3) -> Option<&'a ChunkData> {
        world.chunks.get(&c).map(|c| &**c).or_else(|| self.entries.get(&c)
            .filter(|e| e.version == Self::version(world, c)).map(|e| &*e.data.chunk))
    }
    pub fn prepare(&mut self, world: &mut World, positions: &[IVec3]) -> bool {
        let mut incoming = 0;
        let mut seen = FastSet::default();
        let mut missing = Vec::new();
        let mut last = None;
        for &p in positions {
            let c = chunk_coord_from_world_block(p);
            if last == Some(c) { continue; }
            last = Some(c);
            if world.chunks.contains_key(&c) || (positions.len() > 2 && !seen.insert(c)) {
                continue;
            }
            let Some(entry) = self.entries.get(&c).filter(|e| e.version == Self::version(world, c)) else {
                return false;
            };
            incoming += entry.bytes;
            missing.push(c);
        }
        if !missing.is_empty() {
            let mut initial = Vec::new();
            for &c in &missing {
                initial.extend(world.generated_river_amounts(c, &self.entries[&c].data.chunk));
            }
            if !world.environment.can_encode(&initial) { return false; }
        }
        let pressure = self.edited_bytes.saturating_add(incoming) > self.edited_limit;
        if pressure {
            self.memory_pressure = true;
            world.request_simulation_writeback();
            // Resident mutations consume no admission credit. Refusing them
            // stops the oldest environment event forever while memory is full.
            // Keep the durable dirty-backlog ceiling for newly installed chunks.
            if !missing.is_empty()
                && (missing.len() > self.pressure_chunks_left || world.regions.backlog_full())
            {
                return false;
            }
        }
        if pressure {
            self.pressure_chunks_left -= missing.len();
        }
        for c in missing {
            let entry = self.entries.remove(&c).unwrap();
            self.bytes -= entry.bytes;
            self.edited_bytes += entry.bytes;
            world.install_simulation_chunk(c, entry.data);
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
fn candidate_mask(data: &DiskChunk, c: IVec3, cfg: &WorldSampleConfig) -> Box<crate::world::activity::Mask> {
    let mut mask = Box::new([0; 512]);
    let base = c * crate::world::CHUNK_SIZE;
    for (i, &b) in data.chunk.blocks.iter().enumerate() {
        if !matches!(
            b,
            BlockId::Water | BlockId::Sand | BlockId::Gravel | BlockId::Ash | BlockId::RigidCore
        ) && !super::thermal::active(b)
        {
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
        let active = if super::thermal::active(b) || b == BlockId::RigidCore {
            true
        } else if b == BlockId::Water {
            let volume = data.water.get(&p).copied().unwrap_or(8);
            engine_worldgen::river_water_amount(cfg,p).is_some()
                || down.is_none_or(|(b, v)| b == BlockId::Air || (b == BlockId::Water && v < 8))
                || [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z]
                    .iter()
                    .any(|&d| {
                        read(p + d).is_none_or(|(b, v)| {
                            matches!(b, BlockId::Air | BlockId::Water)
                                && (volume.saturating_sub(v) > 1
                                    || (volume == 1
                                        && v == 0
                                        && read(p + d - IVec3::Y).is_none_or(|(id, amount)| {
                                            id == BlockId::Air
                                                || (id == BlockId::Water && amount < 8)
                                        })))
                        })
                    })
        } else {
            down.is_none_or(|(b, _)| matches!(b, BlockId::Air | BlockId::Water))
                || [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z]
                    .iter()
                    .any(|&d| {
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

#[cfg(test)]
mod admission_tests {
    use super::*;
    use crate::{fluid::VoxelScalePreset, world::WorldType};

    #[test]
    fn environmental_requests_keep_two_slots_from_other_activity() {
        let world = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 5);
        let mut sources = Residency::default();
        let (tx, _rx) = mpsc::sync_channel(CAPACITY);
        sources.tx = Some(tx);
        for i in 0..6 {
            sources.pending.insert(IVec3::new(i, 0, 0), None);
        }
        assert!(!sources.request_for(&world, IVec3::new(100, 0, 0), IVec3::ZERO));
        assert!(!sources.pending.contains_key(&IVec3::new(100, 0, 0)));

        for x in 100..102 {
            let c = IVec3::new(x, 0, 0);
            assert!(!sources.request_environment(&world, c, IVec3::ZERO));
            assert!(sources.pending.contains_key(&c));
        }
        assert!(!sources.request_environment(&world, IVec3::new(102, 0, 0), IVec3::ZERO));
        assert!(!sources.pending.contains_key(&IVec3::new(102, 0, 0)));
    }
}

#[cfg(test)]
mod pressure_tests {
    use super::*;
    use crate::{fluid::VoxelScalePreset, world::WorldType};

    fn world() -> World {
        World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 7)
    }
    fn stage(sources: &mut Residency, world: &World, c: IVec3) {
        let data = DiskChunk {
            chunk: Arc::new(engine_worldgen::generate(&world.sample_config(), c)),
            water: Arc::default(),
        };
        let bytes = data.chunk.blocks.len() * 2;
        sources.entries.insert(c, Entry { version: None, data, bytes, touched: 0 });
        sources.bytes += bytes;
    }

    #[test]
    fn resident_edits_progress_after_pressure_admission_is_exhausted() {
        let mut world = world();
        let mut sources = Residency::default();
        sources.edited_limit = 0;
        stage(&mut sources, &world, IVec3::ZERO);
        assert!(sources.prepare(&mut world, &[IVec3::ZERO]));
        sources.pressure_chunks_left = 0;
        for x in 0..32 {
            let p = IVec3::new(x, 0, 0);
            assert!(sources.prepare(&mut world, &[p]));
            world.set(p, BlockId::Air);
            assert_eq!(world.get(p), BlockId::Air);
        }
        assert!(sources.memory_pressure);
        assert!(world.simulation_writeback_pending());
    }

    #[test]
    fn nonresident_placement_is_admitted_under_pressure() {
        let mut world = world();
        let mut sources = Residency::default();
        sources.edited_bytes = 1;
        sources.edited_limit = 0;
        let c = IVec3::new(0, 10, 0);
        let p = c * crate::world::CHUNK_SIZE;
        stage(&mut sources, &world, c);
        assert!(!world.chunks.contains_key(&c));
        assert!(sources.prepare(&mut world, &[p, p + IVec3::X]));
        world.set(p, BlockId::Stone);
        assert_eq!(world.get(p), BlockId::Stone);
        assert_eq!(sources.pressure_chunks_left, PRESSURE_CHUNKS_PER_POLL - 1);
    }

    #[test]
    fn pressure_admission_is_bounded_and_missing_sources_keep_credit() {
        let mut world = world();
        let mut sources = Residency::default();
        sources.edited_limit = 0;
        assert!(!sources.prepare(&mut world, &[IVec3::ZERO]));
        assert_eq!(sources.pressure_chunks_left, PRESSURE_CHUNKS_PER_POLL);
        for x in 0..=PRESSURE_CHUNKS_PER_POLL {
            let c = IVec3::new(x as i32, 10, 0);
            stage(&mut sources, &world, c);
            assert_eq!(sources.prepare(&mut world, &[c * crate::world::CHUNK_SIZE]),
                x < PRESSURE_CHUNKS_PER_POLL);
        }
        assert_eq!(sources.pressure_chunks_left, 0);
        sources.poll(&mut world);
        assert_eq!(sources.pressure_chunks_left, PRESSURE_CHUNKS_PER_POLL);
    }
}
