//! Per-pixel detail for distant terrain.
//!
//! The backdrop lattice has 128-512 m cells beyond about 6 km, so its vertex
//! normals light a mountain range as a few smooth blobs. This builds a small
//! nested set of height maps (four levels, 16-128 m texels, 512x512 each) from
//! the same filtered far-field evaluator the lattice uses. The world shader
//! reads them for:
//!
//! - shading normals at texel resolution (ridges, spurs and gullies),
//! - a broad cavity term (valleys darker, crests catch sky),
//! - sun visibility by marching the height field toward the sun, so ranges
//!   shade each other at low sun, far beyond the shadow cascades.
//!
//! Texels are world anchored: the map centre is the horizon centre, which is
//! snapped to 512 m, a whole number of texels at every level, so recentring
//! never shifts a texel. Built once per base cover (a recentre), never on edit
//! rebuilds, and each level is sampled with its own texel as the filter
//! footprint, so it does not alias.
use super::parallel;
use crate::world::WorldSampleConfig;
use std::sync::atomic::{AtomicBool, Ordering};

pub(crate) const LEVELS: usize = 4;
pub(crate) const SIZE: usize = 512;
pub(crate) const BASE_TEXEL_M: f32 = 16.0;
/// Extra samples around each level for central differences and the cavity ring.
const BORDER: usize = 2;

/// Packed RGBA16F texels, level-major: height (m), normal x, normal z, cavity.
pub(crate) struct DetailMap {
    pub(crate) center: (i32, i32),
    pub(crate) texels: Vec<[u16; 4]>,
}

pub(crate) fn texel_m(level: usize) -> f32 {
    BASE_TEXEL_M * (1u32 << level) as f32
}

/// `VOXEL_HORIZON_DETAIL=0` disables the maps for A/B captures.
pub(crate) fn enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var("VOXEL_HORIZON_DETAIL").as_deref() != Ok("0"))
}

/// Round-to-nearest-even f32 to IEEE half. Values here are heights of at most
/// a few kilometres and unit-range terms, so overflow saturates to infinity
/// only for corrupt input, which the caller rejects first.
pub(crate) fn f16_bits(value: f32) -> u16 {
    let bits = value.to_bits();
    let sign = ((bits >> 16) & 0x8000) as u16;
    let exponent = ((bits >> 23) & 0xff) as i32;
    let mantissa = bits & 0x7f_ffff;
    if exponent == 0xff {
        return sign | 0x7c00 | if mantissa != 0 { 0x200 } else { 0 };
    }
    let half_exponent = exponent - 127 + 15;
    if half_exponent >= 0x1f {
        return sign | 0x7c00;
    }
    if half_exponent <= 0 {
        if half_exponent < -10 {
            return sign;
        }
        let full = mantissa | 0x80_0000;
        let shift = (14 - half_exponent) as u32;
        let half = full >> shift;
        let rest = full & ((1 << shift) - 1);
        let halfway = 1 << (shift - 1);
        let round = rest > halfway || (rest == halfway && half & 1 == 1);
        return sign | (half + u32::from(round)) as u16;
    }
    let half = ((half_exponent as u32) << 10) | (mantissa >> 13);
    let rest = mantissa & 0x1fff;
    let round = rest > 0x1000 || (rest == 0x1000 && half & 1 == 1);
    sign | (half + u32::from(round)) as u16
}

/// Sample every level around `center` (metres). Returns `None` on cancel or
/// a non-finite height.
pub(super) fn build(cfg: &WorldSampleConfig, center: (i32, i32), cancel: &AtomicBool) -> Option<DetailMap> {
    let scale = cfg.voxel_meters();
    if !scale.is_finite() || scale <= 0.0 {
        return None;
    }
    let side = SIZE + 2 * BORDER;
    let mut texels = Vec::with_capacity(LEVELS * SIZE * SIZE);
    let workers = parallel::workers(parallel::cap_from_env("VOXEL_HORIZON_WORKERS", 16));
    for level in 0..LEVELS {
        let t = texel_m(level);
        let origin = (
            center.0 as f32 - SIZE as f32 * 0.5 * t,
            center.1 as f32 - SIZE as f32 * 0.5 * t,
        );
        let position = |g: usize, origin: f32| origin + (g as f32 - BORDER as f32 + 0.5) * t;
        let rows: Vec<usize> = (0..side).collect();
        let heights: Vec<f32> = parallel::map_blocks(&rows, 4, workers, cancel, |&gz| {
            let z = position(gz, origin.1);
            (0..side)
                .map(|gx| {
                    let x = position(gx, origin.0);
                    let h = crate::world::landscape::preview_height_lod(
                        cfg,
                        (x / scale).floor() as i32,
                        (z / scale).floor() as i32,
                        t,
                    );
                    h.is_finite().then_some(h)
                })
                .collect::<Option<Vec<f32>>>()
        })?
        .into_iter()
        .flatten()
        .collect();
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
        let at = |x: usize, z: usize| heights[z * side + x];
        for j in 0..SIZE {
            for i in 0..SIZE {
                let (x, z) = (i + BORDER, j + BORDER);
                let h = at(x, z);
                let dx = (at(x + 1, z) - at(x - 1, z)) / (2.0 * t);
                let dz = (at(x, z + 1) - at(x, z - 1)) / (2.0 * t);
                let inv = (dx * dx + 1.0 + dz * dz).sqrt().recip();
                // Ring two texels out: positive in valleys and hollows,
                // negative on crests. Divided by the ring radius, so it is a
                // slope-like quantity independent of the level.
                let ring = (at(x - 2, z) + at(x + 2, z) + at(x, z - 2) + at(x, z + 2)) * 0.15
                    + (at(x - 2, z - 2) + at(x + 2, z - 2) + at(x - 2, z + 2) + at(x + 2, z + 2)) * 0.10;
                let cavity = (ring - h) / (2.0 * t);
                texels.push([
                    f16_bits(h),
                    f16_bits(-dx * inv),
                    f16_bits(-dz * inv),
                    f16_bits(cavity.clamp(-4.0, 4.0)),
                ]);
            }
        }
    }
    Some(DetailMap { center, texels })
}

