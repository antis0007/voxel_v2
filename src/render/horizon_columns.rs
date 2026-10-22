//! Voxel-scale distant terrain. Outside the fine smooth grid (`horizon_fine`,
//! the inner 3 km square) and inside the outer square, the smooth backdrop is
//! cut away and replaced by columns on a 64 m grid. Columns sit on the world
//! grid and every corner is a pure function of world position, so recentring
//! never moves them.
//!
//! The grid is a corner grid. Each corner is sampled once and shared by the four
//! columns around it, so neighbouring tops meet exactly. A top is two triangles
//! through its four corner heights, so ridges run continuously across columns
//! instead of stepping once per column. Walls appear only where the columns meet
//! the smooth lattice outside the square, in whichever direction the two
//! surfaces disagree. Materials are sampled per corner as well, so the shader
//! blends rock, soil, grass and snow across each triangle, as it does on the
//! smooth backdrop, rather than giving each column one flat block texture.
use super::edits::{append, clip};
use crate::mesh::{ChunkMesh, Vertex3};
use crate::world::{WorldSampleConfig, distant_surface};
use glam::{IVec3, Vec3};
use std::sync::atomic::{AtomicBool, Ordering};

/// Nested square bands as (half width in metres, column size in metres). The
/// first band lies under the fine grid and builds nothing. Beyond it, one 64 m
/// ring keeps the mesh inside its budget: the 32 m ring cost about as much as
/// the whole fine grid.
const BANDS: [(f32, f32); 2] = [(1536.0, 16.0), (6144.0, 64.0)];
/// Edge sub-segment length in metres. Every column size is a multiple of it.
const SEGMENT_M: f32 = 16.0;
/// Per-column tilt for walls. Lattice-facing walls at the square edge would
/// otherwise share one normal, and a long coast of identical walls reads as one
/// band. The tilt changes the lighting itself, which the shader keeps.
const WALL_TILT: f32 = 0.15;
/// Backdrop `ao_strength` tag for column vertices. Still inside the shader's
/// backdrop range (-1.5, 0); `terrain_is_column` in world_shader.wgsl keys a
/// stepped per-column value on it, which is what breaks a uniform coast.
/// Every other backdrop mesh uses -1.0.
const COLUMN_AO: f32 = -1.25;

/// Terrain sampled at one corner of the column grid.
#[derive(Clone, Copy)]
struct Corner {
    /// Surface height in voxel units.
    height: f32,
    /// Smooth normal from the corner grid.
    normal: Vec3,
    color: [f32; 3],
    /// Backdrop surface detail, as `block.surface()` gives it.
    surface: [f32; 4],
}

#[derive(Clone, Copy)]
struct Column {
    /// Column index in world columns of its band.
    index: (i32, i32),
    /// Lower-left corner in metres (minimum x and z).
    origin: (f32, f32),
    /// Corners in SW, NW, NE, SE order (x0z0, x0z1, x1z1, x1z0). The top is two
    /// triangles split along the SW-NE diagonal.
    corners: [Corner; 4],
}

impl Column {
    /// Surface height in voxel units at a metre position inside the column.
    /// Uses the same two triangles as the top, so a probe reads the drawn
    /// surface exactly. Its only caller, `rest_on_surface`, awaits a tree hook.
    #[cfg_attr(not(test), allow(dead_code))]
    fn height(&self, column_m: f32, x: f32, z: f32) -> f32 {
        let u = ((x - self.origin.0) / column_m).clamp(0.0, 1.0);
        let v = ((z - self.origin.1) / column_m).clamp(0.0, 1.0);
        let [sw, nw, ne, se] = self.corners.map(|c| c.height);
        if v >= u {
            (1.0 - v) * sw + (v - u) * nw + u * ne
        } else {
            (1.0 - u) * sw + v * ne + (u - v) * se
        }
    }

    /// Top height along one face, `along` metres from the face's start corner.
    fn edge_height(&self, column_m: f32, side: Side, along: f32) -> f32 {
        let (lo, hi) = side.edge_corners();
        let start = match side {
            Side::West | Side::East => self.origin.1,
            Side::North | Side::South => self.origin.0,
        };
        let t = ((along - start) / column_m).clamp(0.0, 1.0);
        (1.0 - t) * self.corners[lo].height + t * self.corners[hi].height
    }
}

