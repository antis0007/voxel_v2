//! Opt-in renderer captures for visual review independent of desktop focus.
use std::{path::PathBuf, sync::mpsc};

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
        let result = (|| -> anyhow::Result<()> {
            if let Some(parent) = self.path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            image::save_buffer_with_format(
                &self.path,
                &rgba,
                self.width,
                self.height,
                image::ColorType::Rgba8,
                image::ImageFormat::Png,
            )?;
            Ok(())
        })();
        match result {
            Ok(()) => println!("[capture] saved {}", self.path.display()),
            Err(error) => eprintln!("[capture] {}: {error}", self.path.display()),
        }
        true
    }
}
