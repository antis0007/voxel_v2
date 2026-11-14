//! Physical habitat first, stand history second. Fixed survey distances keep
//! admission independent of terrain LOD. This never calls terrain generation.
use super::*;

pub(super) fn blend(cfg: &WorldSampleConfig, x: f32, z: f32, base: Biome) -> (Biome, f32) {
    use Biome::*;
    if x.hypot(z) < 130.0 || matches!(base, Ocean | Alpine | Badlands | Desert | Tundra) {
        return (base, 0.0);
    }
    let p = crate::geo::planet_for(cfg);
    let (wx, wz) = (x as f64, z as f64);
    let height = |dx, dz| {
        let (px, pz) = (wx + dx, wz + dz);
        let raw = p.grid.bilinear(&p.height, px, pz);
        crate::vistas::height(cfg, px as f32, pz as f32, raw) as f32
    };
    let h = height(0.0, 0.0);
    let ring = [
        height(96.0, 0.0),
        height(-96.0, 0.0),
        height(0.0, 96.0),
        height(0.0, -96.0),
    ];
    let relative = h - ring.iter().sum::<f32>() * 0.25;
    let relief = ring.iter().copied().fold(h, f32::max) - ring.iter().copied().fold(h, f32::min);
    let grade = ((height(24.0, 0.0) - height(-24.0, 0.0)).powi(2)
        + (height(0.0, 24.0) - height(0.0, -24.0)).powi(2))
    .sqrt()
        / 48.0;
    let sea_c = p.grid.bilinear(&p.maat, wx, wz) as f32
        + crate::geo::HOME.lapse_c_per_m as f32
            * (p.grid.bilinear(&p.height, wx, wz) as f32 - crate::WATER_LEVEL_METERS).max(0.0);
    let (sea_heat, wet) = regional_ecology(
        cfg,
        x,
        z,
        (sea_c + 15.0) / 45.0,
        p.grid.bilinear(&p.wet, wx, wz) as f32,
    );
    let heat = lapse_heat(cfg, sea_heat, h);
    let band = |v, lo0, lo1, hi0, hi1| smoothstep(lo0, lo1, v) * (1.0 - smoothstep(hi0, hi1, v));
    let history = value_noise2(
        (x * 0.82 - z * 0.57) / 240.0,
        (x * 0.57 + z * 0.82) / 240.0,
        cfg.seed.wrapping_add(19503),
    );
    let shelter = 1.0 - smoothstep(0.0, 5.0, relative);
    let valley = 1.0 - smoothstep(-5.0, 1.0, relative);
    let flat = 1.0 - smoothstep(0.10, 0.30, grade);
    let runoff = smoothstep(
        2000.0,
        60000.0,
        p.grid.bilinear(&p.discharge, wx, wz) as f32,
    );
    let wetland = smoothstep(0.65, 0.84, wet) * flat * valley.max(runoff * 0.90);
    if base == Coast {
        let shore = (1.0 - smoothstep(0.40, 0.56, heat)) * smoothstep(0.30, 0.62, history);
        return (LastShore, smoothstep(0.18, 0.78, shore));
    }
    let mut best = (base, 0.0f32);
    let mut admit = |biome, score: f32| {
        if score > best.1 {
            best = (biome, score);
        }
    };
    // A shelf is flatter than its surrounding relief and stands above it.
    let shelf = flat * smoothstep(0.5, 5.0, relative) * smoothstep(6.0, 22.0, relief);
    admit(
        LostPlateau,
        shelf
            * band(h, 48.0, 70.0, 175.0, 200.0)
            * smoothstep(0.40, 0.56, heat)
            * smoothstep(0.48, 0.70, wet),
    );
    admit(
        CycadMarsh,
        wetland * smoothstep(0.45, 0.62, heat) * (1.0 - smoothstep(70.0, 100.0, h)),
    );
    let (cloud_height, cloud_relief) = if cfg.generator_version >= engine_voxel::TALL_WOODLAND_GENERATOR {
        // Include humid shoulders below the steep summit belt. Those slopes
        // offer stable soil for a stand while the surrounding ridges trap mist.
        (smoothstep(49.0, 81.0, h), smoothstep(3.0, 14.0, relief))
    } else {
        (smoothstep(65.0, 110.0, h), smoothstep(3.0, 18.0, relief))
    };
    admit(
        CloudForest,
        smoothstep(0.59, 0.80, wet)
            * band(heat, 0.20, 0.30, 0.52, 0.65)
            * cloud_height
            * cloud_relief
            * (1.0 - smoothstep(0.50, 0.85, grade)),
    );
    // Mature groves need drainage; soggy bottoms belong to wetlands.
    admit(
        RedwoodGrove,
        band(heat, 0.26, 0.38, 0.57, 0.70)
            * band(wet, 0.48, 0.62, 0.84, 0.96)
            * (1.0 - wetland)
            * (1.0 - smoothstep(0.45, 0.75, grade))
            * smoothstep(0.32, 0.66, history)
            * smoothstep(15.0, 30.0, h),
    );
    let decay = value_noise2(x / 85.0, z / 85.0, cfg.seed.wrapping_add(19511));
    if matches!(base, Forest | Rainforest) {
        admit(
            FungalGrove,
            smoothstep(0.68, 0.86, wet)
                * shelter
                * flat
                * smoothstep(0.62, 0.83, decay)
                * band(heat, 0.25, 0.36, 0.72, 0.85),
        );
    }
    // Gardens await authored remnants; ordinary meadows keep their identity.
    let core = smoothstep(130.0, 210.0, x.hypot(z));
    (best.0, smoothstep(0.16, 0.80, best.1) * core)
}
