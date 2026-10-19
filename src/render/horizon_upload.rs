//! Incremental staging. Publish a backdrop only after every page is complete.
use super::{GpuMesh, GpuPage, Samples, pages::Page};
use std::{collections::VecDeque, time::Instant};

const FRAME_BYTES: usize = 8 * 1024 * 1024;

struct Current {
    source: Page,
    gpu: GpuPage,
    vertex_offset: usize,
    index_offset: usize,
}

pub(super) struct Upload {
    waiting: VecDeque<Page>,
    current: Option<Current>,
    finished: Vec<GpuPage>,
    samples: Samples,
    center: Option<(i32, i32)>,
    frames: usize,
    bytes: usize,
    started: Instant,
}

impl Upload {
    pub fn new(pages: Vec<Page>, samples: Samples, center: Option<(i32, i32)>) -> Self {
        Self { waiting: pages.into(), current: None, finished: Vec::new(), samples,
            center, frames: 0, bytes: 0, started: Instant::now() }
    }

    pub fn advance(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) -> bool {
        self.frames += 1;
        let mut budget = FRAME_BYTES;
        while budget > 0 {
            if self.current.is_none() {
                let Some(source) = self.waiting.pop_front() else { return true; };
                let buffer = |label, size, usage| device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some(label), size, usage: usage | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                let gpu = GpuPage {
                    vertices: buffer("horizon vertices", std::mem::size_of_val(source.mesh.vertices.as_slice()) as u64, wgpu::BufferUsages::VERTEX),
                    indices: buffer("horizon indices", std::mem::size_of_val(source.mesh.indices.as_slice()) as u64, wgpu::BufferUsages::INDEX),
                    count: source.mesh.indices.len() as u32,
                    batches: source.batches.clone(),
                };
                self.current = Some(Current { source, gpu, vertex_offset: 0, index_offset: 0 });
            }
            let current = self.current.as_mut().unwrap();
            let vertices: &[u8] = bytemuck::cast_slice(&current.source.mesh.vertices);
            let indices: &[u8] = bytemuck::cast_slice(&current.source.mesh.indices);
            // Every size and offset is four-byte aligned. The final slice may
            // split a vertex, but is never drawn until the complete swap.
            for (buffer, bytes, offset) in [
                (&current.gpu.vertices, vertices, &mut current.vertex_offset),
                (&current.gpu.indices, indices, &mut current.index_offset),
            ] {
                let count = (bytes.len() - *offset).min(budget);
                if count > 0 {
                    queue.write_buffer(buffer, *offset as u64, &bytes[*offset..*offset + count]);
                    *offset += count;
                    budget -= count;
                    self.bytes += count;
                }
            }
            if current.vertex_offset == vertices.len() && current.index_offset == indices.len() {
                self.finished.push(self.current.take().unwrap().gpu);
            }
        }
        self.current.is_none() && self.waiting.is_empty()
    }

    pub fn finish(self) -> (GpuMesh, Samples, Option<(i32, i32)>) {
        println!("[horizon] upload_complete pages={} bytes={} frames={} staging_limit_bytes={} elapsed_ms={:.1}",
            self.finished.len(), self.bytes, self.frames, FRAME_BYTES, self.started.elapsed().as_secs_f64() * 1000.0);
        (GpuMesh { pages: self.finished, cull: std::env::var("VOXEL_HORIZON_CULL").as_deref() != Ok("0") }, self.samples, self.center)
    }
}
