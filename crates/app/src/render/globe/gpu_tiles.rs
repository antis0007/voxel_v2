//! Atlas slots for resident globe tiles: upload and eviction.

use super::bake::TileData;
use super::tile::{TILE_SAMPLES, TileId};
use glam::DVec3;
use std::collections::HashMap;

/// Slots per atlas row; 46 x 46 = 2116 slots cover `MAX_RESIDENT`.
pub const ATLAS_COLUMNS: u32 = 46;
pub const MAX_RESIDENT: usize = 2048;
const ATLAS_TEXELS: u32 = ATLAS_COLUMNS * TILE_SAMPLES;
/// Heights are stored unsigned so the atlas can be read with `textureLoad` on `u32`.
pub const HEIGHT_BIAS: i32 = 32_768;
/// Bit of the material channel that marks a water sample.
pub const WATER_BIT: u16 = 1 << 8;
/// Free slots kept even when nothing is incoming, so new tiles always land.
pub const MIN_FREE: usize = 64;

/// One atlas shared by every tile; a texture array would hit the 256-layer limit.
pub struct TilePool {
    pub height_view: wgpu::TextureView,
    pub climate_view: wgpu::TextureView,
    height: wgpu::Texture,
    climate: wgpu::Texture,
    slots: HashMap<TileId, u32>,
    free: Vec<u32>,
    // Reused every frame so the steady state does not allocate.
    evict_scratch: Vec<(f64, TileId)>,
    height_scratch: Vec<u16>,
    climate_scratch: Vec<u8>,
}

impl TilePool {
    pub fn new(device: &wgpu::Device) -> TilePool {
        let atlas = |label, format| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d { width: ATLAS_TEXELS, height: ATLAS_TEXELS, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            })
        };
        let height = atlas("globe height atlas", wgpu::TextureFormat::Rg16Uint);
        let climate = atlas("globe climate atlas", wgpu::TextureFormat::Rg8Unorm);
        TilePool {
            height_view: height.create_view(&Default::default()),
            climate_view: climate.create_view(&Default::default()),
            height,
            climate,
            slots: HashMap::new(),
            // Popped from the back, so slot 0 is used first.
            free: (0..MAX_RESIDENT as u32).rev().collect(),
            evict_scratch: Vec::new(),
            height_scratch: Vec::with_capacity((TILE_SAMPLES * TILE_SAMPLES * 2) as usize),
            climate_scratch: Vec::with_capacity((TILE_SAMPLES * TILE_SAMPLES * 2) as usize),
        }
    }

    pub fn slot(&self, id: TileId) -> Option<u32> {
        self.slots.get(&id).copied()
    }

    pub fn resident(&self) -> usize {
        self.slots.len()
    }

    /// Writes a tile into its slot; returns false when the pool is full and the tile is dropped.
    pub fn upload(&mut self, queue: &wgpu::Queue, tile: &TileData) -> bool {
        let Some(slot) = self.slots.get(&tile.id).copied().or_else(|| self.free.pop()) else {
            return false;
        };
        self.slots.insert(tile.id, slot);
        self.height_scratch.clear();
        for ((h, m), w) in tile.height_m.iter().zip(&tile.material).zip(&tile.water) {
            let flags = u16::from(*m) | if *w != 0 { WATER_BIT } else { 0 };
            self.height_scratch.extend_from_slice(&[(i32::from(*h) + HEIGHT_BIAS) as u16, flags]);
        }
        self.climate_scratch.clear();
        self.climate_scratch.extend(tile.climate.iter().flatten());
        write(queue, &self.height, slot, bytemuck::cast_slice(&self.height_scratch), 4);
        write(queue, &self.climate, slot, &self.climate_scratch, 2);
        true
    }

    /// Frees tiles outside sorted `keep`, farthest from the eye first, until at most `max_resident`
    /// remain and `incoming.max(MIN_FREE)` slots are free.
    pub fn evict_unused(&mut self, keep: &[TileId], max_resident: usize, incoming: usize, eye_body: DVec3) {
        let want_free = incoming.max(MIN_FREE).min(MAX_RESIDENT);
        let done = |pool: &TilePool| pool.slots.len() <= max_resident && pool.free.len() >= want_free;
        if done(self) {
            return;
        }
        super::scheduler::eviction_order(self.slots.keys().copied(), keep, eye_body, &mut self.evict_scratch);
        for i in 0..self.evict_scratch.len() {
            if done(self) {
                break;
            }
            if let Some(slot) = self.slots.remove(&self.evict_scratch[i].1) {
                self.free.push(slot);
            }
        }
    }

    #[cfg(test)]
    pub fn free_slots(&self) -> usize {
        self.free.len()
    }
}

fn write(queue: &wgpu::Queue, texture: &wgpu::Texture, slot: u32, bytes: &[u8], texel_bytes: u32) {
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d {
                x: (slot % ATLAS_COLUMNS) * TILE_SAMPLES,
                y: (slot / ATLAS_COLUMNS) * TILE_SAMPLES,
                z: 0,
            },
            aspect: wgpu::TextureAspect::All,
        },
        bytes,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(TILE_SAMPLES * texel_bytes),
            rows_per_image: Some(TILE_SAMPLES),
        },
        wgpu::Extent3d { width: TILE_SAMPLES, height: TILE_SAMPLES, depth_or_array_layers: 1 },
    );
}
