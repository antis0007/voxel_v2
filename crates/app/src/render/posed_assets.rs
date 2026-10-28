//! GPU half of `mesh::models::posed`: the asset pool stays resident and one
//! compute pass writes this frame's posed copies into a vertex buffer that the
//! main pass draws with the scene pipeline.
//!
//! Before this, the scene rebuilt and re-uploaded about 140k anchorage
//! vertices every frame on the main thread (8 MB of transform and copy).
use crate::mesh::{Vertex3, models::posed::Placement};

const VERTEX_BYTES: u64 = std::mem::size_of::<Vertex3>() as u64;

/// `Placement` in `posed_assets.wgsl`.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuPlacement {
    rotation: [f32; 4],
    translation: [f32; 3],
    scale: f32,
    first: u32,
    count: u32,
    out_start: u32,
    _pad: u32,
}

pub(super) struct PosedAssets {
    pipeline: wgpu::ComputePipeline,
    layout: wgpu::BindGroupLayout,
    params: wgpu::Buffer,
    pool: Option<wgpu::Buffer>,
    pool_capacity: u64,
    pool_uploaded: usize,
    generation: u64,
    placements: Option<wgpu::Buffer>,
    placement_capacity: u64,
    output: Option<wgpu::Buffer>,
    output_capacity: u64,
    bind_group: Option<wgpu::BindGroup>,
    /// Vertices posed this frame; zero skips the dispatch and the draw.
    vertices: u32,
    warned: bool,
}

fn storage_entry(binding: u32, read_only: bool) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

impl PosedAssets {
    pub(super) fn new(device: &wgpu::Device) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("posed assets layout"),
            entries: &[
                storage_entry(0, true),
                storage_entry(1, true),
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(16),
                    },
                    count: None,
                },
                storage_entry(3, false),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("posed assets pipeline layout"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("posed assets shader"),
            source: wgpu::ShaderSource::Wgsl(crate::shader_sources::posed_assets_shader_source().into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("posed assets pipeline"),
            layout: Some(&pipeline_layout),
            module: &module,
            entry_point: Some("cs_pose"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("posed assets params"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            pipeline,
            layout,
            params,
            pool: None,
            pool_capacity: 0,
            pool_uploaded: 0,
            generation: u64::MAX,
            placements: None,
            placement_capacity: 0,
            output: None,
            output_capacity: 0,
            bind_group: None,
            vertices: 0,
            warned: false,
        }
    }

    /// Bring the resident pool up to date and write this frame's placements.
    pub(super) fn upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        placements: &[Placement],
        pool: &[Vertex3],
        generation: u64,
    ) {
        self.vertices = 0;
        let total: u64 = placements.iter().map(|p| u64::from(p.count)).sum();
        if total == 0 {
            return;
        }
        let limit = u64::from(device.limits().max_storage_buffer_binding_size);
        let pool_bytes = pool.len() as u64 * VERTEX_BYTES;
        if pool_bytes.next_power_of_two() > limit || (total * VERTEX_BYTES).next_power_of_two() > limit {
            if !self.warned {
                eprintln!("[posed-assets] pool {pool_bytes} B or output {} B exceeds the {limit} B storage limit; orbital crowns skipped", total * VERTEX_BYTES);
                self.warned = true;
            }
            return;
        }
        let mut rebind = false;
        if generation != self.generation || pool_bytes > self.pool_capacity {
            self.pool_capacity = pool_bytes.next_power_of_two().max(1 << 20);
            self.pool = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("posed assets pool"),
                size: self.pool_capacity,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
            self.pool_uploaded = 0;
            self.generation = generation;
            rebind = true;
        }
        if let Some(buffer) = &self.pool
            && self.pool_uploaded < pool.len() {
                let offset = self.pool_uploaded as u64 * VERTEX_BYTES;
                queue.write_buffer(buffer, offset, bytemuck::cast_slice(&pool[self.pool_uploaded..]));
                self.pool_uploaded = pool.len();
            }
        let placement_bytes = (placements.len() * std::mem::size_of::<GpuPlacement>()) as u64;
        if placement_bytes > self.placement_capacity {
            self.placement_capacity = placement_bytes.next_power_of_two().max(4096);
            self.placements = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("posed assets placements"),
                size: self.placement_capacity,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
            rebind = true;
        }
        let output_bytes = total * VERTEX_BYTES;
        if output_bytes > self.output_capacity {
            self.output_capacity = output_bytes.next_power_of_two().max(1 << 20);
            self.output = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("posed assets vertices"),
                size: self.output_capacity,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }));
            rebind = true;
        }
        let (Some(pool_buffer), Some(placement_buffer), Some(output)) = (&self.pool, &self.placements, &self.output)
        else {
            return;
        };
        let mut out_start = 0u32;
        let gpu: Vec<GpuPlacement> = placements
            .iter()
            .map(|p| {
                let gpu = GpuPlacement {
                    rotation: p.rotation.to_array(),
                    translation: p.translation.to_array(),
                    scale: p.scale,
                    first: p.first,
                    count: p.count,
                    out_start,
                    _pad: 0,
                };
                out_start += p.count;
                gpu
            })
            .collect();
        queue.write_buffer(placement_buffer, 0, bytemuck::cast_slice(&gpu));
        queue.write_buffer(&self.params, 0, bytemuck::cast_slice(&[placements.len() as u32, total as u32, 0, 0]));
        if rebind || self.bind_group.is_none() {
            self.bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("posed assets bind group"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: pool_buffer.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: placement_buffer.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 2, resource: self.params.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 3, resource: output.as_entire_binding() },
                ],
            }));
        }
        self.vertices = total as u32;
    }

    /// Record before the pass that draws them.
    pub(super) fn record(&self, encoder: &mut wgpu::CommandEncoder) {
        let Some(bind_group) = self.bind_group.as_ref().filter(|_| self.vertices > 0) else {
            return;
        };
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("posed assets pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, bind_group, &[]);
        pass.dispatch_workgroups(self.vertices.div_ceil(64), 1, 1);
    }

    /// The posed vertices, if any, for a scene-pipeline draw.
    pub(super) fn vertices(&self) -> Option<(wgpu::BufferSlice<'_>, u32)> {
        let output = self.output.as_ref().filter(|_| self.vertices > 0)?;
        Some((output.slice(..u64::from(self.vertices) * VERTEX_BYTES), self.vertices))
    }
}

#[cfg(test)]
#[path = "posed_assets_tests.rs"]
mod tests;
