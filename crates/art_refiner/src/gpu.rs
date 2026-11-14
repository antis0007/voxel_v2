//! Headless compute path: compile the composed shader, render the sheet, read it back.
use anyhow::{Context, Result, anyhow};
use std::borrow::Cow;
use wgpu::util::DeviceExt;

const CAMERA_BYTES: usize = 16384;
const MAX_TILES: usize = 128;
pub const BANDS: u32 = 4;
const TILE_BYTES: usize = 48;

pub struct Tile {
    pub kind: i32,
    pub block: u32,
    pub face: u32,
    /// Linear colour, as the vertex stage hands it to the surface functions.
    pub base: [f32; 3],
    pub rough: f32,
    pub refl: f32,
    pub metallic: f32,
}

pub struct Scene {
    pub tile_px: u32,
    pub cols: u32,
    pub tiles: Vec<Tile>,
    pub span_m: f32,
    pub fp_scale: f32,
    pub ambient: f32,
    pub sun: f32,
    pub origin: [f32; 3],
    pub light: [f32; 3],
    pub view: [f32; 3],
    pub seed: u32,
}

impl Scene {
    pub fn size(&self) -> (u32, u32) {
        let rows = (self.tiles.len() as u32).div_ceil(self.cols);
        (self.cols * self.tile_px, rows * self.tile_px * BANDS)
    }

    fn refine_bytes(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(80 + MAX_TILES * TILE_BYTES);
        for v in [self.tile_px, self.cols, self.tiles.len() as u32, 0] {
            b.extend(v.to_le_bytes());
        }
        for v in [self.span_m, self.fp_scale, self.ambient, self.sun] {
            b.extend(v.to_le_bytes());
        }
        for v in [self.origin[0], self.origin[1], self.origin[2], 0.0, self.light[0], self.light[1], self.light[2], 0.0] {
            b.extend(v.to_le_bytes());
        }
        for v in [self.view[0], self.view[1], self.view[2], 0.0] {
            b.extend(v.to_le_bytes());
        }
        let blank = Tile { kind: 0, block: 0, face: 0, base: [0.0; 3], rough: 1.0, refl: 0.5, metallic: 0.0 };
        for i in 0..MAX_TILES {
            let t = self.tiles.get(i).unwrap_or(&blank);
            b.extend(t.kind.to_le_bytes());
            b.extend(t.face.to_le_bytes());
            b.extend(t.rough.to_le_bytes());
            b.extend(t.refl.to_le_bytes());
            for v in [t.base[0], t.base[1], t.base[2], t.metallic] {
                b.extend(v.to_le_bytes());
            }
            for v in [t.block, 0, 0, 0] {
                b.extend(v.to_le_bytes());
            }
        }
        b
    }

    /// Camera uniform with only the fields the surface functions read.
    fn camera_bytes(&self) -> Vec<u8> {
        let mut b = vec![0u8; CAMERA_BYTES];
        b[144..148].copy_from_slice(&1.0f32.to_le_bytes()); // world_params.x: one metre voxels
        b[288..292].copy_from_slice(&self.seed.to_le_bytes()); // scene_seed.x
        b[304..308].copy_from_slice(&(-1.0f32).to_le_bytes()); // coverage.x: no override
        b
    }
}

pub struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pub adapter: String,
}

impl Gpu {
    pub fn new() -> Result<Self> {
        let backends = wgpu::Backends::from_env().unwrap_or(wgpu::Backends::VULKAN);
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor { backends, ..wgpu::InstanceDescriptor::new_without_display_handle() });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            ..Default::default()
        }))
        .context("no GPU adapter")?;
        Self::from_adapter(adapter)
    }

    pub(crate) fn from_adapter(adapter: wgpu::Adapter) -> Result<Self> {
        let info = adapter.get_info();
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("art_refiner"),
            ..Default::default()
        }))?;
        Ok(Self { device, queue, adapter: format!("{} / {:?}", info.name, info.backend) })
    }

    /// Compile errors come back as text so a watch loop can keep the last good pipeline.
    pub fn compile(&self, source: &str) -> Result<wgpu::ComputePipeline> {
        let validation_scope = self.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let module = self.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("refiner"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(source)),
        });
        let pipeline = self.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("refiner"),
            layout: None,
            module: &module,
            entry_point: Some("refine_main"),
            compilation_options: Default::default(),
            cache: None,
        });
        match pollster::block_on(validation_scope.pop()) {
            Some(error) => Err(anyhow!("{error}")),
            None => Ok(pipeline),
        }
    }

    /// RGBA8 pixels of the sheet, `scene.size()` wide and tall.
    pub fn render(&self, pipeline: &wgpu::ComputePipeline, scene: &Scene) -> Result<Vec<u8>> {
        let (w, h) = scene.size();
        let bytes = u64::from(w) * u64::from(h) * 4;
        let init = |label, data: &[u8], usage| {
            self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some(label), contents: data, usage })
        };
        let camera = init("camera", &scene.camera_bytes(), wgpu::BufferUsages::UNIFORM);
        let lighting = init("lighting", &[0u8; CAMERA_BYTES], wgpu::BufferUsages::UNIFORM);
        let refine = init("refine", &scene.refine_bytes(), wgpu::BufferUsages::UNIFORM);
        let buffer = |label, usage| {
            self.device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size: bytes, usage, mapped_at_creation: false })
        };
        let pixels = buffer("pixels", wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC);
        let readback = buffer("readback", wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST);
        let bind = |group: u32, entries: &[wgpu::BindGroupEntry]| {
            self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &pipeline.get_bind_group_layout(group),
                entries,
            })
        };
        let group0 = bind(0, &[wgpu::BindGroupEntry { binding: 0, resource: camera.as_entire_binding() }]);
        let group1 = bind(1, &[
            wgpu::BindGroupEntry { binding: 0, resource: lighting.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 60, resource: refine.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 61, resource: pixels.as_entire_binding() },
        ]);
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &group0, &[]);
            pass.set_bind_group(1, &group1, &[]);
            pass.dispatch_workgroups(w.div_ceil(8), h.div_ceil(8), 1);
        }
        encoder.copy_buffer_to_buffer(&pixels, 0, &readback, 0, bytes);
        self.queue.submit([encoder.finish()]);
        let slice = readback.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        self.device.poll(wgpu::PollType::wait_indefinitely())?;
        rx.recv()??;
        let data = slice.get_mapped_range()?.to_vec();
        readback.unmap();
        Ok(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn material_identity_matches_wgsl_tile_stride() {
        let scene = Scene {
            tile_px: 8, cols: 1,
            tiles: vec![Tile { kind: 12, block: 193, face: 2, base: [0.1, 0.2, 0.3],
                rough: 0.4, refl: 0.5, metallic: 0.6 }],
            span_m: 1.0, fp_scale: 1.0, ambient: 0.35, sun: 0.85,
            origin: [0.0; 3], light: [0.0, 1.0, 0.0], view: [0.0, 0.0, 1.0], seed: 0,
        };
        let bytes = scene.refine_bytes();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(bytes.len(), 80 + MAX_TILES * 48);
        assert_eq!(word(80), 12);
        assert_eq!(word(84), 2);
        assert_eq!(word(112), 193);
        assert_eq!(word(128), 0);
        assert_eq!(word(160), 0);
        assert_eq!(scene.size(), (8, 32));
        let camera = scene.camera_bytes();
        assert_eq!(&camera[144..148], &1.0f32.to_le_bytes());
    }
}
