use glam::{IVec3, Vec3};
use std::collections::{HashMap, HashSet};
use std::sync::mpsc;
use wgpu::util::DeviceExt;

use crate::render::{ChunkCache, Gpu};
use crate::world::{
    BlockId, CHUNK_SIZE, World, chunk_coord_from_world_block, mark_chunk_and_neighbors_dirty,
};

const FLUID_SHADER: &str = include_str!("fluid_compute.wgsl");
const HALO: i32 = 1;
// Cubic chunk fluid simulation: each fluid chunk is 34x34x34 (CHUNK_SIZE + 2*HALO).
// Future design: per-cubic-chunk GPU buffers with boundary sync from 6 neighbors,
// top-to-bottom gravity cascading for efficient vertical flow across chunk boundaries.
const SIM_W: i32 = CHUNK_SIZE + HALO * 2;
const SIM_H: i32 = CHUNK_SIZE + HALO * 2;
const VOXEL_COUNT: usize = (SIM_W as usize) * (SIM_W as usize) * (SIM_H as usize);

const fn div_ceil_usize(a: usize, b: usize) -> usize {
    (a + (b - 1)) / b
}

const PACKED_WORDS: usize = div_ceil_usize(VOXEL_COUNT, 4);
const SOLID_WORDS: usize = div_ceil_usize(VOXEL_COUNT, 32);

const WORKGROUP_SIZE: u32 = 64;
const FILL_THRESHOLD: u8 = 165;
const DRAIN_THRESHOLD: u8 = 96;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VoxelScalePreset {
    OneMeter,
    HalfMeter,
    QuarterMeter,
}

impl VoxelScalePreset {
    pub const ALL: [Self; 3] = [Self::OneMeter, Self::HalfMeter, Self::QuarterMeter];

