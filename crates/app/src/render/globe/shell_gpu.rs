//! Shell coverage array, night-light buffer and the globe's two pipelines.

use super::band::GlobeMask;
use super::gpu::{GlobeUniform, TileInstance};
use super::shell::{Light, MAX_LIGHTS, SHELL_REFRESH_S};
use super::shell_worker::ShellBake;

pub const SHELL_FACES: u32 = 6;
/// `GlobeShellBlend`: controls, snapshot times, then obstacle weights.
const BLEND_WORDS: usize = 23 + super::shell_obstacles::CELLS / 4;
const BLEND_BYTES: u64 = (BLEND_WORDS * 16) as u64;

pub struct ShellResources {
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    pub lights: wgpu::Buffer,
    pub blend: wgpu::Buffer,
    pub light_groups: wgpu::Buffer,
    pub planet_depth: engine_gpu::atmosphere::OpticalDepthLut,
    pub noise: super::shell_noise::ShellNoise,
    texture: wgpu::Texture,
    resolution: u32,
    current: [Vec<engine_gpu::image::RgbaMip>; 6],
    previous_meta: [[f32; 4]; 6],
    current_meta: [[f32; 4]; 6],
    previous_time: [f32; 6],
    current_time: [f32; 6],
    time_s: f32,
    wind_m: [f32; 2],
    flow: [f32; 4],
    flow_shape: [f32; 4],
    appearance: [f32; 4],
    layers: [f32; 4],
    obstacles: Option<std::sync::Arc<[f32]>>,
    changed: [Option<std::time::Instant>; 6],
}

