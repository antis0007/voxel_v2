//! Rigid scene geometry built once in local metres and posed by one rotation
//! and translation per use.
//!
//! While `models::build` records a frame, `place` appends a placement and the
//! renderer poses the resident pool in a compute pass (`render/posed_assets.rs`),
//! so a large static structure costs no per-frame CPU vertex work or upload.
//! Outside a recording (tests, tools, no renderer) `place` expands the vertices
//! on the CPU with the arithmetic the callers used before.
use super::Model;
use crate::mesh::Vertex3;
use glam::{Quat, Vec3};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;

/// Pool vertices (56 bytes each) above which the library restarts at the
/// next frame start. Twice this still fits wgpu's default 128 MiB storage
/// binding, since a restart can only happen between frames.
const POOL_LIMIT: usize = 1 << 20;

/// One posed copy of the pool range `first..first + count`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub first: u32,
    pub count: u32,
    pub rotation: Quat,
    /// Metres.
    pub translation: Vec3,
    /// Metres per output unit, the scene model's `scale`.
    pub scale: f32,
}

#[derive(Default)]
struct Library {
    pool: Vec<Vertex3>,
    ranges: HashMap<u64, (u32, u32)>,
    generation: u64,
}

thread_local! {
    static LIBRARY: RefCell<Library> = RefCell::new(Library::default());
    static FRAME: RefCell<Vec<Placement>> = const { RefCell::new(Vec::new()) };
    static RECORDING: Cell<bool> = const { Cell::new(false) };
    static ENABLED: Cell<bool> = const { Cell::new(false) };
}

/// Set by a renderer that consumes placements, on the thread that builds the
/// scene. Per thread, so tests that build scenes never defer by accident.
pub fn set_enabled(enabled: bool) {
    ENABLED.set(enabled);
}

pub(super) fn begin_frame() {
    RECORDING.set(ENABLED.get());
    FRAME.with(|frame| frame.borrow_mut().clear());
    LIBRARY.with(|library| {
        let mut library = library.borrow_mut();
        // Ranges held by this frame's placements must stay valid, so the
        // pool only restarts here.
        if library.pool.len() > POOL_LIMIT {
            library.pool.clear();
            library.ranges.clear();
            library.generation += 1;
        }
    });
}

pub(super) fn end_frame() {
    RECORDING.set(false);
}

/// True while `place` defers to the GPU. Placed vertices skip any later
/// per-vertex pass over `Model::vertices`, so callers bake such passes in.
pub(super) fn recording() -> bool {
    RECORDING.get()
}

/// The pool range of asset `key`, built on first use. A key names one exact
/// vertex list: it must change with anything the vertices depend on.
pub(super) fn intern(key: u64, build: impl FnOnce() -> Vec<Vertex3>) -> (u32, u32) {
    LIBRARY.with(|library| {
        let mut library = library.borrow_mut();
        if let Some(&range) = library.ranges.get(&key) {
            return range;
        }
        let vertices = build();
        let range = (library.pool.len() as u32, vertices.len() as u32);
        library.pool.extend_from_slice(&vertices);
        library.ranges.insert(key, range);
        range
    })
}

pub(super) fn place(mesh: &mut Model, range: (u32, u32), rotation: Quat, translation: Vec3) {
    let scale = mesh.scale;
    if RECORDING.get() {
        FRAME.with(|frame| {
            frame.borrow_mut().push(Placement { first: range.0, count: range.1, rotation, translation, scale })
        });
        return;
    }
    LIBRARY.with(|library| {
        let library = library.borrow();
        let source = &library.pool[range.0 as usize..(range.0 + range.1) as usize];
        mesh.vertices.extend(source.iter().map(|v| pose(v, rotation, translation, scale)));
    });
}

/// The CPU form of `cs_pose` in `posed_assets.wgsl`.
pub fn pose(v: &Vertex3, rotation: Quat, translation: Vec3, scale: f32) -> Vertex3 {
    let mut out = *v;
    out.pos = ((translation + rotation * Vec3::from_array(v.pos)) / scale).to_array();
    out.normal = (rotation * Vec3::from_array(v.normal)).to_array();
    out
}

/// This frame's placements, the pool they index and the pool generation.
/// A changed generation means every earlier pool range is invalid.
pub fn with_frame<R>(f: impl FnOnce(&[Placement], &[Vertex3], u64) -> R) -> R {
    FRAME.with(|frame| LIBRARY.with(|library| {
        let library = library.borrow();
        f(&frame.borrow(), &library.pool, library.generation)
    }))
}

/// FNV-1a over 32-bit key parts.
pub(super) fn key(parts: &[u32]) -> u64 {
    parts.iter().fold(0xcbf2_9ce4_8422_2325_u64, |h, &p| (h ^ u64::from(p)).wrapping_mul(0x1000_0000_01b3))
}

#[cfg(test)]
#[path = "posed_tests.rs"]
mod tests;
