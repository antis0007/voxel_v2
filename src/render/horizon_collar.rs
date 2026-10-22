//! One dry, installed column edge joined to the actual parent surface.
//! No source sampling, radial cutoffs, shader changes or collision geometry.
use crate::mesh::{ChunkMesh, Vertex3};
use glam::{IVec3, Vec3};
use std::{
    mem::size_of,
    ops::Range,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

const WIDTH: usize = 32;
const MAX_INDICES: usize = 262_144;
const MAX_DEPENDENCIES: usize = 64;
const PROFILE_BYTES: usize = 32 * 1024 * 1024;
const PATCH_BYTES: usize = 2 * 1024 * 1024;
static RESERVED: AtomicUsize = AtomicUsize::new(0);

struct Lease(usize);
impl Lease {
    fn new(bytes: usize) -> Option<Self> {
        RESERVED
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |used| {
                used.checked_add(bytes).filter(|&n| n <= PROFILE_BYTES)
            })
            .ok()?;
        Some(Self(bytes))
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        RESERVED.fetch_sub(self.0, Ordering::Relaxed);
    }
}

#[derive(Clone, Copy)]
struct Edge {
    tops: [Option<Vertex3>; WIDTH],
    blocked: u32,
}
impl Default for Edge {
    fn default() -> Self {
        Self {
            tops: [None; WIDTH],
            blocked: 0,
        }
    }
}
pub(crate) struct Edges {
    values: [Edge; 4],
    _lease: Lease,
}

/// Missing is deliberately different from a successfully scanned empty edge.
#[derive(Default)]
pub(crate) enum Prepared {
    #[default]
    Missing,
    Empty,
    Edges(Arc<Edges>),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct Scope {
    pub coord: IVec3,
    pub lod: u8,
    pub world_epoch: u64,
    pub cache_epoch: u64,
    pub job_id: u64,
    pub revision: u64,
}
pub(crate) struct Installed {
    pub scope: Scope,
    profile: Prepared,
    _lease: Lease,
}
impl Installed {
    /// Call only after the upload transaction validates this exact CPU mesh.
    pub(crate) fn new(scope: Scope, profile: Prepared) -> Option<Arc<Self>> {
        if matches!(profile, Prepared::Missing) {
            return None;
        }
        let lease = Lease::new(size_of::<Self>() + 2 * size_of::<usize>())?;
        Some(Arc::new(Self {
            scope,
            profile,
            _lease: lease,
        }))
    }

