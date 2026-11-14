//! Water surface motion between simulation steps. A source patch remembers the
//! fill each changed cell showed, the exact water mesher writes the step and the
//! surface current into each vertex, and the upload stamps the clock at which
//! the mesh first draws. The world shader then slides each vertex from the
//! height the previous mesh showed to its own, and carries ripples downstream.
use super::{ACID_SURFACE_INSET, GeneratedChunk, LAVA_SURFACE_INSET, Vertex3, WATER_SURFACE_INSET};
use crate::world::BlockId;
use glam::{IVec3, Vec2};
use std::collections::HashMap;
use std::sync::{Arc, atomic::{AtomicBool, AtomicU32, Ordering}};

/// `surface.y` at or above this carries packed motion. 2^23 keeps every packed
/// value an exact integer in f32; plain water keeps its specular gain there.
pub const PACKED: f32 = 8_388_608.0;
/// Fastest current the packing stores, metres per second. Mirrored as
/// `WATER_FLOW_MAX` in `world_shader.wgsl`.
pub const FLOW_MAX: f32 = 2.5;
/// Downward speed written on falling and spilling water walls, metres per second.
pub const FALL_SPEED: f32 = 1.8;
/// `surface.x` of a vertex that does not animate: far enough in the past that
/// the shader's step has always finished.
pub const STILL: f32 = -1.0e9;
/// A patch older than this when its mesh uploads is not animated: the mesh was
/// rebuilt for residency or LOD, and replaying the step would move the water back.
const FRESH_SECONDS: f32 = 0.3;
/// Patches closer together than this, with no mesh job between them, keep the
/// oldest fill of each cell, which is what the screen still shows.
const MERGE_SECONDS: f32 = 0.3;

static CLOCK: AtomicU32 = AtomicU32::new(0);
static FRAME: AtomicU32 = AtomicU32::new(0);

/// Publish the shader clock (`torch_meta.y`) and the last frame's step, seconds.
pub fn set_clock(seconds: f32, frame: f32) {
    CLOCK.store(seconds.to_bits(), Ordering::Relaxed);
    FRAME.store(frame.max(0.0).to_bits(), Ordering::Relaxed);
}

fn clock() -> (f32, f32) {
    (f32::from_bits(CLOCK.load(Ordering::Relaxed)), f32::from_bits(FRAME.load(Ordering::Relaxed)))
}

/// Fills a source's cells showed before its latest patches.
#[derive(Clone, Default, Debug)]
pub struct WaterMotion {
    /// Previous fill of each changed cell, 0 for a cell that was dry.
    pub prev: HashMap<IVec3, f32>,
    /// Shader clock of the newest patch that wrote `prev`.
    pub time: f32,
    /// A mesh job took `prev`; the next patch starts a new map.
    pub sealed: Arc<AtomicBool>,
}

impl WaterMotion {
    pub fn seal(&self) {
        self.sealed.store(true, Ordering::Relaxed);
    }

    pub fn allocation_bytes(&self) -> usize {
        self.prev.capacity() * (std::mem::size_of::<(IVec3, f32)>() + 1) * 2
            + 2 * std::mem::size_of::<usize>() + std::mem::size_of::<AtomicBool>()
    }

    /// Note that `p` showed `fill` before the patch being applied now.
    pub fn record(&mut self, p: IVec3, fill: f32) {
        self.record_at(p, fill, clock().0);
    }

    fn record_at(&mut self, p: IVec3, fill: f32, now: f32) {
        if self.sealed.load(Ordering::Relaxed) || now < self.time || now - self.time > MERGE_SECONDS {
            self.prev.clear();
            self.sealed = Arc::new(AtomicBool::new(false));
        }
        self.time = now;
        self.prev.entry(p).or_insert(fill);
    }

    /// `surface.x` the mesher writes: the patch clock, or `STILL`.
    pub fn vertex_clock(&self) -> f32 {
        if self.prev.is_empty() { STILL } else { self.time }
    }
}

/// Fills for surface heights: the source's own, or the ones its last mesh showed.
/// Both views share the source's blocks, so the mesher emits one topology and
/// only the heights differ.
#[derive(Clone, Copy)]
pub(super) struct Fills<'a> {
    pub chunk: &'a GeneratedChunk,
    shown: Option<&'a HashMap<IVec3, f32>>,
}

impl<'a> Fills<'a> {
    pub fn is_water(self, p: IVec3) -> bool {
        self.shown.and_then(|m| m.get(&p)).map_or_else(
            || self.chunk.get_world(p) == BlockId::Water,
            |&fill| fill > 0.0,
        )
    }

    pub fn now(chunk: &'a GeneratedChunk) -> Self {
        Self { chunk, shown: None }
    }

    /// The previous mesh's view, or `None` when no cell changed since it.
    pub fn shown(chunk: &'a GeneratedChunk) -> Option<Self> {
        (!chunk.water_motion.prev.is_empty()).then_some(Self { chunk, shown: Some(&chunk.water_motion.prev) })
    }

