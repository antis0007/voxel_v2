use glam::{DMat4, DVec2, DVec3, Mat4, Vec3, Vec4};

use crate::mesh::Vertex3;
use crate::render::{ChunkCache, render_distance_settings};
use crate::world::CHUNK_SIZE;
#[cfg(test)]
#[path = "shadow_contract.rs"]
mod contract;
#[cfg(test)]
#[path = "shadow_frame_contract.rs"]
mod frame_contract;
use super::origin::{ABSOLUTE_SLOT, DrawOffsets, DrawTransform, RenderOrigin};
use super::{coverage::ResidentCoverage, horizon::GpuMesh};
static ENABLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);
pub fn enabled() -> bool {
    ENABLED.load(std::sync::atomic::Ordering::Relaxed)
}
pub fn set_enabled(value: bool) {
    ENABLED.store(value, std::sync::atomic::Ordering::Relaxed);
}

pub const SHADOW_MAP_SIZE: u32 = 2048;
pub const NEAR_CASCADE_COUNT: usize = 4;
pub const CASCADE_COUNT: usize = NEAR_CASCADE_COUNT + 1;
pub const SHADOW_UPDATE_EPSILON: f32 = 2.0e-4;
pub const FAR_SHADOW_DISTANCE: f32 = 8192.0;

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
const _: [(); 80] = [(); std::mem::size_of::<CascadeData>()];
const _: [(); 416] = [(); std::mem::size_of::<ShadowUniform>()];
const _: [(); 400] = [(); std::mem::offset_of!(ShadowUniform, cascade_count)];

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct ShadowGenUniform {
    light_view_proj: [[f32; 4]; 4],
    world_scale: f32,
    time: f32,
    wind: [f32; 2],
    resident_coverage: ResidentCoverage,
    /// Voxel origin of `light_view_proj`; ownership rebuilds absolute positions.
    render_origin: [i32; 4],
}
const _: [(); 10416] = [(); std::mem::size_of::<ShadowGenUniform>()];
const _: [(); 80] = [(); std::mem::offset_of!(ShadowGenUniform, resident_coverage)];
const _: [(); 10400] = [(); std::mem::offset_of!(ShadowGenUniform, render_origin)];

/// Camera inputs the cascades are fitted around.
#[derive(Clone, Copy, Debug)]
pub struct CascadeView {
    /// Inverse view projection with the eye at zero.
    pub inv_view_proj: Mat4,
    /// Precise eye position, world metres.
    pub eye_m: DVec3,
    /// Render origin the cascade matrices are expressed in.
    pub origin: RenderOrigin,
}

impl CascadeView {
    pub const IDENTITY: CascadeView = CascadeView {
        inv_view_proj: Mat4::IDENTITY,
        eye_m: DVec3::ZERO,
        origin: RenderOrigin { voxel: glam::IVec3::ZERO },
    };
}

/// One cascade's slice of the view frustum.
struct FrustumSlice {
    /// Origin-frame corners, metres.
    corners: [Vec3; 8],
    /// Absolute centre, world metres.
    center_m: DVec3,
    radius: f32,
}

/// One cascade's light projection over origin-frame metres.
struct CascadeFit {
    light_view_proj: DMat4,
    /// World metres per shadow texel.
    texel_m: f64,
    near: f64,
    far: f64,
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
    /// Render origin of each layer's stored matrix.
    active_origins: [Option<RenderOrigin>; CASCADE_COUNT],
    desired_origin: RenderOrigin,

    /// Absolute world metres.
    cascade_centers: [DVec3; CASCADE_COUNT],
    cascade_radii: [f32; CASCADE_COUNT],
    pub draw_lists: [Vec<glam::IVec3>; CASCADE_COUNT],
    wind_time: [f32; 3],
    horizon_generation: u64,
    resident_coverage: ResidentCoverage,
    parent_draw_lists: [Vec<(usize, std::ops::Range<u32>)>; CASCADE_COUNT],
    /// Draw-offset slot of each `draw_lists` entry, index for index.
    draw_slots: [Vec<u32>; CASCADE_COUNT],
    /// One indirect buffer per cascade: writes to a shared one would all land
    /// before submit and every cascade would read the last list.
    indirect: [super::streaming::indirect_draws::IndirectDraws; CASCADE_COUNT],
    multidraw: bool,
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
        q.is_finite()
            && q.w > 0.0
            && q.x.abs() <= edge * q.w
            && q.y.abs() <= edge * q.w
            && q.z > 0.0
            && q.z < q.w
    })
}

