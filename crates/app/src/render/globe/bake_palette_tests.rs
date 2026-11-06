//! Baked surfaces against the globe shader palette.

use super::*;
use engine_core::VoxelScalePreset;
use engine_voxel::WorldType;
use std::collections::BTreeSet;

const SHADER: &str = include_str!("../../shaders/globe_shader.wgsl");

fn palette_len() -> usize {
    let decl = SHADER.split("const GLOBE_PALETTE: array<vec3<f32>, ").nth(1).expect("palette declaration");
    decl[..decl.find('>').expect("palette length")].parse().expect("palette length")
}

#[test]
fn every_mapped_block_has_its_own_palette_slot() {
    let fallback = palette_len() - 1;
    assert!(SHADER.contains(&format!("const GLOBE_FALLBACK: u32 = {fallback}u;")), "fallback is the last of {} slots", fallback + 1);
    for &block in engine_content::ALL_BLOCK_IDS {
        let index = material_index(block);
        assert!(index == UNMAPPED || usize::from(index) < fallback, "{block:?} maps to {index}, past the palette");
    }
}

fn tiles(world_type: WorldType) -> Vec<TileId> {
    let mut tiles: Vec<TileId> = (0..6).map(TileId::root).collect();
    let exact: &[(u8, u32, u32)] = match world_type {
        WorldType::Planet => &[(0, 33, 20), (2, 12, 40), (4, 50, 9)],
        _ => &[(1, 33, 20)],
    };
    tiles.extend(exact.iter().map(|&(face, x, y)| TileId { face, level: 6, x, y }));
    tiles
}

// A block without a colour is drawn as plain grey from orbit, with no warning.
#[test]
fn baked_planet_and_hills_surfaces_all_have_globe_colours() {
    let mut missing = BTreeSet::new();
    for world_type in [WorldType::Planet, WorldType::Hills] {
        let cfg = WorldSampleConfig {
            planet: None,
            generator_version: engine_voxel::CURRENT_GENERATOR_VERSION,
            seed: 74,
            world_type,
            voxel_scale: VoxelScalePreset::OneMeter,
        };
        for id in tiles(world_type) {
            let mut data = TileData::empty(id);
            assert!(bake_tile(&cfg, id, &mut data, &mut BakeCursor::default(), Budget::UNBOUNDED));
            for (k, &m) in data.material.iter().enumerate() {
                if m == UNMAPPED {
                    let (x, z) = sample_coord(&cfg, id, k as u32 % TILE_SAMPLES, k as u32 / TILE_SAMPLES);
                    // The exact column names the block; coarse tiles may filter to a neighbour.
                    missing.insert(format!("{:?}", engine_worldgen::sample_column(&cfg, x, z).surface));
                }
            }
        }
    }
    assert!(missing.is_empty(), "surfaces without a globe colour: {missing:?}");
}