    /// Free surface height of water cell `p` above its floor. Where the cell
    /// above drained since the shown mesh, the surface was up in that cell.
    pub fn level(self, p: IVec3) -> f32 {
        let shown = |q: IVec3| self.shown.and_then(|m| m.get(&q).copied());
        if let Some(above) = shown(p + IVec3::Y).filter(|&a| a > 0.0) {
            return 1.0 + above * (1.0 - WATER_SURFACE_INSET);
        }
        shown(p).unwrap_or_else(|| self.chunk.water_fill(p)) * (1.0 - WATER_SURFACE_INSET)
    }

    /// Absolute free surface height (voxels) of column `(x, z)` near `row`.
    pub fn column_surface(self, x: i32, z: i32, row: i32) -> Option<f32> {
        let water = |y: i32| self.is_water(IVec3::new(x, y, z));
        (row - 1..=row + 1)
            .rev()
            .find(|&y| water(y) && !water(y + 1))
            .map(|y| y as f32 + self.level(IVec3::new(x, y, z)))
    }
}

/// Pack a vertex's step (voxels from the shown height to its own, clamped to
/// one voxel) and its surface current (metres per second) into `surface.y`:
/// 8 bits of step, then 7 bits per current axis on a square-root scale, so
/// slow streams keep their direction.
pub fn pack(step: f32, flow: Vec2) -> f32 {
    let q_step = (step.clamp(-1.0, 1.0) * 127.0).round() + 127.0;
    let axis = |v: f32| {
        let s = (v.abs() / FLOW_MAX).min(1.0).sqrt().copysign(v);
        (s * 63.0).round() + 64.0
    };
    PACKED + q_step * 16384.0 + axis(flow.x) * 128.0 + axis(flow.y)
}

/// Inverse of [`pack`], as the shader decodes it. `None` for plain water.
pub fn unpack(value: f32) -> Option<(f32, Vec2)> {
    if !(value >= PACKED) {
        return None;
    }
    let packed = value - PACKED;
    let q_step = (packed / 16384.0).floor();
    let rest = packed - q_step * 16384.0;
    let qx = (rest / 128.0).floor();
    let axis = |q: f32| {
        let s = (q - 64.0) / 63.0;
        s * s.abs() * FLOW_MAX
    };
    Some(((q_step - 127.0) / 127.0, Vec2::new(axis(qx), axis(rest - qx * 128.0))))
}

/// Surface slope from the four columns touching a corner. Large steps can
/// be ignored when shading a surface next to a separate falling sheet.
pub(super) fn corner_rise(surface: [Option<f32>; 4], max_step: f32) -> Vec2 {
    // Columns in order (x-1, z-1), (x, z-1), (x-1, z), (x, z).
    let mean = |pairs: [(usize, usize); 2]| {
        let (mut sum, mut count) = (0.0, 0u32);
        for (a, b) in pairs {
            if let (Some(a), Some(b)) = (surface[a], surface[b])
                && (b - a).abs() <= max_step
            {
                sum += b - a;
                count += 1;
            }
        }
        if count == 0 { 0.0 } else { sum / count as f32 }
    };
    Vec2::new(mean([(0, 1), (2, 3)]), mean([(0, 2), (1, 3)]))
}

/// Downhill current at a lattice corner, in metres per second. Speed follows
/// the square root of the slope, as open-channel flow does.
pub fn corner_flow(surface: [Option<f32>; 4]) -> Vec2 {
    let rise = corner_rise(surface, f32::INFINITY);
    let slope = rise.length();
    if slope < 1e-4 {
        return Vec2::ZERO;
    }
    -rise / slope * FLOW_MAX * (slope / 0.15).min(1.0).sqrt()
}

/// Full-cell liquids share the surface current encoding with water.
pub(super) fn encode_liquid_current(chunk: &GeneratedChunk, vertex: &mut Vertex3) {
    if chunk.manyfold_shell || vertex.normal[1] <= 0.5 {
        return;
    }
    let liquid = BlockId::from_u32((vertex.surface[2] as u32 / 64).saturating_sub(1));
    let (inset, speed) = match liquid {
        BlockId::Lava => (LAVA_SURFACE_INSET, 0.16),
        BlockId::SulfuricAcid => (ACID_SURFACE_INSET, 0.36),
        _ => return,
    };
    let pos = glam::Vec3::from_array(vertex.pos);
    let row = pos.y.floor() as i32;
    let columns = [(-0.25, -0.25), (0.25, -0.25), (-0.25, 0.25), (0.25, 0.25)];
    let surface = columns.map(|(dx, dz)| {
        let x = (pos.x + dx).floor() as i32;
        let z = (pos.z + dz).floor() as i32;
        let wet = |y| chunk.get_world(IVec3::new(x, y, z)) == liquid;
        (row - 1..=row + 1)
            .rev()
            .find(|&y| wet(y))
            .map(|y| y as f32 + 1.0 - inset)
    });
    vertex.surface[1] = pack(0.0, corner_flow(surface) * speed);
}