#[derive(Clone, Copy)]
enum Side {
    West,
    East,
    North,
    South,
}

impl Side {
    const ALL: [Side; 4] = [Side::West, Side::East, Side::North, Side::South];

    /// Outward unit normal of this face.
    fn outward(self) -> Vec3 {
        match self {
            Side::West => -Vec3::X,
            Side::East => Vec3::X,
            Side::North => -Vec3::Z,
            Side::South => Vec3::Z,
        }
    }

    /// Corner indices at the start and end of this face, in along-face order.
    fn edge_corners(self) -> (usize, usize) {
        match self {
            Side::West => (0, 1),
            Side::East => (3, 2),
            Side::North => (0, 3),
            Side::South => (1, 2),
        }
    }

    /// Metre position of a probe just beyond this face at `along` metres, or
    /// the face's own line when `beyond` is false.
    fn point(self, column: (f32, f32, f32, f32), along: f32, beyond: bool) -> (f32, f32) {
        let (x0, x1, z0, z1) = column;
        let offset = if beyond { SEGMENT_M * 0.5 } else { 0.0 };
        match self {
            Side::West => (x0 - offset, along),
            Side::East => (x1 + offset, along),
            Side::North => (along, z0 - offset),
            Side::South => (along, z1 + offset),
        }
    }
}

struct Band {
    column_m: f32,
    /// Smallest column index of the square, in world columns of `column_m`.
    min: (i32, i32),
    side: i32,
    /// Whether this band builds each column of the square (`side` per row).
    owned: Vec<bool>,
    /// Corner grid, `side + 1` per row. Empty when the band builds nothing.
    corners: Vec<Corner>,
}

impl Band {
    /// Whether this band builds column `(kx, kz)`.
    fn owns(&self, kx: i32, kz: i32) -> bool {
        let (ox, oz) = (kx - self.min.0, kz - self.min.1);
        ox >= 0 && oz >= 0 && ox < self.side && oz < self.side
            && self.owned[(oz * self.side + ox) as usize]
    }

    fn column(&self, kx: i32, kz: i32) -> Option<Column> {
        if !self.owns(kx, kz) {
            return None;
        }
        let (ox, oz) = (kx - self.min.0, kz - self.min.1);
        let row = self.side + 1;
        let at = |dx: i32, dz: i32| {
            let mut corner = self.corners[((oz + dz) * row + ox + dx) as usize];
            corner.color = column_tint(corner.color, kx, kz);
            corner
        };
        Some(Column {
            index: (kx, kz),
            origin: (kx as f32 * self.column_m, kz as f32 * self.column_m),
            corners: [at(0, 0), at(0, 1), at(1, 1), at(1, 0)],
        })
    }
}

/// Band that owns a point: the innermost square containing it. Columns are
/// aligned to every band boundary, so a column's centre decides its owner.
fn owner(center: (f32, f32), x: f32, z: f32) -> Option<usize> {
    BANDS.iter().position(|&(half, _)| {
        (x - center.0).abs() < half && (z - center.1).abs() < half
    })
}

/// Samples the terrain at one corner of the column grid. The corner's voxel is
/// the one that starts at that corner, so every column that shares it reads the
/// same height and material.
fn sample_corner(cfg: &WorldSampleConfig, column_m: f32, kx: i32, kz: i32) -> Corner {
    let scale = cfg.voxel_meters();
    let vx = (kx as f32 * column_m / scale).floor() as i32;
    let vz = (kz as f32 * column_m / scale).floor() as i32;
    let (raw, block) = distant_surface(cfg, vx, vz);
    let material = block.surface();
    Corner {
        height: raw,
        normal: Vec3::Y,
        color: block.color(IVec3::new(vx, raw as i32, vz)),
        surface: [
            material.roughness,
            material.specular,
            0.0,
            material.shader_key(),
        ],
    }
}

