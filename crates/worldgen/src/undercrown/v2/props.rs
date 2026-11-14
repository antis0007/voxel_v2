//! Generator 292 prop models. Each prop kind gets a shape that says what it
//! does: a plinth with a lit cap, a hanging freight cage, a winch drum, a
//! sluice leaf in its frame, a bell clapper, the lens ring, a plaque on a
//! post, a banded counterweight. Older generators keep the shared pedestal.
//!
//! A model stands on the floor the pedestal stood on, 1.8 m under the prop
//! point, and owns a box (`model_box`) that the app also uses for focus.
use super::palette;
use crate::undercrown::plan::{PropKind, Style};
use crate::BlockId as B;
use glam::Vec3;

/// First generator with per-kind prop models. Dormant until bumped.
pub const VERSION: u32 = 292;

/// Height of the floor under the prop point.
const FLOOR_DROP: f32 = 1.8;

/// Half width and height of each model, in metres.
fn extent(kind: PropKind) -> (f32, f32) {
    match kind {
        PropKind::Plinth => (0.75, 2.2),
        PropKind::Cradle => (1.0, 3.6),
        PropKind::Winch => (1.0, 1.8),
        PropKind::SluiceGate => (1.0, 2.4),
        PropKind::BellClapper => (0.6, 2.6),
        PropKind::ObservatoryLens => (1.0, 2.4),
        PropKind::Marker => (0.6, 1.8),
        PropKind::Counterweight => (0.8, 2.6),
        PropKind::Cache => (0.8, 1.4),
        PropKind::Lectern => (0.6, 1.9),
    }
}

/// Half the coarsest voxel: the most grid snapping moves a model.
const SNAP_PAD: f32 = 0.5;

/// Floor centre a model stands on.
pub fn floor_point(pos: Vec3) -> Vec3 {
    Vec3::new(pos.x, pos.y - FLOOR_DROP, pos.z)
}

/// Metre box a model occupies around its prop point, padded so a model
/// snapped to the voxel grid never leaves it.
pub fn model_box(kind: PropKind, pos: Vec3) -> (Vec3, Vec3) {
    let (r, h) = extent(kind);
    let floor = floor_point(pos);
    let pad = Vec3::splat(SNAP_PAD);
    (floor - Vec3::new(r, 0.0, r) - pad, floor + Vec3::new(r, h, r) + pad)
}

/// Within `half` of the axis. Never thinner than one voxel: a cell centre
/// lies within half a voxel of any axis.
fn near(d: f32, half: f32, v: f32) -> bool {
    d.abs() <= half.max(v * 0.5)
}

/// Height `y` (a cell centre over a grid-snapped floor) lies in `lo..hi`,
/// with both edges rounded to whole voxels and the band at least one voxel
/// thick, so no part of a model falls between cell centres.
fn in_band(y: f32, lo: f32, hi: f32, v: f32) -> bool {
    let a = (lo / v).round() * v;
    let b = ((hi / v).round() * v).max(a + v);
    y >= a && y < b
}

/// Chain links alternate silver and copper every voxel.
fn chain(y: f32, v: f32) -> B {
    if (y / v).floor() as i32 % 2 == 0 { B::SilverBlock } else { B::CopperBlock }
}

