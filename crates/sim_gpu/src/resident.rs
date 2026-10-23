//! The GPU-resident cellular world (docs/PLAN2.md A5): the brick pool, its directory and the awake
//! bitmaps stay on the GPU, a tick is one submission of nine dispatches (two of them indirect over a
//! GPU-built list of active bricks), and nothing is read back unless asked. It reproduces
//! `engine_sim::step` byte for byte as long as the pool has room and matter stays inside the window;
//! either limit raises `Snapshot::overflow` instead of diverging silently.
use crate::{GpuStepError, pack_cell, pack_rules, program};
use engine_core::reaction::SimRules;
use engine_sim::{BRICK_CELLS, BRICKS_PER_CHUNK, BrickStore, Cell, NO_SLOT};
use glam::{IVec3, UVec3};

pub const RESIDENT_WGSL: &str = concat!(include_str!("resident.wgsl"), "
", include_str!("alloc.wgsl"));

/// The chunks the GPU world covers and how many bricks its pool holds.
#[derive(Clone, Copy, Debug)]
pub struct Window {
    /// Lowest chunk of the window.
    pub origin: IVec3,
    /// Size in chunks.
    pub size: UVec3,
    /// Pool capacity in bricks.
    pub capacity: u32,
}

#[derive(Debug, PartialEq, Eq)]
pub enum UploadError {
    OutsideWindow(IVec3),
    PoolTooSmall { needed: usize, capacity: u32 },
    /// A buffer would exceed the device's storage-binding limit.
    TooLargeForDevice { bytes: u64, limit: u32 },
    /// The window or the pool is empty, or the window's brick count overflows `u32`.
    BadWindow,
}

const PARAM_STRIDE: u64 = 256;
const SLOT_START: u64 = 0;
const SLOT_PHASE0: u64 = 1;
const SLOT_MID: u64 = 2;
const SLOT_PHASE1: u64 = 3;
const SLOT_END: u64 = 4;
const SLOT_WAKE_ALL: u64 = 5;
/// Bitmaps in the `bits` buffer: dirty, A, S, W0, W1.
const BITMAPS: u32 = 5;
/// Words before the free stack in the `alloc` buffer.
pub(crate) const ALLOC_HEADER: u32 = 4;

pub struct GpuWorld {
    device: wgpu::Device,
    queue: wgpu::Queue,
    window: Window,
    bricks: u32,
    words: u32,
    bits_pipe: wgpu::ComputePipeline,
    compact_pipe: wgpu::ComputePipeline,
    finalize_pipe: wgpu::ComputePipeline,
    blocks_pipe: wgpu::ComputePipeline,
    bind_group: wgpu::BindGroup,
    params: wgpu::Buffer,
    pool: wgpu::Buffer,
    directory: wgpu::Buffer,
    bits: wgpu::Buffer,
    list: wgpu::Buffer,
    alloc: wgpu::Buffer,
}

fn storage(binding: u32, read_only: bool) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only }, has_dynamic_offset: false, min_binding_size: None },
        count: None,
    }
}

