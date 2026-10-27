//! Top-down map colour per column of one generated chunk.
use crate::mesh::{CHUNK_PAD, GeneratedChunk, PADDED_CHUNK_SIZE};
use crate::world::{BlockId, CHUNK_SIZE};
use egui::Color32;
use engine_content::MatterState;
use glam::IVec3;

pub const COLUMNS: usize = (CHUNK_SIZE * CHUNK_SIZE) as usize;
const P: i32 = PADDED_CHUNK_SIZE;

fn index(x: i32, y: i32, z: i32) -> usize {
    ((y * P + z) * P + x) as usize
}

fn visible(block: BlockId) -> bool {
    !block.is_empty()
        && !matches!(
            block.material().physics.matter_state,
            MatterState::Vacuum | MatterState::Gas | MatterState::Plasma
        )
}

/// Highest visible cell of a padded column inside this chunk's own layers.
fn top_cell(chunk: &GeneratedChunk, x: i32, z: i32) -> Option<i32> {
    (CHUNK_PAD..CHUNK_PAD + CHUNK_SIZE)
        .rev()
        .find(|&y| chunk.blocks.get(index(x, y, z)).is_some_and(|b| visible(*b)))
}

fn roof_above(chunk: &GeneratedChunk, x: i32, z: i32) -> Option<i32> {
    let roof = *chunk.sky_ceiling.get((z * P + x) as usize)?;
    (roof > chunk.origin().y + CHUNK_PAD + CHUNK_SIZE - 1).then_some(roof)
}

/// Surface height of a padded column in world cells, for slope shading.
fn height(chunk: &GeneratedChunk, x: i32, z: i32) -> Option<i32> {
    if let Some(roof) = roof_above(chunk, x, z) {
        return Some(roof);
    }
    if let Some(y) = top_cell(chunk, x, z) {
        return Some(chunk.origin().y + y);
    }
    let roof = *chunk.sky_ceiling.get((z * P + x) as usize)?;
    Some(roof)
}

fn rgb(c: [f32; 3]) -> Color32 {
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color32::from_rgb(byte(c[0]), byte(c[1]), byte(c[2]))
}

/// Lit from the north-west: a rising slope brightens, a falling one darkens.
pub fn shade(base: [f32; 3], slope: f32) -> [f32; 3] {
    let factor = 1.0 + 0.22 * (slope * 0.9).tanh();
    base.map(|c| c * factor)
}

/// Water over a bed colour: cover and darkness both grow with depth in metres.
pub fn under_water(bed: Option<[f32; 3]>, depth_m: f32) -> [f32; 3] {
    let water = BlockId::Water.color(IVec3::ZERO);
    let depth = depth_m.max(0.0);
    let cover = bed.map_or(1.0, |_| 0.4 + 0.6 * (1.0 - (-depth * 0.3).exp()));
    let dark = 1.0 - 0.5 * (1.0 - (-depth / 20.0).exp());
    let bed = bed.unwrap_or(water);
    [0, 1, 2].map(|i| (bed[i] + (water[i] - bed[i]) * cover) * dark)
}

/// Colour for one column: `slope` is the height step per column (cells/cells).
pub fn colour(surface: BlockId, bed: Option<BlockId>, water_m: f32, slope: f32) -> Color32 {
    if surface == BlockId::Water {
        rgb(under_water(bed.map(|b| b.color(IVec3::ZERO)), water_m))
    } else {
        rgb(shade(surface.color(IVec3::ZERO), slope))
    }
}

