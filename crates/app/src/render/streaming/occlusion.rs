//! Cave culling: a breadth-first walk from the camera chunk through linked faces.
//!
//! A chunk is drawn only if a walk that never steps back toward the camera on
//! any axis can enter it through faces that open space links. Every visible
//! point lies on a ray that is monotone per axis, so such a walk reaches it.
//! Unknown chunks (unmeshed, LOD above 0, not resident) are fully open.
use super::ChunkCache;
use crate::mesh::ChunkConnectivity;
use crate::world::CHUNK_SIZE;
use glam::{IVec3, Mat4, Vec4};
use std::sync::{Arc, OnceLock, Weak};
use std::sync::atomic::{AtomicUsize, Ordering};

/// Above this many chunks in the walk's box the frame skips cave culling.
const MAX_CELLS: usize = 1 << 19;
const STEPS: [IVec3; 6] = [IVec3::NEG_X, IVec3::X, IVec3::NEG_Y, IVec3::Y, IVec3::NEG_Z, IVec3::Z];

static LAST_CULLED: AtomicUsize = AtomicUsize::new(0);

/// On unless `VOXEL_CAVE_CULL=0`. In a half-metre cavern on 2026-11-04 it cut
/// terrain draws 607 to 354 and triangles 17%; surface poses were unchanged.
pub fn enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var("VOXEL_CAVE_CULL").map_or(true, |v| v.trim() != "0"))
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
#[cfg(test)]
pub(super) struct Reach {
    grid: Grid,
    reached: Vec<bool>,
}

#[derive(Default)]
pub(super) struct Scratch {
    topology: Option<Topology>,
    links: Vec<Option<ChunkConnectivity>>,
    reached: Vec<bool>,
    entered: Vec<u8>,
    view: Vec<u8>,
    queue: Vec<usize>,
    spill_candidates: Vec<usize>,
    spill: Vec<usize>,
    reach_valid: bool,
}

struct Topology {
    owner: Weak<super::residency::VoxelAllocations>,
    revision: u64,
    camera: IVec3,
    grid: Option<Grid>,
}

#[cfg(test)]
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
        let mut scratch = Scratch::default();
        scratch.links.extend_from_slice(links);
        scratch.spill_candidates.extend(links.iter().enumerate()
            .filter_map(|(i, link)| link.is_some_and(ChunkConnectivity::spills).then_some(i)));
        scratch.walk(grid, camera, in_view)?;
        Some(Self { grid, reached: scratch.reached })
    }
}

impl Scratch {
    fn walk(&mut self, grid: Grid, camera: IVec3, in_view: impl Fn(IVec3) -> bool) -> Option<()> {
        let start = grid.index(camera)?;
        let Self { links, reached, entered, view, queue, spill_candidates, spill, .. } = self;
        if links.len() != grid.cells() {
            return None;
        }
        let cells = grid.cells();
        reached.clear();
        reached.resize(cells, false);
        entered.clear();
        entered.resize(cells, 0);
        // 0 unknown, 1 inside, 2 outside.
        view.clear();
        view.resize(cells, 0);
        let mut inside = |j: usize| {
            if view[j] == 0 {
                view[j] = if in_view(grid.coord(j)) { 1 } else { 2 };
            }
            view[j] == 1
        };
        queue.clear();
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
                links[i].map_or(0x3f, |links| links.exits_from(entered[i]))
            };
            for (d, step) in STEPS.iter().enumerate() {
                if exits & (1 << d) == 0 {
                    continue;
                }
                let axis = d / 2;
                let toward_camera = if d & 1 == 0 {
                    c[axis] > camera[axis]
                } else {
                    c[axis] < camera[axis]
                };
                if toward_camera {
                    continue;
                }
                let n = c + *step;
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
        spill.clear();
        spill.extend(spill_candidates.iter().copied()
            .filter(|&i| !reached[i] && inside(i))
            .filter(|&i| {
                let c = grid.coord(i);
                (-1..=1).any(|y| (-1..=1).any(|z| (-1..=1).any(|x| {
                    grid.index(c + IVec3::new(x, y, z)).is_some_and(|j| reached[j])
                })))
            })
        );
        for &i in spill.iter() {
            reached[i] = true;
        }
        Some(())
    }
}

/// Camera position of a perspective view-projection, `None` for orthographic.
pub(super) fn eye_from(view_proj: Mat4) -> Option<glam::Vec3> {
    let h = view_proj.inverse() * Vec4::Z;
    let eye = h.truncate() / h.w;
    (h.w.abs() > f32::EPSILON && eye.is_finite()).then_some(eye)
}

/// Per-frame cave-culling filter for `build_draw_list`.
pub(super) struct Occlusion<'a> {
    reach: Option<(Grid, &'a [bool])>,
    culled: usize,
}

impl<'a> Occlusion<'a> {
    pub(super) fn new(cache: &ChunkCache, eye: Option<glam::Vec3>, scratch: &'a mut Scratch) -> Self {
        let reach = eye.filter(|eye| enabled() && eye.is_finite()).and_then(|eye| {
            let camera = (eye / CHUNK_SIZE as f32).floor().as_ivec3();
            let grid = Self::topology(cache, camera, scratch)?;
            // Frustum culling applies to the final draw list. Cutting off
            // intermediate chunks here made reachability change as the eye
            // turned, even for chunks that stayed inside the view.
            if !scratch.reach_valid {
                scratch.walk(grid, camera, |_| true)?;
                scratch.reach_valid = true;
            }
            Some((grid, scratch.reached.as_slice()))
        });
        Self { reach, culled: 0 }
    }

    fn topology(cache: &ChunkCache, camera: IVec3, scratch: &mut Scratch) -> Option<Grid> {
        let revision = cache.gpu_mesh_revision;
        if let Some(saved) = &scratch.topology
            && saved.revision == revision
            && saved.camera == camera
            && saved.owner.as_ptr() == Arc::as_ptr(&cache.voxel_allocations)
        {
            return saved.grid;
        }
        // A monotone path to a chunk stays inside the box spanning it and the
        // camera, and only LOD 0 chunks can be culled.
        let (min, max) = cache
            .gpu_meshes
            .iter()
            .filter(|(_, mesh)| mesh.lod == 0)
            .fold((camera, camera), |(lo, hi), (&c, _)| (lo.min(c), hi.max(c)));
        let grid = Grid::spanning(min, max);
        // Cache both an accepted grid and a box that exceeded the cell limit.
        scratch.reach_valid = false;
        scratch.topology = Some(Topology {
            owner: Arc::downgrade(&cache.voxel_allocations),
            revision,
            camera,
            grid,
        });
        let grid = grid?;
        scratch.links.clear();
        scratch.links.resize(grid.cells(), None);
        scratch.spill_candidates.clear();
        for (&coord, mesh) in &cache.gpu_meshes {
            if mesh.lod == 0
                && let Some(i) = grid.index(coord)
            {
                scratch.links[i] = mesh.connectivity;
                if mesh.connectivity.is_some_and(ChunkConnectivity::spills) {
                    scratch.spill_candidates.push(i);
                }
            }
        }
        Some(grid)
    }

    /// Whether to drop a frustum-visible chunk; LOD above 0 is never culled.
    pub(super) fn hides(&mut self, coord: IVec3, lod: u8) -> bool {
        let hidden = lod == 0 && self.reach.is_some_and(|(grid, reached)| {
            grid.index(coord).is_some_and(|i| !reached[i])
        });
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
