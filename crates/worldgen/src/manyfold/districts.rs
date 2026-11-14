//! A cylindrical maze: angular neighbors wrap, axial neighbors stop at the ends.
//! The seed chooses passages, never the physical gravity field or route identity.
use super::{CENTER, HALF_LENGTH, OUTER_RADIUS, RADIUS};
use crate::{BlockId, WorldSampleConfig, hash_u32};
use glam::{IVec3, Vec3};
use std::{cell::RefCell, f32::consts::TAU};

pub(super) const VERSION: u32 = 80;
const SECTORS: usize = 24;
const ROWS: usize = 6;
const CELLS: usize = SECTORS * ROWS;
const ARC: f32 = TAU * RADIUS;
const BAY: f32 = ARC / SECTORS as f32;
const ROW: f32 = 2.0 * HALF_LENGTH / ROWS as f32;
const WALL_HEIGHT: f32 = 3.6;
const TALL_WALL_HEIGHT: f32 = 10.0;
const WALL_HALF: f32 = 0.7;
const DOOR_HALF: f32 = 2.0;

struct Layout {
    seed: u32,
    // Bits: clockwise, counterclockwise, forward, backward.
    passages: [u8; CELLS],
}

fn neighbor(cell: usize, direction: usize) -> Option<usize> {
    let (sector, row) = (cell % SECTORS, cell / SECTORS);
    match direction {
        0 => Some(row * SECTORS + (sector + 1) % SECTORS),
        1 => Some(row * SECTORS + (sector + SECTORS - 1) % SECTORS),
        2 if row + 1 < ROWS => Some(cell + SECTORS),
        3 if row > 0 => Some(cell - SECTORS),
        _ => None,
    }
}

impl Layout {
    fn new(seed: u32) -> Self {
        let mut layout = Self {
            seed,
            passages: [0; CELLS],
        };
        let mut visited = [false; CELLS];
        let mut stack = [0usize; CELLS];
        let mut depth = 1;
        let mut state = hash_u32(seed ^ 0x6d61_7a65);
        visited[0] = true;
        // A spanning tree guarantees every chamber has a supported return.
        // Fixed arrays bound generation work and memory independently of seed.
        while depth > 0 {
            let cell = stack[depth - 1];
            let mut choices = [(0usize, 0usize); 4];
            let mut count = 0;
            for direction in 0..4 {
                if let Some(next) = neighbor(cell, direction) {
                    if !visited[next] {
                        choices[count] = (direction, next);
                        count += 1;
                    }
                }
            }
            if count == 0 {
                depth -= 1;
                continue;
            }
            state = hash_u32(state.wrapping_add(0x9e37_79b9));
            let (direction, next) = choices[state as usize % count];
            layout.connect(cell, direction, next);
            visited[next] = true;
            stack[depth] = next;
            depth += 1;
        }
        // Sparse additional doors make loops and alternatives to retracing.
        for cell in 0..CELLS {
            for direction in [0, 2] {
                if let Some(next) = neighbor(cell, direction) {
                    if hash_u32(seed ^ (cell as u32 * 17 + direction as u32) ^ 0x1009) % 7 == 0 {
                        layout.connect(cell, direction, next);
                    }
                }
            }
        }
        layout
    }

    fn connect(&mut self, cell: usize, direction: usize, next: usize) {
        self.passages[cell] |= 1 << direction;
        self.passages[next] |= 1 << (direction ^ 1);
    }
}

thread_local! {
    // One tiny layout per worker, replaced when the world seed changes.
    static LAYOUT: RefCell<Option<Layout>> = const { RefCell::new(None) };
}

/// An existing closed maze wall, never a new worldgen feature or terrain gate.
/// Releasing it gives a second route through two adjacent chambers.
pub(super) fn shortcut(seed: u32) -> Option<(f32, f32)> {
    LAYOUT.with(|cache| {
        let mut cache = cache.borrow_mut();
        let layout = cache.get_or_insert_with(|| Layout::new(seed));
        if layout.seed != seed { *layout = Layout::new(seed); }
        for district in 0..6 {
            let sector = district * 4 + 2;
            for row in [1, 4, 0, 5] {
                if layout.passages[row * SECTORS + sector] & 1 == 0 {
                    let angle = ((sector + 1) as f32 * BAY - 2.0 * BAY) / RADIUS;
                    return Some((angle, -HALF_LENGTH + (row as f32 + 0.5) * ROW));
                }
            }
        }
        None
    })
}

