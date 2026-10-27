//! Cave culling: a breadth-first walk from the camera chunk through linked faces.
//!
//! A chunk is drawn only if a walk that never steps back toward the camera on
//! any axis can enter it through faces that open space links. Every visible
//! point lies on a ray that is monotone per axis, so such a walk reaches it.
//! Unknown chunks (unmeshed, LOD above 0, not resident) are fully open.
use super::ChunkCache;
use super::visibility::chunk_bounds_sphere;
use crate::mesh::ChunkConnectivity;
use crate::world::CHUNK_SIZE;
use glam::{IVec3, Mat4, Vec4};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Above this many chunks in the walk's box the frame skips cave culling.
const MAX_CELLS: usize = 1 << 19;
const STEPS: [IVec3; 6] = [IVec3::NEG_X, IVec3::X, IVec3::NEG_Y, IVec3::Y, IVec3::NEG_Z, IVec3::Z];

static LAST_CULLED: AtomicUsize = AtomicUsize::new(0);

/// Opt-in with `VOXEL_CAVE_CULL=1`. Off by default: residency already skips
/// sealed underground chunks, so on 2026-10-27 the walk culled 0-1 chunks at
/// surface, Undercrown and cave poses for about 0.15 ms of CPU per frame.
pub fn enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var("VOXEL_CAVE_CULL").is_ok_and(|v| v.trim() == "1"))
}

/// Chunks inside the frustum that the last draw list dropped as occluded.
pub fn last_culled() -> usize {
    LAST_CULLED.load(Ordering::Relaxed)
}

/// Dense chunk box shared by the walk's per-chunk arrays.
#[derive(Clone, Copy)]
pub(super) struct Grid {
    min: IVec3,
    size: IVec3,
}

impl Grid {
    pub(super) fn spanning(min: IVec3, max: IVec3) -> Option<Self> {
        let size = max - min + IVec3::ONE;
        let cells = size.as_i64vec3();
        (size.min_element() > 0 && (cells.x * cells.y * cells.z) as u64 <= MAX_CELLS as u64)
            .then_some(Self { min, size })
    }
    pub(super) fn cells(self) -> usize {
        (self.size.x * self.size.y * self.size.z) as usize
    }
    pub(super) fn index(self, c: IVec3) -> Option<usize> {
        let l = c - self.min;
        (l.min_element() >= 0 && l.cmplt(self.size).all())
            .then(|| ((l.y * self.size.z + l.z) * self.size.x + l.x) as usize)
    }
    fn coord(self, i: usize) -> IVec3 {
        let i = i as i32;
        let (x, rest) = (i % self.size.x, i / self.size.x);
        self.min + IVec3::new(x, rest / self.size.z, rest % self.size.z)
    }
}

/// Chunks the walk reached; anything outside the grid counts as reached.
pub(super) struct Reach {
    grid: Grid,
    reached: Vec<bool>,
}

impl Reach {
    pub(super) fn reaches(&self, c: IVec3) -> bool {
        self.grid.index(c).is_none_or(|i| self.reached[i])
    }

