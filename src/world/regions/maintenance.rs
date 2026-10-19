//! Background checkpoint inputs and generation-safe region maintenance.
use super::*;
use std::sync::{OnceLock, Weak, atomic::AtomicU64};

static IDS: AtomicU64 = AtomicU64::new(0);
pub(crate) fn unique_id() -> u64 {
    let time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros() as u64;
    let mut old = IDS.load(Ordering::Relaxed);
    loop {
        let next = time.max(old.saturating_add(1));
        match IDS.compare_exchange_weak(old, next, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => return next,
            Err(actual) => old = actual,
        }
    }
}
pub(super) struct Store {
    pub writer: Mutex<()>,
    owner: Arc<Mutex<Option<fs::File>>>,
    pub generation: u64,
    active: Mutex<HashMap<IVec3, Arc<SegmentPin>>>,
    pins: Mutex<HashMap<SegmentKey, Weak<SegmentPin>>>,
    scan: Mutex<Option<fs::ReadDir>>,
    reclaimed_bytes: AtomicU64,
    reclaimed_files: AtomicU64,
    compaction: Mutex<Option<Compaction>>,
}
impl Store {
    pub fn shared(root: &Path) -> Arc<Self> {
        static STORES: OnceLock<Mutex<HashMap<PathBuf, Weak<Store>>>> = OnceLock::new();
        let key = std::env::current_dir().unwrap_or_default().join(root);
        let mut stores = STORES.get_or_init(Mutex::default).lock().unwrap();
        stores.retain(|_, s| s.strong_count() > 0);
        if let Some(store) = stores.get(&key).and_then(Weak::upgrade) {
            return store;
        }
        let store = Arc::new(Self {
            writer: Mutex::new(()),
            owner: Arc::new(Mutex::new(None)),
            generation: unique_id(),
            active: Mutex::new(HashMap::new()),
            pins: Mutex::new(HashMap::new()),
            scan: Mutex::new(None),
            reclaimed_bytes: AtomicU64::new(0),
            reclaimed_files: AtomicU64::new(0),
            compaction: Mutex::new(None),
        });
        stores.insert(key, Arc::downgrade(&store));
        store
    }
    pub fn ensure_owner(&self, root: &Path) -> Result<()> {
        let mut owner = self.owner.lock().unwrap();
        if owner.is_none() {
            fs::create_dir_all(root).with_context(|| {
                format!("Cannot create world save directory {}", root.display())
            })?;
            let file = OpenOptions::new()
                .create(true)
                .truncate(false)
                .read(true)
                .write(true)
                .open(root.join(".owner-lock"))
                .with_context(|| format!("Cannot open world save lock in {}", root.display()))?;
            match file.try_lock() {
                Ok(()) => {}
                Err(std::fs::TryLockError::WouldBlock) => anyhow::bail!(
                    "World is already open in another process: {}",
                    root.display()
                ),
                Err(std::fs::TryLockError::Error(e)) => {
                    return Err(e).with_context(|| {
                        format!("Cannot lock world save directory {}", root.display())
                    });
                }
            }
            *owner = Some(file);
        }
        Ok(())
    }
    /// Call with the writer gate held, including for registration at load.
    pub fn pin(&self, c: IVec3, generation: u64) -> Arc<SegmentPin> {
        let key = segment_key(c, generation);
        let mut pins = self.pins.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(pin) = pins.get(&key).and_then(Weak::upgrade) {
            return pin;
        }
        let pin = Arc::new(SegmentPin {
            generation,
            _owner: Arc::clone(&self.owner),
        });
        pins.insert(key, Arc::downgrade(&pin));
        pin
    }
    pub fn write_generation(&self, c: IVec3) -> u64 {
        let mut active = self.active.lock().unwrap_or_else(|e| e.into_inner());
        active
            .entry(segment_key(c, 0).region)
            .or_insert_with(|| self.pin(c, self.generation))
            .generation
    }
}
pub struct CheckpointEdits {
    pub view: RegionView,
    pub instance_id: u64,
    jobs: Vec<WriteJob>,
}
impl RegionView {
    pub fn ensure_owner(&self) -> Result<()> {
        self.store.ensure_owner(&self.root)
    }
    /// Manifests protect recovery history; leases protect every live in-memory
    /// record, including records waiting in acknowledgements or compaction.
    pub(in crate::world) fn reclaim(
        &self,
        manifests: &[Arc<(String, HashSet<(IVec3, u64)>)>],
    ) -> Result<ReclaimStats> {
        self.ensure_owner()?;
        let protected: HashSet<_> = manifests
            .iter()
            .filter(|m| m.0 == self.session_name())
            .flat_map(|m| m.1.iter().map(|(c, g)| segment_key(*c, *g)))
            .collect();
        let mut scan = self.store.scan.lock().unwrap_or_else(|e| e.into_inner());
        if scan.is_none() {
            *scan = Some(fs::read_dir(&*self.root)?);
        }
        let start = Instant::now();
        let mut stats = ReclaimStats::default();
        while stats.scanned < 128 && stats.files < 16 {
            let Some(entry) = scan.as_mut().unwrap().next() else {
                *scan = None;
                break;
            };
            let entry = entry?;
            stats.scanned += 1;
            let Some(key) = parse_segment_name(&entry.file_name().to_string_lossy()) else {
                continue;
            };
            if protected.contains(&key) {
                continue;
            }
            // Registration and unlink share this gate. A reader with an existing
            // lease may clone it without a lock; an expired lease cannot revive
            // until its publisher has acquired this gate.
            let _writer = self.store.writer.lock().unwrap_or_else(|e| e.into_inner());
            let mut pins = self.store.pins.lock().unwrap_or_else(|e| e.into_inner());
            if pins.get(&key).and_then(Weak::upgrade).is_some() {
                stats.pinned += 1;
                continue;
            }
            let meta = entry.metadata()?;
            if !meta.is_file() {
                continue;
            }
            match fs::remove_file(entry.path()) {
                Ok(()) => {
                    stats.files += 1;
                    stats.bytes += meta.len();
                    pins.remove(&key);
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    pins.remove(&key);
                }
                // Keep the file and retry on a later pass (e.g. a Windows handle).
                Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {}
                Err(e) => return Err(e.into()),
            }
            if start.elapsed() >= Duration::from_millis(4) {
                break;
            }
        }
        self.store
            .reclaimed_files
            .fetch_add(stats.files, Ordering::Relaxed);
        self.store
            .reclaimed_bytes
            .fetch_add(stats.bytes, Ordering::Relaxed);
        Ok(stats)
    }
    pub fn reclaimed_bytes(&self) -> u64 {
        self.store.reclaimed_bytes.load(Ordering::Relaxed)
    }
}
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct SegmentKey {
    region: IVec3,
    generation: u64,
}
fn segment_key(c: IVec3, generation: u64) -> SegmentKey {
    SegmentKey {
        region: IVec3::new(c.x.div_euclid(8), c.y.div_euclid(8), c.z.div_euclid(8)),
        generation,
    }
}
fn parse_segment_name(name: &str) -> Option<SegmentKey> {
    let mut fields = name.split('.');
    if fields.next()? != "r" {
        return None;
    }
    let region = IVec3::new(
        fields.next()?.parse().ok()?,
        fields.next()?.parse().ok()?,
        fields.next()?.parse().ok()?,
    );
    let suffix = fields.next()?;
    let generation = if suffix == "vxr" {
        0
    } else {
        let generation = suffix.parse().ok()?;
        if fields.next()? != "vxr" {
            return None;
        }
        generation
    };
    if fields.next().is_some() {
        return None;
    }
    Some(SegmentKey { region, generation })
}
#[derive(Debug)]
pub(in crate::world) struct SegmentPin {
    generation: u64,
    // Even a standalone completion record retains process ownership.
    _owner: Arc<Mutex<Option<fs::File>>>,
}
#[derive(Default)]
pub(in crate::world) struct ReclaimStats {
    pub files: u64,
    pub bytes: u64,
    pub scanned: usize,
    pub pinned: usize,
}
impl World {
    /// Pin only dirty arrays, never a second full resident world. Fluid entries
    /// are partitioned once instead of scanning the global map for every chunk.
    pub fn capture_edits(&mut self) -> Result<Option<CheckpointEdits>> {
        let untracked: Vec<_> = self
            .chunks
            .keys()
            .filter(|c| {
                !self.regions.view.records.contains_key(c) && !self.regions.dirty.contains_key(c)
            })
            .copied()
            .collect();
        for c in untracked {
            self.regions.mark_dirty(c);
        }
        let water_count: usize = self
            .regions
            .dirty
            .keys()
            .filter_map(|c| self.water_levels.chunk(*c))
            .map(|m| m.len())
            .sum();
        let pinned = self
            .regions
            .dirty
            .keys()
            .filter_map(|c| self.chunks.get(c))
            .map(|c| chunk_bytes(c))
            .sum::<usize>()
            + (water_count * 4 + self.regions.dirty.len() * 4)
                * (std::mem::size_of::<(IVec3, u8)>() + 1);
        if pinned > 128 * 1024 * 1024 {
            self.regions.force_writeback = true;
            if let Some(error) = self.regions.view.error() {
                anyhow::bail!(
                    "Cannot reduce save snapshot memory after region I/O failure: {error}"
                );
            }
            return Ok(None);
        }
        self.regions.force_writeback = false;
        let mut water: HashMap<IVec3, HashMap<IVec3, u8>> = HashMap::new();
        for &c in self.regions.dirty.keys() {
            if let Some(values) = self.water_levels.chunk(c) {
                water.insert(c, values.clone());
            }
        }

        let jobs = self
            .regions
            .dirty
            .iter()
            .map(|(&coord, &version)| -> Result<_> {
                Ok(WriteJob {
                    coord,
                    version,
                    chunk: self
                        .chunks
                        .get(&coord)
                        .cloned()
                        .context("Dirty edited chunk is not resident")?,
                    water: water.remove(&coord).unwrap_or_default(),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Some(CheckpointEdits {
            view: self.regions.view.clone(),
            instance_id: self.regions.instance_id,
            jobs,
        }))
    }
    pub fn apply_checkpoint(&mut self, coord: IVec3, record: Record) {
        let version = record.version;
        Arc::make_mut(&mut self.regions.view.records).insert(coord, record);
        if self.regions.dirty.get(&coord) == Some(&version) {
            self.regions.dirty.remove(&coord);
            self.regions.modified.remove(&coord);
        }
        self.invalidate_snapshot_at(coord);
    }
}
impl CheckpointEdits {
    pub fn persist(mut self) -> Result<(RegionView, Vec<(IVec3, Record)>)> {
        self.view.ensure_owner()?;
        let mut changed = Vec::new();
        let mut written = HashSet::new();
        for job in self.jobs {
            let record = write_record_inner(&self.view.root, &self.view.store, &job, false)?;
            written.insert(region_path(&self.view.root, job.coord, record.generation));
            Arc::make_mut(&mut self.view.records).insert(job.coord, record.clone());
            changed.push((job.coord, record));
        }
        // A checkpoint publishes all its records together. Flush each touched
        // region once rather than forcing a disk barrier for every chunk.
        for path in written {
            OpenOptions::new().write(true).open(path)?.sync_data()?;
        }
        compact(&mut self.view, &mut changed)?;
        Ok((self.view, changed))
    }
}
/// A large region progresses across checkpoints. Only completed segments enter
/// an index; a crash leaves an unreferenced file eligible for background collection.
struct Compaction {
    todo: VecDeque<(IVec3, Record)>,
    copied: Vec<(IVec3, Record, Record)>,
    file: fs::File,
    generation: u64,
    offset: u64,
    pin: Arc<SegmentPin>,
}
fn compact(view: &mut RegionView, changed: &mut Vec<(IVec3, Record)>) -> Result<()> {
    let store = Arc::clone(&view.store);
    let mut state = store.compaction.lock().unwrap_or_else(|e| e.into_inner());
    let mut remaining = 16 * 1024 * 1024;
    for _ in 0..2 {
        if state.is_none() {
            let mut best = None;
            for group in view.records.groups.iter().flat_map(|b| b.values()) {
                let live: u64 = group.values().map(|r| r.len as u64).sum();
                let paths: HashSet<_> = group
                    .iter()
                    .map(|(c, r)| region_path(&view.root, *c, r.generation))
                    .collect();
                let mut size = 0u64;
                for path in paths {
                    size = size.saturating_add(fs::metadata(path)?.len());
                }
                if size >= 1024 * 1024
                    && size > live.saturating_mul(2)
                    && best.as_ref().is_none_or(|(w, _)| size - live > *w)
                {
                    best = Some((size - live, Arc::clone(group)));
                }
            }
            let Some((_, group)) = best else { break };
            view.ensure_owner()?;
            let generation = unique_id();
            let first = *group.keys().next().unwrap();
            let (file, pin) = {
                let _writer = store.writer.lock().unwrap_or_else(|e| e.into_inner());
                let pin = store.pin(first, generation);
                let file = OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .open(region_path(&view.root, first, generation))?;
                (file, pin)
            };
            *state = Some(Compaction {
                todo: group.iter().map(|(&c, r)| (c, r.clone())).collect(),
                copied: Vec::new(),
                file,
                generation,
                offset: 0,
                pin,
            });
        }
        let mut job = state.take().unwrap();
        while let Some((coord, old)) = job.todo.front() {
            if old.len as usize > remaining {
                break;
            }
            let mut input = fs::File::open(region_path(&view.root, *coord, old.generation))?;
            input.seek(SeekFrom::Start(old.offset))?;
            let mut bytes = vec![0; old.len as usize];
            input.read_exact(&mut bytes)?;
            ensure!(
                checksum(&bytes) == old.checksum,
                "Compaction found corrupt record at {coord}"
            );
            job.file.write_all(&bytes)?;
            let mut new = old.clone();
            new.generation = job.generation;
            new.pin = Some(Arc::clone(&job.pin));
            new.offset = job.offset;
            job.offset += old.len as u64;
            remaining -= old.len as usize;
            job.copied.push((*coord, old.clone(), new));
            job.todo.pop_front();
        }
        if !job.todo.is_empty() {
            *state = Some(job);
            break;
        }
        job.file.sync_all()?;
        if let Some((c, _, _)) = job.copied.first() {
            let _writer = store.writer.lock().unwrap_or_else(|e| e.into_inner());
            let region = IVec3::new(c.x.div_euclid(8), c.y.div_euclid(8), c.z.div_euclid(8));
            store
                .active
                .lock()
                .unwrap()
                .insert(region, Arc::clone(&job.pin));
        }
        for (c, old, new) in job.copied {
            if view.records.get(&c).is_some_and(|r| {
                r.version == old.version && r.generation == old.generation && r.offset == old.offset
            }) {
                Arc::make_mut(&mut view.records).insert(c, new.clone());
                changed.push((c, new));
            }
        }
        if remaining == 0 {
            break;
        }
    }
    Ok(())
}
