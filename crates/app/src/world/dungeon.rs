//! Generator 16 underground complexes. A complex is a graph of rooms joined by
//! corridors and ramps, cut into solid ground below a spiral shaft and a roofed
//! gatehouse. Complexes are cached per 144 m cell, so a voxel costs a bounding
//! box test (plus a few rectangle tests inside one). The Holy Mountain vault
//! tiers reuse the same machinery with hand-authored layouts.
use super::architecture::{Building, Door, Palette, Roof, Windows, Wing, WingSpec};
use super::{BlockId as B, WorldSampleConfig, WorldType, hash2, sample_column};
use glam::Vec3;
use std::cell::RefCell;
use crate::fast_hash::FastMap;
use std::rc::Rc;

mod bearer_hall;
mod layout;

pub const CELL: f32 = 144.0;
const SLOT: f32 = 28.0;
const LEVEL_STEP: f32 = 8.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Hub,
    Hall,
    Crypt,
    Cistern,
    Grotto,
    Treasure,
    Shrine,
    Barracks,
    Forge,
    Library,
    Ore,
    /// Noita item stands: a row of pedestals, one offering each.
    Pedestal,
    /// Aether antechamber: dais, light shaft, and a bronze ring.
    Atrium,
    /// Generator 21: a starlit chamber whose fog is the cosmos. A trail of
    /// crystal in the floor points at a false wall hiding a cache.
    Cosmos,
    /// Generator 21: a maze whose walls lie. Half of them are illusions.
    Labyrinth,
    /// Generator 21: a cross of real floor over pits. The rest is mirage.
    Pitfall,
    /// Generator 28: a processional bridge over connected lower service works.
    WeighingHall,
}

#[derive(Clone, Copy)]
struct Theme {
    wall: B,
    alt: B,
    floor: B,
    name: &'static str,
    kinds: [Kind; 6],
}

const THEMES: [Theme; 4] = [
    Theme {
        wall: B::StoneBricks,
        alt: B::Cobblestone,
        floor: B::Cobblestone,
        name: "Forgotten catacombs",
        kinds: [
            Kind::Hall,
            Kind::Crypt,
            Kind::Crypt,
            Kind::Barracks,
            Kind::Shrine,
            Kind::Library,
        ],
    },
    Theme {
        wall: B::Limestone,
        alt: B::StoneBricks,
        floor: B::Limestone,
        name: "Sunken undercroft",
        kinds: [
            Kind::Cistern,
            Kind::Cistern,
            Kind::Hall,
            Kind::Shrine,
            Kind::Library,
            Kind::Grotto,
        ],
    },
    Theme {
        wall: B::Limestone,
        alt: B::Cobblestone,
        floor: B::FungalSoil,
        name: "Mycelial warren",
        kinds: [
            Kind::Grotto,
            Kind::Grotto,
            Kind::Hall,
            Kind::Cistern,
            Kind::Shrine,
            Kind::Crypt,
        ],
    },
    Theme {
        wall: B::Basalt,
        alt: B::StoneBricks,
        floor: B::Basalt,
        name: "Cinder forge-halls",
        kinds: [
            Kind::Forge,
            Kind::Forge,
            Kind::Hall,
            Kind::Barracks,
            Kind::Library,
            Kind::Ore,
        ],
    },
];

#[derive(Clone, Debug)]
struct Room {
    x0: f32,
    x1: f32,
    z0: f32,
    z1: f32,
    floor: f32,
    h: f32,
    kind: Kind,
    vault: bool,
}

/// Depth of the hidden pit under a Pitfall room's false floor.
const PIT_DEPTH: f32 = 3.0;
/// Height of the false-wall annex off a cosmos chamber.
const ANNEX_H: f32 = 3.0;
const ANNEX_DEPTH: f32 = 2.6;
const ANNEX_HALF_W: f32 = 1.5;

/// A recess behind a mirage plug in one wall of a cosmos chamber. Side 0/1 are
/// the -x/+x walls, 2/3 the -z/+z walls; `along` shifts it off the doorway.
#[derive(Clone, Copy)]
struct Annex {
    side: u8,
    along: f32,
}

impl Room {
    fn centre(&self) -> (f32, f32) {
        ((self.x0 + self.x1) * 0.5, (self.z0 + self.z1) * 0.5)
    }

    fn pit_depth(&self) -> f32 {
        match self.kind {
            Kind::Pitfall => PIT_DEPTH,
            Kind::WeighingHall => bearer_hall::DEPTH,
            _ => 0.0,
        }
    }

    /// Annex placement is fixed by the room's position, so it needs no storage.
    fn annex(&self, seed: u32) -> Annex {
        let h = |k: u32| hash2(self.x0 as i32, self.z0 as i32, seed.wrapping_add(k));
        let side = (h(3101) * 4.0) as u8 % 4;
        let span = if side < 2 {
            self.z1 - self.z0
        } else {
            self.x1 - self.x0
        };
        // Clear of the three-metre doorway at the middle of the wall.
        let reach = (span * 0.5 - ANNEX_HALF_W - 0.4).max(4.8);
        let along =
            (4.8 + h(3102) * (reach - 4.8).max(0.0)) * if h(3103) > 0.5 { 1.0 } else { -1.0 };
        Annex { side, along }
    }

    /// (distance beyond the wall plane, position along the wall) in metres for a
    /// point given in room-local coordinates, for the chosen annex side.
    fn annex_frame(&self, a: Annex, rx: f32, rz: f32) -> (f32, f32) {
        let (w, d) = (self.x1 - self.x0, self.z1 - self.z0);
        match a.side {
            0 => (-rx, rz - d * 0.5 - a.along),
            1 => (rx - w, rz - d * 0.5 - a.along),
            2 => (-rz, rx - w * 0.5 - a.along),
            _ => (rz - d, rx - w * 0.5 - a.along),
        }
    }

    fn height_at(&self, x: f32) -> f32 {
        if self.vault {
            let half = (self.x1 - self.x0) * 0.5;
            self.h - 2.0 * ((x - self.centre().0) / half).powi(2)
        } else {
            self.h
        }
    }
}

#[derive(Clone, Debug)]
struct Corr {
    x0: f32,
    x1: f32,
    z0: f32,
    z1: f32,
    along_x: bool,
    a0: f32,
    a1: f32,
    fa: f32,
    fb: f32,
}

impl Corr {
    fn floor_at(&self, x: f32, z: f32) -> f32 {
        let c = if self.along_x { x } else { z };
        let span = self.a1 - self.a0;
        let t = if span.abs() < 1e-3 {
            0.0
        } else {
            ((c - self.a0) / span).clamp(0.0, 1.0)
        };
        self.fa + (self.fb - self.fa) * t
    }
}

pub struct Dungeon {
    theme: Theme,
    seed: u32,
    rooms: Vec<Room>,
    corrs: Vec<Corr>,
    /// Shaft centre (x, z) and the surface height it climbs to.
    shaft: Option<(f32, f32, f32)>,
    gatehouse: Option<Building>,
    ground: f32,
    min: Vec3,
    max: Vec3,
}

const CORR_W: f32 = 3.0;
const CORR_H: f32 = 3.6;

/// Z-shaped corridor between the facing edges of two rooms. Returns its length.
fn link(rooms: &[Room], a: usize, b: usize, out: Option<&mut Vec<Corr>>) -> f32 {
    let (ra, rb) = (&rooms[a], &rooms[b]);
    let (acx, acz) = ra.centre();
    let (bcx, bcz) = rb.centre();
    let horizontal = (bcx - acx).abs() >= (bcz - acz).abs();
    let (lo, hi) = if horizontal {
        if bcx >= acx { (ra, rb) } else { (rb, ra) }
    } else if bcz >= acz {
        (ra, rb)
    } else {
        (rb, ra)
    };
    let (ls, le) = if horizontal {
        (lo.x1, hi.x0)
    } else {
        (lo.z1, hi.z0)
    };
    let (ts, te) = if horizontal {
        (lo.centre().1, hi.centre().1)
    } else {
        (lo.centre().0, hi.centre().0)
    };
    let mid = (ls + le) * 0.5;
    let (l1, l2, l3) = ((mid - ls).max(0.0), (te - ts).abs(), (le - mid).max(0.0));
    let tot = l1 + l2 + l3;
    let Some(out) = out else {
        return tot;
    };
    let (fs, fe) = (lo.floor, hi.floor);
    let fl = |s: f32| {
        if tot < 1e-3 {
            fs
        } else {
            fs + (fe - fs) * s / tot
        }
    };
    let w = CORR_W * 0.5;
    if horizontal {
        out.push(Corr {
            x0: ls - 0.6,
            x1: mid + w,
            z0: ts - w,
            z1: ts + w,
            along_x: true,
            a0: ls,
            a1: mid,
            fa: fl(0.0),
            fb: fl(l1),
        });
        if l2 > 0.01 {
            let (zlo, zhi) = (ts.min(te), ts.max(te));
            let (fa, fb) = if ts <= te {
                (fl(l1), fl(l1 + l2))
            } else {
                (fl(l1 + l2), fl(l1))
            };
            out.push(Corr {
                x0: mid - w,
                x1: mid + w,
                z0: zlo - w,
                z1: zhi + w,
                along_x: false,
                a0: zlo,
                a1: zhi,
                fa,
                fb,
            });
        }
        out.push(Corr {
            x0: mid - w,
            x1: le + 0.6,
            z0: te - w,
            z1: te + w,
            along_x: true,
            a0: mid,
            a1: le,
            fa: fl(l1 + l2),
            fb: fl(tot),
        });
    } else {
        out.push(Corr {
            x0: ts - w,
            x1: ts + w,
            z0: ls - 0.6,
            z1: mid + w,
            along_x: false,
            a0: ls,
            a1: mid,
            fa: fl(0.0),
            fb: fl(l1),
        });
        if l2 > 0.01 {
            let (xlo, xhi) = (ts.min(te), ts.max(te));
            let (fa, fb) = if ts <= te {
                (fl(l1), fl(l1 + l2))
            } else {
                (fl(l1 + l2), fl(l1))
            };
            out.push(Corr {
                x0: xlo - w,
                x1: xhi + w,
                z0: mid - w,
                z1: mid + w,
                along_x: true,
                a0: xlo,
                a1: xhi,
                fa,
                fb,
            });
        }
        out.push(Corr {
            x0: te - w,
            x1: te + w,
            z0: mid - w,
            z1: le + 0.6,
            along_x: false,
            a0: mid,
            a1: le,
            fa: fl(l1 + l2),
            fb: fl(tot),
        });
    }
    tot
}

fn find(parent: &mut [usize], mut i: usize) -> usize {
    while parent[i] != i {
        parent[i] = parent[parent[i]];
        i = parent[i];
    }
    i
}

/// Noita item stand. The base is wide enough to catch a 1 m voxel centre.
fn item_pedestal(rx: f32, rz: f32, fy: f32, v: f32, px: f32, pz: f32, item: B) -> Option<B> {
    let pr = (rx - px).hypot(rz - pz);
    if pr < 1.15 && fy >= v && fy < v * 2.0 {
        return Some(B::StoneBricks);
    }
    if pr < 0.85 && fy >= v * 2.0 && fy < v * 3.0 {
        return Some(item);
    }
    None
}

/// Three offerings in a row, with a crystal shaft over the middle one.
fn pedestal_row(rx: f32, rz: f32, fy: f32, v: f32, w: f32, d: f32, h: f32) -> Option<B> {
    let mx = w * 0.5;
    let z = d * 0.62;
    item_pedestal(rx, rz, fy, v, mx - 3.4, z, B::Crystal)
        .or_else(|| item_pedestal(rx, rz, fy, v, mx, z, B::GoldBlock))
        .or_else(|| item_pedestal(rx, rz, fy, v, mx + 3.4, z, B::SurveyCache))
        .or_else(|| {
            // Braziers flanking the row.
            let brazier = |px: f32| {
                let pr = (rx - px).hypot(rz - (z - 2.4));
                if pr < 0.9 && fy >= v && fy < v * 2.0 {
                    Some(B::Cobblestone)
                } else if pr < 0.75 && fy >= v * 2.0 && fy < v * 3.0 {
                    Some(B::Torch)
                } else {
                    None
                }
            };
            brazier(mx - 3.4).or_else(|| brazier(mx + 3.4))
        })
        .or_else(|| {
            if (rx - mx).abs() < 0.75 && (rz - z).abs() < 0.75 && fy > h - 2.8 && fy < h - 0.4 {
                Some(B::Crystal)
            } else {
                None
            }
        })
}

