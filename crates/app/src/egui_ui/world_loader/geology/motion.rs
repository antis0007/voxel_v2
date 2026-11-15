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
//! The shader also draws the newer frame's plate seams from that ownership at
//! screen resolution (`globe_seam_line`), coloured by `Motion::tint`.
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
    /// Margin colour (linear rgb) and seam strength (a) on both sides of
    /// every plate boundary, zero elsewhere.
    pub tint: Vec<[u8; 4]>,
}

static CACHE: Mutex<Option<Arc<Motion>>> = Mutex::new(None);

/// Motion into `to`, or `None` when it records none (older generators) or the
/// frames do not share a grid, leaving the caller's crossfade in place.
pub(in super::super) fn of(world: u64, from: &DriftFrame, to: &DriftFrame, colours: &[[f32; 3]; 3])
    -> Option<Arc<Motion>>
{
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
    let probe_t = std::time::Instant::now(); // PROBE
    let plates: Vec<u16> = to.texels.iter().map(|t| t.plate).collect();
    let tint = tint(n, &plates, to, colours);
    let m = Arc::new(Motion { key, n, plates, axes: to.motion.clone(), tint });
    eprintln!("[geology-probe] motion n={} {:.2} ms", n, probe_t.elapsed().as_secs_f64() * 1e3); // PROBE
    *slot = Some(Arc::clone(&m));
    Some(m)
}

/// Two-sided seam tint: each texel touching another plate carries its margin
/// colour, so the field is full strength along the boundary contour wherever
/// the shader's line lands on it.
fn tint(n: usize, plates: &[u16], to: &DriftFrame, colours: &[[f32; 3]; 3]) -> Vec<[u8; 4]> {
    let unit = |v: f32| (255.0 * v.clamp(0.0, 1.0)).round() as u8;
    (0..plates.len())
        .map(|i| {
            let (x, z) = (i % n, i / n % n);
            let own = plates[i];
            let differs = |j: usize| plates[j] != own;
            let edge = (x > 0 && differs(i - 1)) || (x + 1 < n && differs(i + 1))
                || (z > 0 && differs(i - n)) || (z + 1 < n && differs(i + n));
            if !edge {
                return [0; 4];
            }
            let (rgb, strength) = super::margin_tint(to.texels[i].stress, colours);
            [unit(rgb[0]), unit(rgb[1]), unit(rgb[2]), unit(strength)]
        })
        .collect()
}
