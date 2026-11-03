//! Globe tile pipeline: uniforms, per-tile instances and the draw.

use super::bake::TileData;
use super::chart;
use super::gpu_tiles::{MAX_RESIDENT, TilePool};
use super::scheduler::Selection;
use super::tile::{TILE_CELLS, TILE_SAMPLES, TileId};
use glam::{DVec3, Mat3, Mat4, Vec3};

pub const INDEX_COUNT: u32 = TILE_CELLS * TILE_CELLS * 6;

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GlobeUniform {
    /// Absolute eye in body space; only lighting and the air path read it.
    pub eye_body: [f32; 4],
    pub sun_dir_body: [f32; 4],
    pub radius_m: f32,
    pub sea_level_m: f32,
    pub focal_px: f32,
    /// Bit 0 lean tier, bit 1 cloud shadow on, bit 2 night lights on.
    pub flags: u32,
    /// Glow colour rgb, strength.
    pub limb: [f32; 4],
    /// Camera-relative body-space view-projection with reverse-Z.
    pub view_proj: [[f32; 4]; 4],
}
const _: [(); 128] = [(); std::mem::size_of::<GlobeUniform>()];

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TileInstance {
    /// Tile centre minus eye, f32 after the f64 subtraction.
    pub origin_body: [f32; 4],
    pub chart_min: [f32; 2],
    pub cell_m: f32,
    pub face: u32,
    /// Face-local chart point of the tile centre, folded at height 0 by the shader.
    pub centre: [f32; 2],
    pub slot: u32,
    pub pad: u32,
}
const _: [(); 48] = [(); std::mem::size_of::<TileInstance>()];

/// Eye in body space and the body-space view-projection for a chart-space camera.
///
/// `projection` is the reverse-Z projection the terrain uses. The chart frame maps the
/// camera's east/up/north axes to body space, so the camera keeps its local heading.
pub fn body_view_proj(eye_chart_m: DVec3, forward: Vec3, projection: Mat4) -> (DVec3, Mat4) {
    let frame = chart::frame(eye_chart_m);
    let to_body = Mat3::from_cols(frame.x_axis.as_vec3(), frame.y_axis.as_vec3(), frame.z_axis.as_vec3());
    let forward_body = (to_body * forward).normalize();
    let up_body = to_body * Vec3::Y;
    let view = Mat4::look_to_rh(Vec3::ZERO, forward_body, up_body);
    (chart::to_body(eye_chart_m), projection * view)
}

pub struct Globe {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    uniform: wgpu::Buffer,
    instances: wgpu::Buffer,
    indices: wgpu::Buffer,
    tiles: TilePool,
    draw_count: u32,
    instance_scratch: Vec<TileInstance>,
}

/// Per-frame globe counters for the render stats.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameStats {
    pub uploaded: usize,
    pub dropped: usize,
    pub drawn: usize,
    pub resident: usize,
}