/// Aether slider antechamber: a bronze ring, a raised dais, and a light shaft.
fn atrium(rx: f32, rz: f32, fy: f32, v: f32, w: f32, d: f32, h: f32) -> Option<B> {
    let (mx, mz) = (w * 0.5, d * 0.5);
    let dist = (rx - mx).hypot(rz - mz);
    let inset = 2.3;
    let pillar = [
        (inset, inset),
        (w - inset, inset),
        (inset, d - inset),
        (w - inset, d - inset),
    ]
    .into_iter()
    .any(|(px, pz)| (rx - px).hypot(rz - pz) < 0.95);
    if pillar && fy < h - 1.2 {
        if fy > h - 2.3 {
            return Some(B::Crystal);
        }
        return Some(B::Limestone);
    }
    // Aerclouds tucked under the vault, clear of the dais.
    if fy > h - 1.4 && dist > 4.2 && (rx - mx).abs() > 2.0 && (rz - mz).abs() > 2.0 {
        return Some(B::Ice);
    }
    if dist < 2.5 && fy < v {
        return Some(B::StoneBricks);
    }
    if (2.5..3.6).contains(&dist) && fy < v {
        return Some(B::CopperBlock);
    }
    if let Some(item) = item_pedestal(rx, rz, fy, v, mx, mz, B::MithrilBlock) {
        return Some(item);
    }
    // Terraria chandelier / Aether light shaft above the offering.
    if dist < 0.8 && fy > h - 2.6 && fy < h - 0.4 {
        return Some(B::Crystal);
    }
    // 5e side altar: a locked chest off the processional axis.
    if (rx - mx).abs() < 0.8 && (rz - (d - 2.6)).abs() < 0.8 && fy >= v && fy < v * 2.0 {
        return Some(B::GoldBlock);
    }
    if (rx - mx).abs() < 0.6 && (rz - (d - 2.6)).abs() < 0.6 && fy >= v * 2.0 && fy < v * 3.0 {
        return Some(B::SurveyCache);
    }
    None
}

/// Generator 19: each large room hides something in a different corner, off the
/// corridor cross. The prize corner has a short screen so you walk around it.
fn room_interest(seed: u32, r: &Room, rx: f32, rz: f32, fy: f32, v: f32) -> Option<B> {
    if matches!(
        r.kind,
        Kind::Hub
            | Kind::Pedestal
            | Kind::Atrium
            | Kind::Cistern
            | Kind::Treasure
            | Kind::Ore
            | Kind::Cosmos
            | Kind::Labyrinth
            | Kind::Pitfall
    ) {
        return None;
    }
    let (w, d) = (r.x1 - r.x0, r.z1 - r.z0);
    if w < 11.0 || d < 11.0 {
        return None;
    }
    let (mx, mz) = (w * 0.5, d * 0.5);
    // Corridors meet the middle of each wall. Leave that cross walkable.
    if (rx - mx).abs() < 2.15 || (rz - mz).abs() < 2.15 {
        return None;
    }
    let q = match (rx < mx, rz < mz) {
        (true, true) => 0u32,
        (false, true) => 1,
        (true, false) => 2,
        (false, false) => 3,
    };
    // The shrine's altar owns the far wall.
    if r.kind == Kind::Shrine && q >= 2 {
        return None;
    }
    let inset = 2.5;
    let (cx, cz) = match q {
        0 => (inset, inset),
        1 => (w - inset, inset),
        2 => (inset, d - inset),
        _ => (w - inset, d - inset),
    };
    let pr = (rx - cx).hypot(rz - cz);
    let plan = (hash2(r.x0 as i32, r.z0 as i32, seed.wrapping_add(901)) * 4.0) as u32 % 4;
    let prize = (hash2(r.x0 as i32, r.z0 as i32, seed.wrapping_add(907)) * 4.0) as u32 % 4;
    if q == prize {
        if let Some(stand) = item_pedestal(rx, rz, fy, v, cx, cz, B::SurveyCache) {
            return Some(stand);
        }
        // A shoulder-high screen on the room side of the cache. One flank stays open.
        let face = if q & 2 == 0 { cz + 1.15 } else { cz - 1.15 };
        if (rz - face).abs() < 0.45 && (rx - cx).abs() < 1.8 && fy >= v && fy < v * 2.6 {
            return Some(B::StoneBricks);
        }
        if pr < 0.8 && fy > 3.4 && fy < 5.0 && r.h > 6.0 {
            return Some(B::Crystal);
        }
        return None;
    }
    if pr > 1.7 {
        return None;
    }
    match plan {
        0 => {
            if pr < 0.95 && fy < r.h - 1.4 {
                return Some(if fy > r.h - 2.4 {
                    B::Crystal
                } else {
                    B::Limestone
                });
            }
            if pr < 1.5 && pr > 1.05 && fy >= v && fy < v * 2.0 {
                return Some(B::Torch);
            }
            None
        }
        1 => {
            if pr < 1.15 && fy < v {
                return Some(B::FungalSoil);
            }
            if pr < 0.8 && fy >= v && fy < v * 2.0 {
                return Some(B::GlowCap);
            }
            None
        }
        2 => {
            if pr < 1.05 && fy < v {
                return Some(B::Water);
            }
            if (1.05..1.6).contains(&pr) && fy < v {
                return Some(B::Cobblestone);
            }
            None
        }
        _ => {
            if pr < 1.35 && fy < v * 1.6 && ((rx + rz) * 2.0).floor() as i32 % 2 == 0 {
                return Some(B::Cobblestone);
            }
            None
        }
    }
}

/// Minecraft dungeon cage: four posts, a crystal spawner, two chests.
fn spawner_cage(rx: f32, rz: f32, fy: f32, v: f32, mx: f32, mz: f32, w: f32) -> Option<B> {
    let post =
        |ox: f32, oz: f32| (rx - mx - ox).abs() < 0.6 && (rz - mz - oz).abs() < 0.6 && fy < 2.6;
    if post(-1.7, -1.7) || post(1.7, -1.7) || post(-1.7, 1.7) || post(1.7, 1.7) {
        return Some(B::Cobblestone);
    }
    if (rx - mx).abs() < 0.6 && (rz - mz).abs() < 0.6 && fy >= v && fy < v * 3.0 {
        return Some(B::Crystal);
    }
    if (rx - 2.2).abs() < 0.6 && (rz - mz).abs() < 0.6 && fy >= v && fy < v * 2.0 {
        return Some(B::SurveyCache);
    }
    if (rx - (w - 2.2)).abs() < 0.6 && (rz - mz).abs() < 0.6 && fy >= v && fy < v * 2.0 {
        return Some(B::CultivatorCache);
    }
    None
}

// ---------------------------------------------------------------------------
// Generator 21 puzzles: pit under a false floor, a lying maze, a cosmos chamber

/// Everything below a Pitfall room's floor. The room is a cross of real floor
/// (it joins all four doorways) over four pits; each pit is roofed by a one
/// metre layer of mirage that looks exactly like the aisle. Three pits hold
/// water, one holds a cache, and every pit has a hidden stair back up.
fn pit_block(seed: u32, r: &Room, rx: f32, rz: f32, fy: f32, wt: f32) -> B {
    let (w, d) = (r.x1 - r.x0, r.z1 - r.z0);
    let (mx, mz) = (w * 0.5, d * 0.5);
    let y = fy + PIT_DEPTH; // height above the pit bottom
    if y < 0.0 {
        return B::StoneBricks;
    }
    let aisle = 1.7;
    if (rx - mx).abs() < aisle || (rz - mz).abs() < aisle {
        // The beam of real floor runs the full depth so the aisle is solid.
        return B::StoneBricks;
    }
    // Quadrant bookkeeping: which pit, and where its middle is.
    let east = rx > mx;
    let south = rz > mz;
    let q = east as u32 + 2 * south as u32;
    let prize = (hash2(r.x0 as i32, r.z0 as i32, seed.wrapping_add(3301)) * 4.0) as u32 % 4;
    let qcx = if east {
        (mx + aisle + w) * 0.5
    } else {
        (mx - aisle) * 0.5
    };
    let qcz = if south {
        (mz + aisle + d) * 0.5
    } else {
        (mz - aisle) * 0.5
    };
    // The false floor itself.
    if fy >= -wt {
        return B::MirageBrick;
    }
    // Stair: six half-metre risers climbing toward the aisle along z, under the mirage.
    let s = (rz - mz).abs() - aisle;
    if (rx - qcx).abs() < 1.2 && s < 3.2 {
        let riser = 3 - (s / 0.8) as i32; // 3 beside the aisle .. 0 at the foot
        let top = 0.5 * (riser + 1) as f32 * 1.0;
        if y < top {
            return B::StoneBricks;
        }
    }
    if q == prize {
        // A dry cellar with a dais at its middle.
        let (dx, dz) = (
            (rx - qcx).abs(),
            (rz - qcz + 1.6 * if south { 1.0 } else { -1.0 }).abs(),
        );
        if dx < 0.9 && dz < 0.9 && y < 0.8 {
            return B::StoneBricks;
        }
        if dx < 0.5 && dz < 0.5 && (0.8..1.8).contains(&y) {
            return B::SurveyCache;
        }
        if dx > 1.4 && dz < 0.5 && y < 1.0 && (rx - qcx).abs() > 2.6 {
            return B::Crystal;
        }
        return B::Air;
    }
    if y < 1.2 { B::Water } else { B::Air }
}

/// Binary-tree maze. Every wall segment between two cells is either solid
/// brick or a mirage, and only a hash of the segment says which. Returns the
/// block at a room-local point, or None for open air.
fn labyrinth(seed: u32, r: &Room, rx: f32, rz: f32, fy: f32, v: f32) -> Option<B> {
    const PITCH: f32 = 3.0;
    const WALL_H: f32 = 2.7;
    let (w, d) = (r.x1 - r.x0, r.z1 - r.z0);
    let nx = (((w - 4.8) / PITCH) as i32).max(3);
    let nz = (((d - 4.8) / PITCH) as i32).max(3);
    let (ox, oz) = ((w - nx as f32 * PITCH) * 0.5, (d - nz as f32 * PITCH) * 0.5);
    let (u, s) = (rx - ox, rz - oz);
    let t = v.max(0.8);
    let (len_u, len_s) = (nx as f32 * PITCH, nz as f32 * PITCH);
    if u < -t * 0.5 || s < -t * 0.5 || u > len_u + t * 0.5 || s > len_s + t * 0.5 {
        return None;
    }
    let key = |a: i32, b: i32, k: u32| {
        hash2(
            a.wrapping_add(r.x0 as i32),
            b + r.z0 as i32,
            seed.wrapping_add(k),
        )
    };
    // Goal chest at the middle cell.
    let (gi, gj) = (nx / 2, nz / 2);
    let (gcx, gcz) = ((gi as f32 + 0.5) * PITCH, (gj as f32 + 0.5) * PITCH);
    // No dais: the cell is only two metres wide, and the tree must stay walkable.
    if (u - gcx).abs() < 0.45 && (s - gcz).abs() < 0.45 && fy < v.max(0.8) {
        return Some(B::SurveyCache);
    }
    if fy >= WALL_H {
        // Torches on a few posts give the grid a pulse of warm light.
        return None;
    }
    let (k, m) = ((u / PITCH).round() as i32, (s / PITCH).round() as i32);
    let (du, ds) = ((u - k as f32 * PITCH).abs(), (s - m as f32 * PITCH).abs());
    let on_vline = du < t * 0.5;
    let on_hline = ds < t * 0.5;
    if !on_vline && !on_hline {
        return None;
    }
    // Which cell does a coordinate fall in? Cells are clamped inside the grid.
    let ci = ((u / PITCH).floor() as i32).clamp(0, nx - 1);
    let cj = ((s / PITCH).floor() as i32).clamp(0, nz - 1);
    // Binary tree: each cell opens north or east. The top row can only open
    // east and the east column only north, so every cell reaches the corner.
    let opens_north = |i: i32, j: i32| -> bool {
        if j == nz - 1 {
            false
        } else if i == nx - 1 {
            true
        } else {
            key(i, j, 3401) < 0.5
        }
    };
    let opens_east =
        |i: i32, j: i32| -> bool { !(opens_north(i, j) || i == nx - 1 && j == nz - 1) };
    // The way in: one gap in the south boundary wall.
    let entry = (key(0, 0, 3402) * nx as f32) as i32 % nx;
    let solid = if on_vline && on_hline {
        true // posts are always real
    } else if on_vline {
        // Wall between cells (k-1, cj) and (k, cj).
        if k <= 0 || k >= nx {
            true
        } else {
            !opens_east(k - 1, cj)
        }
    } else if m <= 0 {
        // South boundary, with the entrance cut into it.
        !(m == 0 && ci == entry)
    } else if m >= nz {
        true
    } else {
        !opens_north(ci, m - 1)
    };
    if !solid {
        return None;
    }
    if on_vline && on_hline {
        if fy >= v.max(0.8) * 2.0 - 0.01 && fy < v.max(0.8) * 3.0 && key(k, m, 3403) < 0.28 {
            return Some(B::Torch);
        }
        if fy < WALL_H {
            return Some(B::StoneBricks);
        }
        return None;
    }
    let orient = if on_vline { 3410 } else { 3420 };
    let seg = if on_vline { (k, cj) } else { (ci, m) };
    // The outer wall is always real; only interior walls may be illusions.
    let boundary = if on_vline {
        k <= 0 || k >= nx
    } else {
        m <= 0 || m >= nz
    };
    if !boundary && key(seg.0, seg.1, orient) < 0.5 {
        Some(B::MirageBrick)
    } else {
        Some(B::StoneBricks)
    }
}