/// The GPU copy: one RGBA16F array texture (one layer per level) and a small
/// uniform `[centre x m, centre z m, enabled, base texel m]`. Created disabled,
/// so the shader ignores the map until the first published build.
pub(crate) struct Gpu {
    texture: wgpu::Texture,
    pub(crate) view: wgpu::TextureView,
    pub(crate) params: wgpu::Buffer,
}

impl Gpu {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        use wgpu::util::DeviceExt;
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("horizon detail map"),
            size: wgpu::Extent3d {
                width: SIZE as u32,
                height: SIZE as u32,
                depth_or_array_layers: LEVELS as u32,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("horizon detail map view"),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let params = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("horizon detail params"),
            contents: bytemuck::cast_slice(&[0.0f32, 0.0, 0.0, BASE_TEXEL_M]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        Self { texture, view, params }
    }

    /// Upload a published map, or switch the map off with `None`.
    pub(crate) fn apply(&self, queue: &wgpu::Queue, map: Option<&DetailMap>) {
        let Some(map) = map.filter(|map| map.texels.len() == LEVELS * SIZE * SIZE) else {
            queue.write_buffer(&self.params, 0, bytemuck::cast_slice(&[0.0f32, 0.0, 0.0, BASE_TEXEL_M]));
            return;
        };
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytemuck::cast_slice(&map.texels),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(SIZE as u32 * 8),
                rows_per_image: Some(SIZE as u32),
            },
            wgpu::Extent3d {
                width: SIZE as u32,
                height: SIZE as u32,
                depth_or_array_layers: LEVELS as u32,
            },
        );
        let params = [map.center.0 as f32, map.center.1 as f32, 1.0, BASE_TEXEL_M];
        queue.write_buffer(&self.params, 0, bytemuck::cast_slice(&params));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn half_conversion_matches_known_values() {
        assert_eq!(f16_bits(0.0), 0);
        assert_eq!(f16_bits(1.0), 0x3c00);
        assert_eq!(f16_bits(-2.0), 0xc000);
        assert_eq!(f16_bits(0.5), 0x3800);
        assert_eq!(f16_bits(65504.0), 0x7bff);
        assert_eq!(f16_bits(1.0e6), 0x7c00);
        // 2^-24 is the smallest subnormal.
        assert_eq!(f16_bits(5.960_464_5e-8), 1);
        // 1 + 2^-11 is halfway between 1 and the next half; ties go to even.
        assert_eq!(f16_bits(1.0 + 1.0 / 2048.0), 0x3c00);
        assert_eq!(f16_bits(1.0 + 3.0 / 2048.0), 0x3c02);
        assert_eq!(f16_bits(300.25), 0x5cb1);
    }

    #[test]
    fn levels_nest_on_the_same_world_lattice() {
        // A 512 m snapped centre is a whole number of texels at every level,
        // so texel edges never move when the horizon recentres.
        for level in 0..LEVELS {
            let t = texel_m(level);
            assert_eq!((512.0 / t).fract(), 0.0);
            assert_eq!((SIZE as f32 * 0.5 * t / t).fract(), 0.0);
        }
        // The coarsest level covers the far plane even from the farthest
        // allowed eye offset (the shader fades out its outer edge).
        let half = SIZE as f32 * 0.5 * texel_m(LEVELS - 1);
        assert!(half * 0.88 >= super::super::FAR_PLANE + 4096.0);
    }
}
