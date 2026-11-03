//! Transition band: the resident-chunk mask the globe discards under, and the horizon's face clip.

use super::chart::{self, HALF_M, PITCH_M};
use crate::altitude::{LOAD_STOP_M, Tier};
use crate::render::coverage::ResidentCoverage;
use glam::DVec3;

/// Width of the fade from the resident disc edge to an opaque globe; `GLOBE_FADE_M` in globe_mask.wgsl.
pub const FADE_M: f32 = 256.0;

/// `GlobeMask` in globe_mask.wgsl.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GlobeMask {
    /// The same ownership map the world shader reads from the camera uniform.
    pub coverage: ResidentCoverage,
    /// x: chunk column width in metres; y: resident disc radius in metres, at most 0 for no fade;
    /// zw: eye chart x/z in metres.
    pub params: [f32; 4],
}
const _: [(); 10336] = [(); std::mem::size_of::<GlobeMask>()];

impl GlobeMask {
    pub fn new(coverage: ResidentCoverage, chunk_m: f32, disc_radius_m: f32, eye_m: DVec3) -> GlobeMask {
        // A non-finite radius means no published coverage, so nothing fades.
        let radius = if disc_radius_m.is_finite() { disc_radius_m } else { 0.0 };
        GlobeMask { coverage, params: [chunk_m, radius, eye_m.x as f32, eye_m.z as f32] }
    }
}

/// The horizon's chart clip this frame: the face under the eye in the Transition tier below
/// the load stop, `None` elsewhere. `keep` is false where the globe alone covers the ground.
pub fn horizon_clip(tier: Tier, eye_m: DVec3) -> (Option<(f64, f64, f64, f64)>, bool) {
    match tier {
        Tier::Surface => (None, true),
        Tier::Transition if eye_m.y < LOAD_STOP_M => {
            let (face, _) = chart::split(eye_m);
            let x = f64::from(face as u8) * PITCH_M;
            (Some((x - HALF_M, -HALF_M, x + HALF_M, HALF_M)), true)
        }
        _ => (None, false),
    }
}

/// Whether distant ground is ready: off the surface tier the globe is the far field, so the
/// loading hold never fires mid-flight.
pub fn horizon_ready(globe_active: bool, tier: Tier, horizon_mesh: bool) -> bool {
    (globe_active && tier != Tier::Surface) || horizon_mesh
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn horizon_clip_is_the_face_below_the_load_stop_and_off_above_it() {
        let eye = DVec3::new(2.0 * PITCH_M + 100.0, 4_000.0, -50.0);
        let x = 2.0 * PITCH_M;
        assert_eq!(horizon_clip(Tier::Transition, eye), (Some((x - HALF_M, -HALF_M, x + HALF_M, HALF_M)), true));
        assert_eq!(horizon_clip(Tier::Transition, DVec3::new(0.0, 12_000.0, 0.0)), (None, false));
        assert_eq!(horizon_clip(Tier::Orbit, DVec3::new(0.0, 60_000.0, 0.0)), (None, false));
        assert_eq!(horizon_clip(Tier::Surface, DVec3::new(0.0, 100.0, 0.0)), (None, true));
    }

    #[test]
    fn shader_fade_width_matches_the_host() {
        let source = crate::shader_sources::globe_shader_source();
        assert!(source.contains(&format!("const GLOBE_FADE_M: f32 = {FADE_M:.1};")));
    }

    #[test]
    fn horizon_is_ready_off_the_surface_tier_with_the_globe() {
        assert!(horizon_ready(true, Tier::Transition, false));
        assert!(horizon_ready(true, Tier::Orbit, false));
        assert!(!horizon_ready(true, Tier::Surface, false));
        assert!(!horizon_ready(false, Tier::Transition, false));
        assert!(horizon_ready(false, Tier::Surface, true));
    }

    #[test]
    fn mask_without_published_coverage_does_not_fade() {
        let mask = GlobeMask::new(ResidentCoverage::default(), 32.0, f32::INFINITY, DVec3::new(10.0, 0.0, 20.0));
        assert_eq!(mask.params, [32.0, 0.0, 10.0, 20.0]);
    }
}