/// Normals of the corner grid from central differences, in voxel units. Grid
/// edges use one-sided differences. `row` is the corners per row, at least two.
fn corner_normals(corners: &mut [Corner], row: usize, cell_v: f32) {
    let heights: Vec<f32> = corners.iter().map(|c| c.height).collect();
    let last = row - 1;
    let normals: Vec<Vec3> = (0..heights.len())
        .map(|i| {
            let (x, z) = (i % row, i / row);
            let (xa, xb) = (x.saturating_sub(1), (x + 1).min(last));
            let (za, zb) = (z.saturating_sub(1), (z + 1).min(last));
            let h = |x: usize, z: usize| heights[z * row + x];
            let dx = (h(xb, z) - h(xa, z)) / ((xb - xa) as f32 * cell_v);
            let dz = (h(x, zb) - h(x, za)) / ((zb - za) as f32 * cell_v);
            Vec3::new(-dx, 1.0, -dz).normalize()
        })
        .collect();
    for (corner, normal) in corners.iter_mut().zip(normals) {
        corner.normal = normal;
    }
}

/// Deterministic value in [-1, 1] for a column. `salt` picks an independent
/// value per use, so each wall's tilt is independent of the others.
fn column_jitter(kx: i32, kz: i32, salt: u32) -> f32 {
    let mut h = (kx as u32).wrapping_mul(0x9E37_79B1)
        ^ (kz as u32).wrapping_mul(0x85EB_CA77)
        ^ salt.wrapping_mul(0xC2B2_AE3D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    (h & 0xFFFF) as f32 / 32767.5 - 1.0
}

/// Per-column brightness jitter, a fixed function of the column index. The
/// spread is small, so the palette stays readable. Only materials that keep
/// vertex colour (block families) show it.
fn column_tint(color: [f32; 3], kx: i32, kz: i32) -> [f32; 3] {
    let k = 1.0 + 0.15 * column_jitter(kx, kz, 0);
    color.map(|c| (c * k).clamp(0.0, 1.0))
}

/// Wall normal for one face of column `(kx, kz)`: `sign` times the outward
/// direction, tilted along the face. See `WALL_TILT`.
fn wall_normal(side: Side, kx: i32, kz: i32, sign: f32) -> Vec3 {
    let (salt, tangent) = match side {
        Side::West => (3, Vec3::Z),
        Side::East => (4, Vec3::Z),
        Side::North => (5, Vec3::X),
        Side::South => (6, Vec3::X),
    };
    (sign * side.outward() + tangent * (column_jitter(kx, kz, salt) * WALL_TILT)).normalize()
}

/// Whether a metre position lies in a column that the bands build.
fn owned_at(bands: &[Band], center: (f32, f32), x: f32, z: f32) -> bool {
    owner(center, x, z).is_some_and(|index| {
        let band = &bands[index];
        let kx = (x / band.column_m).floor() as i32;
        let kz = (z / band.column_m).floor() as i32;
        band.owns(kx, kz)
    })
}

/// Smooth source surface height in voxel units at a metre position.
fn lattice_height(cfg: &WorldSampleConfig, x: f32, z: f32) -> f32 {
    let scale = cfg.voxel_meters();
    distant_surface(cfg, (x / scale).floor() as i32, (z / scale).floor() as i32).0
}

/// Draws one quad. Corners run around it. The winding flips when the quad faces
/// away from its average normal, so every face matches the lighting normal the
/// shader receives.
fn push_quad(
    mesh: &mut ChunkMesh,
    corners: [Vec3; 4],
    normals: [Vec3; 4],
    colors: [[f32; 3]; 4],
    surfaces: [[f32; 4]; 4],
) {
    let face = normals.iter().fold(Vec3::ZERO, |sum, &n| sum + n);
    let order = if (corners[1] - corners[0])
        .cross(corners[2] - corners[0])
        .dot(face)
        >= 0.0
    {
        [0, 1, 2, 3]
    } else {
        [0, 3, 2, 1]
    };
    let base = mesh.vertices.len() as u32;
    for index in order {
        mesh.vertices.push(Vertex3 {
            pos: corners[index].to_array(),
            color: colors[index],
            normal: normals[index].to_array(),
            surface: surfaces[index],
            ao_strength: COLUMN_AO,
        });
    }
    mesh.indices
        .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
}

/// Draws one column's top through its four corner heights.
fn push_top(mesh: &mut ChunkMesh, column: &Column, column_m: f32, scale: f32) {
    let (x0, z0) = column.origin;
    let (x1, z1) = (x0 + column_m, z0 + column_m);
    let points = [(x0, z0), (x0, z1), (x1, z1), (x1, z0)];
    let corners = std::array::from_fn(|i| {
        Vec3::new(points[i].0 / scale, column.corners[i].height, points[i].1 / scale)
    });
    push_quad(
        mesh,
        corners,
        column.corners.map(|c| c.normal),
        column.corners.map(|c| c.color),
        column.corners.map(|c| c.surface),
    );
}

/// Builds one band: which columns it owns, and the sampled corner grid under
/// them. The square inside `inner_m` belongs to the fine grid and is skipped.
#[allow(clippy::too_many_arguments)]
fn fill_band(
    cfg: &WorldSampleConfig,
    center: (f32, f32),
    inner_m: f32,
    band: usize,
    column_m: f32,
    min: (i32, i32),
    side: i32,
    cancel: &AtomicBool,
) -> Option<Band> {
    let mut owned = vec![false; (side.max(0) * side.max(0)) as usize];
    for oz in 0..side {
        for ox in 0..side {
            let (kx, kz) = (min.0 + ox, min.1 + oz);
            let x = (kx as f32 + 0.5) * column_m;
            let z = (kz as f32 + 0.5) * column_m;
            let inside_inner = (x - center.0).abs() < inner_m && (z - center.1).abs() < inner_m;
            owned[(oz * side + ox) as usize] = !inside_inner && owner(center, x, z) == Some(band);
        }
    }
    if !owned.iter().any(|&built| built) {
        return Some(Band {
            column_m,
            min,
            side,
            owned,
            corners: Vec::new(),
        });
    }

    // Corners are visited one erosion tile (128 voxels) at a time. Row-major
    // order touched about 97 tiles per row, more than the tile caches hold, so
    // each tile was rebuilt about four times.
    let row = (side + 1) as usize;
    let tile = ((128.0 * cfg.voxel_meters() / column_m) as usize).max(1);
    let mut points = Vec::with_capacity(row * row);
    for tile_z in (0..row).step_by(tile) {
        for tile_x in (0..row).step_by(tile) {
            for z in tile_z..(tile_z + tile).min(row) {
                for x in tile_x..(tile_x + tile).min(row) {
                    points.push((z * row + x, min.0 + x as i32, min.1 + z as i32));
                }
            }
        }
    }
    let workers = super::parallel::workers(8);
    let sampled = super::parallel::map_blocks(&points, 256, workers, cancel, |&(_, kx, kz)| {
        Some(sample_corner(cfg, column_m, kx, kz))
    })?;
    let blank = Corner {
        height: 0.0,
        normal: Vec3::Y,
        color: [0.0; 3],
        surface: [0.0; 4],
    };
    let mut corners = vec![blank; row * row];
    for (&(slot, _, _), corner) in points.iter().zip(sampled) {
        corners[slot] = corner;
    }
    corner_normals(&mut corners, row, column_m / cfg.voxel_meters());
    Some(Band {
        column_m,
        min,
        side,
        owned,
        corners,
    })
}

/// Builds every band of the square around `center`, innermost first.
fn fill_bands(
    cfg: &WorldSampleConfig,
    center: (f32, f32),
    inner_m: f32,
    cancel: &AtomicBool,
) -> Option<Vec<Band>> {
    let mut bands = Vec::with_capacity(BANDS.len());
    for (band, &(half, column_m)) in BANDS.iter().enumerate() {
        let min = (
            ((center.0 - half) / column_m).floor() as i32,
            ((center.1 - half) / column_m).floor() as i32,
        );
        let side = (2.0 * half / column_m).round() as i32;
        bands.push(fill_band(cfg, center, inner_m, band, column_m, min, side, cancel)?);
    }
    Some(bands)
}

/// Cut every smooth backdrop triangle that lies inside the outer square.
/// Pieces outside the square are kept. The same four-slab decomposition
/// as the reference patch, so no triangle is emitted twice.
fn cut_square(mesh: &mut ChunkMesh, lo: [f32; 2], hi: [f32; 2], cancel: &AtomicBool) -> Option<()> {
    let indices = std::mem::take(&mut mesh.indices);
    for (n, tri) in indices.chunks_exact(3).enumerate() {
        if n % 4096 == 0 && cancel.load(Ordering::Relaxed) {
            return None;
        }
        let v = [tri[0], tri[1], tri[2]].map(|i| mesh.vertices[i as usize]);
        if v.iter().all(|v| v.pos[0] < lo[0])
            || v.iter().all(|v| v.pos[0] > hi[0])
            || v.iter().all(|v| v.pos[2] < lo[1])
            || v.iter().all(|v| v.pos[2] > hi[1])
        {
            mesh.indices.extend_from_slice(tri);
            continue;
        }
        let mut inside = v.to_vec();
        for (axis, bound, less) in [
            (0, lo[0], true),
            (0, hi[0], false),
            (2, lo[1], true),
            (2, hi[1], false),
        ] {
            append(mesh, &clip(&inside, axis, bound, less));
            inside = clip(&inside, axis, bound, !less);
        }
    }
    Some(())
}

/// The cube band is opt-in (`VOXEL_HORIZON_CUBES=1`). By default the smooth
/// backdrop stays. `VOXEL_HORIZON_SMOOTH=1` also forces the smooth path.
fn enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var_os("VOXEL_HORIZON_CUBES").is_some()
            && std::env::var_os("VOXEL_HORIZON_SMOOTH").is_none()
    })
}