/// Block of a model standing on `floor` at the point `m`, for voxels of
/// `v` metres. `None` leaves the room as it is.
pub fn sample(kind: PropKind, style: Style, floor: Vec3, m: Vec3, v: f32) -> Option<B> {
    let [_, trim, stone, _] = palette(style);
    // The axis sits on a cell centre and the floor on a cell face, so every
    // model is symmetric about one column of voxels at any scale.
    let axis = |a: f32| ((a / v).floor() + 0.5) * v;
    let (x, z) = (m.x - axis(floor.x), m.z - axis(floor.z));
    let y = m.y - (floor.y / v).round() * v;
    let band = |lo: f32, hi: f32| in_band(y, lo, hi, v);
    match kind {
        PropKind::Plinth => {
            if band(0.0, 0.4) && near(x, 0.75, v) && near(z, 0.75, v) {
                Some(trim)
            } else if band(0.4, 1.2) && near(x, 0.45, v) && near(z, 0.45, v) {
                Some(stone)
            } else if near(x, 0.3, v) && near(z, 0.3, v) && band(1.2, 2.2) {
                // A lit crystal standing up out of a copper cap.
                Some(B::Crystal)
            } else if band(1.2, 1.6) && near(x, 0.6, v) && near(z, 0.6, v) {
                Some(B::CopperBlock)
            } else {
                None
            }
        }
        PropKind::Cradle => {
            let corner = !near(x, 0.7, v) && !near(z, 0.7, v);
            if band(0.0, 0.4) {
                Some(B::Planks)
            } else if band(0.4, 2.2) && corner {
                Some(B::IronBlock)
            } else if band(2.2, 2.6) && (!near(x, 0.7, v) || !near(z, 0.7, v)) {
                Some(B::IronBlock)
            } else if band(2.6, 3.6) && near(x, 0.25, v) && near(z, 0.25, v) {
                Some(chain(y, v))
            } else {
                None
            }
        }
        PropKind::Winch => {
            let post = !near(x, 0.7, v) && near(z, 0.3, v) && band(0.0, 1.6);
            let drum = near(x, 0.7, v) && (y - 1.1).hypot(z) <= 0.5f32.max(v * 0.5);
            if post {
                Some(B::Planks)
            } else if drum {
                // Rope wound on a copper-hubbed drum.
                Some(if near(x, 0.15, v) { B::CopperBlock } else { B::Planks })
            } else {
                None
            }
        }
        PropKind::SluiceGate => {
            if !near(z, 0.4, v) {
                None
            } else if !near(x, 0.7, v) || band(2.0, 2.4) {
                Some(stone)
            } else if band(0.0, 2.0) {
                Some(B::Planks)
            } else {
                None
            }
        }
        PropKind::BellClapper => {
            if band(1.4, 2.6) && near(x, 0.2, v) && near(z, 0.2, v) {
                Some(chain(y, v))
            } else if Vec3::new(x, y - 0.9, z).length() <= 0.55f32.max(v * 0.5) {
                Some(B::BronzeBlock)
            } else {
                None
            }
        }
        PropKind::ObservatoryLens => {
            let r = (y - 1.6).hypot(x);
            if band(0.0, 1.0) && near(x, 0.3, v) && near(z, 0.3, v) {
                Some(stone)
            } else {
                // A crystal lens in a copper ring; at coarse scales the lens
                // keeps its centre voxel and the ring sits one voxel out.
                let lens = 0.5f32.max(v * 0.75);
                let ring = 0.85f32.max(lens + v);
                (near(z, 0.3, v) && r < ring).then_some(if r < lens { B::Crystal } else { B::CopperBlock })
            }
        }
        PropKind::Marker => {
            if band(0.0, 0.9) && near(x, 0.2, v) && near(z, 0.2, v) {
                Some(B::Planks)
            } else if band(0.9, 1.7) && near(x, 0.6, v) && near(z, 0.15, v) {
                Some(trim)
            } else {
                None
            }
        }
        PropKind::Cache => {
            // A plank strongbox on a stone sill, copper bands round it and a
            // lit crystal clasp on the front so it reads in the dark.
            if band(0.0, 0.2) && near(x, 0.8, v) && near(z, 0.6, v) {
                Some(stone)
            } else if band(0.2, 1.2) && near(x, 0.7, v) && near(z, 0.5, v) {
                let banded = !near(x, 0.45, v) || band(0.75, 0.95);
                let clasp = near(x, 0.15, v) && band(0.55, 0.95) && !near(z, 0.35, v) && z < 0.0;
                Some(if clasp { B::Crystal } else if banded { B::CopperBlock } else { B::Planks })
            } else {
                None
            }
        }
        PropKind::Lectern => {
            // A post on a trim foot carrying a slanted board with a lit page.
            if band(0.0, 0.2) && near(x, 0.45, v) && near(z, 0.45, v) {
                Some(trim)
            } else if band(0.2, 1.2) && near(x, 0.15, v) && near(z, 0.15, v) {
                Some(B::Planks)
            } else if band(1.2, 1.6) && near(x, 0.55, v) && near(z, 0.4, v) {
                Some(if near(x, 0.3, v) && near(z, 0.2, v) { B::StarGlass } else { B::Planks })
            } else {
                None
            }
        }
        PropKind::Counterweight => {
            if band(0.2, 1.4) && near(x, 0.7, v) && near(z, 0.7, v) {
                Some(if band(0.7, 0.9) { B::CopperBlock } else { B::Basalt })
            } else if band(1.4, 2.6) && near(x, 0.2, v) && near(z, 0.2, v) {
                Some(chain(y, v))
            } else {
                None
            }
        }
    }
}
