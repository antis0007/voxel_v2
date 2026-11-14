//! Shared GPU storage for chunk meshes (docs/plans/2026-10-22-render-arena-and-horizon-evidence.md).
//! A mesh owns an `ArenaLease`; dropping the lease returns its ranges through a
//! channel that the arena drains once per frame, after the previous submit, so a
//! freed range is never rewritten while an unsubmitted draw still reads it.
use std::collections::BTreeMap;
use std::time::{Duration, Instant};
use std::sync::mpsc::{Receiver, Sender, channel};

const VERTEX_BYTES: u64 = std::mem::size_of::<crate::mesh::Vertex3>() as u64;
const INDEX_BYTES: u64 = 4;
/// Indices run about 24 B per quad against 224 B of vertices; both page buffers
/// fit `Limits::default().max_buffer_size` (256 MiB) and are clamped to the device.
/// 128 MiB rounded down to whole vertices, so every page byte is usable.
pub(super) const PAGE_VERTEX_BYTES: u64 = (128 << 20) / VERTEX_BYTES * VERTEX_BYTES;
pub(super) const PAGE_INDEX_BYTES: u64 = 16 << 20;

struct ArenaPage {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    vertex_alloc: RangeAllocator,
    index_alloc: RangeAllocator,
    empty_since: Option<Instant>,
}

impl ArenaPage {
    fn take(&mut self, page: u32, vertex_bytes: u64, index_bytes: u64) -> Option<ArenaRange> {
        if !self.vertex_alloc.may_fit(vertex_bytes) || !self.index_alloc.may_fit(index_bytes) {
            return None;
        }
        let vertex_offset = self.vertex_alloc.alloc(vertex_bytes)?;
        let Some(index_offset) = self.index_alloc.alloc(index_bytes) else {
            self.vertex_alloc.free(vertex_offset, vertex_bytes);
            return None;
        };
        self.empty_since = None;
        Some(ArenaRange { page, vertex_offset, vertex_bytes, index_offset, index_bytes })
    }
}

/// Pages of shared vertex and index buffers. Pages are created on demand up to
/// the byte budget; unused trailing pages are released after leases drop.
pub(super) struct MeshArena {
    pages: Vec<ArenaPage>,
    budget: u64,
    returns: Sender<ArenaRange>,
    freed: Receiver<ArenaRange>,
}

impl MeshArena {
    pub(super) fn new(budget_bytes: usize) -> Self {
        let (returns, freed) = channel();
        Self { pages: Vec::new(), budget: budget_bytes as u64, returns, freed }
    }

    pub(super) fn set_budget(&mut self, budget_bytes: usize) {
        self.budget = budget_bytes as u64;
    }

    fn page_sizes(device: &wgpu::Device) -> (u64, u64) {
        let limit = device.limits().max_buffer_size;
        (PAGE_VERTEX_BYTES.min(limit) / VERTEX_BYTES * VERTEX_BYTES, PAGE_INDEX_BYTES.min(limit) / INDEX_BYTES * INDEX_BYTES)
    }

    /// `None` for an empty mesh, a mesh larger than a page, or a full budget.
    pub(super) fn alloc(&mut self, device: &wgpu::Device, vertex_bytes: u64, index_bytes: u64) -> Option<ArenaLease> {
        if vertex_bytes == 0 || index_bytes == 0 {
            return None;
        }
        let lease = |range| ArenaLease::new(range, self.returns.clone());
        for (i, page) in self.pages.iter_mut().enumerate() {
            if let Some(range) = page.take(i as u32, vertex_bytes, index_bytes) {
                return Some(lease(range));
            }
        }
        let (vertex_page, index_page) = Self::page_sizes(device);
        if vertex_bytes > vertex_page
            || index_bytes > index_page
            || (self.pages.len() as u64 + 1) * (vertex_page + index_page) > self.budget
        {
            return None;
        }
        let mut page = Self::create_page(device, vertex_page, index_page)?;
        let range = page.take(self.pages.len() as u32, vertex_bytes, index_bytes)?;
        self.pages.push(page);
        Some(lease(range))
    }