    pub fn meters(self) -> f32 {
        match self {
            Self::OneMeter => 1.0,
            Self::HalfMeter => 0.5,
            Self::QuarterMeter => 0.25,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::OneMeter => "1.00 m",
            Self::HalfMeter => "0.50 m",
            Self::QuarterMeter => "0.25 m",
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct FluidStats {
    pub active_chunks: usize,
    pub inflight_chunks: usize,
    pub pending_updates: usize,
    pub simulated_chunks_last_step: usize,
    pub applied_updates_last_step: usize,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct FluidParams {
    voxel_count: u32,
    packed_count: u32,
    chunk_w: u32,
    chunk_h: u32,
    dt: f32,
    lateral: f32,
    downward: f32,
    upward: f32,
    ripple_amp: f32,
    ripple_freq: f32,
    ripple_speed: f32,
    time: f32,
}

#[derive(Clone, Copy)]
struct FluidTuning {
    fixed_dt: f32,
    max_substeps_per_frame: usize,
    max_dispatch_per_step: usize,
    max_apply_per_step: usize,
    simulate_radius_chunks: i32,
    keep_radius_chunks: i32,
    max_active_chunks: usize,
    max_seed_probes_per_step: usize,
    lateral_rate: f32,
    downward_rate: f32,
    upward_rate: f32,
    ripple_amp: f32,
    ripple_freq: f32,
    ripple_speed: f32,
}

impl Default for FluidTuning {
    fn default() -> Self {
        Self {
            fixed_dt: 1.0 / 30.0,
            max_substeps_per_frame: 3,
            max_dispatch_per_step: 3,
            max_apply_per_step: 2400,
            simulate_radius_chunks: 8,
            keep_radius_chunks: 12,
            max_active_chunks: 180,
            max_seed_probes_per_step: 14,
            lateral_rate: 1.65,
            downward_rate: 3.75,
            upward_rate: 0.55,
            ripple_amp: 13.0,
            ripple_freq: 0.42,
            ripple_speed: 2.2,
        }
    }
}

struct FluidChunkGpu {
    level_buffers: [wgpu::Buffer; 2],
    solid_buffer: wgpu::Buffer,
    params_buffer: wgpu::Buffer,
    bind_groups: [wgpu::BindGroup; 2],
    readback_buffer: wgpu::Buffer,
    current_buffer: usize,
}

struct FluidChunk {
    levels: Vec<u8>,
    solid_mask: Vec<u32>,
    staging_words: Vec<u32>,
    gpu: FluidChunkGpu,
    nonzero_interior: u32,
    wet_boundary: bool,
    last_delta: u32,
}

struct InflightJob {
    job_id: u64,
}

struct FluidCallback {
    coord: IVec3,
    job_id: u64,
    ok: bool,
}

pub struct FluidSimulation {
    enabled: bool,
    voxel_scale: VoxelScalePreset,
    accumulator: f32,
    sim_time: f32,
    next_job_id: u64,
    tuning: FluidTuning,
    stats: FluidStats,
    pending_updates: HashMap<IVec3, BlockId>,
    chunks: HashMap<IVec3, FluidChunk>,
    inflight: HashMap<IVec3, InflightJob>,
    known_dry: HashSet<IVec3>,
    bind_group_layout: wgpu::BindGroupLayout,
    pipeline: wgpu::ComputePipeline,
    done_tx: mpsc::Sender<FluidCallback>,
    done_rx: mpsc::Receiver<FluidCallback>,
}

impl FluidSimulation {
    pub fn new(device: &wgpu::Device) -> Self {
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("fluid compute layout"),
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
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("fluid compute shader"),
            source: wgpu::ShaderSource::Wgsl(FLUID_SHADER.into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("fluid compute pipeline layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("fluid compute pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("cs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        let (done_tx, done_rx) = mpsc::channel();

        Self {
            enabled: false,
            voxel_scale: VoxelScalePreset::OneMeter,
            accumulator: 0.0,
            sim_time: 0.0,
            next_job_id: 0,
            tuning: FluidTuning::default(),
            stats: FluidStats::default(),
            pending_updates: HashMap::new(),
            chunks: HashMap::new(),
            inflight: HashMap::new(),
            known_dry: HashSet::new(),
            bind_group_layout,
            pipeline,
            done_tx,
            done_rx,
        }
    }

    pub fn reset_for_world(&mut self, voxel_scale: VoxelScalePreset) {
        self.enabled = false;
        self.voxel_scale = voxel_scale;
        self.accumulator = 0.0;
        self.sim_time = 0.0;
        self.next_job_id = 0;
        self.pending_updates.clear();
        self.chunks.clear();
        self.inflight.clear();
        self.known_dry.clear();
        self.stats = FluidStats::default();
        while self.done_rx.try_recv().is_ok() {}
    }

    pub fn set_enabled(&mut self, _enabled: bool) {
        // Fluid simulation is hard-disabled pending cubic chunk update.
        self.enabled = false;
        self.accumulator = 0.0;
        self.pending_updates.clear();
        self.inflight.clear();
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn voxel_scale(&self) -> VoxelScalePreset {
        self.voxel_scale
    }

    pub fn stats(&self) -> FluidStats {
        self.stats
    }

    pub fn update(
        &mut self,
        gpu: &Gpu,
        world: &mut World,
        cache: &ChunkCache,
        player_pos: Vec3,
        dt: f32,
        dirty_blocks: &mut HashSet<IVec3>,
        dirty_chunks: &mut HashSet<IVec3>,
    ) {
        self.stats.simulated_chunks_last_step = 0;
        self.stats.applied_updates_last_step = 0;
        self.collect_finished(gpu, world);

        if !self.enabled || dt <= 0.0 {
            self.stats.active_chunks = self.chunks.len();
            self.stats.inflight_chunks = self.inflight.len();
            self.stats.pending_updates = self.pending_updates.len();
            return;
        }

        self.accumulator = (self.accumulator + dt).min(self.tuning.fixed_dt * 4.0);
        let mut substeps = 0usize;
        while self.accumulator >= self.tuning.fixed_dt
            && substeps < self.tuning.max_substeps_per_frame
        {
            self.step(gpu, world, cache, player_pos);
            self.accumulator -= self.tuning.fixed_dt;
            self.sim_time += self.tuning.fixed_dt;
            substeps += 1;
        }
        if substeps == self.tuning.max_substeps_per_frame {
            self.accumulator = 0.0;
        }

        self.apply_pending(world, dirty_blocks, dirty_chunks);
        self.stats.active_chunks = self.chunks.len();
        self.stats.inflight_chunks = self.inflight.len();
        self.stats.pending_updates = self.pending_updates.len();
    }

    fn step(&mut self, gpu: &Gpu, world: &World, cache: &ChunkCache, player_pos: Vec3) {
        let player_chunk = chunk_coord_from_world_block(player_pos.floor().as_ivec3());
        self.prune_far_chunks(player_chunk);
        self.seed_chunks_near_player(gpu, world, cache, player_chunk);

        let mut candidates = self
            .chunks
            .iter()
            .filter_map(|(&coord, chunk)| {
                if self.inflight.contains_key(&coord) {
                    return None;
                }
                let dx = (coord.x - player_chunk.x).abs();
                let dz = (coord.z - player_chunk.z).abs();
                if dx > self.tuning.simulate_radius_chunks
                    || dz > self.tuning.simulate_radius_chunks
                {
                    return None;
                }
                if chunk.nonzero_interior == 0 && !chunk.wet_boundary {
                    return None;
                }
                Some((dx.max(dz), coord))
            })
            .collect::<Vec<_>>();
        candidates.sort_by_key(|(d, coord)| (*d, coord.x, coord.z));

        let mut dispatched = 0usize;
        for (_, coord) in candidates {
            if dispatched >= self.tuning.max_dispatch_per_step {
                break;
            }
            if self.dispatch_chunk(gpu, world, cache, coord) {
                dispatched += 1;
                self.stats.simulated_chunks_last_step += 1;
            }
        }
    }

    fn prune_far_chunks(&mut self, player_chunk: IVec3) {
        let keep_radius = self.tuning.keep_radius_chunks;
        let inflight_coords = self.inflight.keys().copied().collect::<HashSet<_>>();
        self.chunks.retain(|coord, _| {
            if inflight_coords.contains(coord) {
                return true;
            }
            let dx = (coord.x - player_chunk.x).abs();
            let dz = (coord.z - player_chunk.z).abs();
            dx <= keep_radius && dz <= keep_radius
        });
    }

    fn seed_chunks_near_player(
        &mut self,
        gpu: &Gpu,
        world: &World,
        cache: &ChunkCache,
        player_chunk: IVec3,
    ) {
        if self.chunks.len() >= self.tuning.max_active_chunks {
            return;
        }

        let mut probes = 0usize;
        let radius = self.tuning.simulate_radius_chunks;
        'outer: for ring in 0..=radius {
            for dz in -ring..=ring {
                for dx in -ring..=ring {
                    if ring > 0 && dx.abs() != ring && dz.abs() != ring {
                        continue;
                    }
                    let coord = IVec3::new(player_chunk.x + dx, 0, player_chunk.z + dz);
                    if self.chunks.contains_key(&coord)
                        || self.inflight.contains_key(&coord)
                        || !cache.visible_coords.contains(&coord)
                    {
                        continue;
                    }
                    if self.known_dry.contains(&coord) {
                        continue;
                    }
                    probes += 1;
                    if probes > self.tuning.max_seed_probes_per_step {
                        break 'outer;
                    }

                    if !chunk_has_water_seed(world, cache, coord) {
                        self.known_dry.insert(coord);
                        continue;
                    }

                    let chunk =
                        FluidChunk::new(&gpu.device, &self.bind_group_layout, world, cache, coord);
                    self.chunks.insert(coord, chunk);
                    self.known_dry.remove(&coord);
                    if self.chunks.len() >= self.tuning.max_active_chunks {
                        break 'outer;
                    }
                }
            }
        }
    }

    fn dispatch_chunk(
        &mut self,
        gpu: &Gpu,
        world: &World,
        cache: &ChunkCache,
        coord: IVec3,
    ) -> bool {
        let Some(chunk) = self.chunks.get_mut(&coord) else {
            return false;
        };

        chunk.refresh_from_world(world, cache, coord);
        if chunk.nonzero_interior == 0 && !chunk.wet_boundary {
            return false;
        }
        chunk.pack_levels();

        let current = chunk.gpu.current_buffer;
        let next = 1 - current;

        gpu.queue.write_buffer(
            &chunk.gpu.level_buffers[current],
            0,
            bytemuck::cast_slice(&chunk.staging_words),
        );
        gpu.queue.write_buffer(
            &chunk.gpu.solid_buffer,
            0,
            bytemuck::cast_slice(&chunk.solid_mask),
        );

        let scale = (1.0 / self.voxel_scale.meters()).clamp(1.0, 4.0);
        let params = FluidParams {
            voxel_count: VOXEL_COUNT as u32,
            packed_count: PACKED_WORDS as u32,
            chunk_w: SIM_W as u32,
            chunk_h: SIM_H as u32,
            dt: self.tuning.fixed_dt,
            lateral: self.tuning.lateral_rate * scale,
            downward: self.tuning.downward_rate * scale,
            upward: self.tuning.upward_rate,
            ripple_amp: self.tuning.ripple_amp * scale.sqrt(),
            ripple_freq: self.tuning.ripple_freq,
            ripple_speed: self.tuning.ripple_speed,
            time: self.sim_time,
        };
        gpu.queue
            .write_buffer(&chunk.gpu.params_buffer, 0, bytemuck::bytes_of(&params));

        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("fluid compute encoder"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("fluid compute pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &chunk.gpu.bind_groups[current], &[]);
            let groups = (PACKED_WORDS as u32).div_ceil(WORKGROUP_SIZE);
            pass.dispatch_workgroups(groups.max(1), 1, 1);
        }
        encoder.copy_buffer_to_buffer(
            &chunk.gpu.level_buffers[next],
            0,
            &chunk.gpu.readback_buffer,
            0,
            (PACKED_WORDS * std::mem::size_of::<u32>()) as u64,
        );
        gpu.queue.submit(Some(encoder.finish()));

        let job_id = self.next_job_id;
        self.next_job_id = self.next_job_id.wrapping_add(1);
        let tx = self.done_tx.clone();
        chunk
            .gpu
            .readback_buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = tx.send(FluidCallback {
                    coord,
                    job_id,
                    ok: result.is_ok(),
                });
            });

        chunk.gpu.current_buffer = next;
        self.inflight.insert(coord, InflightJob { job_id });
        true
    }

    fn collect_finished(&mut self, gpu: &Gpu, world: &World) {
        let _ = gpu.device.poll(wgpu::PollType::Poll);
        while let Ok(done) = self.done_rx.try_recv() {
            let Some(job) = self.inflight.get(&done.coord) else {
                continue;
            };
            if job.job_id != done.job_id {
                continue;
            }
            self.inflight.remove(&done.coord);

            let Some(chunk) = self.chunks.get_mut(&done.coord) else {
                continue;
            };
            if !done.ok {
                chunk.gpu.readback_buffer.unmap();
                continue;
            }

            let mapped = chunk.gpu.readback_buffer.slice(..).get_mapped_range();
            let words = bytemuck::cast_slice::<u8, u32>(&mapped);
            let mut delta_accum = 0u32;
            let mut nonzero_interior = 0u32;
            let mut wet_boundary = false;

            for idx in 0..VOXEL_COUNT {
                let word = words[idx >> 2];
                let shift = ((idx & 3) * 8) as u32;
                let next_level = ((word >> shift) & 0xff) as u8;
                let old_level = chunk.levels[idx];
                delta_accum += old_level.abs_diff(next_level) as u32;
                chunk.levels[idx] = next_level;

                if chunk.is_solid(idx) {
                    continue;
                }
                let (lx, ly, lz) = local_coords_from_index(idx);
                let interior = is_interior(lx, ly, lz);
                if interior && next_level > 0 {
                    nonzero_interior += 1;
                }
                if !interior && next_level > 0 {
                    wet_boundary = true;
                }
                if !interior {
                    continue;
                }

                let was_wet = old_level >= FILL_THRESHOLD;
                let is_wet = if was_wet {
                    next_level >= DRAIN_THRESHOLD
                } else {
                    next_level >= FILL_THRESHOLD
                };
                if was_wet == is_wet {
                    continue;
                }

                let world_pos = world_pos_from_components(done.coord, lx, ly, lz);
                let current = world.get(world_pos);
                if !matches!(current, BlockId::Air | BlockId::Water) {
                    continue;
                }
                let target = if is_wet { BlockId::Water } else { BlockId::Air };
                self.pending_updates.insert(world_pos, target);
            }
            drop(mapped);
            chunk.gpu.readback_buffer.unmap();
            chunk.nonzero_interior = nonzero_interior;
            chunk.wet_boundary = wet_boundary;
            chunk.last_delta = delta_accum;
        }
    }

    fn apply_pending(
        &mut self,
        world: &mut World,
        dirty_blocks: &mut HashSet<IVec3>,
        dirty_chunks: &mut HashSet<IVec3>,
    ) {
        if self.pending_updates.is_empty() {
            return;
        }

        let take = self
            .tuning
            .max_apply_per_step
            .min(self.pending_updates.len());
        let keys = self
            .pending_updates
            .keys()
            .copied()
            .take(take)
            .collect::<Vec<_>>();
        let mut edits = Vec::with_capacity(keys.len());
        for key in keys {
            let Some(target) = self.pending_updates.remove(&key) else {
                continue;
            };
            let current = world.get(key);
            if current == target {
                continue;
            }
            if !matches!(current, BlockId::Air | BlockId::Water) {
                continue;
            }
            edits.push((key, target));
        }
        if edits.is_empty() {
            return;
        }

        world.set_many(&edits);
        for (pos, _) in &edits {
            dirty_blocks.insert(*pos);
            mark_chunk_and_neighbors_dirty(dirty_chunks, *pos);
        }
        self.stats.applied_updates_last_step += edits.len();
    }
}

impl FluidChunk {
    fn new(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        world: &World,
        cache: &ChunkCache,
        coord: IVec3,
    ) -> Self {
        let mut levels = vec![0u8; VOXEL_COUNT];
        let mut solid_mask = vec![0u32; SOLID_WORDS];

        let mut nonzero_interior = 0u32;
        let mut wet_boundary = false;

        for idx in 0..VOXEL_COUNT {
            let (lx, ly, lz) = local_coords_from_index(idx);
            let world_pos = world_pos_from_components(coord, lx, ly, lz);
            let block = sample_block_cached(world, cache, world_pos);
            if !matches!(block, BlockId::Air | BlockId::Water) {
                set_solid_bit(&mut solid_mask, idx, true);
                levels[idx] = 0;
                continue;
            }
            levels[idx] = if block == BlockId::Water { 255 } else { 0 };
            let interior = is_interior(lx, ly, lz);
            if interior && levels[idx] > 0 {
                nonzero_interior += 1;
            }
            if !interior && levels[idx] > 0 {
                wet_boundary = true;
            }
        }

        let words_size = (PACKED_WORDS * std::mem::size_of::<u32>()) as u64;
        let mask_size = (SOLID_WORDS * std::mem::size_of::<u32>()) as u64;

        let level_a = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("fluid levels a"),
            size: words_size,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let level_b = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("fluid levels b"),
            size: words_size,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let solid_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("fluid solid mask"),
            size: mask_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("fluid params"),
            contents: bytemuck::bytes_of(&FluidParams {
                voxel_count: VOXEL_COUNT as u32,
                packed_count: PACKED_WORDS as u32,
                chunk_w: SIM_W as u32,
                chunk_h: SIM_H as u32,
                dt: 0.0,
                lateral: 0.0,
                downward: 0.0,
                upward: 0.0,
                ripple_amp: 0.0,
                ripple_freq: 0.0,
                ripple_speed: 0.0,
                time: 0.0,
            }),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let readback_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("fluid readback"),
            size: words_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let bind_group_ab = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("fluid bind group ab"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: level_a.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: level_b.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: solid_buffer.as_entire_binding(),
                },
            ],
        });
        let bind_group_ba = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("fluid bind group ba"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: level_b.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: level_a.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: solid_buffer.as_entire_binding(),
                },
            ],
        });

        Self {
            levels,
            solid_mask,
            staging_words: vec![0u32; PACKED_WORDS],
            gpu: FluidChunkGpu {
                level_buffers: [level_a, level_b],
                solid_buffer,
                params_buffer,
                bind_groups: [bind_group_ab, bind_group_ba],
                readback_buffer,
                current_buffer: 0,
            },
            nonzero_interior,
            wet_boundary,
            last_delta: 0,
        }
    }

