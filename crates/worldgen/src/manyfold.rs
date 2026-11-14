//! Stage A: a finite, editable tube. Coordinates and the field use metres.
use crate::{BlockId, ColumnSample, WorldSampleConfig};
use glam::{IVec3, Vec3};

mod districts;
mod infinite;
mod dream;
mod expedition;
pub mod maze;
mod grand;
mod shell;
mod curve;
mod beacons;
mod bend;
mod inhabit;

#[cfg(test)]
mod dream_tests;
#[cfg(test)]
mod expedition_tests;
#[cfg(test)]
mod bend_tests;
#[cfg(test)]
mod maze_tests;
pub mod architecture;
pub mod undercroft;
pub mod weave;
pub mod strand;

#[cfg(test)]
mod tall_wall_tests;

pub const CENTER: Vec3 = Vec3::new(0.0, 36.0, 0.0);
pub const RADIUS: f32 = 24.0;
pub const OUTER_RADIUS: f32 = 32.0;
pub const HALF_LENGTH: f32 = 24.0;

/// White plaster maze walls, ten metres tall, around the straight tube.
/// Versions through 100 keep the 3.6 m district-coloured partitions.
pub const TALL_WHITE_WALLS: u32 = 101;

/// Unbounded white maze. Versions through 102 keep the finite tube.
pub const INFINITE_MAZE: u32 = 103;

/// Flat floor and ordinary gravity. The shell and the tube keep a radial field.
pub fn flat_maze(version: u32) -> bool {
    version == INFINITE_MAZE
}

pub use curve::{shell_curve, ShellCurve};

pub fn shortcut(cfg: &WorldSampleConfig) -> Option<(f32, f32)> {
    (cfg.generator_version >= districts::VERSION).then(|| districts::shortcut(cfg.seed)).flatten()
}

pub fn socket(cfg: &WorldSampleConfig) -> Option<Vec3> {
    if cfg.generator_version >= shell::VERSION {
        return Some(shell::socket(cfg.generator_version));
    }
    if cfg.generator_version >= INFINITE_MAZE {
        return None;
    }
    let (angle, z) = shortcut(cfg)?;
    let angle = angle - 2.0 / RADIUS;
    Some(CENTER + Vec3::new(angle.sin() * (RADIUS - 0.7), -angle.cos() * (RADIUS - 0.7), z))
}

/// Narrow opening in the selected wall. Samples the unchanged baseline so
/// machinery never replaces unrelated player construction with air.
pub fn shortcut_cells(cfg: &WorldSampleConfig) -> Vec<(IVec3, BlockId)> {
    if cfg.generator_version >= shell::VERSION {
        return shell::passage(cfg);
    }
    if cfg.generator_version >= INFINITE_MAZE {
        return Vec::new();
    }
    let Some((angle, z)) = shortcut(cfg) else { return Vec::new() };
    let mid = CENTER + Vec3::new(angle.sin() * (RADIUS - 1.8), -angle.cos() * (RADIUS - 1.8), z);
    let lo = ((mid - Vec3::splat(3.0)) / cfg.voxel_meters()).floor().as_ivec3();
    let hi = ((mid + Vec3::splat(3.0)) / cfg.voxel_meters()).ceil().as_ivec3();
    let mut cells = Vec::new();
    for y in lo.y..=hi.y { for x in lo.x..=hi.x { for cz in lo.z..=hi.z {
        let cell = IVec3::new(x, y, cz);
        let p = (cell.as_vec3() + Vec3::splat(0.5)) * cfg.voxel_meters() - CENTER;
        let distance = ((p.x.atan2(-p.y) - angle + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI).abs() * RADIUS;
        let height = RADIUS - p.truncate().length();
        if distance < 0.85 && (p.z - z).abs() < 1.9 && (0.0..3.65).contains(&height) {
            let expected = block(cfg, cell);
            if !expected.is_empty() && expected != BlockId::Bedrock { cells.push((cell, expected)); }
        }
    } } }
    cells
}

/// Immutable baseline, shared across docking ticks. Edits are still compared
/// against the current world at commit time; no edited cells enter this cache.
pub fn shortcut_cells_cached(cfg: &WorldSampleConfig) -> std::sync::Arc<Vec<(IVec3, BlockId)>> {
    thread_local! {
        static CACHE: std::cell::RefCell<Option<(WorldSampleConfig, std::sync::Arc<Vec<(IVec3, BlockId)>>)>> = const { std::cell::RefCell::new(None) };
    }
    CACHE.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.as_ref().is_none_or(|(key, _)| key != cfg) {
            *slot = Some((*cfg, std::sync::Arc::new(shortcut_cells(cfg))));
        }
        slot.as_ref().unwrap().1.clone()
    })
}

