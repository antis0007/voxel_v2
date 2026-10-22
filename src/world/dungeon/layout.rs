//! Bounded room-graph planning, cached with each complex; never per voxel.
use super::{Kind, Room, link};

pub(super) const FIRST_VERSION: u32 = 28;

/// Select a non-tree edge before lowering dead ends. Counting loop endpoints
/// toward degree prevents the height pass from silently removing the loop.
pub(super) fn complete_loop(
    edges: &[(f32, usize, usize)], tree: &[(usize, usize)], extra: &mut Vec<(usize, usize)>,
) {
    if extra.is_empty()
        && let Some(&(_, a, b)) = edges.iter().find(|(_, a, b)| !tree.contains(&(*a, *b))) {
            extra.push((a, b));
        }
}

/// Prefer an ordinary terminal room, then an ordinary room on a loop. An
/// all-puzzle complex uses a terminal/loop fallback. Cost follows the actual
/// corridor legs rather than measuring through intervening walls.
pub(super) fn objective(
    rooms: &[Room], tree: &[(usize, usize)], extra: &[(usize, usize)], entrance: usize,
) -> usize {
    let n = rooms.len();
    let mut distances = vec![f32::INFINITY; n];
    let mut visited = vec![false; n];
    let mut degree = vec![0usize; n];
    for &(a, b) in tree.iter().chain(extra) { degree[a] += 1; degree[b] += 1; }
    distances[entrance] = 0.0;
    for _ in 0..n {
        let Some(current) = (0..n).filter(|&i| !visited[i])
            .min_by(|&a, &b| distances[a].total_cmp(&distances[b]).then(a.cmp(&b))) else { break; };
        if !distances[current].is_finite() { break; }
        visited[current] = true;
        for &(a, b) in tree.iter().chain(extra) {
            let next = if a == current { b } else if b == current { a } else { continue; };
            let ra = &rooms[a];
            let rb = &rooms[b];
            let horizontal = (ra.centre().0-rb.centre().0).abs()
                >= (ra.centre().1-rb.centre().1).abs();
            let inside = if horizontal { (ra.x1-ra.x0 + rb.x1-rb.x0) * 0.5 }
                else { (ra.z1-ra.z0 + rb.z1-rb.z0) * 0.5 };
            let cost = link(rooms, a, b, None).hypot(ra.floor-rb.floor) + inside;
            distances[next] = distances[next].min(distances[current] + cost);
        }
    }
    for ordinary_only in [true, false] {
        for terminal_only in [true, false] {
            if let Some(index) = (0..n).filter(|&i| {
                i != entrance && distances[i].is_finite() && (!terminal_only || degree[i] == 1)
                    && (!ordinary_only || !matches!(rooms[i].kind,
                        Kind::Cosmos | Kind::Labyrinth | Kind::Pitfall))
            }).max_by(|&a, &b| distances[a].total_cmp(&distances[b]).then(a.cmp(&b))) {
                return index;
            }
        }
    }
    entrance // Defensive only: the caller's spanning tree connects all rooms.
}
