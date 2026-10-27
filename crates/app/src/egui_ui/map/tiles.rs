//! Mapped chunk columns and the region textures the map widgets draw.
use super::columns::COLUMNS;
use egui::{Color32, ColorImage, TextureHandle, TextureOptions};
use glam::IVec2;
use std::collections::HashMap;
use std::sync::Arc;

pub const TILE: usize = 32;
/// Tiles per region side; one texture covers 8 x 8 chunk columns.
pub const REGION: i32 = 8;
pub const REGION_PX: usize = TILE * REGION as usize;
/// Mapped chunk columns kept in memory, about 6 KiB each.
pub const MAX_TILES: usize = 8192;
/// Source layer of a column restored from disk: any live chunk replaces it.
const SAVED: i16 = i16::MIN;

pub struct Tile {
    /// Shared with an in-flight sidecar write; edits copy on write.
    pub colour: Arc<Vec<Color32>>,
    /// Chunk layer that supplied each column; higher layers win.
    source: Vec<i16>,
}

impl Tile {
    fn empty() -> Self {
        Self { colour: Arc::new(vec![Color32::TRANSPARENT; COLUMNS]), source: vec![SAVED; COLUMNS] }
    }
}

#[derive(Default)]
struct Region {
    texture: Option<TextureHandle>,
    /// One bit per tile awaiting upload, bit = tz * REGION + tx.
    dirty: u64,
}

#[derive(Default)]
pub struct TileStore {
    tiles: HashMap<IVec2, Tile>,
    regions: HashMap<IVec2, Region>,
    /// Columns changed since the last sidecar write.
    pub unsaved: bool,
}

pub fn region_of(tile: IVec2) -> IVec2 {
    IVec2::new(engine_core::floor_div(tile.x, REGION), engine_core::floor_div(tile.y, REGION))
}

fn bit(tile: IVec2) -> u64 {
    let local = tile - region_of(tile) * REGION;
    1 << (local.y * REGION + local.x)
}

impl TileStore {
    pub fn len(&self) -> usize {
        self.tiles.len()
    }

    pub fn is_mapped(&self, tile: IVec2) -> bool {
        self.tiles.contains_key(&tile)
    }

    /// Cheap copy of every tile's colours for a background sidecar write.
    pub fn snapshot(&self) -> Vec<(IVec2, Arc<Vec<Color32>>)> {
        self.tiles.iter().map(|(k, t)| (*k, t.colour.clone())).collect()
    }

    fn mark(&mut self, tile: IVec2) {
        self.regions.entry(region_of(tile)).or_default().dirty |= bit(tile);
    }

    /// Merge one chunk's columns from layer `cy`; true when a pixel changed.
    pub fn apply(&mut self, tile: IVec2, cy: i32, columns: &[Option<Color32>]) -> bool {
        let layer = cy.clamp(i16::MIN as i32 + 1, i16::MAX as i32) as i16;
        let fresh = !self.tiles.contains_key(&tile);
        if fresh && columns.iter().all(Option::is_none) {
            return false;
        }
        let entry = self.tiles.entry(tile).or_insert_with(Tile::empty);
        let (colour, source) = (Arc::make_mut(&mut entry.colour), &mut entry.source);
        let mut changed = fresh;
        for (i, column) in columns.iter().enumerate().take(COLUMNS) {
            match column {
                Some(c) if layer >= source[i] => {
                    changed |= colour[i] != *c;
                    colour[i] = *c;
                    source[i] = layer;
                }
                // The surface left this layer; let the layer below replace it.
                None if source[i] == layer => source[i] = layer - 1,
                _ => {}
            }
        }
        if changed {
            self.mark(tile);
            self.unsaved = true;
        }
        changed
    }

    pub fn insert_saved(&mut self, tile: IVec2, colour: Vec<Color32>) {
        if colour.len() == COLUMNS {
            self.tiles.insert(tile, Tile { colour: Arc::new(colour), source: vec![SAVED; COLUMNS] });
            self.mark(tile);
        }
    }

    /// Drop whole regions farthest from `centre` until under `limit` tiles.
    pub fn evict_far(&mut self, centre: IVec2, limit: usize) {
        if self.tiles.len() <= limit {
            return;
        }
        let home = region_of(centre);
        let mut order: Vec<IVec2> = self.tiles.keys().map(|t| region_of(*t)).collect();
        order.sort_unstable_by_key(|r| std::cmp::Reverse(((*r - home).length_squared(), r.x, r.y)));
        order.dedup();
        for region in order {
            if self.tiles.len() <= limit || region == home {
                break;
            }
            self.tiles.retain(|t, _| region_of(*t) != region);
            self.regions.remove(&region);
            self.unsaved = true;
        }
    }

