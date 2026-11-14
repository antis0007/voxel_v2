//! Pingos: ice-cored hills pushed up from flat wet permafrost.
//!
//! Groundwater freezing under a drained basin lifts a lens of ice that domes
//! the ground into a cone tens of metres high. Warmer ground melts the top of
//! the core, so the summit collapses into a crater.
use super::noise::{smoothstep, unit};

const SPACING_M: f64 = 320.0;
/// Continuous permafrost; discontinuous ground rarely holds a core.
const MAX_MAAT_C: f64 = -4.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pingo {
    /// Ground raise above the plain, metres.
    pub height: f64,
    /// Ice core thickness under this point, metres.
    pub core: f64,
}

/// The pingo at `(x, z)` for local mean annual temperature `maat`, if any.
pub fn at(seed: u32, x: f64, z: f64, maat: f64) -> Option<Pingo> {
    if maat >= MAX_MAAT_C {
        return None;
    }
    let (a, b) = ((x / SPACING_M).floor() as i64, (z / SPACING_M).floor() as i64);
    if unit(a, b, seed ^ 0x9190) > 0.22 {
        return None;
    }
    let sx = (a as f64 + 0.3 + 0.4 * unit(a, b, seed ^ 0x9191)) * SPACING_M;
    let sz = (b as f64 + 0.3 + 0.4 * unit(a, b, seed ^ 0x9192)) * SPACING_M;
    let radius = 22.0 + 40.0 * unit(a, b, seed ^ 0x9193);
    let k = ((x - sx).powi(2) + (z - sz).powi(2)).sqrt() / radius;
    if k >= 1.0 {
        return None;
    }
    let peak = 0.32 * radius;
    let dome = peak * (1.0 - k * k).powf(1.5);
    // The crater widens as the climate warms toward the core's limit.
    let thaw = smoothstep(-9.0, MAX_MAAT_C, maat);
    let crater = 0.35 * peak * thaw * (1.0 - smoothstep(0.0, 0.3 * thaw.max(0.2), k));
    let height = dome - crater;
    Some(Pingo { height, core: (0.75 * height - 1.5 - 0.4 * peak * thaw).max(0.0) })
}
