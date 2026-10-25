//! Player-designed controllers installed in machines. The chip library
//! (projects, immutable revisions, packaged stock) and each installed part's
//! runtime state live here; `engine_chip` owns design checks and the model.
//!
//! An installed controller holds a pump's intake while its output is off. It
//! reads the tanks on the pump's fluid network, is serviced only when due
//! (a min-queue of wake ticks), and pays its energy from the pump's buffer in
//! whole 250 J units. With an empty buffer it browns out: output off.
use super::{Factory, Kind, Node, power::JOULES_PER_UNIT};
use crate::fast_hash::FastMap;
use engine_chip::design::Design;
use engine_chip::firmware::OPEN;
use engine_chip::library::Library;
use engine_chip::registry::Registry;
use engine_chip::firmware::Task;
use engine_chip::runtime::{MAX_CHANNELS, State};
use glam::IVec3;
use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap};
use std::sync::{Arc, OnceLock};

/// One supply unit in nanojoules.
pub const UNIT_NJ: u64 = (JOULES_PER_UNIT * 1.0e9) as u64;
/// Salvaged circuit carrier consumed per prepared blank, and copper (metal
/// layers) per unit of the revision's bill.
pub const BLANK: (usize, u32) = (12, 64);
pub const COPPER: usize = 6;
pub const COPPER_PER_BILL: u32 = 8;
/// Library limits that keep the saved text far below the decode bound.
pub const MAX_PROJECTS: usize = 64;
pub const MAX_REVISIONS: usize = 256;
pub const MAX_PARTS: usize = 1024;

pub fn registry() -> &'static Registry {
    static REGISTRY: OnceLock<Registry> = OnceLock::new();
    REGISTRY.get_or_init(Registry::builtin)
}

/// Machines with a controller socket: pumps, processors and generators.
pub fn controllable(kind: Kind) -> bool {
    kind == Kind::Pump || kind == Kind::Generator || kind.processor()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Installed {
    pub package: engine_chip::library::Package,
    pub state: State,
    /// Tanks read as channels 0, 1, ...: cached per topology.
    #[serde(skip)]
    sensors: Option<Vec<IVec3>>,
}

pub struct Bay {
    /// Shared with the UI's view snapshot; copied only when it changes.
    pub library: Arc<Library>,
    installed: BTreeMap<Key, Installed>,
    /// World ticks (1/120 s) since the factory started.
    now: u64,
    queue: BinaryHeap<Reverse<(u64, Key)>>,
}

pub(super) fn session() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

impl Default for Bay {
    fn default() -> Self {
        let mut library = Library::default();
        library.session = session();
        Self { library: Arc::new(library), installed: BTreeMap::new(), now: 0, queue: BinaryHeap::new() }
    }
}


#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    NewProject(String, Design),
    /// Replace the draft the player saw at `generation`, then release it.
    Release { project: u32, design: Design, generation: u64 },
    SaveDraft { project: u32, design: Design },
    Prototype(u32),
    Install(IVec3, u32),
    Remove(IVec3),
    Thresholds(IVec3, u16, u16),
    Reset(IVec3),
    /// Start a wafer lot of a revision in a lab ("bench" or "clean").
    StartLot { revision: u32, lab: String },
    /// Discard one stocked part.
    Scrap(u32),
}

/// What the machine panel shows about an installed controller.
#[derive(Clone, Debug, PartialEq)]
pub struct Diagnostics {
    pub package: engine_chip::library::Package,
    pub state: State,
    pub model: Option<engine_chip::characterize::Model>,
    pub sensors: Vec<IVec3>,
}

impl Bay {
    pub fn now(&self) -> u64 {
        self.now
    }

    /// False while a controller at `p` holds its machine off.
    pub fn allows(&self, p: IVec3) -> bool {
        self.installed.get(&key(p)).is_none_or(|c| c.state.output)
    }

    #[cfg(test)]
    pub fn installed_count(&self) -> usize {
        self.installed.len()
    }

    /// One look-at line: part, revision and output state.
    pub fn summary(&self, p: IVec3) -> Option<String> {
        let c = self.installed.get(&key(p))?;
        let rev = self.library.revision(c.package.revision)?;
        let name = self.library.project(rev.project).map_or("?", |p| p.name.as_str());
        let state = match (c.state.phase, c.state.output) {
            (engine_chip::runtime::Phase::Unpowered, _) => "No Power",
            (engine_chip::runtime::Phase::Hung, _) => "Hung",
            (engine_chip::runtime::Phase::Booting, _) => "Booting",
            (_, true) => "On",
            (_, false) => "Off",
        };
        Some(format!("{name} {} · {state}", rev.letter()))
    }

    pub fn diagnostics(&self, p: IVec3) -> Option<Diagnostics> {
        let c = self.installed.get(&key(p))?;
        Some(Diagnostics {
            package: c.package,
            state: c.state.clone(),
            model: self.library.model(c.package.revision).copied(),
            sensors: c.sensors.clone().unwrap_or_default(),
        })
    }

    /// Fluid networks changed: sensors are found again on the next wake.
    pub(super) fn topology_changed(&mut self) {
        for c in self.installed.values_mut() {
            c.sensors = None;
        }
    }

    fn schedule(&mut self, k: Key) {
        if let Some(c) = self.installed.get(&k) {
            self.queue.push(Reverse((c.state.due(), k)));
        }
    }

