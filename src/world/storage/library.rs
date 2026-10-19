//! World discovery and named metadata. Directory scans and loads stay off the UI thread.
use super::*;
use std::collections::{HashMap, VecDeque};
use std::io::{Seek, SeekFrom};
use std::sync::mpsc;
use std::time::UNIX_EPOCH;

#[derive(Clone, Debug)]
pub struct WorldEntry {
    pub id: String,
    pub name: String,
    pub favorite: bool,
    pub seed: u32,
    pub kind: WorldType,
    pub scale: VoxelScalePreset,
    pub saved: u64,
    pub chunks: u32,
    pub checkpoints: usize,
    pub problem: Option<String>,
}
#[derive(Clone)]
struct Header {
    id: String,
    kind: WorldType,
    scale: VoxelScalePreset,
    seed: u32,
    saved: u64,
    chunks: u32,
}
fn valid_id(id: &str) -> bool {
    (1..=96).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}
fn root(id: &str) -> Result<PathBuf> {
    ensure!(valid_id(id), "Invalid world identity");
    Ok(super::paths::existing_world(id))
}
fn slot_paths() -> Vec<PathBuf> {
    [
        "quicksave.vxw",
        "autosave.vxw",
        "quicksave.prev.vxw",
        "autosave.prev.vxw",
    ]
    .into_iter()
    .flat_map(|n| {
        super::paths::library_directories()
            .into_iter()
            .map(move |r| r.join(n))
    })
    .filter(|p| p.is_file())
    .collect()
}
fn commits(root: &Path) -> Vec<PathBuf> {
    fs::read_dir(root.join("checkpoints"))
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
fn header(path: &Path) -> Result<Header> {
    let mut file = fs::File::open(path)?;
    let len = file.metadata()?.len();
    ensure!(
        (176..=MAX_FILE_BYTES).contains(&len),
        "Invalid checkpoint size"
    );
    let mut prefix = vec![0u8; 168];
    file.read_exact(&mut prefix)?;
    let version = save_version(&prefix)?;
    if version >= 12 { prefix.resize(180,0); file.read_exact(&mut prefix[168..])?; }
    let word = |offset| u32::from_le_bytes(prefix[offset..offset + 4].try_into().unwrap());
    let kind = match word(16) {
        0 => WorldType::Superflat,
        1 => WorldType::Hills,
        2 => WorldType::Ridged,
        _ => anyhow::bail!("Invalid landscape"),
    };
    let scale = match word(20) {
        0 => VoxelScalePreset::OneMeter,
        1 => VoxelScalePreset::HalfMeter,
        2 => VoxelScalePreset::QuarterMeter,
        _ => anyhow::bail!("Invalid voxel scale"),
    };
    let modern = version >= 5;
    let (id, chunks) = if version <= 2 {
        (
            format!(
                "legacy-{}",
                path.file_name()
                    .and_then(|s| s.to_str())
                    .context("Invalid legacy slot")?
                    .replace('.', "-")
            ),
            word(164),
        )
    } else {
        let count = word(if version >= 12 {176} else {164}) as usize;
        ensure!((1..=96).contains(&count), "Invalid world identity length");
        let mut name = vec![0; count];
        file.read_exact(&mut name)?;
        let id = String::from_utf8(name)?;
        ensure!(valid_id(&id), "Invalid world identity");
        let mut n = [0; 4];
        file.read_exact(&mut n)?;
        (id, u32::from_le_bytes(n))
    };
    ensure!(chunks <= 1_000_000, "Invalid chunk count");
    let saved = if modern {
        file.seek(SeekFrom::End(-16))?;
        let mut n = [0; 8];
        file.read_exact(&mut n)?;
        u64::from_le_bytes(n)
    } else {
        file.metadata()?
            .modified()?
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64
    };
    Ok(Header {
        id,
        kind,
        scale,
        seed: word(12),
        saved,
        chunks,
    })
}
fn default_name(h: &Header) -> String {
    format!(
        "{} · {}",
        match h.kind {
            WorldType::Hills => "Wildlands",
            WorldType::Ridged => "Highlands",
            WorldType::Superflat => "Open canvas",
        },
        h.seed
    )
}
#[derive(Default)]
struct Metadata {
    name: String,
    favorite: bool,
}
fn metadata(id: &str) -> Result<Metadata> {
    if id.starts_with("legacy-") {
        return Ok(Metadata::default());
    }
    let path = root(id)?.join("world.meta");
    if !path.is_file() {
        return Ok(Metadata::default());
    }
    let mut bytes = Vec::new();
    fs::File::open(path)?.take(513).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() >= 21 && bytes.len() <= 512 && &bytes[..8] == b"VXWMETA1",
        "Invalid world metadata"
    );
    let end = bytes.len() - 8;
    ensure!(
        checksum(&bytes[..end]) == u64::from_le_bytes(bytes[end..].try_into()?),
        "World metadata checksum mismatch"
    );
    ensure!(bytes[8] <= 1, "Invalid favorite value");
    let favorite = bytes[8] != 0;
    let len = u32::from_le_bytes(bytes[9..13].try_into()?) as usize;
    ensure!(len <= 256 && 13 + len == end, "Invalid world name");
    let name = std::str::from_utf8(&bytes[13..end])?;
    ensure!(
        name.chars().count() <= 64 && !name.chars().any(char::is_control),
        "Invalid world name"
    );
    Ok(Metadata {
        name: name.to_owned(),
        favorite,
    })
}
fn edit_metadata(id: &str, name: Option<String>, favorite: Option<bool>) -> Result<()> {
    ensure!(
        !id.starts_with("legacy-"),
        "Save this legacy world once before renaming it"
    );
    super::paths::import_world(id)?;
    let dir = root(id)?;
    fs::create_dir_all(&dir)?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(dir.join(".metadata-lock"))?;
    lock.lock()?;
    let mut meta = metadata(id)?;
    if let Some(name) = name {
        let name = name.trim();
        ensure!(
            !name.is_empty()
                && name.chars().count() <= 64
                && name.len() <= 256
                && !name.chars().any(char::is_control),
            "Use a world name of 1–64 characters"
        );
        meta.name = name.to_owned();
    }
    if let Some(favorite) = favorite {
        meta.favorite = favorite;
    }
    let mut bytes = b"VXWMETA1".to_vec();
    bytes.push(u8::from(meta.favorite));
    bytes.extend_from_slice(&(meta.name.len() as u32).to_le_bytes());
    bytes.extend_from_slice(meta.name.as_bytes());
    bytes.extend_from_slice(&checksum(&bytes).to_le_bytes());
    background::write_atomic(
        &dir.join("world.meta"),
        &bytes,
        super::super::regions::checkpoint_id(),
    )?;
    Ok(())
}
fn scan() -> Result<Vec<WorldEntry>> {
    let mut grouped: HashMap<String, Vec<Header>> = HashMap::new();
    let mut damaged = Vec::new();
    let mut paths = slot_paths();
    for directory in super::paths::library_directories() {
        if let Ok(dirs) = fs::read_dir(directory.join("regions")) {
            for dir in dirs {
                let dir = dir?;
                if !dir.file_type()?.is_dir() {
                    continue;
                }
                let id = dir.file_name().to_string_lossy().to_string();
                if !valid_id(&id) {
                    continue;
                }
                let saved = commits(&dir.path());
                if saved.is_empty() {
                    continue;
                }
                let mut found = false;
                for path in saved {
                    match header(&path) {
                        Ok(h) if h.id == id => {
                            grouped.entry(id.clone()).or_default().push(h);
                            found = true;
                        }
                        _ => {}
                    }
                }
                if !found {
                    damaged.push(WorldEntry {
                        id: id.clone(),
                        name: id,
                        favorite: false,
                        seed: 0,
                        kind: WorldType::Hills,
                        scale: VoxelScalePreset::OneMeter,
                        saved: 0,
                        chunks: 0,
                        checkpoints: 0,
                        problem: Some(
                            "No readable checkpoint header. Recovery may still be possible.".into(),
                        ),
                    });
                }
                ensure!(
                    grouped.len() + damaged.len() <= 10_000,
                    "World library exceeds 10,000 entries"
                );
            }
        }
    }
    for path in paths.drain(..) {
        if let Ok(h) = header(&path) {
            grouped.entry(h.id.clone()).or_default().push(h);
        }
    }
    damaged.retain(|e| !grouped.contains_key(&e.id));
    let mut entries = damaged;
    for (id, mut headers) in grouped {
        headers.sort_by_key(|h| std::cmp::Reverse(h.saved));
        headers.dedup_by_key(|h| h.saved);
        let h = &headers[0];
        let meta = metadata(&id);
        let problem = meta
            .as_ref()
            .err()
            .map(|e| format!("Name metadata needs attention: {e:#}"));
        let meta = meta.unwrap_or_default();
        entries.push(WorldEntry {
            id,
            name: if meta.name.is_empty() {
                default_name(h)
            } else {
                meta.name
            },
            favorite: meta.favorite,
            kind: h.kind,
            scale: h.scale,
            seed: h.seed,
            saved: h.saved,
            chunks: h.chunks,
            checkpoints: headers.len(),
            problem,
        });
    }
    entries.sort_by_key(|e| std::cmp::Reverse(e.saved));
    Ok(entries)
}
fn load_selected(id: &str) -> Result<SavedSession> {
    ensure!(valid_id(id), "Invalid world identity");
    if !id.starts_with("legacy-") {
        super::paths::import_world(id)?;
    }
    let mut candidates = Vec::new();
    if !id.starts_with("legacy-") {
        for path in commits(&root(id)?) {
            // A damaged primary must not prevent trying an older complete commit.
            let saved = match header(&path) {
                Ok(h) if h.id == id => h.saved,
                Ok(_) => continue,
                Err(_) => 0,
            };
            candidates.push((saved, path));
        }
    }
    for path in slot_paths() {
        if let Ok(h) = header(&path) {
            if h.id == id {
                candidates.push((h.saved, path));
            }
        }
    }
    ensure!(
        !candidates.is_empty(),
        "No checkpoint is available for this world"
    );
    candidates.sort_by_key(|(saved, _)| std::cmp::Reverse(*saved));
    let mut errors = Vec::new();
    for (index, (_, path)) in candidates.into_iter().enumerate() {
        match load_exact(&path, &quicksave_path()) {
            Ok(mut session) => {
                if !id.starts_with("legacy-") {
                    ensure!(
                        session.world.regions.view.session_name() == id,
                        "Checkpoint belongs to another world"
                    );
                }
                if index > 0 {
                    session.recovery_notice =
                        Some("Recovered an earlier complete checkpoint for this world".into());
                }
                return Ok(session);
            }
            Err(e) => errors.push(format!("{e:#}")),
        }
    }
    anyhow::bail!(
        "Cannot open this world: {}",
        errors.into_iter().take(3).collect::<Vec<_>>().join("; ")
    )
}
enum Command {
    Scan,
    Load(String),
    Edit {
        id: String,
        name: Option<String>,
        favorite: Option<bool>,
    },
}
fn guarded<T>(job: impl FnOnce() -> Result<T>) -> Result<T> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(job)).unwrap_or_else(|_| {
        Err(anyhow::anyhow!(
            "World library operation stopped unexpectedly"
        ))
    })
}
pub enum LibraryEvent {
    Entries(Result<Vec<WorldEntry>>),
    Loaded(Result<SavedSession>),
    Edited(Result<()>),
}
pub struct WorldLibrary {
    tx: mpsc::SyncSender<Command>,
    rx: mpsc::Receiver<LibraryEvent>,
    queue: VecDeque<Command>,
    active: bool,
    loading: bool,
    pub entries: Arc<Vec<WorldEntry>>,
    pub status: Option<String>,
    pub revision: u64,
}
impl WorldLibrary {
    pub fn new(enabled: bool) -> Self {
        let (tx, jobs) = mpsc::sync_channel::<Command>(1);
        let (done, rx) = mpsc::sync_channel(1);
        let status = if enabled {
            std::thread::Builder::new()
                .name("world-library".into())
                .spawn(move || {
                    while let Ok(job) = jobs.recv() {
                        let result = match job {
                            Command::Scan => LibraryEvent::Entries(guarded(scan)),
                            Command::Load(id) => {
                                LibraryEvent::Loaded(guarded(|| load_selected(&id)))
                            }
                            Command::Edit { id, name, favorite } => {
                                LibraryEvent::Edited(guarded(|| edit_metadata(&id, name, favorite)))
                            }
                        };
                        if done.send(result).is_err() {
                            break;
                        }
                    }
                })
                .err()
                .map(|e| format!("Cannot start world library: {e}"))
        } else {
            None
        };
        let mut library = Self {
            tx,
            rx,
            queue: VecDeque::new(),
            active: false,
            loading: false,
            entries: Arc::default(),
            status,
            revision: 0,
        };
        if enabled {
            library.refresh();
        }
        library
    }
    pub fn busy(&self) -> bool {
        self.active || !self.queue.is_empty()
    }
    pub fn loading(&self) -> bool {
        self.loading || self.queue.iter().any(|q| matches!(q, Command::Load(_)))
    }
    fn enqueue(&mut self, command: Command) {
        if self.queue.len() >= 8 {
            self.status = Some("The library is busy; try again shortly".into());
            return;
        }
        self.queue.push_back(command);
    }
    pub fn refresh(&mut self) {
        if !self.queue.iter().any(|q| matches!(q, Command::Scan)) {
            self.enqueue(Command::Scan);
        }
    }
    pub fn open(&mut self, id: String) {
        if !self.loading() {
            self.enqueue(Command::Load(id));
        }
    }
    pub fn rename(&mut self, id: String, name: String) {
        self.enqueue(Command::Edit {
            id,
            name: Some(name),
            favorite: None,
        });
    }
    pub fn favorite(&mut self, id: String, favorite: bool) {
        self.enqueue(Command::Edit {
            id,
            name: None,
            favorite: Some(favorite),
        });
    }
    pub fn poll(&mut self) -> Option<Result<SavedSession>> {
        let mut loaded = None;
        if self.active {
            match self.rx.try_recv() {
                Ok(event) => {
                    self.active = false;
                    self.loading = false;
                    match event {
                        LibraryEvent::Entries(result) => match result {
                            Ok(entries) => {
                                self.entries = Arc::new(entries);
                                self.revision += 1;
                                self.status = None;
                            }
                            Err(e) => self.status = Some(format!("Library scan failed: {e:#}")),
                        },
                        LibraryEvent::Edited(result) => match result {
                            Ok(()) => {
                                self.refresh();
                                self.status = Some("World details saved".into());
                            }
                            Err(e) => {
                                self.status = Some(format!("Cannot save world details: {e:#}"))
                            }
                        },
                        LibraryEvent::Loaded(result) => loaded = Some(result),
                    }
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.active = false;
                    self.loading = false;
                    self.queue.clear();
                    self.status = Some("World library worker stopped".into());
                }
            }
        }
        if !self.active {
            if let Some(command) = self.queue.pop_front() {
                let loading = matches!(command, Command::Load(_));
                match self.tx.try_send(command) {
                    Ok(()) => {
                        self.active = true;
                        self.loading = loading;
                    }
                    Err(mpsc::TrySendError::Full(command)) => self.queue.push_front(command),
                    Err(mpsc::TrySendError::Disconnected(_)) => {
                        self.status = Some("World library is unavailable".into())
                    }
                }
            }
        }
        loaded
    }
}
