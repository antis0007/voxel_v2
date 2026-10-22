//! Chunk-partitioned partial liquid volumes. Paging/snapshots visit selected
//! chunks rather than scanning every active liquid cell in the world.
use super::{BlockId, World, chunk_coord_from_world_block};
use crate::fast_hash::FastMap;
use glam::{IVec3, Vec2, Vec3};
use std::sync::Arc;
/// Find the exposed top of the water column containing, or immediately around,
/// a metric point. The returned Y includes the partial cell's eight-unit
/// volume, so a splash ring sits on the rendered surface rather than the top
/// of the containing voxel. Deep interior points are rejected instead of
/// producing a fake contact ring far above the actual impact.
pub fn surface_at(world: &World, point: Vec3, scale: f32) -> Option<Vec3> {
    if !point.is_finite() || !scale.is_finite() || scale <= 1e-6 {
        return None;
    }
    let base = (point / scale).floor().as_ivec3();
    let mut column = None;
    // A projectile may cross the surface between substeps, so probe the
    // containing cell and its two vertical neighbours before giving up.
    for dy in [0, -1, 1, -2] {
        let cell = base + IVec3::Y * dy;
        if world.get(cell) == BlockId::Water && world.water_volume(cell) > 0 {
            column = Some(cell);
            break;
        }
    }
    let mut top = column?;
    let mut climbed = 0;
    for _ in 0..24 {
        let above = top + IVec3::Y;
        if world.get(above) != BlockId::Water || world.water_volume(above) == 0 {
            break;
        }
        top = above;
        climbed += 1;
    }
    if climbed == 24 || world.get(top + IVec3::Y).is_solid() {
        return None;
    }
    let volume = world.water_volume(top);
    if volume == 0 {
        return None;
    }
    Some(Vec3::new(
        point.x,
        (top.y as f32 + volume as f32 / 8.0) * scale,
        point.z,
    ))
}

/// Immutable partial-volume view. Unchanged chunks share their cell maps with
/// the live world; publishing a render snapshot copies chunk handles only.
#[derive(Clone, Default)]
pub struct WaterSnapshot {
    chunks: FastMap<IVec3, Arc<FastMap<IVec3, u8>>>,
}
impl WaterSnapshot {
    pub fn get(&self, p: &IVec3) -> Option<&u8> {
        self.chunks.get(&chunk_coord_from_world_block(*p))?.get(p)
    }
    pub fn contains_key(&self, p: &IVec3) -> bool {
        self.get(p).is_some()
    }
    pub fn iter(&self) -> impl Iterator<Item = (&IVec3, &u8)> {
        self.chunks.values().flat_map(|m| m.iter())
    }
    pub fn keys(&self) -> impl Iterator<Item = &IVec3> {
        self.iter().map(|(p, _)| p)
    }
    pub fn in_box(&self, min: IVec3, max: IVec3) -> impl Iterator<Item = (&IVec3, &u8)> + '_ {
        let lo = chunk_coord_from_world_block(min);
        let hi = chunk_coord_from_world_block(max - IVec3::ONE);
        (lo.y..=hi.y)
            .flat_map(move |y| (lo.z..=hi.z).flat_map(move |z|
                (lo.x..=hi.x).map(move |x| IVec3::new(x, y, z))))
            .filter_map(|c| self.chunks.get(&c))
            .flat_map(|m| m.iter())
            .filter(move |(p, _)| p.cmpge(min).all() && p.cmplt(max).all())
    }
    /// Compare immutable chunk handles before inspecting partial-volume cells.
    /// The returned maps are old/new, including chunk additions and removals.
    pub fn changed_chunks<'a>(&'a self, old: &'a Self) -> impl Iterator<Item = (
        IVec3, Option<&'a FastMap<IVec3, u8>>, Option<&'a FastMap<IVec3, u8>>,
    )> + 'a {
        let changed = self.chunks.iter().filter_map(move |(&q, current)| {
            let previous = old.chunks.get(&q);
            if previous.is_some_and(|p| Arc::ptr_eq(p, current)) {
                None
            } else {
                Some((q, previous.map(AsRef::as_ref), Some(current.as_ref())))
            }
        });
        let removed = old.chunks.iter().filter_map(move |(&q, previous)| {
            (!self.chunks.contains_key(&q)).then_some((q, Some(previous.as_ref()), None))
        });
        changed.chain(removed)
    }
}