    /// Install a stocked package in the machine at `p`.
    pub fn install(&mut self, nodes: &FastMap<IVec3, Node>, p: IVec3, serial: u32) -> Result<(), &'static str> {
        let Some(node) = nodes.get(&p).filter(|n| controllable(n.kind)) else {
            return Err("No Controller Socket");
        };
        if self.installed.contains_key(&key(p)) {
            return Err("Socket Occupied");
        }
        let task = self
            .library
            .stock
            .iter()
            .find(|s| s.serial == serial)
            .and_then(|s| self.library.model(s.revision))
            .map(|m| m.task)
            .ok_or("Part Not In Stock")?;
        if !fits(task, node) {
            return Err("Task Mismatch");
        }
        let package = Arc::make_mut(&mut self.library).take(serial).ok_or("Part Not In Stock")?;
        let mut state = State::new(self.now);
        if task == Task::HeatGuard {
            // Below the pile's overheat warning (75%) with room to cool.
            (state.lower, state.upper) = (500, 700);
        }
        self.installed.insert(key(p), Installed { package, state, sensors: None });
        self.schedule(key(p));
        Ok(())
    }

    /// Power-cycle a hung part by hand.
    pub fn reset(&mut self, p: IVec3) {
        let now = self.now;
        if let Some(c) = self.installed.get_mut(&key(p)) {
            c.state.reset(now);
            self.schedule(key(p));
        }
    }

    /// Pull the part out; it returns to stock with no runtime state.
    pub fn remove(&mut self, p: IVec3) -> bool {
        match self.installed.remove(&key(p)) {
            Some(c) => {
                Arc::make_mut(&mut self.library).put_back(c.package);
                true
            }
            None => false,
        }
    }

    pub fn set_thresholds(&mut self, p: IVec3, lower: u16, upper: u16) {
        if let Some(c) = self.installed.get_mut(&key(p)) {
            c.state.lower = lower.min(999);
            c.state.upper = upper.clamp(c.state.lower + 1, 1000);
        }
    }

}

impl Factory {
    /// One fixed world step for every installed controller that is due.
    pub(super) fn step_controllers(&mut self, ticks: u64) {
        let bay = &mut self.chips;
        bay.now += ticks;
        let now = bay.now;
        // Only touch the shared library when a lot step is due.
        if bay.library.lots.iter().any(|l| l.busy_until <= now && l.records.last().is_none_or(|r| r.kind != engine_chip::fab::StepKind::Package)) {
            Arc::make_mut(&mut bay.library).advance_lots(registry(), now);
        }
        let mut orphans = Vec::new();
        while let Some(&Reverse((due, k))) = bay.queue.peek() {
            if due > now {
                break;
            }
            bay.queue.pop();
            let p = IVec3::new(k.0, k.1, k.2);
            let Some(c) = bay.installed.get_mut(&k) else { continue };
            if c.state.due() != due {
                continue; // superseded entry
            }
            let Some(model) = bay.library.model(c.package.revision) else {
                orphans.push(k);
                continue;
            };
            if !self.nodes.get(&p).is_some_and(|n| controllable(n.kind)) {
                orphans.push(k);
                continue;
            }
            if c.sensors.is_none() && !self.topology {
                // Networks are rebuilt on the next factory tick: hold, don't guess.
                c.state.defer(now);
                bay.queue.push(Reverse((c.state.due(), k)));
                continue;
            }
            let sensor = model.task.sensor();
            if c.sensors.is_none() {
                c.sensors = Some(sensors(&self.fluids, &self.items, &self.nodes, p, sensor));
            }
            let mut levels = [OPEN; MAX_CHANNELS];
            for (l, q) in levels.iter_mut().zip(c.sensors.iter().flatten()) {
                *l = self.nodes.get(q).map_or(OPEN, |n| level(sensor, n));
            }
            let Some(node) = self.nodes.get_mut(&p) else { continue };
            // A generator feeds its own controller from its fuel: the burning
            // portion pays, fuel on hand keeps a cold, held generator's
            // controller alive. Anything else needs charge in its buffer.
            let generator = node.kind.generator();
            let powered = node.energy > 0 || (generator && (node.burn > 0 || node.input.iter().any(|&v| v > 0)));
            c.state.service(model, now, powered, |i| levels.get(i).copied().unwrap_or(OPEN));
            let from_buffer = c.state.pay(UNIT_NJ, node.energy);
            node.energy -= from_buffer;
            let from_fuel = if generator { c.state.pay(UNIT_NJ, node.burn) } else { 0 };
            node.burn -= from_fuel;
            self.stats.power(0, from_buffer + from_fuel);
            bay.queue.push(Reverse((c.state.due(), k)));
        }
        for k in orphans {
            if let Some(c) = bay.installed.remove(&k) {
                Arc::make_mut(&mut bay.library).put_back(c.package);
            }
        }
    }
}

mod actions;
mod save;
mod sensors;
pub use sensors::fits;
use sensors::{level, sensors};

type Key = (i32, i32, i32);
fn key(p: IVec3) -> Key {
    (p.x, p.y, p.z)
}
pub use actions::{chip_action, resources, review_fixture};

#[cfg(test)]
#[path = "chips_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "chips_scale_tests.rs"]
mod scale_tests;
#[cfg(test)]
#[path = "chips_task_tests.rs"]
mod task_tests;
