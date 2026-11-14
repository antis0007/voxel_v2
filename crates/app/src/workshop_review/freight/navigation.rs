use super::*;
use std::collections::HashMap;

/// Search the terrain the player can actually stand on when the direct bearing
/// hits a ledge. Movement still runs through the ordinary player controller.
pub(super) fn detour(w: &World, p: &Player, goal: Vec3) -> VecDeque<Vec3> {
    let start = p.pos.floor().as_ivec3();
    let horizontal = |q: IVec3| (q.as_vec3() + Vec3::splat(0.5) - goal).xz().length();
    let mut frontier = VecDeque::from([start]);
    let mut previous = HashMap::from([(start, start)]);
    let mut best = start;
    while let Some(cell) = frontier.pop_front() {
        if horizontal(cell) < horizontal(best) { best = cell; }
        if horizontal(best) < 0.5 { break; }
        for side in [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z] {
            for dy in [0, 1, -1] {
                let next = cell + side + IVec3::Y * dy;
                let offset = (next - start).abs();
                if offset.x > 12 || offset.z > 12 || offset.y > 8 || previous.contains_key(&next) {
                    continue;
                }
                if w.get(next).is_solid() || w.get(next + IVec3::Y).is_solid()
                    || !w.get(next - IVec3::Y).is_solid() || (dy > 0 && w.get(cell + 2 * IVec3::Y).is_solid())
                {
                    continue;
                }
                previous.insert(next, cell);
                frontier.push_back(next);
            }
        }
    }
    let mut path = VecDeque::new();
    if horizontal(start) - horizontal(best) < 1.0 { return path; }
    while best != start {
        path.push_front(best.as_vec3() + Vec3::new(0.5, 0.0, 0.5));
        best = previous[&best];
    }
    path
}
