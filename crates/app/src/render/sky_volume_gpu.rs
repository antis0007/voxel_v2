//! GPU side of `sky_volume`: occupancy upload, brick bake dispatch and the
//! probe textures the world shader samples (lighting group, bindings 3 to 6).
use super::sky_volume::{self, Cells, Coarse, Schedule, BITS_DIM, BRICK, COARSE_SCALE, PROBE_DIM, REACH};
use super::ChunkCache;
use crate::world::CHUNK_SIZE;
use glam::{IVec3, Vec3};
use std::collections::HashMap;
use wgpu::util::DeviceExt;

/// Probe bricks traced per frame and level: 32 bricks are 16k probes.
const FINE_BUDGET: usize = 32;
/// The coarse level moves a quarter as often, so it gets half the bake budget.
const COARSE_BUDGET: usize = 16;
/// Chunk downsamples for the coarse level per frame, nearest first.
const COARSE_BUILDS: usize = 16;
/// Every brick of the window can be cleared in one dispatch, plus a bake batch.
const MAX_ENTRIES: usize = (PROBE_DIM / BRICK).pow(3) as usize + FINE_BUDGET;
const PROBES_PER_BRICK: u32 = (BRICK * BRICK * BRICK) as u32;
const WORKGROUP: u32 = 64;

pub struct SkyVolumeGpu {
    pub(super) fine: Level,
    pub(super) coarse: Level,
    /// Downsampled resident chunks inside the coarse window, keyed like `stamped`.
    coarse_chunks: HashMap<IVec3, (u64, Coarse)>,
    /// Frames since start; an idle coarse window rechecks residency every 8th.
    frame: u32,
    pub(super) pipeline: wgpu::ComputePipeline,
    pending: [Option<wgpu::CommandBuffer>; 2],
}

/// One probe window: `scale` voxels per cell.
pub(super) struct Level {
    scale: i32,
    budget: usize,
    schedule: Option<Schedule>,
    occupancy: wgpu::Buffer,
    entries: wgpu::Buffer,
    params: wgpu::Buffer,
    window: wgpu::Buffer,
    /// Read back only by the GPU contract test; the view is what renders.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) texture: wgpu::Texture,
    probes: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
    active: bool,
    stats: Stats,
    /// Set when CPU work for this level (the coarse downsample) began before `update_from`.
    started: Option<std::time::Instant>,
}

/// `VOXEL_SKY_VOLUME_STATS=1` prints CPU time and bake throughput every 240 updates.
#[derive(Default)]
struct Stats {
    updates: u32,
    restamps: u32,
    bricks: usize,
    cpu_ms: Vec<f32>,
}

impl Stats {
    fn record(&mut self, scale: i32, cpu_ms: f32, bricks: usize, restamped: bool, pending: usize) {
        static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        if !*ON.get_or_init(|| std::env::var("VOXEL_SKY_VOLUME_STATS").is_ok_and(|v| v == "1")) {
            return;
        }
        self.updates += 1;
        self.restamps += u32::from(restamped);
        self.bricks += bricks;
        self.cpu_ms.push(cpu_ms);
        if self.updates == 240 {
            self.cpu_ms.sort_by(f32::total_cmp);
            let p = |q: f32| self.cpu_ms[((self.cpu_ms.len() - 1) as f32 * q) as usize];
            println!(
                "[sky-volume x{scale}] updates={} restamps={} bricks_baked={} pending={} cpu_ms p50={:.3} p95={:.3} p99={:.3} max={:.3}",
                self.updates, self.restamps, self.bricks, pending, p(0.5), p(0.95), p(0.99), p(1.0)
            );
            *self = Self::default();
        }
    }
}

