//! Excavation geometry and block output for drills and standalone quarries.
//! The excavation cursor walks one layer row by row, then steps down.
use glam::IVec3;
use super::{Kind, Node, blocks::Material};
use crate::world::World;

/// Footprint width in metres for each drill mode.
pub const WIDTHS: [i32; 3] = [3, 9, 15];
pub const DEPTH_M: i32 = 24;
pub const MAX_DEPTH_M: i32 = 48;
pub const DIRECTIONS: [(&str, IVec3); 4] = [
    ("North", IVec3::NEG_Z),
    ("East", IVec3::X),
    ("South", IVec3::Z),
    ("West", IVec3::NEG_X),
];

pub fn direction(n: &Node) -> usize {
    DIRECTIONS.iter().position(|&(_, facing)| facing == n.facing).unwrap_or(0)
}

pub fn configured_depth(target: u32) -> i32 {
    if target == 0 { DEPTH_M } else { target.min(MAX_DEPTH_M as u32) as i32 }
}

pub fn width_m(mode: u32) -> i32 {
    WIDTHS[(mode as usize).min(WIDTHS.len() - 1)]
}

/// Cells in one layer.
pub fn layer(size: i32, mode: u32) -> i32 {
    let w = width_m(mode) * size;
    w * w
}

/// Cursor value at which the whole volume is excavated.
pub fn limit(size: i32, mode: u32, target: u32) -> i32 {
    layer(size, mode) * configured_depth(target) * size
}

pub(super) fn max_limit(size: i32, mode: u32) -> i32 {
    limit(size, mode, MAX_DEPTH_M as u32)
}

/// Cursor position relative to the drill anchor, in cells.
pub fn offset(index: i32, size: i32, mode: u32) -> IVec3 {
    let w = width_m(mode) * size;
    let half = width_m(mode) / 2 * size;
    IVec3::new(index % w - half, -1 - index / (w * w), (index / w) % w - half)
}

pub(super) fn cell(p: IVec3, n: &Node, size: i32) -> IVec3 {
    let centred = p + offset(n.excavation as i32, size, n.mode);
    if n.kind == Kind::Quarry {
        // The first edge of the pit starts beyond the controller's footprint.
        let shift = (width_m(n.mode) / 2 + 1) * size;
        centred + DIRECTIONS[direction(n)].1 * shift
    } else {
        centred
    }
}

/// Whole metres below the anchor layer that the head has reached.
pub fn depth_m(index: i32, size: i32, mode: u32) -> i32 {
    (index / layer(size, mode)) / size + 1
}

pub(super) enum Product {
    Sediment(super::placer::Feed),
    Resource(usize, u32),
    Block(Material),
}

impl Product {
    pub(super) fn fits(&self, n: &Node) -> bool {
        match *self {
            Self::Sediment(feed) => n.placer.feed.fits(feed),
            Self::Resource(i, count) => n.output[i] <= super::CAPACITY - count,
            Self::Block(material) => n.blocks.output[material.index()] < super::CAPACITY,
        }
    }

    pub(super) fn store(self, world: &mut World, n: &mut Node) {
        match self {
            Self::Sediment(feed) => n.placer.feed.add(feed),
            Self::Resource(i, count) => {
                n.output[i] += count;
                world.factory.stats.made(i, count);
            }
            Self::Block(material) => n.blocks.output[material.index()] += 1,
        }
    }
}

pub(super) fn product(
    world: &World, n: &Node, cell: IVec3, block: crate::world::BlockId, size: i32,
) -> Result<Option<Product>, &'static str> {
    use crate::world::{BlockId as B, expedition, travel::panning};
    use super::placer;

    if n.kind == Kind::Quarry && n.placer.enabled {
        let feed = if block == B::Paydirt {
            Some(placer::Feed::from_grade(placer::PAYDIRT_GRADE, placer::cell_volume(size)))
        } else {
            panning::wet_bed(world, cell, block)
                .and_then(|bed| panning::feed(world, bed))
                .map(|(_, feed)| feed)
        };
        return Ok(feed.map(Product::Sediment));
    }
    if block == B::Bedrock {
        return Err("Bedrock reached");
    }
    if block == B::Air {
        return Ok(None);
    }
    let shaft = n.kind == Kind::MiningDrill && n.mode == 0;
    if shaft && let Some(i @ (0..=3 | 5)) = expedition::resource(block) {
        return Ok(Some(Product::Resource(i, placer::cell_volume(size))));
    }
    Material::from_block(block)
        .filter(|m| !shaft || m.index() < Material::LEGACY_COUNT)
        .map(|m| Some(Product::Block(m)))
        .ok_or("Obstruction in shaft")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shaft_mode_keeps_the_original_cursor_layout() {
        for size in [1, 2, 4] {
            let width = 3 * size;
            let layer = width * width;
            for index in [0, 1, width, layer - 1, layer, layer * 5 + 7] {
                let old = IVec3::new(index % width - size, -1 - index / layer, (index / width) % width - size);
                assert_eq!(offset(index, size, 0), old);
            }
            assert_eq!(limit(size, 0, 0), layer * 24 * size);
        }
    }

    #[test]
    fn a_quarry_covers_its_square_once_per_layer_and_stays_centred() {
        let size = 1;
        let w = width_m(2);
        let mut seen = std::collections::HashSet::new();
        for index in 0..layer(size, 2) {
            let o = offset(index, size, 2);
            assert_eq!(o.y, -1);
            assert!(o.x.abs() <= w / 2 && o.z.abs() <= w / 2);
            assert!(seen.insert((o.x, o.z)));
        }
        assert_eq!(seen.len() as i32, w * w);
        assert_eq!(offset(layer(size, 2), size, 2).y, -2);
        assert_eq!(limit(size, 2, 0), w * w * 24);
    }

    #[test]
    fn depth_counts_whole_metres_at_fine_cell_sizes() {
        assert_eq!(depth_m(0, 4, 1), 1);
        assert_eq!(depth_m(layer(4, 1) * 4 - 1, 4, 1), 1);
        assert_eq!(depth_m(layer(4, 1) * 4, 4, 1), 2);
    }
}
