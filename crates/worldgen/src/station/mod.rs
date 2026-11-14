//! Seeded station interiors: an airlock, a corridor spine and furnished rooms.
//!
//! A station is planned in its own metre grid: +x runs from the outer airlock
//! door along the spine, +y is up with the walkable floor at y = 0, and rooms
//! hang off both sides of the spine (+z and -z). Clear sizes follow the station
//! interiors spec: 3 m corridors, 2 by 3 m openings, a 4 by 6 m airlock, crew
//! rooms of at least 4 by 4 m and a freight bay of at least 8 by 12 m.
//! Everything is a pure function of the seed; callers map the grid into their
//! own frame and voxel scale.
use engine_core::hash::hash_u32;
use glam::IVec3;

mod blocks;
pub use blocks::{block, sealed_block};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Airlock,
    Corridor,
    Trade,
    Crew,
    Quarters,
    Freight,
    Galley,
    Engineering,
    Damaged,
}

/// One clear volume; walls, floor and ceiling sit one cell outside it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Space {
    pub kind: Kind,
    /// Inclusive clear bounds.
    pub min: IVec3,
    pub max: IVec3,
    /// +1 for rooms on the +z side, -1 on the -z side, 0 for the spine.
    pub side: i32,
}

impl Space {
    pub fn contains(&self, p: IVec3) -> bool {
        p.cmpge(self.min).all() && p.cmple(self.max).all()
    }

    fn shell(&self, p: IVec3) -> bool {
        p.cmpge(self.min - IVec3::ONE).all() && p.cmple(self.max + IVec3::ONE).all()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Trader,
    Resident,
    Guard,
    Hostile,
}

/// Where an inhabitant of the given role stands, on a floor cell.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Marker {
    pub role: Role,
    pub at: IVec3,
}

#[derive(Clone, Debug)]
pub struct Plan {
    pub seed: u32,
    pub spaces: Vec<Space>,
    /// Boxes cut through walls: doorways, the outer hatch and the hull breach.
    pub openings: Vec<(IVec3, IVec3)>,
    /// The outer airlock hatch, open to whatever the station docks against.
    pub hatch: (IVec3, IVec3),
    /// Hull breach of the damaged section, open to the outside.
    pub breach: (IVec3, IVec3),
    /// Doorway between the spine and the damaged section.
    pub bulkhead: (IVec3, IVec3),
    pub markers: Vec<Marker>,
    /// Inclusive bounds of every station cell, walls included.
    pub min: IVec3,
    pub max: IVec3,
}

impl Plan {
    pub fn space_at(&self, p: IVec3) -> Option<&Space> {
        self.spaces.iter().find(|s| s.contains(p))
    }

    fn opening(&self, p: IVec3) -> bool {
        self.openings.iter().any(|(a, b)| p.cmpge(*a).all() && p.cmple(*b).all())
    }

    /// Whether `p` lies in the doorway that isolates the damaged section.
    pub fn in_bulkhead(&self, p: IVec3) -> bool {
        p.cmpge(self.bulkhead.0).all() && p.cmple(self.bulkhead.1).all()
    }

    /// Whether `p` starts with breathable gas: every room and doorway on the
    /// spine side of the bulkhead.
    pub fn pressurised(&self, p: IVec3) -> bool {
        let breach = p.cmpge(self.breach.0).all() && p.cmple(self.breach.1).all();
        match self.space_at(p) {
            Some(space) => space.kind != Kind::Damaged,
            None => self.opening(p) && !breach && !self.in_bulkhead(p),
        }
    }
}

pub const CORRIDOR_HALF: i32 = 1;
pub const HEIGHT: i32 = 3;

fn roll(seed: u32, salt: u32) -> u32 {
    hash_u32(seed ^ hash_u32(salt.wrapping_mul(0x9e37_79b9) ^ 0x57a7_1011))
}

fn pick(seed: u32, salt: u32, lo: i32, hi: i32) -> i32 {
    lo + (roll(seed, salt) % (hi - lo + 1) as u32) as i32
}

