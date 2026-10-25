//! Waits for old frames before acquiring a swapchain image.
//!
//! wgpu 27-29 on Vulkan waits at most 1 s inside `get_current_texture` for the
//! image's fence and maps `VK_TIMEOUT` there to `DeviceError::Unexpected`,
//! which wgpu-core treats as a lost device. Any GPU backlog over a second
//! (contention, a heavy upload frame) therefore ended the session with
//! "Unexpected error variant" and no driver reset. The image being acquired
//! is freed by the present of the frame `latency` frames back, so waiting for
//! that submission first keeps wgpu's internal wait short without reducing
//! the frames in flight.
use std::collections::VecDeque;
use std::time::Duration;

/// Longer than any frame we expect to finish; a real hang still ends the wait.
const WAIT_LIMIT: Duration = Duration::from_secs(20);

pub(crate) struct FramePacer<T = wgpu::SubmissionIndex> {
    latency: usize,
    submitted: VecDeque<T>,
}

impl<T> FramePacer<T> {
    pub(crate) fn new(desired_maximum_frame_latency: u32) -> Self {
        let latency = desired_maximum_frame_latency.max(1) as usize;
        Self { latency, submitted: VecDeque::with_capacity(latency + 1) }
    }

    /// Record the last submission of a presented frame.
    pub(crate) fn submitted(&mut self, index: T) {
        self.submitted.push_back(index);
        while self.submitted.len() > self.latency {
            self.submitted.pop_front();
        }
    }

    /// The submission whose present frees the next swapchain image, once
    /// `latency` frames are in flight.
    fn gate(&self) -> Option<&T> {
        (self.submitted.len() == self.latency).then(|| &self.submitted[0])
    }
}

impl FramePacer {
    /// `VOXEL_FRAME_PACER=0` turns the wait off for A/B runs.
    pub(crate) fn wait_before_acquire(&self, device: &wgpu::Device) {
        static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        if !*ENABLED.get_or_init(|| std::env::var("VOXEL_FRAME_PACER").map_or(true, |v| v != "0")) {
            return;
        }
        if let Some(index) = self.gate() {
            let poll = wgpu::PollType::Wait { submission_index: Some(index.clone()), timeout: Some(WAIT_LIMIT) };
            if let Err(error) = device.poll(poll) {
                eprintln!("[frame-pacer] wait before acquire failed: {error:?}");
            }
        }
    }
}

/// Debug hook `VOXEL_DEBUG_GPU_STALL=<frame>:<copies>`: once, at the first
/// world frame from `frame` on, queue `copies` 256 MiB buffer copies to make
/// the GPU fall seconds behind.
/// Each copy is a separate command, so the driver watchdog never fires; it
/// reproduces the acquire timeout this module prevents.
pub(crate) fn debug_stall(device: &wgpu::Device, encoder: &mut wgpu::CommandEncoder, frame_id: u64) {
    use std::sync::OnceLock;
    static PLAN: OnceLock<Option<(u64, u32)>> = OnceLock::new();
    let plan = PLAN.get_or_init(|| {
        let value = std::env::var("VOXEL_DEBUG_GPU_STALL").ok()?;
        let (frame, copies) = value.split_once(':')?;
        Some((frame.parse().ok()?, copies.parse().ok()?))
    });
    let Some((frame, copies)) = *plan else { return };
    // Loading-screen frames advance the same counter, so fire on the first
    // world frame at or after `frame`, once.
    static FIRED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if frame_id < frame || FIRED.swap(true, std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    const BYTES: u64 = 256 << 20;
    let buffer = |label| device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label), size: BYTES, usage: wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let (src, dst) = (buffer("debug-stall-src"), buffer("debug-stall-dst"));
    for _ in 0..copies {
        encoder.copy_buffer_to_buffer(&src, 0, &dst, 0, BYTES);
    }
    eprintln!("[frame-pacer] debug stall queued frame={frame_id} copies={copies}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gate_is_the_submission_frame_latency_frames_back() {
        for latency in [1u32, 2, 3] {
            let mut pacer = FramePacer::<usize>::new(latency);
            for frame in 0..6usize {
                let expected = frame.checked_sub(latency as usize);
                assert_eq!(pacer.gate().copied(), expected, "latency={latency} frame={frame}");
                pacer.submitted(frame);
            }
        }
        assert_eq!(FramePacer::<usize>::new(0).latency, 1, "zero latency still gates one frame back");
    }
}