/// The environment, never a player's remembered route, selects this field.
/// Outside the admitted volume there is no force. At the axis it is zero.
pub fn acceleration(point_m: Vec3) -> Vec3 {
    let p = point_m - CENTER;
    let radial = Vec3::new(p.x, p.y, 0.0);
    if p.z.abs() > HALF_LENGTH + 3.0 || radial.length_squared() > OUTER_RADIUS.powi(2) {
        return Vec3::ZERO;
    }
    radial * (28.0 / RADIUS)
}

/// Local shell radius when the world is the swelling tube. Older fields stay put.
pub fn acceleration_for(version: u32, point_m: Vec3) -> Vec3 {
    if version >= shell::VERSION {
        shell::field(version, point_m)
    } else {
        acceleration(point_m)
    }
}

/// Axis height in metres. The stage-A tube stays at `CENTER`.
pub fn axis_y(version: u32) -> f32 {
    if version >= shell::VERSION {
        shell::center_of(version).y
    } else {
        CENTER.y
    }
}

/// Near and far shell crossings in one chunk column, in metres.
/// `(1.0, 0.0)` means that crossing is absent. The hollow between them is not included.
pub fn column_shell_bands(cfg: &WorldSampleConfig, cx: i32, cz: i32) -> [(f32, f32); 2] {
    if cfg.generator_version < shell::VERSION {
        let pad = 4.0;
        return [(CENTER.y - OUTER_RADIUS - pad, CENTER.y + OUTER_RADIUS + pad), (1.0, 0.0)];
    }
    let version = cfg.generator_version;
    let metres = cfg.voxel_meters();
    let x0 = cx as f32 * 32.0 * metres;
    let x1 = x0 + 32.0 * metres;
    let z0 = cz as f32 * 32.0 * metres;
    let z1 = z0 + 32.0 * metres;
    let axis = shell::center_of(version).y;
    let inward = if version >= maze::FIELD {
        14.0
    } else if version >= shell::GRAND {
        40.0
    } else if version >= shell::VAST {
        28.0
    } else {
        12.0
    };
    let outward = 6.0;
    let xs = [
        x0,
        x1,
        if x0 <= 0.0 && x1 >= 0.0 {
            0.0
        } else if x0.abs() <= x1.abs() {
            x0
        } else {
            x1
        },
    ];
    let mut lower: Option<(f32, f32)> = None;
    let mut upper: Option<(f32, f32)> = None;
    for z in [z0, (z0 + z1) * 0.5, z1] {
        let radius = shell::shell_radius(version, z);
        for x in xs {
            shell_crossing(x, radius, axis, inward, outward, &mut lower, &mut upper);
        }
    }
    match (lower, upper) {
        (None, None) => [(1.0, 0.0), (1.0, 0.0)],
        (Some(band), None) | (None, Some(band)) => [band, (1.0, 0.0)],
        (Some(lo), Some(hi)) if lo.1 >= hi.0 - 1.0 => [(lo.0.min(hi.0), lo.1.max(hi.1)), (1.0, 0.0)],
        (Some(lo), Some(hi)) => [lo, hi],
    }
}

fn shell_crossing(
    x: f32,
    radius: f32,
    axis: f32,
    inward: f32,
    outward: f32,
    lower: &mut Option<(f32, f32)>,
    upper: &mut Option<(f32, f32)>,
) {
    let x = x.abs();
    let outer = radius + outward;
    if x >= outer {
        return;
    }
    let outer_dy = (outer * outer - x * x).sqrt();
    let inner = (radius - inward).max(0.0);
    if x >= inner {
        include_band(lower, axis - outer_dy, axis);
        include_band(upper, axis, axis + outer_dy);
        return;
    }
    let inner_dy = (inner * inner - x * x).sqrt();
    include_band(lower, axis - outer_dy, axis - inner_dy);
    include_band(upper, axis + inner_dy, axis + outer_dy);
}

fn include_band(slot: &mut Option<(f32, f32)>, a: f32, b: f32) {
    let (a, b) = (a.min(b), a.max(b));
    *slot = Some(match *slot {
        Some((lo, hi)) => (lo.min(a), hi.max(b)),
        None => (a, b),
    });
}

/// Inclusive vertical span of the shell, in metres. The stage-A tube stays small.
pub fn vertical_extent(version: u32) -> (f32, f32) {
    if version >= shell::VERSION {
        let y = shell::center_of(version).y;
        let reach = shell::outer_reach(version);
        (y - reach, y + reach)
    } else {
        (CENTER.y - OUTER_RADIUS, CENTER.y + OUTER_RADIUS)
    }
}