/// Replace the smooth backdrop between `inner_m` and the outer square with
/// columns. The square inside `inner_m` belongs to the fine smooth grid, so no
/// column is built there. Runs on the base lattice before water and vegetation
/// are appended.
pub(super) fn apply(
    mesh: &mut ChunkMesh,
    cfg: &WorldSampleConfig,
    center: (i32, i32),
    cancel: &AtomicBool,
    inner_m: f32,
) -> Option<()> {
    if !enabled() {
        return Some(());
    }
    let scale = cfg.voxel_meters();
    if !scale.is_finite() || scale <= 0.0 {
        return None;
    }
    let center = (center.0 as f32, center.1 as f32);
    let fill_started = std::time::Instant::now();
    let bands = fill_bands(cfg, center, inner_m, cancel)?;

    let (outer, _) = BANDS[BANDS.len() - 1];
    let cut_started = std::time::Instant::now();
    cut_square(
        mesh,
        [(center.0 - outer) / scale, (center.1 - outer) / scale],
        [(center.0 + outer) / scale, (center.1 + outer) / scale],
        cancel,
    )?;

    // Stage timings for the column build: fill, cut, then walls.
    println!("[horizon-columns-timing] fill_ms={:.0} cut_ms={:.0}", fill_started.elapsed().as_secs_f64() * 1000.0, cut_started.elapsed().as_secs_f64() * 1000.0);
    let walls_started = std::time::Instant::now();
    let (count, walls) = emit_columns(mesh, cfg, &bands, center, cancel)?;
    println!("[horizon-columns-timing] walls_ms={:.0}", walls_started.elapsed().as_secs_f64() * 1000.0);
    println!(
        "[horizon-columns] bands={:?} columns={:?} wall_segments={walls} outer_m={outer} triangles={}",
        BANDS,
        count,
        mesh.indices.len() / 3,
    );
    Some(())
}