    /// `links[i]` is chunk `i`'s connectivity, `None` when unknown.
    pub(super) fn walk(
        grid: Grid,
        links: &[Option<ChunkConnectivity>],
        camera: IVec3,
        in_view: impl Fn(IVec3) -> bool,
    ) -> Option<Self> {
        let start = grid.index(camera)?;
        if links.len() != grid.cells() {
            return None;
        }
        let cells = grid.cells();
        let mut reached = vec![false; cells];
        let mut entered = vec![0u8; cells];
        // 0 unknown, 1 inside, 2 outside.
        let mut view = vec![0u8; cells];
        let mut inside = |j: usize| {
            if view[j] == 0 {
                view[j] = if in_view(grid.coord(j)) { 1 } else { 2 };
            }
            view[j] == 1
        };
        let mut queue = Vec::with_capacity(4096);
        reached[start] = true;
        queue.push(start);
        let mut head = 0;
        // Every step moves one chunk farther from the camera, so FIFO order
        // finishes each Manhattan layer before the next is dequeued and every
        // entry face is known by then.
        while let Some(&i) = queue.get(head) {
            head += 1;
            let c = grid.coord(i);
            let exits = if i == start {
                0x3f
            } else {
                links[i].unwrap_or(ChunkConnectivity::ALL_OPEN).exits_from(entered[i])
            };
            for (d, step) in STEPS.iter().enumerate() {
                if exits & (1 << d) == 0 {
                    continue;
                }
                let n = c + *step;
                let axis = d / 2;
                if (n[axis] - camera[axis]).abs() <= (c[axis] - camera[axis]).abs() {
                    continue;
                }
                let Some(j) = grid.index(n) else { continue };
                if !inside(j) {
                    continue;
                }
                if !reached[j] {
                    reached[j] = true;
                    queue.push(j);
                }
                entered[j] |= 1 << (d ^ 1);
            }
        }
        // Torches, fittings and models can draw past their own chunk.
        let spill: Vec<usize> = (0..cells)
            .filter(|&i| !reached[i] && links[i].is_some_and(ChunkConnectivity::spills) && inside(i))
            .filter(|&i| {
                let c = grid.coord(i);
                (-1..=1).any(|y| (-1..=1).any(|z| (-1..=1).any(|x| {
                    grid.index(c + IVec3::new(x, y, z)).is_some_and(|j| reached[j])
                })))
            })
            .collect();
        for i in spill {
            reached[i] = true;
        }
        Some(Self { grid, reached })
    }
}

/// Camera position of a perspective view-projection, `None` for orthographic.
pub(super) fn eye_from(view_proj: Mat4) -> Option<glam::Vec3> {
    let h = view_proj.inverse() * Vec4::Z;
    let eye = h.truncate() / h.w;
    (h.w.abs() > f32::EPSILON && eye.is_finite()).then_some(eye)
}

/// Per-frame cave-culling filter for `build_draw_list`.
pub(super) struct Occlusion {
    reach: Option<Reach>,
    culled: usize,
}

impl Occlusion {
    pub(super) fn new(cache: &ChunkCache, view_proj: Mat4, planes: &[Vec4; 6]) -> Self {
        let reach = enabled().then(|| Self::walk(cache, view_proj, planes)).flatten();
        Self { reach, culled: 0 }
    }

    fn walk(cache: &ChunkCache, view_proj: Mat4, planes: &[Vec4; 6]) -> Option<Reach> {
        let camera = (eye_from(view_proj)? / CHUNK_SIZE as f32).floor().as_ivec3();
        // A monotone path to a chunk stays inside the box spanning it and the
        // camera, and only LOD 0 chunks can be culled.
        let (min, max) = cache
            .gpu_meshes
            .iter()
            .filter(|(_, mesh)| mesh.lod == 0)
            .fold((camera, camera), |(lo, hi), (&c, _)| (lo.min(c), hi.max(c)));
        let grid = Grid::spanning(min, max)?;
        let mut links = vec![None; grid.cells()];
        for (&coord, mesh) in &cache.gpu_meshes {
            if mesh.lod == 0
                && let Some(i) = grid.index(coord)
            {
                links[i] = mesh.connectivity;
            }
        }
        let in_view = |c: IVec3| {
            let (center, radius) = chunk_bounds_sphere(c);
            // Same test as the draw list: no far plane.
            planes[..5].iter().all(|p| p.truncate().dot(center) + p.w >= -radius)
        };
        Reach::walk(grid, &links, camera, in_view)
    }

    /// Whether to drop a frustum-visible chunk; LOD above 0 is never culled.
    pub(super) fn hides(&mut self, coord: IVec3, lod: u8) -> bool {
        let hidden = lod == 0 && self.reach.as_ref().is_some_and(|r| !r.reaches(coord));
        self.culled += usize::from(hidden);
        hidden
    }

    pub(super) fn publish(&self) {
        LAST_CULLED.store(self.culled, Ordering::Relaxed);
    }
}

#[cfg(test)]
#[path = "occlusion_tests.rs"]
mod tests;