/// Clear (width along the spine, depth away from it, height) of a room kind.
fn size(seed: u32, kind: Kind, k: u32) -> IVec3 {
    let r = |lo, hi, salt| pick(seed, k * 16 + salt, lo, hi);
    match kind {
        Kind::Trade => IVec3::new(r(8, 10, 1), 4, r(6, 7, 2)),
        Kind::Crew => IVec3::new(r(5, 6, 1), HEIGHT, r(5, 6, 2)),
        Kind::Quarters => IVec3::new(r(7, 9, 1), HEIGHT, 5),
        Kind::Freight => IVec3::new(12, 4, r(8, 9, 2)),
        Kind::Galley => IVec3::new(r(5, 6, 1), HEIGHT, 5),
        Kind::Engineering => IVec3::new(r(5, 7, 1), HEIGHT, r(4, 5, 2)),
        Kind::Damaged => IVec3::new(r(6, 8, 1), HEIGHT, r(5, 6, 2)),
        Kind::Airlock | Kind::Corridor => IVec3::new(4, HEIGHT, 4),
    }
}

/// Rooms for one station, split across the two sides of the spine.
fn programme(seed: u32) -> [Vec<Kind>; 2] {
    let mut north = vec![Kind::Trade, Kind::Crew];
    let mut south = vec![Kind::Quarters, Kind::Freight];
    if roll(seed, 3).is_multiple_of(2) {
        north.push(Kind::Galley);
    }
    if !roll(seed, 4).is_multiple_of(3) {
        south.insert(1, Kind::Engineering);
    }
    if roll(seed, 5).is_multiple_of(2) {
        north.swap(0, 1);
    }
    // The damaged section always closes the far end, away from the airlock.
    if roll(seed, 6).is_multiple_of(2) { north.push(Kind::Damaged) } else { south.push(Kind::Damaged) }
    [north, south]
}

/// The station plan for `seed`.
pub fn plan(seed: u32) -> Plan {
    let mut spaces = Vec::new();
    let mut openings = Vec::new();
    let mut markers = Vec::new();
    let airlock = Space { kind: Kind::Airlock, min: IVec3::new(1, 0, -2), max: IVec3::new(6, HEIGHT - 1, 1), side: 0 };
    let hatch = (IVec3::new(0, 0, -1), IVec3::new(0, HEIGHT - 1, 0));
    openings.push(hatch);
    // The inner hatch leads from the airlock into the spine.
    openings.push((IVec3::new(7, 0, -1), IVec3::new(7, HEIGHT - 1, 0)));
    spaces.push(airlock);
    let start = 8;
    let mut far = start;
    let mut breach = (IVec3::ZERO, IVec3::ZERO);
    let mut bulkhead = (IVec3::ZERO, IVec3::ZERO);
    for (s, (side, kinds)) in [1, -1].into_iter().zip(programme(seed)).enumerate() {
        let mut x = start + s as i32 * pick(seed, 9, 0, 2);
        for (k, kind) in kinds.into_iter().enumerate() {
            let dims = size(seed, kind, (s * 8 + k) as u32);
            // The near wall is the corridor wall at z = +-2; rooms start one cell past it.
            let (z0, z1) = if side > 0 { (CORRIDOR_HALF + 2, CORRIDOR_HALF + 1 + dims.z) } else { (-CORRIDOR_HALF - 1 - dims.z, -CORRIDOR_HALF - 2) };
            let room = Space { kind, min: IVec3::new(x, 0, z0), max: IVec3::new(x + dims.x - 1, dims.y - 1, z1), side };
            let door_x = x + dims.x / 2 - 1;
            let wall_z = side * (CORRIDOR_HALF + 1);
            let door = (IVec3::new(door_x, 0, wall_z), IVec3::new(door_x + 1, HEIGHT - 1, wall_z));
            openings.push(door);
            if kind == Kind::Damaged {
                bulkhead = door;
                let outer = if side > 0 { z1 + 1 } else { z0 - 1 };
                let bx = x + dims.x - 3;
                breach = (IVec3::new(bx, 0, outer), IVec3::new(bx + 1, 1, outer));
                openings.push(breach);
            }
            markers.extend(blocks::markers(&room));
            spaces.push(room);
            x += dims.x + 1;
        }
        far = far.max(x - 1);
    }
    let corridor = Space { kind: Kind::Corridor, min: IVec3::new(start, 0, -CORRIDOR_HALF), max: IVec3::new(far - 1, HEIGHT - 1, CORRIDOR_HALF), side: 0 };
    markers.push(Marker { role: Role::Guard, at: IVec3::new(start + 1, 0, 0) });
    spaces.insert(1, corridor);
    let (mut min, mut max) = (IVec3::splat(i32::MAX), IVec3::splat(i32::MIN));
    for s in &spaces {
        min = min.min(s.min - IVec3::ONE);
        max = max.max(s.max + IVec3::ONE);
    }
    Plan { seed, spaces, openings, hatch, breach, bulkhead, markers, min, max }
}

#[cfg(test)]
mod tests;
