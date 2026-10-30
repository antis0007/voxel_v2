//! Immutable, content-addressed cold pages. The observation checkpoint is the
//! manifest: a page is published there only after its synced write succeeds.
use super::{
    Knowledge, Patch,
    pages::{Key, Page, SIDE},
};
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
const PAGE_BYTES: u64 = 256 * 1024;
pub const MAX_COLD: usize = 65_536;
const RESIDENT_TARGET: usize = 65_536;
const BATCH: usize = 4;

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct Cold {
    pub digest: u64,
    pub count: u16,
}

enum Completed {
    Written(Vec<(Key, Arc<Page>, Cold)>),
    Read(Vec<(Key, u64, Result<Page, String>)>),
}

#[derive(Default)]
pub struct Pager {
    root: Option<PathBuf>,
    job: Option<std::thread::JoinHandle<Completed>>,
    wanted: Mutex<BTreeSet<Key>>,
    failed: BTreeSet<Key>,
    recent: Mutex<BTreeMap<Key, std::time::Instant>>,
    retry_at: Option<std::time::Instant>,
    pub cold_layers: BTreeMap<(u64, [i32; 3]), BTreeSet<u32>>,
    pub error: bool,
}

fn digest(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
    })
}
fn path(root: &Path, hash: u64) -> PathBuf {
    root.join(format!("{hash:016x}.ron"))
}

impl Pager {
    pub fn retry(&mut self) {
        self.failed.clear();
        self.error = false;
        self.retry_at = None;
    }
    pub fn open(&mut self, archive: &Path) {
        self.root = Some(archive.with_extension("pages"));
    }
    pub fn request(&self, key: Key) {
        if self.failed.contains(&key) {
            return;
        }
        let mut wanted = self.wanted.lock().unwrap_or_else(|e| e.into_inner());
        if wanted.len() < 64 {
            wanted.insert(key);
        }
    }
    pub fn touch(&self, key: Key) {
        let mut recent = self.recent.lock().unwrap_or_else(|e| e.into_inner());
        if recent.len() < 8192 || recent.contains_key(&key) {
            recent.insert(key, std::time::Instant::now());
        }
    }
    pub fn request_known_cell(&self, scope: u64, cell: [i32; 3]) -> bool {
        let coord = cell.map(|v| v.div_euclid(SIDE));
        let Some(layers) = self.cold_layers.get(&(scope, coord)) else {
            return false;
        };
        for &layer in layers {
            self.request((layer, coord));
        }
        true
    }
}

impl Knowledge {
    pub(super) fn request_cell(&self, layer: u32, cell: [i32; 3]) -> bool {
        let key = (layer, cell.map(|v| v.div_euclid(SIDE)));
        if self.saved.cold.contains_key(&key) {
            self.pager.request(key);
            true
        } else {
            false
        }
    }