    fn refresh_from_world(&mut self, world: &World, cache: &ChunkCache, coord: IVec3) {
        self.solid_mask.fill(0);
        let mut nonzero_interior = 0u32;
        let mut wet_boundary = false;

        for idx in 0..VOXEL_COUNT {
            let (lx, ly, lz) = local_coords_from_index(idx);
            let world_pos = world_pos_from_components(coord, lx, ly, lz);
            let block = sample_block_cached(world, cache, world_pos);
            let interior = is_interior(lx, ly, lz);
            let boundary = !interior;

            if !matches!(block, BlockId::Air | BlockId::Water) {
                set_solid_bit(&mut self.solid_mask, idx, true);
                self.levels[idx] = 0;
                continue;
            }

            if boundary {
                self.levels[idx] = if block == BlockId::Water { 255 } else { 0 };
            } else if block == BlockId::Water && self.levels[idx] == 0 {
                self.levels[idx] = 255;
            }

            if interior && self.levels[idx] > 0 {
                nonzero_interior += 1;
            }
            if boundary && self.levels[idx] > 0 {
                wet_boundary = true;
            }
        }

        self.nonzero_interior = nonzero_interior;
        self.wet_boundary = wet_boundary;
    }

    fn pack_levels(&mut self) {
        self.staging_words.fill(0);
        for (idx, level) in self.levels.iter().enumerate() {
            let shift = ((idx & 3) * 8) as u32;
            self.staging_words[idx >> 2] |= (*level as u32) << shift;
        }
    }

