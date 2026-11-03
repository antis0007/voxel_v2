//! Shell coverage array, night-light buffer and the globe's two pipelines.

use super::band::GlobeMask;
use super::gpu::{GlobeUniform, TileInstance};
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

/// Ground (`fs_main`, depth write, alpha only in the fade band) or shell (`vs_shell`/`fs_shell`, alpha over, no depth write).
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
        // Blended for the fade band past the resident disc; alpha is 1 elsewhere, so it still reads opaque.
        ("vs_main", "fs_main", target(Some(wgpu::BlendState::ALPHA_BLENDING), wgpu::ColorWrites::ALL), true)
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
        // Ground tiles draw both faces so a skirt wall shows whichever way it faces the eye.
        primitive: wgpu::PrimitiveState { cull_mode: if shell { Some(wgpu::Face::Back) } else { None }, ..Default::default() },
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

/// Group 0 of both globe pipelines; bindings match the `@binding`s in globe_shader.wgsl and globe_mask.wgsl.
pub fn bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    let entry = |binding, visibility, ty| wgpu::BindGroupLayoutEntry { binding, visibility, ty, count: None };
    let vertex_fragment = wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT;
    let texture = |sample_type| wgpu::BindingType::Texture {
        sample_type,
        view_dimension: wgpu::TextureViewDimension::D2,
        multisampled: false,
    };
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("globe layout"),
        entries: &[
            entry(0, vertex_fragment, wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: std::num::NonZeroU64::new(std::mem::size_of::<GlobeUniform>() as u64),
            }),
            entry(1, wgpu::ShaderStages::VERTEX, wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: true },
                has_dynamic_offset: false,
                min_binding_size: std::num::NonZeroU64::new(std::mem::size_of::<TileInstance>() as u64),
            }),
            entry(2, wgpu::ShaderStages::VERTEX, texture(wgpu::TextureSampleType::Uint)),
            entry(3, wgpu::ShaderStages::VERTEX, texture(wgpu::TextureSampleType::Float { filterable: false })),
            entry(4, wgpu::ShaderStages::FRAGMENT, wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2Array,
                multisampled: false,
            }),
            entry(5, wgpu::ShaderStages::FRAGMENT, wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering)),
            entry(6, wgpu::ShaderStages::FRAGMENT, wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: true },
                has_dynamic_offset: false,
                min_binding_size: std::num::NonZeroU64::new(std::mem::size_of::<Light>() as u64),
            }),
            entry(7, wgpu::ShaderStages::FRAGMENT, wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: std::num::NonZeroU64::new(std::mem::size_of::<GlobeMask>() as u64),
            }),
        ],
    })
}
