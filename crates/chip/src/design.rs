//! What a player edits: modules placed on the die's site grid, named nets that
//! join their ports, the wires routed for those nets, and the firmware task.
//! Edits keep the floorplan legal (inside the die, no overlap); everything
//! else is checked by `compile`.
use crate::firmware::Firmware;
use crate::registry::{ModuleSpec, Registry};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Site {
    pub x: u8,
    pub y: u8,
    pub layer: u8,
}

impl Site {
    pub fn new(x: u8, y: u8, layer: u8) -> Self {
        Self { x, y, layer }
    }
    /// One step on a layer, or a via between adjacent layers.
    pub fn adjacent(self, other: Self) -> bool {
        let dx = self.x.abs_diff(other.x);
        let dy = self.y.abs_diff(other.y);
        let dl = self.layer.abs_diff(other.layer);
        (dl == 0 && dx + dy == 1) || (dl == 1 && dx == 0 && dy == 0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Part {
    pub id: u16,
    pub module: String,
    pub x: u8,
    pub y: u8,
    /// Quarter turns clockwise.
    pub rot: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Pin {
    pub part: u16,
    pub port: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Net {
    pub id: u16,
    pub name: String,
    pub pins: Vec<Pin>,
    /// Routed paths: each is a chain of adjacent sites. Empty = unrouted.
    #[serde(default)]
    pub paths: Vec<Vec<Site>>,
    /// Drawn by the player: the router never rips it up.
    #[serde(default)]
    pub manual: bool,
}

impl Net {
    pub fn vias(&self) -> u32 {
        self.paths
            .iter()
            .flat_map(|p| p.windows(2))
            .filter(|w| w[0].layer != w[1].layer)
            .count() as u32
    }
    /// Sites of wire on a layer (vias excluded).
    pub fn length(&self) -> u32 {
        self.paths
            .iter()
            .flat_map(|p| p.windows(2))
            .filter(|w| w[0].layer == w[1].layer)
            .count() as u32
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Design {
    pub name: String,
    pub process: String,
    pub width: u8,
    pub height: u8,
    pub layers: u8,
    pub parts: Vec<Part>,
    pub nets: Vec<Net>,
    pub firmware: Firmware,
    next_id: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditError {
    UnknownModule,
    UnknownPart,
    UnknownPort,
    OutsideDie,
    Overlaps(u16),
}

impl std::fmt::Display for EditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownModule => write!(f, "No such module in the verified library"),
            Self::UnknownPart => write!(f, "No such placed module"),
            Self::UnknownPort => write!(f, "That module has no such port"),
            Self::OutsideDie => write!(f, "Footprint leaves the die"),
            Self::Overlaps(id) => write!(f, "Footprint overlaps module #{id}"),
        }
    }
}

/// Footprint size after `rot` quarter turns.
pub fn rotated_size(spec: &ModuleSpec, rot: u8) -> (u8, u8) {
    if rot.is_multiple_of(2) { spec.size } else { (spec.size.1, spec.size.0) }
}

/// Unrotated footprint site -> offset inside the rotated footprint.
pub fn rotate_local(spec: &ModuleSpec, rot: u8, at: (u8, u8)) -> (u8, u8) {
    let (w, h) = spec.size;
    match rot % 4 {
        0 => at,
        1 => (h - 1 - at.1, at.0),
        2 => (w - 1 - at.0, h - 1 - at.1),
        _ => (at.1, w - 1 - at.0),
    }
}

/// Largest die side a design may have.
pub const MAX_SIDE: u8 = 64;

impl Design {
    /// Structural bounds any stored design must meet: die size, layer
    /// count, known modules inside the die, pins on placed parts, wires on
    /// the die. Electrical rules are `compile`'s job.
    pub fn sane(&self, registry: &Registry) -> Result<(), String> {
        let max = registry.process(&self.process).map_or(1, |p| p.max_layers);
        if !(1..=MAX_SIDE).contains(&self.width) || !(1..=MAX_SIDE).contains(&self.height) || !(1..=max).contains(&self.layers) {
            return Err(format!("design {}: die or layers out of range", self.name));
        }
        for p in &self.parts {
            let (_, _, w, h) = self.rect(registry, p).ok_or_else(|| format!("unknown module {}", p.module))?;
            if u16::from(p.x) + u16::from(w) > u16::from(self.width) || u16::from(p.y) + u16::from(h) > u16::from(self.height) {
                return Err(format!("part {} off the die", p.id));
            }
        }
        for n in &self.nets {
            if n.pins.iter().any(|pin| self.part(pin.part).is_none()) {
                return Err(format!("net {} names a missing part", n.id));
            }
            if n.paths.iter().flatten().any(|s| s.x >= self.width || s.y >= self.height || s.layer >= self.layers) {
                return Err(format!("net {} leaves the die", n.id));
            }
        }
        Ok(())
    }

    pub fn new(name: &str, process: &str, width: u8, height: u8, layers: u8) -> Self {
        Self {
            name: name.to_string(),
            process: process.to_string(),
            width,
            height,
            layers: layers.max(1),
            parts: Vec::new(),
            nets: Vec::new(),
            firmware: Firmware::default(),
            next_id: 1,
        }
    }

    pub(crate) fn fresh_id(&mut self) -> u16 {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1).max(1);
        id
    }

    pub fn part(&self, id: u16) -> Option<&Part> {
        self.parts.iter().find(|p| p.id == id)
    }

    /// (x, y, w, h) of a placed part.
    pub fn rect(&self, registry: &Registry, part: &Part) -> Option<(u8, u8, u8, u8)> {
        let spec = registry.module(&part.module)?;
        let (w, h) = rotated_size(spec, part.rot);
        Some((part.x, part.y, w, h))
    }

    /// Die site of a port, on layer 0 where the module's contacts are.
    pub fn pin_site(&self, registry: &Registry, pin: &Pin) -> Option<Site> {
        let part = self.part(pin.part)?;
        let spec = registry.module(&part.module)?;
        let port = spec.port(&pin.port)?;
        let (dx, dy) = rotate_local(spec, part.rot, port.at);
        Some(Site::new(part.x + dx, part.y + dy, 0))
    }

    /// The part whose footprint covers (x, y).
    pub fn part_at(&self, registry: &Registry, x: u8, y: u8) -> Option<&Part> {
        self.parts.iter().find(|p| {
            self.rect(registry, p)
                .is_some_and(|(px, py, w, h)| x >= px && x < px + w && y >= py && y < py + h)
        })
    }
}
