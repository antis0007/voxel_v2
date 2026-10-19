use glam::{Mat4, Vec3, Vec4};

use crate::mesh::Vertex3;
use crate::render::{ChunkCache, render_distance_settings};
use crate::world::CHUNK_SIZE;
static ENABLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);
pub fn enabled() -> bool { ENABLED.load(std::sync::atomic::Ordering::Relaxed) }
pub fn set_enabled(value: bool) { ENABLED.store(value, std::sync::atomic::Ordering::Relaxed); }

pub const SHADOW_MAP_SIZE: u32 = 2048;
pub const CASCADE_COUNT: usize = 4;
pub const SHADOW_UPDATE_EPSILON: f32 = 2.0e-4;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CascadeData {
    pub light_view_proj: [[f32; 4]; 4],
    /// Normalized texture-coordinate spacing, not world metres.
    pub texel_size: f32,
    pub base_bias: f32,
    pub slope_bias: f32,
    pub far_plane: f32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ShadowUniform {
    pub cascades: [CascadeData; CASCADE_COUNT],
    pub cascade_count: u32,
    pub blend_width: f32,
    pub world_scale: f32,
    pub _padding: u32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct ShadowGenUniform {
    light_view_proj: [[f32; 4]; 4],
    world_scale: f32,
    _padding: [u32; 3],
}

pub struct ShadowRenderer {
    pub uniform_buffer: wgpu::Buffer,
    pub depth_view: wgpu::TextureView,
    pub depth_sampler: wgpu::Sampler,

    gen_buffers: [wgpu::Buffer; CASCADE_COUNT],
    gen_bind_groups: [wgpu::BindGroup; CASCADE_COUNT],
    pipeline: wgpu::RenderPipeline,
    layer_views: [wgpu::TextureView; CASCADE_COUNT],

    active_uniform: ShadowUniform,
    desired_uniform: ShadowUniform,
    pub shadow_map_dirty: bool,
    pub last_mesh_revision: u64,
    pending_cascades: u8,
    urgent_cascades: u8,
    frame_cascades: u8,
    far_cursor: usize,
    initialized: bool,
    force_refresh: bool,
    active_light_dirs: [Vec3; CASCADE_COUNT],
    desired_light_dirs: [Vec3; CASCADE_COUNT],

    cascade_centers: [Vec3; CASCADE_COUNT],
    cascade_radii: [f32; CASCADE_COUNT],
    pub draw_lists: [Vec<glam::IVec3>; CASCADE_COUNT],
}

fn unproject_clip(inv_view_proj: Mat4, ndc: Vec3) -> Vec3 {
    let clip = Vec4::new(ndc.x, ndc.y, ndc.z, 1.0);
    let world = inv_view_proj * clip;
    world.truncate() / world.w.max(1e-6)
}

fn covers_receivers(cascade: &CascadeData, corners: &[Vec3; 8]) -> bool {
    let matrix = Mat4::from_cols_array_2d(&cascade.light_view_proj);
    // Leave three shadow texels for the PCF footprint at the map edge.
    let edge = 1.0 - 6.0 / SHADOW_MAP_SIZE as f32;
    corners.iter().all(|&p| {
        let q = matrix * p.extend(1.0);
        q.is_finite() && q.w > 0.0 && q.x.abs() <= edge * q.w
            && q.y.abs() <= edge * q.w && q.z > 0.0 && q.z < q.w
    })
}

impl ShadowRenderer {
    pub fn new(device: &wgpu::Device, shadow_shader: &wgpu::ShaderModule) -> Self {
        let depth_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("shadow depth array"),
            size: wgpu::Extent3d {
                width: SHADOW_MAP_SIZE,
                height: SHADOW_MAP_SIZE,
                depth_or_array_layers: CASCADE_COUNT as u32,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let depth_view = depth_tex.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });

        let layer_views = std::array::from_fn(|i| {
            depth_tex.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2),
                base_array_layer: i as u32,
                array_layer_count: Some(1),
                ..Default::default()
            })
        });

        let depth_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("shadow sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });

        let gen_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("shadow gen layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("shadow gen pipeline layout"),
            bind_group_layouts: &[&gen_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("shadow gen pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: shadow_shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex3>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3],
                }],
            },
            fragment: None,
            primitive: wgpu::PrimitiveState {
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: Default::default(),
                bias: wgpu::DepthBiasState {
                    constant: 2,
                    slope_scale: 2.0,
                    clamp: 0.0,
                },
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });

        let gen_buffers = std::array::from_fn(|_| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("shadow gen buffer"),
                size: std::mem::size_of::<ShadowGenUniform>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        });

        let gen_bind_groups = std::array::from_fn(|i| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("shadow gen bind group"),
                layout: &gen_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: gen_buffers[i].as_entire_binding(),
                }],
            })
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("shadow uniform buffer"),
            size: std::mem::size_of::<ShadowUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            uniform_buffer,
            depth_view,
            depth_sampler,
            gen_buffers,
            gen_bind_groups,
            pipeline,
            layer_views,
            active_uniform: unsafe { std::mem::zeroed() },
            desired_uniform: unsafe { std::mem::zeroed() },
            shadow_map_dirty: true,
            last_mesh_revision: u64::MAX,
            pending_cascades: 0,
            urgent_cascades: 0,
            frame_cascades: 0,
            far_cursor: 1,
            initialized: false,
            force_refresh: true,
            active_light_dirs: [Vec3::ZERO; CASCADE_COUNT],
            desired_light_dirs: [Vec3::ZERO; CASCADE_COUNT],
            cascade_centers: [Vec3::ZERO; CASCADE_COUNT],
            cascade_radii: [0.0; CASCADE_COUNT],
            draw_lists: [Vec::new(), Vec::new(), Vec::new(), Vec::new()],
        }
    }

    pub fn update(
        &mut self,
        light_dir: Vec3,
        inv_view_proj: Mat4,
        camera_near: f32,
        camera_far: f32,
        world_scale: f32,
    ) {
        let safe_dir = if light_dir.length_squared() > 1e-6 {
            light_dir.normalize()
        } else {
            Vec3::new(0.3, 1.0, 0.2).normalize()
        };
        let scale = world_scale.max(0.25);
        let render_settings = render_distance_settings();

        let lambda = 0.65;
        let n_world = (0.1 * scale).max(0.1);
        let target_f = (render_settings.view_distance_chunks as f32 * CHUNK_SIZE as f32).max(128.0);
        let f_world = target_f.min(512.0).max(192.0) * scale;

        let mut splits = [0.0; CASCADE_COUNT + 1];
        splits[0] = n_world;
        for i in 1..=CASCADE_COUNT {
            let p = i as f32 / CASCADE_COUNT as f32;
            let log = n_world * (f_world / n_world).powf(p);
            let uni = n_world + (f_world - n_world) * p;
            splits[i] = lambda * log + (1.0 - lambda) * uni;
        }

        let up = if safe_dir.y.abs() > 0.96 {
            Vec3::X
        } else {
            Vec3::Y
        };

        let mut cascades = [CascadeData {
            light_view_proj: Mat4::IDENTITY.to_cols_array_2d(),
            texel_size: 0.0,
            base_bias: 0.0,
            slope_bias: 0.0,
            far_plane: 0.0,
        }; CASCADE_COUNT];
        self.desired_light_dirs = [safe_dir; CASCADE_COUNT];

        // The visual frustum extends to the horizon, while shadow coverage stays
        // local. Normalize against the actual projection depth, not the shadow limit.
        let projection_depth = unproject_clip(inv_view_proj, Vec3::new(0.0, 0.0, 1.0))
            .distance(unproject_clip(inv_view_proj, Vec3::ZERO)).max(1e-6);
        for i in 0..CASCADE_COUNT {
            let ratio_near = (splits[i] - camera_near) / projection_depth;
            let ratio_far = (splits[i + 1] - camera_near) / projection_depth;

            let mut corners = [Vec3::ZERO; 8];
            for (idx, &x) in [-1.0, 1.0].iter().enumerate() {
                for (jdx, &y) in [-1.0, 1.0].iter().enumerate() {
                    let near_pt = unproject_clip(inv_view_proj, Vec3::new(x, y, 0.0));
                    let far_pt = unproject_clip(inv_view_proj, Vec3::new(x, y, 1.0));
                    let ray = far_pt - near_pt;
                    let base_idx = (idx * 2 + jdx) * 2;
                    corners[base_idx] = near_pt + ray * ratio_near;
                    corners[base_idx + 1] = near_pt + ray * ratio_far;
                }
            }

            let mut center = Vec3::ZERO;
            for c in &corners {
                center += *c;
            }
            center /= 8.0;

            let mut radius = 0.0f32;
            for c in &corners {
                radius = radius.max(c.distance(center));
            }

            // Discontinuous camera jumps must not reuse shadow projections.
            if i == 0 && center.distance(self.cascade_centers[i]) > radius.max(1.0) * 0.25 {
                self.force_refresh = true;
            }
            self.cascade_centers[i] = center;
            self.cascade_radii[i] = radius;

            // A camera pan does not change an existing world-space shadow.
            // Reuse its actual texture/matrix pair while every new receiver
            // still fits, rather than rerasterizing all four maps each frame.
            if self.initialized && self.active_uniform.world_scale == scale
                && (self.active_uniform.cascades[i].far_plane - splits[i + 1]).abs() < 1e-4
                && safe_dir.distance_squared(self.active_light_dirs[i]) <= 1e-8
                && covers_receivers(&self.active_uniform.cascades[i], &corners)
            {
                cascades[i] = self.active_uniform.cascades[i];
                self.desired_light_dirs[i] = self.active_light_dirs[i];
                continue;
            }
            self.pending_cascades |= 1 << i;
            self.urgent_cascades |= 1 << i;

            // Use a massive Z-pullback to ensure tall objects and far-away terrain
            // casting long shadows at sunrise/sunset are never incorrectly clipped.
            let z_pullback = 8000.0 * scale;
            let light_eye = center + safe_dir * radius;
            // Avoid subtracting two large world positions to recover the light
            // direction: that rotates the basis slightly during distant travel.
            let view = Mat4::look_to_rh(light_eye, -safe_dir, up);

            // A rotation-invariant sphere, with quantized extent, keeps the
            // shadow texel grid from breathing with floating-point camera noise.
            let extent = ((radius + 7.0 * scale) / scale).ceil() * scale;
            let texel_size = (extent * 2.0) / SHADOW_MAP_SIZE as f32;

            let origin_ls = view.transform_point3(Vec3::ZERO);
            let offset_x = origin_ls.x - (origin_ls.x / texel_size).round() * texel_size;
            let offset_y = origin_ls.y - (origin_ls.y / texel_size).round() * texel_size;

            let near_plane = -z_pullback;
            let far_plane = radius * 2.0 + z_pullback;

            let proj = Mat4::orthographic_rh(
                -extent + offset_x,
                extent + offset_x,
                -extent + offset_y,
                extent + offset_y,
                near_plane,
                far_plane,
            );

            cascades[i] = CascadeData {
                light_view_proj: (proj * view).to_cols_array_2d(),
                texel_size: 1.0 / SHADOW_MAP_SIZE as f32,
                // Convert a small physical receiver bias to normalized depth.
                // Fixed depth biases over this 16km range detached shadows by metres.
                base_bias: (0.025 * scale + texel_size * 0.35) / (far_plane - near_plane),
                slope_bias: (0.06 * scale + texel_size * 1.5) / (far_plane - near_plane),
                far_plane: splits[i + 1],
            };
        }

        self.desired_uniform = ShadowUniform {
            cascades,
            cascade_count: CASCADE_COUNT as u32,
            blend_width: 8.0 * scale,
            world_scale: scale,
            _padding: 0,
        };

        if self.active_uniform.world_scale != self.desired_uniform.world_scale
            || self
                .active_uniform
                .cascades
                .iter()
                .zip(&cascades)
                .any(|(a, b)| (a.far_plane - b.far_plane).abs() > 0.01)
        {
            self.force_refresh = true;
        }
        // Each cascade compares against the matrix actually stored in its own
        // texture layer. Deferred layers never publish an unmatched matrix.
        for i in 0..CASCADE_COUNT {
            let a = &self.active_uniform.cascades[i].light_view_proj;
            let b = &self.desired_uniform.cascades[i].light_view_proj;
            let delta = a
                .iter()
                .flatten()
                .zip(b.iter().flatten())
                .map(|(x, y)| (x - y).abs())
                .fold(0.0f32, f32::max);
            if delta > SHADOW_UPDATE_EPSILON {
                self.pending_cascades |= 1 << i;
                // Refresh every changed projection immediately, but retain
                // other layers whose world-space coverage is still valid.
                self.urgent_cascades |= 1 << i;
            }
        }
    }

    pub fn apply_update_if_needed(&mut self, queue: &wgpu::Queue, mesh_revision: u64) -> bool {
        const ALL: u8 = (1 << CASCADE_COUNT) - 1;
        if self.shadow_map_dirty || self.last_mesh_revision != mesh_revision {
            self.pending_cascades |= ALL;
        }
        self.frame_cascades = 0;
        if self.pending_cascades == 0 {
            return false;
        }
        if !self.initialized || self.force_refresh {
            self.frame_cascades = ALL;
        } else {
            // Nearby edits are visible immediately. Fairly rotate the outer
            // cascades: continuous uploads cannot starve a distant layer.
            self.frame_cascades = self.pending_cascades & (1 | self.urgent_cascades);
            for _ in 1..CASCADE_COUNT {
                let i = self.far_cursor;
                self.far_cursor = 1 + i % (CASCADE_COUNT - 1);
                if self.pending_cascades & (1 << i) != 0 && self.frame_cascades & (1 << i) == 0 {
                    self.frame_cascades |= 1 << i;
                    break;
                }
            }
        }
        self.active_uniform.cascade_count = self.desired_uniform.cascade_count;
        self.active_uniform.world_scale = self.desired_uniform.world_scale;
        self.active_uniform.blend_width = self.desired_uniform.blend_width;
        for i in 0..CASCADE_COUNT {
            if !self.renders_cascade(i) {
                continue;
            }
            self.active_uniform.cascades[i] = self.desired_uniform.cascades[i];
            self.active_light_dirs[i] = self.desired_light_dirs[i];
            let gen_uni = ShadowGenUniform {
                light_view_proj: self.active_uniform.cascades[i].light_view_proj,
                world_scale: self.active_uniform.world_scale,
                _padding: [0; 3],
            };
            queue.write_buffer(&self.gen_buffers[i], 0, bytemuck::bytes_of(&gen_uni));
        }
        queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::bytes_of(&self.active_uniform),
        );
        self.pending_cascades &= !self.frame_cascades;
        self.urgent_cascades &= !self.frame_cascades;
        self.shadow_map_dirty = false;
        self.last_mesh_revision = mesh_revision;
        self.initialized = true;
        self.force_refresh = false;
        true
    }

    pub fn renders_cascade(&self, i: usize) -> bool {
        self.frame_cascades & (1 << i) != 0
    }
    pub fn build_cascade_draw_lists(&mut self, cache: &ChunkCache) {
        // Rebuild and submit lists only for layers selected this frame.
        for i in 0..CASCADE_COUNT {
            if !self.renders_cascade(i) {
                continue;
            }
            self.draw_lists[i].clear();
            let planes = crate::render::extract_planes(Mat4::from_cols_array_2d(
                &self.active_uniform.cascades[i].light_view_proj,
            ));
            for (&coord, mesh) in &cache.gpu_meshes {
                if mesh.index_count == 0 {
                    continue;
                }
                let (center, radius) = crate::render::chunk_bounds_sphere(coord);
                let center = center * self.active_uniform.world_scale;
                let radius = radius * self.active_uniform.world_scale;
                if !planes
                    .iter()
                    .any(|p| p.truncate().dot(center) + p.w < -radius)
                {
                    self.draw_lists[i].push(coord);
                }
            }
        }
    }
    pub fn draw_call_count(&self) -> usize {
        (0..CASCADE_COUNT)
            .filter(|&i| self.renders_cascade(i))
            .map(|i| self.draw_lists[i].len())
            .sum()
    }

    pub fn render<'a>(
        &'a self,
        encoder: &mut wgpu::CommandEncoder,
        cache: &'a ChunkCache,
        timestamp_query_set: Option<&'a wgpu::QuerySet>,
        timestamp_queries: &[Option<(u32, u32)>; CASCADE_COUNT],
    ) {
        for i in 0..CASCADE_COUNT {
            if !self.renders_cascade(i) {
                continue;
            }
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(&format!("shadow pass {}", i)),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.layer_views[i],
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: timestamp_query_set.zip(timestamp_queries[i]).map(
                    |(query_set, (begin, end))| wgpu::RenderPassTimestampWrites {
                        query_set,
                        beginning_of_pass_write_index: Some(begin),
                        end_of_pass_write_index: Some(end),
                    },
                ),
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.gen_bind_groups[i], &[]);
            for coord in &self.draw_lists[i] {
                if let Some(mesh) = cache.gpu_meshes.get(coord) {
                    pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                    pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..mesh.index_count, 0, 0..1);
                }
            }
        }
    }
}
