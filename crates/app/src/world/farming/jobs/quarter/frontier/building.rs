//! Paid two-storey shells with a clear switchback and upper entrance.
use std::collections::BTreeSet;
pub type Cell = [i32; 3];

pub fn plan(width: i32, depth: i32, right: bool, variant: u32) -> Result<Vec<Cell>, String> {
    if !(4..=7).contains(&width) || !(6..=8).contains(&depth) {
        return Err("Two-storey building requires an interior upper landing".into());
    }
    let mut cells = BTreeSet::new();
    let rear_window = 1 + (variant % (width - 2) as u32) as i32;
    let side = if right { width - 1 } else { 0 };
    for y in 0..=6 { for z in 0..depth { for x in 0..width {
        let wall = x == 0 || x == width - 1 || z == 0 || z == depth - 1;
        let ground_door = x == width / 2 && z == 0 && (y == 1 || y == 2);
        let upper_door = x == side && z == 4 && (y == 4 || y == 5);
        let window = (y == 2 || y == 5) && ((z == depth / 2 && (x == 0 || x == width - 1))
            || (z == depth - 1 && x == rear_window));
        if y % 3 == 0 || (wall && !ground_door && !upper_door && !window) {
            cells.insert([x, y, z]);
        }
    }}}
    let at = |x, y, z| [if right { width - 1 - x } else { x }, y, z];
    for x in [-2, -1] { for step in 0..=3 { for y in 0..=step {
        cells.insert(at(x, y, 1 + step));
    }}}
    // A two-cell-thick return flight is supported from the first landing.
    // Its lower riser touches the previous tread before the upper cell is paid.
    for x in [-4, -3] { for step in 0..=3 { for y in (2 + step)..=(3 + step) {
        cells.insert(at(x, y, 4 - step));
    }}}
    // The bridge clears the lower flight by five metres.
    for x in [-2, -1] { cells.insert(at(x, 6, 1)); }
    Ok(cells.into_iter().collect())
}

#[cfg(test)]
#[path = "building_tests.rs"]
mod tests;
