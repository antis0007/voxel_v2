//! Minimum-spec mode: `--low-spec` or `VOXEL_PRESET=minimum`.
//!
//! For machines with integrated graphics or little memory. It shrinks the
//! terrain radius and resolution, drops shadows, bounce lighting, the analytic
//! LUT sky, the distant horizon and the exact-edit proxy cover (the work a
//! "Editing proxies" stall comes from), and ignores the saved preferences file
//! so a bad earlier choice cannot undo it and the preset is never written back.
use crate::preferences::Preferences;
use std::sync::atomic::{AtomicBool, Ordering};

static ACTIVE: AtomicBool = AtomicBool::new(false);

pub fn requested() -> bool {
    std::env::args().skip(1).any(|a| a == "--low-spec")
        || std::env::var("VOXEL_PRESET").is_ok_and(|v| v.eq_ignore_ascii_case("minimum"))
}

pub fn active() -> bool {
    ACTIVE.load(Ordering::Relaxed)
}

/// Overwrite every cost-driving preference. Autosave stays on but is rare.
pub fn apply(p: &mut Preferences) {
    *p = Preferences {
        vsync: true,
        fxaa: false,
        atmosphere: 0,
        view: 6,
        lod: 3,
        levels: 2,
        cpu: 256,
        gpu: 0,
        mesh: 192,
        edited: 64,
        interval: 120,
        width: 1024,
        height: 576,
        fullscreen: false,
        cloud_scale: 4,
        // Volumetric clouds alone took 294 ms a frame on an Intel UHD 770.
        clouds: false,
        render_scale: 70,
        indirect_lighting: false,
        ..*p
    };
    ACTIVE.store(true, Ordering::Relaxed);
    crate::render::shadow::set_enabled(false);
    crate::render::sky_volume::set_enabled(false);
}

/// Integrated graphics, detected from the adapter. Unlike `apply`, this keeps
/// the window, memory budgets and the distant horizon, and full-detail terrain
/// reaches six chunks so nothing near the player is simplified. It turns off
/// what an integrated GPU cannot afford (measured on an Intel UHD 770 at
/// 1280x720: clouds 294 ms, terrain 64 ms at full resolution, 40 ms at 70%).
/// The settings are applied only when the player has no saved preferences;
/// shadows are not a saved setting, so they stay off on every launch.
pub fn apply_integrated(p: &mut Preferences, first_run: bool) {
    crate::render::shadow::set_enabled(false);
    crate::render::sky_volume::set_enabled(false);
    eprintln!("[quality] integrated GPU: shadows off{}",
        if first_run { "; first run, integrated profile applied" } else { "; saved preferences kept" });
    if !first_run {
        return;
    }
    *p = Preferences {
        clouds: false,
        cloud_scale: 4,
        indirect_lighting: false,
        render_scale: 70,
        view: 8,
        lod: 6,
        levels: 2,
        ..*p
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimum_preset_is_cheap_and_survives_sanitising() {
        let mut p = Preferences::default();
        apply(&mut p);
        assert!(p.view <= 8 && p.lod < p.view && p.lod >= 2);
        assert!(!p.indirect_lighting && !p.fxaa && p.atmosphere == 0 && !p.clouds);
        assert_eq!(p.cloud_scale, 4);
        assert!(active());
        assert!(!crate::render::shadow::enabled());
    }

    #[test]
    fn integrated_profile_keeps_near_terrain_and_a_saved_choice() {
        let mut p = Preferences::default();
        apply_integrated(&mut p, true);
        assert!(!p.clouds && !p.indirect_lighting && p.render_scale < 100);
        // Full detail must cover most of the view: no coarse terrain nearby.
        assert!(p.lod >= 6 && p.lod < p.view);
        let mut saved = Preferences { clouds: true, view: 20, ..Preferences::default() };
        apply_integrated(&mut saved, false);
        assert!(saved.clouds && saved.view == 20);
        assert!(!crate::render::shadow::enabled());
    }
}