impl SkyVolumeGpu {
    pub fn new(device: &wgpu::Device) -> Self {
        let storage = |binding, read_only| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sky volume bake layout"),
            entries: &[
                storage(0, true),
                storage(1, true),
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba16Uint,
                        view_dimension: wgpu::TextureViewDimension::D3,
                    },
                    count: None,
                },
            ],
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sky volume bake"),
            source: wgpu::ShaderSource::Wgsl(crate::shader_sources::SKY_VOLUME_BAKE_SHADER.into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("sky volume bake"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("sky volume bake"),
            layout: Some(&pipeline_layout),
            module: &module,
            entry_point: Some("bake"),
            compilation_options: Default::default(),
            cache: None,
        });
        Self {
            fine: Level::new(device, &layout, 1, FINE_BUDGET),
            coarse: Level::new(device, &layout, COARSE_SCALE, COARSE_BUDGET),
            coarse_chunks: HashMap::new(),
            frame: 0,
            pipeline,
            pending: [None, None],
        }
    }

    /// Lighting group entries 3 to 6: fine probes and window, coarse probes and window.
    pub fn bindings(&self) -> [wgpu::BindingResource<'_>; 4] {
        [
            wgpu::BindingResource::TextureView(&self.fine.probes),
            self.fine.window.as_entire_binding(),
            wgpu::BindingResource::TextureView(&self.coarse.probes),
            self.coarse.window.as_entire_binding(),
        ]
    }

    /// `eye` is the camera in voxels. Restamps changed chunks, clears entering
    /// bricks and traces a bounded batch per level, nearest first.
    pub fn update(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, cache: &ChunkCache, eye: Vec3) {
        // If a redraw was interrupted, submit before overwriting that bake's inputs.
        if self.pending.iter().any(Option::is_some) {
            queue.submit(self.take_pending().into_iter().flatten());
        }
        let voxel_m = cache.cpu_chunks.values().next().map_or(1.0, |c| c.voxel_meters);
        // The resident allocation joins the revision, so a chunk replaced at the
        // same revision (a world switch inside one frame) still reads as changed.
        let key = |coord: IVec3| {
            cache.cpu_chunks.get(&coord).map(|chunk| {
                let identity = std::sync::Arc::as_ptr(chunk) as usize as u64;
                (&***chunk, cache.published_revision(coord).unwrap_or(0).wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ identity)
            })
        };
        self.pending[0] = self.fine.encode_update(device, queue, &self.pipeline, eye, voxel_m, key);
        // Scanning the ~2200 chunks of the coarse window costs 0.15 ms; a regional
        // term can wait up to 8 frames for an edit while nothing moves or bakes.
        self.frame = self.frame.wrapping_add(1);
        let origin = sky_volume::probe_origin(eye / COARSE_SCALE as f32);
        let busy = !self.coarse.wanted() || self.coarse.schedule.as_ref().is_none_or(|s| s.probe_origin != origin || !s.pending.is_empty());
        if !busy && !self.frame.is_multiple_of(8) {
            return;
        }
        if !self.coarse.wanted() {
            self.coarse_chunks.clear();
        } else {
            self.coarse.started = Some(std::time::Instant::now());
            self.build_coarse(eye, key);
        }
        let chunks = &self.coarse_chunks;
        self.pending[1] = self.coarse.encode_update(device, queue, &self.pipeline, eye, voxel_m, |c| chunks.get(&c).map(|(k, cells)| (cells, *k)));
    }

    pub(super) fn take_pending(&mut self) -> [Option<wgpu::CommandBuffer>; 2] {
        std::mem::take(&mut self.pending)
    }

    /// Downsamples new or changed chunks of the coarse window, nearest first and
    /// at most `COARSE_BUILDS` per frame. A chunk not yet built reads as air and
    /// its arrival rebakes the bricks around it, as for streaming.
    fn build_coarse<'a>(&mut self, eye: Vec3, key: impl Fn(IVec3) -> Option<(&'a crate::mesh::GeneratedChunk, u64)>) {
        let chunk_cells = CHUNK_SIZE / COARSE_SCALE;
        let origin = sky_volume::probe_origin(eye / COARSE_SCALE as f32) - IVec3::splat(REACH);
        let lo = origin.map(|v| engine_core::floor_div(v, chunk_cells));
        let hi = (origin + IVec3::splat(BITS_DIM - 1)).map(|v| engine_core::floor_div(v, chunk_cells));
        self.coarse_chunks.retain(|c, _| c.cmpge(lo).all() && c.cmple(hi).all());
        let centre = (eye / CHUNK_SIZE as f32).floor().as_ivec3();
        let mut wanted = Vec::new();
        for coord in sky_volume::window_chunks(origin, chunk_cells) {
            match key(coord) {
                Some((chunk, k)) => {
                    if self.coarse_chunks.get(&coord).is_none_or(|e| e.0 != k) {
                        wanted.push((coord, chunk, k));
                    }
                }
                None => {
                    self.coarse_chunks.remove(&coord);
                }
            }
        }
        wanted.sort_by_key(|(c, ..)| ((*c - centre).length_squared(), c.to_array()));
        for (coord, chunk, k) in wanted.into_iter().take(COARSE_BUILDS) {
            self.coarse_chunks.insert(coord, (k, Coarse::build(chunk, COARSE_SCALE)));
        }
    }
}

