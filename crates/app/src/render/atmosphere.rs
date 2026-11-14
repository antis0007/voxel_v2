//! Atmosphere runtime resources and shader assembly.
//!
//! The goal of this module is to own atmosphere runtime data explicitly instead
//! of letting `render.rs` grow another set of one-off bindings and textures.
//! The current transition state is intentionally dual-path:
//!
//! - the analytic sky implementation still exists as a named fallback
//! - the runtime now owns atmosphere LUT textures and update policy
//! - shaders consume those resources through an explicit bind group
//!
//! Transmittance and isotropic multiple-scattering transport are computed once
//! for fixed atmospheric coefficients. Dynamic sky-view and aerial-perspective
//! passes integrate both tables at local solar elevation and altitude.

pub(crate) use crate::shader_sources::{
    ATMOSPHERE_LUT_GENERATION_WGSL, cloud_composite_shader_source,
    cloud_shader_source_for_filtering, cloud_temporal_shader_source,
    sky_shader_source_for_filtering,
    world_shader_source_for_filtering,
};

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

use crate::world::{CloudControls, LightingState};

use super::atmosphere_config::{AtmosphereConfig, AtmospherePreset};
use super::atmosphere_params::{AtmosphereMode, AtmosphereParameters, SkyArt};
use super::debug::RenderDebugView;
use super::sky_probe::{SKY_PROBE_BYTES, SkyProbe};

pub const TRANSMITTANCE_LUT_WIDTH: u32 = 256;
pub const TRANSMITTANCE_LUT_HEIGHT: u32 = 64;
pub const SKY_VIEW_LUT_WIDTH: u32 = 192;
pub const SKY_VIEW_LUT_HEIGHT: u32 = 108;
pub const MULTISCATTER_LUT_WIDTH: u32 = 32;
pub const MULTISCATTER_LUT_HEIGHT: u32 = 32;
const ATMOSPHERE_LUT_WORKGROUP_SIZE_X: u32 = 8;
const ATMOSPHERE_LUT_WORKGROUP_SIZE_Y: u32 = 8;

const TRANSMITTANCE_LUT_MAX_ALTITUDE_M: f32 = 60_000.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[derive(Default)]
pub struct AtmosphereRuntimeState {
    pub mode: AtmosphereMode,
    pub debug_view: RenderDebugView,
}


#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct AtmosphereRuntimeUniform {
    // x: atmosphere mode
    // y: renderer debug surface
    // z/w: current output surface dimensions
    mode_debug_surface: [u32; 4],
    // x: max LUT altitude in meters
    // y: reciprocal altitude scale
    transmittance_params: [f32; 4],
}

