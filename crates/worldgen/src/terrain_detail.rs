//! Generator 38 residual relief, applied AFTER the 8 m erosion grid.
//! All distances/amplitudes are metres. Profiles blend continuously through
//! climate transitions; discrete biome labels must never introduce height seams.
use super::{WorldSampleConfig, hash2, landscape::Biome, smoothstep};

// Coherent landforms, hummocks/rills, broken ground, then sub-metre roughness.
const WAVELENGTHS: [f32; 4] = [32.0, 9.0, 2.8, 0.9];

#[derive(Clone, Copy)]
struct Profile {
    amplitude: [f32; 4],
    rock: f32,
    wind: f32,
}

impl Profile {
    const fn new(amplitude: [f32; 4], rock: f32, wind: f32) -> Self {
        Self {
            amplitude,
            rock,
            wind,
        }
    }

    fn blend(self, other: Self, weight: f32) -> Self {
        let t = weight.clamp(0.0, 1.0);
        Self {
            amplitude: std::array::from_fn(|i| {
                self.amplitude[i] + (other.amplitude[i] - self.amplitude[i]) * t
            }),
            rock: self.rock + (other.rock - self.rock) * t,
            wind: self.wind + (other.wind - self.wind) * t,
        }
    }
}

/// Tune a biome here, independently of sampling, filtering, and climate masks.
fn profile(biome: Biome) -> Profile {
    use Biome::*;
    let (amplitude, rock, wind) = match biome {
        Ocean => ([0.35, 0.14, 0.06, 0.02], 0.0, 0.25),
        Coast => ([0.45, 0.20, 0.10, 0.03], 0.0, 0.65),
        Marsh => ([0.38, 0.30, 0.16, 0.05], 0.0, 0.0),
        Fen => ([0.30, 0.38, 0.20, 0.06], 0.0, 0.0),
        Meadow => ([1.45, 0.65, 0.24, 0.07], 0.0, 0.0),
        Forest => ([2.10, 1.05, 0.42, 0.13], 0.08, 0.0),
        Rainforest => ([2.65, 1.35, 0.48, 0.14], 0.12, 0.0),
        Taiga => ([1.80, 0.95, 0.38, 0.12], 0.18, 0.0),
        Tundra => ([1.10, 0.70, 0.30, 0.08], 0.12, 0.12),
        Heathland => ([2.30, 1.20, 0.46, 0.12], 0.32, 0.0),
        Savanna => ([1.60, 0.72, 0.28, 0.08], 0.12, 0.12),
        Desert => ([3.60, 0.95, 0.22, 0.04], 0.0, 0.92),
        Badlands => ([4.20, 1.85, 0.66, 0.18], 0.85, 0.05),
        Alpine => ([5.20, 2.30, 0.85, 0.23], 1.0, 0.0),
    };
    Profile::new(amplitude, rock, wind)
}

fn climate_profile(height: f32, slope: f32, heat: f32, wet: f32) -> Profile {
    let mut p = profile(Biome::Meadow).blend(profile(Biome::Forest), smoothstep(0.46, 0.66, wet));
    p = p.blend(
        profile(Biome::Rainforest),
        smoothstep(0.52, 0.66, heat) * smoothstep(0.62, 0.80, wet),
    );
    p = p.blend(
        profile(Biome::Savanna),
        smoothstep(0.50, 0.64, heat) * (1.0 - smoothstep(0.40, 0.55, wet)),
    );
    p = p.blend(
        profile(Biome::Heathland),
        (1.0 - smoothstep(0.36, 0.48, heat))
            * (1.0 - smoothstep(0.50, 0.64, wet))
            * smoothstep(28.0, 45.0, height),
    );
    let cold = profile(Biome::Tundra).blend(profile(Biome::Taiga), smoothstep(0.34, 0.50, wet));
    p = p.blend(cold, 1.0 - smoothstep(0.18, 0.30, heat));
    let dry = profile(Biome::Desert).blend(profile(Biome::Badlands), smoothstep(0.12, 0.34, slope));
    p = p.blend(
        dry,
        smoothstep(0.57, 0.70, heat) * (1.0 - smoothstep(0.30, 0.44, wet)),
    );
    let wetland = profile(Biome::Marsh).blend(profile(Biome::Fen), smoothstep(0.68, 0.84, wet));
    p = p.blend(
        wetland,
        smoothstep(0.56, 0.80, wet)
            * (1.0 - smoothstep(0.08, 0.22, slope))
            * (1.0 - smoothstep(19.0, 27.0, height)),
    );
    let alpine = smoothstep(82.0, 132.0, height)
        .max(smoothstep(0.40, 0.95, slope) * smoothstep(35.0, 85.0, height));
    p = p.blend(profile(Biome::Alpine), alpine);
    p = profile(Biome::Coast).blend(p, smoothstep(14.0, 20.0, height));
    profile(Biome::Ocean).blend(p, smoothstep(7.0, 12.0, height))
}