    /// The streamer's empty invariant mesh can change its logical LOD without
    /// another upload. Reissue its scope instead of leaving stale LOD metadata.
    pub(crate) fn relabel_empty_lod(&self, lod: u8) -> Option<Arc<Self>> {
        if !matches!(self.profile, Prepared::Empty) {
            return None;
        }
        Self::new(Scope { lod, ..self.scope }, Prepared::Empty)
    }
}

fn axes(side: usize) -> (usize, usize, f32) {
    let axis = if side < 2 { 0 } else { 2 };
    (axis, 2 - axis, if side % 2 == 0 { -1.0 } else { 1.0 })
}
fn dry(v: &Vertex3) -> bool {
    // Material-family allowlist, preserving packed original block identity.
    matches!((v.surface[2] as u32) % 64, 1 | 2 | 3 | 4 | 33 | 36)
        && v.surface[2].is_finite()
        && v.surface[2].fract() == 0.0
        && v.surface[2] >= 0.0
}
fn lerp(a: Vertex3, b: Vertex3, t: f32) -> Vertex3 {
    let mut out = a;
    for i in 0..3 {
        out.pos[i] = a.pos[i] + (b.pos[i] - a.pos[i]) * t;
        out.color[i] = a.color[i] + (b.color[i] - a.color[i]) * t;
        out.normal[i] = a.normal[i] + (b.normal[i] - a.normal[i]) * t;
    }
    out.ao_strength = a.ao_strength + (b.ao_strength - a.ao_strength) * t;
    out
}

/// Triangle/plane intersection. Coplanar triangles are handled separately.
fn section(tri: &[Vertex3], axis: usize, plane: f32, along: usize) -> Option<[Vertex3; 2]> {
    let mut points = [tri[0]; 16];
    let mut n = 0;
    for i in 0..tri.len() {
        let a = tri[i];
        let b = tri[(i + 1) % tri.len()];
        if a.pos[axis] == plane {
            *points.get_mut(n)? = a;
            n += 1;
        }
        if (a.pos[axis] < plane && b.pos[axis] > plane)
            || (a.pos[axis] > plane && b.pos[axis] < plane)
        {
            let mut p = lerp(a, b, (plane - a.pos[axis]) / (b.pos[axis] - a.pos[axis]));
            p.pos[axis] = plane;
            *points.get_mut(n)? = p;
            n += 1;
        }
    }
    if n < 2 {
        return None;
    }
    let mut low = points[0];
    let mut high = points[0];
    for &p in &points[1..n] {
        if p.pos[along] < low.pos[along] {
            low = p;
        }
        if p.pos[along] > high.pos[along] {
            high = p;
        }
    }
    (low.pos[along] < high.pos[along]).then_some([low, high])
}

impl Prepared {
    /// Final encoded/compacted mesh, on its worker; never scan GPU data/frame thread.
    pub(crate) fn from_mesh(coord: IVec3, mesh: &ChunkMesh, cancel: &AtomicBool) -> Self {
        if mesh.indices.len() > MAX_INDICES || mesh.indices.len() % 3 != 0 {
            return Self::Missing;
        }
        let Some(x) = coord.x.checked_mul(32) else {
            return Self::Missing;
        };
        let Some(z) = coord.z.checked_mul(32) else {
            return Self::Missing;
        };
        let origin = [x as f32, 0.0, z as f32];
        // Reject coordinates where one source voxel no longer has an exact f32 span.
        if [x, z].into_iter().any(|v| {
            f64::from(v as f32) != f64::from(v)
                || (v as f32 + 32.0) - v as f32 != 32.0
                || v as f32 + 1.0 == v as f32
        }) {
            return Self::Missing;
        }
        let mut edges = [Edge::default(); 4];
        for (i, indices) in mesh.indices.chunks_exact(3).enumerate() {
            if i % 256 == 0 && cancel.load(Ordering::Relaxed) {
                return Self::Missing;
            }
            let [Some(a), Some(b), Some(c)] = [
                mesh.vertices.get(indices[0] as usize),
                mesh.vertices.get(indices[1] as usize),
                mesh.vertices.get(indices[2] as usize),
            ] else {
                return Self::Missing;
            };
            let tri = [*a, *b, *c];
            if tri.iter().any(|v| {
                !bytemuck::cast_slice::<Vertex3, f32>(std::slice::from_ref(v))
                    .iter()
                    .all(|x| x.is_finite())
            }) {
                return Self::Missing;
            }
            for (side, edge) in edges.iter_mut().enumerate() {
                let (axis, along, sign) = axes(side);
                let plane = origin[axis] + if sign > 0.0 { 32.0 } else { 0.0 };
                let parallel = tri.iter().all(|v| v.pos[axis] == tri[0].pos[axis]);
                // Resident boundary walls can be pushed 0.22 voxels by coarse LOD.
                let wall = parallel && (tri[0].pos[axis] - plane).abs() <= 0.25;
                let segment = if wall {
                    let lo = tri
                        .iter()
                        .min_by(|a, b| a.pos[along].total_cmp(&b.pos[along]))
                        .unwrap();
                    let hi = tri
                        .iter()
                        .max_by(|a, b| a.pos[along].total_cmp(&b.pos[along]))
                        .unwrap();
                    Some([*lo, *hi])
                } else {
                    section(&tri, axis, plane, along)
                };
                let Some([lo, hi]) = segment else {
                    continue;
                };
                let winding = (Vec3::from_array(tri[1].pos) - Vec3::from_array(tri[0].pos))
                    .cross(Vec3::from_array(tri[2].pos) - Vec3::from_array(tri[0].pos));
                let flat_top = !wall
                    && winding.y > 0.0
                    && tri.iter().all(|v| {
                        dry(v) && v.normal == [0.0, 1.0, 0.0] && v.pos[1] == tri[0].pos[1]
                    });
                for unit in 0..WIDTH {
                    let start = origin[along] + unit as f32;
                    if hi.pos[along] <= start || lo.pos[along] >= start + 1.0 {
                        continue;
                    }
                    if !flat_top || lo.pos[along] > start || hi.pos[along] < start + 1.0 {
                        edge.blocked |= 1 << unit;
                        continue;
                    }
                    let t = (start + 0.5 - lo.pos[along]) / (hi.pos[along] - lo.pos[along]);
                    let mut sample = lerp(lo, hi, t);
                    sample.pos[axis] = plane;
                    sample.pos[along] = start + 0.5;
                    if edge.tops[unit].is_some_and(|old| old.pos[1] != sample.pos[1]) {
                        edge.blocked |= 1 << unit;
                    } else {
                        edge.tops[unit] = Some(sample);
                    }
                }
            }
        }
        if cancel.load(Ordering::Relaxed) {
            return Self::Missing;
        }
        if edges
            .iter()
            .all(|e| e.blocked == 0 && e.tops.iter().all(Option::is_none))
        {
            return Self::Empty;
        }
        let Some(lease) = Lease::new(size_of::<Edges>() + 2 * size_of::<usize>()) else {
            return Self::Missing;
        };
        Self::Edges(Arc::new(Edges {
            values: edges,
            _lease: lease,
        }))
    }
}

/// One complete installed column bundle. Retaining dependencies prevents ABA.
pub(crate) struct Input {
    pub column: (i32, i32),
    pub side: usize,
    tops: [Vertex3; WIDTH],
    pub dependencies: Vec<Arc<Installed>>,
}
impl Input {
    pub(crate) fn new(
        column: (i32, i32),
        side: usize,
        dependencies: impl AsRef<[Arc<Installed>]>,
    ) -> Option<Arc<Self>> {
        let dependencies = dependencies.as_ref();
        if side >= 4 || dependencies.is_empty() || dependencies.len() > MAX_DEPENDENCIES {
            return None;
        }
        let scope = dependencies[0].scope;
        let mut tops: [Option<Vertex3>; WIDTH] = [None; WIDTH];
        for source in dependencies {
            if (source.scope.coord.x, source.scope.coord.z) != column
                || source.scope.world_epoch != scope.world_epoch
                || source.scope.cache_epoch != scope.cache_epoch
            {
                return None;
            }
            match &source.profile {
                Prepared::Missing => return None,
                Prepared::Empty => {}
                Prepared::Edges(edges) => {
                    let edge = &edges.values[side];
                    if edge.blocked != 0 {
                        return None;
                    }
                    for (out, incoming) in tops.iter_mut().zip(edge.tops) {
                        if let Some(value) = incoming {
                            if out.is_some_and(|old| old.pos[1] != value.pos[1]) {
                                return None;
                            }
                            *out = Some(value);
                        }
                    }
                }
            }
        }
        let first = tops[0]?;
        let mut complete = [first; WIDTH];
        for (out, value) in complete.iter_mut().zip(tops) {
            *out = value?;
        }
        Some(Arc::new(Self {
            column,
            side,
            tops: complete,
            dependencies: dependencies.to_vec(),
        }))
    }
    pub(crate) fn neighbor(&self) -> Option<(i32, i32)> {
        let (x, z) = self.column;
        match self.side {
            0 => Some((x.checked_sub(1)?, z)),
            1 => Some((x.checked_add(1)?, z)),
            2 => Some((x, z.checked_sub(1)?)),
            3 => Some((x, z.checked_add(1)?)),
            _ => None,
        }
    }
    /// The strip and both transverse caps must be outside every resident owner.
    /// Requiring the diagonal neighbours too avoids masked caps at corners.
    pub(crate) fn unowned_columns(&self) -> Option<[(i32, i32); 3]> {
        Self::outside_columns(self.column, self.side)
    }
    pub(crate) fn outside_columns(column: (i32, i32), side: usize) -> Option<[(i32, i32); 3]> {
        let (x, z) = column;
        let (x, z) = match side {
            0 => (x.checked_sub(1)?, z),
            1 => (x.checked_add(1)?, z),
            2 => (x, z.checked_sub(1)?),
            3 => (x, z.checked_add(1)?),
            _ => return None,
        };
        Some(if side < 2 {
            [(x, z), (x, z.checked_sub(1)?), (x, z.checked_add(1)?)]
        } else {
            [(x, z), (x.checked_sub(1)?, z), (x.checked_add(1)?, z)]
        })
    }
    pub(crate) fn same_installation(&self, other: &Self) -> bool {
        self.column == other.column
            && self.side == other.side
            && self.dependencies.len() == other.dependencies.len()
            && self
                .dependencies
                .iter()
                .zip(&other.dependencies)
                .all(|(a, b)| Arc::ptr_eq(a, b))
    }