fn atmosphere_uniform(
    state: AtmosphereRuntimeState,
    surface_width: u32,
    surface_height: u32,
) -> AtmosphereRuntimeUniform {
    AtmosphereRuntimeUniform {
        mode_debug_surface: [
            state.mode.as_u32(),
            state.debug_view.as_u32(),
            surface_width.max(1),
            surface_height.max(1),
        ],
        transmittance_params: [
            TRANSMITTANCE_LUT_MAX_ALTITUDE_M,
            TRANSMITTANCE_LUT_MAX_ALTITUDE_M.recip(),
            0.0,
            0.0,
        ],
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct AtmosphereLutGenerationUniform {
    sun_dir_daylight: [f32; 4],
    moon_dir_twilight: [f32; 4],
    sun_color_cloud_cover: [f32; 4],
    moon_color_reserved: [f32; 4],
    sky_top_reserved: [f32; 4],
    sky_horizon_reserved: [f32; 4],
    fog_color_max_altitude: [f32; 4],
    /// x: haze extinction 1/m, y: haze scale height m. Transmittance and
    /// multiscatter are built once at startup, so these are startup constants.
    art: [f32; 4],
    /// `AtmospherePreset::uniform_words`: planet, Rayleigh + g, Mie + ozone
    /// width, ozone + centre, Mie absorption.
    physics: [[f32; 4]; 5],
}

fn active_preset() -> &'static AtmospherePreset {
    static ACTIVE: std::sync::OnceLock<AtmospherePreset> = std::sync::OnceLock::new();
    ACTIVE.get_or_init(AtmosphereConfig::shipped_active)
}

/// `preset` with today's aerosol regime applied to its coefficients.
fn with_air(preset: &AtmospherePreset, air: crate::world::sky_air::SkyAir) -> AtmospherePreset {
    let mut p = preset.clone();
    let [r, g, b] = air.spectral_mie();
    p.mie_scattering = (p.mie_scattering.0 * r, p.mie_scattering.1 * g, p.mie_scattering.2 * b);
    let a = air.absorption;
    p.mie_absorption = (p.mie_absorption.0 + a[0], p.mie_absorption.1 + a[1], p.mie_absorption.2 + a[2]);
    p.mie_scale_height_m *= air.height;
    p.mie_g = air.g;
    let o = air.ozone;
    p.ozone_absorption = (p.ozone_absorption.0 * o, p.ozone_absorption.1 * o, p.ozone_absorption.2 * o);
    p
}

impl AtmosphereLutGenerationUniform {
    fn from_parameters(params: AtmosphereParameters) -> Self {
        Self {
            sun_dir_daylight: [
                params.sun_dir.x,
                params.sun_dir.y,
                params.sun_dir.z,
                params.daylight,
            ],
            moon_dir_twilight: [
                params.moon_dir.x,
                params.moon_dir.y,
                params.moon_dir.z,
                params.twilight,
            ],
            sun_color_cloud_cover: [
                params.sun_color.x,
                params.sun_color.y,
                params.sun_color.z,
                params.cloud_cover,
            ],
            moon_color_reserved: [
                params.moon_color.x,
                params.moon_color.y,
                params.moon_color.z,
                0.0,
            ],
            sky_top_reserved: [params.sky_top.x, params.sky_top.y, params.sky_top.z, 0.0],
            sky_horizon_reserved: [
                params.sky_horizon.x,
                params.sky_horizon.y,
                params.sky_horizon.z,
                0.0,
            ],
            fog_color_max_altitude: [
                params.fog_color.x,
                params.fog_color.y,
                params.fog_color.z,
                TRANSMITTANCE_LUT_MAX_ALTITUDE_M,
            ],
            art: [
                SkyArt::active().haze_extinction * if params.hell { 1.0 } else { params.air.density },
                SkyArt::active().haze_scale_height,
                SkyArt::active().fog_seam,
                0.0,
            ],
            physics: if params.hell { AtmospherePreset::hell().uniform_words() } else { with_air(active_preset(), params.air).uniform_words() },
        }
    }
}

struct AtmosphereLut {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    width: u32,
    height: u32,
}

impl AtmosphereLut {
    fn new(
        device: &wgpu::Device,
        label: &'static str,
        width: u32,
        height: u32,
        extra_usages: wgpu::TextureUsages,
    ) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | extra_usages,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            texture,
            view,
            width,
            height,
        }
    }

}

struct AtmosphereLut3D {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    width: u32,
    height: u32,
    depth: u32,
}

struct AtmosphereLutGenerationResources {
    uniform_buffer: wgpu::Buffer,
    transmittance_bind_group: wgpu::BindGroup,
    sky_view_bind_group: wgpu::BindGroup,
    multiscatter_bind_group: wgpu::BindGroup,
    ap_bind_group: wgpu::BindGroup,
    transmittance_pipeline: wgpu::ComputePipeline,
    sky_view_pipeline: wgpu::ComputePipeline,
    multiscatter_pipeline: wgpu::ComputePipeline,
    ap_pipeline: wgpu::ComputePipeline,
}

struct AtmosphereLuts {
    /// Camera-altitude / sun-elevation transmittance. This is already sampled in
    /// active shading when the atmosphere mode selects the LUT path.
    transmittance: AtmosphereLut,
    /// Lighting-driven sky-view texture generated by an atmosphere-owned compute pass.
    sky_view: AtmosphereLut,
    /// Lighting-driven multiscatter texture generated by an atmosphere-owned compute pass.
    multiscatter: AtmosphereLut,
    /// 3D Aerial perspective volume.
    aerial_perspective: AtmosphereLut3D,
}

