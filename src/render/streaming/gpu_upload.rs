//! Unmapped GPU-local destinations and fallible queue staging. Never map an
//! invalid destination buffer after an allocation/validation failure.
use super::{ChunkGpuMesh, ChunkKey};
use crate::mesh::ChunkMesh;
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll, Waker},
};
pub(super) type Check = Pin<Box<dyn Future<Output = Option<String>>>>;
pub(super) fn poll(check: &mut Check) -> Poll<Option<String>> {
    check.as_mut().poll(&mut Context::from_waker(Waker::noop()))
}
pub(super) fn supported(device: &wgpu::Device, mesh: &ChunkMesh) -> Result<(), String> {
    let vertex = std::mem::size_of_val(mesh.vertices.as_slice()) as u64;
    let index = std::mem::size_of_val(mesh.indices.as_slice()) as u64;
    let limit = device.limits().max_buffer_size;
    if vertex > limit || index > limit {
        return Err(format!(
            "Chunk mesh exceeds the device buffer limit ({vertex} vertex bytes, {index} index bytes, limit {limit})"
        ));
    }
    if mesh.indices.len() > u32::MAX as usize
        || (!mesh.indices.is_empty() && mesh.vertices.is_empty())
    {
        return Err("Invalid chunk mesh indices".into());
    }
    Ok(())
}
pub(super) fn start(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    mesh: &ChunkMesh,
    key: ChunkKey,
    lod_invariant: bool,
    empty_vertex: &wgpu::Buffer,
    empty_index: &wgpu::Buffer,
) -> (Result<ChunkGpuMesh, String>, Check) {
    device.push_error_scope(wgpu::ErrorFilter::Internal);
    device.push_error_scope(wgpu::ErrorFilter::Validation);
    device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
    let buffer = |label: &str,
                  bytes: &[u8],
                  usage: wgpu::BufferUsages,
                  empty: &wgpu::Buffer|
     -> Result<wgpu::Buffer, String> {
        let Some(size) = wgpu::BufferSize::new(bytes.len() as u64) else {
            return Ok(empty.clone());
        };
        let result = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: size.get(),
            usage: usage | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let Some(mut staging) = queue.write_buffer_with(&result, 0, size) else {
            return Err(format!(
                "Could not allocate or stage {label} ({} bytes)",
                size.get()
            ));
        };
        staging.copy_from_slice(bytes);
        drop(staging);
        Ok(result)
    };
    let result = (|| {
        let vertex_buffer = buffer(
            "chunk vertex upload",
            bytemuck::cast_slice(&mesh.vertices),
            wgpu::BufferUsages::VERTEX,
            empty_vertex,
        )?;
        let index_buffer = buffer(
            "chunk index upload",
            bytemuck::cast_slice(&mesh.indices),
            wgpu::BufferUsages::INDEX,
            empty_index,
        )?;
        Ok(ChunkGpuMesh {
            vertex_buffer,
            index_buffer,
            index_count: mesh.indices.len() as u32,
            lod: key.lod,
            lod_invariant,
            bytes: std::mem::size_of_val(mesh.vertices.as_slice())
                + std::mem::size_of_val(mesh.indices.as_slice()),
        })
    })();
    // Pop immediately, on the creating thread. Only polling the returned
    // futures is deferred; error scopes never leak across render passes.
    let memory = device.pop_error_scope();
    let validation = device.pop_error_scope();
    let internal = device.pop_error_scope();
    let check = Box::pin(async move {
        let errors = [memory.await, validation.await, internal.await];
        let messages: Vec<_> = errors
            .into_iter()
            .flatten()
            .map(|e| e.to_string())
            .collect();
        (!messages.is_empty()).then(|| messages.join("; "))
    });
    (result, check)
}