impl GpuWorld {
    /// Upload `store` into a new GPU world. Every chunk of the store must lie inside the window.
    pub fn new(device: wgpu::Device, queue: wgpu::Queue, rules: &SimRules, window: Window, store: &BrickStore) -> Result<Self, UploadError> {
        let bricks = window
            .size
            .x
            .checked_mul(window.size.y)
            .and_then(|c| c.checked_mul(window.size.z))
            .and_then(|c| c.checked_mul(BRICKS_PER_CHUNK as u32))
            .filter(|&b| b > 0 && window.capacity > 0)
            .ok_or(UploadError::BadWindow)?;
        let words = bricks.div_ceil(32);
        let chunk_index = |chunk: IVec3| {
            let c = chunk - window.origin;
            (c.cmpge(IVec3::ZERO).all() && c.as_uvec3().cmplt(window.size).all())
                .then(|| c.x as u32 + window.size.x * (c.z as u32 + window.size.z * c.y as u32))
        };
        let limit = device.limits().max_storage_buffer_binding_size;
        let alloc_bytes = (ALLOC_HEADER as u64 + 2 * window.capacity as u64) * 4;
        for bytes in [window.capacity as u64 * BRICK_CELLS as u64 * 8, (4 + bricks as u64) * 4, alloc_bytes] {
            if bytes > limit as u64 {
                return Err(UploadError::TooLargeForDevice { bytes, limit });
            }
        }
        let cell_words = BRICK_CELLS * 2;
        let empty = pack_cell(Cell::EMPTY);
        let mut pool: Vec<u32> = empty.iter().copied().cycle().take(window.capacity as usize * cell_words).collect();
        let mut directory = vec![NO_SLOT; bricks as usize];
        let cpu_pool: &[Cell] = bytemuck::cast_slice(store.pool_bytes());
        let mut used = 0u32;
        for chunk in store.chunks() {
            let Some(slots) = store.chunk_slots(chunk).filter(|s| s.iter().any(|&s| s != NO_SLOT)) else { continue };
            let ci = chunk_index(chunk).ok_or(UploadError::OutsideWindow(chunk))?;
            for (b, &slot) in slots.iter().enumerate().filter(|(_, s)| **s != NO_SLOT) {
                if used >= window.capacity {
                    return Err(UploadError::PoolTooSmall { needed: store.bricks_in_use(), capacity: window.capacity });
                }
                let src = &cpu_pool[slot as usize * BRICK_CELLS..][..BRICK_CELLS];
                for (i, &c) in src.iter().enumerate() {
                    pool[used as usize * cell_words + 2 * i..][..2].copy_from_slice(&pack_cell(c));
                }
                directory[(ci * 64) as usize + b] = used;
                used += 1;
            }
        }
        let mut bits = vec![0u32; (BITMAPS * words) as usize];
        for b in store.dirty_bricks() {
            if let Some(ci) = chunk_index(b.chunk) {
                let wb = ci * 64 + b.brick as u32;
                bits[(wb / 32) as usize] |= 1 << (wb % 32);
            }
        }
        let cap = window.capacity;
        let mut alloc = vec![cap - used, 0, 0, 0];
        // The free stack (popped from the top), then room for slots handed back after a lost race.
        alloc.extend((used..cap).rev());
        alloc.resize((ALLOC_HEADER + 2 * cap) as usize, 0);

        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ca resident"),
            source: wgpu::ShaderSource::Wgsl(program(RESIDENT_WGSL).into()),
        });
        let uniform = wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: true, min_binding_size: None },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ca resident"),
            entries: &[uniform, storage(1, true), storage(2, false), storage(3, false), storage(4, false), storage(5, false), storage(6, false)],
        });
        let pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("ca resident"), bind_group_layouts: &[&layout], push_constant_ranges: &[] });
        let pipe = |entry: &str| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(entry),
                layout: Some(&pipeline_layout),
                module: &module,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        let (bits_pipe, compact_pipe, finalize_pipe, blocks_pipe) = (pipe("bits_pass"), pipe("compact"), pipe("finalize"), pipe("blocks_pass"));
        use wgpu::BufferUsages as U;
        let make = |label: &str, words: &[u32], usage: U| crate::buffer(&device, label, bytemuck::cast_slice(words), usage | U::COPY_SRC | U::COPY_DST);
        let rules_buf = make("ca rules", &pack_rules(rules), U::STORAGE);
        let pool = make("ca pool", &pool, U::STORAGE);
        let directory = make("ca directory", &directory, U::STORAGE);
        let bits = make("ca bits", &bits, U::STORAGE);
        let list = make("ca list", &vec![0u32; 4 + bricks as usize], U::STORAGE | U::INDIRECT);
        let alloc = make("ca alloc", &alloc, U::STORAGE);
        let params = make("ca params", &vec![0u32; (PARAM_STRIDE * 6 / 4) as usize], U::UNIFORM);
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ca resident"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding { buffer: &params, offset: 0, size: wgpu::BufferSize::new(64) }),
                },
                wgpu::BindGroupEntry { binding: 1, resource: rules_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: pool.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: directory.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 4, resource: bits.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 5, resource: list.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 6, resource: alloc.as_entire_binding() },
            ],
        });
        Ok(Self { device, queue, window, bricks, words, bits_pipe, compact_pipe, finalize_pipe, blocks_pipe, bind_group, params, pool, directory, bits, list, alloc })
    }

    fn write_params(&self, slot: u64, seed: u32, phase_counter: u32, phase_or_mode: u32, heat_tick: bool) {
        let (o, s) = (self.window.origin, self.window.size);
        let words: [u32; 16] = [
            seed, phase_counter, phase_or_mode, heat_tick as u32,
            o.x as u32, o.y as u32, o.z as u32, 0,
            s.x, s.y, s.z, self.bricks,
            self.words, self.window.capacity, 0, 0,
        ];
        self.queue.write_buffer(&self.params, slot * PARAM_STRIDE, bytemuck::cast_slice(&words));
    }

    fn bits(&self, pass: &mut wgpu::ComputePass, slot: u64) {
        pass.set_pipeline(&self.bits_pipe);
        pass.set_bind_group(0, &self.bind_group, &[(slot * PARAM_STRIDE) as u32]);
        pass.dispatch_workgroups(self.words.div_ceil(64), 1, 1);
    }

    fn phase(&self, pass: &mut wgpu::ComputePass, slot: u64) {
        pass.set_bind_group(0, &self.bind_group, &[(slot * PARAM_STRIDE) as u32]);
        pass.set_pipeline(&self.compact_pipe);
        pass.dispatch_workgroups(self.bricks.div_ceil(64), 1, 1);
        pass.set_pipeline(&self.finalize_pipe);
        pass.dispatch_workgroups(1, 1, 1);
        pass.set_pipeline(&self.blocks_pipe);
        pass.dispatch_workgroups_indirect(&self.list, 0);
    }

    /// One tick, submitted and not waited for.
    pub fn step(&self, seed: u32, tick: u32) {
        let heat = engine_sim::is_heat_tick(tick);
        let pc = tick.wrapping_mul(2);
        self.write_params(SLOT_START, seed, pc, 0, heat);
        self.write_params(SLOT_PHASE0, seed, pc, 0, heat);
        self.write_params(SLOT_MID, seed, pc, 1, heat);
        self.write_params(SLOT_PHASE1, seed, pc.wrapping_add(1), 1, heat);
        self.write_params(SLOT_END, seed, pc, 2, heat);
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            self.bits(&mut pass, SLOT_START);
            self.phase(&mut pass, SLOT_PHASE0);
            self.bits(&mut pass, SLOT_MID);
            self.phase(&mut pass, SLOT_PHASE1);
            self.bits(&mut pass, SLOT_END);
        }
        self.queue.submit([encoder.finish()]);
    }

    /// Mark every present brick awake (benchmarks and tests).
    pub fn wake_all(&self) {
        self.write_params(SLOT_WAKE_ALL, 0, 0, 3, true);
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            self.bits(&mut pass, SLOT_WAKE_ALL);
        }
        self.queue.submit([encoder.finish()]);
    }

    /// Wait for the GPU to finish everything submitted.
    pub fn finish(&self) -> Result<(), GpuStepError> {
        self.device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None }).map(|_| ()).map_err(GpuStepError::Poll)
    }
}

#[path = "resident_readback.rs"]
mod readback;
pub use readback::Snapshot;