    /// Current eligible edge proofs can outlive an identity-only replacement
    /// (for example interior water changes). Exact edge equality avoids rebuilding
    /// the parent/shadows solely because an unrelated part of the column changed.
    pub(crate) fn same_geometry(&self, other: &Self) -> bool {
        self.column == other.column
            && self.side == other.side
            && self.dependencies.len() == other.dependencies.len()
            && self
                .dependencies
                .iter()
                .zip(&other.dependencies)
                .all(|(a, b)| {
                    a.scope.coord == b.scope.coord
                        && a.scope.lod == b.scope.lod
                        && a.scope.world_epoch == b.scope.world_epoch
                        && a.scope.cache_epoch == b.scope.cache_epoch
                })
            && bytemuck::cast_slice::<Vertex3, u8>(&self.tops)
                == bytemuck::cast_slice::<Vertex3, u8>(&other.tops)
    }
}

#[derive(Clone, Copy)]
struct Outer {
    low: Vertex3,
    high: Vertex3,
}
fn outer_height(outer: &[Outer], along: usize, u: f32) -> Option<f32> {
    let segment = outer
        .iter()
        .find(|s| s.low.pos[along] <= u && u <= s.high.pos[along])?;
    let t = (u - segment.low.pos[along]) / (segment.high.pos[along] - segment.low.pos[along]);
    Some(segment.low.pos[1] + (segment.high.pos[1] - segment.low.pos[1]) * t)
}
fn projected_area(poly: &[Vertex3]) -> f32 {
    if poly.len() < 3 {
        return 0.0;
    }
    (1..poly.len() - 1)
        .map(|i| {
            let a = Vec3::from_array(poly[i].pos) - Vec3::from_array(poly[0].pos);
            let b = Vec3::from_array(poly[i + 1].pos) - Vec3::from_array(poly[0].pos);
            a.cross(b).y.abs() * 0.5
        })
        .sum()
}
fn triangle(mesh: &mut ChunkMesh, mut tri: [Vertex3; 3], desired: Vec3) {
    let p = tri.map(|v| Vec3::from_array(v.pos));
    let mut normal = (p[1] - p[0]).cross(p[2] - p[0]);
    if normal.length_squared() <= 1e-12 {
        return;
    }
    if normal.dot(desired) < 0.0 {
        tri.swap(1, 2);
        normal = -normal;
    }
    for vertex in &mut tri {
        vertex.normal = normal.normalize().to_array();
    }
    let base = mesh.vertices.len() as u32;
    mesh.vertices.extend(tri);
    mesh.indices.extend([base, base + 1, base + 2]);
}
fn in_budget(mesh: &ChunkMesh) -> bool {
    mesh.vertices.len() * size_of::<Vertex3>() + mesh.indices.len() * 4 <= PATCH_BYTES
}

/// Split the surviving parent's cut edges at every collar/cap knot. A centre
/// fan keeps collinear boundary knots referenced instead of dropping them as
/// zero-area triangles in an ordinary vertex fan.
fn append_survivor(
    mesh: &mut ChunkMesh,
    polygon: &[Vertex3],
    boundary: &[Vertex3],
    axis: usize,
    end: f32,
    along: usize,
    limits: [f32; 2],
) {
    let mut split = Vec::new();
    for i in 0..polygon.len() {
        let a = polygon[i];
        let b = polygon[(i + 1) % polygon.len()];
        split.push(a);
        let dimension = if a.pos[axis] == end && b.pos[axis] == end {
            Some(along)
        } else if limits
            .iter()
            .any(|&u| a.pos[along] == u && b.pos[along] == u)
        {
            Some(axis)
        } else {
            None
        };
        let Some(dimension) = dimension else {
            continue;
        };
        let delta = b.pos[dimension] - a.pos[dimension];
        if delta == 0.0 {
            continue;
        }
        let mut knots = Vec::new();
        for &point in boundary {
            let t = (point.pos[dimension] - a.pos[dimension]) / delta;
            if !(0.0 < t && t < 1.0) {
                continue;
            }
            let mut vertex = lerp(a, b, t);
            // A cut knot may differ by a final float-rounding operation. Use
            // its exact published position on BOTH sides, never a depth bias.
            if Vec3::from_array(vertex.pos).distance(Vec3::from_array(point.pos)) <= 0.0001 {
                vertex.pos = point.pos;
                knots.push((t, vertex));
            }
        }
        knots.sort_by(|a, b| a.0.total_cmp(&b.0));
        knots.dedup_by(|a, b| a.1.pos == b.1.pos);
        split.extend(knots.into_iter().map(|(_, v)| v));
    }
    split.dedup_by(|a, b| a.pos == b.pos);
    if split
        .first()
        .zip(split.last())
        .is_some_and(|(a, b)| a.pos == b.pos)
    {
        split.pop();
    }
    if split.len() < 3 {
        return;
    }
    let mut centre = split[0];
    let anchor = Vec3::from_array(centre.pos);
    let mut offset = Vec3::ZERO;
    centre.normal = [0.0; 3];
    centre.color = [0.0; 3];
    for v in &split {
        offset += (Vec3::from_array(v.pos) - anchor) / split.len() as f32;
        for i in 0..3 {
            centre.normal[i] += v.normal[i] / split.len() as f32;
            centre.color[i] += v.color[i] / split.len() as f32;
        }
    }
    centre.pos = (anchor + offset).to_array();
    let base = mesh.vertices.len() as u32;
    mesh.vertices.push(centre);
    mesh.vertices.extend_from_slice(&split);
    for i in 0..split.len() {
        mesh.indices.extend([
            base,
            base + 1 + i as u32,
            base + 1 + ((i + 1) % split.len()) as u32,
        ]);
    }
}

#[derive(Clone)]
pub(crate) struct Ranges {
    pub common: Range<u32>,
    pub collared: Range<u32>,
    pub uncollared: Range<u32>,
}
pub(crate) enum Outcome {
    Ineligible,
    Applied(Ranges),
}

/// A complete transaction: no index/vertex changes until the cutout, top,
/// transverse caps and all profile risers validate within the publication budget.
/// Call after exact edits, BEFORE batch partition/page packing, on the worker.
/// Ineligible leaves geometry unchanged; cancellation returns None. The caller
/// draws Common plus ONE variant, restoring the full affected parent triangles
/// immediately whenever this published collar's dependencies become invalid.
pub(crate) fn apply(
    mesh: &mut ChunkMesh,
    input: &Input,
    cancel: &AtomicBool,
    limit: usize,
) -> Option<Outcome> {
    use super::edits::clip;
    let (axis, along, sign) = axes(input.side);
    let plane = input.tops[0].pos[axis];
    let end = plane + sign;
    let u0 = input.tops[0].pos[along] - 0.5;
    let u1 = u0 + WIDTH as f32;
    if plane == end || u0 + 1.0 == u0 {
        return Some(Outcome::Ineligible);
    }
    let bounds = [
        (axis, plane.min(end), false),
        (axis, plane.max(end), true),
        (along, u0, false),
        (along, u1, true),
    ];
    let mut pieces = Vec::new();
    let mut survivors = Vec::new();
    let mut outer = Vec::new();
    let mut kept = Vec::new();
    let mut original = Vec::new();
    kept.try_reserve_exact(mesh.indices.len()).ok()?;
    let mut patch = ChunkMesh::empty();
    let mut area = 0.0;
    for (i, indices) in mesh.indices.chunks_exact(3).enumerate() {
        if i % 256 == 0 && cancel.load(Ordering::Relaxed) {
            return None;
        }
        let tri = [
            mesh.vertices[indices[0] as usize],
            mesh.vertices[indices[1] as usize],
            mesh.vertices[indices[2] as usize],
        ];
        if bounds.iter().any(|&(a, b, less)| {
            tri.iter()
                .all(|v| if less { v.pos[a] >= b } else { v.pos[a] <= b })
        }) {
            kept.extend_from_slice(indices);
            continue;
        }
        let mut inside = tri.to_vec();
        for &(a, b, less) in &bounds {
            inside = clip(&inside, a, b, less);
        }
        // Exact fallbacks, water, crowns, and model/detail intersections are not
        // a single dry heightfield. Never remove them to force eligibility.
        if tri.iter().any(|v| {
            !super::edits::is_backdrop_ao(v.ao_strength)
                || v.surface[2] != 0.0
                || !bytemuck::cast_slice::<Vertex3, f32>(std::slice::from_ref(v))
                    .iter()
                    .all(|f| f.is_finite())
        }) {
            return Some(Outcome::Ineligible);
        }
        let clipped_area = projected_area(&inside);
        if clipped_area <= 0.0 {
            kept.extend_from_slice(indices);
            continue;
        }
        if let Some([low, high]) = section(&inside, axis, end, along) {
            outer.push(Outer { low, high });
        }
        area += clipped_area;
        original.extend_from_slice(indices);
        pieces.push(inside);
        if pieces.len() > 256 || outer.len() > 64 {
            return Some(Outcome::Ineligible);
        }
        let mut remainder = tri.to_vec();
        for &(a, b, less) in &bounds {
            let outside = clip(&remainder, a, b, !less);
            if projected_area(&outside) > 0.0 {
                survivors.push(outside);
            }
            remainder = clip(&remainder, a, b, less);
        }
        if !in_budget(&patch) {
            return Some(Outcome::Ineligible);
        }
    }
    // A full strip must exist in the parent. Gaps/overlap are ineligible.
    if (area - WIDTH as f32).abs() > 0.001 || outer.is_empty() {
        return Some(Outcome::Ineligible);
    }
    outer.sort_by(|a, b| a.low.pos[along].total_cmp(&b.low.pos[along]));
    let mut cursor = u0;
    let mut last_height = None;
    for segment in &outer {
        if segment.low.pos[along] != cursor
            || last_height.is_some_and(|height| height != segment.low.pos[1])
        {
            return Some(Outcome::Ineligible);
        }
        cursor = segment.high.pos[along];
        last_height = Some(segment.high.pos[1]);
    }
    if cursor != u1 {
        return Some(Outcome::Ineligible);
    }
    let mut knots: Vec<f32> = (0..=WIDTH).map(|i| u0 + i as f32).collect();
    knots.extend(
        outer
            .iter()
            .flat_map(|s| [s.low.pos[along], s.high.pos[along]]),
    );
    knots.sort_by(f32::total_cmp);
    knots.dedup();
    if knots.len() > 128 {
        return Some(Outcome::Ineligible);
    }
    let mapped = |mut v: Vertex3, unit: usize| -> Option<Vertex3> {
        let t = ((v.pos[axis] - plane) / sign).clamp(0.0, 1.0);
        let height = outer_height(&outer, along, v.pos[along])?;
        v.pos[1] = input.tops[unit].pos[1] * (1.0 - t) + height * t;
        Some(v)
    };
    for piece in &pieces {
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
        // End caps join the changed strip cross-section to the unchanged parent.
        for (u, side) in [(u0, -1.0), (u1, 1.0)] {
            if let Some([a, b]) = section(piece, along, u, axis) {
                let unit = if side < 0.0 { 0 } else { WIDTH - 1 };
                let (Some(c), Some(d)) = (mapped(a, unit), mapped(b, unit)) else {
                    return Some(Outcome::Ineligible);
                };
                let da = c.pos[1] - a.pos[1];
                let db = d.pos[1] - b.pos[1];
                let mut spans = vec![(a, b, c, d)];
                if da * db < 0.0 {
                    let t = da / (da - db);
                    let old = lerp(a, b, t);
                    let new = lerp(c, d, t);
                    spans = vec![(a, old, c, new), (old, b, new, d)];
                }
                for (a, b, c, d) in spans {
                    let mut normal = Vec3::ZERO;
                    normal[along] = side
                        * if c.pos[1] + d.pos[1] >= a.pos[1] + b.pos[1] {
                            1.0
                        } else {
                            -1.0
                        };
                    triangle(&mut patch, [a, b, d], normal);
                    triangle(&mut patch, [a, d, c], normal);
                }
            }
        }
        for band in knots.windows(2) {
            if piece.iter().all(|v| v.pos[along] <= band[0])
                || piece.iter().all(|v| v.pos[along] >= band[1])
            {
                continue;
            }
            let polygon = clip(&clip(piece, along, band[0], false), along, band[1], true);
            if polygon.len() < 3 {
                continue;
            }
            let unit = ((band[0] - u0).floor() as usize).min(WIDTH - 1);
            let Some(first) = mapped(polygon[0], unit) else {
                return Some(Outcome::Ineligible);
            };
            for i in 1..polygon.len() - 1 {
                let (Some(b), Some(c)) = (mapped(polygon[i], unit), mapped(polygon[i + 1], unit))
                else {
                    return Some(Outcome::Ineligible);
                };
                triangle(&mut patch, [first, b, c], Vec3::Y);
            }
            if !in_budget(&patch) {
                return Some(Outcome::Ineligible);
            }
        }
    }
    for unit in 1..WIDTH {
        let left = input.tops[unit - 1];
        let right = input.tops[unit];
        if left.pos[1] == right.pos[1] {
            continue;
        }
        let u = u0 + unit as f32;
        let Some(height) = outer_height(&outer, along, u) else {
            return Some(Outcome::Ineligible);
        };
        let mut a = left;
        let mut b = left;
        let mut c = right;
        for v in [&mut a, &mut b, &mut c] {
            v.pos[along] = u;
            // Reuse the existing exact-fallback material and shadow conventions.
            v.ao_strength = -2.0 - v.ao_strength.clamp(0.0, 1.0);
        }
        a.pos[axis] = plane;
        c.pos[axis] = plane;
        b.pos[axis] = end;
        b.pos[1] = height;
        let mut normal = Vec3::ZERO;
        normal[along] = if left.pos[1] > right.pos[1] {
            1.0
        } else {
            -1.0
        };
        triangle(&mut patch, [a, b, c], normal);
    }
    let mut boundary: Vec<_> = patch
        .vertices
        .iter()
        .copied()
        .filter(|v| v.pos[axis] == end || v.pos[along] == u0 || v.pos[along] == u1)
        .collect();
    boundary.sort_unstable_by_key(|v| v.pos.map(f32::to_bits));
    boundary.dedup_by(|a, b| a.pos == b.pos);
    if boundary.len() > 512 {
        return Some(Outcome::Ineligible);
    }
    for polygon in survivors {
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
        append_survivor(&mut patch, &polygon, &boundary, axis, end, along, [u0, u1]);
        if !in_budget(&patch) {
            return Some(Outcome::Ineligible);
        }
    }
    let bytes = (mesh.vertices.len() + patch.vertices.len())
        .checked_mul(size_of::<Vertex3>())?
        .checked_add((kept.len() + patch.indices.len() + original.len()).checked_mul(4)?)?;
    if !in_budget(&patch) || bytes > limit || cancel.load(Ordering::Relaxed) {
        return Some(Outcome::Ineligible);
    }
    mesh.vertices.try_reserve_exact(patch.vertices.len()).ok()?;
    kept.try_reserve_exact(patch.indices.len() + original.len())
        .ok()?;
    let Some(base) = u32::try_from(mesh.vertices.len()).ok() else {
        return Some(Outcome::Ineligible);
    };
    if mesh.vertices.len() + patch.vertices.len() > u32::MAX as usize {
        return Some(Outcome::Ineligible);
    }
    let common_end = u32::try_from(kept.len()).ok()?;
    let collared_end = u32::try_from(kept.len() + patch.indices.len()).ok()?;
    let uncollared_end = u32::try_from(kept.len() + patch.indices.len() + original.len()).ok()?;
    kept.extend(patch.indices.into_iter().map(|i| base + i));
    kept.extend(original);
    mesh.vertices.extend(patch.vertices);
    mesh.indices = kept;
    Some(Outcome::Applied(Ranges {
        common: 0..common_end,
        collared: common_end..collared_end,
        uncollared: collared_end..uncollared_end,
    }))
}
