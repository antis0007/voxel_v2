//! Atlas slots for resident globe tiles: upload and eviction.

use super::bake::TileData;
use super::tile::{TILE_SAMPLES, TileId};
use std::collections::HashMap;

/// Slots per atlas row; 46 x 46 = 2116 slots cover `MAX_RESIDENT`.
pub const ATLAS_COLUMNS: u32 = 46;
pub const MAX_RESIDENT: usize = 2048;
const ATLAS_TEXELS: u32 = ATLAS_COLUMNS * TILE_SAMPLES;
/// Heights are stored unsigned so the atlas can be read with `textureLoad` on `u32`.
pub const HEIGHT_BIAS: i32 = 32_768;
/// Bit of the material channel that marks a water sample.
pub const WATER_BIT: u16 = 1 << 8;

/// One atlas shared by every tile; a texture array would hit the 256-layer limit.
pub struct TilePool {
    pub height_view: wgpu::TextureView,
    pub climate_view: wgpu::TextureView,
    height: wgpu::Texture,
    climate: wgpu::Texture,
    slots: HashMap<TileId, u32>,
    free: Vec<u32>,
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
        }
    }

    pub fn slot(&self, id: TileId) -> Option<u32> {
        self.slots.get(&id).copied()
    }

    pub fn resident(&self) -> usize {
        self.slots.len()
    }

    /// Writes a tile into its slot; a full pool drops the tile until eviction frees one.
    pub fn upload(&mut self, queue: &wgpu::Queue, tile: &TileData) {
        let Some(slot) = self.slots.get(&tile.id).copied().or_else(|| self.free.pop()) else {
            return;
        };
        self.slots.insert(tile.id, slot);
        let height: Vec<u16> = tile
            .height_m
            .iter()
            .zip(&tile.material)
            .zip(&tile.water)
            .flat_map(|((h, m), w)| {
                let flags = u16::from(*m) | if *w != 0 { WATER_BIT } else { 0 };
                [(i32::from(*h) + HEIGHT_BIAS) as u16, flags]
            })
            .collect();
        let climate: Vec<u8> = tile.climate.iter().flatten().copied().collect();
        write(queue, &self.height, slot, bytemuck::cast_slice(&height), 4);
        write(queue, &self.climate, slot, &climate, 2);
    }

    /// Frees tiles outside `keep` until at most `max_resident` remain.
    pub fn evict_unused(&mut self, keep: &[TileId], max_resident: usize) {
        if self.slots.len() <= max_resident {
            return;
        }
        let keep: std::collections::HashSet<TileId> = keep.iter().copied().collect();
        // Finest first: coarse parents are the fallback when children are missing.
        let mut candidates: Vec<TileId> = self.slots.keys().filter(|t| !keep.contains(t)).copied().collect();
        candidates.sort_by_key(|t| std::cmp::Reverse(t.level));
        for id in candidates {
            if self.slots.len() <= max_resident {
                break;
            }
            if let Some(slot) = self.slots.remove(&id) {
                self.free.push(slot);
            }
        }
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
