//! Resource maintenance when surface redraws are suspended.
use super::{ChunkCache, Gpu};

impl ChunkCache {
    /// Service submitted work and returned ranges without acquiring a surface.
    pub(crate) fn maintain_retired_meshes(&mut self, gpu: &Gpu) {
        if gpu.device_usable()
            && let Err(error) = gpu.device.poll(wgpu::PollType::Poll)
        {
            eprintln!("[retirement] GPU poll failed: {error}");
        }
        // No render encoder is open in AboutToWait. Pending products stay
        // unpublished until the usual revision checks in the redraw path.
        self.arena.begin_frame();
    }
}