/// Upload stamp for one vertex. An animated water vertex arrives carrying its
/// patch clock in `surface.x` and leaves carrying the clock of the frame its
/// mesh first draws, one frame after upload; a stale patch leaves `STILL`.
pub fn stamp(vertex: &mut Vertex3, now: f32, frame: f32) {
    if vertex.surface[2] % 64.0 != 8.0 || !(vertex.surface[1] >= PACKED) || vertex.surface[0] == STILL {
        return;
    }
    let age = now - vertex.surface[0];
    vertex.surface[0] = if (0.0..FRESH_SECONDS).contains(&age) { now + frame } else { STILL };
}

/// Clock and frame step for [`stamp`], read once per uploaded mesh.
pub fn upload_clock() -> (f32, f32) {
    clock()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packing_round_trips_step_and_current() {
        for step in [-1.0, -0.37, 0.0, 0.125, 0.86, 1.0] {
            for flow in [Vec2::ZERO, Vec2::new(0.4, -0.1), Vec2::new(-2.5, 2.5), Vec2::new(0.0, -FALL_SPEED)] {
                let value = pack(step, flow);
                assert!(value < 16_777_216.0 && value.fract() == 0.0, "packed {value} must be an exact f32 integer");
                let (s, f) = unpack(value).expect("packed");
                assert!((s - step).abs() <= 0.5 / 127.0, "step {step} -> {s}");
                // Square-root quantisation: error grows with speed, still under 4 %.
                assert!((f - flow).length() <= 0.04 * flow.length().max(0.1) + 0.02, "flow {flow} -> {f}");
            }
        }
        assert_eq!(unpack(0.42), None, "plain water specular is not motion");
        assert_eq!(unpack(pack(0.0, Vec2::ZERO)), Some((0.0, Vec2::ZERO)), "still water decodes exactly");
    }

    #[test]
    fn current_runs_downhill_and_ignores_dry_columns() {
        // Surface falls toward +x by 0.1 voxel per voxel.
        let flow = corner_flow([Some(5.3), Some(5.2), Some(5.3), Some(5.2)]);
        assert!(flow.x > 0.5 && flow.y.abs() < 1e-6, "{flow}");
        // A bank on one side gives no slope across it.
        let shore = corner_flow([None, Some(5.2), None, Some(5.2)]);
        assert_eq!(shore, Vec2::ZERO);
        assert_eq!(corner_flow([Some(4.0); 4]), Vec2::ZERO, "level water is still");
        let steep = corner_flow([Some(6.0), Some(5.0), Some(6.0), Some(5.0)]);
        assert!((steep.length() - FLOW_MAX).abs() < 1e-4, "a one-voxel ramp runs at the cap");
    }

    #[test]
    fn patches_keep_the_shown_fill_until_a_job_takes_it() {
        let p = IVec3::new(-33, 4, 70);
        let mut motion = WaterMotion::default();
        motion.record_at(p, 0.25, 10.0);
        // A second patch before any job: the screen still shows 0.25.
        motion.record_at(p, 0.5, 10.05);
        assert_eq!(motion.prev[&p], 0.25);
        motion.seal();
        motion.record_at(p, 0.75, 10.1);
        assert_eq!(motion.prev[&p], 0.75, "a dispatched job showed 0.5..0.75 already");
        assert_eq!(motion.vertex_clock(), 10.1);
        // A stale map is dropped rather than merged.
        motion.sealed = Arc::new(AtomicBool::new(false));
        motion.record_at(IVec3::ZERO, 1.0, 20.0);
        assert!(!motion.prev.contains_key(&p));
    }

    #[test]
    fn only_fresh_animated_water_is_stamped() {
        let vertex = |x: f32, y: f32, kind: f32| Vertex3 {
            pos: [0.0; 3], color: [0.0; 3], normal: [0.0, 1.0, 0.0],
            surface: [x, y, kind, 8.5], ao_strength: 1.0,
        };
        let moving = pack(0.4, Vec2::ZERO);
        let mut fresh = vertex(100.0, moving, 8.0);
        stamp(&mut fresh, 100.1, 0.016);
        assert_eq!(fresh.surface[0], 100.1 + 0.016);
        let mut stale = vertex(100.0, moving, 8.0);
        stamp(&mut stale, 101.0, 0.016);
        assert_eq!(stale.surface[0], STILL, "a LOD rebuild must not replay the step");
        let mut plain = vertex(0.1, 0.42, 8.0);
        stamp(&mut plain, 100.0, 0.016);
        assert_eq!(plain.surface[0], 0.1, "greedy water keeps its roughness");
        let mut stone = vertex(0.5, moving, 3.0);
        stamp(&mut stone, 100.1, 0.016);
        assert_eq!(stone.surface[0], 0.5);
    }
}
