//! Footprints on the room lattice, and fitting one inside the cell that owns
//! the complex: `dungeon::block` only asks the owning cell, so a room past
//! the cell edge would be cut off mid-wall.
use crate::dungeon::{CELL, cell_index};

/// Lattice spacing between room centres, metres.
pub const SLOT: f32 = 34.0;
/// Furthest a room, its shell and its corridor pads reach from its slot centre.
const REACH: f32 = 12.0 + super::WALL + 3.0;

/// Footprints on a small lattice; each is connected.
const SHAPES: [&[(i32, i32)]; 6] = [
    &[(0, 0), (1, 0), (0, 1), (1, 1), (0, 2), (1, 2)],
    &[(0, 0), (1, 0), (2, 0), (3, 0), (0, 1), (1, 1), (2, 1), (3, 1), (0, 2), (1, 2), (2, 2), (3, 2)],
    &[(0, 0), (0, 1), (0, 2), (0, 3), (1, 3), (2, 3), (1, 2)],
    &[(0, 0), (1, 0), (2, 0), (0, 1), (2, 1), (0, 2), (1, 2), (2, 2)],
    &[(0, 1), (1, 1), (2, 1), (3, 1), (0, 0), (2, 2), (3, 0), (1, 2)],
    &[(1, 0), (0, 1), (1, 1), (2, 1), (1, 2), (1, 3), (0, 3)],
];

/// Lattice cells and the entry's index. Tries every turn and entry, starting
/// from the hashed ones, for a placement whose rooms all stay inside the
/// owning cell; failing that, keeps the cells that fit and are still joined
/// to the entry.
pub fn fit(shape: f32, turn: f32, entry: f32, cx: f32, cz: f32) -> (Vec<(i32, i32)>, usize) {
    let base = SHAPES[(shape * SHAPES.len() as f32) as usize % SHAPES.len()];
    let n = base.len();
    let (t0, e0) = ((turn * 8.0) as usize % 8, (entry * n as f32) as usize % n);
    let (ox, oz) = (cell_index(cx) as f32 * CELL, cell_index(cz) as f32 * CELL);
    let lim = CELL * 0.5 - REACH;
    let fits = |cells: &[(i32, i32)], e: usize, i: usize| {
        let x = cx + (cells[i].0 - cells[e].0) as f32 * SLOT - ox;
        let z = cz + (cells[i].1 - cells[e].1) as f32 * SLOT - oz;
        x.abs() <= lim && z.abs() <= lim
    };
    for k in 0..8 * n {
        let cells = turned(base, (t0 + k / n) % 8);
        let e = (e0 + k) % n;
        if (0..n).all(|i| fits(&cells, e, i)) {
            return (cells, e);
        }
    }
    let cells = turned(base, t0);
    let mut keep = vec![false; n];
    keep[e0] = true;
    let mut queue = vec![e0];
    while let Some(i) = queue.pop() {
        for j in 0..n {
            let (p, q) = (cells[i], cells[j]);
            if !keep[j] && (p.0 - q.0).abs() + (p.1 - q.1).abs() == 1 && fits(&cells, e0, j) {
                keep[j] = true;
                queue.push(j);
            }
        }
    }
    let entry_cell = cells[e0];
    let kept: Vec<_> = cells.into_iter().zip(keep).filter_map(|(c, k)| k.then_some(c)).collect();
    let e = kept.iter().position(|&c| c == entry_cell).unwrap_or(0);
    (kept, e)
}

fn turned(cells: &[(i32, i32)], t: usize) -> Vec<(i32, i32)> {
    cells
        .iter()
        .map(|&(i, j)| {
            let (i, j) = if t & 4 != 0 { (-i, j) } else { (i, j) };
            match t & 3 {
                0 => (i, j),
                1 => (j, -i),
                2 => (-i, -j),
                _ => (-j, i),
            }
        })
        .collect()
}
