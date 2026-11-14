//! Repeating value-noise lattice. Hardware filtering replaces eight integer
//! hashes per orbital detail lookup; the shader supplies quintic weights.
const EDGE: u32 = 64;

pub struct ShellNoise {
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    texture: wgpu::Texture,
    uploaded: bool,
}

impl ShellNoise {
    pub fn new(device: &wgpu::Device) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("orbital cloud detail lattice"),
            size: wgpu::Extent3d { width: EDGE, height: EDGE, depth_or_array_layers: EDGE },
            mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D3,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("orbital cloud detail sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear, min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Self { view, sampler, texture, uploaded: false }
    }

    pub fn upload_once(&mut self, queue: &wgpu::Queue) {
        if self.uploaded { return; }
        // 256 KiB once per renderer; no per-frame generation or transfer.
        let data: Vec<u8> = (0..EDGE.pow(3))
            .map(|i| (engine_core::hash::hash_u32(i ^ 0x6C8E_9CF5) >> 24) as u8).collect();
        queue.write_texture(
            wgpu::TexelCopyTextureInfo { texture: &self.texture, mip_level: 0,
                origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            &data,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(EDGE), rows_per_image: Some(EDGE) },
            wgpu::Extent3d { width: EDGE, height: EDGE, depth_or_array_layers: EDGE },
        );
        self.uploaded = true;
    }
}
