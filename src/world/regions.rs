//! Append-only 8x8x8 chunk regions. Records are immutable, checksummed, and
//! published only after sync. Snapshot indices keep old versions readable.
use super::{BlockId, CHUNK_SIZE, ChunkData, World};
use anyhow::{Context, Result, ensure};
use glam::IVec3;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs::{self, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const VOXELS: usize = (CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE) as usize;
const MAX_RECORD: usize = 1024 * 1024;
const IO_CAPACITY: usize = 8;
static EDITED_MIB: AtomicUsize = AtomicUsize::new(64);
static SESSION_COUNTER: AtomicUsize = AtomicUsize::new(0);
pub fn edited_budget_mib() -> usize {
    EDITED_MIB.load(Ordering::Relaxed)
}
pub fn set_edited_budget_mib(value: usize) {
    EDITED_MIB.store(value.clamp(16, 1024), Ordering::Relaxed);
}

#[derive(Clone, Debug)]
pub struct Record {
    pub(super) pin: Option<Arc<maintenance::SegmentPin>>,
    pub generation: u64,
    pub offset: u64,
    pub len: u32,
    pub checksum: u64,
    pub version: u64,
    pub resident_bytes: u32,
}

// Copy-on-write roots are partitioned: publishing one edited chunk must not
// clone the complete region directory while render/simulation readers pin it.
const INDEX_BUCKETS: usize = 128;
type Group = Arc<HashMap<IVec3, Record>>;
type Groups = Arc<HashMap<IVec3, Group>>;
type Columns = Arc<HashMap<(i32, i32), Arc<HashSet<i32>>>>;
#[derive(Clone)]
pub struct RegionIndex {
    groups: [Groups; INDEX_BUCKETS],
    columns: [Columns; INDEX_BUCKETS],
    count: usize,
}
impl Default for RegionIndex {
    fn default() -> Self {
        Self {
            groups: std::array::from_fn(|_| Arc::default()),
            columns: std::array::from_fn(|_| Arc::default()),
            count: 0,
        }
    }
}
impl RegionIndex {
    fn bucket(x: i32, y: i32, z: i32) -> usize {
        let h = (x as u32).wrapping_mul(0x8da6b343)
            ^ (y as u32).wrapping_mul(0xd8163841)
            ^ (z as u32).wrapping_mul(0xcb1ab31f);
        ((h ^ (h >> 16)) as usize) & (INDEX_BUCKETS - 1)
    }
    fn group(&self, key: IVec3) -> Option<&Group> {
        self.groups[Self::bucket(key.x, key.y, key.z)].get(&key)
    }
    pub fn len(&self) -> usize {
        self.count
    }
    pub fn get(&self, c: &IVec3) -> Option<&Record> {
        self.group(IVec3::new(
            c.x.div_euclid(8),
            c.y.div_euclid(8),
            c.z.div_euclid(8),
        ))
        .and_then(|g| g.get(c))
    }
    pub fn contains_key(&self, c: &IVec3) -> bool {
        self.get(c).is_some()
    }
    pub fn keys(&self) -> impl Iterator<Item = &IVec3> {
        self.groups
            .iter()
            .flat_map(|b| b.values())
            .flat_map(|g| g.keys())
    }
    pub fn nearby(&self, center: IVec3, radius: i32) -> impl Iterator<Item = &IVec3> {
        let min_x = (center.x - radius).div_euclid(8);
        let max_x = (center.x + radius).div_euclid(8);
        let min_z = (center.z - radius).div_euclid(8);
        let max_z = (center.z + radius).div_euclid(8);
        (min_x..=max_x)
            .flat_map(move |x| {
                (min_z..=max_z).flat_map(move |z| {
                    self.columns[Self::bucket(x, 0, z)]
                        .get(&(x, z))
                        .into_iter()
                        .flat_map(move |ys| {
                            ys.iter()
                                .filter_map(move |&y| self.group(IVec3::new(x, y, z)))
                        })
                })
            })
            .flat_map(|g| g.keys())
            .filter(move |c| c.x.abs_diff(center.x).max(c.z.abs_diff(center.z)) <= radius as u32)
    }
    pub fn insert(&mut self, c: IVec3, r: Record) {
        if self
            .get(&c)
            .is_some_and(|old| (old.version, old.generation) > (r.version, r.generation))
        {
            return;
        }
        let key = IVec3::new(c.x.div_euclid(8), c.y.div_euclid(8), c.z.div_euclid(8));
        let xy = (key.x, key.z);
        let column_bucket = Self::bucket(key.x, 0, key.z);
        if !self.columns[column_bucket]
            .get(&xy)
            .is_some_and(|ys| ys.contains(&key.y))
        {
            Arc::make_mut(
                Arc::make_mut(&mut self.columns[column_bucket])
                    .entry(xy)
                    .or_default(),
            )
            .insert(key.y);
        }
        let group = Arc::make_mut(&mut self.groups[Self::bucket(key.x, key.y, key.z)])
            .entry(key)
            .or_default();
        if Arc::make_mut(group).insert(c, r).is_none() {
            self.count += 1;
        }
    }
}
impl From<HashMap<IVec3, Record>> for RegionIndex {
    fn from(records: HashMap<IVec3, Record>) -> Self {
        let mut index = Self::default();
        for (c, r) in records {
            index.insert(c, r);
        }
        index
    }
}
impl std::ops::Index<&IVec3> for RegionIndex {
    type Output = Record;
    fn index(&self, c: &IVec3) -> &Record {
        self.get(c).expect("Missing region record")
    }
}
#[derive(Clone)]
pub struct DiskChunk {
    pub chunk: Arc<ChunkData>,
    pub water: Arc<HashMap<IVec3, u8>>,
}
type ReadCache = VecDeque<(IVec3, u64, DiskChunk)>;
#[derive(Clone)]
pub struct RegionView {
    pub root: Arc<PathBuf>,
    pub records: Arc<RegionIndex>,
    cache: Arc<Mutex<ReadCache>>,
    error: Arc<Mutex<Option<String>>>,
    store: Arc<maintenance::Store>,
}
mod maintenance;
pub(super) use maintenance::CheckpointEdits;
pub(super) use maintenance::ReclaimStats;
pub(super) fn checkpoint_id() -> u64 {
    maintenance::unique_id()
}
fn chunk_bytes(chunk: &ChunkData) -> usize {
    VOXELS * 2 + chunk.torches.capacity() * (std::mem::size_of::<IVec3>() + 1) * 2
}
fn disk_bytes(data: &DiskChunk) -> usize {
    chunk_bytes(&data.chunk) + data.water.capacity() * (std::mem::size_of::<(IVec3, u8)>() + 1) * 2
}
fn checksum(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |h, &b| {
        (h ^ b as u64).wrapping_mul(0x100000001b3)
    })
}
fn region_path(root: &Path, c: IVec3, generation: u64) -> PathBuf {
    if generation != 0 {
        return root.join(format!(
            "r.{}.{}.{}.{}.vxr",
            c.x.div_euclid(8),
            c.y.div_euclid(8),
            c.z.div_euclid(8),
            generation
        ));
    }
    root.join(format!(
        "r.{}.{}.{}.vxr",
        c.x.div_euclid(8),
        c.y.div_euclid(8),
        c.z.div_euclid(8)
    ))
}
fn put(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}
fn take(input: &mut &[u8]) -> Result<u32> {
    ensure!(input.len() >= 4, "Truncated region record");
    let v = u32::from_le_bytes(input[..4].try_into()?);
    *input = &input[4..];
    Ok(v)
}
fn encode(coord: IVec3, version: u64, data: &ChunkData, water: &HashMap<IVec3, u8>) -> Vec<u8> {
    let mut out = b"VXREG001".to_vec();
    for c in coord.to_array() {
        put(&mut out, c as u32);
    }
    out.extend_from_slice(&version.to_le_bytes());
    let mut i = 0;
    while i < VOXELS {
        let start = i;
        let block = data.blocks[i];
        i += 1;
        while i < VOXELS && data.blocks[i] == block {
            i += 1;
        }
        put(&mut out, (i - start) as u32);
        put(&mut out, block.as_u32());
    }
    i = 0;
    while i < VOXELS {
        let start = i;
        let light = data.light[i];
        i += 1;
        while i < VOXELS && data.light[i] == light {
            i += 1;
        }
        put(&mut out, (i - start) as u32);
        put(&mut out, light as u32);
    }
    put(&mut out, water.len() as u32);
    for (&p, &v) in water {
        put(&mut out, ChunkData::local_index(p.x, p.y, p.z) as u32);
        put(&mut out, v as u32);
    }
    out
}
fn decode(coord: IVec3, record: &Record, bytes: &[u8]) -> Result<DiskChunk> {
    ensure!(
        bytes.len() >= 28 && &bytes[..8] == b"VXREG001",
        "Invalid region version"
    );
    ensure!(
        checksum(bytes) == record.checksum,
        "Region checksum mismatch at {coord}"
    );
    let mut input = &bytes[8..];
    let found = IVec3::new(
        take(&mut input)? as i32,
        take(&mut input)? as i32,
        take(&mut input)? as i32,
    );
    ensure!(found == coord, "Region coordinate mismatch");
    let lo = take(&mut input)? as u64;
    let hi = take(&mut input)? as u64;
    ensure!(
        (lo | hi << 32) == record.version,
        "Region revision mismatch"
    );
    let mut blocks = vec![BlockId::Air; VOXELS];
    let mut offset = 0;
    while offset < VOXELS {
        let n = take(&mut input)? as usize;
        let raw = take(&mut input)?;
        ensure!(n > 0 && n <= VOXELS - offset, "Invalid region block run");
        let b = BlockId::from_u32(raw);
        ensure!(b.as_u32() == raw, "Unknown region material");
        blocks[offset..offset + n].fill(b);
        offset += n;
    }
    let mut light = vec![0u8; VOXELS];
    offset = 0;
    while offset < VOXELS {
        let n = take(&mut input)? as usize;
        let v = take(&mut input)?;
        ensure!(
            n > 0 && n <= VOXELS - offset && v <= 255,
            "Invalid region light run"
        );
        light[offset..offset + n].fill(v as u8);
        offset += n;
    }
    let mut water = HashMap::new();
    let count = take(&mut input)? as usize;
    ensure!(count <= VOXELS, "Invalid region water count");
    let mut torches = HashSet::new();
    for (i, b) in blocks.iter().enumerate() {
        if b.is_torch() {
            torches.insert(coord * CHUNK_SIZE + local_pos(i));
        }
    }
    for _ in 0..count {
        let i = take(&mut input)? as usize;
        let v = take(&mut input)?;
        ensure!(
            i < VOXELS && (1..8).contains(&v) && blocks[i] == BlockId::Water,
            "Invalid region water volume"
        );
        ensure!(
            water
                .insert(coord * CHUNK_SIZE + local_pos(i), v as u8)
                .is_none(),
            "Duplicate region water volume"
        );
    }
    ensure!(input.is_empty(), "Unexpected region record data");
    Ok(DiskChunk {
        chunk: Arc::new(ChunkData {
            blocks: blocks
                .into_boxed_slice()
                .try_into()
                .map_err(|_| anyhow::anyhow!("Invalid block array"))?,
            light: light
                .into_boxed_slice()
                .try_into()
                .map_err(|_| anyhow::anyhow!("Invalid light array"))?,
            torches,
        }),
        water: Arc::new(water),
    })
}
fn local_pos(i: usize) -> IVec3 {
    let i = i as i32;
    IVec3::new(
        i % CHUNK_SIZE,
        i / (CHUNK_SIZE * CHUNK_SIZE),
        (i / CHUNK_SIZE) % CHUNK_SIZE,
    )
}
impl RegionView {
    fn new(root: PathBuf) -> Self {
        let store = maintenance::Store::shared(&root);
        Self {
            root: Arc::new(root),
            records: Arc::default(),
            cache: Arc::default(),
            error: Arc::default(),
            store,
        }
    }
    pub fn error(&self) -> Option<String> {
        self.error.lock().unwrap().clone()
    }
    fn report(&self, e: impl std::fmt::Display) {
        *self.error.lock().unwrap() = Some(e.to_string());
    }
    pub fn load(&self, coord: IVec3) -> Result<Option<DiskChunk>> {
        let Some(record) = self.records.get(&coord) else {
            return Ok(None);
        };
        {
            let mut cache = self.cache.lock().unwrap();
            if let Some(i) = cache
                .iter()
                .position(|(c, v, _)| *c == coord && *v == record.version)
            {
                let value = cache.remove(i).unwrap();
                let result = value.2.clone();
                cache.push_back(value);
                return Ok(Some(result));
            }
        }
        let result = (|| -> Result<DiskChunk> {
            ensure!(record.len as usize <= MAX_RECORD, "Oversized region record");
            let mut file = fs::File::open(region_path(&self.root, coord, record.generation))?;
            ensure!(
                record
                    .offset
                    .checked_add(record.len as u64)
                    .is_some_and(|end| end <= file.metadata().map(|m| m.len()).unwrap_or(0)),
                "Region extent exceeds file"
            );
            file.seek(SeekFrom::Start(record.offset))?;
            let mut bytes = vec![0; record.len as usize];
            file.read_exact(&mut bytes)?;
            decode(coord, record, &bytes)
        })();
        match result {
            Ok(data) => {
                let mut cache = self.cache.lock().unwrap();
                let limit = (edited_budget_mib() * 1024 * 1024 / 8).max(1024 * 1024);
                while !cache.is_empty()
                    && (cache.len() >= 16
                        || cache.iter().map(|(_, _, d)| disk_bytes(d)).sum::<usize>()
                            + disk_bytes(&data)
                            > limit)
                {
                    cache.pop_front();
                }
                cache.push_back((coord, record.version, data.clone()));
                Ok(Some(data))
            }
            Err(e) => {
                self.report(format!("Region read failed at {coord}: {e:#}"));
                Err(e)
            }
        }
    }
    pub fn session_name(&self) -> &str {
        self.root.file_name().and_then(|s| s.to_str()).unwrap()
    }
}
struct WriteJob {
    coord: IVec3,
    version: u64,
    chunk: Arc<ChunkData>,
    water: HashMap<IVec3, u8>,
}
enum Command {
    Write(WriteJob),
    Read(IVec3, RegionView),
}
enum Completion {
    Written(IVec3, u64, Result<Record>),
    Loaded(IVec3, u64, Result<Option<DiskChunk>>),
}
pub struct RegionStream {
    pub view: RegionView,
    pub(super) instance_id: u64,
    dirty: HashMap<IVec3, u64>,
    modified: HashMap<IVec3, Instant>,
    write_queue: VecDeque<IVec3>,
    write_queued: HashSet<IVec3>,
    pending: HashMap<IVec3, u64>,
    reading: HashSet<IVec3>,
    next_version: u64,
    tx: mpsc::SyncSender<Command>,
    rx: mpsc::Receiver<Completion>,
    last_plan: Option<(IVec3, i32, usize, usize)>,
    read_plan: VecDeque<IVec3>,
    last_eviction: Option<(IVec3, i32, usize, usize, usize)>,
    retry_after: Instant,
    force_writeback: bool,
    simulation_writeback: bool,
    restart_after: Instant,
}
fn io_worker(view: &RegionView) -> (mpsc::SyncSender<Command>, mpsc::Receiver<Completion>) {
    let (tx, commands) = mpsc::sync_channel::<Command>(IO_CAPACITY);
    let (done, rx) = mpsc::sync_channel(IO_CAPACITY);
    let root = Arc::clone(&view.root);
    let store = Arc::clone(&view.store);
    if let Err(e) = std::thread::Builder::new()
        .name("world-region-io".into())
        .spawn(move || {
            let mut queued = VecDeque::new();
            let mut reads = 0;
            loop {
                if queued.is_empty() {
                    match commands.recv() {
                        Ok(c) => queued.push_back(c),
                        Err(_) => break,
                    }
                }
                while queued.len() < IO_CAPACITY {
                    match commands.try_recv() {
                        Ok(c) => queued.push_back(c),
                        Err(_) => break,
                    }
                }
                // Serve terrain demand ahead of queued writeback. Four reads then
                // one write prevents dirty chunks from starving under continuous flight.
                let prefer_write = reads >= 4;
                let index = queued
                    .iter()
                    .position(|c| {
                        matches!(
                            (prefer_write, c),
                            (true, Command::Write(_)) | (false, Command::Read(..))
                        )
                    })
                    .unwrap_or(0);
                let command = queued.remove(index).unwrap();
                let result = match command {
                    Command::Write(job) => {
                        reads = 0;
                        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            write_record(&root, &store, &job)
                        }))
                        .unwrap_or_else(|_| Err(anyhow::anyhow!("Region write job panicked")));
                        Completion::Written(job.coord, job.version, result)
                    }
                    Command::Read(coord, view) => {
                        reads += 1;
                        let version = view.records[&coord].version;
                        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            view.load(coord)
                        }))
                        .unwrap_or_else(|_| Err(anyhow::anyhow!("Region read job panicked")));
                        Completion::Loaded(coord, version, result)
                    }
                };
                if done.send(result).is_err() {
                    break;
                }
            }
        })
    {
        view.report(format!("Cannot start region worker: {e}"));
    }
    (tx, rx)
}
fn write_record(root: &Path, store: &maintenance::Store, job: &WriteJob) -> Result<Record> {
    write_record_inner(root, store, job, true)
}
fn write_record_inner(
    root: &Path,
    store: &maintenance::Store,
    job: &WriteJob,
    sync: bool,
) -> Result<Record> {
    let bytes = encode(job.coord, job.version, &job.chunk, &job.water);
    let _gate = store.writer.lock().unwrap_or_else(|e| e.into_inner());
    store.ensure_owner(root)?;
    ensure!(
        bytes.len() <= MAX_RECORD,
        "Region record exceeds size limit"
    );
    let generation = store.write_generation(job.coord);
    let pin = store.pin(job.coord, generation);
    let region = region_path(root, job.coord, generation);
    let mut file = OpenOptions::new().create(true).append(true).open(region)?;
    let offset = file.metadata()?.len();
    file.write_all(&bytes)?;
    if sync {
        file.sync_data()?;
    }
    Ok(Record {
        pin: Some(pin),
        generation,
        offset,
        len: bytes.len() as u32,
        checksum: checksum(&bytes),
        version: job.version,
        resident_bytes: (chunk_bytes(&job.chunk)
            + job.water.capacity() * (std::mem::size_of::<(IVec3, u8)>() + 1) * 2)
            as u32,
    })
}
impl RegionStream {
    pub fn new() -> Self {
        let id = format!(
            "world-{}-{}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
            std::process::id(),
            SESSION_COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        Self::open(super::storage::save_directory().join("regions").join(id), HashMap::new())
    }
    pub fn open(root: PathBuf, mut records: HashMap<IVec3, Record>) -> Self {
        let next_version = records
            .values()
            .map(|r| r.version)
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        let mut view = RegionView::new(root);
        {
            let _gate = view.store.writer.lock().unwrap_or_else(|e| e.into_inner());
            for (&c, r) in &mut records {
                r.pin = Some(view.store.pin(c, r.generation));
            }
        }
        view.records = Arc::new(records.into());
        let (tx, rx) = io_worker(&view);
        Self {
            view,
            instance_id: checkpoint_id(),
            dirty: HashMap::new(),
            modified: HashMap::new(),
            write_queue: VecDeque::new(),
            write_queued: HashSet::new(),
            pending: HashMap::new(),
            reading: HashSet::new(),
            next_version,
            tx,
            rx,
            last_plan: None,
            last_eviction: None,
            read_plan: VecDeque::new(),
            retry_after: Instant::now(),
            force_writeback: false,
            simulation_writeback: false,
            restart_after: Instant::now(),
        }
    }
    pub fn mark_dirty(&mut self, c: IVec3) {
        if self.write_queued.insert(c) { self.write_queue.push_back(c); }
        self.dirty.insert(c, self.next_version);
        self.modified.entry(c).or_insert_with(Instant::now);
        self.next_version = self.next_version.wrapping_add(1);
    }
    pub fn pending(&self) -> usize {
        self.pending.len() + self.reading.len()
    }
}
impl World {
    pub fn snapshot_near(&mut self, center: IVec3, radius: i32) -> super::WorldSnapshot {
        self.stream_regions(center, radius);
        if self.snapshot_revision == u64::MAX
            || self.snapshot_range != Some((center, radius))
            || self.snapshot_cache.cfg != self.sample_config()
        {
            let chunks: HashMap<_, _> = self
                .chunks
                .iter()
                .filter(|(c, _)| {
                    c.x.abs_diff(center.x).max(c.z.abs_diff(center.z)) <= (radius + 1) as u32
                })
                .map(|(&c, v)| (c, Arc::clone(v)))
                .collect();
            self.snapshot_cache = super::WorldSnapshot {
                water_levels: Arc::new(self.water_levels.snapshot(chunks.keys().copied())),
                chunks: Arc::new(chunks),
                regions: self.regions.view.clone(),
                cfg: self.sample_config(),
            };
            self.snapshot_revision = self.revision;
            self.snapshot_range = Some((center, radius));
        }
        // Keep region metadata fresh without copying unchanged local voxel or
        // water maps. Otherwise a stationary renderer pins obsolete far-region
        // generations indefinitely while distant simulation keeps publishing.
        if !Arc::ptr_eq(
            &self.snapshot_cache.regions.records,
            &self.regions.view.records,
        ) {
            self.snapshot_cache.regions = self.regions.view.clone();
        }
        self.snapshot_cache.clone()
    }
    pub fn edited_resident_bytes(&self) -> usize {
        self.chunks.allocation_bytes()
            + self.water_levels.capacity() * (std::mem::size_of::<(IVec3, u8)>() + 1) * 2
    }
    fn evict_edited_chunk(&mut self, c: IVec3) {
        self.chunks.remove(&c);
        self.water_levels.remove_chunk(c);

        self.invalidate_snapshot_at(c);
    }
    pub fn region_status(&self) -> String {
        format!(
            "Edited chunks: {} resident / {} stored; {} I/O pending; {} dirty; cache {:.1} / {} MiB; reclaimed {:.1} MiB{}",
            self.chunks.len(),
            self.regions.view.records.len(),
            self.regions.pending(),
            self.regions.dirty.len(),
            self.edited_resident_bytes() as f64 / 1048576.0,
            edited_budget_mib(),
            self.regions.view.reclaimed_bytes() as f64 / 1048576.0,
            self.regions
                .view
                .error()
                .map(|e| format!("; {e}"))
                .unwrap_or_default()
        )
    }
    pub fn install_simulation_chunk(&mut self, c: IVec3, data: DiskChunk) {
        self.install_disk_chunk(c, data);
    }
    pub fn request_simulation_writeback(&mut self) {
        self.regions.simulation_writeback = true;
    }
    pub fn simulation_writeback_pending(&self) -> bool {
        !self.regions.dirty.is_empty()
    }
    fn install_disk_chunk(&mut self, c: IVec3, data: DiskChunk) {
        if self.chunks.contains_key(&c) {
            return;
        }
        self.water_levels
            .extend(data.water.iter().map(|(&p, &v)| (p, v)));
        self.chunks.insert(c, data.chunk);
        if !self.regions.view.records.contains_key(&c) { self.regions.mark_dirty(c); }
        self.invalidate_snapshot_at(c);
    }
    pub(super) fn resident_edit(&mut self, c: IVec3) -> Result<()> {
        if self.chunks.contains_key(&c) {
            return Ok(());
        }
        if let Some(data) = self.regions.view.load(c)? {
            self.install_disk_chunk(c, data);
        } else {
            self.chunks
                .insert(c, Arc::new(ChunkData::generate(&self.sample_config(), c)));
        }
        Ok(())
    }
    pub fn stream_regions(&mut self, center: IVec3, radius: i32) {
        let started = Instant::now();
        for _ in 0..IO_CAPACITY {
            let result = match self.regions.rx.try_recv() {
                Ok(result) => result,
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.regions
                        .view
                        .report("Region worker stopped; unsaved edits remain resident");
                    if Instant::now() >= self.regions.restart_after {
                        let (tx, rx) = io_worker(&self.regions.view);
                        self.regions.tx = tx;
                        self.regions.rx = rx;
                        self.regions.pending.clear();
                        self.regions.reading.clear();
                        self.regions.last_plan = None;
                        self.regions.read_plan.clear();
                        self.regions.restart_after = Instant::now() + Duration::from_secs(5);
                    }
                    break;
                }
            };
            match result {
                Completion::Written(c, v, result) => {
                    self.regions.pending.remove(&c);
                    match result {
                        Ok(record) => {
                            Arc::make_mut(&mut self.regions.view.records).insert(c, record);
                            if self.regions.dirty.get(&c) == Some(&v) {
                                self.regions.dirty.remove(&c);
                                self.regions.modified.remove(&c);
                            } else {
                                self.regions.modified.insert(c, Instant::now());
                            }
                            self.invalidate_snapshot_at(c);
                        }
                        Err(e) => {
                            self.regions
                                .view
                                .report(format!("Region write failed: {e:#}"));
                            self.regions.retry_after = Instant::now() + Duration::from_secs(2);
                        }
                    }
                }
                Completion::Loaded(c, v, result) => {
                    self.regions.reading.remove(&c);
                    if self
                        .regions
                        .view
                        .records
                        .get(&c)
                        .is_some_and(|r| r.version == v)
                        && !self.regions.dirty.contains_key(&c)
                    {
                        match result {
                            Ok(Some(data)) => self.install_disk_chunk(c, data),
                            Err(_) => {
                                self.regions.read_plan.push_back(c);
                                self.regions.retry_after = Instant::now() + Duration::from_secs(2);
                            }
                            _ => {}
                        }
                    }
                }
            }
            if started.elapsed() > Duration::from_millis(2) {
                break;
            }
        }
        let simulation_pressure = std::mem::take(&mut self.regions.simulation_writeback);
        if Instant::now() >= self.regions.retry_after {
            let pressure = self.regions.force_writeback
                || simulation_pressure
                || self.edited_resident_bytes() > edited_budget_mib() * 1024 * 1024;
            // Round-robin admission examines at most 64 coordinates per frame.
            // Hot fronts no longer force a full-world allocation/distance sort.
            let slots = IO_CAPACITY.saturating_sub(self.regions.pending()).min(4);
            let mut dirty = Vec::with_capacity(slots);
            for _ in 0..self.regions.write_queue.len().min(64) {
                if dirty.len() >= slots { break; }
                let c = self.regions.write_queue.pop_front().unwrap();
                self.regions.write_queued.remove(&c);
                let Some(&v) = self.regions.dirty.get(&c) else { continue; };
                self.regions.write_queue.push_back(c);
                self.regions.write_queued.insert(c);
                if !self.regions.pending.contains_key(&c)
                    && (pressure || self.regions.modified.get(&c)
                        .is_none_or(|t| t.elapsed() >= Duration::from_millis(750))) {
                    dirty.push((c, v));
                }
            }
            let selected: HashSet<_> = dirty.iter().map(|(c, _)| *c).collect();
            let mut water_by_chunk: HashMap<IVec3, HashMap<IVec3, u8>> = HashMap::new();
            if !selected.is_empty() {
                for &c in &selected {
                    if let Some(water) = self.water_levels.chunk(c) {
                        water_by_chunk.insert(c, water.clone());
                    }
                }
            }
            for (c, v) in dirty {
                if self.regions.pending() >= IO_CAPACITY {
                    break;
                }
                let Some(chunk) = self.chunks.get(&c).cloned() else {
                    continue;
                };
                let water = water_by_chunk.remove(&c).unwrap_or_default();
                match self.regions.tx.try_send(Command::Write(WriteJob {
                    coord: c,
                    version: v,
                    chunk,
                    water,
                })) {
                    Ok(()) => {
                        self.regions.pending.insert(c, v);
                    }
                    Err(mpsc::TrySendError::Full(_)) => break,
                    Err(mpsc::TrySendError::Disconnected(_)) => {
                        self.regions
                            .view
                            .report("Region worker stopped; dirty chunks retained in memory");
                        break;
                    }
                }
            }
        }
        let byte_limit = edited_budget_mib() * 1024 * 1024;
        let max_resident = byte_limit / (VOXELS * 2 + 1024);
        let eviction_plan = (
            center,
            radius,
            max_resident,
            self.chunks.len(),
            self.regions.dirty.len(),
        );
        if self.regions.last_eviction != Some(eviction_plan) {
            self.regions.last_eviction = Some(eviction_plan);
            let mut clean: Vec<_> = self
                .chunks
                .keys()
                .filter(|c| {
                    self.regions.view.records.contains_key(c)
                        && !self.regions.dirty.contains_key(c)
                        && !self.regions.pending.contains_key(c)
                })
                .copied()
                .collect();
            clean.sort_unstable_by_key(|c| {
                std::cmp::Reverse((*c - center).as_i64vec3().length_squared())
            });
            let mut bytes = self.edited_resident_bytes();
            let mut victims = HashSet::new();
            for c in clean {
                let d = (c - center).abs();
                if d.max_element() <= 2 {
                    continue;
                }
                if d.x.max(d.z) <= radius + 2 && bytes <= byte_limit {
                    continue;
                }
                if let Some(chunk) = self.chunks.get(&c) {
                    bytes = bytes.saturating_sub(chunk_bytes(chunk));
                }
                victims.insert(c);
                if victims.len() >= 32 {
                    break;
                }
            }
            if !victims.is_empty() {
                for c in victims { self.evict_edited_chunk(c); }
                // A bounded sweep must continue next frame if more clean
                // entries remain outside the streaming window.
                self.regions.last_eviction = None;
            }
        }
        let plan = (
            center,
            radius,
            max_resident,
            self.regions.view.records.len(),
        );
        if self.regions.last_plan != Some(plan) {
            let mut wanted: Vec<_> = self
                .regions
                .view
                .records
                .nearby(center, radius + 1)
                .filter(|c| !self.chunks.contains_key(c))
                .copied()
                .collect();
            wanted.sort_unstable_by_key(|c| (*c - center).as_i64vec3().length_squared());
            self.regions.read_plan = wanted.into();
            self.regions.last_plan = Some(plan);
        }
        for _ in 0..4 {
            if self.regions.pending() >= IO_CAPACITY {
                break;
            }
            if Instant::now() < self.regions.retry_after {
                break;
            }
            let Some(c) = self.regions.read_plan.pop_front() else {
                break;
            };
            if self.chunks.contains_key(&c) || self.regions.reading.contains(&c) {
                continue;
            }
            let incoming = self.regions.view.records[&c].resident_bytes as usize;
            let read_reserved = self
                .regions
                .reading
                .iter()
                .filter_map(|c| self.regions.view.records.get(c))
                .map(|r| r.resident_bytes as usize)
                .sum::<usize>();
            let mut fits = true;
            while self.edited_resident_bytes() + read_reserved + incoming > byte_limit {
                let victim = self
                    .chunks
                    .keys()
                    .filter(|p| {
                        !self.regions.dirty.contains_key(p)
                            && !self.regions.pending.contains_key(p)
                            && self.regions.view.records.contains_key(p)
                            && (**p - center).abs().max_element() > 2
                    })
                    .max_by_key(|p| (**p - center).as_i64vec3().length_squared())
                    .copied();
                if let Some(v) = victim.filter(|v| {
                    (*v - center).as_i64vec3().length_squared()
                        > (c - center).as_i64vec3().length_squared()
                }) {
                    self.evict_edited_chunk(v);
                } else {
                    self.regions.read_plan.push_front(c);
                    fits = false;
                    break;
                }
            }
            if !fits {
                break;
            }
            if self.chunks.contains_key(&c) || self.regions.reading.contains(&c) {
                continue;
            }
            match self
                .regions
                .tx
                .try_send(Command::Read(c, self.regions.view.clone()))
            {
                Ok(()) => {
                    self.regions.reading.insert(c);
                }
                Err(mpsc::TrySendError::Full(_)) => {
                    self.regions.read_plan.push_front(c);
                    break;
                }
                Err(mpsc::TrySendError::Disconnected(_)) => {
                    self.regions.view.report("Region worker stopped");
                    break;
                }
            }
        }
    }
}
