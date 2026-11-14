//! Motion-compensated playback between two retained history frames.
//!
//! Frames are fixed-grid snapshots a few Myr apart. Dissolving one into the
//! next makes coasts flip a coarse texel at a time while plates stand still.
//! Instead every display point follows its plate's recorded Euler rotation
//! back into the older frame and forward into the newer one, so crust slides
//! continuously and only newly formed or consumed crust fades.
//!
//! The globe shader does the carrying per pixel (`globe_carry`): the host only
//! uploads the newer frame's plate ownership and each plate's rotation, once
//! per frame interval. Playback between frames costs no CPU and has no steps.
use std::sync::{Arc, Mutex};

use engine_worldgen::planetary::DriftFrame;

/// Plates the shader's rotation table holds; snapshots carry at most 40.
pub const MAX_PLATES: usize = 64;

/// The newer frame's plate per texel and each plate's scaled-axis rotation
/// (radians) from the older frame to it.
pub struct Motion {
    pub key: u64,
    pub n: usize,
    pub plates: Vec<u16>,
    pub axes: Vec<[f32; 3]>,
}

static CACHE: Mutex<Option<Arc<Motion>>> = Mutex::new(None);

/// Motion into `to`, or `None` when it records none (older generators) or the
/// frames do not share a grid, leaving the caller's crossfade in place.
pub(in super::super) fn of(world: u64, from: &DriftFrame, to: &DriftFrame) -> Option<Arc<Motion>> {
    let n = to.n;
    if to.motion.is_empty() || to.motion.len() > MAX_PLATES || from.myr >= to.myr || n < 2
        || from.n != n || from.texels.len() != 6 * n * n || to.texels.len() != 6 * n * n
    {
        return None;
    }
    let key = world.rotate_left(29) ^ 0x2d1b_5a0e_7c39_c000 ^ u64::from(to.myr);
    let mut slot = CACHE.lock().ok()?;
    if let Some(m) = slot.as_ref() && m.key == key {
        return Some(Arc::clone(m));
    }
    let m = Arc::new(Motion {
        key,
        n,
        plates: to.texels.iter().map(|t| t.plate).collect(),
        axes: to.motion.clone(),
    });
    *slot = Some(Arc::clone(&m));
    Some(m)
}