/// Big Dipper in room-local unit coordinates (-1..1), the handle trailing off.
const DIPPER: [(f32, f32); 7] = [
    (0.12, 0.10),
    (0.30, 0.14),
    (0.34, -0.12),
    (0.14, -0.15),
    (-0.08, -0.22),
    (-0.30, -0.12),
    (-0.52, -0.26),
];

impl Dungeon {
    /// Star-glass floor with a trail of crystal laid from the middle of the room
    /// to the false wall. The trail is the clue; the plug is the answer.
    fn cosmos_floor(&self, r: &Room, rx: f32, rz: f32) -> B {
        let (w, d) = (r.x1 - r.x0, r.z1 - r.z0);
        let (cx, cz) = (w * 0.5, d * 0.5);
        let a = r.annex(self.seed);
        let (tx, tz) = match a.side {
            0 => (0.0, cz + a.along),
            1 => (w, cz + a.along),
            2 => (cx + a.along, 0.0),
            _ => (cx + a.along, d),
        };
        let (dx, dz) = (tx - cx, tz - cz);
        let len2 = dx * dx + dz * dz;
        let t = (((rx - cx) * dx + (rz - cz) * dz) / len2.max(1e-3)).clamp(0.0, 1.0);
        let (px, pz) = (cx + dx * t, cz + dz * t);
        let off = (rx - px).hypot(rz - pz);
        // Dashes, so it reads as stars rather than a road.
        let dash = (t * len2.sqrt() / 1.6).fract() < 0.55;
        if off < 0.34 && t > 0.12 && dash {
            B::Crystal
        } else {
            B::StarGlass
        }
    }

    /// The recess behind a cosmos chamber's false wall. None when the point is
    /// outside the annex and its one-block shell.
    fn annex_block(&self, r: &Room, m: Vec3, v: f32, wt: f32) -> Option<B> {
        let a = r.annex(self.seed);
        let (rx, rz) = (m.x - r.x0, m.z - r.z0);
        let fy = m.y - r.floor;
        let (out, t) = r.annex_frame(a, rx, rz);
        let reach = wt + ANNEX_DEPTH;
        let carve = out >= -0.01 && out < reach && t.abs() < ANNEX_HALF_W;
        if carve && (0.0..ANNEX_H).contains(&fy) {
            if out < wt {
                return Some(B::MirageVoid);
            }
            let (oc, hw) = (wt + ANNEX_DEPTH * 0.55, 0.6);
            let unit = v.max(1.0);
            if (out - oc).abs() < hw + 0.3 && t.abs() < hw + 0.3 && fy < unit {
                return Some(B::StarGlass);
            }
            if (out - oc).abs() < 0.5 && t.abs() < 0.5 && fy >= unit && fy < unit * 2.0 {
                return Some(B::SurveyCache);
            }
            if (out - (wt + 0.5)).abs() < 0.4 && (t.abs() - 1.1).abs() < 0.35 && fy < unit {
                return Some(B::Torch);
            }
            return Some(B::Air);
        }
        // One shell block of void around the recess, including its floor and roof.
        let ring_out = out >= 0.0 && out < reach + wt;
        if ring_out && t.abs() < ANNEX_HALF_W + wt && fy >= -wt && fy < ANNEX_H + wt {
            if fy < 0.0 && carve {
                return Some(B::StarGlass);
            }
            return Some(B::VoidStone);
        }
        None
    }

    fn bounds(
        rooms: &[Room],
        corrs: &[Corr],
        shaft: Option<(f32, f32, f32)>,
        ground: f32,
    ) -> (Vec3, Vec3) {
        let mut min = Vec3::splat(f32::MAX);
        let mut max = Vec3::splat(f32::MIN);
        for r in rooms {
            // A cosmos chamber's annex digs up to five metres past its wall.
            let pad = if r.kind == Kind::Cosmos { 6.0 } else { 2.0 };
            min = min.min(Vec3::new(
                r.x0 - pad,
                r.floor - 3.0 - r.pit_depth(),
                r.z0 - pad,
            ));
            max = max.max(Vec3::new(r.x1 + pad, r.floor + r.h + 3.0, r.z1 + pad));
        }
        for c in corrs {
            let lo = c.fa.min(c.fb);
            let hi = c.fa.max(c.fb);
            min = min.min(Vec3::new(c.x0 - 2.0, lo - 3.0, c.z0 - 2.0));
            max = max.max(Vec3::new(c.x1 + 2.0, hi + CORR_H + 3.0, c.z1 + 2.0));
        }
        if let Some((sx, sz, top)) = shaft {
            min = min.min(Vec3::new(sx - 6.0, min.y, sz - 6.0));
            max = max.max(Vec3::new(sx + 6.0, top + 14.0, sz + 6.0));
        }
        let _ = ground;
        (min, max)
    }

    /// A randomly planned complex centred on (cx, cz) below `ground`.
    fn generate(seed: u32, cx: f32, cz: f32, ground: f32, version: u32) -> Dungeon {
        let theme = THEMES[(hash2(1, 1, seed) * 4.0) as usize % 4];
        let base = ground - 26.0;
        let (ox, oz) = (cx - SLOT * 1.5, cz - SLOT * 1.5);
        let mut rooms: Vec<Room> = Vec::new();
        for j in 0..3usize {
            for i in 0..3usize {
                let r = |k: i32| hash2(i as i32 * 7 + k, j as i32 * 13 + 3, seed);
                let hub = i == 1 && j == 1;
                let mut kind = if hub {
                    Kind::Hub
                } else {
                    theme.kinds[(r(9) * 6.0) as usize % 6]
                };
                // Generator 17: fewer flooded cisterns. Generator 18 turns some
                // halls into pedestal rows and Aether antechambers.
                if version >= 17 && !hub && kind == Kind::Cistern && r(12) > 0.25 {
                    kind = Kind::Hall;
                }
                if version >= 18
                    && !hub
                    && matches!(kind, Kind::Hall | Kind::Shrine)
                    && r(14) > 0.40
                {
                    kind = if r(15) > 0.5 {
                        Kind::Atrium
                    } else {
                        Kind::Pedestal
                    };
                }
                // Generator 21 turns some rooms into illusions and puzzles. Own hash
                // keys, so the older layout rolls above are untouched.
                if version >= 21 && !hub {
                    let roll = hash2(
                        i as i32 * 31 + 7,
                        j as i32 * 17 + 5,
                        seed.wrapping_add(2101),
                    );
                    kind = if roll < 0.15 {
                        Kind::Cosmos
                    } else if roll < 0.27 {
                        Kind::Labyrinth
                    } else if roll < 0.39 {
                        Kind::Pitfall
                    } else {
                        kind
                    };
                }
                let puzzle = matches!(kind, Kind::Cosmos | Kind::Labyrinth | Kind::Pitfall);
                let (mut w, mut d) = if hub {
                    (14.0, 14.0)
                } else {
                    (8.0 + r(1) * 14.0, 8.0 + r(2) * 14.0)
                };
                if !hub && r(3) < 0.2 && !matches!(kind, Kind::Pedestal | Kind::Atrium) && !puzzle {
                    w = 6.0 + r(5) * 3.0;
                    d = 6.0 + r(6) * 3.0;
                }
                if matches!(kind, Kind::Pedestal | Kind::Atrium) {
                    w = w.max(13.0);
                    d = d.max(13.0);
                }
                if puzzle {
                    // Mazes need room for at least a 3x3 grid and the walkway
                    // ring; the others need space for the aisle cross and steps.
                    let floor = if kind == Kind::Labyrinth { 17.0 } else { 15.0 };
                    w = w.max(floor);
                    d = d.max(floor);
                }
                let slack = |s: f32| ((SLOT - s - 5.0) * 0.5).max(0.0);
                let scx = ox
                    + (i as f32 + 0.5) * SLOT
                    + if hub {
                        0.0
                    } else {
                        (r(4) - 0.5) * 2.0 * slack(w)
                    };
                let scz = oz
                    + (j as f32 + 0.5) * SLOT
                    + if hub {
                        0.0
                    } else {
                        (r(7) - 0.5) * 2.0 * slack(d)
                    };
                let mut h = if hub { 7.5 } else { 4.5 + r(8) * 3.0 };
                if version >= 17
                    && !hub
                    && matches!(
                        kind,
                        Kind::Hall | Kind::Shrine | Kind::Library | Kind::Pedestal | Kind::Atrium
                    )
                    && r(13) > 0.5
                {
                    h += 5.0;
                }
                if matches!(kind, Kind::Pedestal | Kind::Atrium) {
                    h = h.max(8.5);
                }
                if version >= 17 && hub {
                    h = 10.0;
                }
                if kind == Kind::Cosmos {
                    h = h.max(9.0);
                } else if kind == Kind::Pitfall {
                    h = h.clamp(5.5, 7.0);
                } else if kind == Kind::Labyrinth {
                    h = h.clamp(4.6, 5.6);
                }
                let vault = match kind {
                    Kind::Cosmos => true,
                    Kind::Labyrinth | Kind::Pitfall => false,
                    _ => {
                        (w >= 12.0 && r(10) > 0.35)
                            || (version >= 17 && h >= 9.0)
                            || matches!(kind, Kind::Pedestal | Kind::Atrium)
                    }
                };
                rooms.push(Room {
                    x0: scx - w * 0.5,
                    x1: scx + w * 0.5,
                    z0: scz - d * 0.5,
                    z1: scz + d * 0.5,
                    floor: base,
                    h,
                    kind,
                    vault,
                });
            }
        }
        rooms[4].vault = true;
        // Spanning tree over the 3x3 grid plus a few loops.
        let mut edges: Vec<(f32, usize, usize)> = Vec::new();
        for j in 0..3usize {
            for i in 0..3usize {
                if i + 1 < 3 {
                    edges.push((
                        hash2(i as i32, j as i32, seed.wrapping_add(31)),
                        j * 3 + i,
                        j * 3 + i + 1,
                    ));
                }
                if j + 1 < 3 {
                    edges.push((
                        hash2(i as i32 + 9, j as i32, seed.wrapping_add(37)),
                        j * 3 + i,
                        (j + 1) * 3 + i,
                    ));
                }
            }
        }
        edges.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut parent: Vec<usize> = (0..9).collect();
        let mut tree: Vec<(usize, usize)> = Vec::new();
        let mut extra: Vec<(usize, usize)> = Vec::new();
        for (n, (_, a, b)) in edges.iter().copied().enumerate() {
            let (fa, fb) = (find(&mut parent, a), find(&mut parent, b));
            if fa != fb {
                parent[fa] = fb;
                tree.push((a, b));
            } else if hash2(n as i32, 5, seed.wrapping_add(41)) < 0.3 {
                extra.push((a, b));
            }
        }
        let mut degree = [0usize; 9];
        if version >= layout::FIRST_VERSION {
            layout::complete_loop(&edges, &tree, &mut extra);
        }
        for &(a, b) in &tree {
            degree[a] += 1;
            degree[b] += 1;
        }
        if version >= layout::FIRST_VERSION {
            for &(a, b) in &extra {
                degree[a] += 1;
                degree[b] += 1;
            }
        }
        // Dead ends may sink a level when the corridor is long enough to ramp.
        for &(a, b) in &tree {
            let (leaf, other) = if degree[a] == 1 && a != 4 {
                (a, b)
            } else if degree[b] == 1 && b != 4 {
                (b, a)
            } else {
                continue;
            };
            if link(&rooms, leaf, other, None) >= 18.0
                && hash2(leaf as i32, 8, seed.wrapping_add(43)) < 0.6
            {
                rooms[leaf].floor = rooms[other].floor - LEVEL_STEP;
            }
        }
        // The farthest dead end holds the treasure.
        let mut best = (0usize, 0.0f32);
        for (i, r) in rooms.iter().enumerate() {
            if degree[i] == 1 && i != 4 {
                let d = (r.centre().0 - cx).hypot(r.centre().1 - cz);
                if d >= best.1 {
                    best = (i, d);
                }
            }
        }
        if version >= layout::FIRST_VERSION {
            best = (layout::objective(&rooms, &tree, &extra, 4), 1.0);
        }
        if best.1 > 0.0 {
            rooms[best.0].kind = Kind::Treasure;
            rooms[best.0].vault = false;
        }
        let mut corrs = Vec::new();
        for &(a, b) in &tree {
            link(&rooms, a, b, Some(&mut corrs));
        }
        for &(a, b) in &extra {
            if (rooms[a].floor - rooms[b].floor).abs() < 0.01 {
                link(&rooms, a, b, Some(&mut corrs));
            }
        }
        let shaft = Some((cx, cz, ground));
        let gatehouse = Some(gatehouse(seed));
        let (min, max) = Self::bounds(&rooms, &corrs, shaft, ground);
        Dungeon {
            theme,
            seed,
            rooms,
            corrs,
            shaft,
            gatehouse,
            ground,
            min,
            max,
        }
    }

