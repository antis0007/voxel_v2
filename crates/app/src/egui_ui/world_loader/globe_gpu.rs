//! Paint callback for the loading globe: six face textures shaded per pixel.
//!
//! A new surface is written into the idle texture and crossfaded in, so solve
//! snapshots and survey faces arrive without a pop or a frame stall.
use std::sync::Arc;

use egui::{Painter, Pos2, Rect, vec2};
use egui_wgpu::{Callback, CallbackResources, CallbackTrait, ScreenDescriptor};
use glam::{Mat3, Vec3};

use super::geology::motion::{MAX_PLATES, Motion};
use super::surface::Surface;

/// Relief exaggeration: 500 m on a 20 km globe is invisible at true scale.
// Keep regional relief legible without turning every slope into a dark scarp.
const RELIEF: f32 = 2.25;
/// Uniform size: seven `vec4<f32>`.
const UNIFORM_BYTES: u64 = 7 * 16;
/// Backdrop uniform: three `vec4<f32>` (WGSL `Backdrop`, group 1).
const BACKDROP_BYTES: u64 = 3 * 16;
/// Plate rotation table: `MAX_PLATES` `vec4<f32>` (WGSL `plate_axes`).
const AXES_BYTES: u64 = MAX_PLATES as u64 * 16;

struct Layer {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    ice: wgpu::Texture,
    ice_view: wgpu::TextureView,
    n: u32,
    key: u64,
}

struct Resources {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    uniform: wgpu::Buffer,
    layers: Vec<Layer>,
    current: usize,
    key: Option<u64>,
    swapped_at: f32,
    bind: Option<wgpu::BindGroup>,
    /// Newer history frame's plate per texel (R16Uint) and plate rotations.
    plates: wgpu::Texture,
    plates_view: wgpu::TextureView,
    axes: wgpu::Buffer,
    motion_key: Option<u64>,
    backdrop: wgpu::RenderPipeline,
    backdrop_uniform: wgpu::Buffer,
    backdrop_bind: wgpu::BindGroup,
}

/// Orientation and weather for one frame.
#[derive(Clone, Copy)]
pub struct Pose {
    /// Planet frame to view frame.
    pub rotation: Mat3,
    /// Sun direction in the view frame.
    pub sun: Vec3,
    pub clouds: f32,
    pub cloud_angle: f32,
    /// Sub-texel grain strength, 0..=1. Grain is fixed to the sphere, so a
    /// map whose crust moves (deep-time playback) turns it off.
    pub detail: f32,
}

struct GlobeCallback {
    surface: Arc<Surface>,
    history: Option<(Arc<Surface>, f32)>,
    motion: Option<Arc<Motion>>,
    centre: Pos2,
    radius: f32,
    pose: Pose,
    seconds: f32,
    fade_seconds: f32,
    scan: [f32; 2],
}

/// Paints the globe centred on `centre`; the callback rect covers the air shell.
pub fn paint(p: &Painter, centre: Pos2, radius: f32, surface: Arc<Surface>, pose: Pose, seconds: f32, fade_seconds: f32, scan: [f32; 2]) {
    let rect = Rect::from_center_size(centre, vec2(radius, radius) * 2.2);
    p.add(Callback::new_paint_callback(rect, GlobeCallback { surface, history: None, motion: None, centre, radius, pose, seconds, fade_seconds, scan }));
}

/// Upload only the two published epochs; continuous playback changes a uniform.
/// Two history frames `blend` apart; with `motion` the shader carries each
/// plate along its rotation between them instead of dissolving.
#[allow(clippy::too_many_arguments)]
pub(super) fn paint_history(p: &Painter, centre: Pos2, radius: f32,
    from: Arc<Surface>, to: Arc<Surface>, blend: f32, motion: Option<Arc<Motion>>,
    pose: Pose, seconds: f32, scan: [f32; 2])
{
    let rect = Rect::from_center_size(centre, vec2(radius, radius) * 2.2);
    p.add(Callback::new_paint_callback(rect, GlobeCallback { surface: to,
        history: Some((from, blend)), motion, centre, radius, pose, seconds, fade_seconds: 0.0, scan }));
}

struct BackdropCallback {
    centre: Pos2,
    radius: f32,
    sun: Vec3,
    seconds: f32,
    still: bool,
}

/// Space behind the globe over all of `screen`; paint it before the globe.
pub fn paint_backdrop(p: &Painter, screen: Rect, centre: Pos2, radius: f32, sun: Vec3, seconds: f32, still: bool) {
    p.add(Callback::new_paint_callback(screen, BackdropCallback { centre, radius, sun, seconds, still }));
}

