//! Static optical-depth tables for spherical exponential atmospheres.
//! The table stores density-weighted metres, independent of sun direction/color.

pub const DEPTH_WIDTH: u32 = 128;
pub const DEPTH_HEIGHT: u32 = 64;

#[derive(Clone, Copy)]
pub struct AirModel {
    pub radius_m: f64,
    pub top_m: f64,
    pub rayleigh_height_m: f64,
    pub mie_height_m: f64,
}

pub struct OpticalDepthLut {
    pub view: wgpu::TextureView,
    texture: wgpu::Texture,
    model: AirModel,
    uploaded: bool,
}

impl OpticalDepthLut {
    pub fn new(device: &wgpu::Device, model: AirModel) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("spherical atmosphere optical depth"),
            size: wgpu::Extent3d {
                width: DEPTH_WIDTH,
                height: DEPTH_HEIGHT,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rg32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        Self {
            view,
            texture,
            model,
            uploaded: false,
        }
    }

    /// Built once per resource. No per-frame table generation or readback.
    pub fn upload_once(&mut self, queue: &wgpu::Queue) {
        if self.uploaded {
            return;
        }
        let data = optical_depths(self.model);
        let bytes: Vec<u8> = data
            .into_iter()
            .flat_map(|v| v.into_iter().flat_map(f32::to_le_bytes))
            .collect();
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(DEPTH_WIDTH * 8),
                rows_per_image: Some(DEPTH_HEIGHT),
            },
            wgpu::Extent3d {
                width: DEPTH_WIDTH,
                height: DEPTH_HEIGHT,
                depth_or_array_layers: 1,
            },
        );
        self.uploaded = true;
    }
}

pub fn optical_depths(model: AirModel) -> Vec<[f32; 2]> {
    let mut values = Vec::with_capacity((DEPTH_WIDTH * DEPTH_HEIGHT) as usize);
    for y in 0..DEPTH_HEIGHT {
        let v = f64::from(y) / f64::from(DEPTH_HEIGHT - 1);
        let h = v * v * model.top_m;
        let r = model.radius_m + h;
        let horizon = -(1.0 - (model.radius_m / r).powi(2)).max(0.0).sqrt();
        for x in 0..DEPTH_WIDTH {
            let u = f64::from(x) / f64::from(DEPTH_WIDTH - 1);
            let mu = horizon + u * u * (1.0 - horizon);
            let b = r * mu;
            let reach = -b
                + (b * b + (model.radius_m + model.top_m).powi(2) - r * r)
                    .max(0.0)
                    .sqrt();
            let ds = reach / 64.0;
            let mut depth = [0.0; 2];
            for tap in 0..64 {
                let t = ds * (f64::from(tap) + 0.5);
                let height = (r * r + t * t + 2.0 * b * t).sqrt() - model.radius_m;
                depth[0] += (-height.max(0.0) / model.rayleigh_height_m).exp() * ds;
                depth[1] += (-height.max(0.0) / model.mie_height_m).exp() * ds;
            }
            values.push([depth[0] as f32, depth[1] as f32]);
        }
    }
    values
}