    fn contains(&self, m: Vec3) -> bool {
        m.x >= self.min.x
            && m.x <= self.max.x
            && m.z >= self.min.z
            && m.z <= self.max.z
            && m.y >= self.min.y
            && m.y <= self.max.y
    }

    /// Everything below the surface: rooms, corridors, their walls and furnishings.
    fn structure(&self, cfg: &WorldSampleConfig, m: Vec3) -> Option<B> {
        let v = cfg.voxel_meters();
        let wt = v.max(1.0);
        let mut shell = false;
        let mut floor_hit = false;
        let mut ore = false;
        let mut cosmos = false;
        let mut cosmos_floor: Option<&Room> = None;
        let mut bearer_shell = false;
        for r in &self.rooms {
            if m.x < r.x0 - wt || m.x >= r.x1 + wt || m.z < r.z0 - wt || m.z >= r.z1 + wt {
                continue;
            }
            let fy = m.y - r.floor;
            if fy < -wt - r.pit_depth() || fy >= r.h + wt {
                continue;
            }
            let inside_xz = m.x >= r.x0 && m.x < r.x1 && m.z >= r.z0 && m.z < r.z1;
            if r.kind == Kind::WeighingHall {
                if inside_xz && fy >= -bearer_hall::DEPTH && fy < r.h {
                    return Some(bearer_hall::block(m.x-r.x0, fy, m.z-r.z0,
                        r.x1-r.x0, r.z1-r.z0, self.seed));
                }
                bearer_shell = true;
            }
            if inside_xz && fy >= 0.0 && fy < r.height_at(m.x) {
                return Some(
                    self.furnish(cfg, r, m.x - r.x0, m.z - r.z0, fy, v)
                        .unwrap_or(B::Air),
                );
            }
            if inside_xz && fy < 0.0 && r.kind == Kind::Pitfall {
                return Some(pit_block(self.seed, r, m.x - r.x0, m.z - r.z0, fy, wt));
            }
            shell = true;
            floor_hit |= inside_xz && fy < 0.0;
            if inside_xz && fy < 0.0 && r.kind == Kind::Cosmos {
                cosmos_floor = Some(r);
            }
            ore |= r.kind == Kind::Ore;
            cosmos |= r.kind == Kind::Cosmos;
        }
        // Annexes sit outside the room footprint, so they are tested on their own.
        for r in self.rooms.iter().filter(|r| r.kind == Kind::Cosmos) {
            if let Some(b) = self.annex_block(r, m, v, wt) {
                return Some(b);
            }
        }
        let w = CORR_W * 0.5;
        for c in &self.corrs {
            if m.x < c.x0 - wt || m.x >= c.x1 + wt || m.z < c.z0 - wt || m.z >= c.z1 + wt {
                continue;
            }
            let fy = m.y - c.floor_at(m.x, m.z);
            if fy < -wt || fy >= CORR_H + wt {
                continue;
            }
            let inside_xz = m.x >= c.x0 && m.x < c.x1 && m.z >= c.z0 && m.z < c.z1;
            if inside_xz && (0.0..CORR_H).contains(&fy) {
                // A standing torch every nine metres keeps corridors readable.
                let (along, lateral) = if c.along_x {
                    (m.x, m.z - (c.z0 + w))
                } else {
                    (m.z, m.x - (c.x0 + w))
                };
                if fy < v
                    && lateral > w - v.max(0.6)
                    && along.rem_euclid(9.0) < v.max(0.5)
                    && (c.a1 - c.a0).abs() > 4.0
                {
                    return Some(B::Torch);
                }
                return Some(B::Air);
            }
            shell = true;
            floor_hit |= inside_xz && fy < 0.0;
        }
        if !shell {
            return None;
        }
        if bearer_shell {
            return Some(B::Basalt);
        }
        if let Some(r) = cosmos_floor {
            return Some(self.cosmos_floor(r, m.x - r.x0, m.z - r.z0));
        }
        if floor_hit {
            return Some(self.theme.floor);
        }
        if cosmos {
            // The chamber's walls and vault are void; a rare star-glass tile
            // catches the eye the way a lit window would.
            let n = hash2(
                (m.x * 2.0).floor() as i32 ^ (m.y * 2.0).floor() as i32,
                (m.z * 2.0).floor() as i32,
                self.seed.wrapping_add(72),
            );
            return Some(if n > 0.965 {
                B::StarGlass
            } else {
                B::VoidStone
            });
        }
        let n = hash2(
            (m.x * 2.0).floor() as i32 ^ (m.y * 2.0).floor() as i32,
            (m.z * 2.0).floor() as i32,
            self.seed.wrapping_add(71),
        );
        if ore && n > 0.62 {
            return Some(match (n * 40.0) as u32 % 4 {
                0 => B::CopperOre,
                1 => B::TinOre,
                2 => B::IronOre,
                _ => B::CoalOre,
            });
        }
        Some(if n > 0.8 {
            self.theme.alt
        } else {
            self.theme.wall
        })
    }

    /// Generator 18 dressing: Noita pedestals, an Aether dais, a Terraria life
    /// crystal, and a Minecraft spawner cage. Older room kinds keep their
    /// original blocks wherever this returns `None`.
    fn dress_v18(
        &self,
        cfg: &WorldSampleConfig,
        r: &Room,
        rx: f32,
        rz: f32,
        fy: f32,
        v: f32,
    ) -> Option<B> {
        let (w, d) = (r.x1 - r.x0, r.z1 - r.z0);
        let (mx, mz) = (w * 0.5, d * 0.5);
        match r.kind {
            Kind::Pedestal => pedestal_row(rx, rz, fy, v, w, d, r.h),
            Kind::Atrium => atrium(rx, rz, fy, v, w, d, r.h),
            _ if cfg.generator_version < 18 => None,
            Kind::Shrine if w >= 11.0 && d >= 9.0 => {
                let z = d * 0.42;
                item_pedestal(rx, rz, fy, v, mx - 3.3, z, B::Crystal)
                    .or_else(|| item_pedestal(rx, rz, fy, v, mx, z, B::GoldBlock))
                    .or_else(|| item_pedestal(rx, rz, fy, v, mx + 3.3, z, B::SurveyCache))
            }
            Kind::Treasure if w >= 12.0 && d >= 12.0 => {
                // Aether hoard: a metal offering in each corner, around the chest.
                item_pedestal(rx, rz, fy, v, 2.6, 2.6, B::CopperBlock)
                    .or_else(|| item_pedestal(rx, rz, fy, v, w - 2.6, 2.6, B::SilverBlock))
                    .or_else(|| item_pedestal(rx, rz, fy, v, 2.6, d - 2.6, B::GoldBlock))
                    .or_else(|| item_pedestal(rx, rz, fy, v, w - 2.6, d - 2.6, B::PlatinumBlock))
            }
            Kind::Library => {
                if (rx - mx).abs() < 0.8 && (rz - mz).abs() < 0.8 && fy > 3.0 && fy < r.h - 0.4 {
                    Some(B::Crystal)
                } else if rx < 2.4 && (rz - mz).abs() < 0.7 && fy < v {
                    Some(B::GoldBlock)
                } else {
                    None
                }
            }
            Kind::Ore => spawner_cage(rx, rz, fy, v, mx, mz, w),
            Kind::Grotto => item_pedestal(rx, rz, fy, v, mx, mz, B::Crystal),
            _ => None,
        }
    }

