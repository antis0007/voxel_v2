//! Limits GPU work in flight before acquiring a swapchain image.
//!
//! Queue completion bounds the backlog of our own submissions. The Vulkan
//! acquire can still wait for the presentation engine to release an image.
use std::cell::Cell;
use std::collections::VecDeque;
use std::sync::{
    Arc, OnceLock,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

pub(crate) struct Completion(Arc<AtomicBool>);

pub(crate) struct FramePacer<T = Completion> {
    latency: usize,
    submitted: VecDeque<T>,
    blocked_since: Cell<Option<Instant>>,
}

impl<T> FramePacer<T> {
    pub(crate) fn new(desired_maximum_frame_latency: u32) -> Self {
        let latency = desired_maximum_frame_latency.max(1) as usize;
        Self { latency, submitted: VecDeque::with_capacity(latency + 1), blocked_since: Cell::new(None) }
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
    fn enabled() -> bool {
        static ENABLED: OnceLock<bool> = OnceLock::new();
        *ENABLED.get_or_init(|| std::env::var("VOXEL_FRAME_PACER").map_or(true, |v| v != "0"))
    }

    pub(crate) fn reset(&mut self) {
        self.submitted.clear();
        self.blocked_since.set(None);
    }

    pub(crate) fn track_submission(&mut self, queue: &wgpu::Queue) {
        if !Self::enabled() {
            return;
        }
        let done = Arc::new(AtomicBool::new(false));
        let completion = Arc::clone(&done);
        queue.on_submitted_work_done(move || completion.store(true, Ordering::Release));
        self.submitted(Completion(done));
    }

    /// Service completion callbacks without waiting on a Vulkan fence on
    /// the event thread. Acquiring still requires the same completed frame.
    pub(crate) fn ready(&self, device: &wgpu::Device) -> bool {
        let Some(Completion(done)) = self.gate() else { return true };
        if !done.load(Ordering::Acquire) {
            let now = Instant::now();
            if let Err(error) = device.poll(wgpu::PollType::Poll) {
                eprintln!("[frame-pacer] completion poll failed: {error:?}");
                return false;
            }
            if !done.load(Ordering::Acquire) {
                if self.blocked_since.get().is_none() {
                    self.blocked_since.set(Some(now));
                }
                return false;
            }
        }
        if let Some(started) = self.blocked_since.take() {
            let elapsed = started.elapsed();
            if elapsed >= Duration::from_millis(50) {
                eprintln!("[frame-pacer] GPU backpressure_ms={:.2}", elapsed.as_secs_f64() * 1000.0);
            }
        }
        true
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