/// One colour per column (x fastest), `None` where this chunk holds no surface.
pub fn scan(chunk: &GeneratedChunk) -> Vec<Option<Color32>> {
    let mut out = vec![None; COLUMNS];
    if chunk.blocks.len() != (P * P * P) as usize {
        return out;
    }
    let origin_y = chunk.origin().y;
    for lz in 0..CHUNK_SIZE {
        for lx in 0..CHUNK_SIZE {
            let (x, z) = (lx + CHUNK_PAD, lz + CHUNK_PAD);
            if roof_above(chunk, x, z).is_some() {
                continue;
            }
            let Some(top) = top_cell(chunk, x, z) else { continue };
            let surface = chunk.blocks[index(x, top, z)];
            let entry = &mut out[(lz * CHUNK_SIZE + lx) as usize];
            if surface == BlockId::Water {
                let floor = (CHUNK_PAD..top)
                    .rev()
                    .find(|&y| chunk.blocks[index(x, y, z)] != BlockId::Water);
                let bed = floor.map(|y| chunk.blocks[index(x, y, z)]).filter(|b| visible(*b));
                let bottom = match (floor, chunk.bed_height.get((z * P + x) as usize)) {
                    (Some(y), _) => origin_y + y,
                    (None, Some(&bed_y)) => bed_y.min(origin_y + CHUNK_PAD - 1),
                    (None, None) => origin_y + CHUNK_PAD - 1,
                };
                let depth_m = (origin_y + top - bottom) as f32 * chunk.voxel_meters;
                *entry = Some(colour(surface, bed, depth_m, 0.0));
                continue;
            }
            let here = origin_y + top;
            let step = |nx: i32, nz: i32| height(chunk, nx, nz).map_or(0.0, |h| (here - h) as f32);
            let slope = 0.5 * (step(x, z - 1) + step(x - 1, z));
            *entry = Some(colour(surface, None, 0.0, slope));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk_with(fill: impl Fn(i32, i32, i32) -> BlockId) -> GeneratedChunk {
        let mut blocks = vec![BlockId::Air; (P * P * P) as usize];
        for y in 0..P {
            for z in 0..P {
                for x in 0..P {
                    blocks[index(x, y, z)] = fill(x, y, z);
                }
            }
        }
        GeneratedChunk::new(IVec3::ZERO, blocks)
    }

    fn lum(c: Color32) -> u32 {
        c.r() as u32 + c.g() as u32 + c.b() as u32
    }

    #[test]
    fn map_colour_flat_ground_is_material_colour() {
        let grass = BlockId::Grass.color(IVec3::ZERO);
        assert_eq!(colour(BlockId::Grass, None, 0.0, 0.0), rgb(grass));
        assert!(lum(colour(BlockId::Grass, None, 0.0, 2.0)) > lum(colour(BlockId::Grass, None, 0.0, 0.0)));
        assert!(lum(colour(BlockId::Grass, None, 0.0, -2.0)) < lum(colour(BlockId::Grass, None, 0.0, 0.0)));
    }

    #[test]
    fn map_colour_water_darkens_with_depth() {
        let shallow = colour(BlockId::Water, Some(BlockId::Sand), 0.5, 0.0);
        let deep = colour(BlockId::Water, Some(BlockId::Sand), 30.0, 0.0);
        assert!(lum(deep) < lum(shallow));
        // A shallow sandy bed shows through as a lighter, warmer tint.
        assert!(shallow.r() > deep.r());
    }

    #[test]
    fn map_scan_reads_surface_and_water() {
        // Grass at local y 10 for x < 17, water 4 deep over sand elsewhere.
        let chunk = chunk_with(|x, y, _| match (x < 17, y) {
            (_, 0..=5) => BlockId::Stone,
            (true, 6..=10) => BlockId::Grass,
            (false, 6) => BlockId::Sand,
            (false, 7..=10) => BlockId::Water,
            _ => BlockId::Air,
        });
        let cols = scan(&chunk);
        assert_eq!(cols[5], Some(colour(BlockId::Grass, None, 0.0, 0.0)));
        assert_eq!(cols[20], Some(colour(BlockId::Water, Some(BlockId::Sand), 4.0, 0.0)));
    }

    #[test]
    fn map_scan_skips_columns_roofed_above_the_chunk() {
        let mut chunk = chunk_with(|_, _, _| BlockId::Stone);
        chunk.sky_ceiling = vec![200; (P * P) as usize];
        assert!(scan(&chunk).iter().all(Option::is_none));
        chunk.sky_ceiling = vec![CHUNK_SIZE - 1; (P * P) as usize];
        assert!(scan(&chunk).iter().all(Option::is_some));
    }

    #[test]
    fn map_scan_empty_chunk_has_no_surface() {
        let chunk = chunk_with(|_, _, _| BlockId::Air);
        assert!(scan(&chunk).iter().all(Option::is_none));
    }
}