    fn furnish(
        &self,
        cfg: &WorldSampleConfig,
        r: &Room,
        rx: f32,
        rz: f32,
        fy: f32,
        v: f32,
    ) -> Option<B> {
        if let Some(block) = self.dress_v18(cfg, r, rx, rz, fy, v) {
            return Some(block);
        }
        if cfg.generator_version >= 19
            && let Some(block) = room_interest(self.seed, r, rx, rz, fy, v) {
                return Some(block);
            }
        let (w, d) = (r.x1 - r.x0, r.z1 - r.z0);
        let (mx, mz) = (w * 0.5, d * 0.5);
        let lane = (rx - mx).abs() < 1.9 || (rz - mz).abs() < 1.9;
        let body = v.max(1.0);
        let unit = body;
        match r.kind {
            Kind::Hub => {
                // Four pillars frame the shaft; braziers burn at their feet.
                let px = (rx - mx).abs() - 4.6;
                let pz = (rz - mz).abs() - 4.6;
                if px.abs() < unit * 0.5 && pz.abs() < unit * 0.5 {
                    return Some(self.theme.alt);
                }
                if (px - 1.2).abs() < unit * 0.5 && (pz - 1.2).abs() < unit * 0.5 && fy < v {
                    return Some(B::Torch);
                }
                if (rx - 2.2).abs() < 0.5 && (rz - 2.2).abs() < 0.5 && (1.0..2.0).contains(&fy) {
                    return Some(B::SalvageCrate);
                }
            }
            Kind::Hall => {
                if rx < 2.2 && rz > d - 2.2 && (1.0..2.0).contains(&fy) {
                    return Some(B::SalvageCrate);
                }
                if w >= 11.0 && d >= 11.0 && !lane {
                    let (px, pz) = ((rx - 3.0).rem_euclid(6.0), (rz - 3.0).rem_euclid(6.0));
                    if px < unit && pz < unit {
                        if cfg.generator_version >= 17
                            && r.h >= 9.0
                            && fy > r.h - 2.0
                            && fy < r.h - 0.5
                        {
                            return Some(B::Crystal);
                        }
                        return Some(self.theme.alt);
                    }
                    if (px - 1.5).abs() < unit * 0.5 && pz < unit && fy < v {
                        return Some(B::Torch);
                    }
                }
            }
            Kind::Crypt => {
                if (rx - mx).abs() < 0.5 && rz > d - 2.0 && (1.0..2.0).contains(&fy) {
                    return Some(B::SurveyCache);
                }
                if !lane && rx > 2.0 && rx < w - 2.0 && rz > 2.5 && rz < d - 2.5 {
                    let (px, pz) = ((rx - 2.0).rem_euclid(4.2), (rz - 2.5).rem_euclid(3.0));
                    if px < 2.4f32.max(unit) && pz < unit && fy < body * 0.9 {
                        return Some(B::Limestone);
                    }
                }
            }
            Kind::Cistern => {
                if cfg.generator_version >= 17 {
                    // A font in the middle of a dry floor, not a room full of water.
                    let basin = (rx - mx).abs() < (w * 0.22).max(2.2)
                        && (rz - mz).abs() < (d * 0.22).max(2.2);
                    if fy < v && basin {
                        return Some(B::Water);
                    }
                    if fy < v {
                        return Some(B::Limestone);
                    }
                    if basin && fy >= v && fy < v * 2.0 {
                        return Some(B::Crystal);
                    }
                    return None;
                }
                if fy < v.max(0.8) {
                    let causeway = (rx - mx).abs() < 1.8 || (rz - mz).abs() < 1.8;
                    if causeway {
                        return Some(B::Cobblestone);
                    }
                    if rx > 1.0 && rx < w - 1.0 && rz > 1.0 && rz < d - 1.0 {
                        return Some(B::Water);
                    }
                }
                if (rx - mx).abs() < unit * 0.5 + 1.0
                    && (rz - mz).abs() < unit * 0.5 + 1.0
                    && fy > 1.0
                    && fy < 1.0 + v
                {
                    return Some(B::Crystal);
                }
            }
            Kind::Grotto => {
                if fy < v {
                    return Some(B::FungalSoil);
                }
                return super::sites::fungus(
                    cfg,
                    self.seed,
                    Vec3::new(rx - mx, fy, rz - mz),
                    Some(2),
                );
            }
            Kind::Treasure => {
                let (ax, az) = ((rx - mx).abs(), (rz - mz).abs());
                if ax < 1.0 + unit * 0.5 && az < 1.0 + unit * 0.5 && fy < 1.0f32.max(v) {
                    return Some(B::StoneBricks);
                }
                if ax < 0.5 && az < 0.5 && (1.0..2.0).contains(&fy) {
                    return Some(B::SurveyCache);
                }
                if ax > mx - 1.6 && az > mz - 1.6 && fy < 2.0 {
                    return Some(B::Crystal);
                }
            }
            Kind::Shrine => {
                if rz > d - 2.5 && (rx - mx).abs() < 1.6 && fy < 1.0f32.max(v) {
                    return Some(B::StoneBricks);
                }
                if rz > d - 2.0 && (rx - mx).abs() < 0.5 && (1.0..2.0).contains(&fy) {
                    return Some(B::Crystal);
                }
                if rz > d - 2.5 && ((rx - mx).abs() - 3.0).abs() < 0.5 && fy < v {
                    return Some(B::GlowCap);
                }
            }
            Kind::Barracks => {
                if rx > w - 2.4 && (rz - mz).abs() < 0.5 && (1.0..2.0).contains(&fy) {
                    return Some(B::CultivatorCache);
                }
                if !lane && (rz < 2.2 || rz > d - 2.2) && rx > 1.0 && rx < w - 1.0 {
                    let px = (rx - 1.0).rem_euclid(3.2);
                    if px < 2.0f32.max(unit) && fy < 0.7f32.max(v) {
                        return Some(B::Planks);
                    }
                }
            }
            Kind::Forge => {
                if rz > d - 3.0 && (rx - mx + 2.5).abs() < unit && fy < 2.0f32.max(v) {
                    return Some(B::Smelter);
                }
                if rz > d - 3.0 && (rx - mx - 2.5).abs() < unit && fy < 1.0f32.max(v) {
                    return Some(B::Workbench);
                }
                if rx > w - 2.4 && rz < 2.4 && (1.0..2.0).contains(&fy) {
                    return Some(B::SalvageCrate);
                }
                if rx < 3.0 && rz < 3.0 && fy < 1.5 {
                    return Some(if rz < 1.5 { B::CoalOre } else { B::IronOre });
                }
                if fy < v && ((rx * 0.5).floor() as i32 + (rz * 0.5).floor() as i32) & 1 == 0 {
                    return Some(B::Basalt);
                }
            }
            Kind::Library => {
                if rx < 2.2 && (rz - mz).abs() < 0.5 && (1.0..2.0).contains(&fy) {
                    return Some(B::SurveyCache);
                }
                if !lane && rx > 2.0 && rx < w - 2.0 && rz > 2.0 && rz < d - 2.0 {
                    let pz = (rz - 2.0).rem_euclid(3.0);
                    if pz < unit && fy < 2.4f32.max(v * 2.0) {
                        return Some(B::Planks);
                    }
                }
            }
            Kind::Ore => {
                if (rx - mx).abs() < 1.5 && (rz - mz).abs() < 1.5 && fy < 2.0 {
                    return Some(B::Crystal);
                }
            }
            Kind::Pedestal | Kind::Atrium | Kind::WeighingHall => {}
            Kind::Labyrinth => return labyrinth(self.seed, r, rx, rz, fy, v),
            Kind::Pitfall => {
                // A plinth and lamp at the crossing, and a torch on each aisle end so
                // the way across is lit while the pits stay dark.
                let (ax, az) = ((rx - mx).abs(), (rz - mz).abs());
                if ax < 0.9 && az < 0.9 && fy < unit {
                    return Some(B::StoneBricks);
                }
                if ax < 0.4 && az < 0.4 && fy >= unit && fy < unit * 2.0 {
                    return Some(B::Crystal);
                }
                let at_end = (ax - (mx - 2.0)).abs() < 0.4 && (az - 1.3).abs() < 0.35
                    || (az - (mz - 2.0)).abs() < 0.4 && (ax - 1.3).abs() < 0.35;
                if at_end && fy < unit {
                    return Some(B::Torch);
                }
            }
            Kind::Cosmos => {
                // The orb: one crystal hung in the middle of the dark.
                if (rx - mx).abs() < 0.7 && (rz - mz).abs() < 0.7 && (2.6..3.6).contains(&fy) {
                    return Some(B::Crystal);
                }
                if !lane {
                    let scale = (w.min(d) * 0.5 - 3.0) / 0.55;
                    let (sin, cos) = (0.70f32, 0.714f32);
                    for (px, pz) in DIPPER {
                        let (qx, qz) = (px * cos - pz * sin, px * sin + pz * cos);
                        let (cx, cz) = (mx + qx * scale, mz + qz * scale);
                        if (rx - cx).hypot(rz - cz) < 0.6 {
                            if fy < unit * 1.3 {
                                return Some(B::VoidStone);
                            }
                            if fy < unit * 2.3 {
                                return Some(B::Crystal);
                            }
                        }
                    }
                }
            }
        }
        None
    }

    /// Spiral stair and surface gatehouse above the hub.
    fn shaft_block(&self, cfg: &WorldSampleConfig, m: Vec3) -> Option<B> {
        let (sx, sz, top) = self.shaft?;
        let (dx, dz) = (m.x - sx, m.z - sz);
        if dx.abs() > 2.7 || dz.abs() > 2.7 || m.y > top + v_up(cfg) {
            return None;
        }
        let hub = &self.rooms[4];
        let v = cfg.voxel_meters();
        if m.y < hub.floor + hub.h * 0.5 {
            return None;
        }
        let core = dx.abs() < 1.6 && dz.abs() < 1.6;
        if core {
            let below = top - m.y;
            if below >= 0.0 {
                let turn = below * 0.85;
                let (px, pz) = (turn.cos() * 1.05, turn.sin() * 1.05);
                if (dx - px).hypot(dz - pz) < 0.55 && below.fract() < v.max(0.4) {
                    return Some(B::StoneBricks);
                }
                if below.rem_euclid(8.0) < v && (dx - px).hypot(dz - pz) < 0.7 {
                    return Some(B::GlowCap);
                }
            }
            return Some(B::Air);
        }
        // A brick sleeve keeps caves and water out of the shaft.
        if m.y < top - 1.0 {
            return Some(self.theme.wall);
        }
        None
    }

    pub fn sample(&self, cfg: &WorldSampleConfig, m: Vec3, height: i32) -> Option<B> {
        if !self.contains(m) {
            return None;
        }
        if let Some(b) = self.shaft_block(cfg, m) {
            return Some(b);
        }
        if let (Some(g), Some((sx, sz, top))) = (&self.gatehouse, self.shaft)
            && m.y >= top - 1.0 {
                if let Some(b) = g.sample(
                    Vec3::new(m.x - sx, m.y - top, m.z - sz),
                    cfg.voxel_meters(),
                    (height + 1) as f32 * cfg.voxel_meters() - top,
                ) {
                    return Some(b);
                }
                return None;
            }
        // Keep two metres of rock over every void so rooms never breach the surface.
        let surface = (height + 1) as f32 * cfg.voxel_meters();
        if m.y > surface - 2.0 {
            return None;
        }
        self.structure(cfg, m)
    }
}

fn v_up(cfg: &WorldSampleConfig) -> f32 {
    cfg.voxel_meters() * 2.0
}

fn gatehouse(seed: u32) -> Building {
    let mut s = WingSpec::new(-3.6, -3.6, 2, 2, seed.wrapping_add(5));
    s.h = 4.4;
    s.roof = Roof::Hip;
    s.pal = Palette::STONE;
    s.windows = Windows::Slits;
    s.age = 0.12;
    s.open = true;
    s.doors = vec![Door {
        side: 1,
        u: 3.6,
        hw: 1.1,
        height: 2.6,
    }];
    Building::new(vec![Wing::new(s)], vec![], None)
}

// ---------------------------------------------------------------------------
// Cell lookup

type CellKey = (u32, u32, u8, i32, i32);

thread_local! {
    static CELLS: RefCell<FastMap<CellKey, Option<Rc<Dungeon>>>> = RefCell::new(FastMap::default());
    static LAST: RefCell<Option<(CellKey, Option<Rc<Dungeon>>)>> = const { RefCell::new(None) };
    static VAULTS: RefCell<Option<((u32, u32, u8, i32), Rc<[Dungeon; 3]>)>> = const { RefCell::new(None) };
}

fn cell_key(cfg: &WorldSampleConfig, ix: i32, iz: i32) -> CellKey {
    (
        cfg.seed,
        cfg.generator_version,
        cfg.voxel_scale as u8,
        ix,
        iz,
    )
}

/// Centre of the complex owned by a cell, if it has one.
fn plan_cell(cfg: &WorldSampleConfig, ix: i32, iz: i32) -> Option<Dungeon> {
    let seed = cfg
        .seed
        .wrapping_add((ix as u32).wrapping_mul(2246822519))
        .wrapping_add((iz as u32).wrapping_mul(3266489917));
    if hash2(ix, iz, cfg.seed.wrapping_add(2301)) < 0.52 {
        return None;
    }
    let cx = ix as f32 * CELL + (hash2(ix, iz, cfg.seed.wrapping_add(2302)) - 0.5) * 36.0;
    let cz = iz as f32 * CELL + (hash2(ix, iz, cfg.seed.wrapping_add(2303)) - 0.5) * 36.0;
    if cx.hypot(cz) < 170.0 || super::valley::reserves(cfg, cx, cz) {
        return None;
    }
    let v = cfg.voxel_meters();
    let ground =
        sample_column(cfg, cfg.blocks_for_height(cx), cfg.blocks_for_height(cz)).height as f32 * v;
    if !(30.0..130.0).contains(&ground) {
        return None;
    }
    if super::undercrown::reserved(cfg, Vec3::new(cx, ground, cz)) {
        return None;
    }
    // Reject steep sites: the shaft needs a stable collar and rooms need cover.
    for (ox, oz) in [(-40.0f32, 0.0f32), (40.0, 0.0), (0.0, -40.0), (0.0, 40.0)] {
        let g = sample_column(
            cfg,
            cfg.blocks_for_height(cx + ox),
            cfg.blocks_for_height(cz + oz),
        )
        .height as f32
            * v;
        if g < ground - 16.0 || g < 26.0 {
            return None;
        }
    }
    Some(Dungeon::generate(
        seed,
        cx,
        cz,
        ground,
        cfg.generator_version,
    ))
}

