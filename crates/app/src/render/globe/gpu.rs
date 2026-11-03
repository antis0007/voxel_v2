//! Globe tile pipeline: uniforms, per-tile instances and the draw.

use super::bake::TileData;
use super::chart;
use super::gpu_tiles::{MAX_RESIDENT, TilePool, grid_indices, instance};
use super::scheduler::Selection;
use super::shell::Light;
use super::shell_gpu::{self, SHELL_FACES, ShellResources};
use super::shell_worker::ShellBake;
use super::band::GlobeMask;
use super::tile::{TILE_CELLS, TILE_SAMPLES, TileId};
use glam::{DVec3, Mat3, Mat4, Vec3};

/// Tile vertex grid edge: the 65x65 lattice plus a skirt ring (`GLOBE_SKIRT_SIDE`).
pub const SKIRT_SIDE: u32 = TILE_SAMPLES + 2;
pub const TILE_INDEX_COUNT: u32 = (SKIRT_SIDE - 1) * (SKIRT_SIDE - 1) * 6;
/// The shell has no skirt; it draws the bare lattice.
pub const SHELL_INDEX_COUNT: u32 = TILE_CELLS * TILE_CELLS * 6;

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GlobeUniform {
    /// Absolute eye in body space; only lighting and the air path read it. `w` is the night-light count.
    pub eye_body: [f32; 4],
    pub sun_dir_body: [f32; 4],
    pub radius_m: f32,
    pub sea_level_m: f32,
    pub focal_px: f32,
    /// Bit 0 lean tier, bit 1 cloud shadow on, bit 2 night lights on.
    pub flags: u32,
    /// Glow colour rgb, strength.
    pub limb: [f32; 4],
    /// Camera-relative body-space view-projection with reverse-Z.
    pub view_proj: [[f32; 4]; 4],
}
const _: [(); 128] = [(); std::mem::size_of::<GlobeUniform>()];

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TileInstance {
    /// Tile centre minus eye, f32 after the f64 subtraction.
    pub origin_body: [f32; 4],
    pub chart_min: [f32; 2],
    pub cell_m: f32,
    pub face: u32,
    /// Face-local chart point of the tile centre, folded at height 0 by the shader.
    pub centre: [f32; 2],
    pub slot: u32,
    pub pad: u32,
}
const _: [(); 48] = [(); std::mem::size_of::<TileInstance>()];

/// Eye in body space and the body-space view-projection for a chart-space camera.
///
/// `projection` is the reverse-Z projection the terrain uses. The chart frame maps the
/// camera's east/up/north axes to body space, so the camera keeps its local heading.
pub fn body_view_proj(eye_chart_m: DVec3, forward: Vec3, projection: Mat4) -> (DVec3, Mat4) {
    let frame = chart::frame(eye_chart_m);
    let to_body = Mat3::from_cols(frame.x_axis.as_vec3(), frame.y_axis.as_vec3(), frame.z_axis.as_vec3());
    let forward_body = (to_body * forward).normalize();
    let up_body = to_body * Vec3::Y;
    let view = Mat4::look_to_rh(Vec3::ZERO, forward_body, up_body);
    (chart::to_body(eye_chart_m), projection * view)
}

pub struct Globe {
    pipeline: wgpu::RenderPipeline,
    shell_pipeline: wgpu::RenderPipeline,
    shell: ShellResources,
    /// Faces baked at least once; the shell draws only when all six are.
    shell_faces: u8,
    bind_group: wgpu::BindGroup,
    uniform: wgpu::Buffer,
    mask: wgpu::Buffer,
    instances: wgpu::Buffer,
    indices: wgpu::Buffer,
    shell_indices: wgpu::Buffer,
    tiles: TilePool,
    draw_count: u32,
    instance_scratch: Vec<TileInstance>,
}

/// Per-frame globe counters for the render stats.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameStats {
    pub uploaded: usize,
    pub dropped: usize,
    pub drawn: usize,
    pub resident: usize,
}