pub fn contains_weight(version: u32, position: Vec3) -> bool {
    if version >= shell::VERSION {
        let delta = position - shell::center_of(version);
        let radial = Vec3::new(delta.x, delta.y, 0.0).length();
        return radial.is_finite() && radial < shell::shell_radius(version, delta.z) + 6.0;
    }
    let delta = position - CENTER;
    delta.truncate().length() <= OUTER_RADIUS + 4.0 && delta.z.abs() <= HALF_LENGTH + 4.0
}

/// Where a new expedition leaves the brass weight. Older shells keep the saved cradle.
pub fn cradle(cfg: &WorldSampleConfig) -> Option<Vec3> {
    (cfg.generator_version >= expedition::VERSION).then(|| expedition::cradle(cfg))
}

pub fn spawn(cfg: &WorldSampleConfig) -> Vec3 {
    if cfg.generator_version >= shell::VERSION {
        return shell::spawn(cfg);
    }
    if cfg.generator_version >= INFINITE_MAZE {
        return infinite::spawn(cfg);
    }
    if cfg.generator_version >= strand::VERSION {
        return strand::spawn(cfg);
    }
    (CENTER + Vec3::new(0.0, -RADIUS + 2.0, 0.0)) / cfg.voxel_meters()
}

/// Bay posts near the player, in metres. The site id is the sector and row.
pub fn encounters(cfg: &WorldSampleConfig, player: Vec3) -> Vec<(IVec3, Vec3, u32)> {
    if cfg.generator_version >= maze::TWISTED {
        return inhabit::life(cfg, player);
    }
    let mut out = beacons::encounters(cfg, player);
    if cfg.generator_version >= maze::CLOSE {
        out.extend(inhabit::life(cfg, player));
    }
    out
}

/// Fixed candidate fixtures; the caller checks the edited world before lighting.
pub fn guide_lights(cfg: &WorldSampleConfig) -> Vec<IVec3> {
    if cfg.generator_version >= shell::VERSION || cfg.generator_version >= INFINITE_MAZE {
        return Vec::new();
    }
    if cfg.generator_version >= strand::VERSION {
        return strand::guide_lights(cfg);
    }
    if cfg.generator_version >= weave::VERSION {
        let mut cells = districts::guide_lights(cfg);
        cells.retain(|&cell| block(cfg, cell) == BlockId::Crystal);
        cells.extend((0..weave::DISTRICTS).map(|d| weave::anchor(cfg, d)));
        if cfg.generator_version >= undercroft::VERSION {
            cells.push(undercroft::heart(cfg));
        }
        if cfg.generator_version >= architecture::VERSION {
            cells.extend(architecture::lights(cfg));
            cells.sort_by_key(|p| (p.x, p.y, p.z));
            cells.dedup();
        }
        return cells;
    }
    if cfg.generator_version >= districts::VERSION {
        return districts::guide_lights(cfg);
    }
    let mut cells = Vec::new();
    for i in -9..=9 {
        let angle = (i as f32 * 8.0 + 0.35) / RADIUS;
        for z in [-3.5, 3.5] {
            let point = CENTER + Vec3::new(angle.sin() * (RADIUS - 0.5), -angle.cos() * (RADIUS - 0.5), z);
            let base = (point / cfg.voxel_meters()).floor().as_ivec3();
            // Choose one actual crystal cell per fixture at the selected scale.
            let mut found = None;
            'search: for dz in -1..=1 { for dy in -1..=1 { for dx in -1..=1 {
                let p = base + IVec3::new(dx,dy,dz);
                if block(cfg,p) == BlockId::Crystal { found = Some(p); break 'search; }
            } } }
            if let Some(p) = found { cells.push(p); }
        }
    }
    cells
}

pub fn column(cfg: &WorldSampleConfig, x: i32, z: i32) -> ColumnSample {
    if cfg.generator_version >= shell::VERSION {
        return shell::column(cfg, x, z);
    }
    if cfg.generator_version >= INFINITE_MAZE {
        return infinite::column(cfg, x, z);
    }
    if cfg.generator_version >= strand::VERSION {
        return strand::column(cfg, x, z);
    }
    let height = cfg.blocks_for_height(CENTER.y + OUTER_RADIUS);
    ColumnSample { deposit_m: 0.0, deposit_bed_surface: crate::BlockId::Stone, height, surface: BlockId::Stone, filler: BlockId::Stone,
        vegetation_density: 0.0, cave: Default::default() }
}

