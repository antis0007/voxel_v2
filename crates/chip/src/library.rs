//! A player's chip projects, their immutable released revisions, and the
//! packaged parts built from them. A revision freezes its design and runtime
//! model: later edits make new revisions and never change installed parts.
use crate::characterize::Model;
use crate::compile::{COMPILER_VERSION, compile};
use crate::design::Design;
use crate::fab::{Grade, MaskSet, Provenance};
use crate::lot::WaferLot;
use crate::registry::Registry;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub id: u32,
    pub name: String,
    pub draft: Design,
    /// Edit counter of the draft; a release names the one it was shown.
    #[serde(skip)]
    pub generation: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Revision {
    pub id: u32,
    pub project: u32,
    /// 1, 2, ... within the project; shown as "A", "B", ...
    pub number: u32,
    pub fingerprint: u64,
    pub compiler: u32,
    pub design: Design,
    pub model: Model,
}

impl Revision {
    pub fn letter(&self) -> String {
        let mut n = self.number.max(1) - 1;
        let mut s = String::new();
        loop {
            s.insert(0, char::from(b'A' + (n % 26) as u8));
            if n < 26 {
                return s;
            }
            n = n / 26 - 1;
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Package {
    pub serial: u32,
    pub revision: u32,
    pub provenance: Provenance,
    #[serde(default)]
    pub grade: Grade,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReleaseError {
    UnknownProject,
    /// The draft changed after the report the player released from.
    Stale,
    Failed(Vec<String>),
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Library {
    pub projects: Vec<Project>,
    pub revisions: Vec<Revision>,
    /// Packaged parts not installed in any machine.
    pub stock: Vec<Package>,
    /// Mask sets, one per fabricated revision, reused by every lot.
    #[serde(default)]
    pub masks: Vec<MaskSet>,
    #[serde(default)]
    pub lots: Vec<WaferLot>,
    next_project: u32,
    next_revision: u32,
    pub(crate) next_serial: u32,
    #[serde(default)]
    pub(crate) next_lot: u32,
    /// Changes whenever a library is created or loaded, so views can tell a
    /// different library apart from an edited one.
    #[serde(skip)]
    pub session: u64,
}

/// FNV-1a over the design's canonical text and the compiler version.
pub fn fingerprint(design: &Design) -> u64 {
    let text = ron::to_string(design).unwrap_or_default();
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in text.bytes().chain(COMPILER_VERSION.to_le_bytes()) {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

impl Library {
    pub fn add_project(&mut self, name: &str, draft: Design) -> u32 {
        self.next_project += 1;
        let id = self.next_project;
        self.projects.push(Project { id, name: name.to_string(), draft, generation: 0 });
        id
    }

    pub fn project(&self, id: u32) -> Option<&Project> {
        self.projects.iter().find(|p| p.id == id)
    }

    /// Edit a draft; bumps its generation.
    pub fn edit<R>(&mut self, id: u32, f: impl FnOnce(&mut Design) -> R) -> Option<R> {
        let p = self.projects.iter_mut().find(|p| p.id == id)?;
        p.generation += 1;
        Some(f(&mut p.draft))
    }

    pub fn revision(&self, id: u32) -> Option<&Revision> {
        self.revisions.iter().find(|r| r.id == id)
    }

    pub fn model(&self, revision: u32) -> Option<&Model> {
        self.revision(revision).map(|r| &r.model)
    }

    pub fn revisions_of(&self, project: u32) -> impl Iterator<Item = &Revision> {
        self.revisions.iter().filter(move |r| r.project == project)
    }

    /// Release the draft the player saw at `generation`. An unchanged design
    /// returns its existing revision instead of minting a duplicate.
    pub fn release(&mut self, registry: &Registry, project: u32, generation: u64) -> Result<u32, ReleaseError> {
        let p = self.project(project).ok_or(ReleaseError::UnknownProject)?;
        if p.generation != generation {
            return Err(ReleaseError::Stale);
        }
        let compiled = compile(&p.draft, registry);
        let model = match (&compiled.report, compiled.ok()) {
            (Some(r), true) => r.model,
            _ => return Err(ReleaseError::Failed(compiled.errors().map(|d| d.message.clone()).collect())),
        };
        let design = p.draft.clone();
        let fp = fingerprint(&design);
        if let Some(existing) = self.revisions_of(project).find(|r| r.fingerprint == fp) {
            return Ok(existing.id);
        }
        let number = self.revisions_of(project).count() as u32 + 1;
        self.next_revision += 1;
        let id = self.next_revision;
        let revision = Revision { id, project, number, fingerprint: fp, compiler: COMPILER_VERSION, design, model };
        self.revisions.push(revision);
        Ok(id)
    }

    /// A packaged part of a released revision, into stock. The caller has
    /// already paid its materials (`bill`).
    pub fn package(&mut self, revision: u32, provenance: Provenance) -> Option<u32> {
        self.revision(revision)?;
        self.next_serial += 1;
        let serial = self.next_serial;
        self.stock.push(Package { serial, revision, provenance, grade: Grade::A });
        Some(serial)
    }

    pub fn take(&mut self, serial: u32) -> Option<Package> {
        let i = self.stock.iter().position(|p| p.serial == serial)?;
        Some(self.stock.remove(i))
    }

    pub fn put_back(&mut self, package: Package) {
        if !self.stock.iter().any(|p| p.serial == package.serial) {
            self.stock.push(package);
            self.stock.sort_by_key(|p| p.serial);
        }
    }

    /// Prepared-blank materials: one unit per 8 die sites, plus one per metal layer.
    pub fn bill(&self, revision: u32) -> Option<u32> {
        let d = &self.revision(revision)?.design;
        Some((u32::from(d.width) * u32::from(d.height)).div_ceil(8) + u32::from(d.layers))
    }

    /// Reject a decoded library whose cross-references do not hold.
    pub fn validate(&self) -> Result<(), String> {
        for r in &self.revisions {
            if self.project(r.project).is_none() || r.id > self.next_revision {
                return Err(format!("revision {} has no project or a bad id", r.id));
            }
        }
        for (i, p) in self.stock.iter().enumerate() {
            if self.revision(p.revision).is_none() || p.serial > self.next_serial || self.stock[..i].iter().any(|q| q.serial == p.serial) {
                return Err(format!("package {} is invalid or duplicated", p.serial));
            }
        }
        for l in &self.lots {
            if self.revision(l.revision).is_none() || !self.masks.iter().any(|m| m.id == l.mask_set) || l.id > self.next_lot {
                return Err(format!("lot {} has no revision or mask set", l.id));
            }
        }
        Ok(())
    }

    /// Serial numbers are minted only here; the owner checks installed ones.
    pub fn serial_issued(&self, serial: u32) -> bool {
        serial > 0 && serial <= self.next_serial
    }
}