/// Signed C2 noise and its analytic gradient. Four hashes per resolved band;
/// no neighbouring column queries, erosion tiles, allocations, or recursion.
fn noise_gradient(x: f32, z: f32, seed: u32) -> (f32, [f32; 2]) {
    let ix = x.floor() as i32;
    let iz = z.floor() as i32;
    let fx = x - x.floor();
    let fz = z - z.floor();
    let fade = |t: f32| t * t * t * (t * (t * 6.0 - 15.0) + 10.0);
    let derivative = |t: f32| 30.0 * t * t * (t - 1.0) * (t - 1.0);
    let u = fade(fx);
    let v = fade(fz);
    let a = hash2(ix, iz, seed);
    let b = hash2(ix.wrapping_add(1), iz, seed) - a;
    let c = hash2(ix, iz.wrapping_add(1), seed) - a;
    let d = hash2(ix.wrapping_add(1), iz.wrapping_add(1), seed) - a - b - c;
    (
        2.0 * (a + b * u + c * v + d * u * v) - 1.0,
        [
            2.0 * derivative(fx) * (b + d * v),
            2.0 * derivative(fz) * (c + d * u),
        ],
    )
}

/// Returns height displacement and local slope energy. The latter is combined
/// with the erosion slope by RMS: its directional gradient is not stored there.
pub(super) fn sample(
    cfg: &WorldSampleConfig,
    wx: f32,
    wz: f32,
    height: f32,
    slope: f32,
    heat: f32,
    wet: f32,
    sediment: f32,
    footprint: f32,
) -> (f32, f32) {
    // The farthest preview paths resolve none of these bands. Avoid climate
    // blending and trigonometry entirely for their many height-map samples.
    if footprint >= WAVELENGTHS[0] * 0.5 {
        return (0.0, 0.0);
    }
    // Keep swash zones quiet, and protect the walkable spawn clearing. River,
    // valley and authored landmark shaping run after this residual layer.
    let gate = smoothstep(18.0, 70.0, wx.hypot(wz))
        * smoothstep(0.8, 3.5, (height - super::WATER_LEVEL_METERS).abs());
    if gate == 0.0 {
        return (0.0, 0.0);
    }
    let p = climate_profile(height, slope, heat, wet);
    let soil_cover = 1.0 - sediment.clamp(0.0, 2.0) * 0.16;
    let angle = hash2(0, 0, cfg.seed.wrapping_add(10007)) * std::f32::consts::TAU;
    let (sin, cos) = angle.sin_cos();
    let along = wx * cos + wz * sin;
    let across = -wx * sin + wz * cos;
    let mut height_delta = 0.0;
    let mut gradient = [0.0; 2];
    for (i, wavelength) in WAVELENGTHS.into_iter().enumerate() {
        // Fade before Nyquist. Never shrink wavelengths with viewing distance.
        let resolved = 1.0 - smoothstep(0.20, 0.50, footprint.max(0.0) / wavelength);
        if resolved == 0.0 {
            continue;
        }
        let seed = cfg.seed.wrapping_add(10009 + i as u32 * 101);
        let (mut n, mut dn) = noise_gradient(along / wavelength, across / wavelength, seed);
        if p.wind > 0.0 {
            let (wind, wind_gradient) =
                noise_gradient(along / wavelength, across * 0.28 / wavelength, seed);
            // Blend fixed fields, never their world coordinates. Multiplying
            // kilometre-scale coordinates by a changing biome weight creates
            // arbitrarily high frequencies along otherwise smooth ecotones.
            n += (wind - n) * p.wind;
            dn[0] += (wind_gradient[0] - dn[0]) * p.wind;
            dn[1] += (wind_gradient[1] * 0.28 - dn[1]) * p.wind;
        }
        // Smooth, bounded ribs with incised hollows; no abs/floor cusps. Wind
        // stretches the same field into coherent dunes rather than random grit.
        let shape = n + p.rock * (0.30 - n * n) * 0.8;
        let shape_derivative = 1.0 - p.rock * n * 1.6;
        // Keep lowland soil walkable; reserve broken ground for exposed uplands.
        let walkable = if cfg.generator_version >= 40 {
            let exposed = smoothstep(0.22, 0.65, slope).max(smoothstep(65.0, 120.0, height));
            let lowland = [0.65, 0.30, 0.12, 0.05][i];
            lowland + (1.0 - lowland) * exposed
        } else { 1.0 };
        let amplitude = p.amplitude[i] * resolved * gate * walkable * if i >= 2 { soil_cover } else { 1.0 };
        height_delta += shape * amplitude;
        let gain = shape_derivative * amplitude / wavelength;
        gradient[0] += dn[0] * gain;
        gradient[1] += dn[1] * gain;
    }
    (height_delta, gradient[0].hypot(gradient[1]))
}
