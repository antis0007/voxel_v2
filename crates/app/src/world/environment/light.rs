//! Derived direct-sky availability, cached per 3D substrate surface. Geometry
//! edits invalidate only this vertical column. No light inventory is stored.
use super::*;
pub const MAX_QUERIES: usize = 4_096;
#[derive(Clone, Copy)]
pub struct Query {
    pub next_y: i32,
    pub leaf_cells: u32,
    pub shade: Option<u32>,
}
impl State {
    pub fn can_invalidate_light(&self, p: IVec3) -> bool {
        let additional = self
            .light
            .range((p.x, i32::MIN, i32::MIN)..=(p.x, i32::MAX, i32::MAX))
            .filter(|(k, _)| k.2 == p.z && k.1 < p.y && !self.scheduled.contains_key(*k))
            .count();
        self.scheduled.len() + additional + 7 <= MAX_EVENTS
    }
    pub fn invalidate_light(&mut self, p: IVec3) {
        self.invalidate_sky(p);
        let lo = (p.x, i32::MIN, i32::MIN);
        let hi = (p.x, i32::MAX, i32::MAX);
        let keys: Vec<_> =
            self.light.range(lo..=hi).filter(|(k, _)| k.2 == p.z && k.1 < p.y).map(|(&k, _)| k).collect();
        for k in keys {
            self.light.remove(&k);
            self.wake(position(k));
        }
    }
}
