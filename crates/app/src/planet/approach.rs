//! Approach band around a foreign body's sphere: frame blend, upright handoff, exact velocities.
use super::{BodyId, DVec3, atlas, system};

/// Planet anchors start this many sphere radii out. Between there and the
/// sphere the reference frame blends from home's to the body's, so the
/// handoff spreads over kilometres instead of one step.
pub(super) const APPROACH_BAND: f64 = 2.0;
/// A release still turning its upright toward home happens here regardless.
pub(super) const FORCE_RELEASE: f64 = 3.0;
/// Upright error under which home's frame takes over without a visible roll.
pub(super) const RELEASE_ALIGN_RAD: f64 = 0.005;
pub(super) const TICK_SECONDS: f64 = 1.0 / 120.0;
/// Most upright turn per step that carries the view with it: following
/// curvature at cruise speed 10 km from a moon's centre needs about 0.0025.
pub(super) const CARRY_RAD: f64 = 0.0025;

/// Body share of the reference frame: 1 inside the sphere, 0 at the band edge.
pub(super) fn body_weight(distance: f64, soi: f64, band: f64) -> f64 {
    if band <= 1.0 || distance <= soi {
        return 1.0;
    }
    let t = ((band * soi - distance) / ((band - 1.0) * soi)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Foreign body whose approach band holds an atlas point, nearest in sphere radii.
pub fn approached_body(atlas_m: DVec3, tick: u64) -> Option<BodyId> {
    let system = system()?;
    let home = system.id("home")?;
    super::super::rails::with_snapshot(system, tick, |snapshot| {
        let sample = snapshot.position(home) + system.orientation(home, tick) * atlas::to_body(atlas_m);
        (0..system.len() as u16)
            .map(BodyId)
            .filter(|&b| b != home && system.parent(b).is_some())
            .filter_map(|b| Some((b, snapshot.position(b).distance(sample) / system.soi(b)?)))
            .filter(|&(_, radii)| radii <= APPROACH_BAND)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(b, _)| b)
    })
}

/// Unit view direction in the upright frame.
pub(super) fn view_dir(player: &crate::player::Player) -> DVec3 {
    let (sy, cy) = (player.yaw as f64).sin_cos();
    let (sp, cp) = (player.pitch as f64).sin_cos();
    DVec3::new(sy * cp, sp, -cy * cp)
}

/// Points yaw and pitch along an upright-frame direction.
// No mouse pitch limit here: a handoff can sweep the up through the line of
// sight, and a limit would drag the view; the next mouse move re-limits it.
pub(super) fn aim(player: &mut crate::player::Player, local: DVec3) {
    player.yaw = local.x.atan2(-local.z) as f32;
    player.pitch = local.y.clamp(-1.0, 1.0).asin() as f32;
}