    fn region_image(&self, region: IVec2, mask: u64) -> ColorImage {
        let mut image = ColorImage::filled([REGION_PX, REGION_PX], Color32::TRANSPARENT);
        for b in 0..64 {
            if mask & (1 << b) == 0 {
                continue;
            }
            let local = IVec2::new(b % REGION, b / REGION);
            if let Some(tile) = self.tiles.get(&(region * REGION + local)) {
                for row in 0..TILE {
                    let at = (local.y as usize * TILE + row) * REGION_PX + local.x as usize * TILE;
                    image.pixels[at..at + TILE].copy_from_slice(&tile.colour[row * TILE..(row + 1) * TILE]);
                }
            }
        }
        image
    }

    /// Upload at most `limit` changed tiles; new regions upload whole.
    pub fn upload(&mut self, ctx: &egui::Context, mut limit: usize) {
        let pending: Vec<IVec2> = self.regions.iter().filter(|(_, r)| r.dirty != 0).map(|(k, _)| *k).collect();
        for key in pending {
            if limit == 0 {
                return;
            }
            let Some(region) = self.regions.get(&key) else { continue };
            if region.texture.is_none() {
                let image = self.region_image(key, u64::MAX);
                let name = format!("map-region-{}-{}", key.x, key.y);
                let handle = ctx.load_texture(name, image, TextureOptions::NEAREST);
                if let Some(region) = self.regions.get_mut(&key) {
                    region.texture = Some(handle);
                    region.dirty = 0;
                }
                limit -= 1;
                continue;
            }
            let mut dirty = region.dirty;
            while dirty != 0 && limit > 0 {
                let b = dirty.trailing_zeros() as i32;
                dirty &= dirty - 1;
                limit -= 1;
                let local = IVec2::new(b % REGION, b / REGION);
                let Some(tile) = self.tiles.get(&(key * REGION + local)) else { continue };
                let image = ColorImage::new([TILE, TILE], tile.colour.to_vec());
                if let Some(texture) = self.regions.get_mut(&key).and_then(|r| r.texture.as_mut()) {
                    let at = [local.x as usize * TILE, local.y as usize * TILE];
                    texture.set_partial(at, image, TextureOptions::NEAREST);
                }
            }
            if let Some(region) = self.regions.get_mut(&key) {
                region.dirty = dirty;
            }
        }
    }

    /// Uploaded region textures whose squares overlap `[min, max]` in tiles.
    pub fn textures_in(&self, min: IVec2, max: IVec2) -> Vec<(IVec2, egui::TextureId)> {
        let (lo, hi) = (region_of(min), region_of(max));
        let mut out = Vec::new();
        for (key, region) in &self.regions {
            if key.cmpge(lo).all() && key.cmple(hi).all()
                && let Some(texture) = &region.texture
            {
                out.push((*key, texture.id()));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(c: Color32) -> Vec<Option<Color32>> {
        vec![Some(c); COLUMNS]
    }

    #[test]
    fn map_tiles_higher_layer_wins() {
        let mut store = TileStore::default();
        let t = IVec2::new(-1, 3);
        assert!(store.apply(t, 2, &solid(Color32::RED)));
        assert!(!store.apply(t, 1, &solid(Color32::BLUE)));
        assert_eq!(store.tiles[&t].colour[0], Color32::RED);
        assert!(store.apply(t, 3, &solid(Color32::GREEN)));
        assert_eq!(store.tiles[&t].colour[7], Color32::GREEN);
    }

    #[test]
    fn map_tiles_dug_out_layer_yields_to_layer_below() {
        let mut store = TileStore::default();
        let t = IVec2::ZERO;
        store.apply(t, 2, &solid(Color32::RED));
        store.apply(t, 2, &vec![None; COLUMNS]);
        assert!(store.apply(t, 1, &solid(Color32::BLUE)));
        assert_eq!(store.tiles[&t].colour[0], Color32::BLUE);
    }

    #[test]
    fn map_tiles_skip_empty_chunks_and_saved_tiles_yield() {
        let mut store = TileStore::default();
        assert!(!store.apply(IVec2::ONE, 0, &vec![None; COLUMNS]));
        assert!(!store.is_mapped(IVec2::ONE));
        store.insert_saved(IVec2::ZERO, vec![Color32::WHITE; COLUMNS]);
        assert!(store.apply(IVec2::ZERO, -40, &solid(Color32::RED)));
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn map_tiles_dirty_bits_cover_negative_tiles() {
        let mut store = TileStore::default();
        store.apply(IVec2::new(-1, -1), 0, &solid(Color32::RED));
        let region = &store.regions[&IVec2::new(-1, -1)];
        assert_eq!(region.dirty, 1 << 63);
        assert_eq!(region_of(IVec2::new(-9, 8)), IVec2::new(-2, 1));
    }

    #[test]
    fn map_tiles_evict_farthest_regions_first() {
        let mut store = TileStore::default();
        for x in -40..40 {
            store.apply(IVec2::new(x, 0), 0, &solid(Color32::RED));
        }
        store.evict_far(IVec2::new(30, 0), 24);
        assert!(store.len() <= 24);
        assert!(store.is_mapped(IVec2::new(30, 0)));
        assert!(!store.is_mapped(IVec2::new(-40, 0)));
    }
}
