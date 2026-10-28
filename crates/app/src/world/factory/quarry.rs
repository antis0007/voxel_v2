//! Drill footprint: a 3 m shaft, or a 9 m or 15 m quarry, chosen by mode.
//! The excavation cursor walks one layer row by row, then steps down.
use glam::IVec3;

/// Footprint width in metres for each drill mode.
pub const WIDTHS: [i32; 3] = [3, 9, 15];
pub const DEPTH_M: i32 = 24;

pub fn width_m(mode: u32) -> i32 {
    WIDTHS[(mode as usize).min(WIDTHS.len() - 1)]
}

/// Cells in one layer.
pub fn layer(size: i32, mode: u32) -> i32 {
    let w = width_m(mode) * size;
    w * w
}

/// Cursor value at which the whole volume is excavated.
pub fn limit(size: i32, mode: u32) -> i32 {
    layer(size, mode) * DEPTH_M * size
}

/// Cursor position relative to the drill anchor, in cells.
pub fn offset(index: i32, size: i32, mode: u32) -> IVec3 {
    let w = width_m(mode) * size;
    let half = width_m(mode) / 2 * size;
    IVec3::new(index % w - half, -1 - index / (w * w), (index / w) % w - half)
}

/// Whole metres below the anchor layer that the head has reached.
pub fn depth_m(index: i32, size: i32, mode: u32) -> i32 {
    (index / layer(size, mode)) / size + 1
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
            assert_eq!(limit(size, 0), layer * 24 * size);
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
        assert_eq!(limit(size, 2), w * w * 24);
    }

    #[test]
    fn depth_counts_whole_metres_at_fine_cell_sizes() {
        assert_eq!(depth_m(0, 4, 1), 1);
        assert_eq!(depth_m(layer(4, 1) * 4 - 1, 4, 1), 1);
        assert_eq!(depth_m(layer(4, 1) * 4, 4, 1), 2);
    }
}
