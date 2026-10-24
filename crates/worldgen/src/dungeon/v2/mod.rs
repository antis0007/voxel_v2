//! Generator 32 complexes. A complex has a job (its stone, its rooms, its
//! hazard), a footprint of five to twelve rooms on a 30 m lattice, up to three
//! levels joined by ramps, one tall reveal hall with a balcony that links the
//! top two levels, and a sealed core reached only through a puzzle room.
//! Rooms and corridors are built from one construction kit (piers, ribs,
//! vaults, beams, lintels), so a complex reads as built rather than dug.
//!
//! Planning runs once per complex (`plan.rs`); a voxel costs one grid lookup
//! and a few box tests (`Index`).
use super::Building;
use crate::{BlockId as B, WorldSampleConfig};
use glam::Vec3;

mod chasm;
mod dress;
mod footprint;
mod index;
mod job;
mod kit;
mod landmark;
mod maze;
mod outline;
mod plan;
mod starry;
mod treasure;
#[cfg(test)]
mod tests;

pub use job::Job;
pub use index::Index;
use index::CORR_BIT;
pub use outline::Outline;
pub use plan::generate;

/// First generator with job-driven complexes.
pub const VERSION: u32 = 32;
/// Height between levels, metres.
pub const LEVEL: f32 = 6.0;
/// Shell thickness around rooms and corridors, metres.
pub const WALL: f32 = 1.5;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// Foot of the shaft; the first safe floor.
    Entry,
    /// Two levels tall, balcony ring at the upper level, the job's landmark.
    Reveal,
    Ossuary,
    Crypt,
    Shrine,
    Barracks,
    Cistern,
    Sluice,
    Store,
    Forge,
    Foundry,
    Garden,
    Cells,
    Orrery,
    Library,
    Pitfall,
    Labyrinth,
    /// Split by a rift, crossed by bridges.
    Chasm,
    /// The objective: sealed in tombstone, entered through one puzzle room.
    Core,
}

