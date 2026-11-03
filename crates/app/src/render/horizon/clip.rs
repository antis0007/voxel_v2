//! Chart clip: on the globe the horizon covers only the face under the eye.

use super::Horizon;

/// Chart rectangle `(x0, z0, x1, z1)` in metres.
pub type ChartClip = (f64, f64, f64, f64);

impl Horizon {
    /// Restricts the fine rings to `clip`; `None` lifts it. A change drops the cached base.
    pub fn set_chart_clip(&mut self, clip: Option<ChartClip>) {
        if self.chart_clip != clip {
            self.base = None;
        }
        self.chart_clip = clip;
    }

    pub fn chart_clip(&self) -> Option<ChartClip> {
        self.chart_clip
    }

    /// Surface height in metres at world (x, z) through the fine rings' sampler; NaN outside the clip.
    #[cfg(test)]
    pub fn sample_height_m(&self, x: f64, z: f64) -> f32 {
        let Some(cfg) = self.config else { return f32::NAN };
        let scale = f64::from(cfg.voxel_meters());
        let (x, z) = ((x / scale).floor() as i32, (z / scale).floor() as i32);
        super::fine::clipped_surface(&cfg, self.chart_clip, x, z).0 * scale as f32
    }

    #[cfg(test)]
    pub fn test_new() -> Horizon {
        let mut horizon = Horizon::default();
        horizon.config = Some(crate::world::WorldSampleConfig {
            generator_version: 18,
            seed: 74,
            world_type: crate::world::WorldType::Hills,
            voxel_scale: crate::fluid::VoxelScalePreset::OneMeter,
        });
        horizon
    }
}

/// `DrawBatch` centres and the clip are both world metres (chart metres on the globe).
pub fn within(clip: ChartClip, x: f64, z: f64) -> bool {
    (clip.0..=clip.2).contains(&x) && (clip.1..=clip.3).contains(&z)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn horizon_samples_outside_the_chart_clip_are_non_finite() {
        let mut h = Horizon::test_new();
        h.set_chart_clip(Some((-16_000.0, -16_000.0, 16_000.0, 16_000.0)));
        assert!(!h.sample_height_m(16_001.0, 0.0).is_finite());
        assert!(h.sample_height_m(15_999.0, 0.0).is_finite());
    }
}
