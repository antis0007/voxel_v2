//! Multi-draw indirect for chunk meshes (docs/plans/2026-10-22-render-arena-and-horizon-evidence.md, Task 5).
//! Arguments are written in draw-list order and issued as one
//! `multi_draw_indexed_indirect` per run of consecutive draws on the same arena
//! page. Each list owns its buffer: queued writes all land before the frame's
//! submit, so two lists sharing one buffer would both read the last write.
use super::ChunkCache;
use glam::IVec3;
use std::sync::atomic::{AtomicBool, Ordering};

static ENABLED: AtomicBool = AtomicBool::new(true);
static SUPPORTED: AtomicBool = AtomicBool::new(false);
const ARGS_BYTES: u64 = std::mem::size_of::<wgpu::util::DrawIndexedIndirectArgs>() as u64;

/// `--benchmark-no-multidraw` turns this off for A/B runs.
pub fn set_enabled(enabled: bool) {
    ENABLED.store(enabled, Ordering::Relaxed);
}

/// Recorded once at device creation from the adapter and device capabilities.
///
/// Each draw selects its draw-offset slot through `first_instance`, so without
/// `INDIRECT_FIRST_INSTANCE` every draw goes through the direct path instead.
pub fn set_supported(adapter: &wgpu::Adapter, device_features: wgpu::Features) {
    let flags = adapter.get_downlevel_capabilities().flags;
    let supported = flags.contains(wgpu::DownlevelFlags::INDIRECT_EXECUTION)
        && device_features.contains(wgpu::Features::INDIRECT_FIRST_INSTANCE);
    SUPPORTED.store(supported, Ordering::Relaxed);
}

pub fn active() -> bool {
    ENABLED.load(Ordering::Relaxed) && SUPPORTED.load(Ordering::Relaxed)
}

/// Runs of consecutive equal pages as `(page, first_draw, count)`.
pub(in crate::render) fn split_runs(pages: &[u32]) -> Vec<(u32, u32, u32)> {
    let mut runs: Vec<(u32, u32, u32)> = Vec::new();
    for (i, &page) in pages.iter().enumerate() {
        match runs.last_mut() {
            Some(run) if run.0 == page => run.2 += 1,
            _ => runs.push((page, i as u32, 1)),
        }
    }
    runs
}

/// One draw list's indirect argument buffer.
#[derive(Default)]
pub(in crate::render) struct IndirectDraws {
    buffer: Option<wgpu::Buffer>,
    capacity: u64,
    /// `(page, byte offset, draw count)` from the last `prepare`.
    runs: Vec<(u32, u64, u32)>,
}

impl IndirectDraws {
    /// Writes arguments for the meshes at `coords` (in order, skipping ones with
    /// no geometry) and records the per-page runs. Returns (draws, indices).
    /// `slot_of` gives each mesh's draw-offset slot, written to `first_instance`.
    pub(in crate::render) fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        cache: &ChunkCache,
        coords: impl Iterator<Item = IVec3>,
        mut slot_of: impl FnMut(IVec3) -> u32,
    ) -> (usize, usize) {
        let mut pages = Vec::new();
        let mut indices = 0usize;
        let mut bytes = Vec::new();
        for coord in coords {
            let Some(slice) = cache.draw_slice(coord) else { continue };
            let args = wgpu::util::DrawIndexedIndirectArgs {
                index_count: slice.index_range.len() as u32,
                instance_count: 1,
                first_index: slice.index_range.start,
                base_vertex: slice.base_vertex,
                first_instance: slot_of(coord),
            };
            indices += args.index_count as usize;
            bytes.extend_from_slice(args.as_bytes());
            pages.push(slice.page);
        }
        self.runs = split_runs(&pages)
            .into_iter()
            .map(|(page, first, count)| (page, first as u64 * ARGS_BYTES, count))
            .collect();
        if bytes.is_empty() {
            return (0, 0);
        }
        if self.buffer.is_none() || (bytes.len() as u64) > self.capacity {
            self.capacity = (bytes.len() as u64).next_power_of_two().max(16_384 * ARGS_BYTES);
            self.buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("chunk indirect draws"),
                size: self.capacity,
                usage: wgpu::BufferUsages::INDIRECT | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        if let Some(buffer) = &self.buffer {
            queue.write_buffer(buffer, 0, &bytes);
        }
        (pages.len(), indices)
    }

    /// Issues the prepared runs. Buffers rebind only when the page changes.
    pub(in crate::render) fn draw(&self, pass: &mut wgpu::RenderPass<'_>, cache: &ChunkCache) {
        let Some(buffer) = &self.buffer else { return };
        for &(page, offset, count) in &self.runs {
            let Some((vertices, indices)) = cache.page_buffers(page) else { continue };
            pass.set_vertex_buffer(0, vertices.slice(..));
            pass.set_index_buffer(indices.slice(..), wgpu::IndexFormat::Uint32);
            pass.multi_draw_indexed_indirect(buffer, offset, count);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::split_runs;

    #[test]
    fn indirect_runs_split_on_page_change() {
        assert_eq!(split_runs(&[0, 0, 1, 0]), vec![(0, 0, 2), (1, 2, 1), (0, 3, 1)]);
        assert_eq!(split_runs(&[]), vec![]);
        assert_eq!(split_runs(&[3]), vec![(3, 0, 1)]);
    }
}