/// Receiver split distances, metres: four near layers and the far layer.
fn cascade_splits(scale: f32) -> [f32; CASCADE_COUNT + 1] {
    let lambda = 0.65;
    let n_world = (0.1 * scale).max(0.1);
    let target_f = (render_distance_settings().view_distance_chunks as f32 * CHUNK_SIZE as f32).max(128.0);
    let f_world = target_f.clamp(192.0, 512.0) * scale;

    let mut splits = [0.0; CASCADE_COUNT + 1];
    splits[0] = n_world;
    for i in 1..=NEAR_CASCADE_COUNT {
        let p = i as f32 / NEAR_CASCADE_COUNT as f32;
        let log = n_world * (f_world / n_world).powf(p);
        let uni = n_world + (f_world - n_world) * p;
        splits[i] = lambda * log + (1.0 - lambda) * uni;
    }
    // Keep all four existing receiver intervals and their resolution.
    // One additional layer supplies broad terrain/canopy occlusion; the
    // receiver fades its final 22% smoothly to avoid a hard range edge.
    splits[CASCADE_COUNT] = FAR_SHADOW_DISTANCE;
    splits
}

/// Near-to-far length of the view projection, metres.
fn projection_depth(inv_view_proj: Mat4) -> f32 {
    // The visual frustum extends to the horizon, while shadow coverage stays
    // local. Normalize against the actual projection depth, not the shadow limit.
    unproject_clip(inv_view_proj, Vec3::new(0.0, 0.0, 1.0))
        .distance(unproject_clip(inv_view_proj, Vec3::ZERO))
        .max(1e-6)
}

/// Frustum slice between two depth ratios, measured from the eye.
fn frustum_slice(view: &CascadeView, ratio_near: f32, ratio_far: f32, origin_m: DVec3) -> FrustumSlice {
    let mut corners = [Vec3::ZERO; 8];
    for (idx, &x) in [-1.0, 1.0].iter().enumerate() {
        for (jdx, &y) in [-1.0, 1.0].iter().enumerate() {
            let near_pt = unproject_clip(view.inv_view_proj, Vec3::new(x, y, 0.0));
            let far_pt = unproject_clip(view.inv_view_proj, Vec3::new(x, y, 1.0));
            let ray = far_pt - near_pt;
            let base_idx = (idx * 2 + jdx) * 2;
            corners[base_idx] = near_pt + ray * ratio_near;
            corners[base_idx + 1] = near_pt + ray * ratio_far;
        }
    }
    let center = corners.iter().copied().sum::<Vec3>() / 8.0;
    let radius = corners.iter().fold(0.0f32, |r, c| r.max(c.distance(center)));
    // Eye-relative corners are small; move them to the origin frame in f64.
    let eye_rel = view.eye_m - origin_m;
    FrustumSlice {
        corners: corners.map(|c| (eye_rel + c.as_dvec3()).as_vec3()),
        center_m: view.eye_m + center.as_dvec3(),
        radius,
    }
}

/// Light-space texel phase of absolute world zero.
///
/// Snapping the ortho window to it keeps every cascade's texel grid fixed to
/// the world when the render origin moves.
pub fn world_locked_texel_offset(view_rel: DMat4, origin_m: DVec3, texel_size: f64) -> DVec2 {
    let zero = view_rel.transform_point3(-origin_m);
    let phase = |v: f64| v - (v / texel_size).round() * texel_size;
    DVec2::new(phase(zero.x), phase(zero.y))
}