impl CallbackTrait for BackdropCallback {
    fn prepare(
        &self,
        _: &wgpu::Device,
        queue: &wgpu::Queue,
        screen: &ScreenDescriptor,
        _: &mut wgpu::CommandEncoder,
        resources: &mut CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let Some(r) = resources.get_mut::<Resources>() else {
            return Vec::new();
        };
        let ppp = screen.pixels_per_point;
        let sun = self.sun.normalize_or_zero();
        let data: [f32; 12] = [
            self.centre.x * ppp, self.centre.y * ppp, self.radius * ppp, self.seconds,
            sun.x, sun.y, sun.z, if self.still { 0.0 } else { 1.0 },
            screen.size_in_pixels[0] as f32, screen.size_in_pixels[1] as f32, ppp, 0.0,
        ];
        queue.write_buffer(&r.backdrop_uniform, 0, bytemuck::cast_slice(&data));
        Vec::new()
    }

    fn paint(&self, _: egui::PaintCallbackInfo, pass: &mut wgpu::RenderPass<'static>, resources: &CallbackResources) {
        let Some(r) = resources.get::<Resources>() else {
            return;
        };
        pass.set_pipeline(&r.backdrop);
        pass.set_bind_group(1, &r.backdrop_bind, &[]);
        pass.draw(0..3, 0..1);
    }
}

impl Resources {
    fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, s: &Surface, seconds: f32) {
        let n = s.n as u32;
        let slot = if self.layers.len() < 2 { self.layers.len() } else { 1 - self.current };
        if self.layers.get(slot).is_none_or(|l| l.n != n) {
            let desc = wgpu::TextureDescriptor {
                label: Some("Loading globe faces"),
                size: wgpu::Extent3d { width: n, height: n, depth_or_array_layers: 6 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            };
            let texture = device.create_texture(&desc);
            let ice = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Loading globe sea ice and emission"),
                format: wgpu::TextureFormat::Rgba8Unorm,
                ..desc
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2Array),
                ..Default::default()
            });
            let ice_view = ice.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2Array),
                ..Default::default()
            });
            let layer = Layer { texture, view, ice, ice_view, n, key: s.key };
            if slot < self.layers.len() {
                self.layers[slot] = layer;
            } else {
                self.layers.push(layer);
            }
        }
        let texels = s.rgba16();
        self.layers[slot].key = s.key;
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.layers[slot].texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytemuck::cast_slice(&texels),
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(n * 8), rows_per_image: Some(n) },
            wgpu::Extent3d { width: n, height: n, depth_or_array_layers: 6 },
        );
        // r: sea ice cover; gba: seam emission.
        let unit = |v: f32| (255.0 * v.clamp(0.0, 1.0)).round() as u8;
        let ice: Vec<u8> = s.texels.iter()
            .flat_map(|t| [unit(t.sea_ice), unit(t.glow[0]), unit(t.glow[1]), unit(t.glow[2])])
            .collect();
        queue.write_texture(
            self.layers[slot].ice.as_image_copy(),
            &ice,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(n * 4), rows_per_image: Some(n) },
            wgpu::Extent3d { width: n, height: n, depth_or_array_layers: 6 },
        );
        // The first map has nothing to fade from.
        self.swapped_at = if self.key.is_none() { f32::NEG_INFINITY } else { seconds };
        self.current = slot;
        self.key = Some(s.key);
        self.rebind(device);
    }
}

impl Resources {
    fn rebind(&mut self, device: &wgpu::Device) {
        let Some(now) = self.layers.get(self.current) else {
            return;
        };
        let before = if self.layers.len() > 1 { &self.layers[1 - self.current] } else { now };
        self.bind = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Loading globe"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.uniform.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&now.view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&before.view) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(&now.ice_view) },
                wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::TextureView(&before.ice_view) },
                wgpu::BindGroupEntry { binding: 6, resource: wgpu::BindingResource::TextureView(&self.plates_view) },
                wgpu::BindGroupEntry { binding: 7, resource: self.axes.as_entire_binding() },
            ],
        }));
    }

    /// Plate ownership and rotations for the newer frame; once per interval.
    fn upload_motion(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, m: &Motion) {
        let n = m.n as u32;
        if self.plates.width() != n {
            self.plates = plate_texture(device, n);
            self.plates_view = self.plates.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2Array),
                ..Default::default()
            });
        }
        queue.write_texture(
            self.plates.as_image_copy(),
            bytemuck::cast_slice(&m.plates),
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(n * 2), rows_per_image: Some(n) },
            wgpu::Extent3d { width: n, height: n, depth_or_array_layers: 6 },
        );
        let mut axes = [0.0f32; MAX_PLATES * 4];
        for (k, a) in m.axes.iter().take(MAX_PLATES).enumerate() {
            axes[k * 4..k * 4 + 3].copy_from_slice(a);
        }
        queue.write_buffer(&self.axes, 0, bytemuck::cast_slice(&axes));
        self.motion_key = Some(m.key);
        self.rebind(device);
    }
}

