//! Which chunks a client holds, in what order new ones go, and whether each
//! needs sending at all (design section 6).
//!
//! Chunks within a horizontal and a vertical radius are wanted. Unloading waits
//! one chunk past that, so walking along a boundary does not load and drop the
//! same row each step. New chunks are ordered nearest first, with chunks behind
//! the camera counted up to twice as far, so what the player faces fills in
//! first. Each wanted chunk is then generated, read from the client's cache, or
//! sent, depending on its version and the client's own record.
use glam::{IVec3, Vec3};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Radius {
    pub horizontal: i32,
    pub vertical: i32,
}

impl Radius {
    fn contains(self, offset: IVec3, grow: i32) -> bool {
        let (h, v) = ((self.horizontal + grow).max(1) as f32, (self.vertical + grow).max(1) as f32);
        let d = offset.as_vec3();
        (d.x * d.x + d.z * d.z) / (h * h) + d.y * d.y / (v * v) <= 1.0
    }
}

/// Load and unload lists from one update.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Plan {
    /// Newly wanted chunks, most important first.
    pub load: Vec<IVec3>,
    pub unload: Vec<IVec3>,
}

/// Server side, one per client.
#[derive(Default)]
pub struct ChunkTracker {
    held: HashSet<IVec3>,
}

impl ChunkTracker {
    pub fn held(&self) -> usize {
        self.held.len()
    }

    /// Recomputes the wanted set around `center`, looking along `look`.
    pub fn update(&mut self, center: IVec3, look: Vec3, radius: Radius) -> Plan {
        let mut plan = Plan::default();
        let look = look.normalize_or_zero();
        let mut load: Vec<(f32, IVec3)> = Vec::new();
        let (h, v) = (radius.horizontal, radius.vertical);
        for y in -v..=v {
            for z in -h..=h {
                for x in -h..=h {
                    let offset = IVec3::new(x, y, z);
                    if !radius.contains(offset, 0) || self.held.contains(&(center + offset)) {
                        continue;
                    }
                    let d = offset.as_vec3();
                    let facing = if d == Vec3::ZERO { 1.0 } else { look.dot(d.normalize()) };
                    // Behind the camera counts up to twice the distance.
                    load.push((d.length_squared() * (1.5 - 0.5 * facing).powi(2), center + offset));
                }
            }
        }
        load.sort_by(|a, b| a.0.total_cmp(&b.0).then((a.1.x, a.1.y, a.1.z).cmp(&(b.1.x, b.1.y, b.1.z))));
        plan.load = load.into_iter().map(|(_, c)| c).collect();
        plan.unload = self.held.iter().filter(|c| !radius.contains(**c - center, 1)).copied().collect();
        plan.unload.sort_by_key(|c| (c.x, c.y, c.z));
        for c in &plan.unload {
            self.held.remove(c);
        }
        self.held.extend(plan.load.iter().copied());
        plan
    }

    /// The client dropped a chunk on its own (memory pressure); it is offered again next update.
    pub fn forget(&mut self, chunk: IVec3) {
        self.held.remove(&chunk);
    }
}

/// How one wanted chunk reaches the client.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Delivery {
    /// Never edited: the client generates it and checks this hash.
    Generate { hash: u64 },
    /// The client's cached copy is current.
    Cached,
    /// Edited and not cached at this version: send it.
    Send,
}

/// Chooses delivery from the server's edit version (0 = never edited), the
/// pristine hash, and the version the client says its cache holds.
pub fn delivery(version: u32, pristine_hash: u64, cached: Option<u32>) -> Delivery {
    match (version, cached) {
        (0, _) => Delivery::Generate { hash: pristine_hash },
        (v, Some(c)) if c == v => Delivery::Cached,
        _ => Delivery::Send,
    }
}

/// Client cache index offered on join: chunk to version, at most `limit` entries.
pub fn cache_offer(index: &HashMap<IVec3, u32>, center: IVec3, limit: usize) -> Vec<(IVec3, u32)> {
    let mut near: Vec<(IVec3, u32)> = index.iter().map(|(&c, &v)| (c, v)).collect();
    near.sort_by_key(|(c, _)| {
        // Squares of i32 differences need 64 bits, and their sum one more.
        let d = (c.as_i64vec3() - center.as_i64vec3()).to_array().map(i128::from);
        (d.iter().map(|v| v * v).sum::<i128>(), c.x, c.y, c.z)
    });
    near.truncate(limit);
    near
}

#[cfg(test)]
mod tests;