fn cell_at(cfg: &WorldSampleConfig, ix: i32, iz: i32) -> Option<Rc<Dungeon>> {
    let key = cell_key(cfg, ix, iz);
    if let Some(hit) = LAST.with(|l| {
        l.borrow()
            .as_ref()
            .filter(|(k, _)| *k == key)
            .map(|(_, d)| d.clone())
    }) {
        return hit;
    }
    let found = CELLS.with(|c| c.borrow().get(&key).cloned());
    let slot = match found {
        Some(slot) => slot,
        None => {
            let built = plan_cell(cfg, ix, iz).map(Rc::new);
            CELLS.with(|c| {
                let mut c = c.borrow_mut();
                if c.len() >= 256 {
                    c.clear();
                }
                c.insert(key, built.clone());
            });
            built
        }
    };
    LAST.with(|l| *l.borrow_mut() = Some((key, slot.clone())));
    slot
}

fn cell_index(v: f32) -> i32 {
    (v / CELL).round() as i32
}

/// Block contributed by a random underground complex, if the voxel is inside one.
pub fn block(cfg: &WorldSampleConfig, m: Vec3, height: i32) -> Option<B> {
    if cfg.generator_version < 16 || cfg.world_type == WorldType::Superflat {
        return None;
    }
    let d = cell_at(cfg, cell_index(m.x), cell_index(m.z))?;
    d.sample(cfg, m, height)
}

/// True when a 72 m cavern chamber must stay out of a complex's footprint.
pub fn reserved(cfg: &WorldSampleConfig, m: Vec3) -> bool {
    if cfg.generator_version < 16 || cfg.world_type == WorldType::Superflat {
        return false;
    }
    cell_at(cfg, cell_index(m.x), cell_index(m.z)).is_some_and(|d| {
        m.x >= d.min.x - 6.0 && m.x <= d.max.x + 6.0 && m.z >= d.min.z - 6.0 && m.z <= d.max.z + 6.0
    })
}

/// 0..1: how deep inside a cosmos chamber (or its annex) a point is. Fades in over
/// a few metres so the doorway already glows before you step through it.
pub fn cosmos_at(cfg: &WorldSampleConfig, p: Vec3) -> f32 {
    if cfg.generator_version < 21 || cfg.world_type == WorldType::Superflat {
        return 0.0;
    }
    let Some(d) = cell_at(cfg, cell_index(p.x), cell_index(p.z)) else {
        return 0.0;
    };
    if !d.contains(p) {
        return 0.0;
    }
    let wt = cfg.voxel_meters().max(1.0);
    let mut best = 0.0f32;
    for r in d.rooms.iter().filter(|r| r.kind == Kind::Cosmos) {
        let fy = p.y - r.floor;
        if fy < -0.5 || fy > r.h + 0.5 {
            continue;
        }
        let inset = (p.x - r.x0).min(r.x1 - p.x).min(p.z - r.z0).min(r.z1 - p.z);
        let t = ((inset + 1.5) / 4.0).clamp(0.0, 1.0);
        best = best.max(t * t * (3.0 - 2.0 * t));
        let a = r.annex(d.seed);
        let (out, along) = r.annex_frame(a, p.x - r.x0, p.z - r.z0);
        if out >= 0.0
            && out < wt + ANNEX_DEPTH
            && along.abs() < ANNEX_HALF_W
            && (-0.5..ANNEX_H + 0.5).contains(&fy)
        {
            best = 1.0;
        }
    }
    best
}

/// Name and anchor of the complex at a position, for the HUD and encounters.
pub fn name_at(cfg: &WorldSampleConfig, p: Vec3) -> Option<&'static str> {
    if cfg.generator_version < 16 || cfg.world_type == WorldType::Superflat {
        return None;
    }
    let d = cell_at(cfg, cell_index(p.x), cell_index(p.z))?;
    if !(d.contains(p) && p.y < d.ground - 2.0) {
        return None;
    }
    // Generator 21 puzzle rooms announce themselves by name.
    for r in &d.rooms {
        let inside = p.x >= r.x0 && p.x < r.x1 && p.z >= r.z0 && p.z < r.z1;
        let fy = p.y - r.floor;
        if inside && fy >= -r.pit_depth() && fy < r.h {
            match r.kind {
                Kind::Cosmos => return Some("Hall of Wandering Stars"),
                Kind::Labyrinth => return Some("Labyrinth of False Walls"),
                Kind::Pitfall => return Some("Hall of the Hollow Floor"),
                _ => {}
            }
        }
    }
    if cosmos_at(cfg, p) > 0.9 {
        return Some("Hall of Wandering Stars");
    }
    Some(d.theme.name)
}