    pub(super) fn paging(&mut self, wait: bool) {
        if self
            .pager
            .job
            .as_ref()
            .is_some_and(|job| wait || job.is_finished())
        {
            match self.pager.job.take().unwrap().join() {
                Ok(Completed::Written(pages)) => {
                    if pages.is_empty() {
                        self.pager.error = true;
                        self.pager.retry_at =
                            Some(std::time::Instant::now() + std::time::Duration::from_secs(5));
                    }
                    for (key, snapshot, cold) in pages {
                        // A new observation may have copied this page while the
                        // worker wrote it. Keep the newer resident version.
                        if self
                            .saved
                            .patches
                            .pages
                            .get(&key)
                            .is_some_and(|p| Arc::ptr_eq(p, &snapshot))
                        {
                            self.saved.patches.remove_page(key);
                            self.saved.cold.insert(key, cold);
                            let scope = self.saved.layers[key.0 as usize].scope;
                            self.pager
                                .cold_layers
                                .entry((scope, key.1))
                                .or_default()
                                .insert(key.0);
                            for &cell in snapshot.keys() {
                                let index = (scope, cell);
                                if let Some(layers) = self.locations.get_mut(&index) {
                                    layers.remove(&key.0);
                                }
                                if self.locations.get(&index).is_some_and(BTreeSet::is_empty) {
                                    self.locations.remove(&index);
                                }
                                if self.footsteps.get(&index) == Some(&key.0) {
                                    self.footsteps.remove(&index);
                                }
                            }
                            self.dirty = true;
                        }
                    }
                }
                Ok(Completed::Read(pages)) => {
                    for (key, hash, page) in pages {
                        if self.saved.cold.get(&key).is_none_or(|p| p.digest != hash) {
                            continue;
                        }
                        match page {
                            Ok(page)
                                if self.saved.patches.len() + page.len() <= super::MAX_PATCHES =>
                            {
                                self.saved.cold.remove(&key);
                                let chart = &self.saved.layers[key.0 as usize];
                                if let Some(layers) =
                                    self.pager.cold_layers.get_mut(&(chart.scope, key.1))
                                {
                                    layers.remove(&key.0);
                                }
                                if self
                                    .pager
                                    .cold_layers
                                    .get(&(chart.scope, key.1))
                                    .is_some_and(BTreeSet::is_empty)
                                {
                                    self.pager.cold_layers.remove(&(chart.scope, key.1));
                                }
                                if chart.report.is_none() {
                                    for (&cell, patch) in &page {
                                        self.locations
                                            .entry((chart.scope, cell))
                                            .or_default()
                                            .insert(key.0);
                                        if patch.travelled {
                                            self.footsteps.insert((chart.scope, cell), key.0);
                                        }
                                    }
                                }
                                self.saved.patches.insert_page(key, page);
                                // Moving between cache and manifest is persistent
                                // representation only, not new discovery.
                                self.dirty = true;
                            }
                            Ok(_) => {
                                self.pager.request(key);
                            }
                            Err(_) => {
                                self.pager.failed.insert(key);
                                self.pager.error = true;
                            }
                        }
                    }
                }
                Err(_) => {
                    self.pager.error = true;
                }
            }
        }
        if wait || self.pager.job.is_some() || self.loading.is_some() {
            return;
        }
        if self
            .pager
            .retry_at
            .is_some_and(|at| std::time::Instant::now() < at)
        {
            return;
        }
        let mut recent = self.pager.recent.lock().unwrap_or_else(|e| e.into_inner());
        recent.retain(|_, t| t.elapsed().as_secs() < 3);
        let Some(root) = self.pager.root.clone().filter(|_| self.path.is_some()) else {
            return;
        };
        // Keep decoded metadata and archives untouched until the owning load is
        // complete. Choose far pages by chart-local geometry, never world reads.
        let can_read = self.saved.patches.len() <= super::MAX_PATCHES - BATCH * 512
            && self
                .pager
                .wanted
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .iter()
                .any(|key| self.saved.cold.contains_key(key));
        if !can_read
            && self.saved.patches.len() > RESIDENT_TARGET
            && self.saved.cold.len() + BATCH <= MAX_COLD
        {
            let wanted = self.pager.wanted.lock().unwrap_or_else(|e| e.into_inner());
            // Retain only the four eviction candidates. A large survey must not
            // clone every page handle and sort the entire archive each frame.
            let mut candidates: Vec<(f32, Key)> = Vec::with_capacity(BATCH + 1);
            for (&key, _) in self
                .saved
                .patches
                .pages
                .iter()
                .filter(|(key, _)| !wanted.contains(key) && !recent.contains_key(key))
            {
                let center = (Vec3::from_array(key.1.map(|v| v as f32)) + Vec3::splat(0.5))
                    * SIDE as f32
                    * self.scale;
                let distance = center.distance_squared(self.feet)
                    + if Some(key.0) != self.active {
                        1_000_000.0
                    } else {
                        0.0
                    };
                let at = candidates.partition_point(|(d, _)| *d >= distance);
                if at < BATCH {
                    candidates.insert(at, (distance, key));
                    candidates.truncate(BATCH);
                }
            }
            drop(wanted);
            let pages: Vec<_> = candidates
                .into_iter()
                .map(|(_, key)| (key, self.saved.patches.pages[&key].clone()))
                .collect();
            if !pages.is_empty() {
                self.pager.job = std::thread::Builder::new()
                    .name("map-page-write".into())
                    .spawn(move || {
                        let mut written = Vec::new();
                        if std::fs::create_dir_all(&root).is_err() {
                            return Completed::Written(written);
                        }
                        for (key, page) in pages {
                            let Ok(body) = ron::to_string(&*page) else {
                                continue;
                            };
                            if body.len() as u64 > PAGE_BYTES {
                                continue;
                            }
                            let hash = digest(body.as_bytes());
                            if super::sidecar::write_atomic(&path(&root, hash), body.as_bytes())
                                .is_ok()
                            {
                                written.push((
                                    key,
                                    page.clone(),
                                    Cold {
                                        digest: hash,
                                        count: page.len() as u16,
                                    },
                                ));
                            }
                        }
                        Completed::Written(written)
                    })
                    .ok();
                return;
            }
        }
        drop(recent);
        let mut wanted = self.pager.wanted.lock().unwrap_or_else(|e| e.into_inner());
        let keys: Vec<_> = wanted.iter().take(BATCH).copied().collect();
        for key in &keys {
            wanted.remove(key);
        }
        drop(wanted);
        let reads: Vec<_> = keys
            .into_iter()
            .filter_map(|key| self.saved.cold.get(&key).map(|cold| (key, *cold)))
            .collect();
        if reads.is_empty() {
            return;
        }
        self.pager.job = std::thread::Builder::new()
            .name("map-page-read".into())
            .spawn(move || {
                let pages = reads
                    .into_iter()
                    .map(|(key, cold)| {
                        let load = || -> Result<Page, String> {
                            let file = std::fs::File::open(path(&root, cold.digest))
                                .map_err(|e| e.to_string())?;
                            let mut body = String::new();
                            file.take(PAGE_BYTES + 1)
                                .read_to_string(&mut body)
                                .map_err(|e| e.to_string())?;
                            if body.len() as u64 > PAGE_BYTES
                                || digest(body.as_bytes()) != cold.digest
                            {
                                return Err("Invalid page".into());
                            }
                            let page: BTreeMap<[i32; 3], Patch> =
                                ron::from_str(&body).map_err(|e| e.to_string())?;
                            if page.len() != cold.count as usize
                                || page.len() > (SIDE * SIDE * SIDE) as usize
                                || page
                                    .keys()
                                    .any(|cell| cell.map(|v| v.div_euclid(SIDE)) != key.1)
                            {
                                return Err("Invalid page".into());
                            }
                            Ok(page)
                        };
                        (key, cold.digest, load())
                    })
                    .collect();
                Completed::Read(pages)
            })
            .ok();
    }
}
