//! The cellular kernel's block rule on the GPU (docs/PLAN2.md A5).
//!
//! `block_rule.wgsl` ports `engine_sim`'s rule (movement, conduction, reactions) to u32 arithmetic so it
//! matches the CPU reference byte for byte. `GpuBlockRule::step` drives a tick with the shared
//! `engine_sim::step_with`: the CPU gathers each phase's blocks, the GPU applies the rule to all of them
//! in one dispatch, and the CPU scatters the results with the reference kernel's own bookkeeping. The
//! store therefore stays on the CPU and every phase crosses the bus; a GPU-resident pool with an
//! indirect dispatch over a GPU-built active list is the next step (A5b).
use engine_core::material::{MaterialId, SIM_MATERIAL_ROWS, SimMaterial};
use engine_core::reaction::SimRules;
use engine_sim::{BrickStore, Cell, Outcome, StepStats, gather, scatter, step_with};
use glam::IVec3;

pub mod resident;
pub use resident::{GpuWorld, Snapshot, UploadError, Window};

/// The block rule shared by both entry shaders; it declares no bindings.
pub const RULE_WGSL: &str = concat!(include_str!("pinned.wgsl"), "
", include_str!("rules.wgsl"), "
", include_str!("react.wgsl"));
/// The offload entry: blocks gathered by the CPU in, stepped blocks and outcomes out.
pub const OFFLOAD_WGSL: &str = include_str!("offload.wgsl");

/// An entry shader joined with the rule (module-scope declarations may come in any order).
pub fn program(entry: &str) -> String {
    format!("{entry}
{RULE_WGSL}")
}

/// Word offsets in the packed rules buffer; `block_rule.wgsl` repeats them as constants.
pub const MAT_BASE: usize = 0;
pub const REACTIVE_BASE: usize = MAT_BASE + SIM_MATERIAL_ROWS * 8;
pub const UNARY_BASE: usize = REACTIVE_BASE + SIM_MATERIAL_ROWS / 32;
pub const PAIR_BASE: usize = UNARY_BASE + SIM_MATERIAL_ROWS / 4;
pub const ROW_BASE: usize = PAIR_BASE + SIM_MATERIAL_ROWS * SIM_MATERIAL_ROWS / 4;

/// Blocks per workgroup in the shader.
const WORKGROUP: u32 = 64;
/// Most blocks one dispatch takes: the workgroup count must fit a dimension.
const MAX_BLOCKS_PER_DISPATCH: usize = 65_535 * WORKGROUP as usize;

/// One material row as eight words, field by field (not a memory copy, so the GPU layout is explicit).
fn pack_material(m: &SimMaterial) -> [u32; 8] {
    let lo_hi = |lo: u16, hi: u16| lo as u32 | (hi as u32) << 16;
    let bytes = |a: u8, b: u8, c: u8, d: u8| u32::from_le_bytes([a, b, c, d]);
    let id = |m: MaterialId| m.0;
    [
        m.density,
        m.thermal_mass,
        lo_hi(m.conductivity, m.capacity),
        lo_hi(m.melt, m.boil),
        lo_hi(m.ignite, m.cool),
        m.fuel as u32 | (m.class as u32) << 16 | (m.cohesion as u32) << 24,
        bytes(m.flow, m.pressure, m.electric, id(m.melts_into)),
        bytes(id(m.boils_into), id(m.burns_into), id(m.cools_into), 0),
    ]
}

/// Everything the shader reads about materials and reactions, in one `u32` buffer.
pub fn pack_rules(rules: &SimRules) -> Vec<u32> {
    let mut words = vec![0u32; ROW_BASE];
    for (m, row) in rules.materials().iter().enumerate() {
        words[MAT_BASE + m * 8..][..8].copy_from_slice(&pack_material(row));
        if rules.reactive(m as u8) {
            words[REACTIVE_BASE + m / 32] |= 1 << (m % 32);
        }
    }
    let table = rules.reactions();
    for (i, chunk) in table.unary_index().chunks_exact(4).enumerate() {
        words[UNARY_BASE + i] = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
    }
    for (a, row) in table.pair_index().iter().enumerate() {
        for (j, chunk) in row.chunks_exact(4).enumerate() {
            words[PAIR_BASE + a * SIM_MATERIAL_ROWS / 4 + j] = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        }
    }
    for r in table.rows() {
        words.extend([
            r.rate,
            r.a_above as u32 | (r.a_below as u32) << 16,
            r.a_temp as u32 | (r.b_temp as u32) << 16,
            u32::from_le_bytes([r.a_into.0, r.b_into.0, r.flags, r.reserved]),
        ]);
    }
    words
}

/// A cell as the shader's two words: material, volume, temperature; pressure, charge.
pub fn pack_cell(c: Cell) -> [u32; 2] {
    [c.material as u32 | (c.volume as u32) << 8 | (c.temperature as u32) << 16, c.pressure as u16 as u32 | (c.charge as u16 as u32) << 16]
}

pub fn unpack_cell([w0, w1]: [u32; 2]) -> Cell {
    Cell {
        material: w0 as u8,
        volume: (w0 >> 8) as u8,
        temperature: (w0 >> 16) as u16,
        pressure: w1 as u16 as i16,
        charge: (w1 >> 16) as u16 as i16,
    }
}

#[derive(Debug)]
pub enum GpuStepError {
    /// Waiting for the dispatch failed (device lost or timed out).
    Poll(wgpu::PollError),
    /// Reading the results back failed.
    Map(wgpu::BufferAsyncError),
    /// The map callback never ran.
    Lost,
}

/// The block rule as a compute pipeline, with the rules uploaded once.
pub struct GpuBlockRule {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
    rules: wgpu::Buffer,
}

impl GpuBlockRule {
    pub fn new(device: wgpu::Device, queue: wgpu::Queue, rules: &SimRules) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ca block rule"),
            source: wgpu::ShaderSource::Wgsl(program(OFFLOAD_WGSL).into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("ca block rule"),
            layout: None,
            module: &module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let words = pack_rules(rules);
        let rules = buffer(&device, "ca rules", bytemuck::cast_slice(&words), wgpu::BufferUsages::STORAGE);
        Self { device, queue, pipeline, rules }
    }

    /// Apply the block rule to every block: the cells after and each block's outcome, in input order.
    pub fn run(&self, seed: u32, phase_counter: u32, anchors: &[IVec3], cells: &[[Cell; 8]]) -> Result<(Vec<[Cell; 8]>, Vec<Outcome>), GpuStepError> {
        let mut after = Vec::with_capacity(cells.len());
        let mut outcomes = Vec::with_capacity(cells.len());
        for (a, c) in anchors.chunks(MAX_BLOCKS_PER_DISPATCH).zip(cells.chunks(MAX_BLOCKS_PER_DISPATCH)) {
            let (cells, out) = self.dispatch(seed, phase_counter, a, c)?;
            after.extend(cells);
            outcomes.extend(out);
        }
        Ok((after, outcomes))
    }

    fn dispatch(&self, seed: u32, phase_counter: u32, anchors: &[IVec3], cells: &[[Cell; 8]]) -> Result<(Vec<[Cell; 8]>, Vec<Outcome>), GpuStepError> {
        let n = cells.len();
        if n == 0 {
            return Ok((Vec::new(), Vec::new()));
        }
        let d = &self.device;
        let params = buffer(d, "ca params", bytemuck::cast_slice(&[seed, phase_counter, n as u32, 0]), wgpu::BufferUsages::UNIFORM);
        let anchor_words: Vec<[i32; 4]> = anchors.iter().map(|a| [a.x, a.y, a.z, 0]).collect();
        let anchors = buffer(d, "ca anchors", bytemuck::cast_slice(&anchor_words), wgpu::BufferUsages::STORAGE);
        let block_words: Vec<u32> = cells.iter().flat_map(|b| b.iter().flat_map(|&c| pack_cell(c))).collect();
        let copy = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC;
        let blocks = buffer(d, "ca blocks", bytemuck::cast_slice(&block_words), copy);
        let outcomes = buffer(d, "ca outcomes", &vec![0u8; n * 4], copy);
        let (block_bytes, outcome_bytes) = ((n * 64) as u64, (n * 4) as u64);
        let readback = d.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ca readback"),
            size: block_bytes + outcome_bytes,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let bindings = d.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ca block rule"),
            layout: &self.pipeline.get_bind_group_layout(0),
            entries: &[&params, &self.rules, &anchors, &blocks, &outcomes]
                .iter()
                .enumerate()
                .map(|(i, b)| wgpu::BindGroupEntry { binding: i as u32, resource: b.as_entire_binding() })
                .collect::<Vec<_>>(),
        });
        let mut encoder = d.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bindings, &[]);
            pass.dispatch_workgroups((n as u32).div_ceil(WORKGROUP), 1, 1);
        }
        encoder.copy_buffer_to_buffer(&blocks, 0, &readback, 0, block_bytes);
        encoder.copy_buffer_to_buffer(&outcomes, 0, &readback, block_bytes, outcome_bytes);
        let submission = self.queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        readback.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        d.poll(wgpu::PollType::Wait { submission_index: Some(submission), timeout: None }).map_err(GpuStepError::Poll)?;
        rx.recv().map_err(|_| GpuStepError::Lost)?.map_err(GpuStepError::Map)?;
        let mapped = readback.slice(..).get_mapped_range();
        let words: &[u32] = bytemuck::cast_slice(&mapped);
        let after = (0..n).map(|k| std::array::from_fn(|i| unpack_cell([words[k * 16 + 2 * i], words[k * 16 + 2 * i + 1]]))).collect();
        let flags = &words[n * 16..];
        let outcomes = flags.iter().map(|&f| Outcome { changed: f & 1 != 0, pending: f & 2 != 0 }).collect();
        drop(mapped);
        readback.unmap();
        Ok((after, outcomes))
    }

    /// One tick of `store`, with the block rule on the GPU and the same scheduling as `engine_sim::step`.
    pub fn step(&self, store: &mut BrickStore, seed: u32, tick: u32) -> Result<StepStats, GpuStepError> {
        let mut failure = None;
        let stats = step_with(store, tick, |store, phase_counter, bricks, stats| {
            if failure.is_some() {
                return;
            }
            let blocks = gather(store, bricks, phase_counter);
            match self.run(seed, phase_counter, &blocks.anchors, &blocks.cells) {
                Ok((after, outcomes)) => scatter(store, &blocks, &after, &outcomes, stats),
                Err(e) => failure = Some(e),
            }
        });
        failure.map_or(Ok(stats), Err)
    }
}

fn buffer(device: &wgpu::Device, label: &str, contents: &[u8], usage: wgpu::BufferUsages) -> wgpu::Buffer {
    use wgpu::util::DeviceExt;
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some(label), contents, usage })
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
#[cfg(test)]
#[path = "review_tests.rs"]
mod review_tests;