impl Globe {
    /// Builds the pipeline for the terrain pass's two `color_format` targets and reverse-Z depth.
    pub fn new(device: &wgpu::Device, color_format: wgpu::TextureFormat) -> Globe {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("globe shader"),
            source: wgpu::ShaderSource::Wgsl(crate::shader_sources::globe_shader_source().into()),
        });
        let entry = |binding, visibility, ty| wgpu::BindGroupLayoutEntry { binding, visibility, ty, count: None };
        let vertex_fragment = wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT;
        let texture = |sample_type| wgpu::BindingType::Texture {
            sample_type,
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("globe pipeline layout"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let target = |blend| Some(wgpu::ColorTargetState { format: color_format, blend, write_mask: wgpu::ColorWrites::ALL });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("globe pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[target(Some(wgpu::BlendState::REPLACE)), target(None)],
            }),
            primitive: wgpu::PrimitiveState { cull_mode: Some(wgpu::Face::Back), ..Default::default() },
            // Same depth state as the world pipeline, so chunks and assets sort against the globe.
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Greater,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globe uniform"),
            size: std::mem::size_of::<GlobeUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let instances = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globe tile instances"),
            size: (MAX_RESIDENT * std::mem::size_of::<TileInstance>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let indices = wgpu::util::DeviceExt::create_buffer_init(device, &wgpu::util::BufferInitDescriptor {
            label: Some("globe grid indices"),
            contents: bytemuck::cast_slice(&grid_indices()),
            usage: wgpu::BufferUsages::INDEX,
        });
        let tiles = TilePool::new(device);
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("globe bind group"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: uniform.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: instances.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&tiles.height_view) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&tiles.climate_view) },
            ],
        });
        Globe { pipeline, bind_group, uniform, instances, indices, tiles, draw_count: 0, instance_scratch: Vec::with_capacity(MAX_RESIDENT) }
    }

    /// The only production entry point: evict, upload, then prepare, in that order.
    ///
    /// A slot freed and refilled after `prepare` would make an instance written this
    /// frame draw another tile's data.
    pub fn frame(
        &mut self,
        queue: &wgpu::Queue,
        incoming: Vec<TileData>,
        selection: &Selection,
        eye_body: DVec3,
        uniform: GlobeUniform,
    ) -> FrameStats {
        self.evict_unused(&selection.draw, MAX_RESIDENT, incoming.len(), eye_body);
        let dropped = self.upload(queue, &incoming);
        self.prepare(queue, selection, eye_body, uniform);
        FrameStats {
            uploaded: incoming.len() - dropped,
            dropped,
            drawn: self.draw_count as usize,
            resident: self.tiles.resident(),
        }
    }

    /// Returns the number of tiles dropped because the pool was full.
    pub(crate) fn upload(&mut self, queue: &wgpu::Queue, tiles: &[TileData]) -> usize {
        tiles.iter().filter(|tile| !self.tiles.upload(queue, tile)).count()
    }

    pub(crate) fn evict_unused(&mut self, keep: &[TileId], max_resident: usize, incoming: usize, eye_body: DVec3) {
        self.tiles.evict_unused(keep, max_resident, incoming, eye_body);
    }

    /// Writes the uniform and one instance per resident tile of `selection.draw`.
    pub(crate) fn prepare(&mut self, queue: &wgpu::Queue, selection: &Selection, eye_body: DVec3, uniform: GlobeUniform) {
        self.instance_scratch.clear();
        for id in &selection.draw {
            if self.instance_scratch.len() == MAX_RESIDENT {
                break;
            }
            if let Some(slot) = self.tiles.slot(*id) {
                self.instance_scratch.push(instance(*id, slot, eye_body));
            }
        }
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&uniform));
        if !self.instance_scratch.is_empty() {
            queue.write_buffer(&self.instances, 0, bytemuck::cast_slice(&self.instance_scratch));
        }
        self.draw_count = self.instance_scratch.len() as u32;
    }

    #[cfg(test)]
    pub fn instance_capacity(&self) -> usize {
        self.instance_scratch.capacity()
    }

    #[cfg(test)]
    pub fn free_slots(&self) -> usize {
        self.tiles.free_slots()
    }

    /// Replaces the caller's group 0 and pipeline; the caller rebinds its own afterwards.
    pub fn draw<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        if self.draw_count == 0 {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint16);
        pass.draw_indexed(0..INDEX_COUNT, 0, 0..self.draw_count);
    }

    pub fn resident(&self) -> usize {
        self.tiles.resident()
    }
}

fn instance(id: TileId, slot: u32, eye_body: DVec3) -> TileInstance {
    let min = id.chart_min();
    let centre = (min + id.chart_max()) * 0.5;
    let origin = (id.centre_body() - eye_body).as_vec3();
    TileInstance {
        origin_body: [origin.x, origin.y, origin.z, 0.0],
        chart_min: [min.x as f32, min.y as f32],
        cell_m: id.cell_m() as f32,
        face: u32::from(id.face),
        centre: [centre.x as f32, centre.y as f32],
        slot,
        pad: 0,
    }
}

/// Two triangles per cell, wound counter-clockwise seen from above the surface.
fn grid_indices() -> Vec<u16> {
    let mut out = Vec::with_capacity(INDEX_COUNT as usize);
    for j in 0..TILE_CELLS {
        for i in 0..TILE_CELLS {
            let v = |di: u32, dj: u32| ((j + dj) * TILE_SAMPLES + i + di) as u16;
            out.extend_from_slice(&[v(0, 0), v(0, 1), v(1, 0), v(1, 0), v(0, 1), v(1, 1)]);
        }
    }
    out
}