pub struct AtmosphereRuntime {
    _planet_depth: engine_gpu::atmosphere::OpticalDepthLut,
    bind_group: wgpu::BindGroup,
    uniform_buffer: wgpu::Buffer,
    sky_probe: SkyProbe,
    sky_probe_uniform: wgpu::Buffer,
    _luts: AtmosphereLuts,
    generation: AtmosphereLutGenerationResources,
    state: AtmosphereRuntimeState,
    surface_width: u32,
    surface_height: u32,
    last_parameters: AtmosphereParameters,
    pending_parameters: AtmosphereParameters,
    last_sky_altitude: f32,
    last_aerial: Option<(f32, f32, AtmosphereParameters)>,
    transport_hell: bool,
    /// Aerosol regime the transport tables were last solved with.
    transport_air: crate::world::sky_air::SkyAir,
}

impl AtmosphereRuntime {
    pub fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("atmosphere layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT | wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT | wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT | wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT | wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT | wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D3,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT | wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT | wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(SKY_PROBE_BYTES),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float {
                            filterable: device.features().contains(wgpu::Features::FLOAT32_FILTERABLE),
                        },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        })
    }

    fn create_lut_generation_resources(
        device: &wgpu::Device,
        luts: &AtmosphereLuts,
        params: AtmosphereParameters,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
        pipeline_cache: Option<&wgpu::PipelineCache>,
    ) -> AtmosphereLutGenerationResources {
        let mut generation_entries = vec![
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::StorageTexture {
                    access: wgpu::StorageTextureAccess::WriteOnly,
                    format: wgpu::TextureFormat::Rgba32Float,
                    view_dimension: wgpu::TextureViewDimension::D2,
                },
                count: None,
            },
        ];
        let multiscatter_generation_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("atmosphere transport generation layout"),
                entries: &generation_entries,
            });
        // The transport solve writes this texture. Sky/AP only read it, using
        // a distinct layout to avoid binding one resource for read and write.
        generation_entries.push(wgpu::BindGroupLayoutEntry {
            binding: 3,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        });
        let generation_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("atmosphere lut generation layout"),
            entries: &generation_entries,
        });

        // Dedicated layout for transmittance generation: only needs uniform and output storage texture
        let transmittance_generation_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("atmosphere transmittance generation specific layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2, // Binding 2 for the output storage texture
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::StorageTexture {
                            access: wgpu::StorageTextureAccess::WriteOnly,
                            format: wgpu::TextureFormat::Rgba32Float, // Transmittance LUT is Rgba32Float
                            view_dimension: wgpu::TextureViewDimension::D2,
                        },
                        count: None,
                    },
                ],
            });

        let generation_uniform_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("atmosphere lut generation buffer"),
                contents: bytemuck::bytes_of(&AtmosphereLutGenerationUniform::from_parameters(
                    params,
                )),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        let transmittance_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("atmosphere transmittance generation bind group"),
            layout: &transmittance_generation_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: generation_uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&luts.transmittance.view),
                },
            ],
        });

        let sky_view_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("atmosphere sky-view generation bind group"),
            layout: &generation_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: generation_uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&luts.transmittance.view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&luts.sky_view.view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&luts.multiscatter.view),
                },
            ],
        });
        let multiscatter_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("atmosphere multiscatter generation bind group"),
            layout: &multiscatter_generation_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: generation_uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&luts.transmittance.view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&luts.multiscatter.view),
                },
            ],
        });

        let ap_generation_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("ap generation layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba16Float,
                        view_dimension: wgpu::TextureViewDimension::D3,
                    },
                    count: None,
                }],
            });

        let ap_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ap generation bind group"),
            layout: &ap_generation_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&luts.aerial_perspective.view),
            }],
        });

        let lut_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("atmosphere lut pipeline layout"),
            bind_group_layouts: &[Some(&generation_layout), Some(camera_bind_group_layout)],
            immediate_size: 0,
        });

        let ap_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ap pipeline layout"),
            bind_group_layouts: &[
                Some(&generation_layout),
                Some(camera_bind_group_layout),
                Some(&ap_generation_layout),
            ],
            immediate_size: 0,
        });

        let generation_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("atmosphere lut generation shader"),
            source: wgpu::ShaderSource::Wgsl(ATMOSPHERE_LUT_GENERATION_WGSL.into()),
        });

        let transmittance_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("transmittance pipeline layout"),
                bind_group_layouts: &[Some(&transmittance_generation_layout)],
                immediate_size: 0,
            });
        let multiscatter_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("atmosphere transport pipeline layout"),
                bind_group_layouts: &[Some(&multiscatter_generation_layout)],
                immediate_size: 0,
            });
        let compute = |label: &str, layout: &wgpu::PipelineLayout, entry_point: &str| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(label),
                layout: Some(layout),
                module: &generation_shader,
                entry_point: Some(entry_point),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                cache: pipeline_cache,
            })
        };
        // The driver compiles each entry point when its pipeline is created;
        // the four are independent, so they compile on parallel threads.
        let (transmittance_pipeline, sky_view_pipeline, multiscatter_pipeline, ap_pipeline) =
            std::thread::scope(|scope| {
                let transmittance = scope.spawn(|| {
                    compute("atmosphere transmittance generation pipeline",
                        &transmittance_pipeline_layout, "generate_transmittance_lut")
                });
                let sky_view = scope.spawn(|| {
                    compute("atmosphere sky-view generation pipeline",
                        &lut_pipeline_layout, "generate_sky_view_lut")
                });
                let multiscatter = scope.spawn(|| {
                    compute("atmosphere multiscatter generation pipeline",
                        &multiscatter_pipeline_layout, "generate_multiscatter_lut")
                });
                let ap = compute("atmosphere ap generation pipeline",
                    &ap_pipeline_layout, "generate_aerial_perspective");
                let join = |handle: std::thread::ScopedJoinHandle<'_, wgpu::ComputePipeline>| {
                    handle.join().unwrap_or_else(|panic| std::panic::resume_unwind(panic))
                };
                (join(transmittance), join(sky_view), join(multiscatter), ap)
            });

        AtmosphereLutGenerationResources {
            uniform_buffer: generation_uniform_buffer,
            transmittance_bind_group,
            sky_view_bind_group,
            multiscatter_bind_group,
            ap_bind_group,
            transmittance_pipeline,
            sky_view_pipeline,
            multiscatter_pipeline,
            ap_pipeline,
        }
    }

    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bind_group_layout: wgpu::BindGroupLayout,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
        lighting_bind_group_layout: &wgpu::BindGroupLayout,
        camera_bind_group: &wgpu::BindGroup,
        surface_width: u32,
        surface_height: u32,
        initial_lighting: &LightingState,
        initial_cloud_controls: &CloudControls,
        pipeline_cache: Option<&wgpu::PipelineCache>,
    ) -> Self {
        let surface_width = surface_width.max(1);
        let surface_height = surface_height.max(1);
        let state = AtmosphereRuntimeState::default();
        let last_parameters =
            AtmosphereParameters::from_lighting(initial_lighting, initial_cloud_controls);

        let aerial_perspective_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("atmosphere aerial perspective"),
            size: wgpu::Extent3d {
                width: 48,
                height: 48,
                depth_or_array_layers: 32,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D3,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::STORAGE_BINDING,
            view_formats: &[],
        });
        let aerial_perspective_view =
            aerial_perspective_tex.create_view(&wgpu::TextureViewDescriptor::default());

        let luts = AtmosphereLuts {
            transmittance: AtmosphereLut::new(
                device,
                "atmosphere transmittance lut",
                TRANSMITTANCE_LUT_WIDTH,
                TRANSMITTANCE_LUT_HEIGHT,
                wgpu::TextureUsages::STORAGE_BINDING,
            ),
            sky_view: AtmosphereLut::new(
                device,
                "atmosphere sky-view lut",
                SKY_VIEW_LUT_WIDTH,
                SKY_VIEW_LUT_HEIGHT,
                wgpu::TextureUsages::STORAGE_BINDING,
            ),
            multiscatter: AtmosphereLut::new(
                device,
                "atmosphere multiscatter lut",
                MULTISCATTER_LUT_WIDTH,
                MULTISCATTER_LUT_HEIGHT,
                wgpu::TextureUsages::STORAGE_BINDING,
            ),
            aerial_perspective: AtmosphereLut3D {
                texture: aerial_perspective_tex,
                view: aerial_perspective_view,
                width: 48,
                height: 48,
                depth: 32,
            },
        };
        let generation = Self::create_lut_generation_resources(
            device,
            &luts,
            last_parameters,
            camera_bind_group_layout,
            pipeline_cache,
        );

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("atmosphere sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("atmosphere runtime buffer"),
            contents: bytemuck::bytes_of(&atmosphere_uniform(state, surface_width, surface_height)),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let sky_probe_uniform = SkyProbe::create_uniform(device);
        let mut planet_depth = engine_gpu::atmosphere::OpticalDepthLut::new(device, super::planet_air::model());
        planet_depth.upload_once(queue);
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("atmosphere bind group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&luts.transmittance.view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&luts.sky_view.view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&luts.multiscatter.view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&luts.aerial_perspective.view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: sky_probe_uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::TextureView(&planet_depth.view),
                },
            ],
        });
        let sky_probe = SkyProbe::new(
            device,
            camera_bind_group_layout,
            lighting_bind_group_layout,
            &bind_group_layout,
            pipeline_cache,
        );

        let runtime = Self {
            _planet_depth: planet_depth,
            bind_group,
            uniform_buffer,
            sky_probe,
            sky_probe_uniform,
            _luts: luts,
            generation,
            state,
            surface_width,
            surface_height,
            last_parameters,
            pending_parameters: last_parameters,
            last_sky_altitude: f32::NEG_INFINITY,
            last_aerial: None,
            transport_hell: false,
            transport_air: last_parameters.air,
        };

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("initial atmosphere LUTs"),
        });
        runtime.record_transport(&mut encoder);
        runtime.record_sky_view(&mut encoder, camera_bind_group);
        queue.submit(Some(encoder.finish()));

        runtime
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    /// Refresh `sky_probe` for this frame. Record after the camera, lighting
    /// and atmosphere writes and before the first pass that shades terrain.
    pub fn record_sky_probe(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        camera_bind_group: &wgpu::BindGroup,
        lighting_bind_group: &wgpu::BindGroup,
        timestamps: Option<wgpu::ComputePassTimestampWrites<'_>>,
    ) {
        self.sky_probe.record(
            encoder,
            camera_bind_group,
            lighting_bind_group,
            &self.bind_group,
            &self.sky_probe_uniform,
            timestamps,
        );
    }

    pub fn needs_aerial_perspective(&self) -> bool {
        !matches!(self.state.mode, AtmosphereMode::AnalyticFallback)
    }

    pub fn resize(&mut self, queue: &wgpu::Queue, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);
        if self.surface_width == width && self.surface_height == height {
            return;
        }

        self.surface_width = width;
        self.surface_height = height;
        self.write_uniform(queue);
    }

    pub fn set_state(&mut self, queue: &wgpu::Queue, state: AtmosphereRuntimeState) {
        if self.state == state {
            return;
        }

        self.state = state;
        self.last_aerial = None;
        self.write_uniform(queue);
    }

    /// Stage lighting changes; the actual solve runs after this frame's camera
    /// uniform has been written, so flight never uses last frame's altitude.
    pub fn sync_from_lighting(&mut self, lighting: &LightingState, cloud_controls: &CloudControls) {
        self.pending_parameters = AtmosphereParameters::from_lighting(lighting, cloud_controls);
    }

    pub fn sync_camera(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        camera_bind_group: &wgpu::BindGroup,
        camera_altitude: f32,
        world_extent: f32,
    ) {
        let params = self.pending_parameters;
        // Keep the existing transport, sky-view and aerial invalidation thresholds.
        let transport = params.hell != self.transport_hell
            || params.air.differs(self.transport_air);
        let sky_view = self.needs_dynamic_sky_luts()
            && (transport
                || !self.last_parameters.approximately_matches(params)
                || (self.last_sky_altitude - camera_altitude).abs() >= 1.0);
        let altitude = camera_altitude.max(1.0);
        let extent = (world_extent * 0.82).max(16000.0);
        let aerial = self.needs_aerial_perspective()
            && (transport || !self.last_aerial.is_some_and(|(previous_altitude, previous_extent, lighting)| {
                (previous_altitude - altitude).abs() < 0.5
                    && previous_extent == extent
                    && lighting.approximately_matches(params)
            }));
        if !transport && !sky_view && !aerial {
            return;
        }

        // Every pass reads the same parameters. Record producer passes first
        // so their dependent LUT reads are ordered within one submission.
        queue.write_buffer(
            &self.generation.uniform_buffer,
            0,
            bytemuck::bytes_of(&AtmosphereLutGenerationUniform::from_parameters(params)),
        );
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("atmosphere LUT updates"),
        });
        if transport {
            self.record_transport(&mut encoder);
            self.transport_hell = params.hell;
            self.transport_air = params.air;
            self.last_sky_altitude = f32::NEG_INFINITY;
            self.last_aerial = None;
        }
        if sky_view {
            self.record_sky_view(&mut encoder, camera_bind_group);
            self.last_parameters = params;
            self.last_sky_altitude = camera_altitude;
        }
        if aerial {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("ap generation pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.generation.ap_pipeline);
            pass.set_bind_group(0, &self.generation.sky_view_bind_group, &[]);
            pass.set_bind_group(1, camera_bind_group, &[]);
            pass.set_bind_group(2, &self.generation.ap_bind_group, &[]);
            pass.dispatch_workgroups(
                div_ceil_u32(self._luts.aerial_perspective.width, 8),
                div_ceil_u32(self._luts.aerial_perspective.height, 8),
                div_ceil_u32(self._luts.aerial_perspective.depth, 4),
            );
            self.last_aerial = Some((altitude, extent, params));
        }
        queue.submit(Some(encoder.finish()));
    }

    fn needs_dynamic_sky_luts(&self) -> bool {
        matches!(self.state.mode, AtmosphereMode::SkyViewLut)
            || matches!(
                self.state.debug_view,
                RenderDebugView::AtmosphereSkyView | RenderDebugView::AtmosphereMultiscatter
            )
    }

    fn record_transport(&self, encoder: &mut wgpu::CommandEncoder) {
        for (pipeline, group, width, height) in [
            (&self.generation.transmittance_pipeline, &self.generation.transmittance_bind_group, TRANSMITTANCE_LUT_WIDTH, TRANSMITTANCE_LUT_HEIGHT),
            (&self.generation.multiscatter_pipeline, &self.generation.multiscatter_bind_group, MULTISCATTER_LUT_WIDTH, MULTISCATTER_LUT_HEIGHT),
        ] {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("atmosphere transport LUT"),
                timestamp_writes: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, group, &[]);
            pass.dispatch_workgroups(
                div_ceil_u32(width, ATMOSPHERE_LUT_WORKGROUP_SIZE_X),
                div_ceil_u32(height, ATMOSPHERE_LUT_WORKGROUP_SIZE_Y),
                1,
            );
        }
    }

    fn record_sky_view(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        camera_bind_group: &wgpu::BindGroup,
    ) {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("atmosphere sky-view generation pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.generation.sky_view_pipeline);
        pass.set_bind_group(0, &self.generation.sky_view_bind_group, &[]);
        pass.set_bind_group(1, camera_bind_group, &[]);
        pass.dispatch_workgroups(
            div_ceil_u32(SKY_VIEW_LUT_WIDTH, ATMOSPHERE_LUT_WORKGROUP_SIZE_X),
            div_ceil_u32(SKY_VIEW_LUT_HEIGHT, ATMOSPHERE_LUT_WORKGROUP_SIZE_Y),
            1,
        );
    }

    fn write_uniform(&self, queue: &wgpu::Queue) {
        queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::bytes_of(&atmosphere_uniform(
                self.state,
                self.surface_width,
                self.surface_height,
            )),
        );
    }
}

fn div_ceil_u32(value: u32, divisor: u32) -> u32 {
    value.div_ceil(divisor.max(1))
}