/// (cell id, point) pairs for complexes within `range` metres of the player. The point
/// is the middle of a side room, so fights happen away from the shaft.
pub fn anchors(cfg: &WorldSampleConfig, player: Vec3, range: f32) -> Vec<((i32, i32), Vec3)> {
    let mut out = Vec::new();
    if cfg.generator_version < 16 || cfg.world_type == WorldType::Superflat {
        return out;
    }
    let (ix, iz) = (cell_index(player.x), cell_index(player.z));
    for z in iz - 1..=iz + 1 {
        for x in ix - 1..=ix + 1 {
            if let Some(d) = cell_at(cfg, x, z) {
                const SIDE: [usize; 8] = [0, 1, 2, 3, 5, 6, 7, 8];
                let r = &d.rooms[SIDE[(hash2(x, z, d.seed) * 8.0) as usize % 8]];
                let (rx, rz) = r.centre();
                let point = Vec3::new(rx, r.floor + 1.0, rz);
                if (point - player).length() < range {
                    out.push(((x, z), point));
                }
                let other = SIDE[(hash2(x, z, d.seed.wrapping_add(9)) * 8.0) as usize % 8];
                if other != SIDE[(hash2(x, z, d.seed) * 8.0) as usize % 8] {
                    let r2 = &d.rooms[other];
                    let (rx, rz) = r2.centre();
                    let point = Vec3::new(rx, r2.floor + 1.0, rz);
                    if (point - player).length() < range {
                        out.push(((x, z), point));
                    }
                }
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Holy Mountain vault tiers

fn room(x0: f32, x1: f32, z0: f32, z1: f32, floor: f32, h: f32, kind: Kind, vault: bool) -> Room {
    Room {
        x0,
        x1,
        z0,
        z1,
        floor,
        h,
        kind,
        vault,
    }
}

fn vault_tier(band: usize, ground: f32, seed: u32, version: u32) -> Dungeon {
    // Rooms sit under the shaft at (18, 0); each tier's ceiling is 24 m * band below the surface.
    let floor = ground - 24.0 * band as f32 - 7.0;
    let o = 18.0;
    let r = |x0: f32, x1: f32, z0: f32, z1: f32, h: f32, kind: Kind, vault: bool| {
        room(o + x0, o + x1, z0, z1, floor, h, kind, vault)
    };
    let (theme, rooms, links): (Theme, Vec<Room>, Vec<(usize, usize)>) = match band {
        1 => (
            Theme {
                wall: B::StoneBricks,
                alt: B::Cobblestone,
                floor: B::FungalSoil,
                name: "Sanctum Landing I - Fungal Vault",
                kinds: THEMES[2].kinds,
            },
            vec![
                r(-7.0, 7.0, -7.0, 7.0, 7.0, Kind::Hub, true),
                r(15.0, 37.0, -8.0, 8.0, 7.0, Kind::Shrine, true),
                r(-8.0, 8.0, -27.0, -15.0, 6.0, Kind::Grotto, true),
                r(-9.0, 9.0, 15.0, 28.0, 6.0, Kind::Library, false),
                r(-26.0, -15.0, -6.0, 6.0, 5.5, Kind::Ore, false),
                r(44.0, 52.0, -4.0, 4.0, 5.0, Kind::Treasure, false),
            ],
            vec![(0, 1), (0, 2), (0, 3), (0, 4), (1, 5)],
        ),
        2 => (
            Theme {
                wall: B::Limestone,
                alt: B::Ice,
                floor: B::Ice,
                name: "Sanctum Landing II - Rime Geode Vault",
                kinds: THEMES[1].kinds,
            },
            vec![
                r(-7.0, 7.0, -7.0, 7.0, 7.0, Kind::Hub, true),
                r(16.0, 32.0, -20.0, -6.0, 6.5, Kind::Cistern, true),
                r(16.0, 32.0, 6.0, 20.0, 6.5, Kind::Crypt, true),
                r(-32.0, -16.0, -20.0, -6.0, 6.0, Kind::Barracks, false),
                r(-32.0, -16.0, 6.0, 20.0, 6.0, Kind::Library, false),
                r(40.0, 56.0, -8.0, 8.0, 7.0, Kind::Shrine, true),
                r(62.0, 70.0, -4.0, 4.0, 5.0, Kind::Treasure, false),
            ],
            vec![(0, 1), (0, 2), (0, 3), (0, 4), (1, 5), (2, 5), (5, 6)],
        ),
        _ => (
            Theme {
                wall: B::Basalt,
                alt: B::StoneBricks,
                floor: B::Basalt,
                name: "Sanctum Landing III - Basalt Deep",
                kinds: THEMES[3].kinds,
            },
            vec![
                r(-7.0, 7.0, -7.0, 7.0, 7.0, Kind::Hub, true),
                r(14.0, 44.0, -14.0, 14.0,
                    if version >= layout::FIRST_VERSION { bearer_hall::HEIGHT } else { 8.0 },
                    if version >= layout::FIRST_VERSION { Kind::WeighingHall } else { Kind::Hall },
                    version < layout::FIRST_VERSION),
                r(-9.0, 9.0, -28.0, -15.0, 6.0, Kind::Forge, true),
                r(-9.0, 9.0, 15.0, 28.0, 6.0, Kind::Barracks, false),
                r(-26.0, -15.0, -7.0, 7.0, 6.0, Kind::Ore, false),
                r(52.0, 62.0, -5.0, 5.0, 6.0, Kind::Treasure, false),
            ],
            vec![(0, 1), (0, 2), (0, 3), (0, 4), (1, 5)],
        ),
    };
    let mut corrs = Vec::new();
    for (a, b) in links {
        link(&rooms, a, b, Some(&mut corrs));
    }
    let (min, max) = Dungeon::bounds(&rooms, &corrs, None, ground);
    Dungeon {
        theme,
        seed: seed.wrapping_add(band as u32 * 101),
        rooms,
        corrs,
        shaft: None,
        gatehouse: None,
        ground,
        min,
        max,
    }
}

fn vaults(cfg: &WorldSampleConfig, ground: f32) -> Rc<[Dungeon; 3]> {
    let key = (
        cfg.seed,
        cfg.generator_version,
        cfg.voxel_scale as u8,
        ground.to_bits() as i32,
    );
    if let Some(found) = VAULTS.with(|v| {
        v.borrow()
            .as_ref()
            .filter(|(k, _)| *k == key)
            .map(|(_, d)| d.clone())
    }) {
        return found;
    }
    let built = Rc::new([
        vault_tier(1, ground, cfg.seed, cfg.generator_version),
        vault_tier(2, ground, cfg.seed, cfg.generator_version),
        vault_tier(3, ground, cfg.seed, cfg.generator_version),
    ]);
    VAULTS.with(|v| *v.borrow_mut() = Some((key, built.clone())));
    built
}

/// Vault complex block at `m`; `ground` is the surface height above the shaft.
pub fn vault(cfg: &WorldSampleConfig, m: Vec3, ground: f32) -> Option<B> {
    if m.x < 18.0 - 36.0
        || m.x > 18.0 + 76.0
        || m.z.abs() > 36.0
        || m.y > ground - 20.0
        || m.y < ground - 90.0
    {
        return None;
    }
    let tiers = vaults(cfg, ground);
    tiers
        .iter()
        .find_map(|t| t.contains(m).then(|| t.structure(cfg, m)).flatten())
}

/// Crystal on a Holy Mountain shrine or treasure dais. Tier 1 is the east shrine
/// of the fungal vault, tier 2 the far shrine of the rime vault, tier 3 the
/// treasure dais of the basalt hall.
pub fn wand_altar(cfg: &WorldSampleConfig, m: Vec3, ground: f32) -> Option<u32> {
    if cfg.generator_version < 16 {
        return None;
    }
    let tiers = vaults(cfg, ground);
    for (index, tier) in tiers.iter().enumerate() {
        if !tier.contains(m) {
            continue;
        }
        let altar = tier.rooms.iter().find(|r| {
            matches!(r.kind, Kind::Shrine | Kind::Treasure)
                && m.x >= r.x0
                && m.x < r.x1
                && m.z >= r.z0
                && m.z < r.z1
                && m.y >= r.floor
                && m.y < r.floor + r.h
        });
        if altar.is_some() && tier.structure(cfg, m) == Some(B::Crystal) {
            return Some((index as u32) + 1);
        }
    }
    None
}

pub fn vault_name(cfg: &WorldSampleConfig, m: Vec3, ground: f32) -> Option<&'static str> {
    if cfg.generator_version < 16 {
        return None;
    }
    let tiers = vaults(cfg, ground);
    tiers.iter().find(|t| t.contains(m)).map(|t| t.theme.name)
}

/// Centre of the deepest vault's great hall, for the boss encounter.
pub fn deep_vault_anchor(cfg: &WorldSampleConfig, ground: f32) -> Vec3 {
    let t = &vaults(cfg, ground)[2];
    let r = &t.rooms[1];
    Vec3::new(r.centre().0, r.floor + 1.0, r.centre().1)
}

/// One fight per Holy Mountain landing. Menace rises with the landing.
pub fn sanctum_fights(cfg: &WorldSampleConfig, ground: f32) -> [(Vec3, u32, u8); 3] {
    let tiers = vaults(cfg, ground);
    let mut out = [(Vec3::ZERO, 0, 0); 3];
    for (i, tier) in tiers.iter().enumerate() {
        let r = &tier.rooms[1];
        let kind = match i {
            0 => 1,
            1 => 3,
            _ => 5,
        };
        out[i] = (
            Vec3::new(r.centre().0, r.floor + 1.0, r.centre().1),
            kind,
            (i as u8) + 1,
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fluid::VoxelScalePreset;

    fn cfg(scale: VoxelScalePreset, seed: u32) -> WorldSampleConfig {
        WorldSampleConfig {
            generator_version: 16,
            seed,
            world_type: WorldType::Hills,
            voxel_scale: scale,
        }
    }

    #[test]
    fn corridors_reach_the_floor_of_both_rooms() {
        for seed in 0..30u32 {
            let d = Dungeon::generate(seed * 7919 + 1, 500.0, 500.0, 70.0, 16);
            assert!(d.rooms.len() == 9);
            // Every room must touch a corridor whose floor matches its own.
            for (i, r) in d.rooms.iter().enumerate() {
                let touched = d.corrs.iter().any(|c| {
                    let overlap = c.x1 > r.x0 - 1.0
                        && c.x0 < r.x1 + 1.0
                        && c.z1 > r.z0 - 1.0
                        && c.z0 < r.z1 + 1.0;
                    let (cx, cz) = ((c.x0 + c.x1) * 0.5, (c.z0 + c.z1) * 0.5);
                    let _ = (cx, cz);
                    overlap && ((c.fa - r.floor).abs() < 0.2 || (c.fb - r.floor).abs() < 0.2)
                });
                assert!(touched, "seed {seed}: room {i} has no matching corridor");
            }
        }
    }

    #[test]
    fn rooms_are_hollow_and_walled() {
        let c = cfg(VoxelScalePreset::OneMeter, 5);
        let d = Dungeon::generate(77, 500.0, 500.0, 70.0, 16);
        let hub = &d.rooms[4];
        let (cx, cz) = hub.centre();
        // Open air above the floor between pillars, brick below it.
        let probe = Vec3::new(cx + 2.0, hub.floor + 1.5, cz + 2.0);
        assert!(matches!(
            d.structure(&c, probe),
            Some(B::Air) | Some(B::Torch)
        ));
        let below = Vec3::new(cx + 2.0, hub.floor - 0.5, cz + 2.0);
        assert!(matches!(d.structure(&c, below), Some(b) if b != B::Air));
        let far = Vec3::new(cx + 300.0, hub.floor, cz);
        assert_eq!(d.structure(&c, far), None);
    }

    #[test]
    fn vault_tiers_are_deterministic_and_have_rooms_under_the_shaft() {
        let c = cfg(VoxelScalePreset::OneMeter, 17);
        let ground = 30.0;
        for band in 1..=3 {
            let floor = ground - 24.0 * band as f32 - 7.0;
            let probe = Vec3::new(18.0 + 4.0, floor + 2.5, 3.0);
            let a = vault(&c, probe, ground);
            let b = vault(&c, probe, ground);
            assert_eq!(a, b);
            assert!(
                matches!(a, Some(B::Air) | Some(B::Torch)),
                "tier {band} hub is not open: {a:?}"
            );
        }
    }

    fn find_dungeon(c: &WorldSampleConfig) -> Rc<Dungeon> {
        for r in 2..9i32 {
            for ix in -r..=r {
                for iz in -r..=r {
                    if ix.abs().max(iz.abs()) == r
                        && let Some(d) = cell_at(c, ix, iz) {
                            return d;
                        }
                }
            }
        }
        panic!("no complex within 9 cells");
    }

    #[test]
    fn a_planned_complex_has_a_shaft_a_gatehouse_and_a_hollow_hub() {
        for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::HalfMeter] {
            let c = cfg(scale, 3040332434);
            let v = scale.meters();
            let d = find_dungeon(&c);
            let (sx, sz, top) = d.shaft.unwrap();
            let height = (top / v) as i32;
            let hub = &d.rooms[4];
            let at = |x: f32, y: f32, z: f32| d.sample(&c, Vec3::new(x, y, z), height);
            // Hollow hub, open shaft all the way up, a roofed gatehouse above the collar.
            assert!(matches!(
                at(sx + 3.0, hub.floor + 1.5 * v.max(1.0), sz + 3.0),
                Some(B::Air) | Some(B::Torch)
            ));
            let mut open = 0;
            let mut y = hub.floor + hub.h + 1.0;
            while y < top {
                if matches!(at(sx + 1.2, y, sz + 1.2), Some(B::Air))
                    || at(sx + 1.2, y, sz + 1.2).is_some()
                {
                    open += 1;
                }
                y += v;
            }
            assert!(open > 0, "shaft is not carved at {scale:?}");
            let mut roof = false;
            let mut y = top + 1.0;
            while y < top + 14.0 {
                for (dx, dz) in [
                    (0.0, 0.0),
                    (2.5, 2.5),
                    (-2.5, 2.5),
                    (2.5, -2.5),
                    (-2.5, -2.5),
                ] {
                    roof |= matches!(at(sx + dx, y, sz + dz), Some(b) if b != B::Air);
                }
                y += v;
            }
            assert!(roof, "gatehouse has no roof at {scale:?}");
        }
    }

    #[test]
    fn complexes_never_carve_within_two_metres_of_the_surface() {
        let c = cfg(VoxelScalePreset::OneMeter, 99);
        let d = find_dungeon(&c);
        let hub = &d.rooms[4];
        let (cx, cz) = hub.centre();
        // Claim the terrain surface sits right at the hub ceiling: nothing may be carved above it.
        let height = ((hub.floor + hub.h) as i32) - 1;
        for dy in 0..6 {
            let p = Vec3::new(cx + 3.0, hub.floor + hub.h - 1.0 + dy as f32, cz + 3.0);
            let b = d.sample(&c, p, height);
            assert!(b.is_none(), "carved {b:?} at +{dy} under a thin roof");
        }
    }

    #[test]
    #[ignore = "wall-clock ratio; flaky under parallel test load (1.07x-1.98x on the same binary). Tracked in docs/PLAN.md debt; moves to `just bench` in P2."]
    fn terrain_generation_cost_stays_close_to_the_previous_generator() {
        use std::sync::atomic::AtomicBool;
        let never = AtomicBool::new(false);
        let time = |version: u32| {
            let c = WorldSampleConfig {
                generator_version: version,
                seed: 424242,
                world_type: WorldType::Hills,
                voxel_scale: VoxelScalePreset::OneMeter,
            };
            let start = std::time::Instant::now();
            let mut solid = 0usize;
            for cx in 6..10 {
                for cz in -2..2 {
                    for cy in -2..3 {
                        let chunk =
                            crate::world::generate_padded(&c, glam::IVec3::new(cx, cy, cz), &never)
                                .unwrap();
                        solid += chunk.blocks.iter().filter(|b| **b != B::Air).count();
                    }
                }
            }
            (start.elapsed().as_secs_f64(), solid)
        };
        // Wall-clock on a shared, hybrid-core machine is bimodal per process: the
        // same binary measured v16 at 1.07x and 1.9x of v15 depending on which core
        // the test thread landed on, steady within each process. Timing one version
        // after the other therefore compared two different machines. Alternating
        // short blocks and keeping each version's best gives both versions the same
        // chance to hit the fast state. Each block warms its own thread-local and
        // tile caches first (the discarded call), so a version switch costs neither.
        let mut old = f64::MAX;
        let mut new = f64::MAX;
        for round in 0..4 {
            for version in if round % 2 == 0 { [15, 16] } else { [16, 15] } {
                let _ = time(version);
                let t = time(version).0;
                if version == 15 {
                    old = old.min(t);
                } else {
                    new = new.min(t);
                }
            }
        }
        println!(
            "terrain generation: v15 {old:.3}s, v16 {new:.3}s ({:.2}x)",
            new / old
        );
        assert!(
            new < old * 1.35 + 0.05,
            "v16 generation is {:.2}x slower than v15",
            new / old
        );
    }

    #[test]
    #[ignore]
    fn profile_generation_stages() {
        use glam::IVec3;
        let version = std::env::var("PROFILE_VERSION")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(crate::world::CURRENT_GENERATOR_VERSION);
        let c = WorldSampleConfig {
            generator_version: version,
            seed: 424242,
            world_type: WorldType::Hills,
            voxel_scale: VoxelScalePreset::OneMeter,
        };
        let size = crate::mesh::PADDED_CHUNK_SIZE;
        let chunks: Vec<IVec3> = (6..10)
            .flat_map(|cx| (-2..2).flat_map(move |cz| (-2..3).map(move |cy| IVec3::new(cx, cy, cz))))
            .collect();
        // One timer per whole sweep: per-call timers cost more than the calls.
        let sweep = |name: &str, f: &dyn Fn(IVec3, i32, i32, crate::world::ColumnSample, i32) -> usize| {
            let mut best = f64::MAX;
            let mut sink = 0;
            for _ in 0..3 {
                let t = std::time::Instant::now();
                for &ch in &chunks {
                    let origin = ch * crate::world::CHUNK_SIZE - IVec3::ONE;
                    for z in 0..size {
                        for x in 0..size {
                            let col = sample_column(&c, origin.x + x, origin.z + z);
                            for y in 0..size {
                                sink += f(origin, x, z, col, y);
                            }
                        }
                    }
                }
                best = best.min(t.elapsed().as_secs_f64());
            }
            println!("v{version} {name:<18} {best:.3}s (sink {sink})");
        };
        sweep("columns+loop", &|_, _, _, _, _| 0);
        sweep("undercrown", &|o, x, z, col, y| {
            super::super::undercrown::block(&c, o + IVec3::new(x, y, z), col.height).is_some() as usize
        });
        sweep("sites::block", &|o, x, z, col, y| {
            super::super::sites::block(&c, o + IVec3::new(x, y, z), col.height).is_some() as usize
        });
        sweep("sites::cavern", &|o, x, z, col, y| {
            super::super::sites::cavern(&c, o + IVec3::new(x, y, z), col.height).is_some() as usize
        });
        sweep("cave_carves", &|o, x, z, col, y| {
            super::super::cave_carves(&c, o + IVec3::new(x, y, z), &col) as usize
        });
        sweep("stratum", &|o, x, z, col, y| {
            let p = o + IVec3::new(x, y, z);
            let depth = col.height - p.y;
            if depth > 6 {
                super::super::landscape::geological_stratum(&c, p, col.height, depth) as usize
            } else {
                0
            }
        });
        sweep("tectonics", &|o, x, z, _, y| {
            if y > 0 {
                return 0;
            }
            let m = (o + IVec3::new(x, y, z)).as_vec3();
            super::super::landscape::sample_tectonics_zoomed(m.x, m.z, c.seed, 2.4).fold_wave as usize
        });
        let profiles_for = |o: IVec3| crate::world::RegionProfiles::new(&c, o.x, o.z, size);
        let mut best = f64::MAX;
        for _ in 0..3 {
            let t = std::time::Instant::now();
            for &ch in &chunks {
                let origin = ch * crate::world::CHUNK_SIZE - IVec3::ONE;
                let profiles = profiles_for(origin);
                for z in 0..size {
                    for x in 0..size {
                        let col = sample_column(&c, origin.x + x, origin.z + z);
                        for y in 0..size {
                            let p = origin + IVec3::new(x, y, z);
                            std::hint::black_box(super::super::sample_block_with(&c, p, col, &profiles));
                        }
                    }
                }
            }
            best = best.min(t.elapsed().as_secs_f64());
        }
        println!("v{version} full sample_block   {best:.3}s");
    }

    #[test]
    #[ignore]
    fn profile_dungeon_lookup() {
        let c = cfg(VoxelScalePreset::OneMeter, 424242);
        let mut hits = 0usize;
        let t = std::time::Instant::now();
        for cx in 5 * 32..13 * 32 {
            for cz in -4 * 32..4 * 32 {
                for y in (-2 * 32..3 * 32).step_by(1) {
                    let m = Vec3::new(cx as f32 + 0.5, y as f32 + 0.5, cz as f32 + 0.5);
                    if block(&c, m, 40).is_some() {
                        hits += 1;
                    }
                    if reserved(&c, m) {
                        hits += 1;
                    }
                }
            }
        }
        println!(
            "dungeon::block + reserved over 13M voxels: {:.3}s ({hits} hits)",
            t.elapsed().as_secs_f64()
        );
    }

    #[test]
    fn generator_18_pedestal_holds_an_offering() {
        let c = WorldSampleConfig {
            generator_version: 18,
            seed: 1,
            world_type: WorldType::Hills,
            voxel_scale: VoxelScalePreset::OneMeter,
        };
        for seed in 0..48u32 {
            let d = Dungeon::generate(seed.wrapping_mul(97) + 3, 500.0, 500.0, 70.0, 18);
            let Some(room) = d
                .rooms
                .iter()
                .find(|r| matches!(r.kind, Kind::Pedestal | Kind::Atrium))
            else {
                continue;
            };
            let (w, depth) = (room.x1 - room.x0, room.z1 - room.z0);
            let (px, pz, expect) = if room.kind == Kind::Pedestal {
                (room.x0 + w * 0.5, room.z0 + depth * 0.62, B::GoldBlock)
            } else {
                (room.x0 + w * 0.5, room.z0 + depth * 0.5, B::MithrilBlock)
            };
            assert_eq!(
                d.structure(&c, Vec3::new(px, room.floor + 2.5, pz)),
                Some(expect),
                "seed {seed} {:?}",
                room.kind
            );
            assert_eq!(
                d.structure(&c, Vec3::new(px, room.floor + 1.2, pz)),
                Some(B::StoneBricks),
                "seed {seed}"
            );
            return;
        }
        panic!("expected a pedestal row or an Aether antechamber");
    }

    #[test]
    fn generator_19_corners_are_worth_a_look_and_the_cross_stays_open() {
        let c = WorldSampleConfig {
            generator_version: 19,
            seed: 1,
            world_type: WorldType::Hills,
            voxel_scale: VoxelScalePreset::OneMeter,
        };
        for seed in 0..40u32 {
            let d = Dungeon::generate(seed.wrapping_mul(131) + 11, 500.0, 500.0, 70.0, 19);
            let Some(room) = d.rooms.iter().find(|r| {
                !matches!(
                    r.kind,
                    Kind::Hub
                        | Kind::Pedestal
                        | Kind::Atrium
                        | Kind::Cistern
                        | Kind::Treasure
                        | Kind::Ore
                ) && r.x1 - r.x0 >= 11.0
                    && r.z1 - r.z0 >= 11.0
            }) else {
                continue;
            };
            let (w, depth) = (room.x1 - room.x0, room.z1 - room.z0);
            let mid = d.structure(
                &c,
                Vec3::new(room.x0 + w * 0.5, room.floor + 1.5, room.z0 + depth * 0.5),
            );
            assert_eq!(mid, Some(B::Air), "seed {seed} corridor cross");
            let mut dressed = false;
            for (x, z) in [
                (2.6, 2.6),
                (w - 2.6, 2.6),
                (2.6, depth - 2.6),
                (w - 2.6, depth - 2.6),
            ] {
                for dy in [0.4, 1.3] {
                    let block =
                        d.structure(&c, Vec3::new(room.x0 + x, room.floor + dy, room.z0 + z));
                    if block.is_some_and(|b| b != B::Air) {
                        dressed = true;
                    }
                }
            }
            assert!(dressed, "seed {seed} {:?} should dress a corner", room.kind);
            return;
        }
        panic!("expected a large side room");
    }

    fn cfg21(scale: VoxelScalePreset) -> WorldSampleConfig {
        WorldSampleConfig {
            generator_version: 21,
            ..cfg(scale, 7)
        }
    }

    fn puzzle_room(kind: Kind) -> (Dungeon, usize) {
        for seed in 0..600u32 {
            let d = Dungeon::generate(seed * 7919 + 1, 500.0, 500.0, 70.0, 21);
            if let Some(i) = d.rooms.iter().position(|r| r.kind == kind) {
                return (d, i);
            }
        }
        panic!("no {kind:?} room in 600 complexes");
    }

    #[test]
    fn generator_21_adds_puzzle_rooms_and_older_versions_do_not() {
        for kind in [Kind::Cosmos, Kind::Labyrinth, Kind::Pitfall] {
            puzzle_room(kind);
        }
        for seed in 0..200u32 {
            let d = Dungeon::generate(seed * 7919 + 1, 500.0, 500.0, 70.0, 20);
            assert!(
                !d.rooms
                    .iter()
                    .any(|r| matches!(r.kind, Kind::Cosmos | Kind::Labyrinth | Kind::Pitfall)),
                "seed {seed}: version 20 must keep its old layout"
            );
        }
    }

    #[test]
    fn cosmos_annex_hides_a_cache_behind_a_mirage_plug() {
        let c = cfg21(VoxelScalePreset::OneMeter);
        let (d, i) = puzzle_room(Kind::Cosmos);
        let r = &d.rooms[i];
        let (mut plug, mut cache) = (0, 0);
        let mut y = r.floor;
        while y < r.floor + ANNEX_H {
            let mut x = r.x0 - 7.0;
            while x < r.x1 + 7.0 {
                let mut z = r.z0 - 7.0;
                while z < r.z1 + 7.0 {
                    match d.structure(&c, Vec3::new(x, y, z)) {
                        Some(B::MirageVoid) => plug += 1,
                        Some(B::SurveyCache) => cache += 1,
                        _ => {}
                    }
                    z += 0.5;
                }
                x += 0.5;
            }
            y += 0.5;
        }
        assert!(plug > 0, "the annex has no false wall");
        assert!(cache > 0, "the annex has no cache");
        let (cx, cz) = r.centre();
        assert!(cosmos_at(&c, Vec3::new(cx, r.floor + 1.0, cz)).abs() <= 1.0);
        // Straight down the middle of the chamber the fog is full cosmos.
        let probe = Vec3::new(cx, r.floor + 1.0, cz);
        let inset = (probe.x - r.x0)
            .min(r.x1 - probe.x)
            .min(probe.z - r.z0)
            .min(r.z1 - probe.z);
        assert!(inset > 2.5);
    }

    #[test]
    fn the_phantom_floor_is_mirage_over_a_pit_and_the_aisle_holds() {
        let c = cfg21(VoxelScalePreset::OneMeter);
        let (d, i) = puzzle_room(Kind::Pitfall);
        let r = &d.rooms[i];
        let (cx, cz) = r.centre();
        // The aisle cross is real brick the whole way down.
        for dy in [-0.5, -1.5, -2.5] {
            assert_eq!(
                d.structure(&c, Vec3::new(cx + 0.3, r.floor + dy, cz + 5.0)),
                Some(B::StoneBricks)
            );
        }
        // A pit quadrant has a one-metre mirage skin and open space beneath.
        let (qx, qz) = (cx + 5.5, cz + 5.5);
        assert_eq!(
            d.structure(&c, Vec3::new(qx, r.floor - 0.5, qz)),
            Some(B::MirageBrick)
        );
        let below = d.structure(&c, Vec3::new(qx + 2.0, r.floor - 1.8, qz + 2.0));
        assert!(
            matches!(
                below,
                Some(B::Air) | Some(B::Water) | Some(B::SurveyCache) | Some(B::StoneBricks)
            ),
            "{below:?}"
        );
        assert_eq!(
            d.structure(&c, Vec3::new(qx, r.floor - PIT_DEPTH - 0.5, qz)),
            Some(B::StoneBricks)
        );
        // The skin is passable and everything else in the room is not.
        assert!(!B::MirageBrick.is_solid());
        assert!(B::StoneBricks.is_solid());
    }

    #[test]
    fn the_maze_is_solvable_by_its_real_walls_alone() {
        let c = cfg21(VoxelScalePreset::OneMeter);
        let (d, i) = puzzle_room(Kind::Labyrinth);
        let r = &d.rooms[i];
        let (w, dd) = (r.x1 - r.x0, r.z1 - r.z0);
        let step = 0.25f32;
        let (nx, nz) = ((w / step) as usize, (dd / step) as usize);
        let blocked = |ix: usize, iz: usize| {
            let p = Vec3::new(
                r.x0 + (ix as f32 + 0.5) * step,
                r.floor + 0.5,
                r.z0 + (iz as f32 + 0.5) * step,
            );
            match d.structure(&c, p) {
                Some(B::Air) | Some(B::Torch) | None => false,
                // A mirage is walkable, but the solution must not depend on one.
                _ => true,
            }
        };
        let mut seen = vec![false; nx * nz];
        let start = (2usize, 2usize);
        assert!(!blocked(start.0, start.1), "ring walkway start is blocked");
        let mut stack = vec![start];
        seen[start.1 * nx + start.0] = true;
        while let Some((x, z)) = stack.pop() {
            for (dx, dz) in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)] {
                let (ax, az) = (x as i32 + dx, z as i32 + dz);
                if ax < 0 || az < 0 || ax >= nx as i32 || az >= nz as i32 {
                    continue;
                }
                let (ax, az) = (ax as usize, az as usize);
                if !seen[az * nx + ax] && !blocked(ax, az) {
                    seen[az * nx + ax] = true;
                    stack.push((ax, az));
                }
            }
        }
        // The cache cell must be touched: some reached sample sits beside a cache block.
        let mut reached_cache = false;
        for iz in 0..nz {
            for ix in 0..nx {
                if !seen[iz * nx + ix] {
                    continue;
                }
                let p = Vec3::new(
                    r.x0 + (ix as f32 + 0.5) * step,
                    r.floor + 0.5,
                    r.z0 + (iz as f32 + 0.5) * step,
                );
                for (dx, dz) in [(0.5f32, 0.0f32), (-0.5, 0.0), (0.0, 0.5), (0.0, -0.5)] {
                    if d.structure(&c, p + Vec3::new(dx, 0.0, dz)) == Some(B::SurveyCache) {
                        reached_cache = true;
                    }
                }
            }
        }
        assert!(
            reached_cache,
            "no route reaches the cache through real walls"
        );
    }

    #[test]
    #[ignore = "prints review coordinates"]
    fn print_review_rooms() {
        let c = WorldSampleConfig {
            generator_version: 21,
            seed: 0xA341_316C,
            world_type: WorldType::Hills,
            voxel_scale: VoxelScalePreset::OneMeter,
        };
        for ix in -8..=8 {
            for iz in -8..=8 {
                if let Some(d) = cell_at(&c, ix, iz) {
                    for r in &d.rooms {
                        if matches!(r.kind, Kind::Cosmos | Kind::Labyrinth | Kind::Pitfall) {
                            let (cx, cz) = r.centre();
                            let a = r.annex(d.seed);
                            println!(
                                "ROOM {:?} cell=({ix},{iz}) centre=({cx:.1},{cz:.1}) floor={:.1} size={:.1}x{:.1} h={:.1} annex_side={} along={:.1} ground={:.1}",
                                r.kind,
                                r.floor,
                                r.x1 - r.x0,
                                r.z1 - r.z0,
                                r.h,
                                a.side,
                                a.along,
                                d.ground
                            );
                        }
                    }
                }
            }
        }
    }
}