fn plate_texture(device: &wgpu::Device, n: u32) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Loading globe plates"),
        size: wgpu::Extent3d { width: n, height: n, depth_or_array_layers: 6 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::R16Uint,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

pub(in crate::egui_ui) fn install(renderer: &mut egui_wgpu::Renderer, device: &wgpu::Device, format: wgpu::TextureFormat) {
    let uniform = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Loading globe pose"),
        size: UNIFORM_BYTES,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let texture_entry = |binding| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2Array,
            multisampled: false,
        },
        count: None,
    };
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Loading globe"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(UNIFORM_BYTES),
                },
                count: None,
            },
            texture_entry(1),
            texture_entry(2),
            texture_entry(4),
            texture_entry(5),
            wgpu::BindGroupLayoutEntry {
                binding: 6,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Uint,
                    view_dimension: wgpu::TextureViewDimension::D2Array,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 7,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(AXES_BYTES),
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    });
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("Loading globe faces"),
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Loading globe"),
        source: wgpu::ShaderSource::Wgsl(crate::shader_sources::loading_globe_shader().into()),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Loading globe"),
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Loading globe"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs_main"), buffers: &[], compilation_options: Default::default() },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    });
    let backdrop_uniform = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Loading backdrop"),
        size: BACKDROP_BYTES,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let backdrop_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Loading backdrop"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: wgpu::BufferSize::new(BACKDROP_BYTES),
            },
            count: None,
        }],
    });
    let backdrop_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Loading backdrop"),
        layout: &backdrop_layout,
        entries: &[wgpu::BindGroupEntry { binding: 0, resource: backdrop_uniform.as_entire_binding() }],
    });
    // Group 0 (the globe's textures) is unused by the backdrop entry point.
    let backdrop_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Loading backdrop"),
        bind_group_layouts: &[None, Some(&backdrop_layout)],
        immediate_size: 0,
    });
    let backdrop = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Loading backdrop"),
        layout: Some(&backdrop_pipeline_layout),
        vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs_main"), buffers: &[], compilation_options: Default::default() },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_backdrop"),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    });
    let plates = plate_texture(device, 1);
    let plates_view = plates.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
    });
    let axes = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Loading globe plate rotations"),
        size: AXES_BYTES,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    renderer.callback_resources.insert(Resources {
        pipeline,
        layout,
        sampler,
        uniform,
        layers: Vec::new(),
        current: 0,
        key: None,
        swapped_at: f32::NEG_INFINITY,
        bind: None,
        plates,
        plates_view,
        axes,
        motion_key: None,
        backdrop,
        backdrop_uniform,
        backdrop_bind,
    });
}

impl CallbackTrait for GlobeCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        screen: &ScreenDescriptor,
        _: &mut wgpu::CommandEncoder,
        resources: &mut CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let Some(r) = resources.get_mut::<Resources>() else {
            return Vec::new();
        };
        if let Some((from, _)) = &self.history {
            let previous = r.layers.get(1 - r.current.min(1)).map(|l| l.key);
            if from.key != self.surface.key
                && !(r.key == Some(self.surface.key) && previous == Some(from.key))
                && r.key != Some(from.key)
            {
                r.upload(device, queue, from, self.seconds);
            }
        }
        if r.key != Some(self.surface.key) {
            r.upload(device, queue, &self.surface, self.seconds);
        }
        // Carry plates only when both endpoints are the resident layers.
        let carry = match (&self.motion, &self.history) {
            (Some(m), Some((from, blend))) if from.key != self.surface.key && m.n == self.surface.n => {
                if r.motion_key != Some(m.key) {
                    r.upload_motion(device, queue, m);
                }
                blend.clamp(0.0, 1.0)
            }
            _ => -1.0,
        };
        let fade = if let Some((from, blend)) = &self.history {
            if from.key == self.surface.key { 1.0 } else { blend.clamp(0.0, 1.0) }
        } else if self.fade_seconds == 0.0 {
            1.0
        } else {
            ((self.seconds - r.swapped_at) / self.fade_seconds).clamp(0.0, 1.0)
        };
        let previous_n = r.layers.get(1 - r.current.min(1)).map_or(self.surface.n as f32, |l| l.n as f32);
        let ppp = screen.pixels_per_point;
        let m = self.pose.rotation;
        let sun = self.pose.sun.normalize_or_zero();
        let data: [f32; 28] = [
            m.row(0).x, m.row(0).y, m.row(0).z, self.pose.detail,
            m.row(1).x, m.row(1).y, m.row(1).z, carry,
            m.row(2).x, m.row(2).y, m.row(2).z, 0.0,
            sun.x, sun.y, sun.z, self.seconds,
            self.centre.x * ppp, self.centre.y * ppp, self.radius * ppp, RELIEF,
            self.surface.n as f32, previous_n, fade, self.surface.radius_km,
            self.pose.clouds, self.pose.cloud_angle, self.scan[0], self.scan[1],
        ];
        queue.write_buffer(&r.uniform, 0, bytemuck::cast_slice(&data));
        Vec::new()
    }

    fn paint(&self, _: egui::PaintCallbackInfo, pass: &mut wgpu::RenderPass<'static>, resources: &CallbackResources) {
        let Some(r) = resources.get::<Resources>() else {
            return;
        };
        let Some(bind) = &r.bind else {
            return;
        };
        pass.set_pipeline(&r.pipeline);
        pass.set_bind_group(0, bind, &[]);
        pass.draw(0..3, 0..1);
    }
}
