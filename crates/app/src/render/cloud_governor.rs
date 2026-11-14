//! Adaptive cloud resolution. The cloud raymarch dominates GPU time and scales
//! with pixel count, so when measured GPU time stays over budget the governor
//! raises the cloud divisor by one step (fewer cloud pixels), and lowers it
//! again only after a long stretch with real headroom. Asymmetric windows keep
//! it from oscillating, since every change recreates the cloud targets.

/// GPU pass total above this (ms) counts as over budget.
const OVER_MS: f32 = 14.0;
/// GPU pass total below this (ms) counts as headroom.
const UNDER_MS: f32 = 8.0;
const OVER_SECONDS: f32 = 1.5;
const UNDER_SECONDS: f32 = 10.0;
/// Frames longer than this are hitches (streaming, load), not sustained cost.
const HITCH_MS: f32 = 60.0;
pub const MAX_BUMP: u32 = 2;

#[derive(Debug, Default)]
pub struct CloudGovernor {
    over_s: f32,
    under_s: f32,
    bump: u32,
}

impl CloudGovernor {
    pub fn bump(&self) -> u32 {
        self.bump
    }

    /// Terrain/shadow uploads should not reduce cloud detail when the march
    /// itself is already cheap. Only sustained reducible work pays for a drop.
    pub fn update_cloud_cost(&mut self, gpu_ms: f32, cloud_ms: f32, dt: f32) -> bool {
        if !cloud_ms.is_finite() || cloud_ms < 0.0 { return false; }
        if gpu_ms > OVER_MS && cloud_ms < 3.0 {
            self.over_s=0.0;
            self.under_s=0.0;
            return false;
        }
        self.update(gpu_ms,dt)
    }

    /// Feed one frame. `gpu_ms` is the measured GPU pass total, `dt` the
    /// wall-clock seconds the frame took. Returns true when the bump changed.
    pub fn update(&mut self, gpu_ms: f32, dt: f32) -> bool {
        if !gpu_ms.is_finite() || gpu_ms <= 0.0 || gpu_ms > HITCH_MS || !dt.is_finite() || dt <= 0.0 {
            return false;
        }
        let dt = dt.min(0.25);
        if gpu_ms > OVER_MS {
            self.over_s += dt;
            self.under_s = 0.0;
        } else if gpu_ms < UNDER_MS {
            self.under_s += dt;
            self.over_s = 0.0;
        } else {
            self.over_s = (self.over_s - dt).max(0.0);
            self.under_s = (self.under_s - dt).max(0.0);
        }
        if self.over_s >= OVER_SECONDS && self.bump < MAX_BUMP {
            self.bump += 1;
            self.over_s = 0.0;
            return true;
        }
        if self.under_s >= UNDER_SECONDS && self.bump > 0 {
            self.bump -= 1;
            self.under_s = 0.0;
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raises_after_sustained_load_only() {
        let mut g = CloudGovernor::default();
        for _ in 0..60 {
            g.update(30.0, 0.016);
        }
        assert_eq!(g.bump(), 0, "under a second is not sustained");
        for _ in 0..60 {
            g.update(30.0, 0.016);
        }
        assert_eq!(g.bump(), 1);
    }

    #[test]
    fn hitch_does_not_count() {
        let mut g = CloudGovernor::default();
        for _ in 0..200 {
            g.update(120.0, 0.12);
        }
        assert_eq!(g.bump(), 0);
    }

    #[test]
    fn recovers_slowly_and_caps() {
        let mut g = CloudGovernor::default();
        for _ in 0..2000 {
            g.update(30.0, 0.016);
        }
        assert_eq!(g.bump(), MAX_BUMP);
        for _ in 0..300 {
            g.update(5.0, 0.016);
        }
        assert_eq!(g.bump(), MAX_BUMP, "4.8 s of headroom must not release");
        for _ in 0..2000 {
            g.update(5.0, 0.016);
        }
        assert_eq!(g.bump(), 0);
    }
}
