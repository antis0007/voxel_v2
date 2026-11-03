//! Shell coverage array, night-light buffer and the globe's two pipelines.

use super::shell::{Light, MAX_LIGHTS};
use super::shell_worker::ShellBake;

pub const SHELL_FACES: u32 = 6;

pub struct ShellResources {
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    pub lights: wgpu::Buffer,
    texture: wgpu::Texture,
    resolution: u32,
}

impl ShellResources {
    pub fn new(device: &wgpu::Device, resolution: u32) -> ShellResources {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("globe cloud shell"),
            size: wgpu::Extent3d { width: resolution, height: resolution, depth_or_array_layers: SHELL_FACES },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("globe cloud shell sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let lights = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globe night lights"),
            size: (MAX_LIGHTS * std::mem::size_of::<Light>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        ShellResources { view, sampler, lights, texture, resolution }
    }

    /// Writes one face layer; a bake at another resolution is dropped.
    pub fn upload_face(&self, queue: &wgpu::Queue, bake: &ShellBake) -> bool {
        let res = self.resolution;
        if bake.resolution != res || bake.coverage.len() != (res * res) as usize || u32::from(bake.face) >= SHELL_FACES {
            return false;
        }
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x: 0, y: 0, z: u32::from(bake.face) },
                aspect: wgpu::TextureAspect::All,
            },
            &bake.coverage,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(res), rows_per_image: Some(res) },
            wgpu::Extent3d { width: res, height: res, depth_or_array_layers: 1 },
        );
        true
    }

    /// Uploads at most `MAX_LIGHTS` and returns the count the shader loops over.
    pub fn upload_lights(&self, queue: &wgpu::Queue, lights: &[Light]) -> u32 {
        let lights = &lights[..lights.len().min(MAX_LIGHTS)];
        if !lights.is_empty() {
            queue.write_buffer(&self.lights, 0, bytemuck::cast_slice(lights));
        }
        lights.len() as u32
    }

    pub fn resolution(&self) -> u32 {
        self.resolution
    }
}

/// Opaque (`fs_main`, depth write) or shell (`vs_shell`/`fs_shell`, alpha over, no depth write).
pub fn pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    color_format: wgpu::TextureFormat,
    shell: bool,
) -> wgpu::RenderPipeline {
    let target = |blend, write_mask| Some(wgpu::ColorTargetState { format: color_format, blend, write_mask });
    let (vs, fs, color, depth_write) = if shell {
        ("vs_shell", "fs_shell", target(Some(wgpu::BlendState::ALPHA_BLENDING), wgpu::ColorWrites::ALL), false)
    } else {
        ("vs_main", "fs_main", target(Some(wgpu::BlendState::REPLACE), wgpu::ColorWrites::ALL), true)
    };
    // The shell leaves the reflection receiver target to the ground beneath it.
    let receiver = target(None, if shell { wgpu::ColorWrites::empty() } else { wgpu::ColorWrites::ALL });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(if shell { "globe shell pipeline" } else { "globe pipeline" }),
        layout: Some(layout),
        vertex: wgpu::VertexState { module: shader, entry_point: Some(vs), compilation_options: Default::default(), buffers: &[] },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some(fs),
            compilation_options: Default::default(),
            targets: &[color, receiver],
        }),
        primitive: wgpu::PrimitiveState { cull_mode: Some(wgpu::Face::Back), ..Default::default() },
        // Same depth state as the world pipeline, so chunks and assets sort against the globe.
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: depth_write,
            depth_compare: wgpu::CompareFunction::Greater,
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: Default::default(),
        multiview: None,
        cache: None,
    })
}