impl Level {
    fn new(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, scale: i32, budget: usize) -> Self {
        let occupancy = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sky volume occupancy"),
            size: (sky_volume::Window::new(IVec3::ZERO, 1).words.len() * 4) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let entries = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sky volume bricks"),
            size: (MAX_ENTRIES * 16) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sky volume bake params"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        // Edge 0 until the first bake, so the shader keeps the column sky access.
        let window = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("sky volume window"),
            contents: bytemuck::cast_slice(&[0, 0, 0, 0, scale, 0, 0, 0]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("sky volume probes"),
            size: wgpu::Extent3d {
                width: PROBE_DIM as u32,
                height: PROBE_DIM as u32,
                depth_or_array_layers: PROBE_DIM as u32,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D3,
            format: wgpu::TextureFormat::Rgba16Uint,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let probes = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sky volume bake"),
            layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: occupancy.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: entries.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: params.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&probes) },
            ],
        });
        Self {
            scale,
            budget,
            schedule: None,
            occupancy,
            entries,
            params,
            window,
            texture,
            probes,
            bind_group,
            active: false,
            stats: Stats::default(),
            started: None,
        }
    }

    fn wanted(&self) -> bool {
        sky_volume::enabled() && sky_volume::env_allows() && (self.scale == 1 || sky_volume::coarse_allowed())
    }

    /// `eye` is in voxels; `source` returns a chunk's cells at this level and
    /// the revision that identifies them.
    #[cfg(test)]
    pub(super) fn update_from<'a, C: Cells + 'a>(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        pipeline: &wgpu::ComputePipeline,
        eye: Vec3,
        voxel_m: f32,
        source: impl Fn(IVec3) -> Option<(&'a C, u64)>,
    ) {
        if let Some(bake) = self.encode_update(device, queue, pipeline, eye, voxel_m, source) {
            queue.submit([bake]);
        }
    }

    fn encode_update<'a, C: Cells + 'a>(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        pipeline: &wgpu::ComputePipeline,
        eye: Vec3,
        voxel_m: f32,
        source: impl Fn(IVec3) -> Option<(&'a C, u64)>,
    ) -> Option<wgpu::CommandBuffer> {
        let start = self.started.take().unwrap_or_else(std::time::Instant::now);
        if !self.wanted() {
            if self.active {
                queue.write_buffer(&self.window, 0, bytemuck::cast_slice(&[0, 0, 0, 0, self.scale, 0, 0, 0]));
                self.active = false;
            }
            self.schedule = None;
            return None;
        }
        let eye = eye / self.scale as f32;
        let origin = sky_volume::probe_origin(eye);
        let schedule = self.schedule.get_or_insert_with(|| Schedule::new(origin, CHUNK_SIZE / self.scale));
        let window_changed = !self.active || schedule.probe_origin != origin;
        let restamped = schedule.refresh(origin, source);
        if restamped {
            queue.write_buffer(&self.occupancy, 0, bytemuck::cast_slice(&schedule.window.words));
        }
        let batch = schedule.take_batch(eye, self.budget);
        // Workgroups are unordered: a brick baked now must not also be cleared.
        let mut entries: Vec<[i32; 4]> = schedule.cleared.drain(..)
            .filter(|b| !batch.contains(b))
            .map(|b| [b.x, b.y, b.z, 1])
            .collect();
        entries.extend(batch.iter().map(|b| [b.x, b.y, b.z, 0]));
        let o = schedule.probe_origin;
        let window = [o.x, o.y, o.z, PROBE_DIM, self.scale, 0, 0, 0];
        let bake = if !entries.is_empty() {
            entries.truncate(MAX_ENTRIES);
            queue.write_buffer(&self.entries, 0, bytemuck::cast_slice(&entries));
            let leaf = sky_volume::leaf_transmittance(voxel_m * self.scale as f32);
            let mut params = [0u8; 32];
            params[..16].copy_from_slice(bytemuck::cast_slice(&[o.x, o.y, o.z, entries.len() as i32]));
            params[16..].copy_from_slice(bytemuck::cast_slice(&[leaf, REACH as f32, BITS_DIM as f32, 0.0]));
            queue.write_buffer(&self.params, 0, &params);
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("sky volume bake") });
            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("sky volume bake"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(pipeline);
                pass.set_bind_group(0, &self.bind_group, &[]);
                pass.dispatch_workgroups(entries.len() as u32 * PROBES_PER_BRICK / WORKGROUP, 1, 1);
            }
            Some(encoder.finish())
        } else {
            None
        };
        // The bake is ordered before world draws in the frame submission.
        if window_changed {
            queue.write_buffer(&self.window, 0, bytemuck::cast_slice(&window));
        }
        self.active = true;
        let pending = schedule.pending.len();
        self.stats.record(self.scale, start.elapsed().as_secs_f32() * 1000.0, batch.len(), restamped, pending);
        bake
    }
}

#[cfg(test)]
#[path = "sky_volume_gpu_tests.rs"]
mod tests;