    fn is_solid(&self, idx: usize) -> bool {
        let word = self.solid_mask[idx >> 5];
        ((word >> (idx & 31)) & 1) != 0
    }
}

fn chunk_has_water_seed(world: &World, cache: &ChunkCache, coord: IVec3) -> bool {
    let ox = coord.x * CHUNK_SIZE;
    let oy = coord.y * CHUNK_SIZE;
    let oz = coord.z * CHUNK_SIZE;
    for y in 0..CHUNK_SIZE {
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let p = IVec3::new(ox + x, oy + y, oz + z);
                if sample_block_cached(world, cache, p) == BlockId::Water {
                    return true;
                }
            }
        }
    }
    false
}

fn sample_block_cached(world: &World, cache: &ChunkCache, p: IVec3) -> BlockId {
    let coord = chunk_coord_from_world_block(p);
    if let Some(chunk) = cache.cpu_chunks.get(&coord) {
        return chunk.get_world(p);
    }
    world.get(p)
}

fn set_solid_bit(mask: &mut [u32], index: usize, solid: bool) {
    let word = index >> 5;
    let bit = 1u32 << (index & 31);
    if solid {
        mask[word] |= bit;
    } else {
        mask[word] &= !bit;
    }
}

fn local_coords_from_index(index: usize) -> (i32, i32, i32) {
    let w = SIM_W as usize;
    let x = (index % w) as i32;
    let yz = index / w;
    let z = (yz % w) as i32;
    let y = (yz / w) as i32;
    (x, y, z)
}

fn world_pos_from_components(coord: IVec3, lx: i32, ly: i32, lz: i32) -> IVec3 {
    IVec3::new(
        coord.x * CHUNK_SIZE + lx - HALO,
        coord.y * CHUNK_SIZE + ly - HALO,
        coord.z * CHUNK_SIZE + lz - HALO,
    )
}

fn is_interior(lx: i32, ly: i32, lz: i32) -> bool {
    lx >= HALO
        && lx < CHUNK_SIZE + HALO
        && ly >= HALO
        && ly < CHUNK_SIZE + HALO
        && lz >= HALO
        && lz < CHUNK_SIZE + HALO
}