pub fn block(cfg: &WorldSampleConfig, cell: IVec3) -> BlockId {
    if cfg.generator_version >= shell::VERSION {
        let placed = shell::block(cfg, cell);
        if placed == BlockId::Air {
            if let Some(mark) = beacons::mark(cfg, cell) {
                return mark;
            }
        }
        return placed;
    }
    if cfg.generator_version >= INFINITE_MAZE {
        return infinite::block(cfg, cell);
    }
    if cfg.generator_version >= strand::VERSION {
        return strand::block(cfg, cell);
    }
    if cfg.generator_version >= undercroft::VERSION {
        return undercroft::block(cfg, cell);
    }
    if cfg.generator_version >= architecture::VERSION {
        return architecture::block(cfg, cell);
    }
    if cfg.generator_version >= weave::VERSION {
        return weave::block(cfg, cell);
    }
    if cfg.generator_version >= districts::VERSION {
        return districts::block(cfg, cell);
    }
    let p = (cell.as_vec3() + Vec3::splat(0.5)) * cfg.voxel_meters() - CENTER;
    let r = p.truncate().length();
    if r > OUTER_RADIUS || p.z.abs() > HALF_LENGTH + 2.0 { return BlockId::Air; }
    // Closed ends and an outer safety shell bound the supported gravity domain.
    if p.z.abs() >= HALF_LENGTH || r >= OUTER_RADIUS - 2.0 { return BlockId::Bedrock; }
    let angle = p.x.atan2(-p.y);
    let arc = angle * RADIUS;
    let ring = p.z.abs() < 3.0;
    if r >= RADIUS {
        if r > RADIUS + 2.0 { return BlockId::Air; }
        return if ring { BlockId::Sandstone }
            else if p.y < 0.0 && r < RADIUS + 0.75 { BlockId::Grass }
            else if p.y > 0.0 { BlockId::StoneBricks }
            else { BlockId::Stone };
    }
    let height = RADIUS - r;
    // A broad uninterrupted circumference and two axial aisles are reserved
    // before the low maze partitions. No angular seam closes either route.
    let aisle = arc.abs() < 3.0 || (arc.abs() - std::f32::consts::PI * RADIUS).abs() < 3.0;
    if !ring && !aisle && height < 1.4 {
        let bay = (arc / 8.0).floor() as i32;
        let cross_wall = arc.rem_euclid(8.0) < 0.8
            && (p.z + if bay.rem_euclid(2) == 0 { 5.0 } else { -5.0 }).abs() > 2.0;
        let long_wall = (p.z.abs() - 12.0).abs() < 0.45 && arc.rem_euclid(16.0) > 4.0;
        if cross_wall || long_wall { return BlockId::StoneBricks; }
    }
    // Plant beds beside, not on, the wrapping sandstone route.
    if (p.z.abs() - 4.5).abs() < 0.65 && arc.rem_euclid(7.0) < 2.0 && height < 0.9 {
        return BlockId::Leaves;
    }
    // Warm luminous guide stones repeat around the entire circumference.
    if (p.z.abs() - 3.5).abs() < 0.6 && arc.rem_euclid(8.0) < 1.2 && height < 1.3 {
        return BlockId::Crystal;
    }
    // Entrance belfry: four piers and an open crown, upright toward the hollow.
    let entrance = Vec3::new(p.x + 6.0, p.y + RADIUS, p.z + 9.0);
    if entrance.x.abs() < 2.0 && entrance.z.abs() < 2.0 && (0.0..8.0).contains(&entrance.y) {
        if (entrance.x.abs() > 1.1 && entrance.z.abs() > 1.1) || entrance.y > 6.4 {
            return if entrance.y > 7.2 { BlockId::Sandstone } else { BlockId::StoneBricks };
        }
    }
    // Opposing workshop: inward-facing pillars, copper-coloured roof and bench.
    let shop = Vec3::new(p.x - 6.0, RADIUS - p.y, p.z - 9.0);
    if shop.x.abs() < 4.0 && shop.z.abs() < 3.0 && (0.0..5.5).contains(&shop.y) {
        if shop.y > 4.6 { return BlockId::Sandstone; }
        if shop.x.abs() > 3.1 && shop.z.abs() > 2.1 { return BlockId::Log; }
        if shop.y < 1.1 && shop.x.abs() < 2.0 && (shop.z - 1.5).abs() < 0.6 { return BlockId::Planks; }
        if shop.y > 4.0 && shop.x.abs() < 0.5 && shop.z.abs() < 0.5 { return BlockId::Crystal; }
    }
    BlockId::Air
}