impl ShellResources {
    pub fn new(device: &wgpu::Device, resolution: u32) -> ShellResources {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("globe cloud shell"),
            size: wgpu::Extent3d { width: resolution, height: resolution, depth_or_array_layers: SHELL_FACES * 2 },
            mip_level_count: resolution.ilog2() + 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
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
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        let lights = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globe night lights"),
            size: (MAX_LIGHTS * std::mem::size_of::<Light>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let blend = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globe cloud blend weights"),
            size: BLEND_BYTES,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let light_groups = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globe light influence groups"), size: 512,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let planet_depth = engine_gpu::atmosphere::OpticalDepthLut::new(device, super::super::planet_air::model());
        ShellResources { view, sampler, lights, blend, light_groups, planet_depth, texture, resolution,
            noise: super::shell_noise::ShellNoise::new(device),
            previous_meta: [[0.0; 4]; 6], current_meta: [[0.0; 4]; 6], wind_m: [0.0; 2],
            previous_time: [0.0; 6], current_time: [0.0; 6], time_s: 0.0,
            flow: [1.0, 0.0, 0.0, 0.0], flow_shape: [0.0; 4],
            appearance: [0.75, 0.48, 0.70, 0.5], layers: [0.08, 0.15, 0.04, 0.25], obstacles: None,
            current: std::array::from_fn(|_| Vec::new()), changed: [None; 6] }
    }

    /// Writes both temporal layers, starting at the mip matching the bake resolution.
    pub fn upload_face(&mut self, queue: &wgpu::Queue, bake: ShellBake) -> bool {
        let res = self.resolution;
        if !bake.resolution.is_power_of_two() || bake.resolution > res
            || bake.coverage.len() != (bake.resolution * bake.resolution) as usize
            || u32::from(bake.face) >= SHELL_FACES || bake.mips.is_empty() { return false; }
        let face = usize::from(bake.face);
        // Frozen/slow clocks often produce the identical field. Keep its
        // completed blend instead of uploading it twice and shading two copies
        // for another eight seconds. Compare the base mip; the rest is derived.
        if self.unchanged(&bake) { return true; }
        let first_mip = res.ilog2() - bake.resolution.ilog2();
        let previous = if self.current[face].is_empty() {
            engine_gpu::image::rgba_mips(bake.resolution, [0, 128, 128, 128].repeat(bake.coverage.len()))
        } else { std::mem::take(&mut self.current[face]) };
        let previous_mip = res.ilog2() - previous[0].edge.ilog2();
        engine_gpu::image::upload_rgba_mips(queue, &self.texture, u32::from(bake.face) + SHELL_FACES, previous_mip, &previous);
        engine_gpu::image::upload_rgba_mips(queue, &self.texture, u32::from(bake.face), first_mip, &bake.mips);
        self.previous_meta[face] = if self.changed[face].is_none() {
            [bake.drift_m[0], bake.drift_m[1], previous_mip as f32, 0.0]
        } else { self.current_meta[face] };
        self.current_meta[face] = [bake.drift_m[0], bake.drift_m[1], first_mip as f32, 1.0];
        self.previous_time[face] = self.current_time[face];
        self.current_time[face] = bake.time_s;
        self.current[face] = bake.mips;
        self.changed[face] = Some(std::time::Instant::now());
        true
    }

    /// Wait for the preceding transition rather than rebuilding it on the render thread.
    pub fn can_upload(&self, face: u8) -> bool {
        usize::from(face) < 6 && self.face_weight(usize::from(face)) >= 1.0
    }

    pub fn clear_history(&mut self) {
        for levels in &mut self.current { levels.clear(); }
        self.changed = [None; 6];
        self.previous_meta = [[0.0; 4]; 6];
        self.current_meta = [[0.0; 4]; 6];
    }

    pub fn upload_bytes(&self, bake: &ShellBake) -> usize {
        if self.unchanged(bake) { return 0; }
        let previous_edge = self.current.get(usize::from(bake.face)).and_then(|m| m.first())
            .map_or(bake.resolution, |m| m.edge);
        (engine_gpu::image::scalar_mip_bytes(previous_edge) + engine_gpu::image::scalar_mip_bytes(bake.resolution)) * 4
    }

    fn unchanged(&self, bake: &ShellBake) -> bool {
        let face = usize::from(bake.face);
        let Some(level) = self.current.get(face).and_then(|m| m.first()) else { return false };
        self.current_time[face] == bake.time_s
            && self.current_meta[face][..2] == bake.drift_m
            && bake.mips.first().is_some_and(|m| m.edge == level.edge && m.texels == level.texels)
    }

    pub fn set_wind(&mut self, wind_m: [f32; 2]) { self.wind_m = wind_m; }

    pub fn set_flow(&mut self, controls: &crate::world::CloudControls, sky_time_s: f32) {
        self.time_s = sky_time_s;
        let angle = controls.wind_dir_angle.to_radians();
        self.flow = [angle.cos(), angle.sin(), controls.wind_speed.max(0.0), sky_time_s * controls.advection];
        self.flow_shape = [controls.wind_shear.clamp(0.0, 1.0), controls.cumulus, controls.cumulonimbus,
            if self.obstacles.is_some() { 1.0 } else { 0.0 }];
        self.appearance = [super::shell::column_opacity(controls), controls.erosion, controls.detail_strength, controls.noise_scale];
        self.layers = [controls.stratus, controls.altocumulus, controls.altostratus, controls.cirrus];
    }

    /// Terrain obstacle weights; until set the shell keeps the march's hashed obstacles.
    pub fn set_obstacles(&mut self, weights: Option<std::sync::Arc<[f32]>>) {
        self.obstacles = weights.filter(|w| w.len() == super::shell_obstacles::CELLS);
        self.flow_shape[3] = if self.obstacles.is_some() { 1.0 } else { 0.0 };
    }

    fn face_weight(&self, face: usize) -> f32 {
        // Smooth updates without repeatedly rebaking the weather field.
        let t = self.changed[face].map_or(1.0, |at| (at.elapsed().as_secs_f32() / SHELL_REFRESH_S as f32).min(1.0));
        t * t * (3.0 - 2.0 * t)
    }

    pub fn update_blend(&self, queue: &wgpu::Queue) {
        // GlobeShellBlend: weights, wind, old/new snapshots, flow and cloud controls.
        let mut words = [[0.0f32; 4]; BLEND_WORDS];
        for face in 0..6 { words[face / 4][face % 4] = self.face_weight(face); }
        words[2] = [self.wind_m[0], self.wind_m[1], self.time_s, 0.0];
        words[3..9].copy_from_slice(&self.previous_meta);
        words[9..15].copy_from_slice(&self.current_meta);
        words[15] = self.flow;
        words[16] = self.flow_shape;
        for face in 0..6 {
            words[17 + face / 4][face % 4] = self.previous_time[face];
            words[19 + face / 4][face % 4] = self.current_time[face];
        }
        words[21] = self.appearance;
        words[22] = self.layers;
        if let Some(weights) = &self.obstacles {
            for (word, chunk) in words[23..].iter_mut().zip(weights.chunks_exact(4)) {
                word.copy_from_slice(chunk);
            }
        }
        queue.write_buffer(&self.blend, 0, bytemuck::cast_slice(&words));
    }

    /// Uploads at most `MAX_LIGHTS` and returns the count the shader loops over.
    pub fn upload_lights(&self, queue: &wgpu::Queue, lights: &[Light]) -> u32 {
        let lights = &lights[..lights.len().min(MAX_LIGHTS)];
        if !lights.is_empty() {
            let spheres: Vec<_> = lights.iter().map(|light| (light.body, light.radius_m)).collect();
            let (order, groups) = engine_gpu::spatial::sphere_groups(&spheres, 16);
            let sorted: Vec<_> = order.into_iter().map(|i| lights[i]).collect();
            let mut bounds = [[0.0f32; 4]; 32];
            for (i, group) in groups.iter().enumerate() {
                bounds[i * 2] = group.min_first;
                bounds[i * 2 + 1] = group.max_count;
            }
            queue.write_buffer(&self.lights, 0, bytemuck::cast_slice(&sorted));
            queue.write_buffer(&self.light_groups, 0, bytemuck::cast_slice(&bounds));
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
        // Skirt walls face out of their tile, toward the neighbour a seam is seen from. Drawing
        // back faces too put the inside of the far hemisphere over the sky.
        primitive: wgpu::PrimitiveState { cull_mode: Some(wgpu::Face::Back), ..Default::default() },
        // Same depth state as the world pipeline, so chunks and assets sort against the globe.
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: Some(depth_write),
            depth_compare: Some(wgpu::CompareFunction::Greater),
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    })
}

/// Group 0 of both globe pipelines; bindings match the `@binding`s in globe_shader.wgsl and globe_mask.wgsl.
pub fn bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    // Tile and shell shaders share read-only helpers across both stages.
    // Keep their common layout valid when a helper moves between VS and FS.
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
            entry(1, vertex_fragment, wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: true },
                has_dynamic_offset: false,
                min_binding_size: std::num::NonZeroU64::new(std::mem::size_of::<TileInstance>() as u64),
            }),
            entry(2, vertex_fragment, texture(wgpu::TextureSampleType::Uint)),
            entry(3, vertex_fragment, texture(wgpu::TextureSampleType::Float { filterable: false })),
            entry(4, vertex_fragment, wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2Array,
                multisampled: false,
            }),
            entry(5, vertex_fragment, wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering)),
            entry(6, vertex_fragment, wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: true },
                has_dynamic_offset: false,
                min_binding_size: std::num::NonZeroU64::new(std::mem::size_of::<Light>() as u64),
            }),
            entry(7, vertex_fragment, wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: std::num::NonZeroU64::new(std::mem::size_of::<GlobeMask>() as u64),
            }),
            entry(8, vertex_fragment, wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: std::num::NonZeroU64::new(BLEND_BYTES),
            }),
            entry(9, vertex_fragment, wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false,
                min_binding_size: std::num::NonZeroU64::new(512),
            }),
            entry(10, vertex_fragment, texture(wgpu::TextureSampleType::Float {
                filterable: device.features().contains(wgpu::Features::FLOAT32_FILTERABLE),
            })),
            entry(11, vertex_fragment, wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D3, multisampled: false,
            }),
            entry(12, vertex_fragment, wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering)),
            entry(13, vertex_fragment, texture(wgpu::TextureSampleType::Float { filterable: false })),
        ],
    })
}
