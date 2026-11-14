use super::{ALLOC_HEADER, GpuWorld, Snapshot, Window};
use crate::GpuStepError;
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    task::Poll,
};

const MAX_REQUESTS: usize = 3;

#[derive(Debug, PartialEq, Eq)]
pub enum ReadbackStartError {
    Busy,
    ByteBudget { needed: u64, budget: u64 },
    DeviceLimit { needed: u64, limit: u64 },
}

struct Lease {
    ledger: Arc<Mutex<(usize, u64)>>,
    bytes: u64,
}
impl Drop for Lease {
    fn drop(&mut self) {
        let mut state = self.ledger.lock().unwrap_or_else(|e| e.into_inner());
        state.0 -= 1;
        state.1 -= self.bytes;
    }
}

struct Mapping {
    buffer: wgpu::Buffer,
    _lease: Lease,
    abandoned: AtomicBool,
    unmapped: AtomicBool,
}
impl Mapping {
    fn unmap(&self) {
        if !self.unmapped.swap(true, Ordering::AcqRel) {
            self.buffer.unmap();
        }
    }
}

/// A single captured submission; its request identity cannot be relabelled.
pub struct PendingReadback {
    device: wgpu::Device,
    window: Window,
    bricks: u32,
    words: u32,
    request_id: u64,
    mapping: Option<Arc<Mapping>>,
    receiver: mpsc::Receiver<Result<(), wgpu::BufferAsyncError>>,
}

/// Admitted staging storage, before the captured GPU work is submitted.
pub struct ReadbackReservation {
    device: wgpu::Device,
    queue: wgpu::Queue,
    window: Window,
    bricks: u32,
    words: u32,
    request_id: u64,
    mapping: Arc<Mapping>,
    sources: [(wgpu::Buffer, u64); 4],
}

impl ReadbackReservation {
    /// Capture all queue work submitted before this call.
    pub fn capture(self) -> PendingReadback {
        let mut encoder = self.device.create_command_encoder(&Default::default());
        let mut offset = 0;
        for (source, size) in &self.sources {
            encoder.copy_buffer_to_buffer(source, 0, &self.mapping.buffer, offset, *size);
            offset += size;
        }
        self.queue.submit([encoder.finish()]);
        let (sender, receiver) = mpsc::sync_channel(1);
        let callback_mapping = self.mapping.clone();
        self.mapping
            .buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let delivered = sender.try_send(result).is_ok();
                if !delivered || callback_mapping.abandoned.load(Ordering::Acquire) {
                    callback_mapping.unmap();
                }
            });
        PendingReadback {
            device: self.device,
            window: self.window,
            bricks: self.bricks,
            words: self.words,
            request_id: self.request_id,
            mapping: Some(self.mapping),
            receiver,
        }
    }
}

impl PendingReadback {
    fn abandon(&mut self) {
        if let Some(mapping) = self.mapping.take() {
            mapping.abandoned.store(true, Ordering::Release);
            if self.receiver.try_recv().is_ok() {
                mapping.unmap();
            }
        }
    }
    pub fn request_id(&self) -> u64 {
        self.request_id
    }

    /// Poll mapping progress without waiting for device completion.
    pub fn poll(&mut self) -> Poll<Result<Snapshot, GpuStepError>> {
        let Some(mapping) = &self.mapping else {
            return Poll::Ready(Err(GpuStepError::ReadbackConsumed));
        };
        if let Err(error) = self.device.poll(wgpu::PollType::Poll) {
            self.abandon();
            return Poll::Ready(Err(GpuStepError::Poll(error)));
        }
        let result = match self.receiver.try_recv() {
            Ok(result) => result.map_err(GpuStepError::Map),
            Err(mpsc::TryRecvError::Empty) => return Poll::Pending,
            Err(mpsc::TryRecvError::Disconnected) => Err(GpuStepError::Lost),
        };
        let result = result.and_then(|()| {
            let mapped = mapping.buffer.slice(..).get_mapped_range().map_err(GpuStepError::MapRange)?;
            Snapshot::decode(
                bytemuck::cast_slice(&mapped),
                self.window,
                self.bricks,
                self.words,
            )
        });
        mapping.unmap();
        self.mapping = None;
        Poll::Ready(result)
    }
}

impl Drop for PendingReadback {
    fn drop(&mut self) {
        self.abandon();
    }
}

impl GpuWorld {
    /// Total pending staging bytes, including abandoned requests awaiting completion.
    pub fn pending_readback_bytes(&self) -> u64 {
        self.readback_budget
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .1
    }

    /// Capture resident state under an aggregate staging-byte admission limit.
    pub fn begin_read_back(
        &self,
        request_id: u64,
        byte_budget: u64,
    ) -> Result<PendingReadback, ReadbackStartError> {
        Ok(self.reserve_read_back(request_id, byte_budget)?.capture())
    }

    /// Admit staging before changing a world that must be read back.
    pub fn reserve_read_back(
        &self,
        request_id: u64,
        byte_budget: u64,
    ) -> Result<ReadbackReservation, ReadbackStartError> {
        let sources = [
            (&self.directory, u64::from(self.bricks) * 4),
            (&self.bits, u64::from(self.words) * 4),
            (&self.alloc, u64::from(ALLOC_HEADER) * 4),
            (&self.pool, self.pool.size()),
        ];
        let bytes: u64 = sources.iter().map(|s| s.1).sum();
        let limit = self.device.limits().max_buffer_size;
        if bytes > limit {
            return Err(ReadbackStartError::DeviceLimit {
                needed: bytes,
                limit,
            });
        }
        let mut state = self
            .readback_budget
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if state.0 >= MAX_REQUESTS {
            return Err(ReadbackStartError::Busy);
        }
        let needed = state.1.saturating_add(bytes);
        if needed > byte_budget {
            return Err(ReadbackStartError::ByteBudget {
                needed,
                budget: byte_budget,
            });
        }
        state.0 += 1;
        state.1 = needed;
        drop(state);
        let lease = Lease {
            ledger: self.readback_budget.clone(),
            bytes,
        };
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ca asynchronous readback"),
            size: bytes,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mapping = Arc::new(Mapping {
            buffer,
            _lease: lease,
            abandoned: AtomicBool::new(false),
            unmapped: AtomicBool::new(false),
        });
        Ok(ReadbackReservation {
            device: self.device.clone(),
            queue: self.queue.clone(),
            window: self.window,
            bricks: self.bricks,
            words: self.words,
            request_id,
            mapping,
            sources: sources.map(|(source, bytes)| (source.clone(), bytes)),
        })
    }
}