impl Kind {
    pub fn is_puzzle(self) -> bool {
        matches!(self, Self::Pitfall | Self::Labyrinth)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Damage {
    Intact,
    /// A ceiling breach with a rubble cone; a crawl way stays open.
    Collapsed,
    /// Standing water over the floor.
    Flooded,
    /// Fungal soil, glowing caps and roots through the ceiling.
    Overgrown,
    /// A cave broke through one wall.
    Breach,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ceiling {
    /// Flat, with beams across the short span at every bay.
    Beams,
    /// Barrel vault with ribs landing on the piers.
    Ribbed,
    /// Stepped corbel vault.
    Corbel,
    /// Flat with coffers and an oculus; reveal halls only.
    Coffered,
}

#[derive(Clone, Debug)]
pub struct Room {
    pub x0: f32,
    pub x1: f32,
    pub z0: f32,
    pub z1: f32,
    pub floor: f32,
    /// Clear height at the walls.
    pub h: f32,
    pub level: u8,
    pub kind: Kind,
    pub damage: Damage,
    pub ceiling: Ceiling,
    pub outline: Outline,
    /// Depth of a cleft's rift, metres; zero elsewhere.
    pub rift: f32,
    /// A dead end that pays for the detour: a salvage cache on a plinth.
    pub cache: bool,
    /// Tombstone shell: no digging into or out of this room.
    pub sealed: bool,
    /// Per-room hash salt for layouts.
    pub salt: u32,
}

impl Room {
    pub fn centre(&self) -> (f32, f32) {
        ((self.x0 + self.x1) * 0.5, (self.z0 + self.z1) * 0.5)
    }
    pub fn w(&self) -> f32 {
        self.x1 - self.x0
    }
    pub fn d(&self) -> f32 {
        self.z1 - self.z0
    }
    /// Space below the floor that belongs to the room (pits, basins, trenches).
    pub fn sub(&self) -> f32 {
        match self.kind {
            Kind::Pitfall => 3.0,
            Kind::Cistern | Kind::Sluice => 2.0,
            Kind::Foundry => 1.0,
            Kind::Reveal => 4.0,
            Kind::Chasm => self.rift + 1.0,
            Kind::Orrery if starry::plan(self) == starry::Plan::Islands => starry::VOID,
            Kind::Orrery => 1.2,
            _ => 0.0,
        }
    }
    /// Highest point of the ceiling above the floor.
    pub fn top(&self) -> f32 {
        self.h + kit::rise(self) + 1.0
    }
    fn inside(&self, m: Vec3) -> bool {
        m.x >= self.x0 && m.x < self.x1 && m.z >= self.z0 && m.z < self.z1
            && m.y >= self.floor - self.sub() - 1.0 && m.y < self.floor + self.top()
    }
    fn in_shell(&self, m: Vec3) -> bool {
        m.x >= self.x0 - WALL && m.x < self.x1 + WALL && m.z >= self.z0 - WALL && m.z < self.z1 + WALL
            && m.y >= self.floor - self.sub() - 1.0 - WALL && m.y < self.floor + self.top() + WALL
    }
}

#[derive(Clone, Debug)]
pub struct Corr {
    pub x0: f32,
    pub x1: f32,
    pub z0: f32,
    pub z1: f32,
    pub along_x: bool,
    pub a0: f32,
    pub a1: f32,
    pub fa: f32,
    pub fb: f32,
    pub h: f32,
    /// Pointed (stepped) roof instead of a flat lintel.
    pub pointed: bool,
    /// Loose fill across the passage (a sealed or caved-in door).
    pub plug: Option<B>,
}

impl Corr {
    pub fn floor_at(&self, x: f32, z: f32) -> f32 {
        let c = if self.along_x { x } else { z };
        let span = self.a1 - self.a0;
        let t = if span.abs() < 1e-3 { 0.0 } else { ((c - self.a0) / span).clamp(0.0, 1.0) };
        self.fa + (self.fb - self.fa) * t
    }
    fn lo(&self) -> f32 {
        self.fa.min(self.fb)
    }
    fn hi(&self) -> f32 {
        self.fa.max(self.fb)
    }
    fn inside(&self, m: Vec3) -> bool {
        m.x >= self.x0 && m.x < self.x1 && m.z >= self.z0 && m.z < self.z1
            && m.y >= self.lo() - 1.0 && m.y < self.hi() + self.h + 1.0
            && {
                let f = self.floor_at(m.x, m.z);
                m.y >= f - 1.0 && m.y < f + self.h + 1.0
            }
    }
    fn in_shell(&self, m: Vec3) -> bool {
        let pad = 1.0;
        m.x >= self.x0 - pad && m.x < self.x1 + pad && m.z >= self.z0 - pad && m.z < self.z1 + pad && {
            let f = self.floor_at(m.x, m.z);
            m.y >= f - 1.0 - pad && m.y < f + self.h + 1.0 + pad
        }
    }
}

pub struct Complex {
    pub job: Job,
    pub pal: job::Palette,
    pub seed: u32,
    pub ground: f32,
    pub rooms: Vec<Room>,
    pub corrs: Vec<Corr>,
    /// Shaft centre and the surface it climbs to.
    pub shaft: (f32, f32, f32),
    pub gatehouse: Building,
    pub entry: usize,
    pub reveal: usize,
    pub objective: Option<usize>,
    /// Two clear floor points for fights: near the core, and in the reveal hall.
    pub anchors: [Vec3; 2],
    pub min: Vec3,
    pub max: Vec3,
    pub index: Index,
}

impl Complex {
    pub fn contains(&self, m: Vec3) -> bool {
        m.cmpge(self.min).all() && m.cmple(self.max).all()
    }

    pub fn sample(&self, cfg: &WorldSampleConfig, m: Vec3, height: i32) -> Option<B> {
        if !self.contains(m) {
            return None;
        }
        let v = cfg.voxel_meters();
        if let Some(b) = kit::shaft(self, m, v) {
            return Some(b);
        }
        let (sx, sz, top) = self.shaft;
        if m.y >= top - 1.0 {
            return self.gatehouse.sample(
                Vec3::new(m.x - sx, m.y - top, m.z - sz),
                v,
                (height + 1) as f32 * v - top,
            );
        }
        if m.y > (height + 1) as f32 * v - 2.0 {
            return None;
        }
        let ids = self.index.at(m.x, m.z);
        for &id in ids.iter().filter(|id| *id & CORR_BIT == 0) {
            let r = &self.rooms[id as usize];
            if r.inside(m) {
                return Some(kit::room(self, r, m, v));
            }
        }
        for &id in ids.iter().filter(|id| *id & CORR_BIT != 0) {
            let c = &self.corrs[(id & !CORR_BIT) as usize];
            if c.inside(m) {
                let b = kit::corridor(self, c, m, v);
                // A corridor's roof and floor never open a seam in a seal.
                let sealed = || ids.iter().any(|&id| id & CORR_BIT == 0 && self.rooms[id as usize].sealed && self.rooms[id as usize].in_shell(m));
                return Some(if b.is_solid() && sealed() { B::TombStone } else { b });
            }
        }
        for &id in ids.iter().filter(|id| *id & CORR_BIT == 0) {
            let r = &self.rooms[id as usize];
            if r.in_shell(m) {
                return Some(kit::shell(self, r, m, v));
            }
        }
        for &id in ids.iter().filter(|id| *id & CORR_BIT != 0) {
            if self.corrs[(id & !CORR_BIT) as usize].in_shell(m) {
                return Some(kit::stone(self, m, v));
            }
        }
        None
    }

    /// Name for the HUD at a point, if it is inside this complex.
    pub fn name_at(&self, p: Vec3) -> Option<&'static str> {
        if !(self.contains(p) && p.y < self.ground - 2.0) {
            return None;
        }
        for r in &self.rooms {
            let inside = p.x >= r.x0 && p.x < r.x1 && p.z >= r.z0 && p.z < r.z1;
            if inside && p.y >= r.floor - r.sub() && p.y < r.floor + r.top() {
                return Some(match r.kind {
                    Kind::Labyrinth => "Labyrinth of False Walls",
                    Kind::Pitfall => "Hall of the Hollow Floor",
                    Kind::Orrery => "Hall of Wandering Stars",
                    Kind::Chasm => "The Cleft",
                    Kind::Core => "The sealed vault",
                    _ => self.job.name(),
                });
            }
        }
        Some(self.job.name())
    }

    /// 0..1 inside an orrery room, fading in over the doorway.
    pub fn cosmos_at(&self, p: Vec3) -> f32 {
        let mut best = 0.0f32;
        for r in self.rooms.iter().filter(|r| r.kind == Kind::Orrery) {
            let fy = p.y - r.floor;
            if fy < -0.5 || fy > r.top() {
                continue;
            }
            let inset = (p.x - r.x0).min(r.x1 - p.x).min(p.z - r.z0).min(r.z1 - p.z);
            let t = ((inset + 1.5) / 4.0).clamp(0.0, 1.0);
            best = best.max(t * t * (3.0 - 2.0 * t));
        }
        best
    }
}
