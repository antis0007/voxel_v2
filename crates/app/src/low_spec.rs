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
        render_scale: 70,
        indirect_lighting: false,
        ..*p
    };
    ACTIVE.store(true, Ordering::Relaxed);
    crate::render::shadow::set_enabled(false);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimum_preset_is_cheap_and_survives_sanitising() {
        let mut p = Preferences::default();
        apply(&mut p);
        assert!(p.view <= 8 && p.lod < p.view && p.lod >= 2);
        assert!(!p.indirect_lighting && !p.fxaa && p.atmosphere == 0);
        assert_eq!(p.cloud_scale, 4);
        assert!(active());
        assert!(!crate::render::shadow::enabled());
    }
}
