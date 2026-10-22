//! Unmapped GPU-local destinations and fallible queue staging. Never map an
//! invalid destination buffer after an allocation/validation failure.
use super::arena::{ArenaLease, MeshArena};
use super::{ChunkGpuMesh, ChunkKey};
use crate::mesh::ChunkMesh;
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll, Waker},
    sync::OnceLock,
    time::Instant,
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
    arena: &MeshArena,
    alloc: Option<ArenaLease>,
) -> (Result<ChunkGpuMesh, String>, Check) {
    // Diagnostic only: keep allocation, staging and error-scope wall time
    // distinct. A slow upload is not itself proof of a driver/allocator cause.
    static TRACE: OnceLock<bool> = OnceLock::new();
    let trace = *TRACE.get_or_init(|| std::env::var("VOXEL_RENDER_TIMING").as_deref() == Ok("1"));
    let started = trace.then(Instant::now);
    device.push_error_scope(wgpu::ErrorFilter::Internal);
    device.push_error_scope(wgpu::ErrorFilter::Validation);
    device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
    let push_ms = started.map_or(0.0, |t| t.elapsed().as_secs_f64() * 1000.0);
    let mut phases = [[0.0f64; 4]; 2];
    let mut write = |slot: usize, label: &str, buffer: &wgpu::Buffer, offset: u64, bytes: &[u8]|
     -> Result<(), String> {
        let Some(size) = wgpu::BufferSize::new(bytes.len() as u64) else {
            return Ok(());
        };
        let phase = trace.then(Instant::now);
        let staging = queue.write_buffer_with(buffer, offset, size);
        if let Some(t) = phase { phases[slot][1] += t.elapsed().as_secs_f64() * 1000.0; }
        let Some(mut staging) = staging else {
            return Err(format!(
                "Could not stage {label} ({} bytes at {offset})",
                size.get()
            ));
        };
        let phase = trace.then(Instant::now);
        staging.copy_from_slice(bytes);
        if let Some(t) = phase { phases[slot][2] += t.elapsed().as_secs_f64() * 1000.0; }
        let phase = trace.then(Instant::now);
        drop(staging);
        if let Some(t) = phase { phases[slot][3] += t.elapsed().as_secs_f64() * 1000.0; }
        Ok(())
    };
    let result = (|| {
        match &alloc {
            Some(lease) => {
                let range = *lease.range();
                let (vertices, indices) = arena
                    .buffers(range.page)
                    .ok_or_else(|| format!("Chunk mesh arena page {} is missing", range.page))?;
                write(0, "chunk vertex upload", vertices, range.vertex_offset, bytemuck::cast_slice(&mesh.vertices))?;
                write(1, "chunk index upload", indices, range.index_offset, bytemuck::cast_slice(&mesh.indices))?;
            }
            // A published mesh with indices but no storage would draw nothing: a hole.
            None if !mesh.indices.is_empty() => return Err("Chunk mesh has no arena range".into()),
            None => {}
        }
        Ok(ChunkGpuMesh { boundary_profile:None,
            alloc,
            index_count: mesh.indices.len() as u32,
            lod: key.lod,
            lod_invariant,
            bytes: std::mem::size_of_val(mesh.vertices.as_slice())
                + std::mem::size_of_val(mesh.indices.as_slice()),
        })
    })();
    // Pop immediately, on the creating thread. Only polling the returned
    // futures is deferred; error scopes never leak across render passes.
    let phase = trace.then(Instant::now);
    let memory = device.pop_error_scope();
    let validation = device.pop_error_scope();
    let internal = device.pop_error_scope();
    let pop_ms = phase.map_or(0.0, |t| t.elapsed().as_secs_f64() * 1000.0);
    let check = Box::pin(async move {
        let errors = [memory.await, validation.await, internal.await];
        let messages: Vec<_> = errors
            .into_iter()
            .flatten()
            .map(|e| e.to_string())
            .collect();
        (!messages.is_empty()).then(|| messages.join("; "))
    });
    if let Some(started) = started {
        let total_ms = started.elapsed().as_secs_f64() * 1000.0;
        if total_ms > 25.0 {
            let accounted = push_ms + pop_ms + phases.iter().flatten().sum::<f64>();
            println!("[resident-upload-gpu] key={key:?} vertex_bytes={} index_bytes={} total_ms={total_ms:.4} scope_push_ms={push_ms:.4} vertex_stage_ms={:.4} vertex_copy_ms={:.4} vertex_stage_drop_ms={:.4} index_stage_ms={:.4} index_copy_ms={:.4} index_stage_drop_ms={:.4} scope_pop_ms={pop_ms:.4} residual_ms={:.4}",
                std::mem::size_of_val(mesh.vertices.as_slice()), std::mem::size_of_val(mesh.indices.as_slice()),
                phases[0][1], phases[0][2], phases[0][3],
                phases[1][1], phases[1][2], phases[1][3], (total_ms-accounted).max(0.0));
        }
    }
    (result, check)
}
