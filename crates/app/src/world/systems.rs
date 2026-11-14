//! World systems: whole gameplay pillars (machines, magic, space...) that a
//! world can leave out. The catalogue is `assets/systems.ron`; a world keeps
//! the ids it switched off in a text sidecar, so a world made before a system
//! existed plays with it on, exactly as it always did. No save version change.
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

pub const FILE: &str = "systems.ron";
const CATALOG_RON: &str = include_str!("../../../../assets/systems.ron");

/// One catalogue row; see the header of `assets/systems.ron`.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Def {
    pub bit: u8,
    pub id: String,
    pub label: String,
    pub hint: String,
    #[serde(default)]
    pub requires: Vec<String>,
    #[serde(default)]
    pub tabs: Vec<u32>,
    #[serde(default)]
    pub parity: Vec<String>,
}

/// Every system, in catalogue order. A broken table logs once and leaves
/// every system on, which is how the game played before switches existed.
pub fn catalog() -> &'static [Def] {
    static CATALOG: OnceLock<Vec<Def>> = OnceLock::new();
    CATALOG.get_or_init(|| match ron::from_str::<Vec<Def>>(CATALOG_RON) {
        Ok(rows) if rows.iter().all(|d| d.bit < 64) => rows,
        Ok(_) => {
            eprintln!("[systems] assets/systems.ron: bit must be below 64");
            Vec::new()
        }
        Err(e) => {
            eprintln!("[systems] assets/systems.ron: {e}");
            Vec::new()
        }
    })
}

pub fn def(id: &str) -> Option<&'static Def> {
    catalog().iter().find(|d| d.id == id)
}

/// The systems one world plays with, as a mask of the ones switched off.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Systems {
    off: u64,
}

#[derive(Default, Deserialize, Serialize)]
struct Sidecar {
    off: Vec<String>,
}

impl Systems {
    /// Unknown ids are on: code may ask about a system before its row exists.
    pub fn enabled(self, id: &str) -> bool {
        def(id).is_none_or(|d| self.off & (1 << d.bit) == 0)
    }

    pub fn automation(self) -> bool {
        self.enabled("automation")
    }

    pub fn magic(self) -> bool {
        self.enabled("magic")
    }

    /// Switches one system, carrying its requirements along: switching on
    /// also switches on what it requires; switching off also switches off
    /// everything that requires it. Returns false for an unknown id.
    pub fn set(&mut self, id: &str, on: bool) -> bool {
        let Some(d) = def(id) else { return false };
        if on {
            self.off &= !(1 << d.bit);
            for need in &d.requires {
                if !self.enabled(need) {
                    self.set(need, true);
                }
            }
        } else {
            self.off |= 1 << d.bit;
            for other in catalog() {
                if other.requires.iter().any(|r| r == id) && self.enabled(&other.id) {
                    self.set(&other.id, false);
                }
            }
        }
        true
    }

    /// False when a switched-off system owns inventory page `page`.
    pub fn tab_visible(self, page: u32) -> bool {
        catalog().iter().all(|d| self.enabled(&d.id) || !d.tabs.contains(&page))
    }

    /// Ids switched off, in catalogue order.
    pub fn off_ids(self) -> Vec<&'static str> {
        catalog().iter().filter(|d| !self.enabled(&d.id)).map(|d| d.id.as_str()).collect()
    }

    /// The systems in `world_dir`. A missing file means every system is on;
    /// ids the catalogue no longer has are ignored with a log line.
    pub fn load(world_dir: &Path) -> Result<Self> {
        let path = world_dir.join(FILE);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(e).with_context(|| format!("Cannot read {}", path.display())),
        };
        let file: Sidecar = ron::from_str(&text).with_context(|| path.display().to_string())?;
        let mut systems = Self::default();
        for id in &file.off {
            if !systems.set(id, false) {
                eprintln!("[systems] {}: unknown system {id:?} ignored", path.display());
            }
        }
        Ok(systems)
    }

    /// Writes the sidecar through a temporary; with every system on the
    /// file is removed so the folder looks like any older world.
    pub fn save(self, world_dir: &Path) -> Result<()> {
        let path = world_dir.join(FILE);
        if self.off == 0 {
            return match std::fs::remove_file(&path) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                    Err(e).with_context(|| format!("Cannot remove {}", path.display()))
                }
                _ => Ok(()),
            };
        }
        let file = Sidecar { off: self.off_ids().into_iter().map(str::to_owned).collect() };
        let text = ron::ser::to_string_pretty(&file, ron::ser::PrettyConfig::default())?;
        let tmp = world_dir.join(format!("{FILE}.tmp"));
        std::fs::write(&tmp, text).with_context(|| format!("Cannot write {}", tmp.display()))?;
        std::fs::rename(&tmp, &path).with_context(|| format!("Cannot publish {}", path.display()))
    }

    /// Makes these the systems menus show; the simulation reads the world's own.
    pub fn publish(self) {
        ACTIVE.store(self.off, Ordering::Relaxed);
    }

    /// The systems of the open world for presentation code without a world
    /// handle. Every system shows when no world has published.
    pub fn active() -> Self {
        Self { off: ACTIVE.load(Ordering::Relaxed) }
    }
}

static ACTIVE: AtomicU64 = AtomicU64::new(0);