/// Fits the light projection around a sphere, in origin-frame metres.
fn fit_cascade(center_m: DVec3, radius: f64, light_dir: DVec3, up: DVec3, origin_m: DVec3, scale: f64) -> CascadeFit {
    // A large Z-pullback keeps tall objects and far terrain that cast long
    // shadows at sunrise and sunset inside the light's depth range.
    let z_pullback = 8000.0 * scale;
    // Subtract the origin before anything rounds: far from spawn the
    // absolute centre has no f32 precision left.
    let light_eye = center_m - origin_m + light_dir * radius;
    // Avoid subtracting two large world positions to recover the light
    // direction: that rotates the basis slightly during distant travel.
    let view = DMat4::look_to_rh(light_eye, -light_dir, up);
    // A rotation-invariant sphere, with quantized extent, keeps the
    // shadow texel grid from breathing with floating-point camera noise.
    let extent = ((radius + 7.0 * scale) / scale).ceil() * scale;
    let texel_m = (extent * 2.0) / f64::from(SHADOW_MAP_SIZE);
    let offset = world_locked_texel_offset(view, origin_m, texel_m);
    let near = -z_pullback;
    let far = radius * 2.0 + z_pullback;
    let proj = DMat4::orthographic_rh(
        -extent + offset.x,
        extent + offset.x,
        -extent + offset.y,
        extent + offset.y,
        near,
        far,
    );
    CascadeFit { light_view_proj: proj * view, texel_m, near, far }
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
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
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
                buffers: &[
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<Vertex3>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &[
                            wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x3, offset: 0, shader_location: 0 },
                            wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 36, shader_location: 3 },
                            wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32, offset: 52, shader_location: 4 },
                        ],
                    },
                    DrawTransform::LAYOUT,
                ],
            },
            fragment: Some(wgpu::FragmentState {
                module: shadow_shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[],
            }),
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
            active_origins: [None; CASCADE_COUNT],
            desired_origin: CascadeView::IDENTITY.origin,
            cascade_centers: [DVec3::ZERO; CASCADE_COUNT],
            cascade_radii: [0.0; CASCADE_COUNT],
            draw_lists: std::array::from_fn(|_| Vec::new()),
            wind_time: [0.0; 3],
            horizon_generation: u64::MAX,
            resident_coverage: ResidentCoverage::default(),
            parent_draw_lists: std::array::from_fn(|_| Vec::new()),
            draw_slots: std::array::from_fn(|_| Vec::new()),
            indirect: std::array::from_fn(|_| Default::default()),
            multidraw: false,
        }
    }

    pub fn update(
        &mut self,
        light_dir: Vec3,
        view: CascadeView,
        camera_near: f32,
        world_scale: f32,
    ) {
        let safe_dir = if light_dir.length_squared() > 1e-6 {
            light_dir.normalize()
        } else {
            Vec3::new(0.3, 1.0, 0.2).normalize()
        };
        let scale = world_scale.max(0.25);
        let splits = cascade_splits(scale);
        let origin_m = view.origin.voxel.as_dvec3() * f64::from(scale);

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
        self.desired_origin = view.origin;

        let projection_depth = projection_depth(view.inv_view_proj);
        for i in 0..CASCADE_COUNT {
            let ratio_near = (splits[i] - camera_near) / projection_depth;
            let ratio_far = (splits[i + 1] - camera_near) / projection_depth;
            let FrustumSlice { corners, center_m, radius } =
                frustum_slice(&view, ratio_near, ratio_far, origin_m);

            // Discontinuous camera jumps must not reuse shadow projections.
            if i == 0 && center_m.distance(self.cascade_centers[i]) > f64::from(radius.max(1.0) * 0.25) {
                self.force_refresh = true;
            }
            self.cascade_centers[i] = center_m;
            self.cascade_radii[i] = radius;

            // A camera pan does not change an existing world-space shadow.
            // Reuse its actual texture/matrix pair while every new receiver
            // still fits, rather than rerasterizing all four maps each frame.
            // A matrix from another render origin is in another frame.
            if self.initialized
                && self.active_origins[i] == Some(view.origin)
                && self.active_uniform.world_scale == scale
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

            let fit = fit_cascade(
                center_m,
                f64::from(radius),
                safe_dir.as_dvec3(),
                up.as_dvec3(),
                origin_m,
                f64::from(scale),
            );
            let texel_size = fit.texel_m as f32;
            let depth_range = (fit.far - fit.near) as f32;
            cascades[i] = CascadeData {
                light_view_proj: fit.light_view_proj.as_mat4().to_cols_array_2d(),
                texel_size: 1.0 / SHADOW_MAP_SIZE as f32,
                // Convert a small physical receiver bias to normalized depth.
                // Fixed depth biases over this 16km range detached shadows by metres.
                base_bias: (0.025 * scale + texel_size * 0.35) / depth_range,
                slope_bias: (0.06 * scale + texel_size * 1.5) / depth_range,
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

    pub fn set_detail_wind(&mut self, time: f32, wind: [f32; 2]) {
        if self.wind_time != [time, wind[0], wind[1]]
            && (wind[0].abs() + wind[1].abs() + self.wind_time[1].abs() + self.wind_time[2].abs()) > 0.001
        {
            // Tuft shadows only belong to the two near cascades. Refresh both
            // with the same time as color/depth; distant cascades omit tufts.
            self.pending_cascades |= 3;
            self.urgent_cascades |= 3;
        }
        self.wind_time = [time, wind[0], wind[1]];
    }

    pub fn set_horizon_source(&mut self, generation: u64, coverage: &ResidentCoverage) {
        if self.horizon_generation != generation
            || self.resident_coverage != *coverage
        {
            self.horizon_generation = generation;
            self.resident_coverage = *coverage;
            // A parent publication or ownership handoff invalidates every
            // layer together. Do not retain shadows from an obsolete owner.
            self.shadow_map_dirty = true;
            self.force_refresh = true;
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
            self.active_origins[i] = Some(self.desired_origin);
            let gen_uni = ShadowGenUniform {
                light_view_proj: self.active_uniform.cascades[i].light_view_proj,
                world_scale: self.active_uniform.world_scale,
                time: if i < 2 { self.wind_time[0] } else { -1.0 },
                wind: [self.wind_time[1], self.wind_time[2]],
                resident_coverage: self.resident_coverage,
                render_origin: self.desired_origin.voxel.extend(0).to_array(),
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
    /// Takes draw-offset slots for this frame's cascade lists.
    ///
    /// Also writes indirect arguments when multi-draw is active. Call after
    /// `build_cascade_draw_lists` and before `offsets` is uploaded.
    pub fn prepare_draws(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        cache: &ChunkCache,
        offsets: &mut DrawOffsets,
        transform: impl Fn(glam::IVec3) -> DrawTransform,
    ) {
        self.multidraw = super::streaming::indirect_draws::active();
        for i in 0..CASCADE_COUNT {
            if !self.renders_cascade(i) {
                continue;
            }
            self.draw_slots[i].clear();
            let mut slot_of = |c| offsets.slot(c, || transform(c));
            self.draw_slots[i].extend(self.draw_lists[i].iter().map(|&c| slot_of(c)));
            if self.multidraw {
                let coords = self.draw_lists[i].iter().copied();
                self.indirect[i].prepare(device, queue, cache, coords, slot_of);
            }
        }
    }

    /// Cascade 0's pipeline and bind group, its uniform set to `light_view_proj`.
    #[cfg(test)]
    pub(in crate::render) fn test_cascade(
        &self,
        queue: &wgpu::Queue,
        light_view_proj: Mat4,
    ) -> (&wgpu::RenderPipeline, &wgpu::BindGroup) {
        let uniform = ShadowGenUniform {
            light_view_proj: light_view_proj.to_cols_array_2d(),
            world_scale: 1.0,
            time: 0.0,
            wind: [0.0; 2],
            resident_coverage: ResidentCoverage::default(),
            render_origin: [0; 4],
        };
        queue.write_buffer(&self.gen_buffers[0], 0, bytemuck::bytes_of(&uniform));
        (&self.pipeline, &self.gen_bind_groups[0])
    }

    pub fn build_cascade_draw_lists(&mut self, cache: &ChunkCache, horizon: Option<&GpuMesh>) {
        // Rebuild and submit lists only for layers selected this frame.
        for i in 0..CASCADE_COUNT {
            if !self.renders_cascade(i) {
                continue;
            }
            self.draw_lists[i].clear();
            self.parent_draw_lists[i].clear();
            let matrix = Mat4::from_cols_array_2d(&self.active_uniform.cascades[i].light_view_proj);
            let planes = crate::render::extract_planes(matrix);
            // The matrix maps origin-frame metres; offset the chunk in integers first.
            let origin = self.active_origins[i].unwrap_or(self.desired_origin);
            let scale = self.active_uniform.world_scale;
            let origin_m = origin.voxel.as_vec3() * scale;
            let chunk_center = |coord| {
                let corner = Vec4::from_array(origin.chunk_offset(coord)).truncate();
                (corner + Vec3::splat(CHUNK_SIZE as f32 * 0.5)) * scale
            };
            for (&coord, mesh) in &cache.gpu_meshes {
                if !cache.published_coords.contains(&coord) {continue;}
                if self.resident_coverage.header[3] != 0
                    && !self.resident_coverage.contains_column(coord.x, coord.z)
                {
                    continue;
                }
                if mesh.index_count == 0 {
                    continue;
                }
                let (_, radius) = crate::render::chunk_bounds_sphere(coord);
                let radius = radius + 0.04 / scale.max(0.0001);
                let center = chunk_center(coord);
                let radius = radius * scale;
                if !planes
                    .iter()
                    .any(|p| p.truncate().dot(center) + p.w < -radius)
                {
                    self.draw_lists[i].push(coord);
                }
            }
            // Depth is LessEqual in this pass. Fill the light's nearer surfaces
            // first so buried cave/fluid geometry can fail early depth testing.
            // Hash-map order otherwise changes after resident mesh updates.
            let depth = |coord| (matrix * chunk_center(coord).extend(1.0)).z;
            self.draw_lists[i].sort_unstable_by(|a, b| depth(*a).total_cmp(&depth(*b))
                .then_with(|| (a.x, a.y, a.z).cmp(&(b.x, b.y, b.z))));
            if let Some(horizon) = horizon {
                for (page_index, page) in horizon.pages.iter().enumerate() {
                    if !horizon.cull && !page.batches.iter().any(|b|b.conditional()) {
                        if page.count != 0 {
                            self.parent_draw_lists[i].push((page_index, 0..page.count));
                        }
                    } else {
                        for batch in &page.batches {
                            if batch.selected(horizon.collared) && (!horizon.cull || batch.visible(origin_m, &planes)) {
                                self.parent_draw_lists[i].push((page_index, batch.indices.clone()));
                            }
                        }
                    }
                }
            }
        }
    }
    pub fn draw_call_count(&self) -> usize {
        (0..CASCADE_COUNT)
            .filter(|&i| self.renders_cascade(i))
            .map(|i| self.draw_lists[i].len() + self.parent_draw_lists[i].len())
            .sum()
    }

    pub fn render<'a>(
        &'a self,
        encoder: &mut wgpu::CommandEncoder,
        cache: &'a ChunkCache,
        horizon: Option<&'a GpuMesh>,
        timestamp_query_set: Option<&'a wgpu::QuerySet>,
        timestamp_queries: &[Option<(u32, u32)>; CASCADE_COUNT],
        draw_offsets: &wgpu::Buffer,
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
            pass.set_vertex_buffer(1, draw_offsets.slice(..));
            let mut bound_page = None;
            if self.multidraw {
                self.indirect[i].draw(&mut pass, cache);
            }
            let direct = self.draw_lists[i].iter().zip(&self.draw_slots[i]).filter(|_| !self.multidraw);
            for (coord, &slot) in direct {
                if let Some(slice) = cache.draw_slice(*coord) {
                    if bound_page != Some(slice.page) {
                        pass.set_vertex_buffer(0, slice.vertices.slice(..));
                        pass.set_index_buffer(slice.indices.slice(..), wgpu::IndexFormat::Uint32);
                        bound_page = Some(slice.page);
                    }
                    pass.draw_indexed(slice.index_range, slice.base_vertex, slot..slot + 1);
                }
            }
            if let Some(horizon) = horizon {
                for (page_index, indices) in &self.parent_draw_lists[i] {
                    let page = &horizon.pages[*page_index];
                    pass.set_vertex_buffer(0, page.vertices.slice(..));
                    pass.set_index_buffer(page.indices.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(indices.clone(), 0, ABSOLUTE_SLOT..ABSOLUTE_SLOT + 1);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::{DVec2, DVec3, IVec3};

    /// Texel coordinates of `world_point` (voxels) in cascade 0.
    fn cascade_texel_of(origin: RenderOrigin, eye: DVec3, world_point: DVec3, scale: f64) -> DVec2 {
        let projection = Mat4::perspective_rh(1.2, 16.0 / 9.0, 0.05, 4096.0);
        let forward = Vec3::new(1.0, -0.3, 0.2).normalize();
        let view = CascadeView {
            inv_view_proj: (projection * Mat4::look_to_rh(Vec3::ZERO, forward, Vec3::Y)).inverse(),
            eye_m: eye * scale,
            origin,
        };
        let origin_m = origin.voxel.as_dvec3() * scale;
        let splits = cascade_splits(scale as f32);
        let depth = projection_depth(view.inv_view_proj);
        let near = 0.05 * scale as f32;
        let slice = frustum_slice(&view, (splits[0] - near) / depth, (splits[1] - near) / depth, origin_m);
        let dir = DVec3::new(0.4, 0.6, 0.3).normalize();
        let fit = fit_cascade(slice.center_m, f64::from(slice.radius), dir, DVec3::Y, origin_m, scale);
        let ndc = fit.light_view_proj.project_point3(world_point * scale - origin_m);
        (ndc.truncate() * 0.5 + 0.5) * f64::from(SHADOW_MAP_SIZE)
    }

    #[test]
    fn cascade_texel_grid_is_world_locked_across_origin_moves() {
        // Same absolute camera and sun; origins one grid cell apart.
        let scale = 1.0;
        let eye = DVec3::new(900_040.0, 120.0, -77.0);
        let a = RenderOrigin { voxel: IVec3::new(897_024, 0, -4096) };
        let b = RenderOrigin { voxel: IVec3::new(901_120, 0, -4096) };
        let world_point = DVec3::new(900_050.5, 100.0, -70.25);
        let ta = cascade_texel_of(a, eye, world_point, scale);
        let tb = cascade_texel_of(b, eye, world_point, scale);
        assert!((ta - tb).abs().max_element() < 1e-3, "{ta} vs {tb}");
    }
}
