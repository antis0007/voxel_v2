//! One bounded save worker: immutable captures, durable commits and fallback recovery.
use super::super::regions::{CheckpointEdits, Record, RegionView};
use super::*;
use std::collections::{HashMap, VecDeque};
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    mpsc,
};
use std::time::{Duration, Instant};

static AUTOSAVE: AtomicBool = AtomicBool::new(true);
static INTERVAL: AtomicUsize = AtomicUsize::new(30);
pub fn autosave_enabled() -> bool {
    AUTOSAVE.load(Ordering::Relaxed)
}
pub fn set_autosave_enabled(value: bool) {
    AUTOSAVE.store(value, Ordering::Relaxed);
}
pub fn autosave_seconds() -> usize {
    INTERVAL.load(Ordering::Relaxed)
}
pub fn set_autosave_seconds(value: usize) {
    INTERVAL.store(value.clamp(10, 300), Ordering::Relaxed);
}
fn id() -> u64 {
    super::super::regions::checkpoint_id()
}
fn autosave_path() -> PathBuf {
    save_directory().join("autosave.vxw")
}
fn previous(path: &Path) -> PathBuf {
    path.with_extension("prev.vxw")
}
struct Job {
    edits: CheckpointEdits,
    header: Vec<u8>,
    footer: Vec<u8>,
    activity: super::super::activity::Snapshot,
    manual: bool,
    id: u64,
}
enum Work {
    Save(Job),
    Collect(RegionView),
}
enum Finished {
    Saved(Result<Completed>),
    Collected(Result<super::super::regions::ReclaimStats>),
}
struct Completed {
    instance_id: u64,
    view: RegionView,
    changed: VecDeque<(IVec3, Record)>,
    message: String,
}
pub struct SaveService {
    tx: Option<mpsc::SyncSender<Work>>,
    rx: mpsc::Receiver<Finished>,
    active: bool,
    pending: bool,
    manual: bool,
    applying: Option<Completed>,
    session: Option<PathBuf>,
    next_due: Instant,
    retry_capture: Instant,
    next_collect: Instant,
    pub status: Option<String>,
    pub failed: bool,
}
impl SaveService {
    pub fn new() -> Self {
        let (tx, jobs) = mpsc::sync_channel::<Work>(1);
        let (done, rx) = mpsc::sync_channel(1);
        let spawn = std::thread::Builder::new()
            .name("world-checkpoint".into())
            .spawn(move || {
                let mut reclaimer = Reclaimer::default();
                while let Ok(work) = jobs.recv() {
                    let result = match work {
                        Work::Save(job) => Finished::Saved(
                            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| persist(job)))
                                .unwrap_or_else(|_| {
                                    Err(anyhow::anyhow!(
                                        "Checkpoint worker panicked; previous commits remain intact"
                                    ))
                                }),
                        ),
                        Work::Collect(view) => Finished::Collected(
                            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                reclaimer.collect(&view)
                            }))
                            .unwrap_or_else(|_| {
                                Err(anyhow::anyhow!(
                                    "Region collector panicked; remaining files retained"
                                ))
                            }),
                        ),
                    };
                    if done.send(result).is_err() {
                        break;
                    }
                }
            });
        let status = spawn
            .as_ref()
            .err()
            .map(|e| format!("Cannot start save worker: {e}"));
        Self {
            tx: spawn.ok().map(|_| tx),
            rx,
            active: false,
            pending: false,
            manual: false,
            applying: None,
            session: None,
            next_due: Instant::now(),
            retry_capture: Instant::now(),
            next_collect: Instant::now(),
            status,
            failed: false,
        }
    }
    pub fn request(&mut self, manual: bool) {
        self.retry_capture = Instant::now();
        self.pending = true;
        self.manual |= manual;
        self.failed = false;
        self.status = Some("Saving world in the background…".into());
    }
    pub fn busy(&self) -> bool {
        self.active || self.pending || self.applying.is_some()
    }
    /// Called from AboutToWait, including while minimized. Acknowledgements are
    /// incremental so a large compacted index cannot monopolize a frame.
    pub fn tick(&mut self, mut world: Option<&mut World>, player: &Player, day: &DayNightCycle) {
        if self.applying.is_none() && self.active {
            match self.rx.try_recv() {
                Ok(Finished::Saved(Ok(done))) => self.applying = Some(done),
                Ok(Finished::Saved(Err(e))) => {
                    self.active = false;
                    self.pending = false;
                    self.manual = false;
                    self.failed = true;
                    self.status = Some(format!("Save failed: {e:#}"));
                    self.next_due = Instant::now() + Duration::from_secs(5);
                }
                Ok(Finished::Collected(result)) => {
                    self.active = false;
                    if let Err(e) = result {
                        self.next_collect = Instant::now() + Duration::from_secs(30);
                        eprintln!("Region reclamation deferred: {e:#}");
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.active = false;
                    self.pending = false;
                    self.failed = true;
                    self.status =
                        Some("Save worker stopped; unsaved changes remain in memory".into());
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if let Some(done) = self.applying.as_mut() {
            let start = Instant::now();
            for _ in 0..64 {
                let Some((c, r)) = done.changed.pop_front() else {
                    break;
                };
                if let Some(w) = world.as_deref_mut().filter(|w| {
                    w.regions.instance_id == done.instance_id
                        && w.regions.view.root == done.view.root
                }) {
                    w.apply_checkpoint(c, r);
                }
                if start.elapsed() >= Duration::from_millis(2) {
                    break;
                }
            }
            if done.changed.is_empty() {
                self.status = Some(done.message.clone());
                self.applying = None;
                self.active = false;
                self.failed = false;
                self.next_due = Instant::now() + Duration::from_secs(autosave_seconds() as u64);
            }
        }
        let Some(w) = world else {
            self.session = None;
            self.pending = false;
            self.manual = false;
            return;
        };
        if self.session.as_deref() != Some(w.regions.view.root.as_path()) {
            self.session = Some((*w.regions.view.root).clone());
            self.next_due = Instant::now() + Duration::from_secs(1);
        }
        if !self.active && autosave_enabled() && Instant::now() >= self.next_due {
            self.pending = true;
        }
        if !self.pending && !self.active && Instant::now() >= self.next_collect {
            if self
                .tx
                .as_ref()
                .is_some_and(|tx| tx.try_send(Work::Collect(w.regions.view.clone())).is_ok())
            {
                self.active = true;
            }
            self.next_collect = Instant::now() + Duration::from_secs(1);
        }
        if !self.pending || self.active || Instant::now() < self.retry_capture {
            return;
        }
        let result = (|| -> Result<Option<Job>> {
            let header = session_header(w, player, day)?;
            let footer = session_footer(w);
            let Some(edits) = w.capture_edits()? else {
                return Ok(None);
            };
            Ok(Some(Job {
                edits,
                header,
                footer,
                activity: w.activity.snapshot(),
                manual: self.manual,
                id: id(),
            }))
        })();
        match result {
            Ok(Some(job)) => {
                if self
                    .tx
                    .as_ref()
                    .is_some_and(|tx| tx.try_send(Work::Save(job)).is_ok())
                {
                    self.pending = false;
                    self.manual = false;
                    self.active = true;
                    self.failed = false;
                    self.status = Some("Saving world in the background…".into());
                } else {
                    self.pending = false;
                    self.failed = true;
                    self.next_due = Instant::now() + Duration::from_secs(5);
                    self.status = Some("Save worker unavailable; changes remain in memory".into());
                }
            }
            Ok(None) => {
                self.retry_capture = Instant::now() + Duration::from_millis(100);
                self.status =
                    Some("Saving: paging dirty chunks within the snapshot memory budget…".into())
            }
            Err(e) => {
                self.pending = false;
                self.failed = true;
                self.next_due = Instant::now() + Duration::from_secs(5);
                self.status = Some(format!("Save capture failed: {e:#}"));
            }
        }
    }
}
pub(super) fn write_atomic(path: &Path, bytes: &[u8], serial: u64) -> Result<()> {
    let temporary = path.with_extension(format!("{serial}.tmp"));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    fs::rename(&temporary, path).with_context(|| format!("Cannot publish checkpoint {}", path.display()))?;
    Ok(())
}
fn persist(job: Job) -> Result<Completed> {
    let instance_id = job.edits.instance_id;
    let (view, changed) = job.edits.persist()?;
    let mut footer = job.footer;
    job.activity.encode(&mut footer);
    let bytes = manifest(job.header, &view, &footer, job.id)?;
    let commits = view.root.join("checkpoints");
    fs::create_dir_all(&commits)?;
    let commit = commits.join(format!("commit-{:020}.vxw", job.id));
    write_atomic(&commit, &bytes, job.id)?;
    let slot = if job.manual {
        quicksave_path()
    } else {
        autosave_path()
    };
    // Slots are discovery pointers. Immutable per-world commits remain durable
    // even if slot replacement is interrupted or another process publishes later.
    let slot_lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(save_directory().join(".checkpoint-lock"))
        .with_context(|| format!("Cannot open save lock in {}", save_directory().display()))?;
    slot_lock.lock()?;
    if read_manifest(&slot)
        .ok()
        .and_then(|b| commit_id(&b))
        .unwrap_or(0)
        < job.id
    {
        if let Ok(old) = read_manifest(&slot) {
            write_atomic(&previous(&slot), &old, id())?;
        }
        write_atomic(&slot, &bytes, id())?;
    }
    drop(slot_lock);
    // Retention only removes manifests. Data collection happens on safe reopen.
    let mut retained = checkpoint_paths(&commits);
    retained.sort_by(|a, b| b.cmp(a));
    for path in retained.into_iter().skip(3) {
        let _ = fs::remove_file(path);
    }
    Ok(Completed {
        instance_id,
        view,
        changed: changed.into(),
        message: if job.manual {
            "World saved".into()
        } else {
            "World autosaved".into()
        },
    })
}
fn checkpoint_paths(dir: &Path) -> Vec<PathBuf> {
    fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
                n.starts_with("commit-")
                    && n.ends_with(".vxw")
                    && n[7..n.len() - 4].parse::<u64>().is_ok()
            })
        })
        .collect()
}
fn read_manifest(path: &Path) -> Result<Vec<u8>> {
    let file = fs::File::open(path)?;
    ensure!(
        file.metadata()?.len() <= MAX_FILE_BYTES,
        "Oversized checkpoint"
    );
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() >= 16 && bytes.len() as u64 <= MAX_FILE_BYTES,
        "Invalid checkpoint size"
    );
    let end = bytes.len() - 8;
    ensure!(
        checksum(&bytes[..end]) == u64::from_le_bytes(bytes[end..].try_into()?),
        "Checkpoint checksum mismatch"
    );
    Ok(bytes)
}
fn commit_id(bytes: &[u8]) -> Option<u64> {
    if save_version(bytes).ok()? < 5 {
        return None;
    }
    Some(u64::from_le_bytes(
        bytes
            .get(bytes.len().checked_sub(16)?..bytes.len() - 8)?
            .try_into()
            .ok()?,
    ))
}
fn references(bytes: &[u8]) -> Result<(String, HashSet<(IVec3, u64)>)> {
    let version = save_version(bytes)?;
    let five = version >= 5;
    if version < 3 {
        return Ok((String::new(), HashSet::new()));
    }
    let mut d = Decoder {
        bytes: &bytes[..bytes.len() - 8],
        offset: if version >= 12 {176} else {164},
    };
    let len = d.u32()? as usize;
    ensure!((1..=96).contains(&len), "Invalid world identity");
    let end = d
        .offset
        .checked_add(len)
        .context("World identity overflow")?;
    let name = std::str::from_utf8(
        d.bytes
            .get(d.offset..end)
            .context("Truncated world identity")?,
    )?
    .to_owned();
    ensure!(
        name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'),
        "Invalid world identity"
    );
    d.offset = end;
    let count = d.u32()? as usize;
    ensure!(count <= 1_000_000, "Oversized index");
    let mut segments = HashSet::new();
    for _ in 0..count {
        let c = IVec3::new(d.u32()? as i32, d.u32()? as i32, d.u32()? as i32);
        ensure!(
            c.to_array().iter().all(|v| v.unsigned_abs() <= 32_000),
            "Invalid retained coordinate"
        );
        for _ in 0..8 {
            d.u32()?;
        }
        let generation = if five {
            d.u32()? as u64 | ((d.u32()? as u64) << 32)
        } else {
            0
        };
        segments.insert((
            IVec3::new(
                c.x.div_euclid(8) * 8,
                c.y.div_euclid(8) * 8,
                c.z.div_euclid(8) * 8,
            ),
            generation,
        ));
    }
    Ok((name, segments))
}
fn segment(root: &Path, c: IVec3, g: u64) -> PathBuf {
    if g == 0 {
        root.join(format!(
            "r.{}.{}.{}.vxr",
            c.x.div_euclid(8),
            c.y.div_euclid(8),
            c.z.div_euclid(8)
        ))
    } else {
        root.join(format!(
            "r.{}.{}.{}.{}.vxr",
            c.x.div_euclid(8),
            c.y.div_euclid(8),
            c.z.div_euclid(8),
            g
        ))
    }
}
pub(super) fn validate_extents(view: &RegionView) -> Result<()> {
    let mut sizes = HashMap::new();
    for &c in view.records.keys() {
        let r = &view.records[&c];
        let path = segment(&view.root, c, r.generation);
        let size = match sizes.get(&path) {
            Some(s) => *s,
            None => {
                let s = fs::metadata(&path)
                    .context("Checkpoint segment missing")?
                    .len();
                sizes.insert(path, s);
                s
            }
        };
        ensure!(
            r.offset
                .checked_add(r.len as u64)
                .is_some_and(|n| n <= size),
            "Checkpoint references a truncated region"
        );
    }
    Ok(())
}
type ManifestRefs = (String, HashSet<(IVec3, u64)>);
#[derive(Default)]
struct Reclaimer {
    cache: HashMap<PathBuf, (u64, u64, Arc<ManifestRefs>)>,
}
impl Reclaimer {
    fn collect(&mut self, view: &RegionView) -> Result<super::super::regions::ReclaimStats> {
        use std::io::{Seek, SeekFrom};
        view.ensure_owner()?;
        // Lock discovery pointers until unlink finishes. A concurrent publisher
        // cannot add a retained reference between the mark and sweep.
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(save_directory().join(".checkpoint-lock"))
            .with_context(|| format!("Cannot open save lock in {}", save_directory().display()))?;
        if let Err(e) = lock.try_lock() {
            return match e {
                std::fs::TryLockError::WouldBlock => Ok(Default::default()),
                std::fs::TryLockError::Error(e) => Err(e.into()),
            };
        }
        let mut paths = checkpoint_paths(&view.root.join("checkpoints"));
        for slot in [quicksave_path(), autosave_path()] {
            for path in [slot.clone(), previous(&slot)] {
                if path.is_file() {
                    paths.push(path);
                }
            }
        }
        let mut refs = Vec::new();
        let mut seen = HashSet::new();
        for path in paths {
            seen.insert(path.clone());
            let mut file = fs::File::open(&path)?;
            let len = file.metadata()?.len();
            ensure!(
                (16..=MAX_FILE_BYTES).contains(&len),
                "Invalid retained manifest size"
            );
            file.seek(SeekFrom::End(-8))?;
            let mut fingerprint = [0; 8];
            file.read_exact(&mut fingerprint)?;
            let fingerprint = u64::from_le_bytes(fingerprint);
            if let Some((old_len, old_hash, entries)) = self.cache.get(&path) {
                if *old_len == len && *old_hash == fingerprint {
                    refs.push(Arc::clone(entries));
                    continue;
                }
            }
            let entries = Arc::new(references(&read_manifest(&path)?)?);
            refs.push(Arc::clone(&entries));
            self.cache.insert(path, (len, fingerprint, entries));
        }
        self.cache.retain(|p, _| seen.contains(p));
        view.reclaim(&refs)
    }
}