/// Draws the top and walls of every column the bands own. Walls look at the
/// neighbouring bands too, so every band is passed together. Returns the column
/// count per band and the wall segment count.
fn emit_columns(
    mesh: &mut ChunkMesh,
    cfg: &WorldSampleConfig,
    bands: &[Band],
    center: (f32, f32),
    cancel: &AtomicBool,
) -> Option<([usize; BANDS.len()], usize)> {
    let scale = cfg.voxel_meters();
    let mut count = [0usize; BANDS.len()];
    let mut walls = 0usize;
    for (band_index, band) in bands.iter().enumerate() {
        let segments = (band.column_m / SEGMENT_M).round() as i32;
        for kz in band.min.1..band.min.1 + band.side {
            if cancel.load(Ordering::Relaxed) {
                return None;
            }
            for kx in band.min.0..band.min.0 + band.side {
                let Some(column) = band.column(kx, kz) else {
                    continue;
                };
                count[band_index] += 1;
                push_top(mesh, &column, band.column_m, scale);
                for side in Side::ALL {
                    walls += emit_walls(
                        mesh, cfg, bands, center, side, &column, band.column_m, segments,
                    );
                }
            }
        }
    }
    Some((count, walls))
}

/// Walls along one face, one per segment. Only lattice neighbours need one: an
/// owned neighbour shares this column's edge exactly. Where the lattice stands
/// higher, the wall rises from the column top to it. Where the column stands
/// higher, the wall drops from the top to the lattice. Both directions matter:
/// a column above lower smooth ground would otherwise have an open side.
#[allow(clippy::too_many_arguments)]
fn emit_walls(
    mesh: &mut ChunkMesh,
    cfg: &WorldSampleConfig,
    bands: &[Band],
    center: (f32, f32),
    side: Side,
    column: &Column,
    column_m: f32,
    segments: i32,
) -> usize {
    let scale = cfg.voxel_meters();
    let (ox, oz) = column.origin;
    let bounds = (ox, ox + column_m, oz, oz + column_m);
    let (start, fixed) = match side {
        Side::West => (oz, ox),
        Side::East => (oz, ox + column_m),
        Side::North => (ox, oz),
        Side::South => (ox, oz + column_m),
    };
    let segment_m = column_m / segments as f32;
    let (lo, hi) = side.edge_corners();
    let mut emitted = 0;
    for segment in 0..segments {
        let a = start + segment as f32 * segment_m;
        let b = a + segment_m;
        let (probe_x, probe_z) = side.point(bounds, (a + b) * 0.5, true);
        if owned_at(bands, center, probe_x, probe_z) {
            continue;
        }
        let neighbour = lattice_height(cfg, probe_x, probe_z);
        let top_a = column.edge_height(column_m, side, a);
        let top_b = column.edge_height(column_m, side, b);
        let (low_a, high_a) = (top_a.min(neighbour), top_a.max(neighbour));
        let (low_b, high_b) = (top_b.min(neighbour), top_b.max(neighbour));
        if high_a - low_a + high_b - low_b <= 0.0 {
            continue;
        }
        let outward = (top_a + top_b) * 0.5 > neighbour;
        let normal = wall_normal(side, column.index.0, column.index.1, if outward { 1.0 } else { -1.0 });
        let point = |along: f32, height: f32| -> Vec3 {
            let along = along / scale;
            let fixed = fixed / scale;
            match side {
                Side::West | Side::East => Vec3::new(fixed, height, along),
                Side::North | Side::South => Vec3::new(along, height, fixed),
            }
        };
        // A wall end takes the material of the column corner nearest to it.
        let nearest = |along: f32| -> Corner {
            if along - start < column_m * 0.5 {
                column.corners[lo]
            } else {
                column.corners[hi]
            }
        };
        let (ca, cb) = (nearest(a), nearest(b));
        push_quad(
            mesh,
            [
                point(a, low_a),
                point(b, low_b),
                point(b, high_b),
                point(a, high_a),
            ],
            [normal; 4],
            [ca.color, cb.color, cb.color, ca.color],
            [ca.surface, cb.surface, cb.surface, ca.surface],
        );
        emitted += 1;
    }
    emitted
}

