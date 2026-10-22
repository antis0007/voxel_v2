//! Opt-in renderer captures for visual review independent of desktop focus.
use std::{
    path::PathBuf,
    sync::{mpsc, OnceLock},
    time::{Duration, Instant},
};

#[path = "capture_video.rs"]
mod video;
pub(super) const MAX_PENDING_READBACKS: usize = 2;

struct EncodeJob {
    path: PathBuf,
    rgba: Vec<u8>,
    width: u32,
    height: u32,
}

enum WriterMessage {
    Frame(EncodeJob),
    Flush(mpsc::Sender<bool>, Instant),
}

// Two waiting RGBA frames (~7 MiB at 720p), plus the one being encoded.
// Video retains one previous real frame; key PNGs use one temporary copy.
// Slow storage never creates an unbounded queue or blocks the render thread.
static WRITER: OnceLock<Option<mpsc::SyncSender<WriterMessage>>> = OnceLock::new();

fn enqueue(job: EncodeJob) {
    let writer = WRITER.get_or_init(|| {
        let (sender, receiver) = mpsc::sync_channel(2);
        match std::thread::Builder::new()
            .name("capture-writer".into())
            .spawn(move || {
                let mut video = video::VideoWriter::from_env();
                while let Ok(message) = receiver.recv() {
                    match message {
                        WriterMessage::Frame(job) => {
                            if video::bench_index(&job.path).is_some() {
                                if let Some(video) = &mut video {
                                    video.frame(job);
                                } else {
                                    write_png(job);
                                }
                            } else {
                                write_png(job);
                            }
                        }
                        WriterMessage::Flush(done, deadline) => {
                            let success = video.as_mut().is_none_or(|v| v.finish(deadline));
                            let _ = done.send(success);
                        }
                    }
                }
            }) {
            Ok(_) => Some(sender),
            Err(error) => {
                eprintln!("[capture] cannot start PNG writer: {error}");
                None
            }
        }
    });
    let Some(writer) = writer else {
        eprintln!(
            "[capture] dropped {}: PNG writer unavailable",
            job.path.display()
        );
        return;
    };
    match writer.try_send(WriterMessage::Frame(job)) {
        Ok(()) => {}
        Err(mpsc::TrySendError::Full(WriterMessage::Frame(job))) => {
            eprintln!(
                "[capture] dropped {}: PNG queue full (capacity=2)",
                job.path.display()
            );
        }
        Err(mpsc::TrySendError::Disconnected(WriterMessage::Frame(job))) => {
            eprintln!(
                "[capture] dropped {}: PNG writer disconnected",
                job.path.display()
            );
        }
        _ => unreachable!("enqueue sends frames only"),
    }
}

fn write_png(job: EncodeJob) {
    let result = (|| -> anyhow::Result<()> {
        if let Some(parent) = job.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        image::save_buffer_with_format(
            &job.path,
            &job.rgba,
            job.width,
            job.height,
            image::ColorType::Rgba8,
            image::ImageFormat::Png,
        )?;
        Ok(())
    })();
    match result {
        Ok(()) => println!("[capture] saved {}", job.path.display()),
        Err(error) => eprintln!("[capture] write failed {}: {error}", job.path.display()),
    }
}