    /// `Ok(None)` for a mesh with nothing to draw, `Err(())` when it does not fit.
    pub(super) fn alloc_mesh(
        &mut self,
        device: &wgpu::Device,
        mesh: &crate::mesh::ChunkMesh,
    ) -> Result<Option<ArenaLease>, ()> {
        if mesh.indices.is_empty() {
            return Ok(None);
        }
        let vertex_bytes = std::mem::size_of_val(mesh.vertices.as_slice()) as u64;
        let index_bytes = std::mem::size_of_val(mesh.indices.as_slice()) as u64;
        self.alloc(device, vertex_bytes, index_bytes).map(Some).ok_or(())
    }

    /// Page creation has its own error scopes: an out-of-memory page must be a
    /// rejected upload, not wgpu's uncaptured-error panic.
    fn create_page(device: &wgpu::Device, vertex_bytes: u64, index_bytes: u64) -> Option<ArenaPage> {
        let memory_scope = device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
        let validation_scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let buffer = |label, size, usage: wgpu::BufferUsages| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage: usage | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        let vertices = buffer("chunk mesh arena vertices", vertex_bytes, wgpu::BufferUsages::VERTEX);
        let indices = buffer("chunk mesh arena indices", index_bytes, wgpu::BufferUsages::INDEX);
        let mut validation = validation_scope.pop();
        let mut memory = memory_scope.pop();
        // Native wgpu resolves scopes immediately; a still-pending scope is
        // treated as success and any late error surfaces through the upload Check.
        for scope in [&mut validation, &mut memory] {
            if let std::task::Poll::Ready(Some(error)) = std::pin::Pin::new(scope)
                .poll(&mut std::task::Context::from_waker(std::task::Waker::noop()))
            {
                eprintln!("Chunk mesh arena page rejected: {error}");
                return None;
            }
        }
        Some(ArenaPage {
            vertices,
            indices,
            vertex_alloc: RangeAllocator::new(vertex_bytes, VERTEX_BYTES),
            index_alloc: RangeAllocator::new(index_bytes, INDEX_BYTES),
            empty_since: None,
        })
    }

    /// Return dropped leases to their pages. Call only while no encoder that
    /// reads the arena is open (between a frame's submit and the next encode).
    pub(super) fn begin_frame(&mut self) {
        // A reset can return thousands of ranges at once. Unprocessed ranges
        // remain allocated and cannot be reused until a later maintenance pass.
        let started = Instant::now();
        for serviced in 0..128 {
            if serviced > 0 && started.elapsed() >= Duration::from_micros(250) { break; }
            let Ok(range) = self.freed.try_recv() else { break; };
            if let Some(page) = self.pages.get_mut(range.page as usize) {
                page.vertex_alloc.free(range.vertex_offset, range.vertex_bytes);
                page.index_alloc.free(range.index_offset, range.index_bytes);
            }
        }
        let now = Instant::now();
        for page in &mut self.pages {
            if page.vertex_alloc.used() == 0 && page.index_alloc.used() == 0 {
                page.empty_since.get_or_insert(now);
            } else { page.empty_since = None; }
        }
        // Preserve stable page indices and avoid allocating 144 MiB again on
        // the next edited mesh. Budget reductions still reclaim empty tails.
        // Driver destruction is outside our time control; retire at most one
        // pair of buffers per pass, preserving all remaining page indices.
        if let Some(page) = self.pages.last() {
            let Some(since) = page.empty_since else { return; };
            let allocated: u64 = self.pages.iter().map(|p|p.vertex_alloc.capacity()+p.index_alloc.capacity()).sum();
            if allocated <= self.budget && now.duration_since(since) < Duration::from_secs(5) { return; }
            self.pages.pop();
        }
    }

    pub(super) fn buffers(&self, page: u32) -> Option<(&wgpu::Buffer, &wgpu::Buffer)> {
        self.pages.get(page as usize).map(|p| (&p.vertices, &p.indices))
    }

    pub(super) fn capacity_bytes(&self) -> u64 {
        self.pages.iter().map(|page| page.vertices.size() + page.indices.size()).sum()
    }