/// Stand a tree on the column surface that covers it, not the smooth surface.
/// `base` is in metres, and so is the returned point.
pub(super) fn rest_on_surface(cfg: &WorldSampleConfig, center: (i32, i32), base: Vec3) -> Vec3 {
    if !enabled() {
        return base;
    }
    let center = (center.0 as f32, center.1 as f32);
    let Some(index) = owner(center, base.x, base.z) else {
        return base;
    };
    let (_, column_m) = BANDS[index];
    let kx = (base.x / column_m).floor() as i32;
    let kz = (base.z / column_m).floor() as i32;
    let corners = [(0, 0), (0, 1), (1, 1), (1, 0)]
        .map(|(dx, dz)| sample_corner(cfg, column_m, kx + dx, kz + dz));
    let column = Column {
        index: (kx, kz),
        origin: (kx as f32 * column_m, kz as f32 * column_m),
        corners,
    };
    Vec3::new(base.x, column.height(column_m, base.x, base.z) * cfg.voxel_meters(), base.z)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn corner_at(height: f32) -> Corner {
        Corner {
            height,
            normal: Vec3::Y,
            color: [0.5; 3],
            surface: [0.0; 4],
        }
    }

    /// A column whose corner heights come from the plane `a*x + b*z + c`.
    fn plane_column(origin: (f32, f32), column_m: f32, a: f32, b: f32, c: f32) -> Column {
        let at = |x: f32, z: f32| corner_at(a * x + b * z + c);
        let (x0, z0) = origin;
        let (x1, z1) = (x0 + column_m, z0 + column_m);
        Column {
            index: (0, 0),
            origin,
            corners: [at(x0, z0), at(x0, z1), at(x1, z1), at(x1, z0)],
        }
    }

    /// Band edges must sit on their own column grid, and column sizes must grow
    /// outward. Snapped centres are multiples of CENTER_GRID, so that grid must
    /// also be a whole number of the largest column.
    #[test]
    fn bands_nest_outward_and_align_to_their_columns() {
        let mut previous = (0.0_f32, 0.0_f32);
        for &(half, column_m) in &BANDS {
            assert!(half > previous.0, "bands must nest outward");
            assert!(column_m > previous.1, "column size must grow with distance");
            assert_eq!(half % column_m, 0.0, "band edge must fall on a column boundary");
            assert_eq!(column_m % SEGMENT_M, 0.0, "columns must split into whole wall segments");
            assert_eq!(
                super::super::CENTER_GRID as f32 % column_m,
                0.0,
                "snapped centres must keep every band aligned"
            );
            previous = (half, column_m);
        }
    }

    /// The top must reproduce a linear surface exactly. Both triangles are
    /// linear, so any error here would be a bug in the barycentric weights.
    #[test]
    fn column_top_reproduces_a_plane_exactly() {
        let column = plane_column((128.0, -64.0), 64.0, 0.3, -0.7, 40.0);
        for i in 0..=20 {
            for j in 0..=20 {
                let x = 128.0 + i as f32 * 3.2;
                let z = -64.0 + j as f32 * 3.2;
                let want = 0.3 * x - 0.7 * z + 40.0;
                assert!((column.height(64.0, x, z) - want).abs() < 1e-3, "at ({x}, {z})");
            }
        }
    }

    /// Each face must read the corner heights along its own two corners, which
    /// the neighbour's matching face also uses. That is what keeps the seam closed.
    #[test]
    fn column_edges_read_their_own_corners() {
        let column = plane_column((0.0, 0.0), 64.0, 0.2, 0.5, 10.0);
        for t in [0.0, 16.0, 40.0, 64.0] {
            assert!((column.edge_height(64.0, Side::West, t) - (0.5 * t + 10.0)).abs() < 1e-4);
            assert!((column.edge_height(64.0, Side::East, t) - (0.2 * 64.0 + 0.5 * t + 10.0)).abs() < 1e-4);
            assert!((column.edge_height(64.0, Side::North, t) - (0.2 * t + 10.0)).abs() < 1e-4);
            assert!((column.edge_height(64.0, Side::South, t) - (0.2 * t + 0.5 * 64.0 + 10.0)).abs() < 1e-4);
        }
    }

    /// Heights rise along x and stay flat along z, so the normal tilts toward -x.
    #[test]
    fn corner_normals_follow_the_slope() {
        let row = 5;
        let mut corners: Vec<Corner> = (0..row * row)
            .map(|i| corner_at(0.5 * (i % row) as f32))
            .collect();
        corner_normals(&mut corners, row, 1.0);
        let normal = corners[2 * row + 2].normal;
        let want = Vec3::new(-0.5, 1.0, 0.0).normalize();
        assert!(normal.distance(want) < 1e-5, "{normal:?}");
    }

    /// Wall tilts must stay close to their face so push_quad's winding test
    /// keeps the same sign, and walls must not all share one normal.
    #[test]
    fn wall_normals_tilt_within_bounds_and_vary() {
        let mut seen = std::collections::HashSet::new();
        for kx in -20..20 {
            for kz in -20..20 {
                for side in Side::ALL {
                    let n = wall_normal(side, kx, kz, 1.0);
                    assert!((n.length() - 1.0).abs() < 1e-5);
                    assert!(n.dot(side.outward()) > 0.98, "wall tilt too large");
                    seen.insert(((n.x * 1000.0) as i32, (n.z * 1000.0) as i32));
                }
            }
        }
        assert!(seen.len() > 1000, "wall tilts barely vary: {}", seen.len());
    }

    /// The shader keys its per-column value on the column tag and the 64 m
    /// column grid. Both sides must agree, or the steps land mid-column.
    #[test]
    fn shader_column_contract_matches_bands() {
        let shader = include_str!("../shaders/world_shader.wgsl");
        assert!(shader.contains("const HORIZON_COLUMN_M: f32 = 64.0;"));
        assert!(shader.contains("return ao < -1.125 && ao > -1.375;"));
        assert!(COLUMN_AO < -1.125 && COLUMN_AO > -1.375, "tag outside the shader's column range");
        assert!(COLUMN_AO < 0.0 && COLUMN_AO > -1.5, "tag must stay a backdrop value");
        // CPU consumers (collar eligibility, water-depth lerp) must still see
        // columns as plain backdrop, and must not see anything else as one.
        let backdrop = super::super::edits::is_backdrop_ao;
        assert!(backdrop(-1.0) && backdrop(COLUMN_AO));
        assert!(!backdrop(-2.0) && !backdrop(-1.5) && !backdrop(0.0) && !backdrop(1.0));
        let built: Vec<f32> = BANDS.iter().skip(1).map(|&(_, m)| m).collect();
        assert_eq!(built, vec![64.0], "shader assumes one built band of 64 m columns");
    }

    /// Builds the real 64 m band around the origin and checks what the renderer
    /// receives: finite vertices, valid indices, no band under the fine grid,
    /// walls at the lattice seam, and neighbouring columns sharing their edge
    /// heights exactly, which closes the seams between tops.
    #[test]
    fn built_band_is_finite_and_its_seams_close() {
        use crate::{fluid::VoxelScalePreset, world::WorldType};
        let cfg = WorldSampleConfig {
            seed: 2_738_958_700,
            generator_version: 25,
            world_type: WorldType::Hills,
            voxel_scale: VoxelScalePreset::OneMeter,
        };
        let cancel = AtomicBool::new(false);
        let center = (0.0, 0.0);
        let bands = fill_bands(&cfg, center, 1536.0, &cancel).expect("bands fill");
        let mut mesh = ChunkMesh {
            vertices: Vec::new(),
            indices: Vec::new(),
        };
        let (count, walls) =
            emit_columns(&mut mesh, &cfg, &bands, center, &cancel).expect("columns emit");
        assert_eq!(count[0], 0, "the inner band must build nothing");
        assert!(count[1] > 30_000, "outer band built {} columns", count[1]);
        assert!(walls > 0, "no walls at the lattice seam");
        assert!(
            mesh.vertices
                .iter()
                .all(|v| v.pos.iter().chain(&v.normal).all(|c| c.is_finite())),
            "non-finite vertex"
        );
        assert!(
            mesh.indices.iter().all(|&i| (i as usize) < mesh.vertices.len()),
            "index out of range"
        );

        let band = &bands[1];
        let mut shared = 0;
        for kz in band.min.1..band.min.1 + band.side {
            for kx in band.min.0..band.min.0 + band.side {
                let Some(a) = band.column(kx, kz) else {
                    continue;
                };
                if let Some(east) = band.column(kx + 1, kz) {
                    assert_eq!(a.corners[3].height, east.corners[0].height);
                    assert_eq!(a.corners[2].height, east.corners[1].height);
                    shared += 1;
                }
                if let Some(north) = band.column(kx, kz + 1) {
                    assert_eq!(a.corners[1].height, north.corners[0].height);
                    assert_eq!(a.corners[2].height, north.corners[3].height);
                    shared += 1;
                }
            }
        }
        assert!(shared > 50_000, "only {shared} shared column edges");
    }
}
