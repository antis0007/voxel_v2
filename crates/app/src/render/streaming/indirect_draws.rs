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
#[cfg(test)]
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

/// Stable sort by page: one run per page, list order kept within each page.
fn group_by_page<T>(draws: &mut [(u32, T)]) {
    draws.sort_by_key(|&(page, _)| page);
}

/// One draw list's indirect argument buffer.
#[derive(Default)]
pub(in crate::render) struct IndirectDraws {
    buffer: Option<wgpu::Buffer>,
    capacity: u64,
    /// `(page, byte offset, draw count)` from the last `prepare`.
    runs: Vec<(u32, u64, u32)>,
    // Retain CPU staging capacity just as we retain the GPU argument buffer.
    draws: Vec<(u32, wgpu::util::DrawIndexedIndirectArgs)>,
    upload_bytes: Vec<u8>,
    uploaded_bytes: Vec<u8>,
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
        slot_of: impl FnMut(IVec3) -> u32,
    ) -> (usize, usize) {
        self.prepare_ordered(device, queue, cache, coords, slot_of, false)
    }

    /// `prepare`, but one run per arena page, keeping list order within a page.
    ///
    /// Neighbouring chunks rarely share a page, so a list in spatial order
    /// splits into about one run (and one vertex/index rebind) per draw. For
    /// depth-only passes the lost cross-page order costs less than the rebinds.
    pub(in crate::render) fn prepare_grouped(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        cache: &ChunkCache,
        coords: impl Iterator<Item = IVec3>,
        slot_of: impl FnMut(IVec3) -> u32,
    ) -> (usize, usize) {
        self.prepare_ordered(device, queue, cache, coords, slot_of, true)
    }

    fn prepare_ordered(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        cache: &ChunkCache,
        coords: impl Iterator<Item = IVec3>,
        mut slot_of: impl FnMut(IVec3) -> u32,
        group_pages: bool,
    ) -> (usize, usize) {
        self.draws.clear();
        let mut indices = 0usize;
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
            self.draws.push((slice.page, args));
        }
        if group_pages {
            group_by_page(&mut self.draws);
        }
        self.pack_arguments();
        if self.upload_bytes.is_empty() {
            return (0, 0);
        }
        let allocate = self.buffer.is_none() || (self.upload_bytes.len() as u64) > self.capacity;
        if allocate {
            self.capacity = (self.upload_bytes.len() as u64).next_power_of_two().max(16_384 * ARGS_BYTES);
            self.buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("chunk indirect draws"),
                size: self.capacity,
                usage: wgpu::BufferUsages::INDIRECT | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        if (allocate || self.upload_bytes != self.uploaded_bytes)
            && let Some(buffer) = &self.buffer
        {
            queue.write_buffer(buffer, 0, &self.upload_bytes);
            std::mem::swap(&mut self.upload_bytes, &mut self.uploaded_bytes);
        }
        (self.draws.len(), indices)
    }

    /// Pack arguments and page runs together, in the exact submitted order.
    /// Empty batches must also clear runs so no previous-frame draws survive.
    fn pack_arguments(&mut self) {
        self.upload_bytes.clear();
        self.upload_bytes.reserve(self.draws.len() * ARGS_BYTES as usize);
        self.runs.clear();
        for (index, (page, args)) in self.draws.iter().enumerate() {
            self.upload_bytes.extend_from_slice(args.as_bytes());
            match self.runs.last_mut() {
                Some((previous_page, _, count)) if previous_page == page => *count += 1,
                _ => self.runs.push((*page, index as u64 * ARGS_BYTES, 1)),
            }
        }
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
    use super::{group_by_page, split_runs};

    #[test]
    fn grouping_issues_one_run_per_page_and_keeps_order_within_a_page() {
        // Spatial order alternating across three pages: one run per draw.
        let mut draws: Vec<(u32, usize)> = [2, 0, 1, 0, 2, 1, 0].into_iter().zip(0..).collect();
        let pages: Vec<u32> = draws.iter().map(|d| d.0).collect();
        assert_eq!(split_runs(&pages).len(), 7);
        group_by_page(&mut draws);
        let pages: Vec<u32> = draws.iter().map(|d| d.0).collect();
        assert_eq!(split_runs(&pages), vec![(0, 0, 3), (1, 3, 2), (2, 5, 2)]);
        assert_eq!(draws.iter().map(|d| d.1).collect::<Vec<_>>(), vec![1, 3, 6, 2, 5, 0, 4]);
    }

    #[test]
    fn indirect_runs_split_on_page_change() {
        assert_eq!(split_runs(&[0, 0, 1, 0]), vec![(0, 0, 2), (1, 2, 1), (0, 3, 1)]);
        assert_eq!(split_runs(&[]), vec![]);
        assert_eq!(split_runs(&[3]), vec![(3, 0, 1)]);
    }
}