/// Flush already-enqueued frames at a review boundary, never during drawing.
/// The FIFO barrier includes the current encode and waits at most two seconds.
/// GPU readbacks not yet handed to this writer are outside this barrier.
pub(crate) fn flush(timeout: Duration) -> bool {
    let writer = match WRITER.get() {
        None => {
            video::abort("flush before capture writer started");
            return true;
        }
        Some(Some(writer)) => writer,
        Some(None) => {
            eprintln!("[capture] flush failed: PNG writer unavailable");
            return false;
        }
    };
    let deadline = Instant::now() + timeout.min(Duration::from_secs(2));
    let (done, received) = mpsc::channel();
    let mut message = WriterMessage::Flush(done, deadline);
    loop {
        match writer.try_send(message) {
            Ok(()) => break,
            Err(mpsc::TrySendError::Full(returned)) => {
                if Instant::now() >= deadline {
                    eprintln!("[capture] flush timed out: queued frames may be unfinished");
                    video::abort("capture writer queue flush timed out");
                    return false;
                }
                message = returned;
                std::thread::sleep(Duration::from_millis(1));
            }
            Err(mpsc::TrySendError::Disconnected(_)) => {
                eprintln!("[capture] flush failed: PNG writer disconnected");
                video::abort("capture writer disconnected during flush");
                return false;
            }
        }
    }
    match received.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
        Ok(success) => success,
        Err(_) => {
            eprintln!("[capture] flush timed out: queued frames may be unfinished");
            video::abort("capture writer finalization barrier timed out");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_writer_flush_preserves_pixels() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("voxel-capture-{}-{nonce}.png", std::process::id()));
        let pixels = vec![255, 10, 20, 255, 30, 40, 200, 255];
        enqueue(EncodeJob {
            path: path.clone(),
            rgba: pixels.clone(),
            width: 2,
            height: 1,
        });
        assert!(
            flush(Duration::from_secs(2)),
            "writer did not drain before the deadline"
        );
        let decoded = image::open(&path).unwrap().to_rgba8();
        assert_eq!(decoded.dimensions(), (2, 1));
        assert_eq!(decoded.into_raw(), pixels);
        std::fs::remove_file(path).unwrap();
    }
}

pub(super) struct PendingCapture {
    buffer: wgpu::Buffer,
    path: PathBuf,
    width: u32,
    height: u32,
    padded_row: u32,
    bgra: bool,
    sender: Option<mpsc::Sender<Result<(), wgpu::BufferAsyncError>>>,
    receiver: mpsc::Receiver<Result<(), wgpu::BufferAsyncError>>,
}

impl PendingCapture {
    pub fn encode(
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        texture: &wgpu::Texture,
        config: &wgpu::SurfaceConfiguration,
        path: PathBuf,
    ) -> Option<Self> {
        if !config.usage.contains(wgpu::TextureUsages::COPY_SRC) {
            eprintln!("[capture] surface readback is unavailable");
            return None;
        }
        let bgra = match config.format {
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb => true,
            wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Rgba8UnormSrgb => false,
            _ => {
                eprintln!("[capture] unsupported surface format");
                return None;
            }
        };
        let padded_row = (config.width * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("visual review readback"),
            size: padded_row as u64 * config.height as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_row),
                    rows_per_image: Some(config.height),
                },
            },
            wgpu::Extent3d {
                width: config.width,
                height: config.height,
                depth_or_array_layers: 1,
            },
        );
        let (sender, receiver) = mpsc::channel();
        Some(Self {
            buffer,
            path,
            width: config.width,
            height: config.height,
            padded_row,
            bgra,
            sender: Some(sender),
            receiver,
        })
    }

    pub fn map(&mut self) {
        let sender = self.sender.take().expect("capture mapped once");
        self.buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = sender.send(result);
            });
    }

    pub fn finish_if_ready(&self) -> bool {
        match self.receiver.try_recv() {
            Err(mpsc::TryRecvError::Empty) => return false,
            Ok(Ok(())) => {}
            _ => {
                eprintln!("[capture] readback failed: {}", self.path.display());
                return true;
            }
        }
        let mapped = self.buffer.slice(..).get_mapped_range();
        let mut rgba = Vec::with_capacity((self.width * self.height * 4) as usize);
        for row in mapped.chunks_exact(self.padded_row as usize) {
            for pixel in row[..self.width as usize * 4].chunks_exact(4) {
                let rgb = if self.bgra {
                    [pixel[2], pixel[1], pixel[0]]
                } else {
                    [pixel[0], pixel[1], pixel[2]]
                };
                rgba.extend_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
            }
        }
        drop(mapped);
        self.buffer.unmap();
        enqueue(EncodeJob {
            path: self.path.clone(),
            rgba,
            width: self.width,
            height: self.height,
        });
        true
    }
}