    /// Includes ranges queued for release until the allocator drains them.
    pub(super) fn used_bytes(&self) -> u64 {
        self.pages.iter().map(|p| p.vertex_alloc.used() + p.index_alloc.used()).sum()
    }
}

/// First-fit sub-allocator over one buffer. Sizes round up to `align`, so every
/// offset handed out is a multiple of it.
pub(super) struct RangeAllocator {
    capacity: u64,
    align: u64,
    used: u64,
    // Conservative upper bound. Allocation cannot enlarge the actual maximum;
    // a failed first-fit scan refreshes this to the exact largest free range.
    largest_free_bound: u64,
    /// Free ranges, offset -> length; neighbours are always coalesced.
    free: BTreeMap<u64, u64>,
}

impl RangeAllocator {
    pub(super) fn new(capacity: u64, align: u64) -> Self {
        let align = align.max(1);
        let capacity = capacity / align * align;
        let free = if capacity > 0 { BTreeMap::from([(0, capacity)]) } else { BTreeMap::new() };
        Self { capacity, align, used: 0, largest_free_bound: capacity, free }
    }

    fn round(&self, size: u64) -> u64 {
        size.div_ceil(self.align) * self.align
    }

    fn may_fit(&self, size: u64) -> bool {
        self.round(size) <= self.largest_free_bound.min(self.capacity - self.used)
    }

    pub(super) fn alloc(&mut self, size: u64) -> Option<u64> {
        if size == 0 || !self.may_fit(size) {
            return None;
        }
        let size = self.round(size);
        let mut largest = 0;
        let Some((&offset, &len)) = self.free.iter().find(|(_, len)| {
            largest = largest.max(**len);
            **len >= size
        }) else {
            self.largest_free_bound = largest;
            return None;
        };
        self.free.remove(&offset);
        if len > size {
            self.free.insert(offset + size, len - size);
        }
        self.used += size;
        Some(offset)
    }

    pub(super) fn free(&mut self, offset: u64, size: u64) {
        let size = self.round(size);
        debug_assert!(offset + size <= self.capacity, "free outside the allocator");
        let (mut start, mut len) = (offset, size);
        if let Some((&prev, &prev_len)) = self.free.range(..offset).next_back()
            && prev + prev_len == offset
        {
            self.free.remove(&prev);
            start = prev;
            len += prev_len;
        }
        if let Some(next_len) = self.free.remove(&(offset + size)) {
            len += next_len;
        }
        self.free.insert(start, len);
        self.used -= size;
        self.largest_free_bound = self.largest_free_bound.max(len);
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn used(&self) -> u64 {
        self.used
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn largest_free(&self) -> u64 {
        self.free.values().copied().max().unwrap_or(0)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn capacity(&self) -> u64 {
        self.capacity
    }
}

/// Where one mesh lives: a page and a byte range in each of its two buffers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ArenaRange {
    pub page: u32,
    pub vertex_offset: u64,
    pub vertex_bytes: u64,
    pub index_offset: u64,
    pub index_bytes: u64,
}

impl ArenaRange {
    pub(super) fn first_index(&self) -> u32 {
        (self.index_offset / INDEX_BYTES) as u32
    }

    pub(super) fn base_vertex(&self) -> i32 {
        debug_assert_eq!(self.vertex_offset % VERTEX_BYTES, 0, "vertex range not vertex aligned");
        (self.vertex_offset / VERTEX_BYTES) as i32
    }
}

/// Ownership of an `ArenaRange`. Dropping it hands the range back to the arena.
pub(super) struct ArenaLease {
    range: ArenaRange,
    returns: Sender<ArenaRange>,
}

impl ArenaLease {
    pub(super) fn new(range: ArenaRange, returns: Sender<ArenaRange>) -> Self {
        Self { range, returns }
    }

    pub(super) fn range(&self) -> &ArenaRange {
        &self.range
    }
}

impl Drop for ArenaLease {
    fn drop(&mut self) {
        // The arena may already be gone (device reset); then nothing owns pages.
        let _ = self.returns.send(self.range);
    }
}

#[cfg(test)]
#[path = "arena_tests.rs"]
mod tests;