#[derive(Default)]
pub struct WaterLevels {
    chunks: FastMap<IVec3, Arc<FastMap<IVec3, u8>>>,
    capacity: usize,
    /// Runtime-only horizontal momentum in volume units per simulation tick.
    /// It is deliberately outside the checkpoint format: it describes a
    /// transient wave, not authoritative fluid mass. Partitioned by chunk so
    /// dropping a chunk's waves is one removal, not a scan of every wave.
    motion: FastMap<IVec3, FastMap<IVec3, Vec2>>,
    motion_cells: usize,
}
impl WaterLevels {
    const MAX_MOTION_CELLS: usize = 65_536;

    pub fn get(&self, p: &IVec3) -> Option<&u8> {
        self.chunks.get(&chunk_coord_from_world_block(*p))?.get(p)
    }

    pub fn velocity(&self, p: IVec3) -> Vec2 {
        self.motion
            .get(&chunk_coord_from_world_block(p))
            .and_then(|m| m.get(&p))
            .copied()
            .unwrap_or(Vec2::ZERO)
    }

    pub fn set_velocity(&mut self, p: IVec3, v: Vec2) {
        if !v.is_finite() || v.length_squared() < 0.0025 {
            self.remove_velocity(&p);
            return;
        }
        let c = chunk_coord_from_world_block(p);
        let room = self.motion_cells < Self::MAX_MOTION_CELLS;
        if !room && !self.motion.get(&c).is_some_and(|m| m.contains_key(&p)) {
            return;
        }
        let m = self.motion.entry(c).or_default();
        if m.insert(p, v.clamp_length_max(6.0)).is_none() {
            self.motion_cells += 1;
        }
    }

    pub fn remove_velocity(&mut self, p: &IVec3) {
        let c = chunk_coord_from_world_block(*p);
        let Some(m) = self.motion.get_mut(&c) else {
            return;
        };
        if m.remove(p).is_some() {
            self.motion_cells -= 1;
            if m.is_empty() {
                self.motion.remove(&c);
            }
        }
    }
    pub fn insert(&mut self, p: IVec3, v: u8) -> Option<u8> {
        let m = self
            .chunks
            .entry(chunk_coord_from_world_block(p))
            .or_default();
        if m.get(&p) == Some(&v) {
            return Some(v);
        }
        self.capacity -= m.capacity();
        let old = Arc::make_mut(m).insert(p, v);
        self.capacity += m.capacity();
        old
    }
    pub fn remove(&mut self, p: &IVec3) -> Option<u8> {
        let c = chunk_coord_from_world_block(*p);
        let m = self.chunks.get_mut(&c)?;
        // A full-volume cell has no partial entry. Removing it must not copy
        // a shared chunk map when there is no mutation to publish.
        m.get(p)?;
        let previous_capacity = m.capacity();
        let old = Arc::make_mut(m).remove(p);
        self.capacity = self.capacity - previous_capacity + m.capacity();
        if m.is_empty() {
            self.remove_chunk(c);
        }
        old
    }
    pub fn remove_chunk(&mut self, c: IVec3) {
        if let Some(m) = self.chunks.remove(&c) {
            self.capacity -= m.capacity();
        }
        if let Some(m) = self.motion.remove(&c) {
            self.motion_cells -= m.len();
        }
    }
    pub fn chunk(&self, c: IVec3) -> Option<&FastMap<IVec3, u8>> {
        self.chunks.get(&c).map(AsRef::as_ref)
    }
    pub fn capacity(&self) -> usize {
        self.capacity + self.chunks.capacity()
    }
    pub fn iter(&self) -> impl Iterator<Item = (&IVec3, &u8)> {
        self.chunks.values().flat_map(|m| m.iter())
    }
    pub fn extend(&mut self, items: impl IntoIterator<Item = (IVec3, u8)>) {
        for (p, v) in items {
            self.insert(p, v);
        }
    }
    pub fn snapshot(&self, coords: impl Iterator<Item = IVec3>) -> WaterSnapshot {
        WaterSnapshot {
            chunks: coords.filter_map(|c| self.chunks.get(&c).map(|m| (c, Arc::clone(m)))).collect(),
        }
    }
    pub fn snapshot_all(&self) -> WaterSnapshot {
        WaterSnapshot { chunks: self.chunks.clone() }
    }
}