fn maze_wall(seed: u32, arc: f32, z: f32) -> bool {
    let sector = ((arc / BAY).floor() as usize).min(SECTORS - 1);
    let row = (((z + HALF_LENGTH) / ROW).floor() as usize).min(ROWS - 1);
    let u = arc - sector as f32 * BAY;
    let v = z + HALF_LENGTH - row as f32 * ROW;
    LAYOUT.with(|cache| {
        let mut cache = cache.borrow_mut();
        let layout = cache.get_or_insert_with(|| Layout::new(seed));
        if layout.seed != seed {
            *layout = Layout::new(seed);
        }
        let doors = layout.passages[row * SECTORS + sector];
        let angular_door = (v - ROW * 0.5).abs() < DOOR_HALF;
        let axial_door = (u - BAY * 0.5).abs() < DOOR_HALF;
        (u < WALL_HALF && !(doors & 2 != 0 && angular_door))
            || (u > BAY - WALL_HALF && !(doors & 1 != 0 && angular_door))
            || (v < WALL_HALF && !(doors & 8 != 0 && axial_door))
            || (v > ROW - WALL_HALF && !(doors & 4 != 0 && axial_door))
    })
}

// Material roles follow the same six places on every visit. Seed changes doors.
fn palette(district: usize) -> (BlockId, BlockId, BlockId) {
    match district {
        0 => (BlockId::Grass, BlockId::WeatheredBricks, BlockId::Sandstone),
        1 => (BlockId::Sandstone, BlockId::Limestone, BlockId::StoneBricks),
        2 => (
            BlockId::StoneBricks,
            BlockId::StoneBricks,
            BlockId::Limestone,
        ),
        3 => (
            BlockId::Cobblestone,
            BlockId::WeatheredBricks,
            BlockId::CopperBlock,
        ),
        4 => (BlockId::GroveMoss, BlockId::StoneBricks, BlockId::Log),
        _ => (BlockId::Stone, BlockId::Basalt, BlockId::Limestone),
    }
}

/// Match the actual crystal fixtures, including those on the overhead streets.
pub(super) fn guide_lights(cfg: &WorldSampleConfig) -> Vec<IVec3> {
    let mut cells = Vec::with_capacity(24);
    for district in 0..6 {
        let angle = district as f32 * TAU / 6.0;
        let crown = 5.0 + district as f32 * 0.65;
        for (angle, height, z) in [
            (angle, 0.4, -3.6),
            (angle, 0.4, 3.6),
            (angle, 4.5, 15.0),
            (angle - 4.5 / RADIUS, crown - 0.5, -5.2),
        ] {
            let radius = RADIUS - height;
            let point = CENTER + Vec3::new(angle.sin() * radius, -angle.cos() * radius, z);
            let base = (point / cfg.voxel_meters()).floor().as_ivec3();
            'fixture: for dz in -1..=1 {
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        let cell = base + IVec3::new(dx, dy, dz);
                        if block(cfg, cell) == BlockId::Crystal {
                            if !cells.contains(&cell) {
                                cells.push(cell);
                            }
                            break 'fixture;
                        }
                    }
                }
            }
        }
    }
    cells
}