impl Globe {
    /// Builds the pipeline for the terrain pass's two `color_format` targets and reverse-Z depth.
    pub fn new(device: &wgpu::Device, color_format: wgpu::TextureFormat) -> Globe {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("globe shader"),
            source: wgpu::ShaderSource::Wgsl(crate::shader_sources::globe_shader_source().into()),
        });
        let layout = shell_gpu::bind_group_layout(device);
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("globe pipeline layout"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let pipeline = shell_gpu::pipeline(device, &pipeline_layout, &shader, color_format, false);
        let shell_pipeline = shell_gpu::pipeline(device, &pipeline_layout, &shader, color_format, true);
        let shell = ShellResources::new(device, super::shell::shell_resolution());
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globe uniform"),
            size: std::mem::size_of::<GlobeUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let instances = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globe tile instances"),
            // The six shell roots follow the tile instances.
            size: ((MAX_RESIDENT + SHELL_FACES as usize) * std::mem::size_of::<TileInstance>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let index_buffer = |label, side| {
            wgpu::util::DeviceExt::create_buffer_init(device, &wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: bytemuck::cast_slice(&grid_indices(side)),
                usage: wgpu::BufferUsages::INDEX,
            })
        };
        let indices = index_buffer("globe grid indices", SKIRT_SIDE);
        let shell_indices = index_buffer("globe shell indices", TILE_SAMPLES);
        // Zeroed: no resident map and no fade, so the globe draws everywhere until `set_mask`.
        let mask = wgpu::util::DeviceExt::create_buffer_init(device, &wgpu::util::BufferInitDescriptor {
            label: Some("globe resident mask"),
            contents: bytemuck::bytes_of(&<GlobeMask as bytemuck::Zeroable>::zeroed()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let tiles = TilePool::new(device);
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("globe bind group"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: uniform.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: instances.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&tiles.height_view) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&tiles.climate_view) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(&shell.view) },
                wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::Sampler(&shell.sampler) },
                wgpu::BindGroupEntry { binding: 6, resource: shell.lights.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 7, resource: mask.as_entire_binding() },
            ],
        });
        Globe { pipeline, shell_pipeline, shell, shell_faces: 0, bind_group, uniform, mask, instances, indices, shell_indices, tiles, draw_count: 0, instance_scratch: Vec::with_capacity(MAX_RESIDENT + SHELL_FACES as usize) }
    }

    /// The only production entry point: evict, upload, then prepare, in that order.
    ///
    /// `keep` is sorted (see `scheduler::keep_set`) and protects tiles from eviction.
    /// A slot freed and refilled after `prepare` would make an instance written this
    /// frame draw another tile's data.
    pub fn frame(
        &mut self,
        queue: &wgpu::Queue,
        incoming: Vec<TileData>,
        selection: &Selection,
        keep: &[TileId],
        eye_body: DVec3,
        uniform: GlobeUniform,
    ) -> FrameStats {
        self.evict_unused(keep, MAX_RESIDENT, incoming.len(), eye_body);
        let dropped = self.upload(queue, &incoming);
        self.prepare(queue, selection, eye_body, uniform);
        FrameStats {
            uploaded: incoming.len() - dropped,
            dropped,
            drawn: self.draw_count as usize,
            resident: self.tiles.resident(),
        }
    }

    /// Returns the number of tiles dropped because the pool was full.
    pub(crate) fn upload(&mut self, queue: &wgpu::Queue, tiles: &[TileData]) -> usize {
        tiles.iter().filter(|tile| !self.tiles.upload(queue, tile)).count()
    }

    pub(crate) fn evict_unused(&mut self, keep: &[TileId], max_resident: usize, incoming: usize, eye_body: DVec3) {
        self.tiles.evict_unused(keep, max_resident, incoming, eye_body);
    }

    /// Writes the uniform and one instance per resident tile of `selection.draw`.
    pub(crate) fn prepare(&mut self, queue: &wgpu::Queue, selection: &Selection, eye_body: DVec3, uniform: GlobeUniform) {
        self.instance_scratch.clear();
        for id in &selection.draw {
            if self.instance_scratch.len() == MAX_RESIDENT {
                break;
            }
            if let Some(slot) = self.tiles.slot(*id) {
                self.instance_scratch.push(instance(*id, slot, eye_body));
            }
        }
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&uniform));
        let count = self.instance_scratch.len();
        self.instance_scratch.extend((0..SHELL_FACES as u8).map(|face| instance(TileId::root(face), 0, eye_body)));
        if count > 0 {
            queue.write_buffer(&self.instances, 0, bytemuck::cast_slice(&self.instance_scratch[..count]));
        }
        let shell_offset = (MAX_RESIDENT * std::mem::size_of::<TileInstance>()) as u64;
        queue.write_buffer(&self.instances, shell_offset, bytemuck::cast_slice(&self.instance_scratch[count..]));
        self.instance_scratch.truncate(count);
        self.draw_count = count as u32;
    }

    /// Writes the resident-chunk mask the opaque tiles discard under.
    pub fn set_mask(&self, queue: &wgpu::Queue, mask: &GlobeMask) {
        queue.write_buffer(&self.mask, 0, bytemuck::bytes_of(mask));
    }

    /// Uploads finished shell faces; a face from a stale resolution is ignored.
    pub fn upload_shell(&mut self, queue: &wgpu::Queue, bakes: &[ShellBake]) {
        for bake in bakes {
            if self.shell.upload_face(queue, bake) {
                self.shell_faces |= 1 << bake.face;
            }
        }
    }

    /// Returns the light count for `GlobeUniform.eye_body[3]`.
    pub fn upload_lights(&self, queue: &wgpu::Queue, lights: &[Light]) -> u32 {
        self.shell.upload_lights(queue, lights)
    }

    pub fn shell_resolution(&self) -> u32 {
        self.shell.resolution()
    }

    pub fn shell_ready(&self) -> bool {
        self.shell_faces == 0b11_1111
    }

    #[cfg(test)]
    pub fn instance_capacity(&self) -> usize {
        self.instance_scratch.capacity()
    }

    #[cfg(test)]
    pub fn free_slots(&self) -> usize {
        self.tiles.free_slots()
    }

    /// Replaces the caller's group 0 and pipeline; the caller rebinds its own afterwards.
    pub fn draw<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        if self.draw_count == 0 {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint16);
        pass.draw_indexed(0..TILE_INDEX_COUNT, 0, 0..self.draw_count);
        if self.shell_ready() {
            let first = MAX_RESIDENT as u32;
            pass.set_pipeline(&self.shell_pipeline);
            pass.set_index_buffer(self.shell_indices.slice(..), wgpu::IndexFormat::Uint16);
            pass.draw_indexed(0..SHELL_INDEX_COUNT, 0, first..first + SHELL_FACES);
        }
    }

    pub fn resident(&self) -> usize {
        self.tiles.resident()
    }

    pub fn resident_contains(&self, id: TileId) -> bool {
        self.tiles.slot(id).is_some()
    }
}