pub(super) fn block(cfg: &WorldSampleConfig, cell: IVec3) -> BlockId {
    let p = (cell.as_vec3() + Vec3::splat(0.5)) * cfg.voxel_meters() - CENTER;
    let radius = p.truncate().length();
    if radius > OUTER_RADIUS || p.z.abs() > HALF_LENGTH + 2.0 {
        return BlockId::Air;
    }
    if p.z.abs() >= HALF_LENGTH || radius >= OUTER_RADIUS - 2.0 {
        return BlockId::Bedrock;
    }
    // Shift by half a district: entrance garden and opposite workshop both
    // straddle the cardinal orientations, rather than an arbitrary atan2 seam.
    let arc = (p.x.atan2(-p.y) * RADIUS + 2.0 * BAY).rem_euclid(ARC);
    let district = ((arc / (4.0 * BAY)).floor() as usize).min(5);
    let local = arc - (district as f32 * 4.0 + 2.0) * BAY;
    let (floor, wall, trim) = palette(district);
    let ring = p.z.abs() < 3.0;
    let promenade = local.abs() < 2.3;
    let court = local.abs() < 5.4 && p.z.abs() < 7.0;
    let pavilion_z = p.z - 15.0;
    let pavilion = local.abs() < 4.0 && pavilion_z.abs() < 3.5;
    let height = RADIUS - radius;
    if radius >= RADIUS {
        if radius > RADIUS + 2.0 {
            return BlockId::Air;
        }
        return if ring {
            BlockId::Sandstone
        } else if promenade || court {
            trim
        } else if radius < RADIUS + 0.8 {
            floor
        } else {
            BlockId::Stone
        };
    }
    // A clear 6 m circumference and six axial streets connect all districts.
    // Doors are 4 m wide at the floor; walls grow inward with local up.
    // Ten-metre white partitions stay short of the axis, so the hollow stays open
    // and the same walls are visible from every facing, including overhead.
    let tall = cfg.generator_version >= super::TALL_WHITE_WALLS;
    let wall_height = if tall { TALL_WALL_HEIGHT } else { WALL_HEIGHT };
    let (shaft, cap) = if tall {
        (BlockId::WhitePlaster, BlockId::WhitePlaster)
    } else {
        (wall, trim)
    };
    if !ring
        && !promenade
        && !court
        && !pavilion
        && height < wall_height
        && maze_wall(cfg.seed, arc, p.z)
    {
        return if height > wall_height - 0.8 { cap } else { shaft };
    }
    // Landmark pavilions sit on a continuous floor beyond each courtyard.
    // Preserve a central walk-through opening and generous standing clearance.
    if pavilion {
        if (5.0..6.3).contains(&height) {
            return trim;
        }
        if height < 5.0 && local.abs() > 2.6 && pavilion_z.abs() > 2.1 {
            return wall;
        }
        if (4.0..5.0).contains(&height) && local.abs() < 0.65 && pavilion_z.abs() < 0.65 {
            return BlockId::Crystal;
        }
        // Side furniture leaves the axial passage open. These are ordinary
        // solid materials; machines with world-up placement are not faked here.
        if local.abs() > 2.6 && pavilion_z.abs() < 1.5 {
            if district == 2 && height < 3.2 {
                return BlockId::Planks;
            }
            if district == 3 && height < 1.1 {
                return BlockId::Planks;
            }
            if (district == 0 || district == 4) && height < 0.9 {
                return BlockId::Leaves;
            }
        }
    }
    // Distinct open crowns identify the six streets across the central hollow.
    // Towers stand beside the promenade, not on its return path.
    let tower_u = local + 4.5;
    let tower_z = p.z + 5.2;
    let crown = 5.0 + district as f32 * 0.65;
    if tower_u.abs() < 1.4 && tower_z.abs() < 1.4 && height < crown + 1.0 {
        if height > crown {
            return trim;
        }
        if tower_u.abs() > 0.65 && tower_z.abs() > 0.65 {
            return wall;
        }
        if height > crown - 1.0 && tower_u.abs() < 0.65 && tower_z.abs() < 0.65 {
            return BlockId::Crystal;
        }
    }
    // Dry planted aqueduct edges leave the entire walking band unobstructed.
    // Avoid water until its simulation can follow the same radial field.
    if !promenade && !court && (p.z.abs() - 4.5).abs() < 0.65 && height < 0.8 {
        return if district == 0 || district == 4 {
            BlockId::Leaves
        } else {
            trim
        };
    }
    if (p.z.abs() - 3.6).abs() < 0.5 && local.abs() < 0.7 && height < 0.8 {
        return BlockId::Crystal;
    }
    BlockId::Air
}
